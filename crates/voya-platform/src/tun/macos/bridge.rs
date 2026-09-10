//! Objective-C bridge to the macOS PacketTunnel `NETunnelProviderManager`.
//!
//! Every entry point returns an owned C string that the Rust side copies and
//! frees exactly once; the non-macOS variants keep the callers compiling on
//! other platforms.

use crate::tun::{NativeTunError, TunBackend};

#[cfg(target_os = "macos")]
mod macos_packet_tunnel_bridge {
    use std::ffi::{CStr, CString};

    use libc::c_char;

    use super::{NativeTunError, TunBackend};

    // These declarations mirror the `voya_macos_packet_tunnel_*` definitions in
    // `crates/voya-platform/native/macos_packet_tunnel_bridge.m`, which this
    // crate's own `build.rs` compiles and statically links, so the two sides
    // cannot drift apart at run time: `char *` is `*mut c_char`, `const char *`
    // is `*const c_char`, `int64_t` is `i64`, and no declaration is variadic.
    // The Objective-C side copies each borrowed argument into an `NSString`
    // before returning and never retains a Rust pointer or calls back into
    // Rust, so arguments only have to stay valid for the duration of the call.
    // Every string return is a `strdup` buffer whose ownership moves to the
    // caller, is never aliased, and is released exactly once by
    // `voya_macos_packet_tunnel_free` (a plain `free`, the matching allocator).
    // SAFETY: calling these functions therefore only requires the argument
    // lifetime and return ownership contract described above.
    unsafe extern "C" {
        fn voya_macos_packet_tunnel_status() -> *mut c_char;
        fn voya_macos_packet_tunnel_start(
            config_path: *const c_char,
            profile_id: *const c_char,
            timeout_ms: i64,
        ) -> *mut c_char;
        fn voya_macos_packet_tunnel_stop() -> *mut c_char;
        fn voya_macos_packet_tunnel_last_error() -> *mut c_char;
        fn voya_macos_packet_tunnel_container_path() -> *mut c_char;
        fn voya_macos_packet_tunnel_free(value: *mut c_char);
    }

    pub fn status() -> Result<String, NativeTunError> {
        bridge_string("query macOS PacketTunnel status", || {
            // SAFETY: the Objective-C bridge takes no arguments and returns an
            // owned C string that `bridge_string` validates and releases.
            unsafe { voya_macos_packet_tunnel_status() }
        })
    }

    pub fn start(
        config_path: &str,
        profile_id: Option<&str>,
        timeout_ms: i64,
    ) -> Result<String, NativeTunError> {
        let config_path = c_string(config_path, "main config path")?;
        let profile_id = match profile_id {
            Some(profile_id) => Some(c_string(profile_id, "active node id")?),
            None => None,
        };
        bridge_string("start macOS PacketTunnel", || {
            // SAFETY: both C strings remain alive for the duration of the call;
            // the optional profile pointer is either valid or null.
            unsafe {
                voya_macos_packet_tunnel_start(
                    config_path.as_ptr(),
                    profile_id
                        .as_ref()
                        .map_or(std::ptr::null(), |profile_id| profile_id.as_ptr()),
                    timeout_ms,
                )
            }
        })
    }

    pub fn stop() -> Result<String, NativeTunError> {
        bridge_string("stop macOS PacketTunnel", || {
            // SAFETY: the bridge takes no arguments and returns an owned C
            // string that `bridge_string` validates and releases.
            unsafe { voya_macos_packet_tunnel_stop() }
        })
    }

    pub fn last_error() -> Result<String, NativeTunError> {
        bridge_string("query macOS PacketTunnel last error", || {
            // SAFETY: the bridge takes no arguments and returns an owned C
            // string that `bridge_string` validates and releases.
            unsafe { voya_macos_packet_tunnel_last_error() }
        })
    }

    pub fn container_path() -> Result<String, NativeTunError> {
        bridge_string("query macOS PacketTunnel container path", || {
            // SAFETY: the bridge takes no arguments and returns an owned C
            // string that `bridge_string` validates and releases.
            unsafe { voya_macos_packet_tunnel_container_path() }
        })
    }

    fn c_string(value: &str, label: &'static str) -> Result<CString, NativeTunError> {
        CString::new(value).map_err(|_| NativeTunError::InvalidRequest {
            backend: TunBackend::MacosPacketTunnel,
            message: format!("{label} contains an interior NUL byte"),
        })
    }

    fn bridge_string(
        action: &'static str,
        invoke: impl FnOnce() -> *mut c_char,
    ) -> Result<String, NativeTunError> {
        let value = invoke();
        if value.is_null() {
            return Err(NativeTunError::CommandFailed {
                action,
                status_code: None,
                output: "macOS PacketTunnel bridge returned a null response".to_string(),
            });
        }

        // SAFETY: the null check above and the bridge contract guarantee that
        // `value` points to a NUL-terminated string until it is freed below.
        let output = unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: `value` was allocated by the bridge and is released exactly
        // once after its contents have been copied into a Rust String.
        unsafe {
            voya_macos_packet_tunnel_free(value);
        }
        Ok(output)
    }
}

#[cfg(target_os = "macos")]
pub(super) fn macos_packet_tunnel_bridge_status() -> Result<String, NativeTunError> {
    macos_packet_tunnel_bridge::status()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn macos_packet_tunnel_bridge_status() -> Result<String, NativeTunError> {
    Err(NativeTunError::ComponentMissing {
        backend: TunBackend::MacosPacketTunnel,
        message: "PacketTunnel bridge is not available on this platform".to_string(),
    })
}

#[cfg(target_os = "macos")]
pub(super) fn macos_packet_tunnel_bridge_start(
    config_path: &str,
    profile_id: Option<&str>,
    timeout_ms: i64,
) -> Result<String, NativeTunError> {
    macos_packet_tunnel_bridge::start(config_path, profile_id, timeout_ms)
}

#[cfg(target_os = "macos")]
pub(super) fn macos_packet_tunnel_bridge_stop() -> Result<String, NativeTunError> {
    macos_packet_tunnel_bridge::stop()
}

#[cfg(target_os = "macos")]
pub(super) fn macos_packet_tunnel_bridge_last_error() -> Result<String, NativeTunError> {
    macos_packet_tunnel_bridge::last_error()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn macos_packet_tunnel_bridge_last_error() -> Result<String, NativeTunError> {
    Ok(String::new())
}

#[cfg(target_os = "macos")]
pub(super) fn macos_packet_tunnel_bridge_container_path() -> Result<String, NativeTunError> {
    macos_packet_tunnel_bridge::container_path()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn macos_packet_tunnel_bridge_container_path() -> Result<String, NativeTunError> {
    Ok(String::new())
}
