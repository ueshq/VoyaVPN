use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use thiserror::Error;
use voya_core::CoreType;
use zeroize::Zeroizing;

use crate::{
    coreinfo::TargetOs,
    process::{GeneratedScript, ProcessRole, ProcessSpawn, ProcessStdin},
};

pub const RUN_AS_SUDO_SCRIPT_FILE_NAME: &str = "run_as_sudo.sh";
pub const KILL_AS_SUDO_LINUX_SCRIPT_FILE_NAME: &str = "kill_as_sudo_linux.sh";
pub const KILL_AS_SUDO_MACOS_SCRIPT_FILE_NAME: &str = "kill_as_sudo_osx.sh";

#[derive(Debug, Default)]
pub struct SudoPasswordStore {
    password: Mutex<Zeroizing<String>>,
}

impl SudoPasswordStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_password(&self, password: impl Into<String>) -> Result<(), SudoPasswordError> {
        self.set_password_secret(Zeroizing::new(password.into()))
    }

    pub fn set_password_secret(
        &self,
        password: Zeroizing<String>,
    ) -> Result<(), SudoPasswordError> {
        let mut guard = self
            .password
            .lock()
            .map_err(|_| SudoPasswordError::LockPoisoned)?;
        *guard = password;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), SudoPasswordError> {
        let mut guard = self
            .password
            .lock()
            .map_err(|_| SudoPasswordError::LockPoisoned)?;
        *guard = Zeroizing::new(String::new());
        Ok(())
    }

    pub fn has_password(&self) -> Result<bool, SudoPasswordError> {
        self.password
            .lock()
            .map(|guard| !guard.is_empty())
            .map_err(|_| SudoPasswordError::LockPoisoned)
    }

    pub fn read_password(&self) -> Result<Option<Zeroizing<String>>, SudoPasswordError> {
        let password = self
            .password
            .lock()
            .map_err(|_| SudoPasswordError::LockPoisoned)?
            .clone();
        if password.is_empty() {
            Ok(None)
        } else {
            Ok(Some(password))
        }
    }
}

#[derive(Debug, Error)]
pub enum SudoPasswordError {
    #[error("sudo password lock is poisoned")]
    LockPoisoned,
}

#[must_use]
pub const fn should_use_unix_sudo(
    os: TargetOs,
    core_type: CoreType,
    tun_enabled: bool,
    may_need_sudo: bool,
) -> bool {
    may_need_sudo
        && tun_enabled
        && matches!(core_type, CoreType::sing_box | CoreType::mihomo)
        && matches!(os, TargetOs::Linux | TargetOs::Macos)
}

pub fn wrap_spawn_with_unix_sudo(
    base: ProcessSpawn,
    script_dir: impl AsRef<Path>,
    password: Zeroizing<String>,
) -> ProcessSpawn {
    let script_dir = script_dir.as_ref();
    let script_path = script_dir.join(RUN_AS_SUDO_SCRIPT_FILE_NAME);
    let script_body = unix_sudo_run_script(&base);

    ProcessSpawn {
        role: base.role,
        executable: script_path.clone(),
        arguments: Vec::new(),
        working_dir: base.working_dir,
        environment: base.environment,
        display_log: base.display_log,
        stdin: Some(ProcessStdin::new(password)),
        generated_scripts: vec![GeneratedScript::new(
            script_dir.to_path_buf(),
            script_path,
            script_body,
            true,
        )],
    }
}

pub fn unix_sudo_kill_spawn(
    os: TargetOs,
    script_dir: impl AsRef<Path>,
    target_pid: u32,
    expected_executable: impl AsRef<Path>,
    working_dir: impl Into<PathBuf>,
    password: Zeroizing<String>,
) -> Result<ProcessSpawn, ElevationError> {
    let script_file_name =
        unix_sudo_kill_script_file_name(os).ok_or(ElevationError::UnsupportedOs)?;
    let script_dir = script_dir.as_ref();
    let script_path = script_dir.join(script_file_name);
    let expected_names = expected_process_comm_names(expected_executable.as_ref())?;
    let mut command = format!(
        "sudo -S {} {target_pid}",
        quote_shell_arg(script_path.to_string_lossy().as_ref()),
    );
    for expected_name in expected_names {
        command.push(' ');
        command.push_str(&quote_shell_arg(&expected_name));
    }

    Ok(ProcessSpawn::new(ProcessRole::SudoKill, "/bin/bash")
        .with_arguments(["-c".to_string(), command])
        .with_working_dir(working_dir)
        .with_display_log(true)
        .with_stdin(ProcessStdin::new(password))
        .with_generated_script(GeneratedScript::new(
            script_dir.to_path_buf(),
            script_path,
            unix_sudo_kill_script(os)?,
            true,
        )))
}

#[must_use]
pub const fn unix_sudo_kill_script_file_name(os: TargetOs) -> Option<&'static str> {
    match os {
        TargetOs::Linux => Some(KILL_AS_SUDO_LINUX_SCRIPT_FILE_NAME),
        TargetOs::Macos => Some(KILL_AS_SUDO_MACOS_SCRIPT_FILE_NAME),
        TargetOs::Windows | TargetOs::Other => None,
    }
}

fn unix_sudo_run_script(base: &ProcessSpawn) -> String {
    let mut command = quote_shell_arg(base.executable.to_string_lossy().as_ref());
    for argument in &base.arguments {
        command.push(' ');
        command.push_str(&quote_shell_arg(argument));
    }

    format!("#!/bin/bash\nexec sudo -S -- {command}\n")
}

fn unix_sudo_kill_script(os: TargetOs) -> Result<String, ElevationError> {
    let child_lookup = match os {
        TargetOs::Linux => "ps -o pid= --ppid \"$parent\"",
        TargetOs::Macos => "ps -axo pid=,ppid= | awk -v ppid=\"$parent\" '$2==ppid {print $1}'",
        TargetOs::Windows | TargetOs::Other => return Err(ElevationError::UnsupportedOs),
    };

    Ok(format!(
        r#"#!/bin/bash
set +e

if [ "$#" -lt 2 ]; then
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

fn expected_process_comm_names(executable: &Path) -> Result<Vec<String>, ElevationError> {
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
    fn process_linux_and_macos_sudo_kill_share_sudo_s_except_script_name() {
        let linux = unix_sudo_kill_spawn(
            TargetOs::Linux,
            "/tmp/voya scripts",
            42,
            "/tmp/voya cores/sing-box",
            "/tmp",
            Zeroizing::new("pw".to_string()),
        )
        .expect("linux kill plan");
        let macos = unix_sudo_kill_spawn(
            TargetOs::Macos,
            "/tmp/voya scripts",
            42,
            "/tmp/voya cores/sing-box",
            "/tmp",
            Zeroizing::new("pw".to_string()),
        )
        .expect("macos kill plan");

        assert_eq!(linux.executable, PathBuf::from("/bin/bash"));
        assert_eq!(macos.executable, PathBuf::from("/bin/bash"));
        assert_eq!(linux.arguments[0], "-c");
        assert_eq!(macos.arguments[0], "-c");
        assert!(linux.arguments[1].starts_with("sudo -S "));
        assert!(macos.arguments[1].starts_with("sudo -S "));
        assert!(linux.arguments[1].ends_with(" 42 sing-box"));
        assert!(macos.arguments[1].ends_with(" 42 sing-box"));
        assert_ne!(
            linux.generated_scripts[0].path.file_name(),
            macos.generated_scripts[0].path.file_name()
        );
    }

    #[test]
    fn process_sudo_kill_uses_expected_comm_aliases_for_pid_validation() {
        let spawn = unix_sudo_kill_spawn(
            TargetOs::Linux,
            "/tmp/voya/scripts",
            42,
            "/tmp/voya cores/mihomo-linux-amd64-v1",
            "/tmp",
            Zeroizing::new("pw".to_string()),
        )
        .expect("linux kill plan");

        assert!(spawn.arguments[1].ends_with(" 42 mihomo-linux-amd64-v1 mihomo-linux-am"));
        let script = &spawn.generated_scripts[0].contents;
        assert!(script.contains("tree_has_expected_process"));
        assert!(script.contains("refusing to sudo kill pid $PID"));
        assert!(script.contains("sudo kill target pid $PID is still running"));
    }

    #[test]
    fn process_sudo_kill_rejects_target_without_comparable_process_name() {
        let error = unix_sudo_kill_spawn(
            TargetOs::Linux,
            "/tmp/voya/scripts",
            42,
            "/",
            "/tmp",
            Zeroizing::new("pw".to_string()),
        )
        .expect_err("missing process name");

        assert!(matches!(error, ElevationError::InvalidKillTarget { .. }));
    }

    #[test]
    fn process_unix_sudo_wrap_reads_password_into_spawn_stdin() {
        let base = ProcessSpawn::new(ProcessRole::Main, "/tmp/Voya VPN/sing-box")
            .with_arguments(split_command_line("run -c config.json").expect("args"))
            .with_working_dir("/tmp/Voya VPN/binConfigs");
        let wrapped = wrap_spawn_with_unix_sudo(
            base,
            "/tmp/Voya VPN/scripts",
            Zeroizing::new("pw".to_string()),
        );

        assert_eq!(
            wrapped.executable,
            PathBuf::from("/tmp/Voya VPN/scripts/run_as_sudo.sh")
        );
        assert!(wrapped.has_stdin());
        assert!(wrapped.generated_scripts[0]
            .contents
            .contains("exec sudo -S -- '/tmp/Voya VPN/sing-box' run -c config.json"));
    }

    #[test]
    fn process_sudo_password_store_reports_empty_until_collected() {
        let store = SudoPasswordStore::new();
        assert!(!store.has_password().expect("has password"));

        store.set_password("pw").expect("set password");
        assert!(store.has_password().expect("has password"));
        let password = store
            .read_password()
            .expect("read password")
            .map(|password| password.as_str().to_string());
        assert_eq!(password.as_deref(), Some("pw"));

        store.clear().expect("clear password");
        assert!(!store.has_password().expect("has password"));
    }
}
