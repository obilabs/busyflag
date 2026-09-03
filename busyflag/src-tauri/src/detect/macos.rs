//! macOS: CoreAudio (`kAudioDevicePropertyDeviceIsRunningSomewhere`) for the
//! microphone and CoreMediaIO (`kCMIODevicePropertyDeviceIsRunningSomewhere`)
//! for cameras. Read-only property queries; no TCC permission required.
//! Optionally the macOS 14+ process-object API attributes usage to apps.

use super::{dedup, Detector};
use crate::config::Config;
use std::ffi::c_void;
use std::os::raw::c_char;

#[repr(C)]
struct PropAddr {
    selector: u32,
    scope: u32,
    element: u32,
}

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

const SYSTEM_OBJECT: u32 = 1;
const PROP_DEVICES: u32 = fourcc(b"dev#");
const SCOPE_GLOBAL: u32 = fourcc(b"glob");
const SCOPE_INPUT: u32 = fourcc(b"inpt");
const PROP_STREAM_CONFIG: u32 = fourcc(b"slay");
const PROP_RUNNING_SOMEWHERE: u32 = fourcc(b"gone");
const PROP_NAME: u32 = fourcc(b"lnam");
const PROP_PROCESS_LIST: u32 = fourcc(b"prs#");
const PROP_PROCESS_PID: u32 = fourcc(b"ppid");
const PROP_PROCESS_BUNDLE: u32 = fourcc(b"pbid");
const PROP_PROCESS_INPUT_RUNNING: u32 = fourcc(b"piri");
const CF_UTF8: u32 = 0x0800_0100;

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(obj: u32, addr: *const PropAddr, qsize: u32, qdata: *const c_void, out: *mut u32) -> i32;
    fn AudioObjectGetPropertyData(obj: u32, addr: *const PropAddr, qsize: u32, qdata: *const c_void, iosize: *mut u32, out: *mut c_void) -> i32;
}

#[link(name = "CoreMediaIO", kind = "framework")]
extern "C" {
    fn CMIOObjectGetPropertyDataSize(obj: u32, addr: *const PropAddr, qsize: u32, qdata: *const c_void, out: *mut u32) -> i32;
    fn CMIOObjectGetPropertyData(obj: u32, addr: *const PropAddr, qsize: u32, qdata: *const c_void, size: u32, used: *mut u32, out: *mut c_void) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetCString(s: *const c_void, buf: *mut c_char, size: isize, enc: u32) -> bool;
    fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, enc: u32) -> *const c_void;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CFBooleanGetValue(b: *const c_void) -> bool;
    fn CFRelease(cf: *const c_void);
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGSessionCopyCurrentDictionary() -> *const c_void;
}

/// True when the login session's screen is locked (CGSSessionScreenIsLocked).
fn screen_is_locked() -> bool {
    unsafe {
        let dict = CGSessionCopyCurrentDictionary();
        if dict.is_null() {
            return false;
        }
        let key = CFStringCreateWithCString(std::ptr::null(), c"CGSSessionScreenIsLocked".as_ptr(), CF_UTF8);
        let val = CFDictionaryGetValue(dict, key);
        let locked = !val.is_null() && CFBooleanGetValue(val);
        CFRelease(key);
        CFRelease(dict);
        locked
    }
}

fn ca_get(obj: u32, selector: u32, scope: u32) -> Option<Vec<u8>> {
    let addr = PropAddr { selector, scope, element: 0 };
    let mut size: u32 = 0;
    unsafe {
        if AudioObjectGetPropertyDataSize(obj, &addr, 0, std::ptr::null(), &mut size) != 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if AudioObjectGetPropertyData(obj, &addr, 0, std::ptr::null(), &mut size, buf.as_mut_ptr() as *mut c_void) != 0 {
            return None;
        }
        buf.truncate(size as usize);
        Some(buf)
    }
}

fn cmio_get(obj: u32, selector: u32) -> Option<Vec<u8>> {
    let addr = PropAddr { selector, scope: SCOPE_GLOBAL, element: 0 };
    let mut size: u32 = 0;
    unsafe {
        if CMIOObjectGetPropertyDataSize(obj, &addr, 0, std::ptr::null(), &mut size) != 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let mut used: u32 = 0;
        if CMIOObjectGetPropertyData(obj, &addr, 0, std::ptr::null(), size, &mut used, buf.as_mut_ptr() as *mut c_void) != 0 {
            return None;
        }
        buf.truncate(used as usize);
        Some(buf)
    }
}

fn u32s(raw: &[u8]) -> Vec<u32> {
    raw.chunks_exact(4).map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn first_u32(raw: Option<Vec<u8>>) -> u32 {
    raw.filter(|r| r.len() >= 4).map(|r| u32::from_ne_bytes([r[0], r[1], r[2], r[3]])).unwrap_or(0)
}

/// Convert an owned CFStringRef (8 bytes) into a String, releasing it.
fn cfstring(raw: Option<Vec<u8>>) -> Option<String> {
    let r = raw?;
    if r.len() < 8 {
        return None;
    }
    let bytes: [u8; 8] = r[..8].try_into().ok()?;
    let ptr = usize::from_ne_bytes(bytes) as *const c_void;
    if ptr.is_null() {
        return None;
    }
    let mut buf = vec![0 as c_char; 512];
    unsafe {
        let ok = CFStringGetCString(ptr, buf.as_mut_ptr(), buf.len() as isize, CF_UTF8);
        CFRelease(ptr);
        if !ok {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

fn audio_devices() -> Vec<u32> {
    ca_get(SYSTEM_OBJECT, PROP_DEVICES, SCOPE_GLOBAL).map(|r| u32s(&r)).unwrap_or_default()
}

fn input_channels(dev: u32) -> u32 {
    // AudioBufferList: u32 mNumberBuffers, (pad), AudioBuffer mBuffers[] at offset 8,
    // each { u32 mNumberChannels, u32 mDataByteSize, void* mData } = 16 bytes.
    let Some(raw) = ca_get(dev, PROP_STREAM_CONFIG, SCOPE_INPUT) else { return 0 };
    if raw.len() < 4 {
        return 0;
    }
    let n = u32::from_ne_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    (0..n)
        .filter_map(|i| raw.get(8 + i * 16..12 + i * 16))
        .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .sum()
}

fn audio_device_name(dev: u32) -> String {
    cfstring(ca_get(dev, PROP_NAME, SCOPE_GLOBAL)).unwrap_or_else(|| format!("audio device {dev}"))
}

fn audio_running_somewhere(dev: u32) -> bool {
    first_u32(ca_get(dev, PROP_RUNNING_SOMEWHERE, SCOPE_GLOBAL)) != 0
}

/// Bundle ids (or pids) of processes CoreAudio says are running input. macOS 14+.
fn processes_running_input() -> Vec<String> {
    let Some(raw) = ca_get(SYSTEM_OBJECT, PROP_PROCESS_LIST, SCOPE_GLOBAL) else { return vec![] };
    u32s(&raw)
        .into_iter()
        .filter(|&p| first_u32(ca_get(p, PROP_PROCESS_INPUT_RUNNING, SCOPE_GLOBAL)) != 0)
        .map(|p| {
            let bundle = cfstring(ca_get(p, PROP_PROCESS_BUNDLE, SCOPE_GLOBAL)).unwrap_or_default();
            if bundle.is_empty() {
                format!("pid {}", first_u32(ca_get(p, PROP_PROCESS_PID, SCOPE_GLOBAL)))
            } else {
                bundle
            }
        })
        .collect()
}

fn cameras() -> Vec<u32> {
    cmio_get(SYSTEM_OBJECT, PROP_DEVICES).map(|r| u32s(&r)).unwrap_or_default()
}

fn camera_name(dev: u32) -> String {
    cfstring(cmio_get(dev, PROP_NAME)).unwrap_or_else(|| format!("camera {dev}"))
}

fn camera_running_somewhere(dev: u32) -> bool {
    first_u32(cmio_get(dev, PROP_RUNNING_SOMEWHERE)) != 0
}

pub struct MacDetector;

impl MacDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Detector for MacDetector {
    fn mic_active(&mut self, cfg: &Config) -> Vec<String> {
        let mut out: Vec<String> = audio_devices()
            .into_iter()
            .filter(|&d| input_channels(d) > 0 && audio_running_somewhere(d))
            .map(audio_device_name)
            .filter(|n| !cfg.ignores_device(n))
            .collect();
        // Attribution: which apps hold the mic. Only counts as busy on its own
        // when process_level_detection is on (Bluetooth headset workaround).
        let apps: Vec<String> = processes_running_input()
            .into_iter()
            .filter(|a| !cfg.ignores_app(a))
            .collect();
        if !out.is_empty() || cfg.process_level_detection {
            out.extend(apps);
        }
        dedup(out)
    }

    fn screen_locked(&mut self) -> bool {
        screen_is_locked()
    }

    fn camera_active(&mut self, cfg: &Config) -> Vec<String> {
        cameras()
            .into_iter()
            .filter(|&d| camera_running_somewhere(d))
            .map(camera_name)
            .filter(|n| !cfg.ignores_device(n))
            .collect()
    }
}
