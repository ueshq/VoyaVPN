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
    filesystem,
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
    launcher_path: Option<PathBuf>,
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
            launcher_path: elevate_launcher_path(target_os),
        }
    }

    /// Override where the root launcher is expected to live.
    ///
    /// The real path is fixed per platform; this exists so the stale-grant
    /// sweep can be exercised without writing to a system directory.
    #[must_use]
    pub fn with_launcher_path(mut self, launcher_path: Option<PathBuf>) -> Self {
        self.launcher_path = launcher_path;
        self
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
        let _ = filesystem::remove_dir_all_if_exists(&work_dir);

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
        if !self.launcher_present() && !self.state.is_granted() {
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

    /// Remove a launcher + sudoers drop-in left behind by a previous run.
    ///
    /// [`Self::revoke`] only ever runs on a clean exit, so a crash, a SIGKILL or
    /// a power loss leaves the root-owned launcher and its `NOPASSWD` drop-in
    /// installed indefinitely — a passwordless root primitive for every local
    /// process, long after the session that asked for it ended. Startup calls
    /// this before anything can spawn a core, so the grant never outlives the
    /// app run that requested it.
    ///
    /// Returns whether a stale launcher was found. Removal itself is
    /// best-effort: `sudo -n` fails silently when the drop-in is already gone.
    pub fn revoke_stale_grant(&self) -> bool {
        if !self.launcher_present() {
            return false;
        }

        tracing::warn!("removing a TUN elevation launcher left behind by a previous run");
        self.revoke();
        true
    }

    fn launcher_present(&self) -> bool {
        self.launcher_path
            .as_deref()
            .and_then(|launcher| filesystem::file_exists(launcher).ok())
            .unwrap_or(false)
    }

    fn stage_install_sources(
        &self,
        plan: &privilege::ElevationInstallPlan,
    ) -> Result<(), ElevationError> {
        filesystem::stage_private_files(
            &plan.work_dir,
            &[
                (&plan.src_launcher_path, &plan.launcher_contents),
                (&plan.src_sudoers_path, &plan.sudoers_contents),
                (&plan.install_script_path, &plan.install_script_contents),
            ],
        )
        .map_err(Into::into)
    }
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
        // The launcher path is overridden so a real installation on the
        // developer's machine cannot influence the result.
        ElevationManager::with_target_os(runner, unique_temp_dir(name), "/tmp/app/bin", os)
            .with_launcher_path(Some(unique_temp_dir(name).join("voya-elevate")))
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

    /// A crash never reaches the exit-time revoke, so the root launcher and its
    /// NOPASSWD drop-in can outlive the run that installed them. Startup must
    /// sweep them before anything can use them.
    #[test]
    fn startup_sweep_revokes_a_launcher_left_behind_by_a_crash() {
        let runner = Arc::new(RecordingRunner::default());
        let work_dir = unique_temp_dir("stale-launcher");
        std::fs::create_dir_all(&work_dir).expect("create stale launcher dir");
        let launcher = work_dir.join("voya-elevate");
        std::fs::write(&launcher, b"#!/bin/sh\n").expect("write stale launcher");

        let manager = manager(runner.clone(), TargetOs::Macos, "stale-launcher")
            .with_launcher_path(Some(launcher));

        assert!(manager.revoke_stale_grant());
        assert_eq!(runner.events().as_slice(), ["oneshot:SudoKill"]);
        assert!(!manager.is_granted());

        let _ = std::fs::remove_dir_all(&work_dir);
    }

    #[test]
    fn startup_sweep_is_a_no_op_without_a_stale_launcher() {
        let runner = Arc::new(RecordingRunner::default());
        let manager = manager(runner.clone(), TargetOs::Macos, "no-launcher")
            .with_launcher_path(Some(unique_temp_dir("no-launcher").join("absent")));

        assert!(!manager.revoke_stale_grant());
        assert!(runner.events().is_empty());
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
