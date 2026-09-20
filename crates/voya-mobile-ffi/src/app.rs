//! The host object the platform holds.
//!
//! `VoyaApp` plays the part `apps/desktop/src-tauri`'s `AppState` +
//! `bootstrap.rs` play: it owns the tokio runtime, opens the database through
//! `AppServices::connect`, injects the dependencies the managers need, and
//! keeps them alive for the life of the app.
//!
//! What differs from the shell is only what a phone differs in: no core is
//! spawned as a child process (the tunnel provider runs it), nothing is
//! elevated, no system proxy is set, and the self-hosted node is not assembled
//! at all.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use voya_app::{
    config_mutation::ConfigMutationCoordinator,
    services::AppServices,
    supervisor::{CoreSupervisor, SupervisorDeps},
};
use voya_platform::{
    coreinfo::TargetOs,
    paths::AppPaths,
    privilege::ElevationState,
    process::{ProcessError, ProcessHandle, ProcessOutput, ProcessRunner, ProcessSpawn},
};

use crate::{
    dispatch,
    sinks::{EventListener, HostSinks},
    tunnel::{HostTunController, TunnelHost},
};

/// A startup that never produced an app.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum StartupError {
    #[error("could not prepare the application directories: {message}")]
    Paths { message: String },
    #[error("could not open the database: {message}")]
    Database { message: String },
    #[error("could not start the runtime: {message}")]
    Runtime { message: String },
}

/// Everything a dispatcher needs, behind one handle.
pub struct MobileState {
    pub(crate) services: AppServices,
    pub(crate) config_mutations: Arc<ConfigMutationCoordinator>,
    pub(crate) supervisor: CoreSupervisor,
    pub(crate) sinks: Arc<HostSinks>,
    pub(crate) elevation: Arc<ElevationState>,
}

#[derive(uniffi::Object)]
pub struct VoyaApp {
    /// Owned rather than borrowed from the host: iOS and Android both call in
    /// from threads that have no runtime of their own, and the managers spawn
    /// tasks that must outlive any one call.
    runtime: tokio::runtime::Runtime,
    state: Arc<MobileState>,
}

#[uniffi::export(async_runtime = "tokio")]
impl VoyaApp {
    /// Opens the app's storage and brings the managers up.
    ///
    /// `data_dir` is the container both the app and its tunnel provider can
    /// reach — the App Group container on iOS — because the handshake stages
    /// rule sets there. `locale` seeds the language on a fresh install only.
    #[uniffi::constructor]
    pub fn new(
        data_dir: String,
        locale: Option<String>,
        events: Arc<dyn EventListener>,
        tunnel: Arc<dyn TunnelHost>,
    ) -> Result<Arc<Self>, StartupError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| StartupError::Runtime {
                message: error.to_string(),
            })?;
        let data_dir = PathBuf::from(data_dir);
        let paths = AppPaths::new(&data_dir);
        paths.ensure_dirs().map_err(|error| StartupError::Paths {
            message: error.to_string(),
        })?;

        let state =
            runtime.block_on(connect(&data_dir, paths, locale.as_deref(), events, tunnel))?;

        Ok(Arc::new(Self {
            runtime,
            state: Arc::new(state),
        }))
    }

    /// Runs one backend command.
    ///
    /// `args_json` is the same named-argument object Tauri receives, and the
    /// answer is the command's return value as JSON. A failure comes back as a
    /// serialized `AppError`, so the frontend branches on the same typed `kind`
    /// it does on the desktop rather than on a message.
    pub async fn invoke(&self, command: String, args_json: String) -> Result<String, String> {
        dispatch::invoke(&self.state, &command, &args_json)
            .await
            .map_err(|error| {
                serde_json::to_string(&error).unwrap_or_else(|_| {
                    // An AppError that will not serialize is a bug in the
                    // contract, not a reason to drop the failure.
                    tracing::error!(?error, "an AppError could not be serialized");
                    r#"{"kind":{"type":"internal"},"subsystem":"app","message":"the failure could not be encoded"}"#.to_string()
                })
            })
    }

    /// Stops what the app started. Safe to call more than once.
    pub fn shutdown(&self) {
        self.runtime.block_on(async {
            if let Err(error) = self.state.supervisor.stop().await {
                tracing::warn!(?error, "the core did not stop cleanly during shutdown");
            }
        });
    }
}

async fn connect(
    data_dir: &Path,
    paths: AppPaths,
    locale: Option<&str>,
    events: Arc<dyn EventListener>,
    tunnel: Arc<dyn TunnelHost>,
) -> Result<MobileState, StartupError> {
    let database_path = data_dir.join(voya_app::startup::DATABASE_NAME);
    let services = AppServices::connect(&database_path, paths)
        .await
        .map_err(database_failure)?;
    let config = services
        .load_config_for(TargetOs::current(), locale)
        .await
        .map_err(database_failure)?;
    let config_mutations = Arc::new(
        services
            .config_mutations(Arc::new(RwLock::new(config)))
            .with_target_os(TargetOs::current()),
    );
    let sinks = Arc::new(HostSinks::new(events));
    let elevation = Arc::new(ElevationState::new());
    let supervisor = CoreSupervisor::spawn(
        SupervisorDeps::new(Arc::new(NoProcessRunner), Arc::clone(&elevation))
            .with_native_tun_controller(Arc::new(HostTunController::new(
                tunnel,
                data_dir.to_path_buf(),
            )))
            .with_event_sink(Arc::clone(&sinks) as Arc<_>),
    );

    Ok(MobileState {
        config_mutations,
        elevation,
        services,
        sinks,
        supervisor,
    })
}

/// A runner that refuses.
///
/// Nothing on a phone may spawn a process: the core runs inside the tunnel
/// provider, and `SupervisorActor::plan_start` takes the native branch before
/// it reaches a runner at all. Installing a rejecting one rather than a no-op
/// means a path that *would* have spawned fails loudly here instead of
/// silently doing nothing.
pub(crate) struct NoProcessRunner;

/// A runner for the desktop-shaped constructors that still take one.
pub(crate) fn no_process_runner() -> Arc<dyn ProcessRunner> {
    Arc::new(NoProcessRunner)
}

const NO_CHILD_PROCESSES: &str =
    "this platform runs the core inside its tunnel provider and spawns no child processes";

impl ProcessRunner for NoProcessRunner {
    fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
        Err(refused(&request))
    }

    fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
        Err(refused(&request))
    }

    fn stop(&self, _handle: &ProcessHandle) -> Result<(), ProcessError> {
        Ok(())
    }
}

fn database_failure(error: impl std::fmt::Display) -> StartupError {
    StartupError::Database {
        message: error.to_string(),
    }
}

fn refused(request: &ProcessSpawn) -> ProcessError {
    ProcessError::Spawn {
        executable: PathBuf::from(&request.executable),
        source: std::io::Error::new(std::io::ErrorKind::Unsupported, NO_CHILD_PROCESSES),
    }
}

#[cfg(test)]
mod tests;
