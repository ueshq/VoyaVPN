//! AppKit preserves native caption actions while keeping their inset after layout.

use std::ffi::c_void;

// SAFETY: Implemented by the AppKit bridge compiled into this crate on macOS.
unsafe extern "C" {
    fn voya_install_window_chrome(window: *mut c_void, left: f64, top: f64);
    fn voya_hide_window_leaving_fullscreen(window: *mut c_void);
    fn voya_raise_window(window: *mut c_void);
}

/// Keep the native traffic lights inset when AppKit relays out the titlebar.
///
/// # Safety
/// `window` must be a live NSWindow, and this must be called on the main thread.
/// The bridge keeps only a weak window reference after this call returns.
// SAFETY: Callers must uphold the live-window and main-thread requirements above.
pub unsafe fn install_native_caption_inset(window: *mut c_void, left: f64, top: f64) {
    // SAFETY: The caller supplies a live NSWindow on the AppKit main thread.
    unsafe { voya_install_window_chrome(window, left, top) };
}

/// Hide a window, leaving full screen first: a full-screen window that is
/// simply ordered out leaves its Space behind as an empty black screen. The
/// hide then happens once the exit animation has finished.
///
/// # Safety
/// `window` must be a live NSWindow, and this must be called on the main thread.
// SAFETY: Callers must uphold the live-window and main-thread requirements above.
pub unsafe fn hide_leaving_fullscreen(window: *mut c_void) {
    // SAFETY: The caller supplies a live NSWindow on the AppKit main thread.
    unsafe { voya_hide_window_leaving_fullscreen(window) };
}

/// Put a shown window in front of other apps and ask the system to activate
/// this one, without relying on the deprecated activation the windowing
/// library uses.
///
/// # Safety
/// `window` must be a live NSWindow, and this must be called on the main thread.
// SAFETY: Callers must uphold the live-window and main-thread requirements above.
pub unsafe fn raise(window: *mut c_void) {
    // SAFETY: The caller supplies a live NSWindow on the AppKit main thread.
    unsafe { voya_raise_window(window) };
}
