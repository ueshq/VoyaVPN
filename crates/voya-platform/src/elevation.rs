use std::path::{Path, PathBuf};

use thiserror::Error;
use voya_core::CoreType;

use crate::{
    coreinfo::TargetOs,
    process::{ProcessRole, ProcessSpawn},
};

/// Absolute path to the system `sudo` binary used for passwordless elevation.
///
/// A fixed path (instead of a `PATH` lookup) keeps the privileged command
/// deterministic regardless of the environment the GUI was launched with.
pub const SUDO_EXECUTABLE: &str = "/usr/bin/sudo";

#[must_use]
pub const fn should_use_unix_sudo(
    os: TargetOs,
    core_type: CoreType,
    tun_enabled: bool,
    may_need_sudo: bool,
) -> bool {
    let _ = core_type;
    may_need_sudo && tun_enabled && matches!(os, TargetOs::Linux | TargetOs::Macos)
}

/// Wrap a core launch so it runs through the root-owned elevation launcher via
/// passwordless `sudo -n`.
///
/// The app never stores or pipes an admin password; a one-time native
/// authorization grant installs a `NOPASSWD` sudoers rule for `launcher`, so
/// this spawn succeeds without any stdin secret.
#[must_use]
pub fn wrap_spawn_with_unix_sudo_passwordless(base: ProcessSpawn, launcher: &Path) -> ProcessSpawn {
    let mut arguments = vec![
        "-n".to_string(),
        "--".to_string(),
        launcher.to_string_lossy().into_owned(),
        "run".to_string(),
        base.executable.to_string_lossy().into_owned(),
    ];
    arguments.extend(base.arguments);

    ProcessSpawn {
        role: base.role,
        executable: PathBuf::from(SUDO_EXECUTABLE),
        arguments,
        working_dir: base.working_dir,
        environment: base.environment,
        display_log: base.display_log,
        stdin: None,
        generated_scripts: Vec::new(),
    }
}

/// Build the passwordless `sudo -n` kill plan that drives the launcher's `kill`
/// verb against an elevated core process tree.
pub fn unix_sudo_kill_spawn_passwordless(
    os: TargetOs,
    launcher: &Path,
    target_pid: u32,
    expected_executable: impl AsRef<Path>,
    working_dir: impl Into<PathBuf>,
) -> Result<ProcessSpawn, ElevationError> {
    if !matches!(os, TargetOs::Linux | TargetOs::Macos) {
        return Err(ElevationError::UnsupportedOs);
    }

    let expected_names = expected_process_comm_names(expected_executable.as_ref())?;
    let mut arguments = vec![
        "-n".to_string(),
        "--".to_string(),
        launcher.to_string_lossy().into_owned(),
        "kill".to_string(),
        target_pid.to_string(),
    ];
    arguments.extend(expected_names);

    Ok(ProcessSpawn::new(ProcessRole::SudoKill, SUDO_EXECUTABLE)
        .with_arguments(arguments)
        .with_working_dir(working_dir)
        .with_display_log(true))
}

/// Bash body (no shebang) that terminates an elevated core process tree.
///
/// It expects positional arguments `<PID> <expected process name>...` and is
/// embedded into the root-owned launcher's `kill` verb so the privileged
/// teardown logic itself lives in a root-only-writable file.
pub fn unix_sudo_kill_body(os: TargetOs) -> Result<String, ElevationError> {
    let child_lookup = match os {
        TargetOs::Linux => "ps -o pid= --ppid \"$parent\"",
        TargetOs::Macos => "ps -axo pid=,ppid= | awk -v ppid=\"$parent\" '$2==ppid {print $1}'",
        TargetOs::Windows | TargetOs::Other => return Err(ElevationError::UnsupportedOs),
    };

    Ok(format!(
        r#"if [ "$#" -lt 2 ]; then
  echo "Usage: $0 <PID> <expected process name>..." >&2
  exit 64
fi

PID="$1"
shift
if ! kill -0 "$PID" 2>/dev/null; then
  exit 0
fi

child_pids() {{
  local parent="$1"
  {child_lookup}
}}

process_comm() {{
  local process_id="$1"
  if [ -r "/proc/$process_id/comm" ]; then
    head -n 1 "/proc/$process_id/comm" 2>/dev/null
    return
  fi
  ps -p "$process_id" -o comm= 2>/dev/null | awk 'NR==1 {{print $1}}'
}}

process_matches_expected() {{
  local process_id="$1"
  shift
  local comm
  local base
  local expected
  comm=$(process_comm "$process_id")
  if [ -z "$comm" ]; then
    return 1
  fi
  base="${{comm##*/}}"
  for expected in "$@"; do
    if [ "$comm" = "$expected" ] || [ "$base" = "$expected" ]; then
      return 0
    fi
  done
  return 1
}}

tree_has_expected_process() {{
  local parent="$1"
  shift
  local child
  if process_matches_expected "$parent" "$@"; then
    return 0
  fi
  for child in $(child_pids "$parent"); do
    if tree_has_expected_process "$child" "$@"; then
      return 0
    fi
  done
  return 1
}}

descendant_pids() {{
  local parent="$1"
  local child
  for child in $(child_pids "$parent"); do
    echo "$child"
    descendant_pids "$child"
  done
}}

kill_children() {{
  local parent="$1"
  local child
  for child in $(child_pids "$parent"); do
    kill_children "$child"
    kill -9 "$child" 2>/dev/null || true
  done
}}

wait_for_exit() {{
  local process_id="$1"
  local attempts=20
  while [ "$attempts" -gt 0 ]; do
    if ! kill -0 "$process_id" 2>/dev/null; then
      return 0
    fi
    sleep 0.1
    attempts=$((attempts - 1))
  done
  return 1
}}

if ! tree_has_expected_process "$PID" "$@"; then
  echo "refusing to sudo kill pid $PID: target process tree does not contain an expected core" >&2
  exit 65
fi

DESCENDANTS=$(descendant_pids "$PID")
kill -15 "$PID" 2>/dev/null || true
sleep 1
if kill -0 "$PID" 2>/dev/null; then
  kill_children "$PID"
  kill -9 "$PID" 2>/dev/null || true
fi

FAILED=0
if ! wait_for_exit "$PID"; then
  echo "sudo kill target pid $PID is still running" >&2
  FAILED=1
fi
for child in $DESCENDANTS; do
  if ! wait_for_exit "$child"; then
    echo "sudo kill child pid $child is still running" >&2
    FAILED=1
  fi
done
exit "$FAILED"
"#
    ))
}

const LINUX_COMM_MAX_VISIBLE_CHARS: usize = 15;

pub(crate) fn expected_process_comm_names(
    executable: &Path,
) -> Result<Vec<String>, ElevationError> {
    let file_name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ElevationError::InvalidKillTarget {
            executable: executable.to_path_buf(),
        })?;

    let mut names = vec![file_name.to_string()];
    let truncated: String = file_name
        .chars()
        .take(LINUX_COMM_MAX_VISIBLE_CHARS)
        .collect();
    if truncated != file_name {
        names.push(truncated);
    }
    Ok(names)
}

#[must_use]
pub fn quote_shell_arg(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    if value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '/' | ':')
    }) {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

#[derive(Debug, Error)]
pub enum ElevationError {
    #[error("unix sudo is not supported for this target OS")]
    UnsupportedOs,
    #[error("sudo kill target executable has no comparable process name: {executable}")]
    InvalidKillTarget { executable: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{split_command_line, ProcessRole};

    #[test]
    fn process_unix_sudo_passwordless_wrap_routes_core_through_launcher() {
        let base = ProcessSpawn::new(ProcessRole::Main, "/tmp/Voya VPN/sing-box")
            .with_arguments(split_command_line("run -c config.json").expect("args"))
            .with_working_dir("/tmp/Voya VPN/binConfigs");
        let launcher = PathBuf::from("/usr/local/libexec/voya-vpn/voya-elevate");
        let wrapped = wrap_spawn_with_unix_sudo_passwordless(base, &launcher);

        assert_eq!(wrapped.executable, PathBuf::from(SUDO_EXECUTABLE));
        assert!(!wrapped.has_stdin());
        assert!(wrapped.generated_scripts.is_empty());
        assert_eq!(
            wrapped.arguments,
            vec![
                "-n".to_string(),
                "--".to_string(),
                "/usr/local/libexec/voya-vpn/voya-elevate".to_string(),
                "run".to_string(),
                "/tmp/Voya VPN/sing-box".to_string(),
                "run".to_string(),
                "-c".to_string(),
                "config.json".to_string(),
            ]
        );
    }

    #[test]
    fn process_unix_sudo_passwordless_kill_targets_launcher_kill_verb() {
        let launcher = PathBuf::from("/usr/libexec/voya-vpn/voya-elevate");
        let spawn = unix_sudo_kill_spawn_passwordless(
            TargetOs::Linux,
            &launcher,
            42,
            "/tmp/voya cores/sing-box",
            "/tmp",
        )
        .expect("linux kill plan");

        assert_eq!(spawn.executable, PathBuf::from(SUDO_EXECUTABLE));
        assert!(!spawn.has_stdin());
        assert_eq!(
            spawn.arguments,
            vec![
                "-n".to_string(),
                "--".to_string(),
                "/usr/libexec/voya-vpn/voya-elevate".to_string(),
                "kill".to_string(),
                "42".to_string(),
                "sing-box".to_string(),
            ]
        );
    }

    #[test]
    fn process_unix_sudo_passwordless_kill_rejects_target_without_comparable_process_name() {
        let launcher = PathBuf::from("/usr/libexec/voya-vpn/voya-elevate");
        let error = unix_sudo_kill_spawn_passwordless(TargetOs::Linux, &launcher, 42, "/", "/tmp")
            .expect_err("missing process name");

        assert!(matches!(error, ElevationError::InvalidKillTarget { .. }));
    }

    #[test]
    fn process_unix_sudo_kill_body_validates_tree_before_killing() {
        let linux = unix_sudo_kill_body(TargetOs::Linux).expect("linux body");
        let macos = unix_sudo_kill_body(TargetOs::Macos).expect("macos body");

        assert!(!linux.starts_with("#!"));
        assert!(linux.contains("tree_has_expected_process"));
        assert!(linux.contains("refusing to sudo kill pid $PID"));
        assert!(linux.contains("ps -o pid= --ppid"));
        assert!(macos.contains("ps -axo pid=,ppid="));
        assert!(unix_sudo_kill_body(TargetOs::Windows).is_err());
    }

    #[test]
    fn process_quote_shell_arg_escapes_single_quotes_and_spaces() {
        assert_eq!(quote_shell_arg(""), "''");
        assert_eq!(quote_shell_arg("/usr/local/bin"), "/usr/local/bin");
        assert_eq!(quote_shell_arg("a b"), "'a b'");
        assert_eq!(quote_shell_arg("it's"), "'it'\\''s'");
    }
}
