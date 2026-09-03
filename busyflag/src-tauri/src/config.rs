//! User configuration, stored as JSON in the platform config directory.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub type Rgb = [u8; 3];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Colour while the microphone (or camera) is in use.
    pub busy_colour: Rgb,
    /// Colour while nothing is capturing.
    pub free_colour: Rgb,
    /// Colour while paused from the tray menu (black = off).
    pub paused_colour: Rgb,
    /// Colour while the screen is locked and nothing is busy (black = off).
    pub locked_colour: Rgb,
    /// Show `locked_colour` when the screen is locked.
    pub lock_detection: bool,
    /// Minutes a plain "Force busy" click lasts (0 = until turned off).
    pub force_busy_default_minutes: u64,
    /// How long a "test light" colour stays before normal control resumes.
    pub test_duration_s: u64,
    /// Treat camera use as busy too.
    pub use_camera: bool,
    /// Overall brightness 1..=100, applied to every colour.
    pub brightness: u8,
    /// How often to poll the OS, in milliseconds.
    pub poll_interval_ms: u64,
    /// Keep showing busy this long after the last activity, to avoid flicker
    /// when an app briefly releases and re-grabs the microphone.
    pub busy_hold_ms: u64,
    /// Luxafor fade speed for transitions (1 = instant-ish, larger = slower).
    pub fade_speed: u8,
    /// Input devices to ignore (case-insensitive substring of the device name).
    pub ignore_devices: Vec<String>,
    /// Apps to ignore (bundle id on macOS, package/exe name on Windows,
    /// application.name on Linux). Case-insensitive substring match.
    pub ignore_apps: Vec<String>,
    /// macOS only: also treat a process reporting "input running" as busy.
    /// Works around Bluetooth headsets that never report device-level activity.
    pub process_level_detection: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            busy_colour: [255, 0, 0],
            free_colour: [0, 255, 0],
            paused_colour: [0, 0, 0],
            locked_colour: [255, 170, 0],
            lock_detection: true,
            force_busy_default_minutes: 30,
            test_duration_s: 3,
            use_camera: false,
            brightness: 100,
            poll_interval_ms: 500,
            busy_hold_ms: 2000,
            fade_speed: 10,
            ignore_devices: vec![],
            ignore_apps: vec![
                // macOS system services that report "input running" while idle.
                "com.apple.CoreSpeech".into(),
                "com.apple.assistantd".into(),
                "com.apple.Siri".into(),
                "com.apple.SiriNCService".into(),
            ],
            process_level_detection: false,
        }
    }
}

impl Config {
    pub fn sanitised(mut self) -> Self {
        self.brightness = self.brightness.clamp(1, 100);
        self.poll_interval_ms = self.poll_interval_ms.clamp(100, 10_000);
        self.busy_hold_ms = self.busy_hold_ms.min(60_000);
        self.fade_speed = self.fade_speed.max(1);
        self.force_busy_default_minutes = self.force_busy_default_minutes.min(24 * 60);
        self.test_duration_s = self.test_duration_s.clamp(1, 600);
        self
    }

    pub fn scaled(&self, rgb: Rgb) -> Rgb {
        let b = self.brightness as u32;
        [
            (rgb[0] as u32 * b / 100) as u8,
            (rgb[1] as u32 * b / 100) as u8,
            (rgb[2] as u32 * b / 100) as u8,
        ]
    }

    pub fn ignores_app(&self, name: &str) -> bool {
        let n = name.to_lowercase();
        self.ignore_apps.iter().any(|a| !a.is_empty() && n.contains(&a.to_lowercase()))
    }

    pub fn ignores_device(&self, name: &str) -> bool {
        let n = name.to_lowercase();
        self.ignore_devices.iter().any(|a| !a.is_empty() && n.contains(&a.to_lowercase()))
    }
}

pub fn path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("busyflag"));
    dir.join("config.json")
}

pub fn load(app: &AppHandle) -> Config {
    let p = path(app);
    match std::fs::read_to_string(&p) {
        Ok(s) => match serde_json::from_str::<Config>(&s) {
            Ok(c) => {
                let c = c.sanitised();
                // Rewrite so newly added keys appear in the file with their defaults.
                let _ = save(app, &c);
                c
            }
            Err(e) => {
                log::warn!("config {} unreadable ({e}); using defaults", p.display());
                Config::default()
            }
        },
        Err(_) => {
            let c = Config::default();
            let _ = save(app, &c);
            c
        }
    }
}

pub fn save(app: &AppHandle, cfg: &Config) -> Result<(), String> {
    let p = path(app);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&p, s).map_err(|e| e.to_string())
}
