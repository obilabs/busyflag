//! The state machine: polls the detector, decides free/busy, drives the light,
//! and publishes status changes to the tray and the settings window.

use crate::config::{Config, Rgb};
use crate::detect::new_detector;
use crate::light::Light;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Free,
    Busy,
    ForcedBusy,
    Locked,
    Paused,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct Status {
    pub state: State,
    pub mic: Vec<String>,
    pub cam: Vec<String>,
    pub light_connected: bool,
    /// Minutes left on a timed force-busy (None = no timer).
    pub forced_minutes_left: Option<u64>,
}

impl Default for Status {
    fn default() -> Self {
        Self { state: State::Free, mic: vec![], cam: vec![], light_connected: false, forced_minutes_left: None }
    }
}

/// Force-busy mode: not forced, forced until cleared, or forced until a deadline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Forced {
    Off,
    Indefinite,
    Until(Instant),
}

impl Status {
    pub fn headline(&self) -> String {
        let all = self.mic.iter().chain(self.cam.iter()).cloned().collect::<Vec<_>>();
        // Keep the tray line short: two names, then "+N".
        let who: Vec<String> = if all.len() > 2 {
            let mut v = all[..2].to_vec();
            v.push(format!("+{}", all.len() - 2));
            v
        } else {
            all
        };
        let s = match self.state {
            State::Free => "Free".to_string(),
            State::Busy if who.is_empty() => "Busy".to_string(),
            State::Busy => format!("Busy: {}", who.join(", ")),
            State::ForcedBusy => match self.forced_minutes_left {
                Some(m) => format!("Busy (forced, {m} min left)"),
                None => "Busy (forced)".to_string(),
            },
            State::Locked => "Away (screen locked)".to_string(),
            State::Paused => "Paused".to_string(),
        };
        if self.light_connected {
            s
        } else {
            format!("{s} · No Luxafor Flag found, plug it in")
        }
    }
}

/// One use of the microphone or camera by one app or device.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ActivityEntry {
    pub source: String,
    /// "mic" or "cam"
    pub kind: String,
    /// Unix milliseconds
    pub start_ms: u64,
    pub end_ms: Option<u64>,
}

const ACTIVITY_KEEP: usize = 500;
const CSV_HEADER: &str = "start,end,duration_seconds,kind,source,start_ms,end_ms";

fn local_stamp(ms: u64) -> String {
    let fmt = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    time::OffsetDateTime::from_unix_timestamp((ms / 1000) as i64)
        .map(|t| t.to_offset(offset).format(&fmt).unwrap_or_default())
        .unwrap_or_default()
}

fn csv_quote(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Minimal RFC 4180 field splitter (quotes, doubled quotes, commas in quotes).
fn csv_fields(line: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

impl ActivityEntry {
    fn to_csv(&self) -> String {
        let end = self.end_ms.map(local_stamp).unwrap_or_default();
        let dur = self.end_ms.map(|e| (e.saturating_sub(self.start_ms) / 1000).to_string()).unwrap_or_default();
        format!(
            "{},{},{},{},{},{},{}",
            local_stamp(self.start_ms),
            end,
            dur,
            self.kind,
            csv_quote(&self.source),
            self.start_ms,
            self.end_ms.map(|e| e.to_string()).unwrap_or_default()
        )
    }

    fn from_csv(line: &str) -> Option<Self> {
        let f = csv_fields(line);
        if f.len() < 7 {
            return None;
        }
        Some(Self {
            kind: f[3].clone(),
            source: f[4].clone(),
            start_ms: f[5].parse().ok()?,
            end_ms: f[6].parse().ok(),
        })
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

struct Activity {
    entries: VecDeque<ActivityEntry>,
    /// (kind, source) -> index into `entries` of the open row
    open: HashMap<(String, String), usize>,
    path: std::path::PathBuf,
}

impl Activity {
    fn load(path: std::path::PathBuf, retention_days: u64) -> Self {
        let cutoff = now_ms().saturating_sub(retention_days * 86_400_000);
        // One-time migration from the earlier JSON-lines format.
        let legacy = path.with_extension("jsonl");
        let text = std::fs::read_to_string(&path).or_else(|_| std::fs::read_to_string(&legacy));
        let mut entries: VecDeque<ActivityEntry> = text
            .map(|s| {
                s.lines()
                    .filter_map(|l| ActivityEntry::from_csv(l).or_else(|| serde_json::from_str::<ActivityEntry>(l).ok()))
                    .filter(|e| e.start_ms >= cutoff)
                    .map(|mut e| {
                        // A row left open by a crash: close it at its start (duration unknown).
                        if e.end_ms.is_none() {
                            e.end_ms = Some(e.start_ms);
                        }
                        e
                    })
                    .collect()
            })
            .unwrap_or_default();
        while entries.len() > ACTIVITY_KEEP {
            entries.pop_front();
        }
        let a = Self { entries, open: HashMap::new(), path };
        a.rewrite();
        let _ = std::fs::remove_file(legacy);
        a
    }

    /// Bring the set of currently active (kind, source) pairs up to date. Returns true on change.
    fn update(&mut self, current: &[(String, String)]) -> bool {
        let now = now_ms();
        let mut changed = false;
        let current: std::collections::HashSet<(String, String)> = current.iter().cloned().collect();
        // Close rows whose source went away.
        let closed: Vec<(String, String)> = self.open.keys().filter(|k| !current.contains(*k)).cloned().collect();
        for k in closed {
            if let Some(i) = self.open.remove(&k) {
                if let Some(e) = self.entries.get_mut(i) {
                    e.end_ms = Some(now);
                }
                changed = true;
            }
        }
        // Open rows for new sources.
        for k in current {
            if !self.open.contains_key(&k) {
                self.entries.push_back(ActivityEntry { kind: k.0.clone(), source: k.1.clone(), start_ms: now, end_ms: None });
                if self.entries.len() > ACTIVITY_KEEP {
                    self.entries.pop_front();
                    // Indexes shifted by one.
                    for v in self.open.values_mut() {
                        *v = v.saturating_sub(1);
                    }
                }
                self.open.insert(k, self.entries.len() - 1);
                changed = true;
            }
        }
        if changed {
            self.rewrite();
        }
        changed
    }

    fn close_all(&mut self) {
        let now = now_ms();
        for (_, i) in self.open.drain() {
            if let Some(e) = self.entries.get_mut(i) {
                e.end_ms = Some(now);
            }
        }
        self.rewrite();
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.open.clear();
        self.rewrite();
    }

    fn rewrite(&self) {
        let mut out = String::from(CSV_HEADER);
        out.push('\n');
        for e in &self.entries {
            out.push_str(&e.to_csv());
            out.push('\n');
        }
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("jsonl.tmp");
        if std::fs::File::create(&tmp).and_then(|mut f| f.write_all(out.as_bytes())).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }
}

pub enum TestCmd {
    Colour(Rgb),
    Strobe(Rgb),
    Off,
}

struct Inner {
    app: AppHandle,
    cfg: Mutex<Config>,
    status: Mutex<Status>,
    paused: AtomicBool,
    forced: Mutex<Forced>,
    quitting: AtomicBool,
    test: Mutex<Option<TestCmd>>,
    activity: Mutex<Option<Activity>>,
    clear_activity: AtomicBool,
}

#[derive(Clone)]
pub struct Manager(Arc<Inner>);

impl Manager {
    pub fn start(app: AppHandle, cfg: Config) -> Self {
        let inner = Arc::new(Inner {
            app,
            cfg: Mutex::new(cfg),
            status: Mutex::new(Status::default()),
            paused: AtomicBool::new(false),
            forced: Mutex::new(Forced::Off),
            quitting: AtomicBool::new(false),
            test: Mutex::new(None),
            activity: Mutex::new(None),
            clear_activity: AtomicBool::new(false),
        });
        let worker = inner.clone();
        std::thread::Builder::new()
            .name("busyflag-poll".into())
            .spawn(move || {
                // A bug in a detector must not take the whole app down: log it and restart.
                let mut restarts = 0u32;
                while !worker.quitting.load(Ordering::Relaxed) {
                    let w = worker.clone();
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || run_loop(w)));
                    if r.is_ok() {
                        break;
                    }
                    restarts += 1;
                    log::error!("poll loop crashed (restart #{restarts}); see message above");
                    std::thread::sleep(Duration::from_secs(restarts.min(30) as u64));
                }
            })
            .expect("spawn poll thread");
        Self(inner)
    }

    pub fn config(&self) -> Config {
        self.0.cfg.lock().unwrap().clone()
    }

    pub fn set_config(&self, cfg: Config) {
        *self.0.cfg.lock().unwrap() = cfg;
    }

    pub fn status(&self) -> Status {
        self.0.status.lock().unwrap().clone()
    }

    pub fn is_paused(&self) -> bool {
        self.0.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, v: bool) {
        self.0.paused.store(v, Ordering::Relaxed);
    }

    pub fn forced(&self) -> Forced {
        *self.0.forced.lock().unwrap()
    }

    /// `None` clears; `Some(0)` forces until cleared; `Some(n)` forces for n minutes.
    pub fn set_forced_minutes(&self, minutes: Option<u64>) {
        *self.0.forced.lock().unwrap() = match minutes {
            None => Forced::Off,
            Some(0) => Forced::Indefinite,
            Some(m) => Forced::Until(Instant::now() + Duration::from_secs(m * 60)),
        };
    }

    /// Minutes remaining on a timed force (rounded up), if any.
    pub fn forced_minutes_left(&self) -> Option<u64> {
        match self.forced() {
            Forced::Until(t) => Some((t.saturating_duration_since(Instant::now()).as_secs() + 59) / 60),
            _ => None,
        }
    }

    pub fn set_use_camera(&self, v: bool) -> Config {
        let mut c = self.0.cfg.lock().unwrap();
        c.use_camera = v;
        c.clone()
    }

    /// Most recent first.
    pub fn activity(&self) -> Vec<ActivityEntry> {
        self.0.activity.lock().unwrap().as_ref().map(|a| a.entries.iter().rev().cloned().collect()).unwrap_or_default()
    }

    pub fn clear_activity(&self) {
        self.0.clear_activity.store(true, Ordering::Relaxed);
    }

    pub fn test(&self, cmd: TestCmd) {
        *self.0.test.lock().unwrap() = Some(cmd);
    }

    /// Turn the light off and stop the worker; returns once the light is off (or after a timeout).
    pub fn shutdown(&self) {
        self.0.quitting.store(true, Ordering::Relaxed);
        // The worker flips light_connected to false once it has switched the light off.
        let t0 = Instant::now();
        while self.0.status.lock().unwrap().light_connected && t0.elapsed() < Duration::from_millis(1500) {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn run_loop(inner: Arc<Inner>) {
    let mut light = Light::new();
    let mut det = new_detector();
    {
        let cfg = inner.cfg.lock().unwrap().clone();
        let path = crate::config::activity_path(&inner.app);
        *inner.activity.lock().unwrap() = Some(Activity::load(path, cfg.activity_retention_days));
    }
    let mut last_busy: Option<Instant> = None;
    let mut last_colour: Option<Rgb> = None;
    let mut test_until: Option<Instant> = None;
    // Report "not connected" only after the flag has been missing this long,
    // so a USB re-enumeration blip doesn't flash the tray icon.
    const DISCONNECT_GRACE: Duration = Duration::from_secs(3);
    let mut missing_since: Option<Instant> = None;
    // Relative times in the tray submenu go stale; refresh them once a minute.
    let mut last_activity_refresh: Option<Instant> = None;
    // For "Free (after Busy for 27 min)" style log lines.
    let mut state_since = Instant::now();
    let mut prev_state = State::Free;

    loop {
        if inner.quitting.load(Ordering::Relaxed) {
            if let Some(a) = inner.activity.lock().unwrap().as_mut() {
                a.close_all();
            }
            let _ = light.off();
            let mut st = inner.status.lock().unwrap();
            st.light_connected = false;
            return;
        }
        let cfg = inner.cfg.lock().unwrap().clone();
        let now = Instant::now();

        let mic = det.mic_active(&cfg);
        let cam = if cfg.use_camera { det.camera_active(&cfg) } else { vec![] };
        let raw_busy = !mic.is_empty() || !cam.is_empty();

        // Activity log: one row per (kind, source) while it is active.
        if inner.clear_activity.swap(false, Ordering::Relaxed) {
            if let Some(a) = inner.activity.lock().unwrap().as_mut() {
                a.clear();
                let _ = inner.app.emit("activity", ());
            }
        }
        if cfg.activity_log {
            let current: Vec<(String, String)> = mic
                .iter()
                .map(|s| ("mic".to_string(), s.clone()))
                .chain(cam.iter().map(|s| ("cam".to_string(), s.clone())))
                .collect();
            let changed = inner.activity.lock().unwrap().as_mut().map(|a| a.update(&current)).unwrap_or(false);
            if changed || last_activity_refresh.map(|t: Instant| t.elapsed() > Duration::from_secs(60)).unwrap_or(true) {
                let _ = inner.app.emit("activity", ());
                let recent: Vec<ActivityEntry> = inner.activity.lock().unwrap().as_ref().map(|a| a.entries.iter().rev().take(5).cloned().collect()).unwrap_or_default();
                crate::tray::update_activity(&inner.app, &recent);
                last_activity_refresh = Some(Instant::now());
            }
        }
        if raw_busy {
            last_busy = Some(now);
        }
        let held = last_busy.map(|t| t.elapsed() < Duration::from_millis(cfg.busy_hold_ms)).unwrap_or(false);

        // Expire a timed force-busy.
        {
            let mut f = inner.forced.lock().unwrap();
            if let Forced::Until(t) = *f {
                if now >= t {
                    *f = Forced::Off;
                }
            }
        }
        let forced = *inner.forced.lock().unwrap();
        let forced_minutes_left = match forced {
            Forced::Until(t) => Some((t.saturating_duration_since(now).as_secs() + 59) / 60),
            _ => None,
        };

        let state = if inner.paused.load(Ordering::Relaxed) {
            State::Paused
        } else if forced != Forced::Off {
            State::ForcedBusy
        } else if raw_busy || held {
            State::Busy
        } else if cfg.lock_detection && det.screen_locked() {
            State::Locked
        } else {
            State::Free
        };
        let colour = cfg.scaled(match state {
            State::Free => cfg.free_colour,
            State::Busy | State::ForcedBusy => cfg.busy_colour,
            State::Locked => cfg.locked_colour,
            State::Paused => cfg.paused_colour,
        });

        light.verify();
        let reconnected = light.try_connect(false);
        if reconnected {
            last_colour = None;
        }

        if let Some(cmd) = inner.test.lock().unwrap().take() {
            let r = match cmd {
                TestCmd::Colour(c) => light.colour(c),
                TestCmd::Strobe(c) => light.strobe(c, 10, 5),
                TestCmd::Off => light.off(),
            };
            if r.is_ok() {
                test_until = Some(now + Duration::from_secs(cfg.test_duration_s));
                last_colour = None;
            }
        }
        let testing = test_until.map(|t| now < t).unwrap_or(false);
        if !testing && light.connected() && last_colour != Some(colour) {
            if light.fade(colour, cfg.fade_speed).is_ok() {
                last_colour = Some(colour);
            }
        }

        let light_connected = if light.connected() {
            missing_since = None;
            true
        } else {
            let since = *missing_since.get_or_insert(now);
            now.duration_since(since) < DISCONNECT_GRACE
        };
        let status = Status { state, mic, cam, light_connected, forced_minutes_left };
        let changed = {
            let mut cur = inner.status.lock().unwrap();
            if *cur != status {
                *cur = status.clone();
                true
            } else {
                false
            }
        };
        if changed {
            if status.state != prev_state {
                let held = state_since.elapsed().as_secs();
                let dur = match held {
                    0..=59 => format!("{held} s"),
                    60..=3599 => format!("{} min", held / 60),
                    _ => format!("{:.1} h", held as f64 / 3600.0),
                };
                let prev = match prev_state {
                    State::Free => "Free",
                    State::Busy => "Busy",
                    State::ForcedBusy => "Busy (forced)",
                    State::Locked => "Away (screen locked)",
                    State::Paused => "Paused",
                };
                log::info!("{} (after {prev} for {dur})", status.headline());
                state_since = now;
                prev_state = status.state;
            } else {
                log::info!("{}", status.headline());
            }
            let _ = inner.app.emit("status", &status);
            crate::tray::update(&inner.app, &status);
        }

        std::thread::sleep(Duration::from_millis(cfg.poll_interval_ms));
    }
}
