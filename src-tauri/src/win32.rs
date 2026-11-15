//! Win32 glue for pinning overlay windows to the real desktop wallpaper layer.
//!
//! Goal: the slideshow renders *behind* the desktop icons and is never covered
//! by other application windows — the Wallpaper Engine / Lively behaviour.
//!
//! Windows exposes this layer in three shapes, and the same machine can switch
//! between them across OS updates:
//!
//!   * Classic (Win10, older Win11): `0x052C` makes the shell spawn a top-level
//!     `WorkerW` right behind the window that hosts the icons. We reparent into
//!     that WorkerW.
//!   * Raised desktop (Win11 24H2+): `Progman` itself has two children — the
//!     icon host `SHELLDLL_DefView` and a `WorkerW` that paints the wallpaper.
//!     We reparent into Progman and sit *between* those two. Progman has no
//!     redirection bitmap in this mode, so the overlay must be a layered window
//!     or it is never composed.
//!   * Neither: reparent into Progman at the bottom of its z-order.

#![cfg(windows)]

use std::ffi::c_void;
use std::iter::once;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetSystemMetrics, GetWindowLongPtrW, IsWindow,
    SendMessageTimeoutW, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW, SetWindowPos,
    GWL_EXSTYLE, HWND_BOTTOM, LWA_ALPHA, SMTO_NORMAL, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, WS_EX_LAYERED,
};

const DWMWCP_DONOTROUND: u32 = 1;
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
/// Undocumented Progman message that makes the shell set up the wallpaper layer.
const WM_SPAWN_WORKERW: u32 = 0x052C;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(once(0)).collect()
}

/// Where an overlay gets attached, by desktop layout.
enum Host {
    /// Classic: fill the WorkerW behind the icon host.
    Worker(HWND),
    /// Raised desktop: child of Progman, directly below the icon host.
    Raised { progman: HWND, defview: HWND },
    /// Last resort: child of Progman at the bottom.
    Progman(HWND),
}

fn find_host() -> Option<Host> {
    unsafe {
        let progman = FindWindowW(wide("Progman").as_ptr(), null());
        if progman.is_null() {
            return None;
        }

        // Nudge the shell into creating the wallpaper layer. Two variants cover
        // different Windows builds; both are harmless if ignored.
        let mut result: usize = 0;
        SendMessageTimeoutW(progman, WM_SPAWN_WORKERW, 0, 0, SMTO_NORMAL, 1000, &mut result);
        SendMessageTimeoutW(progman, WM_SPAWN_WORKERW, 0x0D, 0x01, SMTO_NORMAL, 1000, &mut result);

        // The shell builds the layer asynchronously (noticeably so right after
        // Explorer restarts), so look for it a few times before settling for
        // the bare Progman fallback.
        for attempt in 0..15 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            // Classic: the WorkerW that is a sibling *after* the SHELLDLL_DefView host.
            let mut found: HWND = null_mut();
            EnumWindows(Some(enum_proc), &mut found as *mut HWND as LPARAM);
            if !found.is_null() {
                return Some(Host::Worker(found));
            }

            // Raised desktop: Progman owns both the icon host and the wallpaper WorkerW.
            let defview =
                FindWindowExW(progman, null_mut(), wide("SHELLDLL_DefView").as_ptr(), null());
            let worker = FindWindowExW(progman, null_mut(), wide("WorkerW").as_ptr(), null());
            if !defview.is_null() && !worker.is_null() {
                return Some(Host::Raised { progman, defview });
            }
            if !worker.is_null() {
                return Some(Host::Worker(worker));
            }
        }

        Some(Host::Progman(progman))
    }
}

unsafe extern "system" fn enum_proc(top: HWND, lparam: LPARAM) -> i32 {
    // Does this top-level window host the desktop icons?
    let shell = FindWindowExW(top, null_mut(), wide("SHELLDLL_DefView").as_ptr(), null());
    if !shell.is_null() {
        // The WorkerW we want is the next sibling of this window.
        let worker = FindWindowExW(null_mut(), top, wide("WorkerW").as_ptr(), null());
        if !worker.is_null() {
            let out = lparam as *mut HWND;
            *out = worker;
            return 0; // found it — stop enumeration
        }
    }
    1 // continue
}

/// Reparent `overlay` into the desktop wallpaper layer and position it
/// (physical pixels, absolute virtual-desktop coordinates). Returns a short
/// label of the layout used, or `None` if the desktop window was not found.
///
/// After SetParent the child's coordinates are relative to the host origin,
/// which equals the virtual screen's top-left.
pub fn attach_to_wallpaper(overlay: HWND, x: i32, y: i32, w: i32, h: i32) -> Option<&'static str> {
    let host = find_host()?;
    unsafe {
        let cx = x - GetSystemMetrics(SM_XVIRTUALSCREEN);
        let cy = y - GetSystemMetrics(SM_YVIRTUALSCREEN);
        let flags = SWP_NOACTIVATE | SWP_SHOWWINDOW;
        match host {
            Host::Worker(worker) => {
                SetParent(overlay, worker);
                SetWindowPos(overlay, HWND_BOTTOM, cx, cy, w, h, flags);
                Some("workerw")
            }
            Host::Raised { progman, defview } => {
                // Must be layered to be composed under a Progman that has no
                // redirection bitmap.
                let ex = GetWindowLongPtrW(overlay, GWL_EXSTYLE);
                SetWindowLongPtrW(overlay, GWL_EXSTYLE, ex | WS_EX_LAYERED as isize);
                SetLayeredWindowAttributes(overlay, 0, 255, LWA_ALPHA);
                SetParent(overlay, progman);
                // Insert right below the icon host: above the wallpaper WorkerW,
                // beneath the icons.
                SetWindowPos(overlay, defview, cx, cy, w, h, flags);
                Some("raised-desktop")
            }
            Host::Progman(progman) => {
                SetParent(overlay, progman);
                SetWindowPos(overlay, HWND_BOTTOM, cx, cy, w, h, flags);
                Some("progman")
            }
        }
    }
}

/// Whether a native window handle still refers to a live window.
pub fn is_window(hwnd: HWND) -> bool {
    unsafe { IsWindow(hwnd) != 0 }
}

/// Whether Explorer's desktop window currently exists.
pub fn desktop_ready() -> bool {
    unsafe { !FindWindowW(wide("Progman").as_ptr(), null()).is_null() }
}

/// Disable the Windows 11 rounded corners and 1px border on a frameless window.
/// Harmlessly ignored on Windows 10.
pub fn square_corners_no_border(overlay: HWND) {
    unsafe {
        let pref: u32 = DWMWCP_DONOTROUND;
        DwmSetWindowAttribute(
            overlay,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &pref as *const u32 as *const c_void,
            4,
        );
        let color: u32 = DWMWA_COLOR_NONE;
        DwmSetWindowAttribute(
            overlay,
            DWMWA_BORDER_COLOR as u32,
            &color as *const u32 as *const c_void,
            4,
        );
    }
}
