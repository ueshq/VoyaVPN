//! The Windows Defender Firewall rule that lets the self-hosted node accept
//! connections from other devices.
//!
//! Windows asks the user the first time a program listens on a non-loopback
//! address, and a hidden child process of a tray app is easy to miss or to
//! decline, after which every peer times out with nothing on screen. The app
//! therefore offers to add one inbound allow rule for the sing-box binary it
//! launches. Adding it needs administrator rights, so the change runs in an
//! elevated PowerShell behind a UAC prompt; reading it back does not.
//!
//! macOS and Linux leave their firewalls to the user: the app only explains.

use std::{path::Path, sync::Arc};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use thiserror::Error;

use crate::{
    coreinfo::TargetOs,
    process::{ProcessError, ProcessRole, ProcessRunner, ProcessSpawn},
};

/// Display name of the one rule the app owns.
pub const SELF_HOST_FIREWALL_RULE_NAME: &str = "VoyaVPN self-hosted node";
const POWERSHELL: &str = "powershell.exe";
/// Exit code the status script uses for "no rule with our name".
const RULE_MISSING_EXIT_CODE: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallRuleStatus {
    /// This platform's firewall is not managed by the app.
    NotManaged,
    Missing,
    /// A rule exists but allows a different program (an older install path).
    Stale,
    Present,
}

#[derive(Debug, Error)]
pub enum FirewallError {
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("the firewall change was cancelled or failed (exit code {0:?})")]
    ElevatedCommandFailed(Option<i32>),
    #[error("reading the firewall rule failed: {0}")]
    StatusFailed(String),
    #[error("the firewall is not managed on this platform")]
    Unsupported,
}

#[derive(Clone)]
pub struct FirewallService {
    runner: Arc<dyn ProcessRunner>,
    target_os: TargetOs,
}

impl FirewallService {
    #[must_use]
    pub fn new(runner: Arc<dyn ProcessRunner>, target_os: TargetOs) -> Self {
        Self { runner, target_os }
    }

    #[must_use]
    pub fn manages_firewall(&self) -> bool {
        self.target_os == TargetOs::Windows
    }

    /// Whether the rule exists and allows `program`.
    pub fn status(&self, program: &Path) -> Result<FirewallRuleStatus, FirewallError> {
        if !self.manages_firewall() {
            return Ok(FirewallRuleStatus::NotManaged);
        }
        let output = self.runner.run_oneshot(powershell(&status_script()))?;
        match output.status_code {
            Some(0) => {
                let allowed = output
                    .stdout
                    .lines()
                    .map(str::trim)
                    .any(|line| same_program(line, program));
                Ok(if allowed {
                    FirewallRuleStatus::Present
                } else {
                    FirewallRuleStatus::Stale
                })
            }
            Some(RULE_MISSING_EXIT_CODE) => Ok(FirewallRuleStatus::Missing),
            _ => Err(FirewallError::StatusFailed(
                output.stderr.trim().to_string(),
            )),
        }
    }

    /// Replaces the rule with one that allows `program`, behind a UAC prompt.
    pub fn allow_program(&self, program: &Path) -> Result<(), FirewallError> {
        if !self.manages_firewall() {
            return Err(FirewallError::Unsupported);
        }
        self.run_elevated(&allow_script(program))
    }

    /// Removes the rule, behind a UAC prompt. Absent is fine.
    pub fn remove_rule(&self) -> Result<(), FirewallError> {
        if !self.manages_firewall() {
            return Err(FirewallError::Unsupported);
        }
        self.run_elevated(&remove_script())
    }

    fn run_elevated(&self, script: &str) -> Result<(), FirewallError> {
        let output = self
            .runner
            .run_oneshot(powershell(&elevated_launcher(script)))?;
        if output.success() {
            Ok(())
        } else {
            Err(FirewallError::ElevatedCommandFailed(output.status_code))
        }
    }
}

fn powershell(script: &str) -> ProcessSpawn {
    ProcessSpawn::new(ProcessRole::Firewall, POWERSHELL).with_arguments([
        "-NoProfile".to_string(),
        "-NonInteractive".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-EncodedCommand".to_string(),
        encode_powershell(script),
    ])
}

/// A launcher that runs `script` in an elevated PowerShell and exits with its
/// exit code. The inner script travels base64-encoded, so no path in it is
/// ever re-parsed by a second quoting layer.
pub(crate) fn elevated_launcher(script: &str) -> String {
    format!(
        "$process = Start-Process -FilePath '{POWERSHELL}' -Verb RunAs -Wait -PassThru \
         -WindowStyle Hidden -ArgumentList '-NoProfile','-NonInteractive','-ExecutionPolicy',\
         'Bypass','-EncodedCommand','{}'; exit $process.ExitCode",
        encode_powershell(script)
    )
}

pub(crate) fn status_script() -> String {
    format!(
        "$rule = Get-NetFirewallRule -DisplayName {} -ErrorAction SilentlyContinue; \
         if (-not $rule) {{ exit {RULE_MISSING_EXIT_CODE} }}; \
         $rule | Get-NetFirewallApplicationFilter | ForEach-Object {{ $_.Program }}; exit 0",
        powershell_literal(SELF_HOST_FIREWALL_RULE_NAME)
    )
}

pub(crate) fn allow_script(program: &Path) -> String {
    format!(
        "$ErrorActionPreference = 'Stop'; {} \
         New-NetFirewallRule -DisplayName {} -Direction Inbound -Action Allow \
         -Program {} -Profile Any | Out-Null; exit 0",
        remove_statement(),
        powershell_literal(SELF_HOST_FIREWALL_RULE_NAME),
        powershell_literal(&program.to_string_lossy())
    )
}

pub(crate) fn remove_script() -> String {
    format!("{} exit 0", remove_statement())
}

fn remove_statement() -> String {
    format!(
        "Get-NetFirewallRule -DisplayName {} -ErrorAction SilentlyContinue | Remove-NetFirewallRule;",
        powershell_literal(SELF_HOST_FIREWALL_RULE_NAME)
    )
}

/// A single-quoted PowerShell string: nothing inside is expanded, and a quote
/// is escaped by doubling it.
pub(crate) fn powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// `-EncodedCommand` takes base64 of the UTF-16LE script.
pub(crate) fn encode_powershell(script: &str) -> String {
    let bytes = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    STANDARD.encode(bytes)
}

fn same_program(listed: &str, program: &Path) -> bool {
    !listed.is_empty() && listed.eq_ignore_ascii_case(&program.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{process::ProcessOutput, test_support::RecordingRunner};

    fn service(target_os: TargetOs, output: ProcessOutput) -> (FirewallService, RecordingRunner) {
        let runner = RecordingRunner::default().with_oneshot_output(output);
        (
            FirewallService::new(Arc::new(runner.clone()), target_os),
            runner,
        )
    }

    fn output(status_code: i32, stdout: &str) -> ProcessOutput {
        ProcessOutput {
            status_code: Some(status_code),
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    fn decode_utf16_base64(encoded: &str) -> String {
        let bytes = STANDARD.decode(encoded).expect("base64");
        let units = bytes
            .chunks(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).expect("utf-16")
    }

    #[test]
    fn encoded_commands_round_trip_utf16() {
        for script in ["", "a", "ab", "abc", "exit 0; 'O''Brien' → ✓"] {
            assert_eq!(decode_utf16_base64(&encode_powershell(script)), script);
        }
    }

    #[test]
    fn literals_cannot_break_out_of_their_quotes() {
        assert_eq!(
            powershell_literal("C:\\a'b\\sing-box.exe"),
            "'C:\\a''b\\sing-box.exe'"
        );
        let script = allow_script(Path::new("C:\\Users\\O'Neil\\sing-box.exe"));
        assert!(script.contains("-Program 'C:\\Users\\O''Neil\\sing-box.exe'"));
        assert!(script.starts_with("$ErrorActionPreference = 'Stop';"));
        assert!(script.contains("Remove-NetFirewallRule"));
    }

    #[test]
    fn the_elevated_launcher_carries_the_script_encoded() {
        let launcher = elevated_launcher(&remove_script());
        assert!(launcher.contains("-Verb RunAs"));
        let encoded = launcher
            .split('\'')
            .max_by_key(|part| part.len())
            .expect("encoded script");
        assert_eq!(decode_utf16_base64(encoded), remove_script());
    }

    #[test]
    fn other_platforms_do_not_run_anything() {
        let (service, runner) = service(TargetOs::Macos, output(0, ""));
        assert_eq!(
            service.status(Path::new("/app/sing-box")).expect("status"),
            FirewallRuleStatus::NotManaged
        );
        assert!(matches!(
            service.allow_program(Path::new("/app/sing-box")),
            Err(FirewallError::Unsupported)
        ));
        assert!(runner.oneshots().is_empty());
    }

    #[test]
    fn status_compares_the_allowed_program() {
        let program = PathBuf::from("C:\\Users\\a\\AppData\\bin\\sing-box.exe");
        let (present, runner) = service(
            TargetOs::Windows,
            output(0, "c:\\users\\a\\appdata\\bin\\SING-BOX.EXE\r\n"),
        );
        assert_eq!(
            present.status(&program).expect("status"),
            FirewallRuleStatus::Present
        );
        let spawn = &runner.oneshots()[0];
        assert_eq!(spawn.role, ProcessRole::Firewall);
        assert!(spawn.arguments.contains(&"-EncodedCommand".to_string()));

        let (stale, _) = service(TargetOs::Windows, output(0, "C:\\old\\sing-box.exe\n"));
        assert_eq!(
            stale.status(&program).expect("status"),
            FirewallRuleStatus::Stale
        );
        let (missing, _) = service(TargetOs::Windows, output(RULE_MISSING_EXIT_CODE, ""));
        assert_eq!(
            missing.status(&program).expect("status"),
            FirewallRuleStatus::Missing
        );
        let (failed, _) = service(TargetOs::Windows, output(1, ""));
        assert!(failed.status(&program).is_err());
    }

    #[test]
    fn a_declined_prompt_is_an_error() {
        let (service, _) = service(TargetOs::Windows, output(1, ""));
        assert!(matches!(
            service.allow_program(Path::new("C:\\sing-box.exe")),
            Err(FirewallError::ElevatedCommandFailed(Some(1)))
        ));
    }
}
