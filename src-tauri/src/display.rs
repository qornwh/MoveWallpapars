use crate::models::{Bounds, DisplayInfo};
use tauri::{AppHandle, Manager};

/// Enumerate connected monitors in physical pixels. `id` is the OS device name
/// (e.g. `\\.\DISPLAY1`) so config keys stay stable across sessions.
pub fn get_displays(app: &AppHandle) -> Vec<DisplayInfo> {
    let Some(win) = app.get_webview_window("main") else {
        return Vec::new();
    };
    let monitors = match win.available_monitors() {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };
    let primary_name = win
        .primary_monitor()
        .ok()
        .flatten()
        .and_then(|m| m.name().cloned());

    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let pos = m.position();
            let size = m.size();
            let name = m.name().cloned();
            let id = name
                .clone()
                .unwrap_or_else(|| format!("display-{i}"));
            let is_primary = match (&name, &primary_name) {
                (Some(a), Some(b)) => a == b,
                _ => pos.x == 0 && pos.y == 0,
            };
            DisplayInfo {
                id,
                label: format!("모니터 {} ({}×{})", i + 1, size.width, size.height),
                bounds: Bounds {
                    x: pos.x,
                    y: pos.y,
                    width: size.width as i32,
                    height: size.height as i32,
                },
                scale_factor: m.scale_factor(),
                primary: is_primary,
            }
        })
        .collect()
}
