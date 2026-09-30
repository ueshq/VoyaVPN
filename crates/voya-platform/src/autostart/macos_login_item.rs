//! Launch at login on macOS through `SMAppService`, bridged from
//! `native/macos_login_item.m`. The app registers the launchd agent bundled at
//! `Contents/Library/LaunchAgents`, so nothing is written outside the signed
//! bundle. Every other platform gets [`AutostartError::LoginItemUnavailable`].

use super::{AutostartError, LoginItemState};

#[cfg(target_os = "macos")]
mod bridge {
    use std::ffi::{CStr, CString};

    use libc::c_char;

    use super::{AutostartError, LoginItemState};

    // These declarations mirror `crates/voya-platform/native/macos_login_item.h`,
    // which this crate's `build.rs` compiles and statically links: `const char *`
    // is `*const c_char`, `char **` is `*mut *mut c_char`, `int32_t` is `i32`,
    // and nothing is variadic. The Objective-C side copies the plist name into
    // an `NSString` before returning and keeps no Rust pointer. It sets `*error`
    // to null or to a `strdup` buffer whose ownership moves to the caller and
    // which `voya_macos_login_item_free` (a plain `free`) releases once.
    // SAFETY: calling these functions only requires that contract.
    unsafe extern "C" {
        fn voya_macos_login_item_register(
            plist_name: *const c_char,
            error: *mut *mut c_char,
        ) -> i32;
        fn voya_macos_login_item_unregister(
            plist_name: *const c_char,
            error: *mut *mut c_char,
        ) -> i32;
        fn voya_macos_login_item_free(value: *mut c_char);
    }

    pub(super) fn register(plist_name: &str) -> Result<LoginItemState, AutostartError> {
        call("register", plist_name, |name, error| {
            // SAFETY: `name` is a live NUL-terminated string and `error` points
            // at a null-initialised local; see the contract above.
            unsafe { voya_macos_login_item_register(name, error) }
        })
    }

    pub(super) fn unregister(plist_name: &str) -> Result<LoginItemState, AutostartError> {
        call("unregister", plist_name, |name, error| {
            // SAFETY: as in `register`.
            unsafe { voya_macos_login_item_unregister(name, error) }
        })
    }

    fn call(
        operation: &'static str,
        plist_name: &str,
        invoke: impl FnOnce(*const c_char, *mut *mut c_char) -> i32,
    ) -> Result<LoginItemState, AutostartError> {
        let name = CString::new(plist_name).map_err(|_| AutostartError::LoginItem {
            operation,
            message: "login item plist name contains a NUL byte".to_string(),
        })?;
        let mut error: *mut c_char = std::ptr::null_mut();
        let code = invoke(name.as_ptr(), &mut error);
        let message = take_message(error);
        super::state_from_code(operation, code, message)
    }

    fn take_message(error: *mut c_char) -> Option<String> {
        if error.is_null() {
            return None;
        }
        // SAFETY: a non-null `error` is a NUL-terminated `strdup` buffer that
        // stays valid until it is freed below.
        let message = unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: the buffer came from the bridge and is released exactly once,
        // after its contents were copied.
        unsafe { voya_macos_login_item_free(error) };
        Some(message)
    }
}

#[cfg(not(target_os = "macos"))]
mod bridge {
    use super::{AutostartError, LoginItemState};

    fn unavailable() -> Result<LoginItemState, AutostartError> {
        Err(AutostartError::LoginItemUnavailable {
            reason: "login items are a macOS feature".to_string(),
        })
    }

    pub(super) fn register(_plist_name: &str) -> Result<LoginItemState, AutostartError> {
        unavailable()
    }

    pub(super) fn unregister(_plist_name: &str) -> Result<LoginItemState, AutostartError> {
        unavailable()
    }
}

pub(super) fn set_enabled(
    plist_name: &str,
    enabled: bool,
) -> Result<LoginItemState, AutostartError> {
    if enabled {
        bridge::register(plist_name)
    } else {
        bridge::unregister(plist_name)
    }
}

/// Maps a bridge return code (see `macos_login_item.h`) to a state or error.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn state_from_code(
    operation: &'static str,
    code: i32,
    message: Option<String>,
) -> Result<LoginItemState, AutostartError> {
    match code {
        0 => Ok(LoginItemState::NotRegistered),
        1 => Ok(LoginItemState::Enabled),
        2 => Ok(LoginItemState::RequiresApproval),
        3 => Ok(LoginItemState::NotFound),
        -2 => Err(AutostartError::LoginItemUnavailable {
            reason: message.unwrap_or_else(|| "login items are unavailable".to_string()),
        }),
        -1 => Err(AutostartError::LoginItem {
            operation,
            message: message.unwrap_or_else(|| "no error message".to_string()),
        }),
        other => Err(AutostartError::LoginItem {
            operation,
            message: format!("unexpected bridge result {other}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_codes_map_to_states_and_errors() {
        assert_eq!(
            state_from_code("query", 0, None).ok(),
            Some(LoginItemState::NotRegistered)
        );
        assert_eq!(
            state_from_code("register", 1, None).ok(),
            Some(LoginItemState::Enabled)
        );
        assert_eq!(
            state_from_code("register", 2, None).ok(),
            Some(LoginItemState::RequiresApproval)
        );
        assert_eq!(
            state_from_code("query", 3, None).ok(),
            Some(LoginItemState::NotFound)
        );
        assert!(matches!(
            state_from_code("register", -2, Some("not in a bundle".into())),
            Err(AutostartError::LoginItemUnavailable { reason }) if reason == "not in a bundle"
        ));
        assert!(matches!(
            state_from_code("register", -1, Some("denied".into())),
            Err(AutostartError::LoginItem { operation: "register", message }) if message == "denied"
        ));
        assert!(matches!(
            state_from_code("unregister", 7, None),
            Err(AutostartError::LoginItem { message, .. }) if message.contains('7')
        ));
    }
}
