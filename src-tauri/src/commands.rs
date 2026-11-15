use crate::config::load_config;
use crate::display::get_displays as list_displays;
use crate::images::{filter_images, list_images, IMAGE_EXTS};
use crate::models::{Config, DisplayInfo, OverlayConfig, Region, StartPayload, StartResult, Status};
use crate::overlay::{build_picker, current_status, notify_status, report_attach_failure, start_overlays,
    stop_overlays,
};
use crate::state::AppState;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;

#[tauri::command]
pub fn get_displays(app: AppHandle) -> Vec<DisplayInfo> {
    list_displays(&app)
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Config {
    load_config(&app)
}

#[tauri::command]
pub fn get_status(app: AppHandle) -> Status {
    current_status(&app)
}

#[tauri::command]
pub fn count_images(folder: String) -> usize {
    list_images(&folder).len()
}

#[tauri::command]
pub fn get_overlay_config(state: State<'_, AppState>, display: String) -> Option<OverlayConfig> {
    let inner = state.inner.lock().unwrap();
    inner.overlays.get(&display).cloned()
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let picked = rx.await.ok().flatten();
    picked.and_then(|fp| fp.as_path().map(|p| p.to_string_lossy().into_owned()))
}

/// Multi-select image files for a custom list.
#[tauri::command]
pub async fn pick_files(app: AppHandle) -> Vec<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("이미지", IMAGE_EXTS)
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });
    rx.await
        .ok()
        .flatten()
        .unwrap_or_default()
        .iter()
        .filter_map(|fp| fp.as_path().map(|p| p.to_string_lossy().into_owned()))
        .collect()
}

/// How many entries of a custom list exist and are usable images.
#[tauri::command]
pub fn count_files(files: Vec<String>) -> usize {
    filter_images(&files).len()
}

#[tauri::command]
pub async fn pick_region(
    app: AppHandle,
    state: State<'_, AppState>,
    display_id: String,
) -> Result<Option<Region>, String> {
    let displays = list_displays(&app);
    let Some(d) = displays.into_iter().find(|x| x.id == display_id) else {
        return Ok(None);
    };

    // Hide the control window so it does not block the target monitor.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }

    let (tx, rx) = oneshot::channel();
    *state.region_tx.lock().unwrap() = Some(tx);

    // Build directly — `build()` marshals to the main loop itself; wrapping it
    // in `run_on_main_thread` would deadlock.
    let b = d.bounds;
    build_picker(&app, b.x, b.y, b.width, b.height);

    let region = rx.await.unwrap_or(None);

    if let Some(picker) = app.get_webview_window("picker-main") {
        let _ = picker.destroy();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(region)
}

#[tauri::command]
pub fn region_picked(state: State<'_, AppState>, region: Option<Region>) {
    if let Some(tx) = state.region_tx.lock().unwrap().take() {
        let _ = tx.send(region);
    }
}

// Async so the command runs off the main thread; window creation inside
// `start_overlays` then marshals to the main event loop without deadlocking.
#[tauri::command]
pub async fn start(app: AppHandle, payload: StartPayload) -> StartResult {
    let cfg = Config {
        interval_sec: payload.interval_sec.max(1),
        shuffle: payload.shuffle,
        monitors: payload.monitors,
    };
    let outcome = start_overlays(&app, &cfg);
    if outcome.attach_failed {
        report_attach_failure(&app);
    }
    notify_status(&app);
    StartResult {
        ok: true,
        active: outcome.active,
        attach_failed: outcome.attach_failed,
    }
}

#[tauri::command]
pub async fn stop(app: AppHandle) -> Value {
    stop_overlays(&app);
    notify_status(&app);
    json!({ "ok": true })
}

#[tauri::command]
pub fn next(app: AppHandle) -> Value {
    let _ = app.emit("next", ());
    json!({ "ok": true })
}

#[tauri::command]
pub fn overlay_log(message: String) {
    println!("[overlay] {message}");
}
