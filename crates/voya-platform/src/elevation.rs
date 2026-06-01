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
        let mut guard = self
            .password
            .lock()
            .map_err(|_| SudoPasswordError::LockPoisoned)?;
        *guard = Zeroizing::new(password.into());
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
    let script_path = script_dir.as_ref().join(RUN_AS_SUDO_SCRIPT_FILE_NAME);
    let script_body = unix_sudo_run_script(&base);

    ProcessSpawn {
        role: base.role,
        executable: script_path.clone(),
        arguments: Vec::new(),
        working_dir: base.working_dir,
        environment: base.environment,
        display_log: base.display_log,
        stdin: Some(ProcessStdin::new(password)),
        generated_scripts: vec![GeneratedScript {
            path: script_path,
            contents: script_body,
            executable: true,
        }],
    }
}

pub fn unix_sudo_kill_spawn(
    os: TargetOs,
    script_dir: impl AsRef<Path>,
    target_pid: u32,
    working_dir: impl Into<PathBuf>,
    password: Zeroizing<String>,
) -> Result<ProcessSpawn, ElevationError> {
    let script_file_name =
        unix_sudo_kill_script_file_name(os).ok_or(ElevationError::UnsupportedOs)?;
    let script_path = script_dir.as_ref().join(script_file_name);
    let command = format!(
        "sudo -S {} {}",
        quote_shell_arg(script_path.to_string_lossy().as_ref()),
        target_pid
    );

    Ok(ProcessSpawn::new(ProcessRole::SudoKill, "/bin/bash")
        .with_arguments(["-c".to_string(), command])
        .with_working_dir(working_dir)
        .with_display_log(true)
        .with_stdin(ProcessStdin::new(password))
        .with_generated_script(GeneratedScript {
            path: script_path,
            contents: unix_sudo_kill_script(os)?,
            executable: true,
        }))
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

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <PID>"
  exit 1
fi

PID="$1"
if ! kill -0 "$PID" 2>/dev/null; then
  exit 0
fi

kill_children() {{
  local parent="$1"
  local children
  children=$({child_lookup})
  for child in $children; do
    kill_children "$child"
    kill -9 "$child" 2>/dev/null || true
  done
}}

kill -15 "$PID" 2>/dev/null || true
sleep 1
if kill -0 "$PID" 2>/dev/null; then
  kill_children "$PID"
  kill -9 "$PID" 2>/dev/null || true
fi
"#
    ))
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
            "/tmp",
            Zeroizing::new("pw".to_string()),
        )
        .expect("linux kill plan");
        let macos = unix_sudo_kill_spawn(
            TargetOs::Macos,
            "/tmp/voya scripts",
            42,
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
        assert_ne!(
            linux.generated_scripts[0].path.file_name(),
            macos.generated_scripts[0].path.file_name()
        );
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
