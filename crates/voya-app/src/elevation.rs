//! Session elevation orchestration.
//!
//! Replaces the previous "collect and store a sudo password" flow with a
//! one-time native authorization that installs a root-owned launcher + a
//! `NOPASSWD` sudoers drop-in. No admin password is ever held by the app; the
//! shared [`ElevationState`] flag is what the supervisor and TUN status read.

use std::{path::PathBuf, sync::Arc};

use thiserror::Error;
use voya_platform::{
    coreinfo::TargetOs,
    privilege::{
        self, build_install_plan, build_uninstall_spawn, classify_elevation_outcome,
        current_username, elevate_launcher_path, ElevationOutcome, ElevationState, PrivilegeError,
    },
    process::{ProcessError, ProcessRunner},
};

const ELEVATE_WORK_DIR_NAME: &str = "elevate";

/// Drives one-time native elevation and exposes the shared grant flag.
#[derive(Clone)]
pub struct ElevationManager {
    state: Arc<ElevationState>,
    runner: Arc<dyn ProcessRunner>,
    target_os: TargetOs,
    temp_dir: PathBuf,
    bin_prefix: PathBuf,
}

impl ElevationManager {
    #[must_use]
    pub fn new(
        runner: Arc<dyn ProcessRunner>,
        temp_dir: impl Into<PathBuf>,
        bin_prefix: impl Into<PathBuf>,
    ) -> Self {
        Self::with_target_os(runner, temp_dir, bin_prefix, TargetOs::current())
    }

    #[must_use]
    pub fn with_target_os(
        runner: Arc<dyn ProcessRunner>,
        temp_dir: impl Into<PathBuf>,
        bin_prefix: impl Into<PathBuf>,
        target_os: TargetOs,
    ) -> Self {
        Self {
            state: Arc::new(ElevationState::new()),
            runner,
            target_os,
            temp_dir: temp_dir.into(),
            bin_prefix: bin_prefix.into(),
        }
    }

    /// Shared grant flag wired into the supervisor and TUN status reporting.
    #[must_use]
    pub fn state(&self) -> Arc<ElevationState> {
        Arc::clone(&self.state)
    }

    #[must_use]
    pub fn is_granted(&self) -> bool {
        self.state.is_granted()
    }

    /// Trigger the native authorization dialog (once) and install the launcher.
    ///
    /// Idempotent while already granted. Blocks until the user responds to the
    /// system prompt.
    pub fn request(&self) -> Result<(), ElevationError> {
        if self.state.is_granted() {
            return Ok(());
        }

        let username = current_username().ok_or(ElevationError::MissingUsername)?;
        let work_dir = self.temp_dir.join(ELEVATE_WORK_DIR_NAME);
        let plan = build_install_plan(self.target_os, &username, &self.bin_prefix, &work_dir)?;

        self.stage_install_sources(&plan)?;
        let output = self.runner.run_oneshot(plan.command.clone())?;
        let _ = std::fs::remove_dir_all(&work_dir);

        match classify_elevation_outcome(output.status_code, &output.stderr) {
            ElevationOutcome::Granted => {
                self.state.set_granted(true);
                Ok(())
            }
            ElevationOutcome::Cancelled => Err(ElevationError::Cancelled),
            ElevationOutcome::Failed => Err(ElevationError::Failed {
                status_code: output.status_code,
                message: install_failure_message(&output.stderr),
            }),
        }
    }

    /// Remove the launcher + sudoers drop-in (passwordless) and clear the grant.
    ///
    /// Best-effort: failures are logged, never returned, so app exit is never
    /// blocked.
    pub fn revoke(&self) {
        let launcher_present =
            elevate_launcher_path(self.target_os).is_some_and(|launcher| launcher.exists());
        if !launcher_present && !self.state.is_granted() {
            return;
        }

        match build_uninstall_spawn(self.target_os) {
            Ok(spawn) => {
                if let Err(error) = self.runner.run_oneshot(spawn) {
                    tracing::warn!(?error, "failed to revoke TUN elevation on exit");
                }
            }
            Err(error) => {
                tracing::warn!(?error, "unable to build TUN elevation revoke command");
            }
        }
        self.state.set_granted(false);
    }

    fn stage_install_sources(
        &self,
        plan: &privilege::ElevationInstallPlan,
    ) -> Result<(), ElevationError> {
        std::fs::create_dir_all(&plan.work_dir)?;
        restrict_dir_permissions(&plan.work_dir)?;
        write_private_file(&plan.src_launcher_path, &plan.launcher_contents)?;
        write_private_file(&plan.src_sudoers_path, &plan.sudoers_contents)?;
        write_private_file(&plan.install_script_path, &plan.install_script_contents)?;
        Ok(())
    }
}

fn write_private_file(path: &std::path::Path, contents: &str) -> Result<(), ElevationError> {
    std::fs::write(path, contents)?;
    set_private_file_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
fn restrict_dir_permissions(path: &std::path::Path) -> Result<(), ElevationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_dir_permissions(_path: &std::path::Path) -> Result<(), ElevationError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &std::path::Path) -> Result<(), ElevationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &std::path::Path) -> Result<(), ElevationError> {
    Ok(())
}

fn install_failure_message(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        "native authorization failed".to_string()
    } else {
        trimmed.lines().last().unwrap_or(trimmed).to_string()
    }
}

#[derive(Debug, Error)]
pub enum ElevationError {
    #[error("could not determine the current user for elevation")]
    MissingUsername,
    #[error("native authorization was cancelled")]
    Cancelled,
    #[error("native authorization failed ({status_code:?}): {message}")]
    Failed {
        status_code: Option<i32>,
        message: String,
    },
    #[error(transparent)]
    Privilege(#[from] PrivilegeError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("failed to stage elevation install sources: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use voya_platform::test_support::RecordingRunner;

    fn unique_temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("voya-elevation-{}-{name}", std::process::id()))
    }

    fn manager(runner: Arc<dyn ProcessRunner>, os: TargetOs, name: &str) -> ElevationManager {
        ElevationManager::with_target_os(runner, unique_temp_dir(name), "/tmp/app/bin", os)
    }

    #[test]
    fn elevation_state_starts_ungranted() {
        let manager = manager(
            Arc::new(RecordingRunner::default()),
            TargetOs::Macos,
            "ungranted",
        );
        assert!(!manager.is_granted());
        assert!(!manager.state().is_granted());
    }

    #[test]
    fn elevation_request_grants_when_native_command_succeeds() {
        let runner = Arc::new(RecordingRunner::default());
        let manager = manager(runner.clone(), TargetOs::Macos, "grant");

        manager.request().expect("request should be granted");

        assert!(manager.is_granted());
        assert_eq!(runner.events().as_slice(), ["oneshot:Probe"]);
        // Idempotent while granted: no second native command is issued.
        manager.request().expect("already granted");
        assert_eq!(runner.events().as_slice(), ["oneshot:Probe"]);

        let _ = std::fs::remove_dir_all(unique_temp_dir("grant"));
    }
}
