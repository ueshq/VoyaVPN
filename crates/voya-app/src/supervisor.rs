use std::{path::PathBuf, sync::Arc};

use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use voya_core::CoreType;
use voya_platform::{
    coreinfo::{CoreLaunch, TargetOs},
    elevation::{
        should_use_unix_sudo, unix_sudo_kill_spawn, wrap_spawn_with_unix_sudo, SudoPasswordError,
        SudoPasswordStore,
    },
    process::{
        NoopProcessJobFactory, PlatformProcessJobFactory, ProcessError, ProcessHandle, ProcessJob,
        ProcessJobFactory, ProcessOutput, ProcessRole, ProcessRunner, ProcessSpawn,
        StdProcessRunner,
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
    pub sudo_passwords: Arc<SudoPasswordStore>,
    pub job_factory: Arc<dyn ProcessJobFactory>,
    pub tun_cleaner: Arc<dyn TunCleaner>,
    pub target_os: TargetOs,
}

impl SupervisorDeps {
    #[must_use]
    pub fn new(runner: Arc<dyn ProcessRunner>, sudo_passwords: Arc<SudoPasswordStore>) -> Self {
        Self {
            runner,
            sudo_passwords,
            job_factory: Arc::new(NoopProcessJobFactory),
            tun_cleaner: Arc::new(NoopTunCleaner),
            target_os: TargetOs::current(),
        }
    }

    #[must_use]
    pub fn platform() -> Self {
        Self {
            runner: Arc::new(StdProcessRunner::new()),
            sudo_passwords: Arc::new(SudoPasswordStore::new()),
            job_factory: Arc::new(PlatformProcessJobFactory),
            tun_cleaner: Arc::new(PlatformTunCleaner),
            target_os: TargetOs::current(),
        }
    }

    #[must_use]
    pub fn platform_with_runner(
        runner: Arc<dyn ProcessRunner>,
        sudo_passwords: Arc<SudoPasswordStore>,
    ) -> Self {
        Self {
            runner,
            sudo_passwords,
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
        tokio::spawn(async move {
            let mut actor = SupervisorActor::new(deps);
            while let Some(command) = rx.recv().await {
                actor.handle(command);
            }
        });

        Self { tx }
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

        self.stop_running(running)
    }

    fn stop_running(&self, running: RunningCore) -> Result<SupervisorSnapshot, SupervisorError> {
        let mut first_error = None;

        for handle in &running.elevated {
            if let Err(error) = self.sudo_kill(handle, &running) {
                first_error.get_or_insert(error);
            }
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

        drop(running.job);

        match first_error {
            Some(error) => Err(error),
            None => Ok(SupervisorSnapshot::disconnected()),
        }
    }

    fn cleanup_partial_start(
        &self,
        running: RunningCore,
        start_error: SupervisorError,
    ) -> Result<SupervisorSnapshot, SupervisorError> {
        match self.stop_running(running) {
            Ok(_) => Err(start_error),
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
            let password = self
                .deps
                .sudo_passwords
                .read_password()?
                .ok_or(SupervisorError::MissingSudoPassword(spec.core_type))?;
            spawn = wrap_spawn_with_unix_sudo(spawn, &request.sudo_script_dir, password);
        }

        let handle = self.deps.runner.spawn(spawn)?;
        Ok(handle)
    }

    fn sudo_kill(
        &self,
        handle: &ProcessHandle,
        running: &RunningCore,
    ) -> Result<ProcessOutput, SupervisorError> {
        let Some(request) = &running.last_request else {
            return Ok(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            });
        };
        let password = self.deps.sudo_passwords.read_password()?.ok_or(
            SupervisorError::MissingSudoPassword(
                running.running_core_type.unwrap_or(CoreType::sing_box),
            ),
        )?;
        let spawn = unix_sudo_kill_spawn(
            self.deps.target_os,
            &request.sudo_script_dir,
            handle.id(),
            request.main.launch.working_dir.clone(),
            password,
        )?;
        self.deps.runner.run_oneshot(spawn).map_err(Into::into)
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

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("supervisor command channel is closed")]
    CommandChannelClosed,
    #[error("supervisor response channel was dropped")]
    ResponseDropped,
    #[error("sudo password is required before spawning elevated {0:?}")]
    MissingSudoPassword(CoreType),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    SudoPassword(#[from] SudoPasswordError),
    #[error(transparent)]
    TunCleanup(#[from] TunCleanupError),
    #[error("process job error: {0}")]
    Job(String),
    #[error("elevation error: {0}")]
    Elevation(String),
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
    }

    impl FakeRunner {
        fn new(events: SharedEvents) -> Self {
            Self {
                events,
                fail_spawn_role: Arc::new(Mutex::new(None)),
                next_pid: Arc::new(Mutex::new(100)),
            }
        }

        fn with_fail_spawn_role(self, role: ProcessRole) -> Self {
            *self.fail_spawn_role.lock().expect("fail spawn role") = Some(role);
            self
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
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
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
        sudo_passwords: Arc<SudoPasswordStore>,
    ) -> CoreSupervisor {
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), sudo_passwords)
            .with_target_os(target_os);
        CoreSupervisor::spawn(deps)
    }

    #[tokio::test]
    async fn supervisor_stop_teardown_order_is_sudo_kill_main_pre() {
        let events = SharedEvents::default();
        let sudo_passwords = Arc::new(SudoPasswordStore::new());
        sudo_passwords.set_password("pw").expect("sudo password");
        let supervisor = supervisor_with(&events, TargetOs::Macos, sudo_passwords);

        let snapshot = supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::Xray,
                    launch("/tmp/xray", "run -c config.json"),
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
                "oneshot:SudoKill:stdin=true",
                "stop:Main:pid=100",
                "stop:Pre:pid=101"
            ]
        );
    }

    #[tokio::test]
    async fn supervisor_sudo_password_is_read_synchronously_at_spawn() {
        let events = SharedEvents::default();
        let sudo_passwords = Arc::new(SudoPasswordStore::new());
        let supervisor = supervisor_with(&events, TargetOs::Linux, Arc::clone(&sudo_passwords));
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
            .expect_err("missing sudo password should fail");
        assert!(matches!(
            missing,
            SupervisorError::MissingSudoPassword(CoreType::sing_box)
        ));

        sudo_passwords.set_password("pw").expect("sudo password");
        supervisor
            .start(request)
            .await
            .expect("start with password");
        assert_eq!(events.lock().as_slice(), ["spawn:Main:pid=100:stdin=true"]);
    }

    #[tokio::test]
    async fn supervisor_crash_restarts_serialized_lifecycle() {
        let events = SharedEvents::default();
        let sudo_passwords = Arc::new(SudoPasswordStore::new());
        let supervisor = supervisor_with(&events, TargetOs::Linux, sudo_passwords);

        let request = SupervisorStartRequest {
            active_profile_id: Some("active".to_string()),
            main: CoreProcessSpec::new(CoreType::Xray, launch("/tmp/xray", "run -c config.json"))
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

    #[tokio::test]
    async fn supervisor_windows_tun_cleanup_runs_before_process_start_and_assigns_job() {
        let events = SharedEvents::default();
        let sudo_passwords = Arc::new(SudoPasswordStore::new());
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), sudo_passwords)
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
                    CoreType::Xray,
                    launch("/tmp/xray", "run -c pre.json"),
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
    async fn supervisor_tun_sudo_wraps_singbox_and_mihomo_but_not_xray() {
        let events = SharedEvents::default();
        let sudo_passwords = Arc::new(SudoPasswordStore::new());
        sudo_passwords.set_password("pw").expect("sudo password");
        let supervisor = supervisor_with(&events, TargetOs::Linux, sudo_passwords);

        supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("xray".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::Xray,
                    launch("/tmp/xray", "run -c config.json"),
                ),
                pre: None,
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
            })
            .await
            .expect("xray start");
        supervisor.stop().await.expect("xray stop");

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
        supervisor.stop().await.expect("sing-box stop");

        supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("mihomo".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::mihomo,
                    launch("/tmp/mihomo", "-f config.json -d /tmp/voya/bin"),
                ),
                pre: None,
                tun_enabled: true,
                sudo_script_dir: "/tmp/voya/scripts".into(),
                restart_on_crash: false,
            })
            .await
            .expect("mihomo start");

        assert_eq!(
            events.lock().as_slice(),
            [
                "spawn:Main:pid=100:stdin=false",
                "stop:Main:pid=100",
                "spawn:Main:pid=101:stdin=true",
                "oneshot:SudoKill:stdin=true",
                "stop:Main:pid=101",
                "spawn:Main:pid=102:stdin=true"
            ]
        );
    }

    #[tokio::test]
    async fn supervisor_tun_partial_start_failure_kills_elevated_main_before_returning() {
        let events = SharedEvents::default();
        let sudo_passwords = Arc::new(SudoPasswordStore::new());
        sudo_passwords.set_password("pw").expect("sudo password");
        let runner = FakeRunner::new(events.clone()).with_fail_spawn_role(ProcessRole::Pre);
        let deps =
            SupervisorDeps::new(Arc::new(runner), sudo_passwords).with_target_os(TargetOs::Linux);
        let supervisor = CoreSupervisor::spawn(deps);

        let error = supervisor
            .start(SupervisorStartRequest {
                active_profile_id: Some("active".to_string()),
                main: CoreProcessSpec::new(
                    CoreType::sing_box,
                    launch("/tmp/sing-box", "run -c config.json --disable-color"),
                ),
                pre: Some(CoreProcessSpec::new(
                    CoreType::Xray,
                    launch("/tmp/xray", "run -c pre.json"),
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
                "spawn:Main:pid=100:stdin=true",
                "spawn-fail:Pre",
                "oneshot:SudoKill:stdin=true",
                "stop:Main:pid=100"
            ]
        );
        assert_eq!(
            supervisor.status().await.expect("status").state,
            SupervisorConnectionState::Disconnected
        );
    }
}
