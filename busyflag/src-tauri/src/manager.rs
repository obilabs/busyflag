//! The state machine: polls the detector, decides free/busy, drives the light,
//! and publishes status changes to the tray and the settings window.

use crate::config::{Config, Rgb};
use crate::detect::new_detector;
use crate::light::Light;
use serde::Serialize;
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
        let who = self.mic.iter().chain(self.cam.iter()).cloned().collect::<Vec<_>>();
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
    let mut last_busy: Option<Instant> = None;
    let mut last_colour: Option<Rgb> = None;
    let mut test_until: Option<Instant> = None;
    // Report "not connected" only after the flag has been missing this long,
    // so a USB re-enumeration blip doesn't flash the tray icon.
    const DISCONNECT_GRACE: Duration = Duration::from_secs(3);
    let mut missing_since: Option<Instant> = None;

    loop {
        if inner.quitting.load(Ordering::Relaxed) {
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
            log::info!("{}", status.headline());
            let _ = inner.app.emit("status", &status);
            crate::tray::update(&inner.app, &status);
        }

        std::thread::sleep(Duration::from_millis(cfg.poll_interval_ms));
    }
}
