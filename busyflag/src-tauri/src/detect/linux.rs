//! Linux: PipeWire/PulseAudio capture streams via `pactl` (JSON output on
//! pactl >= 16, text parsing otherwise), falling back to raw ALSA capture PCM
//! status under /proc/asound. Cameras: any process holding /dev/video* open.

use super::{dedup, Detector};
use crate::config::Config;
use std::collections::HashSet;
use std::process::Command;

fn pactl(args: &[&str]) -> Option<String> {
    let out = Command::new("pactl").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Indexes of monitor sources (loopbacks of outputs), which are not microphones.
fn monitor_sources() -> HashSet<u64> {
    let mut set = HashSet::new();
    if let Some(json) = pactl(&["-f", "json", "list", "sources"]) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            for s in v.as_array().into_iter().flatten() {
                let name = s["name"].as_str().unwrap_or("");
                if name.ends_with(".monitor") {
                    if let Some(i) = s["index"].as_u64() {
                        set.insert(i);
                    }
                }
            }
            return set;
        }
    }
    if let Some(txt) = pactl(&["list", "sources"]) {
        let mut idx = None;
        for line in txt.lines() {
            let t = line.trim();
            if let Some(n) = t.strip_prefix("Source #") {
                idx = n.parse().ok();
            } else if let Some(n) = t.strip_prefix("Name: ") {
                if n.ends_with(".monitor") {
                    if let Some(i) = idx {
                        set.insert(i);
                    }
                }
            }
        }
    }
    set
}

fn pulse_capture_streams(cfg: &Config) -> Option<Vec<String>> {
    let monitors = monitor_sources();
    if let Some(json) = pactl(&["-f", "json", "list", "source-outputs"]) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            let mut out = vec![];
            for so in v.as_array().into_iter().flatten() {
                let src = so["source"].as_u64().unwrap_or(u64::MAX);
                let corked = so["corked"].as_bool().unwrap_or(false);
                if corked || monitors.contains(&src) {
                    continue;
                }
                let app = so["properties"]["application.name"].as_str().unwrap_or("unknown app");
                if !cfg.ignores_app(app) {
                    out.push(app.to_string());
                }
            }
            return Some(out);
        }
    }
    let txt = pactl(&["list", "source-outputs"])?;
    let mut out = vec![];
    let (mut src, mut corked, mut app) = (u64::MAX, false, String::new());
    let flush = |src: u64, corked: bool, app: &str, out: &mut Vec<String>| {
        if src != u64::MAX && !corked && !monitors.contains(&src) && !cfg.ignores_app(app) {
            out.push(if app.is_empty() { "unknown app".into() } else { app.to_string() });
        }
    };
    for line in txt.lines() {
        let t = line.trim();
        if t.starts_with("Source Output #") {
            flush(src, corked, &app, &mut out);
            src = u64::MAX;
            corked = false;
            app.clear();
        } else if let Some(n) = t.strip_prefix("Source: ") {
            src = n.parse().unwrap_or(u64::MAX);
        } else if let Some(n) = t.strip_prefix("Corked: ") {
            corked = n == "yes";
        } else if let Some(n) = t.strip_prefix("application.name = ") {
            app = n.trim_matches('"').to_string();
        }
    }
    flush(src, corked, &app, &mut out);
    Some(out)
}

/// Raw ALSA fallback: any capture substream in RUNNING state.
fn alsa_capture_running() -> Vec<String> {
    let mut out = vec![];
    let Ok(cards) = std::fs::read_dir("/proc/asound") else { return out };
    for card in cards.flatten() {
        let name = card.file_name().to_string_lossy().into_owned();
        if !name.starts_with("card") {
            continue;
        }
        let Ok(pcms) = std::fs::read_dir(card.path()) else { continue };
        for pcm in pcms.flatten() {
            let p = pcm.file_name().to_string_lossy().into_owned();
            if !(p.starts_with("pcm") && p.ends_with('c')) {
                continue;
            }
            let Ok(subs) = std::fs::read_dir(pcm.path()) else { continue };
            for sub in subs.flatten() {
                let status = sub.path().join("status");
                if let Ok(s) = std::fs::read_to_string(status) {
                    if s.lines().any(|l| l.trim() == "state: RUNNING") {
                        out.push(format!("ALSA {name}/{p}"));
                    }
                }
            }
        }
    }
    out
}

/// Processes with /dev/video* open (same user only; other users' fds are unreadable).
fn video_device_users(cfg: &Config) -> Vec<String> {
    let mut out = vec![];
    let Ok(procs) = std::fs::read_dir("/proc") else { return out };
    let me = std::process::id().to_string();
    for p in procs.flatten() {
        let pid = p.file_name().to_string_lossy().into_owned();
        if !pid.chars().all(|c| c.is_ascii_digit()) || pid == me {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(p.path().join("fd")) else { continue };
        let uses_video = fds
            .flatten()
            .filter_map(|fd| std::fs::read_link(fd.path()).ok())
            .any(|target| target.to_string_lossy().starts_with("/dev/video"));
        if uses_video {
            let comm = std::fs::read_to_string(p.path().join("comm")).unwrap_or_default();
            let comm = comm.trim().to_string();
            let comm = if comm.is_empty() { format!("pid {pid}") } else { comm };
            if !cfg.ignores_app(&comm) {
                out.push(comm);
            }
        }
    }
    dedup(out)
}

/// Session lock via systemd-logind's LockedHint, falling back to the
/// org.freedesktop.ScreenSaver D-Bus interface (GNOME, KDE, xfce4-screensaver...).
fn session_locked() -> bool {
    let logind = Command::new("loginctl")
        .args(["show-session", "auto", "-p", "LockedHint", "--value"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "yes")
        .unwrap_or(false);
    if logind {
        return true;
    }
    Command::new("busctl")
        .args([
            "--user", "call", "org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver",
            "org.freedesktop.ScreenSaver", "GetActive",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "b true")
        .unwrap_or(false)
}

pub struct LinuxDetector {
    pulse_ok: bool,
}

impl LinuxDetector {
    pub fn new() -> Self {
        let pulse_ok = pactl(&["info"]).is_some();
        if !pulse_ok {
            log::warn!("pactl unavailable; using ALSA /proc/asound fallback for microphone detection");
        }
        Self { pulse_ok }
    }
}

impl Detector for LinuxDetector {
    fn mic_active(&mut self, cfg: &Config) -> Vec<String> {
        if self.pulse_ok {
            if let Some(v) = pulse_capture_streams(cfg) {
                return dedup(v);
            }
            self.pulse_ok = false;
            log::warn!("pactl stopped responding; switching to ALSA fallback");
        }
        alsa_capture_running()
    }

    fn camera_active(&mut self, cfg: &Config) -> Vec<String> {
        video_device_users(cfg)
    }

    fn screen_locked(&mut self) -> bool {
        session_locked()
    }
}
