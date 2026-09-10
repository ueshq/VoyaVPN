//! AppKit preserves native caption actions while keeping their inset after layout.

use std::ffi::c_void;

// SAFETY: Implemented by the AppKit bridge compiled into this crate on macOS.
unsafe extern "C" {
    fn voya_install_window_chrome(window: *mut c_void, left: f64, top: f64);
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
