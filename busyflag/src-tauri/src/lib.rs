mod config;
mod detect;
mod light;
mod manager;
mod tray;

use config::Config;
use manager::{Manager, Status, TestCmd};
use tauri::{AppHandle, Emitter, Manager as _};
use tauri_plugin_autostart::ManagerExt as _;

trait EmitConfig {
    fn emit_config(&self) -> tauri::Result<()>;
}

impl EmitConfig for AppHandle {
    fn emit_config(&self) -> tauri::Result<()> {
        let cfg = self.state::<Manager>().config();
        self.emit("config", &cfg)
    }
}

/// Log to stderr and to a file in the platform log directory (shown in Settings).
fn init_logging(app: &AppHandle) -> Option<std::path::PathBuf> {
    struct Tee(std::fs::File);
    impl std::io::Write for Tee {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            let _ = std::io::stderr().write_all(b);
            self.0.write(b)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.flush()
        }
    }
    let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    let path = app.path().app_log_dir().ok().map(|d| d.join("busyflag.log"));
    let file = path.as_ref().and_then(|p| {
        std::fs::create_dir_all(p.parent()?).ok()?;
        // Start fresh when the log gets large.
        if std::fs::metadata(p).map(|m| m.len() > 2_000_000).unwrap_or(false) {
            let _ = std::fs::remove_file(p);
        }
        std::fs::OpenOptions::new().create(true).append(true).open(p).ok()
    });
    match file {
        Some(f) => builder.target(env_logger::Target::Pipe(Box::new(Tee(f)))),
        None => builder.target(env_logger::Target::Stderr),
    };
    let _ = builder.try_init();
    path
}

#[tauri::command]
fn log_path(app: AppHandle) -> String {
    app.path().app_log_dir().map(|d| d.join("busyflag.log").display().to_string()).unwrap_or_default()
}

pub fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn get_config(mgr: tauri::State<Manager>) -> Config {
    mgr.config()
}

#[tauri::command]
fn save_config(app: AppHandle, mgr: tauri::State<Manager>, cfg: Config) -> Result<Config, String> {
    let cfg = cfg.sanitised();
    config::save(&app, &cfg)?;
    mgr.set_config(cfg.clone());
    tray::update(&app, &mgr.status());
    Ok(cfg)
}

#[tauri::command]
fn get_status(mgr: tauri::State<Manager>) -> Status {
    mgr.status()
}

#[tauri::command]
fn config_path(app: AppHandle) -> String {
    config::path(&app).display().to_string()
}

#[tauri::command]
fn set_paused(app: AppHandle, mgr: tauri::State<Manager>, paused: bool) {
    mgr.set_paused(paused);
    tray::update(&app, &mgr.status());
}

/// `minutes`: null clears, 0 forces until cleared, n forces for n minutes.
#[tauri::command]
fn set_forced(app: AppHandle, mgr: tauri::State<Manager>, minutes: Option<u64>) {
    mgr.set_forced_minutes(minutes);
    tray::update(&app, &mgr.status());
}

#[tauri::command]
fn test_light(mgr: tauri::State<Manager>, kind: String, colour: Option<[u8; 3]>) {
    let c = colour.unwrap_or([255, 255, 255]);
    mgr.test(match kind.as_str() {
        "blink" => TestCmd::Strobe(c),
        "off" => TestCmd::Off,
        _ => TestCmd::Colour(c),
    });
}

#[derive(serde::Serialize)]
struct Controls {
    paused: bool,
    /// -1 off, 0 until cleared, n minutes left
    forced: i64,
}

#[tauri::command]
fn autostart_enabled(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let al = app.autolaunch();
    let r = if enabled { al.enable() } else { al.disable() };
    r.map_err(|e| e.to_string())?;
    let now = al.is_enabled().unwrap_or(enabled);
    tray::update(&app, &app.state::<Manager>().status());
    Ok(now)
}

#[tauri::command]
fn controls(mgr: tauri::State<Manager>) -> Controls {
    let forced = match mgr.forced() {
        manager::Forced::Off => -1,
        manager::Forced::Indefinite => 0,
        manager::Forced::Until(_) => mgr.forced_minutes_left().unwrap_or(1) as i64,
    };
    Controls { paused: mgr.is_paused(), forced }
}

/// Route panics through the log so a crash leaves a trace in busyflag.log.
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info.payload().downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        log::error!("panic: {msg} at {loc}");
        default(info);
    }));
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub fn run() {
    install_panic_hook();
    tauri::Builder::default()
        // A second launch just brings up the settings window of the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| show_settings(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let log = init_logging(&handle);
            log::info!("Busyflag {} starting", env!("CARGO_PKG_VERSION"));
            if let Some(p) = log {
                log::info!("log file {}", p.display());
            }
            let cfg = config::load(&handle);
            log::info!("config at {}", config::path(&handle).display());
            let (use_camera, force_default) = (cfg.use_camera, cfg.force_busy_default_minutes);
            app.manage(Manager::start(handle.clone(), cfg));
            tray::build(&handle, use_camera, force_default)?;
            // First run: start at login by default. A stamp file records that we asked once,
            // so a user who turns it off is not overridden on the next launch.
            let stamp = config::path(&handle).with_file_name("autostart.initialised");
            if !stamp.exists() {
                match handle.autolaunch().enable() {
                    Ok(()) => log::info!("start at login enabled (first run)"),
                    Err(e) => log::warn!("could not enable start at login: {e}"),
                }
                let _ = std::fs::write(&stamp, b"1");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the settings window just hides it; the app lives in the tray.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_status,
            config_path,
            log_path,
            app_version,
            set_paused,
            set_forced,
            test_light,
            autostart_enabled,
            set_autostart,
            controls
        ])
        .run(tauri::generate_context!())
        .expect("error while running Busyflag");
}
