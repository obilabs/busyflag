//! Windows 10 (1903+) / 11: the CapabilityAccessManager consent store records,
//! per app, when it last stopped using the microphone or webcam. A
//! `LastUsedTimeStop` of 0 means "in use right now". Packaged (Store) apps are
//! direct subkeys; classic Win32 apps live under `NonPackaged` with the exe
//! path encoded using `#` instead of `\`.

use super::{dedup, Detector};
use crate::config::Config;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;
use windows_sys::Win32::System::RemoteDesktop::{
    WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTSINFOEXW, WTS_CURRENT_SESSION,
    WTS_SESSIONSTATE_LOCK,
};

/// True when the current session is locked (WTSINFOEX SessionFlags, Windows 8+).
fn session_locked() -> bool {
    unsafe {
        let mut buf: *mut u16 = std::ptr::null_mut();
        let mut len: u32 = 0;
        let ok = WTSQuerySessionInformationW(std::ptr::null_mut(), WTS_CURRENT_SESSION, WTSSessionInfoEx, &mut buf, &mut len);
        if ok == 0 || buf.is_null() || (len as usize) < std::mem::size_of::<WTSINFOEXW>() {
            return false;
        }
        let info = &*(buf as *const WTSINFOEXW);
        let locked = info.Level == 1 && info.Data.WTSInfoExLevel1.SessionFlags as u32 == WTS_SESSIONSTATE_LOCK;
        WTSFreeMemory(buf as *mut _);
        locked
    }
}

const STORE: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";

use std::collections::HashMap;
use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

/// Names for executables whose version resource is missing or unhelpful, and for Store packages.
fn known_name(stem: &str) -> Option<&'static str> {
    Some(match stem.to_ascii_lowercase().as_str() {
        "msedge" => "Microsoft Edge",
        "chrome" => "Google Chrome",
        "firefox" => "Firefox",
        "opera" | "opera_gx" => "Opera",
        "brave" => "Brave",
        "ms-teams" | "teams" | "msteams" => "Microsoft Teams",
        "zoom" => "Zoom",
        "slack" => "Slack",
        "discord" => "Discord",
        "obs64" | "obs32" => "OBS Studio",
        "microsoft.windowscamera" => "Camera",
        "microsoft.windowssoundrecorder" => "Sound Recorder",
        "microsoft.skypeapp" => "Skype",
        "microsoftteams" => "Microsoft Teams",
        _ => return None,
    })
}

/// FileDescription (falling back to ProductName) from an exe's version resource.
fn version_description(path: &str) -> Option<String> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, buf.as_mut_ptr() as *mut _) == 0 {
            return None;
        }
        // First language/codepage pair in the translation table.
        let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let q: Vec<u16> = "\\VarFileInfo\\Translation".encode_utf16().chain(std::iter::once(0)).collect();
        if VerQueryValueW(buf.as_ptr() as *const _, q.as_ptr(), &mut ptr, &mut len) == 0 || len < 4 {
            return None;
        }
        let lang = *(ptr as *const u16);
        let cp = *(ptr as *const u16).add(1);
        for key in ["FileDescription", "ProductName"] {
            let q: Vec<u16> = format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\{key}").encode_utf16().chain(std::iter::once(0)).collect();
            let mut p: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut l = 0u32;
            if VerQueryValueW(buf.as_ptr() as *const _, q.as_ptr(), &mut p, &mut l) != 0 && l > 1 {
                let s = std::slice::from_raw_parts(p as *const u16, l as usize);
                let s = String::from_utf16_lossy(s).trim_end_matches('\0').trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}

/// Registry key name -> something a person recognises.
fn friendly(key: &str, cache: &mut HashMap<String, String>) -> String {
    if let Some(n) = cache.get(key) {
        return n.clone();
    }
    let name = if key.contains('#') {
        // NonPackaged: C:#Program Files#Zoom#bin#Zoom.exe
        let path = key.replace('#', "\\");
        let file = key.rsplit('#').next().unwrap_or(key);
        let stem = file.strip_suffix(".exe").or_else(|| file.strip_suffix(".EXE")).unwrap_or(file);
        known_name(stem)
            .map(str::to_string)
            .or_else(|| version_description(&path))
            .unwrap_or_else(|| stem.to_string())
    } else {
        // Packaged: Microsoft.WindowsCamera_8wekyb3d8bbwe
        let pkg = key.split('_').next().unwrap_or(key);
        known_name(pkg)
            .map(str::to_string)
            .unwrap_or_else(|| pkg.rsplit('.').next().unwrap_or(pkg).to_string())
    };
    cache.insert(key.to_string(), name.clone());
    name
}

fn active_in(kind: &str, cfg: &Config, cache: &mut HashMap<String, String>) -> Vec<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(root) = hkcu.open_subkey(format!(r"{STORE}\{kind}")) else { return vec![] };
    let mut out = vec![];
    let mut walk = |key: &RegKey, prefix: &str| {
        for name in key.enum_keys().flatten() {
            if let Ok(sub) = key.open_subkey(&name) {
                if let Ok(stop) = sub.get_value::<u64, _>("LastUsedTimeStop") {
                    if stop == 0 {
                        let n = friendly(&name, cache);
                        if !cfg.ignores_app(&n) && !cfg.ignores_app(&name) {
                            out.push(format!("{prefix}{n}"));
                        }
                    }
                }
            }
        }
    };
    walk(&root, "");
    if let Ok(np) = root.open_subkey("NonPackaged") {
        walk(&np, "");
    }
    dedup(out)
}

pub struct WinDetector {
    names: HashMap<String, String>,
}

impl WinDetector {
    pub fn new() -> Self {
        Self { names: HashMap::new() }
    }
}

impl Detector for WinDetector {
    fn mic_active(&mut self, cfg: &Config) -> Vec<String> {
        active_in("microphone", cfg, &mut self.names)
    }

    fn camera_active(&mut self, cfg: &Config) -> Vec<String> {
        active_in("webcam", cfg, &mut self.names)
    }

    fn screen_locked(&mut self) -> bool {
        session_locked()
    }
}
