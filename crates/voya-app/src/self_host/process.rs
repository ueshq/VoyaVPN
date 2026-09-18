//! Writing the node's config and running its core on the dedicated runner.
//!
//! Every function here blocks (file writes, `spawn`, and a `stop` that waits
//! for the child to be reaped), so the manager calls them on
//! `spawn_blocking`.

use std::{path::PathBuf, time::Instant};

use tokio::sync::mpsc::UnboundedSender;
use voya_contracts::SelfHostProblem;
use voya_core::{generate_singbox_selfhost_config_json, SelfHostSpec};
use voya_platform::{
    coreinfo::{core_launch, CoreInfoError},
    filesystem,
    process::{ProcessExit, ProcessExitHandler, ProcessHandle, ProcessRole, ProcessSpawn},
};

use super::{Result, SelfHostDeps, SelfHostError};
use crate::{
    runtime::{resolve_core_executable, write_core_config},
    supervisor::ClashApiSecret,
};

pub(super) const SELF_HOST_CONFIG_FILE_NAME: &str = "configSelfHost.json";

/// The node's core while it runs.
#[derive(Debug)]
pub(super) struct RunningCore {
    pub(super) handle: ProcessHandle,
    pub(super) clash_port: u16,
    pub(super) clash_secret: ClashApiSecret,
    pub(super) started_at: Instant,
}

/// Forwards the dedicated runner's exit callbacks, which arrive on a reaper
/// thread, into the manager's async loop.
pub(super) struct ExitForwarder(pub(super) UnboundedSender<ProcessExit>);

impl ProcessExitHandler for ExitForwarder {
    fn process_exited(&self, exit: ProcessExit) {
        if self.0.send(exit).is_err() {
            tracing::debug!(
                pid = exit.process_id,
                "self-hosted exit arrived after shutdown"
            );
        }
    }
}

/// The sing-box binary the node runs: the same one the connection core and
/// the speed test resolve, so the firewall rule names the right program.
pub(super) fn core_executable(deps: &SelfHostDeps) -> Result<PathBuf> {
    Ok(resolve_core_executable(
        &deps.paths,
        deps.core_seed_resource_dir.as_deref(),
        deps.target_os,
    )?)
}

/// Writes the config (0600, like every generated config) and spawns the core.
pub(super) fn start_core(deps: &SelfHostDeps, spec: &SelfHostSpec) -> Result<ProcessHandle> {
    let json = generate_singbox_selfhost_config_json(spec)?;
    write_core_config(&deps.paths, SELF_HOST_CONFIG_FILE_NAME, &json).map_err(|error| {
        SelfHostError::WriteConfig {
            path: error.path,
            source: error.source,
        }
    })?;
    let spawned = core_executable(deps).and_then(|executable| {
        let launch = core_launch(executable, &deps.paths, SELF_HOST_CONFIG_FILE_NAME);
        let spawn = ProcessSpawn::from_core_launch(ProcessRole::SelfHost, &launch, true)?;
        Ok(deps.runner.spawn(spawn)?)
    });
    if spawned.is_err() {
        remove_config(deps);
    }
    spawned
}

pub(super) fn stop_core(deps: &SelfHostDeps, handle: &ProcessHandle) {
    if let Err(error) = deps.runner.stop(handle) {
        tracing::warn!(?error, "failed to stop the self-hosted core");
    }
    remove_config(deps);
}

/// The config embeds the node's private key, so it only exists while the core
/// runs.
pub(super) fn remove_config(deps: &SelfHostDeps) {
    let path = deps.paths.bin_config_file(SELF_HOST_CONFIG_FILE_NAME);
    if let Err(error) = filesystem::remove_file_if_exists(&path) {
        tracing::warn!(path = %path.display(), ?error, "failed to remove the self-hosted config");
    }
}

/// What a failed start means for the page.
pub(super) fn start_problem(error: &SelfHostError) -> SelfHostProblem {
    match error {
        SelfHostError::CoreInfo(CoreInfoError::ExecutableNotFound { .. }) => {
            SelfHostProblem::CoreMissing
        }
        SelfHostError::NoFreePort => SelfHostProblem::PortInUse,
        _ => SelfHostProblem::StartFailed,
    }
}
