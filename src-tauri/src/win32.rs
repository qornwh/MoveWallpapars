//! Win32 glue for pinning overlay windows to the real desktop wallpaper layer.
//!
//! Goal: the slideshow renders *behind* the desktop icons and is never covered
//! by other application windows — the Wallpaper Engine / Lively behaviour.
//!
//! Windows exposes this layer differently across builds:
//!   * Win10 / older Win11: a dedicated `WorkerW` window sits behind the icons.
//!   * Newer Win11 (22H2+): `0x052C` may not spawn a separate WorkerW; the icon
//!     host `SHELLDLL_DefView` is a direct child of `Progman`.
//!
//! We therefore try, in order: the classic WorkerW sibling, a WorkerW child of
//! Progman, and finally Progman itself — reparenting the overlay into whichever
//! we find and pushing it to the bottom of the z-order so the icons stay on top.

#![cfg(windows)]

use std::ffi::c_void;
use std::iter::once;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetSystemMetrics, SendMessageTimeoutW, SetParent,
    SetWindowPos, HWND_BOTTOM, SMTO_NORMAL, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE,
    SWP_SHOWWINDOW,
};

const DWMWCP_DONOTROUND: u32 = 1;
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
/// Undocumented Progman message that makes the shell spawn a WorkerW behind icons.
const WM_SPAWN_WORKERW: u32 = 0x052C;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(once(0)).collect()
}

/// Locate the window the overlay should be reparented into so it renders as the
/// desktop wallpaper. Returns the host HWND (a WorkerW when available, otherwise
/// Progman itself). `None` only if Progman can't be found at all.
pub fn find_wallpaper_host() -> Option<HWND> {
    unsafe {
        let progman = FindWindowW(wide("Progman").as_ptr(), null());
        if progman.is_null() {
            return None;
        }

        // Nudge the shell into creating the WorkerW layer. Two variants cover
        // different Windows builds; both are harmless if ignored.
        let mut result: usize = 0;
        SendMessageTimeoutW(progman, WM_SPAWN_WORKERW, 0, 0, SMTO_NORMAL, 1000, &mut result);
        SendMessageTimeoutW(progman, WM_SPAWN_WORKERW, 0x0D, 0x01, SMTO_NORMAL, 1000, &mut result);

        // Strategy A: the WorkerW that is a sibling *after* the SHELLDLL_DefView host.
        let mut found: HWND = null_mut();
        EnumWindows(Some(enum_proc), &mut found as *mut HWND as LPARAM);
        if !found.is_null() {
            return Some(found);
        }

        // Strategy B: a WorkerW that is a direct child of Progman.
        let worker = FindWindowExW(progman, null_mut(), wide("WorkerW").as_ptr(), null());
        if !worker.is_null() {
            return Some(worker);
        }

        // Strategy C: no WorkerW on this build — reparent into Progman directly
        // and rely on the bottom z-order push to stay behind the icons.
        Some(progman)
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

/// Reparent `overlay` into `host` and position it (physical pixels, absolute
/// virtual-desktop coordinates), pushed to the bottom of the z-order so desktop
/// icons remain visible on top. After SetParent the child's coordinates become
/// relative to the host origin, which equals the virtual screen's top-left.
pub fn attach_to_wallpaper(overlay: HWND, host: HWND, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        SetParent(overlay, host);
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        SetWindowPos(
            overlay,
            HWND_BOTTOM,
            x - vx,
            y - vy,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
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
