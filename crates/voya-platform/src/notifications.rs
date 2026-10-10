//! OS notifications through `UNUserNotificationCenter`.
//!
//! The first one asks the user to allow them, in the system's own prompt.
//! Tauri's notification plugin posts through `NSUserNotificationCenter`
//! instead, which is deprecated and has no such request.

use std::ffi::CString;

use libc::c_char;

// Implemented in `native/macos_notifications.m`, which this crate's `build.rs`
// compiles and links. It copies both strings before returning, and `body` may
// be null.
// SAFETY: calling this function only requires that contract.
unsafe extern "C" {
    fn voya_macos_post_notification(title: *const c_char, body: *const c_char) -> bool;
}

/// Posts a notification. `false` means the system could not be asked: the
/// process is not an app bundle, as under `tauri dev`. A user who has turned
/// notifications off still gets `true`.
pub fn post(title: &str, body: Option<&str>) -> bool {
    let Ok(title) = CString::new(title) else {
        return false;
    };
    let body = body.and_then(|body| CString::new(body).ok());
    let body_pointer = body.as_ref().map_or(std::ptr::null(), |body| body.as_ptr());
    // SAFETY: `title` and `body` are live NUL-terminated strings for the whole
    // call, and a null `body` is allowed.
    unsafe { voya_macos_post_notification(title.as_ptr(), body_pointer) }
}
