use std::{path::PathBuf, sync::Arc};

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
    tun::{NoopTunCleaner, PlatformTunCleaner, TunCleaner, TunCleanupError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreProcessSpec {
    pub core_type: CoreType,
    pub launch: CoreLaunch,
    pub display_log: bool,
    pub may_need_sudo: bool,
}

impl CoreProcessSpec {
    #[must_use]
    pub const fn new(core_type: CoreType, launch: CoreLaunch) -> Self {
        Self {
            core_type,
            launch,
            display_log: true,
            may_need_sudo: true,
        }
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
        }
    }
}

#[derive(Clone)]
pub struct SupervisorDeps {
    pub runner: Arc<dyn ProcessRunner>,
    pub elevation: Arc<ElevationState>,
    pub job_factory: Arc<dyn ProcessJobFactory>,
    pub tun_cleaner: Arc<dyn TunCleaner>,
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
            target_os: TargetOs::current(),
        }
    }

    #[must_use]
    pub fn platform() -> Self {
        Self {
            runner: Arc::new(StdProcessRunner::new()),
            elevation: Arc::new(ElevationState::new()),
            job_factory: Arc::new(PlatformProcessJobFactory),
            tun_cleaner: Arc::new(PlatformTunCleaner),
            target_os: TargetOs::current(),
        }
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
            target_os: TargetOs::current(),
        }
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
    #[must_use]
    pub fn spawn(deps: SupervisorDeps) -> Self {
        let (tx, mut rx) = mpsc::channel(16);
        let supervisor = Self { tx: tx.clone() };
        deps.runner
            .set_exit_handler(Some(Arc::new(SupervisorProcessExitHandler {
                tx: tx.downgrade(),
                runtime: tokio::runtime::Handle::current(),
            })));
        tokio::spawn(async move {
            let mut actor = SupervisorActor::new(deps);
            while let Some(command) = rx.recv().await {
                actor.handle(command);
            }
        });

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
}

struct SupervisorActor {
    deps: SupervisorDeps,
    running: RunningCore,
}

impl SupervisorActor {
    fn new(deps: SupervisorDeps) -> Self {
        Self {
            deps,
            running: RunningCore::empty(),
        }
    }

    fn handle(&mut self, command: SupervisorCommand) {
        match command {
            SupervisorCommand::Start(request, reply) => {
                let _ = reply.send(self.start(*request));
            }
            SupervisorCommand::Stop(reply) => {
                let _ = reply.send(self.stop());
            }
            SupervisorCommand::Restart(request, reply) => {
                let _ = reply.send(self.stop().and_then(|_| self.start(*request)));
            }
            SupervisorCommand::Status(reply) => {
                let _ = reply.send(Ok(self.running.snapshot()));
            }
            SupervisorCommand::ProcessExited {
                process_id,
                exit_code,
                reply,
            } => {
                let _ = reply.send(self.process_exited(process_id, exit_code));
            }
        }
    }

    fn start(
        &mut self,
        request: SupervisorStartRequest,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        self.stop()?;

        if self.deps.target_os == TargetOs::Windows && request.tun_enabled {
            self.deps.tun_cleaner.cleanup_before_start()?;
        }

        let job = if self.deps.target_os == TargetOs::Windows {
            self.deps.job_factory.create_job()?
        } else {
            None
        };

        let mut partial = RunningCore {
            active_profile_id: request.active_profile_id.clone(),
            main: None,
            pre: None,
            elevated: Vec::new(),
            job,
            last_request: Some(request.clone()),
            running_core_type: Some(request.main.core_type),
        };

        let main = self.spawn_process(ProcessRole::Main, &request.main, &request)?;
        partial.main = Some(main.clone());
        if process_uses_unix_sudo(&self.deps, &request.main, request.tun_enabled) {
            partial.elevated.push(main.clone());
        }
        if let Some(job) = partial.job.as_mut() {
            if let Err(error) = job.assign(&main) {
                return self.cleanup_partial_start(partial, SupervisorError::from(error));
            }
        }

        if let Some(pre_spec) = &request.pre {
            let pre = match self.spawn_process(ProcessRole::Pre, pre_spec, &request) {
                Ok(pre) => pre,
                Err(error) => return self.cleanup_partial_start(partial, error),
            };
            partial.pre = Some(pre.clone());
            if process_uses_unix_sudo(&self.deps, pre_spec, request.tun_enabled) {
                partial.elevated.push(pre.clone());
            }
            if let Some(job) = partial.job.as_mut() {
                if let Err(error) = job.assign(&pre) {
                    return self.cleanup_partial_start(partial, SupervisorError::from(error));
                }
            }
        }

        let running_core_type = request
            .pre
            .as_ref()
            .map_or(request.main.core_type, |pre| pre.core_type);
        partial.running_core_type = Some(running_core_type);

        self.running = partial;

        Ok(self.running.snapshot())
    }

    fn stop(&mut self) -> Result<SupervisorSnapshot, SupervisorError> {
        let running = std::mem::replace(&mut self.running, RunningCore::empty());

        match self.stop_running(&running) {
            Ok(()) => Ok(SupervisorSnapshot::disconnected()),
            Err(error) => {
                self.running = running;
                Err(error)
            }
        }
    }

    fn stop_running(&self, running: &RunningCore) -> Result<(), SupervisorError> {
        let mut first_error = None;

        for handle in &running.elevated {
            self.sudo_kill(handle, running)?;
        }

        if let Some(main) = &running.main {
            if let Err(error) = self.deps.runner.stop(main) {
                first_error.get_or_insert(SupervisorError::from(error));
            }
        }

        if let Some(pre) = &running.pre {
            if let Err(error) = self.deps.runner.stop(pre) {
                first_error.get_or_insert(SupervisorError::from(error));
            }
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn cleanup_partial_start(
        &self,
        running: RunningCore,
        start_error: SupervisorError,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        match self.stop_running(&running) {
            Ok(()) => Err(start_error),
            Err(cleanup_error) => Err(cleanup_error),
        }
    }

    fn process_exited(
        &mut self,
        process_id: u32,
        _exit_code: Option<i32>,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        if !self.running.contains_pid(process_id) {
            return Ok(self.running.snapshot());
        }

        let restart = self
            .running
            .last_request
            .clone()
            .filter(|request| request.restart_on_crash);
        self.stop()?;

        if let Some(request) = restart {
            self.start(request)
        } else {
            Ok(SupervisorSnapshot::disconnected())
        }
    }

    fn spawn_process(
        &self,
        role: ProcessRole,
        spec: &CoreProcessSpec,
        request: &SupervisorStartRequest,
    ) -> Result<ProcessHandle, SupervisorError> {
        let mut spawn = ProcessSpawn::from_core_launch(role, &spec.launch, spec.display_log)?;

        if process_uses_unix_sudo(&self.deps, spec, request.tun_enabled) {
            let launcher = self.elevation_launcher(spec.core_type)?;
            spawn = wrap_spawn_with_unix_sudo_passwordless(spawn, &launcher);
        }

        let handle = self.deps.runner.spawn(spawn)?;
        Ok(handle)
    }

    fn sudo_kill(
        &self,
        handle: &ProcessHandle,
        running: &RunningCore,
    ) -> Result<(), SupervisorError> {
        if running.last_request.is_none() {
            return Ok(());
        }
        let target = running
            .sudo_kill_target(handle)
            .ok_or(SupervisorError::UnknownSudoKillTarget { pid: handle.id() })?;
        let launcher = self.elevation_launcher(target.core_type)?;
        let spawn = unix_sudo_kill_spawn_passwordless(
            self.deps.target_os,
            &launcher,
            handle.id(),
            &target.launch.executable,
            target.launch.working_dir.clone(),
        )?;
        let output = self.deps.runner.run_oneshot(spawn)?;
        ensure_sudo_kill_success(handle.id(), output)
    }

    /// Resolve the root-owned elevation launcher, requiring an active grant.
    fn elevation_launcher(&self, core_type: CoreType) -> Result<PathBuf, SupervisorError> {
        if !self.deps.elevation.is_granted() {
            return Err(SupervisorError::ElevationNotGranted(core_type));
        }
        elevate_launcher_path(self.deps.target_os)
            .ok_or(SupervisorError::ElevationNotGranted(core_type))
    }
}

impl Drop for SupervisorActor {
    fn drop(&mut self) {
        let running = std::mem::replace(&mut self.running, RunningCore::empty());
        if let Err(error) = self.stop_running(&running) {
            tracing::warn!(?error, "failed to stop core supervisor during actor drop");
        }
    }
}

fn process_uses_unix_sudo(
    deps: &SupervisorDeps,
    spec: &CoreProcessSpec,
    tun_enabled: bool,
) -> bool {
    should_use_unix_sudo(
        deps.target_os,
        spec.core_type,
        tun_enabled,
        spec.may_need_sudo,
    )
}

struct RunningCore {
    active_profile_id: Option<String>,
    main: Option<ProcessHandle>,
    pre: Option<ProcessHandle>,
    elevated: Vec<ProcessHandle>,
    job: Option<Box<dyn ProcessJob>>,
    last_request: Option<SupervisorStartRequest>,
    running_core_type: Option<CoreType>,
}

impl RunningCore {
    fn empty() -> Self {
        Self {
            active_profile_id: None,
            main: None,
            pre: None,
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
        let connected = self.main.is_some();
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
        }
    }
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
    #[error("process job error: {0}")]
    Job(String),
    #[error("elevation error: {0}")]
    Elevation(String),
    #[error("sudo kill target pid {pid} does not match a tracked elevated process")]
    UnknownSudoKillTarget { pid: u32 },
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
    use std::{
        collections::BTreeMap,
        io,
        sync::{Mutex, MutexGuard},
    };
    #[cfg(unix)]
    use std::{fs, time::Duration};

    use voya_platform::{
        process::{ProcessOutput, ProcessRunner},
        tun::TunCleanupError,
    };

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
            *self.fail_spawn_role.lock().expect("fail spawn role") = Some(role);
            self
        }

        fn with_oneshot_output(self, output: ProcessOutput) -> Self {
            *self.oneshot_output.lock().expect("oneshot output") = output;
            self
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

    struct RecordingTunCleaner {
        events: SharedEvents,
    }

    impl TunCleaner for RecordingTunCleaner {
        fn cleanup_before_start(&self) -> Result<(), TunCleanupError> {
            self.events.push("tun:cleanup");
            Ok(())
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
        let supervisor = supervisor_with(&events, TargetOs::Macos, elevation);

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
            let mut actor = SupervisorActor::new(deps);
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
    async fn supervisor_windows_tun_cleanup_runs_before_process_start_and_assigns_job() {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), elevation)
            .with_target_os(TargetOs::Windows)
            .with_tun_cleaner(Arc::new(RecordingTunCleaner {
                events: events.clone(),
            }))
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
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
            })
            .await
            .expect("start");

        assert_eq!(
            events.lock().as_slice(),
            [
                "tun:cleanup",
                "job:create",
                "spawn:Main:pid=100:stdin=false",
                "job:assign:Main:pid=100",
                "spawn:Pre:pid=101:stdin=false",
                "job:assign:Pre:pid=101"
            ]
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
}
