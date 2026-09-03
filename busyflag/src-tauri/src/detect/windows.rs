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

fn friendly(key: &str) -> String {
    if key.contains('#') {
        // C:#Program Files#Zoom#bin#Zoom.exe -> Zoom.exe
        key.rsplit('#').next().unwrap_or(key).to_string()
    } else {
        // Microsoft.Teams_8wekyb3d8bbwe -> Microsoft.Teams
        key.split('_').next().unwrap_or(key).to_string()
    }
}

fn active_in(kind: &str, cfg: &Config) -> Vec<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(root) = hkcu.open_subkey(format!(r"{STORE}\{kind}")) else { return vec![] };
    let mut out = vec![];
    let mut walk = |key: &RegKey, prefix: &str| {
        for name in key.enum_keys().flatten() {
            if let Ok(sub) = key.open_subkey(&name) {
                if let Ok(stop) = sub.get_value::<u64, _>("LastUsedTimeStop") {
                    if stop == 0 {
                        let n = friendly(&name);
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

pub struct WinDetector;

impl WinDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Detector for WinDetector {
    fn mic_active(&mut self, cfg: &Config) -> Vec<String> {
        active_in("microphone", cfg)
    }

    fn camera_active(&mut self, cfg: &Config) -> Vec<String> {
        active_in("webcam", cfg)
    }

    fn screen_locked(&mut self) -> bool {
        session_locked()
    }
}
