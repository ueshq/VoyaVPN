//! The Linux installer for the one-time elevation grant: a `pkexec` prompt
//! installs a fixed-path, root-owned launcher plus a `NOPASSWD` sudoers drop-in
//! that authorizes only that launcher.
//!
//! It is compiled for Linux only (and for tests). macOS runs the tunnel in its
//! NetworkExtension provider and never escalates privileges, so no macOS
//! binary carries an installer, a sudoers rule or an admin prompt (App Review
//! Guideline 2.4.5; ADR 0004).

use std::path::Path;

use super::{elevate_launcher_dir, ElevationInstallPlan, PrivilegeError, LAUNCHER_FILE_NAME};
use crate::{
    coreinfo::TargetOs,
    elevation::{quote_shell_arg, unix_sudo_kill_body},
    process::{ProcessRole, ProcessSpawn},
};

/// Fixed sudoers drop-in path. The name has no `.` so sudo loads it (sudo
/// ignores files in `sudoers.d` whose name contains a dot).
const SUDOERS_DROP_IN_PATH: &str = "/etc/sudoers.d/voya-vpn";

/// See [`super::build_install_plan`].
pub(super) fn build_install_plan(
    username: &str,
    bin_prefix: &Path,
    work_dir: &Path,
) -> Result<ElevationInstallPlan, PrivilegeError> {
    let launcher_dir =
        elevate_launcher_dir(TargetOs::Linux).ok_or(PrivilegeError::UnsupportedOs)?;
    let launcher_path = launcher_dir.join(LAUNCHER_FILE_NAME);
    if username.is_empty() {
        return Err(PrivilegeError::MissingUsername);
    }

    let src_launcher_path = work_dir.join(LAUNCHER_FILE_NAME);
    let src_sudoers_path = work_dir.join("voya-vpn.sudoers");
    let install_script_path = work_dir.join("install.sh");

    let launcher_contents = launcher_script(bin_prefix)?;
    let sudoers_contents = sudoers_drop_in(username, &launcher_path);
    let install_script_contents = install_script(
        &launcher_dir,
        &launcher_path,
        &src_launcher_path,
        &src_sudoers_path,
    );
    let command = ProcessSpawn::new(ProcessRole::Probe, "/usr/bin/pkexec")
        .with_arguments([
            "/bin/sh".to_string(),
            install_script_path.to_string_lossy().into_owned(),
        ])
        .with_display_log(false);

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

/// Root-owned launcher script. Dispatches `run` / `kill` / `uninstall` verbs.
///
/// The `run` verb requires an absolute path with no `..` component, resolves the
/// containing directory with `cd -P`/`pwd -P` (so symlinked directories cannot
/// escape), and only then requires the resolved path to be a non-symlink regular
/// file inside the resolved `bin_prefix`. The resolved path is what gets exec'd.
pub(super) fn launcher_script(bin_prefix: &Path) -> Result<String, PrivilegeError> {
    let kill_body =
        unix_sudo_kill_body(TargetOs::Linux).map_err(|_| PrivilegeError::UnsupportedOs)?;
    let prefix = quote_shell_arg(&bin_prefix.to_string_lossy());
    let sudoers = quote_shell_arg(SUDOERS_DROP_IN_PATH);
    let launcher_dir =
        elevate_launcher_dir(TargetOs::Linux).ok_or(PrivilegeError::UnsupportedOs)?;
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
    case "/$EXE/" in
      */../*) echo "voya-elevate: core path must not contain '..'" >&2; exit 64 ;;
    esac
    EXE_DIR="${{EXE%/*}}"
    [ -n "$EXE_DIR" ] || EXE_DIR="/"
    EXE_NAME="${{EXE##*/}}"
    REAL_DIR=$(cd -P -- "$EXE_DIR" 2>/dev/null && pwd -P) || {{
      echo "voya-elevate: core directory could not be resolved" >&2
      exit 64
    }}
    REAL_PREFIX=$(cd -P -- "$PREFIX" 2>/dev/null && pwd -P) || {{
      echo "voya-elevate: allowed directory could not be resolved" >&2
      exit 64
    }}
    EXE="$REAL_DIR/$EXE_NAME"
    if [ -L "$EXE" ] || [ ! -f "$EXE" ]; then
      echo "voya-elevate: core path is not a regular file" >&2
      exit 64
    fi
    case "$EXE" in
      "$REAL_PREFIX"/*) ;;
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
pub(super) fn sudoers_drop_in(username: &str, launcher: &Path) -> String {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn privilege_launcher_confines_run_to_bin_prefix_and_embeds_kill() {
        let script = launcher_script(Path::new("/home/test/.local/share/Voya VPN/bin"))
            .expect("launcher script");

        assert!(script.starts_with("#!/bin/bash"));
        assert!(
            script.contains("PREFIX='/home/test/.local/share/Voya VPN/bin'"),
            "prefix should be shell-quoted: {script}"
        );
        assert!(script.contains("LAUNCHER_DIR=/usr/libexec/voya-vpn"));
        assert!(script.contains("\"$REAL_PREFIX\"/*) ;;"));
        assert!(script.contains("exec \"$EXE\" \"$@\""));
        assert!(script.contains("target_has_expected_process"));
        assert!(script.contains("rm -f \"$SUDOERS\""));
    }

    #[cfg(unix)]
    #[test]
    fn privilege_launcher_run_rejects_traversal_and_symlinked_directory_escapes() {
        use std::fs;

        let root = unique_temp_root("privilege-launcher-run");
        let prefix = root.join("app").join("bin");
        let outside = root.join("outside");
        fs::create_dir_all(&prefix).expect("create prefix");
        fs::create_dir_all(&outside).expect("create outside directory");

        let allowed = prefix.join("sing-box");
        write_executable(&allowed, "#!/bin/sh\necho voya-allowed\n");
        let forbidden = outside.join("evil");
        write_executable(&forbidden, "#!/bin/sh\necho voya-escaped\n");
        std::os::unix::fs::symlink(&outside, prefix.join("outside-link")).expect("symlink");

        let launcher = root.join(LAUNCHER_FILE_NAME);
        let script = launcher_script(&prefix).expect("launcher script");
        fs::write(&launcher, script).expect("write launcher");

        let allowed_run = run_launcher(&launcher, &allowed);
        assert!(
            allowed_run.status.success(),
            "a genuine core under the prefix must still run: {}",
            String::from_utf8_lossy(&allowed_run.stderr)
        );
        assert!(String::from_utf8_lossy(&allowed_run.stdout).contains("voya-allowed"));

        for escape in [
            prefix.join("..").join("..").join("outside").join("evil"),
            prefix.join("outside-link").join("evil"),
            forbidden.clone(),
        ] {
            let output = run_launcher(&launcher, &escape);
            assert_eq!(
                output.status.code(),
                Some(64),
                "{} must be rejected, stdout: {} stderr: {}",
                escape.display(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!String::from_utf8_lossy(&output.stdout).contains("voya-escaped"));
        }

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) {
        use std::{fs, os::unix::fs::PermissionsExt};

        fs::write(path, contents).expect("write script");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod script");
    }

    #[cfg(unix)]
    fn run_launcher(launcher: &Path, executable: &Path) -> std::process::Output {
        std::process::Command::new("bash")
            .arg(launcher)
            .arg("run")
            .arg(executable)
            .output()
            .expect("run launcher")
    }

    #[cfg(unix)]
    fn unique_temp_root(name: &str) -> PathBuf {
        tempfile::Builder::new()
            .prefix(&format!("voyavpn-{name}-"))
            .tempdir()
            .expect("privilege test temp dir")
            .keep()
    }

    #[test]
    fn privilege_sudoers_grants_only_the_launcher() {
        let sudoers = sudoers_drop_in("afu", Path::new("/usr/libexec/voya-vpn/voya-elevate"));
        assert_eq!(
            sudoers,
            "afu ALL=(root) NOPASSWD: /usr/libexec/voya-vpn/voya-elevate\n"
        );
    }

    #[test]
    fn privilege_install_command_uses_pkexec() {
        let plan = build_install_plan(
            "afu",
            Path::new("/home/afu/app/bin"),
            Path::new("/home/afu/app/tmp/elevate"),
        )
        .expect("linux plan");

        assert_eq!(plan.command.executable, PathBuf::from("/usr/bin/pkexec"));
        assert_eq!(
            plan.command.arguments,
            ["/bin/sh", "/home/afu/app/tmp/elevate/install.sh"]
        );
        assert!(plan.install_script_contents.contains("visudo -cf"));
        assert!(plan
            .sudoers_contents
            .starts_with("afu ALL=(root) NOPASSWD: /usr/libexec/voya-vpn/voya-elevate"));
    }

    #[test]
    fn privilege_install_plan_rejects_a_blank_user() {
        assert!(matches!(
            build_install_plan("", Path::new("/bin"), Path::new("/tmp")),
            Err(PrivilegeError::MissingUsername)
        ));
    }
}
