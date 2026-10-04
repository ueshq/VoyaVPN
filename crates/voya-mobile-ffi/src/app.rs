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
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    time::Duration,
};

use voya_app::{
    config_mutation::ConfigMutationCoordinator,
    contract_map::errors::database_error,
    lifecycle::exit_step,
    proxy_runtime::{ProxyMonitorController, ProxyRuntimeManager},
    services::AppServices,
    speedtest::SpeedtestManager,
    startup::{AppOpening, OpenedApp},
    supervisor::{CoreSupervisor, SupervisorDeps},
    sysproxy::SystemProxyManager,
};
use voya_contracts::{AppError, AppErrorKind, AppErrorSubsystem};
use voya_platform::{
    coreinfo::TargetOs,
    paths::AppPaths,
    privilege::ElevationState,
    process::{ProcessError, ProcessHandle, ProcessOutput, ProcessRunner, ProcessSpawn},
    tun::NativeTunController,
};

use crate::{
    dispatch::{self, SupervisorRecoverySink},
    probe::{HostProbeCoreLauncher, ProbeCoreHost},
    sinks::{EventListener, HostSinks},
    tunnel::{HostTunController, TunnelHost},
};

/// A command the backend rejected.
///
/// The payload is a serialized `AppError` — the very value the Tauri transport
/// rejects with — so the frontend branches on the typed `kind` rather than on a
/// message, on both platforms. uniffi wants an error *type* here; making it
/// carry JSON rather than modelling every `AppErrorKind` variant is the same
/// envelope choice ADR 0012 makes for the arguments and the answer.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CommandError {
    #[error("{app_error_json}")]
    Rejected { app_error_json: String },
}

/// What the app's runtime names its worker threads.
const RUNTIME_THREAD_NAME: &str = "voya-mobile";

/// How long `shutdown` waits for the tunnel to come down. The host's own stop
/// is bounded, but it is a blocking call into the platform, and the thread
/// that invalidates the native module must not wait on it forever.
const DISCONNECT_SHUTDOWN_LIMIT: Duration = Duration::from_secs(15);
/// How long `shutdown` waits for commands still holding a connection.
const DATABASE_SHUTDOWN_LIMIT: Duration = Duration::from_secs(5);

const SHUT_DOWN: &str = "the app has been shut down";

/// The one failure that cannot be reported as itself.
const UNENCODABLE_FAILURE: &str = r#"{"kind":{"type":"internal"},"subsystem":"app","message":"the failure could not be encoded"}"#;

/// A startup that never produced an app.
///
/// Every variant carries a serialized `AppError` — the same envelope
/// [`CommandError`] rejects with — so the frontend branches on the typed
/// `kind`: a `database` startup failure keeps its code (`schemaUnsupported`,
/// `corrupt`) and is the one failure the app can recover from on its own.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum StartupError {
    #[error("{app_error_json}")]
    Paths { app_error_json: String },
    #[error("{app_error_json}")]
    Database { app_error_json: String },
    #[error("{app_error_json}")]
    Runtime { app_error_json: String },
}

/// An application-data reset the host could not perform.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ResetError {
    #[error("{app_error_json}")]
    Failed { app_error_json: String },
}

/// Moves the app's database aside so the next `VoyaApp` construction starts
/// fresh, and nothing else: settings live in the same file, and a phone has
/// no rule-set cache worth keeping across a reset.
///
/// The user-facing counterpart of the desktop's reset dialog — on a phone
/// there is no shell to run the manual recovery command in, so this is the
/// only recovery a rejected database has.
#[uniffi::export]
pub fn reset_application_data(data_dir: String) -> Result<(), ResetError> {
    let database_path = PathBuf::from(&data_dir).join(voya_app::startup::DATABASE_NAME);
    voya_app::startup::reset_database(&database_path)
        .map(|_| ())
        .map_err(|error| ResetError::Failed {
            app_error_json: encode_app_error(&database_error(&error, AppErrorSubsystem::App)),
        })
}

/// Everything a dispatcher needs, behind one handle.
pub struct MobileState {
    pub(crate) services: AppServices,
    pub(crate) config_mutations: Arc<ConfigMutationCoordinator>,
    pub(crate) supervisor: CoreSupervisor,
    pub(crate) sinks: Arc<HostSinks>,
    pub(crate) elevation: Arc<ElevationState>,
    /// The host's tunnel, shared by the supervisor and every per-command
    /// `TunManager`. Without it the status reads would fall back to
    /// `voya-platform`'s controller, which on a phone can only answer that the
    /// tunnel is not its to report.
    pub(crate) native_tun: Arc<dyn NativeTunController>,
    /// One manager — and therefore one HTTP client, TLS config and keep-alive
    /// pool — for every proxy command, so the loopback connection into the
    /// running core survives between them.
    pub(crate) proxy_runtime: ProxyRuntimeManager,
    pub(crate) proxy_monitor: ProxyMonitorController,
    pub(crate) speedtest: SpeedtestManager,
    /// One manager over a service that will refuse, which is the honest shape:
    /// the platform reports `SystemProxyManagement::Unsupported` for both
    /// phones, so every path through `core_flow` skips the proxy before it
    /// reaches this.
    pub(crate) system_proxy_manager: SystemProxyManager,
    /// This state itself, for work that outlives the call that started it:
    /// the IPv6 egress check after a connect, and the recovery the supervisor
    /// asks for when the provider goes down. Weak so the state does not keep
    /// itself alive.
    pub(crate) this: Weak<MobileState>,
    /// The app's runtime, for that same work: the supervisor's callbacks
    /// arrive on its own actor, which must stay free while recovery runs.
    pub(crate) runtime: tokio::runtime::Handle,
}

#[derive(uniffi::Object)]
pub struct VoyaApp {
    /// Owned rather than borrowed from the host: iOS and Android both call in
    /// from threads that have no runtime of their own, and the managers spawn
    /// tasks that must outlive any one call. Every command runs here — see
    /// [`VoyaApp::invoke`].
    runtime: tokio::runtime::Runtime,
    state: Arc<MobileState>,
    /// Set by [`VoyaApp::shutdown`]. The platform keeps this object until its
    /// own collector frees it, long after the host is done with it.
    shut_down: AtomicBool,
}

impl VoyaApp {
    /// [`VoyaApp::new`] for a stated platform. The host is always built for
    /// the phone it runs on; the tests stand in for one, on whatever machine
    /// runs them.
    pub(crate) fn open(
        target_os: TargetOs,
        data_dir: String,
        locale: Option<String>,
        events: Arc<dyn EventListener>,
        tunnel: Arc<dyn TunnelHost>,
        probe_core: Arc<dyn ProbeCoreHost>,
    ) -> Result<Arc<Self>, StartupError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .thread_name(RUNTIME_THREAD_NAME)
            .enable_all()
            .build()
            .map_err(|error| StartupError::Runtime {
                app_error_json: encode_app_error(&AppError::internal(
                    AppErrorSubsystem::App,
                    error.to_string(),
                )),
            })?;
        // Before the first `warn!` of the startup sequence, so early failures
        // (a failed rule-set seed, a rejected database sweep) reach the Logs
        // page instead of vanishing.
        crate::logging::install_subscriber();
        let data_dir = PathBuf::from(data_dir);
        let paths = AppPaths::new(&data_dir);
        paths.ensure_dirs().map_err(|error| StartupError::Paths {
            app_error_json: encode_app_error(&AppError::new(
                AppErrorSubsystem::App,
                AppErrorKind::Io,
                error.to_string(),
            )),
        })?;

        let state = runtime.block_on(connect(
            target_os,
            &data_dir,
            paths,
            locale.as_deref(),
            events,
            tunnel,
            probe_core,
        ))?;

        // The queued lines this app produced so far now have a listener to
        // reach, once a Logs screen asks for them.
        crate::logging::attach(runtime.handle(), Arc::clone(&state.sinks));

        Ok(Arc::new(Self {
            runtime,
            state,
            shut_down: AtomicBool::new(false),
        }))
    }
}

#[uniffi::export]
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
        probe_core: Arc<dyn ProbeCoreHost>,
    ) -> Result<Arc<Self>, StartupError> {
        Self::open(
            TargetOs::current(),
            data_dir,
            locale,
            events,
            tunnel,
            probe_core,
        )
    }

    /// Runs one backend command.
    ///
    /// `args_json` is the same named-argument object Tauri receives, and the
    /// answer is the command's return value as JSON. A failure comes back as a
    /// serialized `AppError`, so the frontend branches on the same typed `kind`
    /// it does on the desktop rather than on a message.
    ///
    /// The platform polls this future on a thread of its own, which has no
    /// runtime: left there, the command would run on that thread and whatever
    /// it spawned would have nowhere to go. So the command is handed to the
    /// app's runtime and only its result is awaited here — which also means a
    /// call the platform abandons still runs to completion, as a Tauri command
    /// does.
    pub async fn invoke(&self, command: String, args_json: String) -> Result<String, CommandError> {
        if self.shut_down.load(Ordering::Acquire) {
            return Err(CommandError::Rejected {
                app_error_json: encode_app_error(&AppError::internal(
                    AppErrorSubsystem::App,
                    SHUT_DOWN.to_string(),
                )),
            });
        }
        let state = Arc::clone(&self.state);
        let name = command.clone();
        let task = self
            .runtime
            .spawn(async move { dispatch::invoke(&state, &command, &args_json).await });

        task.await
            .unwrap_or_else(|error| {
                Err(AppError::internal(
                    AppErrorSubsystem::App,
                    format!("{name} did not finish: {error}"),
                ))
            })
            .map_err(|error| CommandError::Rejected {
                app_error_json: encode_app_error(&error),
            })
    }

    /// Stops what the app started and lets go of the database. Safe to call
    /// more than once.
    ///
    /// The host moves the database aside right after this on a reset, and
    /// builds a second app on the same file after a reload, so the pool has to
    /// be closed here: the object itself lives until the platform frees it.
    pub fn shutdown(&self) {
        if self.shut_down.swap(true, Ordering::AcqRel) {
            return;
        }
        let state = &self.state;
        crate::logging::detach(&state.sinks);
        // Its task holds the sinks, and through them the platform's listener.
        if let Err(error) = state.proxy_monitor.stop() {
            tracing::warn!(
                ?error,
                "the connection monitor did not stop during shutdown"
            );
        }
        // Probe cores belong to the app process, not to the supervisor, and an
        // abandoned run leaves one listening; the desktop shell reaps them
        // the same way in `apps/desktop/src-tauri/src/lifecycle.rs` and `voya_app::lifecycle`.
        state.speedtest.shutdown();
        self.runtime.block_on(async {
            // Through the runtime manager rather than the supervisor: it waits
            // for a connect still in flight, and removes the generated config,
            // which carries the node's credentials.
            let runtime = state.services.runtime(state.supervisor.clone(), None);
            if let Some(Err(error)) = exit_step(
                "core disconnect",
                DISCONNECT_SHUTDOWN_LIMIT,
                runtime.disconnect(),
            )
            .await
            {
                tracing::warn!(?error, "the core did not stop cleanly during shutdown");
            }
            exit_step(
                "database close",
                DATABASE_SHUTDOWN_LIMIT,
                state.services.close(),
            )
            .await;
        });
    }
}

async fn connect(
    target_os: TargetOs,
    data_dir: &Path,
    paths: AppPaths,
    locale: Option<&str>,
    events: Arc<dyn EventListener>,
    tunnel: Arc<dyn TunnelHost>,
    probe_core: Arc<dyn ProbeCoreHost>,
) -> Result<Arc<MobileState>, StartupError> {
    let database_path = data_dir.join(voya_app::startup::DATABASE_NAME);
    // The error arrives by inference rather than by name: spelling out the
    // persistence crate's error type here would reach past the `voya-app`
    // facade the architecture gate exists to protect.
    let opening = AppOpening::open(&database_path, paths, target_os, locale)
        .await
        .map_err(|error| StartupError::Database {
            app_error_json: encode_app_error(&database_error(&error, AppErrorSubsystem::App)),
        })?;
    // The same sequence `apps/desktop/src-tauri/src/bootstrap.rs` runs; a phone
    // has no system proxy to undo between its two halves.
    let OpenedApp {
        services,
        config_mutations,
    } = opening.finish().await;

    let sinks = Arc::new(HostSinks::new(events));
    let elevation = Arc::new(ElevationState::new());
    let native_tun: Arc<dyn NativeTunController> =
        Arc::new(HostTunController::new(tunnel, data_dir.to_path_buf()));
    let system_proxy_manager = services.system_proxy_manager(Arc::new(NoProcessRunner));
    let runtime = tokio::runtime::Handle::current();

    // Cyclic because the supervisor is told, as it starts, whom to ask for
    // recovery when the provider dies — and that is this very state.
    Ok(Arc::new_cyclic(|this: &Weak<MobileState>| {
        let supervisor = CoreSupervisor::spawn(
            SupervisorDeps::new(Arc::new(NoProcessRunner), Arc::clone(&elevation))
                .with_target_os(target_os)
                .with_native_tun_controller(Arc::clone(&native_tun))
                .with_event_sink(Arc::new(SupervisorRecoverySink::new(
                    Weak::clone(this),
                    runtime.clone(),
                ))),
        );
        // While connected the test goes through the provider's own core, so
        // this launcher only ever starts the in-app instance a disconnected
        // run needs.
        let speedtest = services.speedtest_manager_with_launcher(
            Arc::new(HostProbeCoreLauncher::new(probe_core)),
            supervisor.clone(),
        );

        MobileState {
            config_mutations,
            elevation,
            native_tun,
            proxy_monitor: ProxyMonitorController::new(),
            proxy_runtime: ProxyRuntimeManager::new(),
            runtime,
            services,
            sinks,
            speedtest,
            supervisor,
            system_proxy_manager,
            this: Weak::clone(this),
        }
    }))
}

/// A runner that refuses.
///
/// Nothing on a phone may spawn a process: the core runs inside the tunnel
/// provider, and `SupervisorActor::plan_start` takes the native branch before
/// it reaches a runner at all. Installing a rejecting one rather than a no-op
/// means a path that *would* have spawned fails loudly here instead of
/// silently doing nothing.
struct NoProcessRunner;

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

/// An `AppError` that will not serialize is a bug in the contract, not a
/// reason to drop the failure.
fn encode_app_error(error: &AppError) -> String {
    serde_json::to_string(error).unwrap_or_else(|_| {
        tracing::error!(?error, "an AppError could not be serialized");
        UNENCODABLE_FAILURE.to_string()
    })
}

fn refused(request: &ProcessSpawn) -> ProcessError {
    ProcessError::Spawn {
        executable: PathBuf::from(&request.executable),
        source: std::io::Error::new(std::io::ErrorKind::Unsupported, NO_CHILD_PROCESSES),
    }
}

#[cfg(test)]
mod tests;
