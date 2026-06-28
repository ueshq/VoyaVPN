//! One-time native privilege elevation for TUN.
//!
//! Instead of storing an admin password and piping it to `sudo -S`, the app
//! asks the OS once (macOS `osascript ... with administrator privileges`,
//! Linux `pkexec`) to install a fixed-path, root-owned launcher plus a
//! `NOPASSWD` sudoers drop-in that authorizes only that launcher. Subsequent
//! core start/stop run passwordlessly through the launcher; the launcher is
//! removed on exit. The password never touches the app process.
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
    elevation::{quote_shell_arg, unix_sudo_kill_body, SUDO_EXECUTABLE},
    process::{ProcessRole, ProcessSpawn},
};

/// Fixed sudoers drop-in path. The name has no `.` so sudo loads it (sudo
/// ignores files in `sudoers.d` whose name contains a dot).
pub const SUDOERS_DROP_IN_PATH: &str = "/etc/sudoers.d/voya-vpn";

const MACOS_LAUNCHER_DIR: &str = "/usr/local/libexec/voya-vpn";
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
        TargetOs::Macos => Some(PathBuf::from(MACOS_LAUNCHER_DIR)),
        TargetOs::Linux => Some(PathBuf::from(LINUX_LAUNCHER_DIR)),
        TargetOs::Windows | TargetOs::Other => None,
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
    use std::ffi::CStr;

    // SAFETY: `getpwuid` returns a pointer into static storage that we copy out
    // immediately; `geteuid` has no preconditions.
    unsafe {
        let entry = libc::getpwuid(libc::geteuid());
        if entry.is_null() {
            return None;
        }
        let name = (*entry).pw_name;
        if name.is_null() {
            return None;
        }
        CStr::from_ptr(name).to_str().ok().map(str::to_owned)
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
/// directory where the install sources are staged.
pub fn build_install_plan(
    os: TargetOs,
    username: &str,
    bin_prefix: &Path,
    work_dir: &Path,
) -> Result<ElevationInstallPlan, PrivilegeError> {
    let launcher_dir = elevate_launcher_dir(os).ok_or(PrivilegeError::UnsupportedOs)?;
    let launcher_path = launcher_dir.join(LAUNCHER_FILE_NAME);
    if username.is_empty() {
        return Err(PrivilegeError::MissingUsername);
    }

    let src_launcher_path = work_dir.join(LAUNCHER_FILE_NAME);
    let src_sudoers_path = work_dir.join("voya-vpn.sudoers");
    let install_script_path = work_dir.join("install.sh");

    let launcher_contents = launcher_script(os, bin_prefix)?;
    let sudoers_contents = sudoers_drop_in(username, &launcher_path);
    let install_script_contents = install_script(
        &launcher_dir,
        &launcher_path,
        &src_launcher_path,
        &src_sudoers_path,
    );
    let command = elevated_install_command(os, &install_script_path)?;

    Ok(ElevationInstallPlan {
        work_dir: work_dir.to_path_buf(),
        src_launcher_path,
        src_sudoers_path,
        install_script_path,
        launcher_contents,
        sudoers_contents,
        install_script_contents,
        command,
    })
}

/// Passwordless `sudo -n` plan that drives the launcher's self-removing
/// `uninstall` verb (removes the sudoers drop-in and the launcher directory).
pub fn build_uninstall_spawn(os: TargetOs) -> Result<ProcessSpawn, PrivilegeError> {
    let launcher = elevate_launcher_path(os).ok_or(PrivilegeError::UnsupportedOs)?;
    Ok(ProcessSpawn::new(ProcessRole::SudoKill, SUDO_EXECUTABLE)
        .with_arguments([
            "-n".to_string(),
            "--".to_string(),
            launcher.to_string_lossy().into_owned(),
            "uninstall".to_string(),
        ])
        .with_display_log(false))
}

/// Root-owned launcher script. Dispatches `run` / `kill` / `uninstall` verbs and
/// confines `run` to absolute, non-symlink regular files under `bin_prefix`.
pub fn launcher_script(os: TargetOs, bin_prefix: &Path) -> Result<String, PrivilegeError> {
    let kill_body = unix_sudo_kill_body(os).map_err(|_| PrivilegeError::UnsupportedOs)?;
    let prefix = quote_shell_arg(&bin_prefix.to_string_lossy());
    let sudoers = quote_shell_arg(SUDOERS_DROP_IN_PATH);
    let launcher_dir = elevate_launcher_dir(os).ok_or(PrivilegeError::UnsupportedOs)?;
    let launcher_dir = quote_shell_arg(&launcher_dir.to_string_lossy());

    Ok(format!(
        r#"#!/bin/bash
# VoyaVPN privileged elevation launcher (root-owned, fixed path).
# Authorized by a NOPASSWD sudoers rule so the app can start/stop the elevated
# core without storing an admin password.
PREFIX={prefix}
SUDOERS={sudoers}
LAUNCHER_DIR={launcher_dir}

VERB="${{1:-}}"
shift 2>/dev/null || true

case "$VERB" in
  run)
    EXE="${{1:-}}"
    shift 2>/dev/null || true
    case "$EXE" in
      /*) ;;
      *) echo "voya-elevate: core path must be absolute" >&2; exit 64 ;;
    esac
    if [ -L "$EXE" ] || [ ! -f "$EXE" ]; then
      echo "voya-elevate: core path is not a regular file" >&2
      exit 64
    fi
    case "$EXE" in
      "$PREFIX"/*) ;;
      *) echo "voya-elevate: core path is outside the allowed directory" >&2; exit 64 ;;
    esac
    exec "$EXE" "$@"
    ;;
  kill)
{kill_body}
    ;;
  uninstall)
    rm -f "$SUDOERS"
    rm -rf "$LAUNCHER_DIR"
    exit 0
    ;;
  *)
    echo "voya-elevate: unknown verb '$VERB'" >&2
    exit 64
    ;;
esac
"#
    ))
}

/// Sudoers drop-in granting `username` passwordless use of exactly `launcher`.
#[must_use]
pub fn sudoers_drop_in(username: &str, launcher: &Path) -> String {
    format!(
        "{username} ALL=(root) NOPASSWD: {}\n",
        launcher.to_string_lossy()
    )
}

/// Installer (runs as root) that copies the staged launcher + sudoers into
/// place, validating the sudoers file with `visudo` before activating it.
fn install_script(
    launcher_dir: &Path,
    launcher_path: &Path,
    src_launcher_path: &Path,
    src_sudoers_path: &Path,
) -> String {
    let launcher_dir = quote_shell_arg(&launcher_dir.to_string_lossy());
    let launcher = quote_shell_arg(&launcher_path.to_string_lossy());
    let sudoers = quote_shell_arg(SUDOERS_DROP_IN_PATH);
    let src_launcher = quote_shell_arg(&src_launcher_path.to_string_lossy());
    let src_sudoers = quote_shell_arg(&src_sudoers_path.to_string_lossy());

    format!(
        r#"#!/bin/sh
set -eu
umask 022

LAUNCHER_DIR={launcher_dir}
LAUNCHER={launcher}
SUDOERS={sudoers}
SRC_LAUNCHER={src_launcher}
SRC_SUDOERS={src_sudoers}

mkdir -p "$LAUNCHER_DIR"
chmod 0755 "$LAUNCHER_DIR"
install -m 0755 "$SRC_LAUNCHER" "$LAUNCHER"
install -m 0440 "$SRC_SUDOERS" "$SUDOERS.tmp"
visudo -cf "$SUDOERS.tmp"
mv -f "$SUDOERS.tmp" "$SUDOERS"
"#
    )
}

/// Native privileged command that runs the installer as root.
fn elevated_install_command(
    os: TargetOs,
    install_script_path: &Path,
) -> Result<ProcessSpawn, PrivilegeError> {
    match os {
        TargetOs::Macos => {
            let applescript = format!(
                "do shell script \"/bin/sh \" & quoted form of \"{}\" with administrator privileges",
                applescript_escape(&install_script_path.to_string_lossy())
            );
            Ok(ProcessSpawn::new(ProcessRole::Probe, "/usr/bin/osascript")
                .with_arguments(["-e".to_string(), applescript])
                .with_display_log(false))
        }
        TargetOs::Linux => Ok(ProcessSpawn::new(ProcessRole::Probe, "/usr/bin/pkexec")
            .with_arguments([
                "/bin/sh".to_string(),
                install_script_path.to_string_lossy().into_owned(),
            ])
            .with_display_log(false)),
        TargetOs::Windows | TargetOs::Other => Err(PrivilegeError::UnsupportedOs),
    }
}

/// Escape a string for embedding inside an AppleScript double-quoted literal.
fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Classify the outcome of a native elevation command from its exit/stderr.
#[must_use]
pub fn classify_elevation_outcome(status_code: Option<i32>, stderr: &str) -> ElevationOutcome {
    if status_code == Some(0) {
        return ElevationOutcome::Granted;
    }
    // macOS osascript reports a cancelled auth dialog as error -128; pkexec
    // uses exit code 126 for "dismissed / not authorized".
    if stderr.contains("-128")
        || stderr.contains("User canceled")
        || stderr.contains("User cancelled")
        || status_code == Some(126)
    {
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
    #[error("could not determine the current user for the sudoers grant")]
    MissingUsername,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            elevate_launcher_path(TargetOs::Macos),
            Some(PathBuf::from("/usr/local/libexec/voya-vpn/voya-elevate"))
        );
        assert_eq!(
            elevate_launcher_path(TargetOs::Linux),
            Some(PathBuf::from("/usr/libexec/voya-vpn/voya-elevate"))
        );
        assert_eq!(elevate_launcher_path(TargetOs::Windows), None);
    }

    #[test]
    fn privilege_launcher_confines_run_to_bin_prefix_and_embeds_kill() {
        let script = launcher_script(
            TargetOs::Macos,
            Path::new("/Users/test/Library/Application Support/VoyaVPN/bin"),
        )
        .expect("launcher script");

        assert!(script.starts_with("#!/bin/bash"));
        assert!(
            script.contains("PREFIX='/Users/test/Library/Application Support/VoyaVPN/bin'"),
            "prefix should be shell-quoted: {script}"
        );
        assert!(script.contains("\"$PREFIX\"/*) ;;"));
        assert!(script.contains("exec \"$EXE\" \"$@\""));
        assert!(script.contains("tree_has_expected_process"));
        assert!(script.contains("rm -f \"$SUDOERS\""));
    }

    #[test]
    fn privilege_sudoers_grants_only_the_launcher() {
        let sudoers = sudoers_drop_in("afu", Path::new("/usr/local/libexec/voya-vpn/voya-elevate"));
        assert_eq!(
            sudoers,
            "afu ALL=(root) NOPASSWD: /usr/local/libexec/voya-vpn/voya-elevate\n"
        );
    }

    #[test]
    fn privilege_install_command_uses_native_dialog_per_platform() {
        let plan = build_install_plan(
            TargetOs::Macos,
            "afu",
            Path::new("/Users/afu/app/bin"),
            Path::new("/Users/afu/app/tmp/elevate"),
        )
        .expect("macos plan");
        assert_eq!(plan.command.executable, PathBuf::from("/usr/bin/osascript"));
        assert_eq!(plan.command.arguments[0], "-e");
        assert!(plan.command.arguments[1].contains("with administrator privileges"));
        assert!(plan.command.arguments[1].contains("quoted form of"));
        assert!(plan.install_script_contents.contains("visudo -cf"));
        assert!(plan
            .sudoers_contents
            .starts_with("afu ALL=(root) NOPASSWD:"));

        let linux = build_install_plan(
            TargetOs::Linux,
            "afu",
            Path::new("/home/afu/app/bin"),
            Path::new("/home/afu/app/tmp/elevate"),
        )
        .expect("linux plan");
        assert_eq!(linux.command.executable, PathBuf::from("/usr/bin/pkexec"));
        assert_eq!(linux.command.arguments[0], "/bin/sh");
    }

    #[test]
    fn privilege_install_plan_rejects_unsupported_platforms_and_blank_user() {
        assert!(matches!(
            build_install_plan(
                TargetOs::Windows,
                "afu",
                Path::new("/bin"),
                Path::new("/tmp")
            ),
            Err(PrivilegeError::UnsupportedOs)
        ));
        assert!(matches!(
            build_install_plan(TargetOs::Macos, "", Path::new("/bin"), Path::new("/tmp")),
            Err(PrivilegeError::MissingUsername)
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
            classify_elevation_outcome(Some(0), ""),
            ElevationOutcome::Granted
        );
        assert_eq!(
            classify_elevation_outcome(Some(1), "User canceled. (-128)"),
            ElevationOutcome::Cancelled
        );
        assert_eq!(
            classify_elevation_outcome(Some(126), ""),
            ElevationOutcome::Cancelled
        );
        assert_eq!(
            classify_elevation_outcome(Some(1), "visudo: invalid"),
            ElevationOutcome::Failed
        );
    }

    #[test]
    fn privilege_applescript_escapes_quotes_and_backslashes() {
        assert_eq!(applescript_escape(r#"/a/b"c\d"#), r#"/a/b\"c\\d"#);
    }
}
