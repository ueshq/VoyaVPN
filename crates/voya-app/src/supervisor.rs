use std::{fmt, path::PathBuf, sync::Arc, time::Duration};

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use voya_core::CoreType;
use voya_platform::{
    coreinfo::{CoreLaunch, TargetOs},
    elevation::{
        should_use_unix_sudo, unix_sudo_kill_spawn_passwordless,
        wrap_spawn_with_unix_sudo_passwordless,
    },
    privilege::{elevate_launcher_path, ElevationState},
    process::{
        NoopProcessJobFactory, PlatformProcessJobFactory, ProcessError, ProcessExit,
        ProcessExitHandler, ProcessHandle, ProcessJob, ProcessJobFactory, ProcessOutput,
        ProcessRole, ProcessRunner, ProcessSpawn, StdProcessRunner,
    },
    tun::{
        tun_backend, NativeTunController, NativeTunError, NativeTunProviderState,
        NativeTunStartRequest, NoopNativeTunController, PlatformNativeTunController, TunBackend,
    },
    tun::{NoopTunCleaner, PlatformTunCleaner, TunCleaner, TunCleanupError},
};

/// Bearer token the running core's Clash API requires.
///
/// sing-box's Clash API listens on loopback, which is *not* a trust boundary:
/// any local process, and any web page a browser can be pointed at, could
/// otherwise read the live connection list, switch every route to direct or pin
/// a node. A fresh token is minted per core launch, written into the generated
/// `experimental.clash_api.secret`, and demanded of every REST call and
/// websocket upgrade.
///
/// The value is deliberately opaque: `Debug` redacts it so a token can never
/// reach a tracing field, a log file or a crash report, and reading it back
/// takes the explicit [`ClashApiSecret::as_str`].
#[derive(Clone, PartialEq, Eq)]
pub struct ClashApiSecret(String);

impl fmt::Debug for ClashApiSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClashApiSecret(<redacted>)")
    }
}

impl ClashApiSecret {
    /// Mints a token for one core launch.
    ///
    /// Two v4 UUIDs give 244 random bits from the platform CSPRNG (each carries
    /// 122; six bits are version and variant markers). `uuid` is already a
    /// workspace dependency, so this adds no new supply-chain surface.
    #[must_use]
    pub fn generate() -> Self {
        Self(format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_token(self) -> String {
        self.0
    }
}

/// Everything a Clash API client needs to reach the core that is *running*.
///
/// Port and token are minted together by one core launch and are useless apart:
/// a stale token against a restarted core is a 401, and the port alone is a
/// 401 too. Carrying them as one value keeps every caller from re-deriving
/// either half.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClashApiAccess {
    pub port: Option<u16>,
    pub secret: Option<ClashApiSecret>,
}

impl ClashApiAccess {
    #[must_use]
    pub const fn new(port: Option<u16>, secret: Option<ClashApiSecret>) -> Self {
        Self { port, secret }
    }

    /// Access to a core on `port` that needs no token. Test and preview helper.
    #[must_use]
    pub const fn unauthenticated(port: u16) -> Self {
        Self {
            port: Some(port),
            secret: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreProcessSpec {
    pub core_type: CoreType,
    pub launch: CoreLaunch,
    pub config_path: Option<PathBuf>,
    pub display_log: bool,
    pub may_need_sudo: bool,
}

impl CoreProcessSpec {
    #[must_use]
    pub const fn new(core_type: CoreType, launch: CoreLaunch) -> Self {
        Self {
            core_type,
            launch,
            config_path: None,
            display_log: true,
            may_need_sudo: true,
        }
    }

    #[must_use]
    pub fn with_config_path(mut self, config_path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(config_path.into());
        self
    }

    #[must_use]
    pub const fn with_display_log(mut self, display_log: bool) -> Self {
        self.display_log = display_log;
        self
    }

    #[must_use]
    pub const fn with_may_need_sudo(mut self, may_need_sudo: bool) -> Self {
        self.may_need_sudo = may_need_sudo;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorStartRequest {
    pub active_profile_id: Option<String>,
    pub main: CoreProcessSpec,
    pub pre: Option<CoreProcessSpec>,
    pub tun_enabled: bool,
    pub sudo_script_dir: PathBuf,
    pub restart_on_crash: bool,
    /// Clash API port of the *main* generated config.
    ///
    /// This cannot be recomputed from `AppConfig`: on a pre-socks topology the
    /// builder clears `is_tun_enabled` on the main context, so the main process
    /// listens on `api2` while the pre-socks one takes `api2 + 1`. Deriving it
    /// from the TUN setting instead would point every client at the pre-socks
    /// process, which has no selector or per-node statistics.
    pub clash_api_port: i32,
    /// Bearer token the *main* generated config wrote into
    /// `experimental.clash_api.secret`, if the caller minted one.
    pub clash_api_secret: Option<ClashApiSecret>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorConnectionState {
    Disconnected,
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorSnapshot {
    pub state: SupervisorConnectionState,
    pub active_profile_id: Option<String>,
    pub main_pid: Option<u32>,
    pub pre_pid: Option<u32>,
    pub running_core_type: Option<CoreType>,
    /// Clash API port the running main config actually listens on.
    pub clash_api_port: Option<i32>,
    /// Bearer token the running main config demands on that port.
    pub clash_api_secret: Option<ClashApiSecret>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTunExitEvent {
    pub active_profile_id: Option<String>,
    pub backend: TunBackend,
    pub message: String,
}

pub trait SupervisorEventSink: Send + Sync {
    fn native_tun_exited(&self, event: NativeTunExitEvent);

    /// A tracked core process exited and the supervisor has decided what to do
    /// about it. The default ignores the event so sinks that only care about
    /// the native TUN backend keep compiling.
    fn core_exited(&self, _event: CoreExitEvent) {}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSupervisorEventSink;

impl SupervisorEventSink for NoopSupervisorEventSink {
    fn native_tun_exited(&self, _event: NativeTunExitEvent) {}
}

impl SupervisorSnapshot {
    #[must_use]
    pub const fn disconnected() -> Self {
        Self {
            state: SupervisorConnectionState::Disconnected,
            active_profile_id: None,
            main_pid: None,
            pre_pid: None,
            running_core_type: None,
            clash_api_port: None,
            clash_api_secret: None,
        }
    }

    /// How to reach the running core's Clash API.
    ///
    /// Both halves are cleared while nothing is running, so a caller that hands
    /// this straight to a client dials nothing instead of dialling a port the
    /// previous core no longer owns.
    #[must_use]
    pub fn clash_api_access(&self) -> ClashApiAccess {
        ClashApiAccess {
            port: self
                .clash_api_port
                .and_then(|port| u16::try_from(port).ok()),
            secret: self.clash_api_secret.clone(),
        }
    }
}

#[derive(Clone)]
pub struct SupervisorDeps {
    pub runner: Arc<dyn ProcessRunner>,
    pub elevation: Arc<ElevationState>,
    pub job_factory: Arc<dyn ProcessJobFactory>,
    pub tun_cleaner: Arc<dyn TunCleaner>,
    pub native_tun_controller: Arc<dyn NativeTunController>,
    pub native_tun_health_interval: Duration,
    pub event_sink: Arc<dyn SupervisorEventSink>,
    pub clock: Arc<dyn SupervisorClock>,
    pub crash_restart_policy: CrashRestartPolicy,
    pub target_os: TargetOs,
}

impl SupervisorDeps {
    #[must_use]
    pub fn new(runner: Arc<dyn ProcessRunner>, elevation: Arc<ElevationState>) -> Self {
        Self {
            runner,
            elevation,
            job_factory: Arc::new(NoopProcessJobFactory),
            tun_cleaner: Arc::new(NoopTunCleaner),
            native_tun_controller: Arc::new(NoopNativeTunController),
            native_tun_health_interval: Duration::from_secs(3),
            event_sink: Arc::new(NoopSupervisorEventSink),
            clock: Arc::new(SystemSupervisorClock),
            crash_restart_policy: CrashRestartPolicy::default(),
            target_os: TargetOs::current(),
        }
    }

    #[must_use]
    pub fn platform() -> Self {
        Self::platform_with_runner(
            Arc::new(StdProcessRunner::new()),
            Arc::new(ElevationState::new()),
        )
    }

    #[must_use]
    pub fn platform_with_runner(
        runner: Arc<dyn ProcessRunner>,
        elevation: Arc<ElevationState>,
    ) -> Self {
        Self {
            runner,
            elevation,
            job_factory: Arc::new(PlatformProcessJobFactory),
            tun_cleaner: Arc::new(PlatformTunCleaner),
            native_tun_controller: Arc::new(PlatformNativeTunController),
            native_tun_health_interval: Duration::from_secs(3),
            event_sink: Arc::new(NoopSupervisorEventSink),
            clock: Arc::new(SystemSupervisorClock),
            crash_restart_policy: CrashRestartPolicy::default(),
            target_os: TargetOs::current(),
        }
    }

    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn SupervisorClock>) -> Self {
        self.clock = clock;
        self
    }

    #[must_use]
    pub const fn with_crash_restart_policy(mut self, policy: CrashRestartPolicy) -> Self {
        self.crash_restart_policy = policy;
        self
    }

    #[must_use]
    pub fn with_job_factory(mut self, job_factory: Arc<dyn ProcessJobFactory>) -> Self {
        self.job_factory = job_factory;
        self
    }

    #[must_use]
    pub fn with_tun_cleaner(mut self, tun_cleaner: Arc<dyn TunCleaner>) -> Self {
        self.tun_cleaner = tun_cleaner;
        self
    }

    #[must_use]
    pub fn with_native_tun_controller(
        mut self,
        native_tun_controller: Arc<dyn NativeTunController>,
    ) -> Self {
        self.native_tun_controller = native_tun_controller;
        self
    }

    #[must_use]
    pub const fn with_native_tun_health_interval(mut self, interval: Duration) -> Self {
        self.native_tun_health_interval = interval;
        self
    }

    #[must_use]
    pub fn with_event_sink(mut self, event_sink: Arc<dyn SupervisorEventSink>) -> Self {
        self.event_sink = event_sink;
        self
    }

    #[must_use]
    pub const fn with_target_os(mut self, target_os: TargetOs) -> Self {
        self.target_os = target_os;
        self
    }
}

#[derive(Clone)]
pub struct CoreSupervisor {
    tx: mpsc::Sender<SupervisorCommand>,
}

impl CoreSupervisor {
    /// Start the supervisor actor.
    ///
    /// The actor loop gets an OS thread of its own rather than a tokio task.
    /// Every handler is synchronous and blocks for as long as the OS takes:
    /// `ChildControl::stop` waits on the reaper thread's `kill`+`wait`, the
    /// sudo kill runs a launcher script that sleeps a second before polling,
    /// and the macOS PacketTunnel bridge waits up to 20 s for the provider to
    /// activate and again for it to connect. On a tokio worker that removed a
    /// worker from the pool for the whole call — a Tokio contract violation
    /// that, on a low-core machine with a second blocking call in flight, stalls
    /// event emission app-wide. Commands still queue behind a long Start/Stop
    /// because the actor is deliberately sequential; that is the actor model,
    /// not the defect this addresses.
    #[must_use]
    pub fn spawn(deps: SupervisorDeps) -> Self {
        let (tx, mut rx) = mpsc::channel(16);
        let supervisor = Self { tx: tx.clone() };
        let runtime = tokio::runtime::Handle::current();
        deps.runner
            .set_exit_handler(Some(Arc::new(SupervisorProcessExitHandler {
                tx: tx.downgrade(),
                runtime: runtime.clone(),
            })));
        // Only the returned `CoreSupervisor` holds a strong sender, so the loop
        // ends — and the actor's `Drop` stops the running core — as soon as the
        // last handle goes away.
        let actor_tx = tx.downgrade();
        drop(tx);
        if let Err(error) = std::thread::Builder::new()
            .name("core-supervisor".to_string())
            .spawn(move || {
                let mut actor = SupervisorActor::new(deps, actor_tx, Some(runtime));
                while let Some(command) = rx.blocking_recv() {
                    actor.handle(command);
                }
            })
        {
            tracing::error!(?error, "failed to start the core supervisor thread");
        }

        supervisor
    }

    pub async fn start(
        &self,
        request: SupervisorStartRequest,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(|reply| SupervisorCommand::Start(Box::new(request), reply))
            .await
    }

    pub async fn stop(&self) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(SupervisorCommand::Stop).await
    }

    pub async fn restart(
        &self,
        request: SupervisorStartRequest,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(|reply| SupervisorCommand::Restart(Box::new(request), reply))
            .await
    }

    pub async fn process_exited(
        &self,
        process_id: u32,
        exit_code: Option<i32>,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(|reply| SupervisorCommand::ProcessExited {
            process_id,
            exit_code,
            reply,
        })
        .await
    }

    pub async fn status(&self) -> Result<SupervisorSnapshot, SupervisorError> {
        self.request(SupervisorCommand::Status).await
    }

    async fn request<F>(&self, build: F) -> Result<SupervisorSnapshot, SupervisorError>
    where
        F: FnOnce(
            oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>,
        ) -> SupervisorCommand,
    {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(build(reply))
            .await
            .map_err(|_| SupervisorError::CommandChannelClosed)?;
        response
            .await
            .map_err(|_| SupervisorError::ResponseDropped)?
    }
}

struct SupervisorProcessExitHandler {
    tx: mpsc::WeakSender<SupervisorCommand>,
    runtime: tokio::runtime::Handle,
}

impl ProcessExitHandler for SupervisorProcessExitHandler {
    fn process_exited(&self, exit: ProcessExit) {
        let Some(tx) = self.tx.upgrade() else {
            return;
        };
        let supervisor = CoreSupervisor { tx };
        self.runtime.spawn(async move {
            if let Err(error) = supervisor
                .process_exited(exit.process_id, exit.exit_code)
                .await
            {
                tracing::warn!(
                    pid = exit.process_id,
                    role = ?exit.role,
                    ?error,
                    "failed to process core process exit"
                );
            }
        });
    }
}

enum SupervisorCommand {
    Start(
        Box<SupervisorStartRequest>,
        oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>,
    ),
    Stop(oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>),
    Restart(
        Box<SupervisorStartRequest>,
        oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>,
    ),
    Status(oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>),
    ProcessExited {
        process_id: u32,
        exit_code: Option<i32>,
        reply: oneshot::Sender<Result<SupervisorSnapshot, SupervisorError>>,
    },
    NativeTunExited {
        generation: u64,
        message: String,
    },
    DelayedRestart(Box<DelayedRestart>),
}

/// A crash restart waiting on its backoff timer.
///
/// `generation` is compared against the actor's current restart generation, so
/// a user Start/Stop/Restart issued while the timer runs cancels it.
struct DelayedRestart {
    generation: u64,
    attempt: u32,
    process_id: u32,
    exit_code: Option<i32>,
    request: SupervisorStartRequest,
}

struct SupervisorActor {
    deps: SupervisorDeps,
    tx: mpsc::WeakSender<SupervisorCommand>,
    /// The actor loop runs off the runtime, so background work it schedules
    /// (the crash backoff timer, the native TUN health watcher) needs an
    /// explicit handle. `None` in unit tests that drive the actor directly.
    runtime: Option<tokio::runtime::Handle>,
    running: RunningCore,
    native_tun_generation: u64,
    restart_generation: u64,
    crash: CrashTracker,
}

mod actor;
mod crash;

pub use crash::{
    CoreExitEvent, CoreExitGiveUp, CoreExitOutcome, CrashRestartPolicy, SupervisorClock,
    SystemSupervisorClock,
};

pub(crate) use crash::CrashTracker;

fn process_uses_unix_sudo(
    deps: &SupervisorDeps,
    spec: &CoreProcessSpec,
    tun_enabled: bool,
) -> bool {
    if supervisor_tun_backend(deps.target_os, tun_enabled) != TunBackend::Process {
        return false;
    }

    should_use_unix_sudo(
        deps.target_os,
        spec.core_type,
        tun_enabled,
        spec.may_need_sudo,
    )
}

fn supervisor_tun_backend(target_os: TargetOs, tun_enabled: bool) -> TunBackend {
    if tun_enabled {
        tun_backend(target_os)
    } else {
        TunBackend::Process
    }
}

fn native_tun_start_request(
    request: &SupervisorStartRequest,
    backend: TunBackend,
) -> Result<NativeTunStartRequest, SupervisorError> {
    let main_config_path =
        request
            .main
            .config_path
            .clone()
            .ok_or(SupervisorError::MissingNativeTunConfigPath {
                role: ProcessRole::Main,
            })?;
    let pre_config_path = request
        .pre
        .as_ref()
        .map(|pre| {
            pre.config_path
                .clone()
                .ok_or(SupervisorError::MissingNativeTunConfigPath {
                    role: ProcessRole::Pre,
                })
        })
        .transpose()?;

    Ok(NativeTunStartRequest {
        backend,
        active_profile_id: request.active_profile_id.clone(),
        main_launch: request.main.launch.clone(),
        pre_launch: request.pre.as_ref().map(|pre| pre.launch.clone()),
        main_config_path,
        pre_config_path,
    })
}

struct RunningCore {
    active_profile_id: Option<String>,
    main: Option<ProcessHandle>,
    pre: Option<ProcessHandle>,
    native_tun: Option<RunningNativeTun>,
    elevated: Vec<ProcessHandle>,
    job: Option<Box<dyn ProcessJob>>,
    last_request: Option<SupervisorStartRequest>,
    running_core_type: Option<CoreType>,
}

struct RunningNativeTun {
    backend: TunBackend,
    generation: u64,
}

impl RunningCore {
    fn empty() -> Self {
        Self {
            active_profile_id: None,
            main: None,
            pre: None,
            native_tun: None,
            elevated: Vec::new(),
            job: None,
            last_request: None,
            running_core_type: None,
        }
    }

    fn contains_pid(&self, process_id: u32) -> bool {
        self.main
            .as_ref()
            .is_some_and(|handle| handle.id() == process_id)
            || self
                .pre
                .as_ref()
                .is_some_and(|handle| handle.id() == process_id)
    }

    fn sudo_kill_target(&self, handle: &ProcessHandle) -> Option<&CoreProcessSpec> {
        let request = self.last_request.as_ref()?;
        if self
            .main
            .as_ref()
            .is_some_and(|main| main.id() == handle.id())
        {
            return Some(&request.main);
        }
        if self.pre.as_ref().is_some_and(|pre| pre.id() == handle.id()) {
            return request.pre.as_ref();
        }
        None
    }

    fn snapshot(&self) -> SupervisorSnapshot {
        let connected = self.main.is_some() || self.native_tun.is_some();
        // Port and token both describe a *live* Clash API, so a stale request
        // must not leak either of them once the core is gone.
        let live_request = connected.then_some(self.last_request.as_ref()).flatten();
        SupervisorSnapshot {
            state: if connected {
                SupervisorConnectionState::Connected
            } else {
                SupervisorConnectionState::Disconnected
            },
            active_profile_id: self.active_profile_id.clone(),
            main_pid: self.main.as_ref().map(ProcessHandle::id),
            pre_pid: self.pre.as_ref().map(ProcessHandle::id),
            running_core_type: self.running_core_type,
            clash_api_port: live_request.map(|request| request.clash_api_port),
            clash_api_secret: live_request.and_then(|request| request.clash_api_secret.clone()),
        }
    }
}

fn terminal_native_tun_message(status: &voya_platform::tun::NativeTunStatus) -> Option<String> {
    if !matches!(
        status.provider_state,
        NativeTunProviderState::Stopped
            | NativeTunProviderState::Error
            | NativeTunProviderState::PermissionRequired
            | NativeTunProviderState::MissingComponent
    ) {
        return None;
    }

    Some(status.message.clone().unwrap_or_else(|| {
        format!(
            "native TUN provider ended with state {:?}",
            status.provider_state
        )
    }))
}

fn ensure_sudo_kill_success(pid: u32, output: ProcessOutput) -> Result<(), SupervisorError> {
    if output.status_code == Some(0) {
        return Ok(());
    }

    Err(SupervisorError::SudoKillFailed {
        pid,
        status_code: output.status_code,
        stderr: sudo_kill_error_message(&output),
    })
}

fn sudo_kill_error_message(output: &ProcessOutput) -> String {
    let stderr = output.stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    let stdout = output.stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_string();
    }
    "sudo kill command failed".to_string()
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("supervisor command channel is closed")]
    CommandChannelClosed,
    #[error("supervisor response channel was dropped")]
    ResponseDropped,
    #[error("system authorization is required before spawning elevated {0:?}")]
    ElevationNotGranted(CoreType),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    TunCleanup(#[from] TunCleanupError),
    #[error(transparent)]
    NativeTun(#[from] NativeTunError),
    #[error("process job error: {0}")]
    Job(String),
    #[error("elevation error: {0}")]
    Elevation(String),
    #[error("sudo kill target pid {pid} does not match a tracked elevated process")]
    UnknownSudoKillTarget { pid: u32 },
    #[error("missing runtime config path for native TUN {role:?} process")]
    MissingNativeTunConfigPath { role: ProcessRole },
    #[error("sudo kill for pid {pid} failed with status {status_code:?}: {stderr}")]
    SudoKillFailed {
        pid: u32,
        status_code: Option<i32>,
        stderr: String,
    },
}

impl From<voya_platform::elevation::ElevationError> for SupervisorError {
    fn from(error: voya_platform::elevation::ElevationError) -> Self {
        Self::Elevation(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::fs;
    use std::{
        collections::BTreeMap,
        io,
        sync::{Mutex, MutexGuard},
        time::{Duration, Instant},
    };

    use voya_platform::process::{ProcessOutput, ProcessRunner};

    use super::*;

    #[derive(Clone, Default)]
    struct SharedEvents(Arc<Mutex<Vec<String>>>);

    impl SharedEvents {
        fn push(&self, event: impl Into<String>) {
            self.0.lock().expect("events").push(event.into());
        }

        fn lock(&self) -> MutexGuard<'_, Vec<String>> {
            self.0.lock().expect("events")
        }
    }

    #[derive(Clone)]
    struct FakeRunner {
        events: SharedEvents,
        fail_spawn_role: Arc<Mutex<Option<ProcessRole>>>,
        next_pid: Arc<Mutex<u32>>,
        oneshot_output: Arc<Mutex<ProcessOutput>>,
        oneshot_requests: Arc<Mutex<Vec<ProcessSpawn>>>,
    }

    impl FakeRunner {
        fn new(events: SharedEvents) -> Self {
            Self {
                events,
                fail_spawn_role: Arc::new(Mutex::new(None)),
                next_pid: Arc::new(Mutex::new(100)),
                oneshot_output: Arc::new(Mutex::new(ProcessOutput {
                    status_code: Some(0),
                    stdout: String::new(),
                    stderr: String::new(),
                })),
                oneshot_requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_fail_spawn_role(self, role: ProcessRole) -> Self {
            self.fail_spawn_role_from_now(role);
            self
        }

        /// Start failing spawns for `role` mid-test, so a respawn can fail
        /// after the first start succeeded.
        fn fail_spawn_role_from_now(&self, role: ProcessRole) {
            *self.fail_spawn_role.lock().expect("fail spawn role") = Some(role);
        }

        fn with_oneshot_output(self, output: ProcessOutput) -> Self {
            self.set_oneshot_output(output);
            self
        }

        /// Change what the launcher returns mid-test, so a sudo kill can start
        /// failing after the first start succeeded.
        fn set_oneshot_output(&self, output: ProcessOutput) {
            *self.oneshot_output.lock().expect("oneshot output") = output;
        }

        fn oneshot_requests(&self) -> Vec<ProcessSpawn> {
            self.oneshot_requests
                .lock()
                .expect("oneshot requests")
                .clone()
        }
    }

    impl ProcessRunner for FakeRunner {
        fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
            if self
                .fail_spawn_role
                .lock()
                .expect("fail spawn role")
                .is_some_and(|role| role == request.role)
            {
                self.events.push(format!("spawn-fail:{:?}", request.role));
                return Err(ProcessError::Spawn {
                    executable: request.executable,
                    source: io::Error::other("fake spawn failure"),
                });
            }

            let mut next_pid = self.next_pid.lock().expect("next pid");
            let pid = *next_pid;
            *next_pid += 1;
            self.events.push(format!(
                "spawn:{:?}:pid={pid}:stdin={}",
                request.role,
                request.has_stdin()
            ));
            Ok(ProcessHandle::new(pid, request.role))
        }

        fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
            self.events.push(format!(
                "oneshot:{:?}:stdin={}",
                request.role,
                request.has_stdin()
            ));
            self.oneshot_requests
                .lock()
                .expect("oneshot requests")
                .push(request);
            Ok(self.oneshot_output.lock().expect("oneshot output").clone())
        }

        fn stop(&self, handle: &ProcessHandle) -> Result<(), ProcessError> {
            self.events
                .push(format!("stop:{:?}:pid={}", handle.role(), handle.id()));
            Ok(())
        }
    }

    struct RecordingNativeTunController {
        events: SharedEvents,
    }

    impl NativeTunController for RecordingNativeTunController {
        fn status(&self, backend: TunBackend) -> voya_platform::tun::NativeTunStatus {
            voya_platform::tun::NativeTunStatus {
                backend,
                provider_state: NativeTunProviderState::Stopped,
                component_ready: true,
                message: None,
            }
        }

        fn start(&self, request: NativeTunStartRequest) -> Result<(), NativeTunError> {
            self.events.push(format!(
                "native:start:{:?}:main={}:pre={}",
                request.backend,
                request.main_config_path.display(),
                request.pre_config_path.is_some()
            ));
            Ok(())
        }

        fn stop(&self, backend: TunBackend) -> Result<(), NativeTunError> {
            self.events.push(format!("native:stop:{backend:?}"));
            Ok(())
        }
    }

    #[derive(Clone)]
    struct FlippableNativeTunController {
        events: SharedEvents,
        status: Arc<Mutex<voya_platform::tun::NativeTunStatus>>,
    }

    impl FlippableNativeTunController {
        fn new(events: SharedEvents, backend: TunBackend) -> Self {
            Self {
                events,
                status: Arc::new(Mutex::new(voya_platform::tun::NativeTunStatus {
                    backend,
                    provider_state: NativeTunProviderState::Running,
                    component_ready: true,
                    message: None,
                })),
            }
        }

        fn set_provider_state(
            &self,
            provider_state: NativeTunProviderState,
            message: Option<String>,
        ) {
            let mut status = self.status.lock().expect("native tun status");
            status.provider_state = provider_state;
            status.message = message;
        }
    }

    impl NativeTunController for FlippableNativeTunController {
        fn status(&self, _backend: TunBackend) -> voya_platform::tun::NativeTunStatus {
            self.status.lock().expect("native tun status").clone()
        }

        fn start(&self, request: NativeTunStartRequest) -> Result<(), NativeTunError> {
            self.events
                .push(format!("native:start:{:?}", request.backend));
            self.set_provider_state(NativeTunProviderState::Running, None);
            Ok(())
        }

        fn stop(&self, backend: TunBackend) -> Result<(), NativeTunError> {
            self.events.push(format!("native:stop:{backend:?}"));
            self.set_provider_state(NativeTunProviderState::Stopped, None);
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct RecordingSupervisorEventSink {
        events: Arc<Mutex<Vec<NativeTunExitEvent>>>,
        exits: Arc<Mutex<Vec<CoreExitEvent>>>,
    }

    impl RecordingSupervisorEventSink {
        fn events(&self) -> Vec<NativeTunExitEvent> {
            self.events.lock().expect("supervisor events").clone()
        }

        fn exits(&self) -> Vec<CoreExitEvent> {
            self.exits.lock().expect("core exit events").clone()
        }

        fn outcomes(&self) -> Vec<CoreExitOutcome> {
            self.exits()
                .into_iter()
                .map(|event| event.outcome)
                .collect()
        }
    }

    impl SupervisorEventSink for RecordingSupervisorEventSink {
        fn native_tun_exited(&self, event: NativeTunExitEvent) {
            self.events.lock().expect("supervisor events").push(event);
        }

        fn core_exited(&self, event: CoreExitEvent) {
            self.exits.lock().expect("core exit events").push(event);
        }
    }

    /// Advanceable clock so the crash window can be exercised without sleeping.
    #[derive(Clone)]
    struct FakeClock(Arc<Mutex<Instant>>);

    impl FakeClock {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(Instant::now())))
        }

        fn advance(&self, delta: Duration) {
            let mut now = self.0.lock().expect("fake clock");
            *now += delta;
        }
    }

    impl SupervisorClock for FakeClock {
        fn now(&self) -> Instant {
            *self.0.lock().expect("fake clock")
        }
    }

    struct RecordingJobFactory {
        events: SharedEvents,
    }

    impl ProcessJobFactory for RecordingJobFactory {
        fn create_job(&self) -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
            self.events.push("job:create");
            Ok(Some(Box::new(RecordingJob {
                events: self.events.clone(),
            })))
        }
    }

    struct RecordingJob {
        events: SharedEvents,
    }

    impl ProcessJob for RecordingJob {
        fn assign(&mut self, handle: &ProcessHandle) -> Result<(), ProcessError> {
            self.events.push(format!(
                "job:assign:{:?}:pid={}",
                handle.role(),
                handle.id()
            ));
            Ok(())
        }
    }

    fn launch(executable: &str, arguments: &str) -> CoreLaunch {
        CoreLaunch {
            executable: executable.into(),
            arguments: arguments.to_string(),
            working_dir: "/tmp/voya/binConfigs".into(),
            environment: BTreeMap::new(),
        }
    }

    fn supervisor_with(
        events: &SharedEvents,
        target_os: TargetOs,
        elevation: Arc<ElevationState>,
    ) -> CoreSupervisor {
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), elevation)
            .with_target_os(target_os);
        CoreSupervisor::spawn(deps)
    }

    #[tokio::test]
    async fn supervisor_stop_teardown_order_is_sudo_kill_main_pre() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let supervisor = supervisor_with(&events, TargetOs::Linux, elevation);

        let snapshot = supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box-main", "run -c config.json --disable-color"),
                ),
                pre: Some(CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box", "run -c pre.json --disable-color"),
                )),
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect("start");

        assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
        assert_eq!(snapshot.main_pid, Some(100));
        assert_eq!(snapshot.pre_pid, Some(101));

        supervisor.stop().await.expect("stop");
        let events = events.lock().clone();
        assert_eq!(
            &events[2..],
            [
                "oneshot:SudoKill:stdin=false",
                "oneshot:SudoKill:stdin=false",
                "stop:Main:pid=100",
                "stop:Pre:pid=101"
            ]
        );
    }

    #[tokio::test]
    async fn supervisor_sudo_kill_passes_expected_core_name_for_pid_validation() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let runner = Arc::new(FakeRunner::new(events.clone()));
        let deps = SupervisorDeps::new(runner.clone(), elevation).with_target_os(TargetOs::Linux);
        let supervisor = CoreSupervisor::spawn(deps);

        supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch(
                        "/tmp/voya cores/sing-box-client",
                        "run -c config.json --disable-color",
                    ),
                ),
                pre: None,
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect("start");
        supervisor.stop().await.expect("stop");

        let requests = runner.oneshot_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].executable, PathBuf::from("/usr/bin/sudo"));
        assert_eq!(
            requests[0].arguments,
            vec![
                "-n".to_string(),
                "--".to_string(),
                "/usr/libexec/voya-vpn/voya-elevate".to_string(),
                "kill".to_string(),
                "100".to_string(),
                "sing-box-client".to_string(),
            ]
        );
        // The kill logic now lives in the root-owned launcher, not a generated
        // user-owned script, and no admin password is piped in.
        assert!(requests[0].generated_scripts.is_empty());
        assert!(!requests[0].has_stdin());
    }

    #[tokio::test]
    async fn supervisor_sudo_kill_nonzero_status_is_typed_error() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let runner = Arc::new(
            FakeRunner::new(events.clone()).with_oneshot_output(ProcessOutput {
                status_code: Some(65),
                stdout: String::new(),
                stderr: "refusing to sudo kill pid 100".to_string(),
            }),
        );
        let deps = SupervisorDeps::new(runner, elevation).with_target_os(TargetOs::Linux);
        let supervisor = CoreSupervisor::spawn(deps);

        supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box", "run -c config.json --disable-color"),
                ),
                pre: None,
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect("start");

        let error = supervisor.stop().await.expect_err("sudo kill should fail");
        assert!(matches!(
            error,
            SupervisorError::SudoKillFailed {
                pid: 100,
                status_code: Some(65),
                ref stderr,
            } if stderr == "refusing to sudo kill pid 100"
        ));
        let snapshot = supervisor.status().await.expect("status after failed stop");
        assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
        assert_eq!(snapshot.main_pid, Some(100));
        assert_eq!(
            events.lock().as_slice(),
            [
                "spawn:Main:pid=100:stdin=false",
                "oneshot:SudoKill:stdin=false"
            ]
        );
    }

    #[test]
    fn supervisor_actor_drop_stops_running_core_with_sudo_kill() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), elevation)
            .with_target_os(TargetOs::Linux);

        {
            let (tx, _rx) = mpsc::channel(1);
            let mut actor = SupervisorActor::new(deps, tx.downgrade(), None);
            actor
                .start(SupervisorStartRequest {
                    active_profile_id: Some("active".to_string()),
                    main: CoreProcessSpec::new(
                        CoreType::sing_box,
                        launch("/tmp/sing-box", "run -c config.json --disable-color"),
                    ),
                    pre: None,
                    tun_enabled: true,
                    sudo_script_dir: "/tmp/voya/scripts".into(),
                    restart_on_crash: false,
                    clash_api_port: 0,
                    clash_api_secret: None,
                })
                .expect("start");
        }

        assert_eq!(
            events.lock().as_slice(),
            [
                "spawn:Main:pid=100:stdin=false",
                "oneshot:SudoKill:stdin=false",
                "stop:Main:pid=100"
            ]
        );
    }

    #[tokio::test]
    async fn supervisor_elevation_grant_gates_elevated_spawn() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        let supervisor = supervisor_with(&events, TargetOs::Linux, Arc::clone(&elevation));
        let request = SupervisorStartRequest {
            active_profile_id: Some("active".to_string()),
            main: CoreProcessSpec::new(
                CoreType::sing_box,
                launch("/tmp/sing-box", "run -c config.json --disable-color"),
            ),
            pre: None,
            tun_enabled: true,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: false,
            clash_api_port: 0,
            clash_api_secret: None,
        };

        let missing = supervisor
            .start(request.clone())
            .await
            .expect_err("ungranted elevation should fail");
        assert!(matches!(
            missing,
            SupervisorError::ElevationNotGranted(CoreType::sing_box)
        ));

        elevation.set_granted(true);
        supervisor
            .start(request)
            .await
            .expect("start with elevation grant");
        assert_eq!(events.lock().as_slice(), ["spawn:Main:pid=100:stdin=false"]);
    }

    #[tokio::test]
    async fn supervisor_crash_restarts_serialized_lifecycle() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        let supervisor = supervisor_with(&events, TargetOs::Linux, elevation);

        let request = SupervisorStartRequest {
            active_profile_id: Some("active".to_string()),
            main: CoreProcessSpec::new(
                CoreType::sing_box,
                launch("/tmp/sing-box", "run -c config.json --disable-color"),
            )
            .with_may_need_sudo(false),
            pre: None,
            tun_enabled: false,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: true,
            clash_api_port: 0,
            clash_api_secret: None,
        };

        let snapshot = supervisor.start(request).await.expect("start");
        supervisor
            .process_exited(snapshot.main_pid.expect("main pid"), Some(1))
            .await
            .expect("restart after crash");

        assert_eq!(
            events.lock().as_slice(),
            [
                "spawn:Main:pid=100:stdin=false",
                "stop:Main:pid=100",
                "spawn:Main:pid=101:stdin=false"
            ]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supervisor_reaper_callback_restarts_crashed_core() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = std::env::temp_dir().join(format!(
            "voya-supervisor-reaper-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let script = temp_dir.join("core.sh");
        let count_file = temp_dir.join("restart-count");
        fs::write(
            &script,
            r#"#!/bin/sh
count_file="$PWD/restart-count"
if [ -f "$count_file" ]; then
  count=$(cat "$count_file")
else
  count=0
fi
count=$((count + 1))
printf '%s\n' "$count" > "$count_file"
if [ "$count" -eq 1 ]; then
  exit 7
fi
sleep 30
"#,
        )
        .expect("write script");
        let mut permissions = fs::metadata(&script)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("chmod script");

        let supervisor = CoreSupervisor::spawn(
            SupervisorDeps::new(
                Arc::new(StdProcessRunner::new()),
                Arc::new(ElevationState::new()),
            )
            .with_target_os(TargetOs::Linux),
        );
        let snapshot = supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    CoreLaunch {
                        executable: script,
                        arguments: String::new(),
                        working_dir: temp_dir.clone(),
                        environment: BTreeMap::new(),
                    },
                )
                .with_display_log(false)
                .with_may_need_sudo(false),
                pre: None,
                tun_enabled: false,
                sudo_script_dir: temp_dir.join("scripts"),
                restart_on_crash: true,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect("start");
        let first_pid = snapshot.main_pid.expect("main pid");

        let restarted = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(contents) = fs::read_to_string(&count_file) {
                    if let Ok(count) = contents.trim().parse::<u32>() {
                        let snapshot = supervisor.status().await.expect("status");
                        if count >= 2 && snapshot.main_pid.is_some_and(|pid| pid != first_pid) {
                            break snapshot;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        let stop_result = supervisor.stop().await;
        let _ = fs::remove_dir_all(&temp_dir);
        let restarted = restarted.expect("restart observed");
        stop_result.expect("stop restarted child");

        assert_eq!(restarted.state, SupervisorConnectionState::Connected);
        assert_ne!(restarted.main_pid, Some(first_pid));
    }

    #[tokio::test]
    async fn supervisor_windows_non_tun_process_start_assigns_job() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), elevation)
            .with_target_os(TargetOs::Windows)
            .with_job_factory(Arc::new(RecordingJobFactory {
                events: events.clone(),
            }));
        let supervisor = CoreSupervisor::spawn(deps);

        supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box", "run -c config.json --disable-color"),
                ),
                pre: Some(CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box-pre", "run -c pre.json --disable-color"),
                )),
                tun_enabled: false,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect("start");

        assert_eq!(
            events.lock().as_slice(),
            [
                "job:create",
                "spawn:Main:pid=100:stdin=false",
                "job:assign:Main:pid=100",
                "spawn:Pre:pid=101:stdin=false",
                "job:assign:Pre:pid=101"
            ]
        );
    }

    #[tokio::test]
    async fn supervisor_windows_tun_uses_native_service_backend_without_process_spawn() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), elevation)
            .with_target_os(TargetOs::Windows)
            .with_native_tun_controller(Arc::new(RecordingNativeTunController {
                events: events.clone(),
            }));
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box", "run -c config.json --disable-color"),
                )
                .with_config_path("/tmp/voya/config.json"),
                pre: Some(
                    CoreProcessSpec::new(
                        CoreType::sing_box,
                        launch("/tmp/sing-box-pre", "run -c pre.json --disable-color"),
                    )
                    .with_config_path("/tmp/voya/pre.json"),
                ),
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect("native tun start");

        assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
        assert_eq!(snapshot.main_pid, None);
        assert_eq!(snapshot.pre_pid, None);

        supervisor.stop().await.expect("native tun stop");

        assert_eq!(
            events.lock().as_slice(),
            [
                "native:start:WindowsService:main=/tmp/voya/config.json:pre=true",
                "native:stop:WindowsService"
            ]
        );
    }

    #[tokio::test]
    async fn supervisor_native_tun_health_disconnects_once_without_restart() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        let controller = Arc::new(FlippableNativeTunController::new(
            events.clone(),
            TunBackend::WindowsService,
        ));
        let sink = RecordingSupervisorEventSink::default();
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), elevation)
            .with_target_os(TargetOs::Windows)
            .with_native_tun_controller(controller.clone())
            .with_native_tun_health_interval(Duration::from_millis(10))
            .with_event_sink(Arc::new(sink.clone()));
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box", "run -c config.json --disable-color"),
                )
                .with_config_path("/tmp/voya/config.json"),
                pre: None,
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: true,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect("native tun start");
        assert_eq!(snapshot.state, SupervisorConnectionState::Connected);

        controller.set_provider_state(
            NativeTunProviderState::Error,
            Some("PacketTunnel provider exited".to_string()),
        );

        for _ in 0..50 {
            if supervisor.status().await.expect("status").state
                == SupervisorConnectionState::Disconnected
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected
        );
        assert_eq!(
            events.lock().as_slice(),
            ["native:start:WindowsService", "native:stop:WindowsService"]
        );
        assert_eq!(
            sink.events(),
            [NativeTunExitEvent {
                active_profile_id: Some("active".to_string()),
                backend: TunBackend::WindowsService,
                message: "PacketTunnel provider exited".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn supervisor_tun_sudo_wraps_singbox() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let supervisor = supervisor_with(&events, TargetOs::Linux, elevation);

        supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("singbox".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box", "run -c config.json --disable-color"),
                ),
                pre: None,
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect("sing-box start");

        assert_eq!(
            events.lock().as_slice(),
            ["spawn:Main:pid=100:stdin=false",]
        );
    }

    #[tokio::test]
    async fn supervisor_tun_partial_start_failure_kills_elevated_main_before_returning() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let runner = FakeRunner::new(events.clone()).with_fail_spawn_role(ProcessRole::Pre);
        let deps = SupervisorDeps::new(Arc::new(runner), elevation).with_target_os(TargetOs::Linux);
        let supervisor = CoreSupervisor::spawn(deps);

        let error = supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box", "run -c config.json --disable-color"),
                ),
                pre: Some(CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box-pre", "run -c pre.json --disable-color"),
                )),
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
                clash_api_port: 0,
                clash_api_secret: None,
            })
            .await
            .expect_err("pre spawn failure");

        assert!(matches!(
            error,
            SupervisorError::Process(ProcessError::Spawn { .. })
        ));
        assert_eq!(
            events.lock().as_slice(),
            [
                "spawn:Main:pid=100:stdin=false",
                "spawn-fail:Pre",
                "oneshot:SudoKill:stdin=false",
                "stop:Main:pid=100"
            ]
        );
        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected
        );
    }

    fn crash_test_request() -> SupervisorStartRequest {
        SupervisorStartRequest {
            active_profile_id: Some("active".to_string()),
            main: CoreProcessSpec::new(
                CoreType::sing_box,
                launch("/tmp/sing-box", "run -c config.json --disable-color"),
            )
            .with_may_need_sudo(false),
            pre: None,
            tun_enabled: false,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: true,
            clash_api_port: 0,
            clash_api_secret: None,
        }
    }

    /// Crash restarts are immediate but bounded: without the budget a core that
    /// exits on every spawn (a bound listen port, a missing rule-set file) was
    /// respawned as fast as the reaper could report it, forever, while the
    /// snapshot kept saying Connected.
    #[tokio::test]
    async fn supervisor_gives_up_after_the_crash_budget_is_spent() {
        let events = SharedEvents::default();
        let sink = RecordingSupervisorEventSink::default();
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(events.clone())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Linux)
        .with_event_sink(Arc::new(sink.clone()))
        .with_crash_restart_policy(CrashRestartPolicy {
            max_restarts: 2,
            healthy_uptime: Duration::from_secs(30),
            initial_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        });
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor.start(crash_test_request()).await.expect("start");
        let mut pid = snapshot.main_pid.expect("main pid");
        for _ in 0..2 {
            pid = supervisor
                .process_exited(pid, Some(1))
                .await
                .expect("restart after crash")
                .main_pid
                .expect("restarted pid");
        }

        let final_snapshot = supervisor
            .process_exited(pid, Some(1))
            .await
            .expect("give up is not an error");

        assert_eq!(
            final_snapshot.state,
            SupervisorConnectionState::Disconnected
        );
        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected
        );
        assert_eq!(
            events.lock().as_slice(),
            [
                "spawn:Main:pid=100:stdin=false",
                "stop:Main:pid=100",
                "spawn:Main:pid=101:stdin=false",
                "stop:Main:pid=101",
                "spawn:Main:pid=102:stdin=false",
                "stop:Main:pid=102",
            ]
        );
        let outcomes = sink.outcomes();
        assert_eq!(outcomes.len(), 3);
        assert!(matches!(
            outcomes[0],
            CoreExitOutcome::Restarted { attempt: 1, .. }
        ));
        assert!(matches!(
            outcomes[1],
            CoreExitOutcome::Restarted { attempt: 2, .. }
        ));
        assert_eq!(
            outcomes[2],
            CoreExitOutcome::GaveUp(CoreExitGiveUp::CrashLoop { restarts: 2 })
        );
    }

    #[tokio::test]
    async fn supervisor_treats_a_clean_exit_as_intentional() {
        let events = SharedEvents::default();
        let sink = RecordingSupervisorEventSink::default();
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(events.clone())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Linux)
        .with_event_sink(Arc::new(sink.clone()));
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor.start(crash_test_request()).await.expect("start");
        supervisor
            .process_exited(snapshot.main_pid.expect("main pid"), Some(0))
            .await
            .expect("clean exit");

        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected
        );
        assert_eq!(
            events.lock().as_slice(),
            ["spawn:Main:pid=100:stdin=false", "stop:Main:pid=100"]
        );
        assert_eq!(
            sink.outcomes(),
            [CoreExitOutcome::GaveUp(CoreExitGiveUp::IntentionalExit)]
        );
    }

    #[tokio::test]
    async fn supervisor_reports_a_failed_respawn_instead_of_staying_silent() {
        let events = SharedEvents::default();
        let sink = RecordingSupervisorEventSink::default();
        let runner = Arc::new(FakeRunner::new(events.clone()));
        let deps = SupervisorDeps::new(
            Arc::clone(&runner) as Arc<dyn ProcessRunner>,
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Linux)
        .with_event_sink(Arc::new(sink.clone()));
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor.start(crash_test_request()).await.expect("start");
        runner.fail_spawn_role_from_now(ProcessRole::Main);

        let error = supervisor
            .process_exited(snapshot.main_pid.expect("main pid"), Some(1))
            .await
            .expect_err("the respawn fails");

        assert!(matches!(
            error,
            SupervisorError::Process(ProcessError::Spawn { .. })
        ));
        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected
        );
        let outcomes = sink.outcomes();
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(
            outcomes[0],
            CoreExitOutcome::GaveUp(CoreExitGiveUp::RestartFailed(_))
        ));
    }

    #[tokio::test]
    async fn supervisor_crash_after_a_healthy_run_opens_a_fresh_streak() {
        let events = SharedEvents::default();
        let sink = RecordingSupervisorEventSink::default();
        let clock = FakeClock::new();
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(events.clone())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Linux)
        .with_event_sink(Arc::new(sink.clone()))
        .with_clock(Arc::new(clock.clone()))
        .with_crash_restart_policy(CrashRestartPolicy {
            max_restarts: 1,
            healthy_uptime: Duration::from_secs(30),
            initial_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        });
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor.start(crash_test_request()).await.expect("start");
        let restarted = supervisor
            .process_exited(snapshot.main_pid.expect("main pid"), Some(1))
            .await
            .expect("first crash restarts");

        // The replacement stays up long enough to count as healthy, so the next
        // crash is a new problem rather than the tail of a restart storm.
        clock.advance(Duration::from_secs(120));
        let second = supervisor
            .process_exited(restarted.main_pid.expect("restarted pid"), Some(1))
            .await
            .expect("a crash after a healthy run restarts again");

        assert_eq!(second.state, SupervisorConnectionState::Connected);
        assert_eq!(
            sink.outcomes()
                .iter()
                .filter(|outcome| matches!(outcome, CoreExitOutcome::Restarted { attempt: 1, .. }))
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn supervisor_backs_off_before_the_second_restart_of_a_streak() {
        let events = SharedEvents::default();
        let sink = RecordingSupervisorEventSink::default();
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(events.clone())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Linux)
        .with_event_sink(Arc::new(sink.clone()))
        .with_crash_restart_policy(CrashRestartPolicy {
            max_restarts: 3,
            healthy_uptime: Duration::from_secs(30),
            initial_delay: Duration::from_millis(20),
            max_delay: Duration::from_millis(20),
        });
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor.start(crash_test_request()).await.expect("start");
        let restarted = supervisor
            .process_exited(snapshot.main_pid.expect("main pid"), Some(1))
            .await
            .expect("first crash restarts immediately");

        let scheduled = supervisor
            .process_exited(restarted.main_pid.expect("restarted pid"), Some(1))
            .await
            .expect("second crash is deferred");
        assert_eq!(scheduled.state, SupervisorConnectionState::Disconnected);
        assert!(matches!(
            sink.outcomes().last(),
            Some(CoreExitOutcome::RestartScheduled { attempt: 2, .. })
        ));

        for _ in 0..50 {
            if supervisor.status().await.expect("status").state
                == SupervisorConnectionState::Connected
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Connected
        );
        assert!(matches!(
            sink.outcomes().last(),
            Some(CoreExitOutcome::Restarted { attempt: 2, .. })
        ));
    }

    #[tokio::test]
    async fn supervisor_stop_cancels_a_pending_crash_restart() {
        let events = SharedEvents::default();
        let sink = RecordingSupervisorEventSink::default();
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(events.clone())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Linux)
        .with_event_sink(Arc::new(sink.clone()))
        .with_crash_restart_policy(CrashRestartPolicy {
            max_restarts: 3,
            healthy_uptime: Duration::from_secs(30),
            initial_delay: Duration::from_millis(80),
            max_delay: Duration::from_millis(80),
        });
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor.start(crash_test_request()).await.expect("start");
        let restarted = supervisor
            .process_exited(snapshot.main_pid.expect("main pid"), Some(1))
            .await
            .expect("first crash restarts immediately");
        supervisor
            .process_exited(restarted.main_pid.expect("restarted pid"), Some(1))
            .await
            .expect("second crash is deferred");

        supervisor.stop().await.expect("user disconnect");
        tokio::time::sleep(Duration::from_millis(250)).await;

        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected
        );
        assert_eq!(
            events
                .lock()
                .iter()
                .filter(|event| event.starts_with("spawn:Main"))
                .count(),
            2,
            "a user disconnect must cancel the restart waiting on its timer"
        );
    }

    /// A start whose preconditions cannot be met must not take the healthy core
    /// down with it: everything that does not depend on the old core being gone
    /// is resolved before `stop`.
    #[tokio::test]
    async fn supervisor_start_precondition_failure_keeps_the_running_core() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let supervisor = supervisor_with(&events, TargetOs::Linux, Arc::clone(&elevation));
        let request = SupervisorStartRequest {
            active_profile_id: Some("active".to_string()),
            main: CoreProcessSpec::new(
                CoreType::sing_box,
                launch("/tmp/sing-box", "run -c config.json --disable-color"),
            ),
            pre: None,
            tun_enabled: true,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: false,
            clash_api_port: 0,
            clash_api_secret: None,
        };

        supervisor.start(request.clone()).await.expect("start");
        elevation.set_granted(false);

        let error = supervisor
            .start(request)
            .await
            .expect_err("the elevation grant is gone");

        assert!(matches!(
            error,
            SupervisorError::ElevationNotGranted(CoreType::sing_box)
        ));
        let snapshot = supervisor.status().await.expect("status");
        assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
        assert_eq!(snapshot.main_pid, Some(100));
        assert_eq!(events.lock().as_slice(), ["spawn:Main:pid=100:stdin=false"]);
    }

    #[tokio::test]
    async fn supervisor_start_failure_after_stop_reports_disconnected() {
        let events = SharedEvents::default();
        let runner = Arc::new(FakeRunner::new(events.clone()));
        let deps = SupervisorDeps::new(
            Arc::clone(&runner) as Arc<dyn ProcessRunner>,
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Linux);
        let supervisor = CoreSupervisor::spawn(deps);

        supervisor.start(crash_test_request()).await.expect("start");
        runner.fail_spawn_role_from_now(ProcessRole::Main);

        supervisor
            .start(crash_test_request())
            .await
            .expect_err("the replacement cannot spawn");

        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected,
            "the old core was stopped, so the shell must be told the core is down"
        );
    }
    /// `Restart` is not a distinct state machine — it is `Start`, which stops
    /// the running core itself — but nothing asserted that ordering.
    #[tokio::test]
    async fn supervisor_restart_stops_the_previous_core_before_spawning_a_new_one() {
        let events = SharedEvents::default();
        let supervisor = supervisor_with(&events, TargetOs::Linux, Arc::new(ElevationState::new()));

        let first = supervisor.start(crash_test_request()).await.expect("start");
        let second = supervisor
            .restart(crash_test_request())
            .await
            .expect("restart");

        assert_eq!(second.state, SupervisorConnectionState::Connected);
        assert_ne!(first.main_pid, second.main_pid);
        assert_eq!(
            events.lock().as_slice(),
            [
                "spawn:Main:pid=100:stdin=false",
                "stop:Main:pid=100",
                "spawn:Main:pid=101:stdin=false"
            ]
        );
    }

    /// Each native TUN start spawns its own health watcher, and the watcher from
    /// the superseded start is still alive after a restart: it observes the
    /// `Stopped` the restart itself caused. The generation guard is what stops
    /// that from tearing the freshly started provider down again.
    #[tokio::test]
    async fn supervisor_ignores_the_health_watcher_of_a_superseded_native_tun_start() {
        let events = SharedEvents::default();
        let controller = Arc::new(FlippableNativeTunController::new(
            events.clone(),
            TunBackend::WindowsService,
        ));
        let sink = RecordingSupervisorEventSink::default();
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(events.clone())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Windows)
        .with_native_tun_controller(controller.clone())
        .with_native_tun_health_interval(Duration::from_millis(10))
        .with_event_sink(Arc::new(sink.clone()));
        let supervisor = CoreSupervisor::spawn(deps);

        supervisor
            .start(native_tun_test_request())
            .await
            .expect("first native tun start");
        let restarted = supervisor
            .restart(native_tun_test_request())
            .await
            .expect("native tun restart");
        assert_eq!(restarted.state, SupervisorConnectionState::Connected);

        controller.set_provider_state(
            NativeTunProviderState::Error,
            Some("provider exited".to_string()),
        );
        for _ in 0..50 {
            if supervisor.status().await.expect("status").state
                == SupervisorConnectionState::Disconnected
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // Give the watcher from the first generation several more ticks to
        // deliver its own terminal report.
        tokio::time::sleep(Duration::from_millis(60)).await;

        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected
        );
        let reported = sink.events();
        assert_eq!(
            reported.len(),
            1,
            "only the current generation may report the provider exit: {reported:?}"
        );
        assert_eq!(reported[0].message, "provider exited");
    }

    /// The pre-process is tracked for crash restarts too: when it dies the whole
    /// pair is replaced, not just the main core.
    #[tokio::test]
    async fn supervisor_restarts_both_processes_when_the_pre_process_crashes() {
        let events = SharedEvents::default();
        let sink = RecordingSupervisorEventSink::default();
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(events.clone())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Linux)
        .with_event_sink(Arc::new(sink.clone()));
        let supervisor = CoreSupervisor::spawn(deps);

        let snapshot = supervisor
            .start(SupervisorStartRequest {
                pre: Some(
                    CoreProcessSpec::new(
                        CoreType::sing_box,
                        launch("/tmp/sing-box-pre", "run -c pre.json --disable-color"),
                    )
                    .with_may_need_sudo(false),
                ),
                ..crash_test_request()
            })
            .await
            .expect("start");

        let restarted = supervisor
            .process_exited(snapshot.pre_pid.expect("pre pid"), Some(1))
            .await
            .expect("a pre-process crash restarts the pair");

        assert_eq!(restarted.state, SupervisorConnectionState::Connected);
        assert_ne!(restarted.main_pid, snapshot.main_pid);
        assert_ne!(restarted.pre_pid, snapshot.pre_pid);
        assert!(matches!(
            sink.outcomes().last(),
            Some(CoreExitOutcome::Restarted { attempt: 1, .. })
        ));
        assert_eq!(
            events.lock().as_slice(),
            [
                "spawn:Main:pid=100:stdin=false",
                "spawn:Pre:pid=101:stdin=false",
                "stop:Main:pid=100",
                "stop:Pre:pid=101",
                "spawn:Main:pid=102:stdin=false",
                "spawn:Pre:pid=103:stdin=false"
            ]
        );
    }

    /// A start begins by stopping the previous core. If that stop fails — the
    /// launcher could not kill an elevated core — the old core is still running,
    /// so the start must abort and leave the tracked pids alone rather than
    /// spawn a second core over the top of it.
    #[tokio::test]
    async fn supervisor_start_keeps_the_running_core_when_the_sudo_kill_fails() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let runner = Arc::new(FakeRunner::new(events.clone()));
        let deps = SupervisorDeps::new(
            Arc::clone(&runner) as Arc<dyn ProcessRunner>,
            Arc::clone(&elevation),
        )
        .with_target_os(TargetOs::Linux);
        let supervisor = CoreSupervisor::spawn(deps);
        let request = SupervisorStartRequest {
            active_profile_id: Some("active".to_string()),
            main: CoreProcessSpec::new(
                CoreType::sing_box,
                launch("/tmp/sing-box", "run -c config.json --disable-color"),
            ),
            pre: None,
            tun_enabled: true,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: false,
            clash_api_port: 0,
            clash_api_secret: None,
        };

        let first = supervisor.start(request.clone()).await.expect("start");
        runner.set_oneshot_output(ProcessOutput {
            status_code: Some(1),
            stdout: String::new(),
            stderr: "kill: operation not permitted".to_string(),
        });

        let error = supervisor
            .start(request)
            .await
            .expect_err("the elevated core could not be stopped");

        assert!(matches!(error, SupervisorError::SudoKillFailed { .. }));
        let snapshot = supervisor.status().await.expect("status");
        assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
        assert_eq!(snapshot.main_pid, first.main_pid);
        assert_eq!(
            events
                .lock()
                .iter()
                .filter(|event| event.starts_with("spawn:Main"))
                .count(),
            1,
            "a failed teardown must not leave two cores running"
        );
    }

    /// A token that reaches a tracing field ends up in the rolling log file and
    /// in every bug report attached to it, which is the same exposure the
    /// secret exists to close.
    #[test]
    fn supervisor_clash_api_secret_never_appears_in_debug_output() {
        let secret = ClashApiSecret::generate();
        let request = SupervisorStartRequest {
            clash_api_secret: Some(secret.clone()),
            ..crash_test_request()
        };
        let snapshot = SupervisorSnapshot {
            clash_api_secret: Some(secret.clone()),
            ..SupervisorSnapshot::disconnected()
        };

        for rendered in [format!("{request:?}"), format!("{snapshot:?}")] {
            assert!(!rendered.contains(secret.as_str()), "{rendered}");
            assert!(rendered.contains("<redacted>"), "{rendered}");
        }
        assert_eq!(secret.as_str().len(), 64, "two v4 UUIDs of hex digits");
    }

    #[tokio::test]
    async fn supervisor_reports_the_clash_api_secret_only_while_connected() {
        let events = SharedEvents::default();
        let supervisor = supervisor_with(&events, TargetOs::Linux, Arc::new(ElevationState::new()));
        let secret = ClashApiSecret::generate();

        let connected = supervisor
            .start(SupervisorStartRequest {
                clash_api_port: 9_190,
                clash_api_secret: Some(secret.clone()),
                ..crash_test_request()
            })
            .await
            .expect("start");
        assert_eq!(
            connected.clash_api_access(),
            ClashApiAccess::new(Some(9_190), Some(secret))
        );

        let disconnected = supervisor.stop().await.expect("stop");

        assert_eq!(
            disconnected.clash_api_access(),
            ClashApiAccess::default(),
            "a stale token must not outlive the core that accepted it"
        );
    }

    fn native_tun_test_request() -> SupervisorStartRequest {
        SupervisorStartRequest {
            active_profile_id: Some("active".to_string()),
            main: CoreProcessSpec::new(
                CoreType::sing_box,
                launch("/tmp/sing-box", "run -c config.json --disable-color"),
            )
            .with_config_path("/tmp/voya/config.json"),
            pre: None,
            tun_enabled: true,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: true,
            clash_api_port: 0,
            clash_api_secret: None,
        }
    }
}
