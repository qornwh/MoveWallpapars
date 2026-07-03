use crate::config::save_config;
use crate::display::get_displays;
use crate::images::{list_images, shuffle};
use crate::models::{Config, DisplayInfo, OverlayConfig, Region, Status};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// Sanitize a display id into a value usable in window labels and URLs.
fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Absolute physical-pixel placement for a monitor + optional sub-region.
fn resolve_bounds(d: &DisplayInfo, region: &Option<Region>) -> (i32, i32, i32, i32) {
    let b = d.bounds;
    match region {
        None => (b.x, b.y, b.width, b.height),
        Some(r) => {
            let rx = r.x.max(0).min(b.width - 50).max(0);
            let ry = r.y.max(0).min(b.height - 50).max(0);
            let rw = r.width.clamp(50, (b.width - rx).max(50));
            let rh = r.height.clamp(50, (b.height - ry).max(50));
            (b.x + rx, b.y + ry, rw, rh)
        }
    }
}

/// Build one overlay window, reparent it into the desktop wallpaper layer, and show it.
/// Must run on the main thread.
fn build_overlay(app: &AppHandle, label: &str, key: &str, x: i32, y: i32, w: i32, h: i32) {
    let url = format!("index.html?window=overlay&display={key}");
    let init = format!(
        "window.__MW_ROLE__='overlay';window.__MW_DISPLAY__='{key}';"
    );
    let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title("wallpaper")
        .decorations(false)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .focused(false)
        .visible(false)
        .inner_size(w as f64, h as f64)
        .position(x as f64, y as f64)
        .initialization_script(&init)
        .build();

    let win = match built {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[overlay] build failed for {label}: {e}");
            return;
        }
    };
    println!("[overlay] built {label} at ({x},{y}) {w}x{h}");

    #[cfg(windows)]
    {
        use crate::win32;
        match win.hwnd() {
            Ok(hwnd) => {
                let overlay = hwnd.0 as *mut core::ffi::c_void;
                win32::square_corners_no_border(overlay);
                match win32::find_wallpaper_host() {
                    Some(host) => {
                        println!("[overlay] wallpaper host {host:?}; reparenting {label}");
                        win32::attach_to_wallpaper(overlay, host, x, y, w, h);
                    }
                    None => eprintln!("[overlay] Progman NOT found; {label} stays floating"),
                }
            }
            Err(e) => eprintln!("[overlay] hwnd() failed for {label}: {e}"),
        }
    }

    let _ = win.set_ignore_cursor_events(true);
    let _ = win.show();
}

/// Build the fullscreen transparent region-picker window over a monitor.
/// Must run on the main thread.
pub fn build_picker(app: &AppHandle, x: i32, y: i32, w: i32, h: i32) {
    let init = "window.__MW_ROLE__='picker';";
    let built = WebviewWindowBuilder::new(
        app,
        "picker-main",
        WebviewUrl::App("index.html?window=picker".into()),
    )
    .title("region")
    .decorations(false)
    .transparent(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .always_on_top(true)
    .visible(false)
    .inner_size(w as f64, h as f64)
    .position(x as f64, y as f64)
    .initialization_script(init)
    .build();

    if let Ok(win) = built {
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
        let _ = win.set_size(tauri::PhysicalSize::new(w as u32, h as u32));
        let _ = win.show();
        let _ = win.set_focus();
    } else if let Err(e) = built {
        eprintln!("[picker] build failed: {e}");
    }
}

/// Compute image plans from config, stash overlay configs in state, then create
/// the overlay windows on the main thread. Returns whether any overlay started.
pub fn start_overlays(app: &AppHandle, cfg: &Config) -> bool {
    stop_overlays(app);
    save_config(app, cfg);

    let displays = get_displays(app);
    let mut plan: Vec<(String, String, i32, i32, i32, i32)> = Vec::new();

    println!(
        "[start] displays={} config_monitors={:?}",
        displays.len(),
        cfg.monitors.keys().collect::<Vec<_>>()
    );

    {
        let state = app.state::<AppState>();
        let mut inner = state.inner.lock().unwrap();
        inner.overlays.clear();
        inner.overlay_labels.clear();

        for d in &displays {
            let Some(m) = cfg.monitors.get(&d.id) else {
                println!("[start] display {} has no config, skip", d.id);
                continue;
            };
            let Some(folder) = &m.folder else {
                println!("[start] display {} has no folder, skip", d.id);
                continue;
            };
            let mut images = list_images(folder);
            println!("[start] display {} folder={} images={}", d.id, folder, images.len());
            if images.is_empty() {
                continue;
            }
            if cfg.shuffle {
                shuffle(&mut images);
            }
            let (x, y, w, h) = resolve_bounds(d, &m.region);
            let key = sanitize(&d.id);
            let label = format!("overlay-{key}");
            inner.overlays.insert(
                key.clone(),
                OverlayConfig {
                    images,
                    interval_sec: cfg.interval_sec,
                },
            );
            inner.overlay_labels.push(label.clone());
            plan.push((label, key, x, y, w, h));
        }

        inner.is_running = !plan.is_empty();
        inner.last_config = Some(cfg.clone());
    }

    let active = !plan.is_empty();
    // Build directly on the command thread. Tauri v2's `build()` dispatches
    // window creation to the main event loop internally, so wrapping this in
    // `run_on_main_thread` would deadlock (waiting for the main thread from it).
    for (label, key, x, y, w, h) in plan {
        build_overlay(app, &label, &key, x, y, w, h);
    }
    active
}

pub fn stop_overlays(app: &AppHandle) {
    let labels: Vec<String> = {
        let state = app.state::<AppState>();
        let mut inner = state.inner.lock().unwrap();
        inner.overlays.clear();
        inner.is_running = false;
        std::mem::take(&mut inner.overlay_labels)
    };
    for label in labels {
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.destroy();
        }
    }
}

pub fn current_status(app: &AppHandle) -> Status {
    let state = app.state::<AppState>();
    let inner = state.inner.lock().unwrap();
    Status {
        active: inner.is_running,
        interval_sec: inner.last_config.as_ref().map(|c| c.interval_sec),
        monitor_count: inner.overlay_labels.len(),
    }
}

pub fn notify_status(app: &AppHandle) {
    let _ = app.emit("status", current_status(app));
}
