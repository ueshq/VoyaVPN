//! One-time native privilege elevation for TUN, on Linux only.
//!
//! Instead of storing an admin password and piping it to `sudo -S`, the app
//! asks the OS once (`pkexec`) to install a fixed-path, root-owned launcher
//! plus a `NOPASSWD` sudoers drop-in that authorizes only that launcher.
//! Subsequent core start/stop run passwordlessly through the launcher; the
//! launcher is removed on exit. The password never touches the app process.
//!
//! macOS has no elevation path at all: its tunnel runs in the NetworkExtension
//! provider, and the installer is not compiled into a macOS binary (App Review
//! Guideline 2.4.5; ADR 0004). Windows runs its tunnel in a service.
//!
//! Security note: the core binaries live in a user-writable app directory, so a
//! `NOPASSWD` grant cannot fully eliminate local privilege-escalation risk if
//! an attacker can already run code as the same user and replace a core binary.
//! The fixed root-owned launcher (the only sudoers target) and its path checks
//! contain the blast radius but do not remove that residual risk.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use thiserror::Error;

use crate::{
    coreinfo::TargetOs,
    elevation::{sudo_launcher_arguments, SUDO_EXECUTABLE},
    process::{ProcessRole, ProcessSpawn, HELPER_TIMEOUT},
};

#[cfg(any(target_os = "linux", test))]
mod linux_installer;

const LINUX_LAUNCHER_DIR: &str = "/usr/libexec/voya-vpn";
const LAUNCHER_FILE_NAME: &str = "voya-elevate";

/// Shared "elevation has been granted this session" flag.
///
/// The same `Arc<ElevationState>` is wired into the supervisor (decides whether
/// to spawn/kill via the launcher) and the TUN status reporting (decides
/// `allow_enable_tun`).
#[derive(Debug, Default)]
pub struct ElevationState {
    granted: AtomicBool,
}

impl ElevationState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_granted(&self) -> bool {
        self.granted.load(Ordering::SeqCst)
    }

    pub fn set_granted(&self, granted: bool) {
        self.granted.store(granted, Ordering::SeqCst);
    }
}

/// Directory that holds the root-owned launcher for `os`.
#[must_use]
pub fn elevate_launcher_dir(os: TargetOs) -> Option<PathBuf> {
    match os {
        TargetOs::Linux => Some(PathBuf::from(LINUX_LAUNCHER_DIR)),
        TargetOs::Macos
        | TargetOs::Windows
        | TargetOs::Ios
        | TargetOs::Android
        | TargetOs::Other => None,
    }
}

/// Absolute path to the root-owned elevation launcher for `os`.
#[must_use]
pub fn elevate_launcher_path(os: TargetOs) -> Option<PathBuf> {
    elevate_launcher_dir(os).map(|dir| dir.join(LAUNCHER_FILE_NAME))
}

/// Read the invoking user's login name (before any elevation).
#[cfg(unix)]
#[must_use]
pub fn current_username() -> Option<String> {
    use std::{ffi::CStr, mem::MaybeUninit, ptr};

    // The name ends up in a root-owned sudoers rule. `getpwuid` returns a
    // shared static that any other passwd lookup in the process (WebKit, a
    // plugin) may overwrite before it is copied; `getpwuid_r` fills only the
    // buffers passed to it.
    let mut buffer: Vec<libc::c_char> = vec![0; 1024];
    loop {
        let mut entry = MaybeUninit::<libc::passwd>::uninit();
        let mut found: *mut libc::passwd = ptr::null_mut();
        // SAFETY: `geteuid` has no preconditions; every pointer names storage
        // owned by this frame, and `buffer.len()` is the buffer's real size.
        let status = unsafe {
            libc::getpwuid_r(
                libc::geteuid(),
                entry.as_mut_ptr(),
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut found,
            )
        };
        if status == libc::ERANGE && buffer.len() < 1 << 20 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status != 0 || found.is_null() {
            return None;
        }
        // SAFETY: success with a non-null `found` means `entry` is initialised
        // and `pw_name` is null or a C string inside `buffer`, still alive here.
        let name = unsafe { (*found).pw_name };
        if name.is_null() {
            return None;
        }
        // SAFETY: as above, `name` is a NUL-terminated string in `buffer`.
        return unsafe { CStr::from_ptr(name) }
            .to_str()
            .ok()
            .map(str::to_owned);
    }
}

#[cfg(not(unix))]
#[must_use]
pub fn current_username() -> Option<String> {
    None
}

/// Everything the caller needs to perform a one-time native elevation: the
/// files to write (as the user) and the privileged command that installs them.
#[derive(Debug, Clone)]
pub struct ElevationInstallPlan {
    pub work_dir: PathBuf,
    pub src_launcher_path: PathBuf,
    pub src_sudoers_path: PathBuf,
    pub install_script_path: PathBuf,
    pub launcher_contents: String,
    pub sudoers_contents: String,
    pub install_script_contents: String,
    pub command: ProcessSpawn,
}

/// Build the install plan for a one-time native elevation.
///
/// `bin_prefix` is the user app `bin` directory; only core binaries under it
/// are accepted by the launcher's `run` verb. `work_dir` is a user-owned scratch
/// directory where the install sources are staged. Every OS but Linux gets
/// [`PrivilegeError::UnsupportedOs`].
#[cfg(target_os = "linux")]
pub fn build_install_plan(
    os: TargetOs,
    username: &str,
    bin_prefix: &Path,
    work_dir: &Path,
) -> Result<ElevationInstallPlan, PrivilegeError> {
    match os {
        TargetOs::Linux => linux_installer::build_install_plan(username, bin_prefix, work_dir),
        TargetOs::Macos
        | TargetOs::Windows
        | TargetOs::Ios
        | TargetOs::Android
        | TargetOs::Other => Err(PrivilegeError::UnsupportedOs),
    }
}

/// See the Linux definition. The installer is not compiled into this binary.
#[cfg(not(target_os = "linux"))]
pub fn build_install_plan(
    _os: TargetOs,
    _username: &str,
    _bin_prefix: &Path,
    _work_dir: &Path,
) -> Result<ElevationInstallPlan, PrivilegeError> {
    Err(PrivilegeError::UnsupportedOs)
}

/// Passwordless `sudo -n` plan that drives the launcher's self-removing
/// `uninstall` verb (removes the sudoers drop-in and the launcher directory).
pub fn build_uninstall_spawn(os: TargetOs) -> Result<ProcessSpawn, PrivilegeError> {
    let launcher = elevate_launcher_path(os).ok_or(PrivilegeError::UnsupportedOs)?;
    // Bounded like every other `sudo -n` helper: the revoke runs on the main
    // thread at startup and at exit, where one that never answers would hang
    // the window or the quit.
    Ok(ProcessSpawn::new(ProcessRole::SudoKill, SUDO_EXECUTABLE)
        .with_arguments(sudo_launcher_arguments(&launcher, "uninstall"))
        .with_display_log(false)
        .with_timeout(HELPER_TIMEOUT))
}

/// Classify the outcome of a native elevation command from its exit code.
#[must_use]
pub fn classify_elevation_outcome(status_code: Option<i32>) -> ElevationOutcome {
    if status_code == Some(0) {
        return ElevationOutcome::Granted;
    }
    // pkexec uses exit code 126 for "dismissed / not authorized".
    if status_code == Some(126) {
        return ElevationOutcome::Cancelled;
    }
    ElevationOutcome::Failed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationOutcome {
    Granted,
    Cancelled,
    Failed,
}

#[derive(Debug, Error)]
pub enum PrivilegeError {
    #[error("native privilege elevation is not supported on this platform")]
    UnsupportedOs,
    #[error("could not determine the current user for the elevation grant")]
    MissingUsername,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn privilege_current_username_matches_the_effective_user() {
        let expected = std::process::Command::new("id")
            .arg("-un")
            .output()
            .expect("id -un");
        let expected = String::from_utf8_lossy(&expected.stdout).trim().to_string();

        assert_eq!(current_username(), Some(expected));
    }

    #[test]
    fn privilege_elevation_state_tracks_grant() {
        let state = ElevationState::new();
        assert!(!state.is_granted());
        state.set_granted(true);
        assert!(state.is_granted());
        state.set_granted(false);
        assert!(!state.is_granted());
    }

    #[test]
    fn privilege_launcher_path_is_fixed_per_platform() {
        assert_eq!(
            elevate_launcher_path(TargetOs::Linux),
            Some(PathBuf::from("/usr/libexec/voya-vpn/voya-elevate"))
        );
        // macOS has no launcher: its tunnel is the NetworkExtension provider.
        assert_eq!(elevate_launcher_path(TargetOs::Macos), None);
        assert_eq!(elevate_launcher_path(TargetOs::Windows), None);
    }

    #[test]
    fn privilege_install_plan_is_unsupported_off_linux() {
        for os in [TargetOs::Macos, TargetOs::Windows, TargetOs::Ios] {
            assert!(matches!(
                build_install_plan(os, "afu", Path::new("/bin"), Path::new("/tmp")),
                Err(PrivilegeError::UnsupportedOs)
            ));
        }
        assert!(matches!(
            build_uninstall_spawn(TargetOs::Macos),
            Err(PrivilegeError::UnsupportedOs)
        ));
    }

    #[test]
    fn privilege_uninstall_runs_passwordless_through_launcher() {
        let spawn = build_uninstall_spawn(TargetOs::Linux).expect("uninstall spawn");
        assert_eq!(spawn.executable, PathBuf::from(SUDO_EXECUTABLE));
        assert_eq!(
            spawn.arguments,
            vec![
                "-n".to_string(),
                "--".to_string(),
                "/usr/libexec/voya-vpn/voya-elevate".to_string(),
                "uninstall".to_string(),
            ]
        );
    }

    #[test]
    fn privilege_classifies_native_outcomes() {
        assert_eq!(
            classify_elevation_outcome(Some(0)),
            ElevationOutcome::Granted
        );
        // pkexec exits 126 when the prompt is dismissed.
        assert_eq!(
            classify_elevation_outcome(Some(126)),
            ElevationOutcome::Cancelled
        );
        assert_eq!(
            classify_elevation_outcome(Some(1)),
            ElevationOutcome::Failed
        );
        assert_eq!(classify_elevation_outcome(None), ElevationOutcome::Failed);
    }
}
