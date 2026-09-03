//! System tray icon and menu.

use crate::config::Rgb;
use crate::manager::{Manager, State, Status, TestCmd};
use crate::EmitConfig;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager as _, Wry};

pub const TRAY_ID: &str = "main";

pub struct TrayUi {
    status: MenuItem<Wry>,
    pause: CheckMenuItem<Wry>,
    force_quick: CheckMenuItem<Wry>,
    /// (minutes, item); 0 = until turned off.
    force: Vec<(u64, CheckMenuItem<Wry>)>,
    camera: CheckMenuItem<Wry>,
}

const FORCE_OPTIONS: &[(u64, &str)] = &[
    (5, "5 minutes"),
    (15, "15 minutes"),
    (30, "30 minutes"),
    (60, "1 hour"),
    (120, "2 hours"),
    (0, "Until I turn it off"),
];

fn quick_label(mins: u64) -> String {
    match mins {
        0 => "Force busy".into(),
        m if m % 60 == 0 => format!("Force busy ({} h)", m / 60),
        m => format!("Force busy ({m} min)"),
    }
}

pub fn build(app: &AppHandle, use_camera: bool, force_default: u64) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", "Starting…", false, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause (light off)", true, false, None::<&str>)?;
    let force_quick = CheckMenuItem::with_id(app, "force_quick", quick_label(force_default), true, false, None::<&str>)?;
    let mut force = vec![];
    for (mins, label) in FORCE_OPTIONS {
        force.push((*mins, CheckMenuItem::with_id(app, format!("force_{mins}"), *label, true, false, None::<&str>)?));
    }
    let force_clear = MenuItem::with_id(app, "force_clear", "Clear", true, None::<&str>)?;
    let force_menu = {
        let mut items: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = force.iter().map(|(_, i)| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
        let sep = PredefinedMenuItem::separator(app)?;
        items.push(&sep);
        items.push(&force_clear);
        Submenu::with_items(app, "Force busy for…", true, &items)?
    };
    let camera = CheckMenuItem::with_id(app, "camera", "Camera counts as busy", true, use_camera, None::<&str>)?;
    let test = Submenu::with_items(
        app,
        "Test light",
        true,
        &[
            &MenuItem::with_id(app, "test_red", "Red", true, None::<&str>)?,
            &MenuItem::with_id(app, "test_green", "Green", true, None::<&str>)?,
            &MenuItem::with_id(app, "test_blue", "Blue", true, None::<&str>)?,
            &MenuItem::with_id(app, "test_blink", "Blink", true, None::<&str>)?,
            &MenuItem::with_id(app, "test_off", "Off", true, None::<&str>)?,
        ],
    )?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Busyflag", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &pause,
            &force_quick,
            &force_menu,
            &camera,
            &PredefinedMenuItem::separator(app)?,
            &test,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(icon_for(&Status::default()))
        .tooltip("Busyflag")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| on_menu(app, ev.id().as_ref()))
        .build(app)?;

    app.manage(TrayUi { status, pause, force_quick, force, camera });
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    let mgr = app.state::<Manager>();
    match id {
        "pause" => mgr.set_paused(!mgr.is_paused()),
        "force_clear" => mgr.set_forced_minutes(None),
        "force_quick" => {
            if mgr.forced() == crate::manager::Forced::Off {
                mgr.set_forced_minutes(Some(mgr.config().force_busy_default_minutes));
            } else {
                mgr.set_forced_minutes(None);
            }
        }
        id if id.starts_with("force_") => {
            if let Ok(mins) = id["force_".len()..].parse::<u64>() {
                // Clicking the active option turns it off.
                let same = matches!((mgr.forced(), mins), (crate::manager::Forced::Indefinite, 0))
                    || (mins > 0 && mgr.forced_minutes_left().is_some() && current_force_choice(&mgr) == Some(mins));
                mgr.set_forced_minutes(if same { None } else { Some(mins) });
            }
        }
        "camera" => {
            let cfg = mgr.set_use_camera(!mgr.config().use_camera);
            let _ = crate::config::save(app, &cfg);
            let _ = app.emit_config();
        }
        "test_red" => mgr.test(TestCmd::Colour([255, 0, 0])),
        "test_green" => mgr.test(TestCmd::Colour([0, 255, 0])),
        "test_blue" => mgr.test(TestCmd::Colour([0, 0, 255])),
        "test_blink" => mgr.test(TestCmd::Strobe([255, 255, 0])),
        "test_off" => mgr.test(TestCmd::Off),
        "settings" => crate::show_settings(app),
        "quit" => {
            mgr.shutdown();
            app.exit(0);
        }
        _ => {}
    }
    // Reflect toggles immediately.
    update(app, &mgr.status());
}

/// Push a status into the tray icon, tooltip and menu. Safe from any thread.
pub fn update(app: &AppHandle, st: &Status) {
    let mgr = app.try_state::<Manager>();
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon_for(st)));
        let _ = tray.set_tooltip(Some(format!("Busyflag: {}", st.headline())));
    }
    if let Some(ui) = app.try_state::<TrayUi>() {
        let _ = ui.status.set_text(st.headline());
        if let Some(m) = mgr {
            let _ = ui.pause.set_checked(m.is_paused());
            let _ = ui.force_quick.set_checked(m.forced() != crate::manager::Forced::Off);
            let _ = ui.force_quick.set_text(quick_label(m.config().force_busy_default_minutes));
            let choice = current_force_choice(&m);
            for (mins, item) in &ui.force {
                let _ = item.set_checked(choice == Some(*mins));
            }
            let _ = ui.camera.set_checked(m.config().use_camera);
        }
    }
}

/// Which force option is active: 0 for indefinite, else the option the timer was started with.
fn current_force_choice(m: &Manager) -> Option<u64> {
    match m.forced() {
        crate::manager::Forced::Off => None,
        crate::manager::Forced::Indefinite => Some(0),
        crate::manager::Forced::Until(_) => {
            let left = m.forced_minutes_left().unwrap_or(0);
            FORCE_OPTIONS.iter().map(|(mins, _)| *mins).filter(|&mins| mins >= left && mins > 0).min()
        }
    }
}

/// A filled dot in the state colour; hollow ring when the light is not connected.
pub fn icon_for(st: &Status) -> Image<'static> {
    let colour: Rgb = match st.state {
        State::Free => [52, 199, 89],
        State::Busy | State::ForcedBusy => [255, 59, 48],
        State::Locked => [255, 170, 0],
        State::Paused => [142, 142, 147],
    };
    render_dot(36, colour, !st.light_connected)
}

fn render_dot(size: u32, rgb: Rgb, hollow: bool) -> Image<'static> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    let r_outer = c - 2.0;
    let r_inner = if hollow { r_outer - 4.0 } else { -1.0 };
    let ss = 4; // supersampling per axis
    for y in 0..size {
        for x in 0..size {
            let mut cover = 0u32;
            for sy in 0..ss {
                for sx in 0..ss {
                    let fx = x as f32 + (sx as f32 + 0.5) / ss as f32 - c;
                    let fy = y as f32 + (sy as f32 + 0.5) / ss as f32 - c;
                    let d = (fx * fx + fy * fy).sqrt();
                    if d <= r_outer && d >= r_inner {
                        cover += 1;
                    }
                }
            }
            let a = (cover * 255 / (ss * ss)) as u8;
            let i = ((y * size + x) * 4) as usize;
            px[i] = rgb[0];
            px[i + 1] = rgb[1];
            px[i + 2] = rgb[2];
            px[i + 3] = a;
        }
    }
    Image::new_owned(px, size, size)
}
