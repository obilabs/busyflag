//! Platform detectors answer one question: who is using the microphone / camera right now?

use crate::config::Config;

pub trait Detector: Send {
    /// Names of devices/apps currently capturing audio. Empty = idle.
    fn mic_active(&mut self, cfg: &Config) -> Vec<String>;
    /// Names of cameras/apps currently capturing video. Empty = idle.
    fn camera_active(&mut self, cfg: &Config) -> Vec<String>;
    /// True while the session is locked (lock screen / secure desktop).
    fn screen_locked(&mut self) -> bool {
        false
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

pub fn new_detector() -> Box<dyn Detector> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacDetector::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WinDetector::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxDetector::new())
    }
}

/// Deduplicate while keeping order.
pub(crate) fn dedup(mut v: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.retain(|s| seen.insert(s.clone()));
    v
}
