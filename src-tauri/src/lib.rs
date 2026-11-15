mod commands;
mod config;
mod display;
mod images;
mod models;
mod overlay;
mod state;
#[cfg(windows)]
mod win32;

use config::load_config;
use overlay::{notify_status, start_overlays, stop_overlays};
use state::AppState;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_minimized().unwrap_or(false) {
            let _ = win.unminimize();
        }
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn handle_tray(app: &AppHandle, id: &str) {
    match id {
        "show" => show_main(app),
        "start" => {
            let cfg = {
                let state = app.state::<AppState>();
                let last = state.inner.lock().unwrap().last_config.clone();
                last.unwrap_or_else(|| load_config(app))
            };
            let has_source = |m: &models::MonitorConfig| {
                m.folder.is_some() || m.files.as_ref().is_some_and(|f| !f.is_empty())
            };
            if cfg.monitors.values().any(has_source) {
                // Off the main thread so window creation can marshal to the event loop.
                let app = app.clone();
                std::thread::spawn(move || {
                    if start_overlays(&app, &cfg).attach_failed {
                        overlay::report_attach_failure(&app);
                    }
                    notify_status(&app);
                });
            } else {
                show_main(app);
            }
        }
        "stop" => {
            let app = app.clone();
            std::thread::spawn(move || {
                stop_overlays(&app);
                notify_status(&app);
            });
        }
        "next" => {
            use tauri::Emitter;
            let _ = app.emit("next", ());
        }
        "quit" => {
            // Overlays are children of WorkerW and are torn down on process exit.
            app.exit(0);
        }
        _ => {}
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "설정 창 보이기", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "start", "시작", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "중지", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "즉시 다음", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let items: [&dyn IsMenuItem<_>; 7] = [
        &show, &sep1, &start, &stop, &next, &sep2, &quit,
    ];
    let menu = Menu::with_items(app, &items)?;

    // The tray uses a simplified mark that stays legible at 16px.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Move Wallpapers")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_tray(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_displays,
            commands::get_config,
            commands::get_status,
            commands::count_images,
            commands::get_overlay_config,
            commands::pick_folder,
            commands::pick_files,
            commands::count_files,
            commands::pick_region,
            commands::region_picked,
            commands::start,
            commands::stop,
            commands::next,
            commands::overlay_log,
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            overlay::spawn_watchdog(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Control window closes to tray instead of quitting.
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
