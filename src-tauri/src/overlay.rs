use crate::config::save_config;
use crate::display::get_displays;
use crate::images::{filter_images, list_images, shuffle};
use crate::models::{Config, DisplayInfo, OverlayConfig, Region, Status};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// Monotonic suffix that keeps overlay window labels unique across restarts.
static OVERLAY_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

/// Build one overlay window and reparent it into the desktop wallpaper layer.
/// Returns whether it ended up attached; a window that could not be attached is
/// destroyed rather than left floating over the desktop.
fn build_overlay(app: &AppHandle, label: &str, key: &str, x: i32, y: i32, w: i32, h: i32) -> bool {
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
            return false;
        }
    };
    println!("[overlay] built {label} at ({x},{y}) {w}x{h}");

    // Click-through first: it touches the extended window style, and attaching
    // afterwards leaves the layered attributes in their final state.
    let _ = win.set_ignore_cursor_events(true);

    #[cfg(windows)]
    {
        use crate::win32;
        match win.hwnd() {
            Ok(hwnd) => {
                let overlay = hwnd.0 as *mut core::ffi::c_void;
                win32::square_corners_no_border(overlay);
                match win32::attach_to_wallpaper(overlay, x, y, w, h) {
                    // Attaching also shows the window at its place in the z-order.
                    Some(layout) => {
                        println!("[overlay] {label} attached ({layout})");
                        return true;
                    }
                    None => eprintln!("[overlay] desktop window not found for {label}"),
                }
            }
            Err(e) => eprintln!("[overlay] hwnd() failed for {label}: {e}"),
        }
    }

    let _ = win.destroy();
    false
}

/// True while the overlay's native window still exists. Explorer restarting
/// destroys the desktop window and every overlay parented into it.
fn overlay_alive(app: &AppHandle, label: &str) -> bool {
    let Some(win) = app.get_webview_window(label) else {
        return false;
    };
    #[cfg(windows)]
    {
        match win.hwnd() {
            Ok(hwnd) => crate::win32::is_window(hwnd.0 as *mut core::ffi::c_void),
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = win;
        true
    }
}

/// Background check that puts the wallpaper back after Explorer restarts.
pub fn spawn_watchdog(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(3));

        let (running, labels, cfg) = {
            let state = app.state::<AppState>();
            let inner = state.inner.lock().unwrap();
            (
                inner.is_running,
                inner.overlay_labels.clone(),
                inner.last_config.clone(),
            )
        };
        let Some(cfg) = cfg else { continue };
        if !running || labels.iter().all(|l| overlay_alive(&app, l)) {
            continue;
        }

        println!("[watchdog] overlay window lost; waiting for the desktop to come back");
        #[cfg(windows)]
        {
            // Give Explorer up to a minute to recreate the desktop, then let it settle.
            for _ in 0..60 {
                if crate::win32::desktop_ready() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        // The user may have pressed stop while we were waiting.
        if !app.state::<AppState>().inner.lock().unwrap().is_running {
            continue;
        }
        // Explorer can take a moment to finish building the desktop; try a few
        // times before telling the user.
        let mut outcome = start_overlays(&app, &cfg);
        for _ in 0..4 {
            if outcome.active && !outcome.attach_failed {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
            outcome = start_overlays(&app, &cfg);
        }
        if outcome.attach_failed {
            report_attach_failure(&app);
        }
        notify_status(&app);
    });
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
/// the overlay windows. Reports whether any overlay is showing and whether any
/// could not be attached to the desktop.
pub fn start_overlays(app: &AppHandle, cfg: &Config) -> StartOutcome {
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
            // An explicit file list wins over the folder.
            let mut images = match (&m.files, &m.folder) {
                (Some(files), _) if !files.is_empty() => filter_images(files),
                (_, Some(folder)) => list_images(folder),
                _ => {
                    println!("[start] display {} has no source, skip", d.id);
                    continue;
                }
            };
            println!("[start] display {} images={}", d.id, images.len());
            if images.is_empty() {
                continue;
            }
            if cfg.shuffle {
                shuffle(&mut images);
            }
            let (x, y, w, h) = resolve_bounds(d, &m.region);
            let key = sanitize(&d.id);
            // A fresh label every time: a window whose native handle died (Explorer
            // restart) or is still being torn down must never collide with its
            // replacement.
            let label = format!(
                "overlay-{key}-{}",
                OVERLAY_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            );
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

    // Build directly on the command thread. Tauri v2's `build()` dispatches
    // window creation to the main event loop internally, so wrapping this in
    // `run_on_main_thread` would deadlock (waiting for the main thread from it).
    let mut failed: Vec<(String, String)> = Vec::new();
    for (label, key, x, y, w, h) in plan {
        if !build_overlay(app, &label, &key, x, y, w, h) {
            failed.push((label, key));
        }
    }

    // Forget the overlays that never made it onto the desktop.
    let active = {
        let state = app.state::<AppState>();
        let mut inner = state.inner.lock().unwrap();
        for (label, key) in &failed {
            inner.overlay_labels.retain(|l| l != label);
            inner.overlays.remove(key);
        }
        inner.is_running = !inner.overlay_labels.is_empty();
        inner.is_running
    };

    let attach_failed = !failed.is_empty();
    StartOutcome { active, attach_failed }
}

/// Tell the control panel that the wallpaper could not be put on the desktop.
pub fn report_attach_failure(app: &AppHandle) {
    let _ = app.emit(
        "error",
        serde_json::json!({
            "message": "바탕화면에 배경을 붙이지 못했습니다. Windows 업데이트로 바탕화면 구조가 바뀌었을 수 있습니다. 앱을 최신 버전으로 업데이트해 보세요."
        }),
    );
}

pub struct StartOutcome {
    pub active: bool,
    pub attach_failed: bool,
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
