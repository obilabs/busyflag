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

/// Machine-wide defaults an administrator can deploy. Keys here override the
/// built-in defaults; the user's own config overrides these in turn.
pub fn system_defaults_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Application Support/Busyflag/defaults.json")
    }
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        base.join("Busyflag").join("defaults.json")
    }
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/etc/busyflag/defaults.json")
    }
}

fn read_json(p: &std::path::Path) -> Option<serde_json::Value> {
    let s = std::fs::read_to_string(p).ok()?;
    match serde_json::from_str::<serde_json::Value>(&s) {
        Ok(v) if v.is_object() => Some(v),
        Ok(_) => {
            log::warn!("{} is not a JSON object; ignored", p.display());
            None
        }
        Err(e) => {
            log::warn!("{} unreadable ({e}); ignored", p.display());
            None
        }
    }
}

/// If the user's config exists but cannot be parsed, move it aside so the
/// rewrite below does not destroy their edits.
fn preserve_broken(p: &std::path::Path) {
    if !p.exists() {
        return;
    }
    let ok = std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).map(|v| v.is_object()).unwrap_or(false);
    if !ok {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let backup = p.with_extension(format!("broken-{stamp}.json"));
        match std::fs::rename(p, &backup) {
            Ok(()) => log::warn!("config could not be parsed; kept a copy at {}", backup.display()),
            Err(e) => log::warn!("config could not be parsed and could not be backed up: {e}"),
        }
    }
}

/// Built-in defaults <- system defaults <- user config, key by key.
fn layered(app: &AppHandle) -> (Config, bool) {
    let mut merged = serde_json::to_value(Config::default()).unwrap_or_default();
    let mut have_user = false;
    let sys = system_defaults_path();
    if let Some(v) = read_json(&sys) {
        log::info!("applying system defaults from {}", sys.display());
        merge_into(&mut merged, v);
    }
    if let Some(v) = read_json(&path(app)) {
        have_user = true;
        merge_into(&mut merged, v);
    }
    let cfg = serde_json::from_value::<Config>(merged).unwrap_or_default().sanitised();
    (cfg, have_user)
}

fn merge_into(base: &mut serde_json::Value, over: serde_json::Value) {
    if let (Some(b), Some(o)) = (base.as_object_mut(), over.as_object()) {
        for (k, v) in o {
            b.insert(k.clone(), v.clone());
        }
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
    preserve_broken(&path(app));
    let (cfg, have_user) = layered(app);
    // Write the user file so every key is visible with its effective value
    // (first run creates it; later runs pick up newly added keys).
    if !have_user || serde_json::to_value(&cfg).ok() != read_json(&path(app)) {
        let _ = save(app, &cfg);
    }
    cfg
}

pub fn save(app: &AppHandle, cfg: &Config) -> Result<(), String> {
    let p = path(app);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&p, s).map_err(|e| e.to_string())
}
