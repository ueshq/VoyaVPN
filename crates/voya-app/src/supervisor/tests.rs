#[cfg(unix)]
use std::fs;
use std::{
    io,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};
use voya_platform::{
    coreinfo::CoreLaunch,
    privilege::ElevationState,
    process::{
        ProcessError, ProcessJob, ProcessJobFactory, ProcessOutput, ProcessRunner, StdProcessRunner,
    },
    tun::NativeTunController,
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
        self.events
            .push(format!("spawn:{:?}:pid={pid}", request.role));
        Ok(ProcessHandle::new(pid, request.role))
    }

    fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
        self.events.push(format!("oneshot:{:?}", request.role));
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

    fn set_provider_state(&self, provider_state: NativeTunProviderState, message: Option<String>) {
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

struct FailedStartController {
    cleanup_fails: std::sync::atomic::AtomicBool,
    accepted: bool,
}
impl NativeTunController for FailedStartController {
    fn status(&self, backend: TunBackend) -> voya_platform::tun::NativeTunStatus {
        voya_platform::tun::NativeTunStatus {
            backend,
            provider_state: NativeTunProviderState::Running,
            component_ready: true,
            message: None,
        }
    }
    fn start(&self, _request: NativeTunStartRequest) -> Result<(), NativeTunError> {
        if self.accepted {
            Err(NativeTunError::StartCleanupFailed {
                start_error: "original start error".into(),
                cleanup_error: "stop timed out".into(),
            })
        } else {
            Err(NativeTunError::Bridge {
                action: "start",
                message: "request rejected".into(),
            })
        }
    }
    fn stop(&self, _backend: TunBackend) -> Result<(), NativeTunError> {
        if self
            .cleanup_fails
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            Err(NativeTunError::Bridge {
                action: "stop",
                message: "stop timed out".into(),
            })
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn failed_native_start_keeps_cleanup_record_until_stop_succeeds() {
    for accepted in [true, false] {
        let controller = Arc::new(FailedStartController {
            cleanup_fails: std::sync::atomic::AtomicBool::new(true),
            accepted,
        });
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(SharedEvents::default())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Macos)
        .with_native_tun_controller(controller.clone());
        let supervisor = CoreSupervisor::spawn(deps);
        let error = supervisor
            .start(native_tun_test_request())
            .await
            .expect_err("failed start");
        let status = supervisor.status().await.expect("status");
        assert_eq!(status.active_tun_backend, None);
        assert_eq!(status.connected_duration_ms, None);
        if accepted {
            assert!(error.to_string().contains("original start error"));
            assert!(error.to_string().contains("stop timed out"));
            assert_eq!(status.state, SupervisorConnectionState::CleanupPending);
            assert!(status.clash_api_port.is_none());
            assert!(supervisor.stop().await.is_err());
            assert_eq!(
                supervisor.status().await.expect("pending status").state,
                SupervisorConnectionState::CleanupPending
            );
        } else {
            assert_eq!(status.state, SupervisorConnectionState::Disconnected);
        }
        controller
            .cleanup_fails
            .store(false, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            supervisor.stop().await.expect("retry stop").state,
            SupervisorConnectionState::Disconnected
        );
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
            active_group_id: None,
            main: CoreProcessSpec::new(launch(
                "/tmp/sing-box-main",
                "run -c config.json --disable-color",
            )),
            pre: Some(CoreProcessSpec::new(launch(
                "/tmp/sing-box",
                "run -c pre.json --disable-color",
            ))),
            tun_enabled: true,
            kill_switch: false,
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
            "oneshot:SudoKill",
            "oneshot:SudoKill",
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
            active_group_id: None,
            main: CoreProcessSpec::new(launch(
                "/tmp/voya cores/sing-box-client",
                "run -c config.json --disable-color",
            )),
            pre: None,
            tun_enabled: true,
            kill_switch: false,
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
    // user-owned script.
    assert!(requests[0].generated_scripts.is_empty());
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
    let clock = FakeClock::new();
    let deps = SupervisorDeps::new(runner, elevation)
        .with_target_os(TargetOs::Linux)
        .with_clock(Arc::new(clock.clone()));
    let supervisor = CoreSupervisor::spawn(deps);

    supervisor
        .start(SupervisorStartRequest {
            active_profile_id: Some("active".to_string()),
            active_group_id: None,
            main: CoreProcessSpec::new(launch(
                "/tmp/sing-box",
                "run -c config.json --disable-color",
            )),
            pre: None,
            tun_enabled: true,
            kill_switch: false,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: false,
            clash_api_port: 0,
            clash_api_secret: None,
        })
        .await
        .expect("start");

    clock.advance(Duration::from_secs(7));
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
    assert_eq!(snapshot.connected_duration_ms, Some(7000));
    assert_eq!(
        events.lock().as_slice(),
        ["spawn:Main:pid=100", "oneshot:SudoKill"]
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
                active_group_id: None,
                main: CoreProcessSpec::new(launch(
                    "/tmp/sing-box",
                    "run -c config.json --disable-color",
                )),
                pre: None,
                tun_enabled: true,
                kill_switch: false,
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
            "spawn:Main:pid=100",
            "oneshot:SudoKill",
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
        active_group_id: None,
        main: CoreProcessSpec::new(launch(
            "/tmp/sing-box",
            "run -c config.json --disable-color",
        )),
        pre: None,
        tun_enabled: true,
        kill_switch: false,
        sudo_script_dir: "/tmp/voya/scripts".into(),
        restart_on_crash: false,
        clash_api_port: 0,
        clash_api_secret: None,
    };

    let missing = supervisor
        .start(request.clone())
        .await
        .expect_err("ungranted elevation should fail");
    assert!(matches!(missing, SupervisorError::ElevationNotGranted));

    elevation.set_granted(true);
    supervisor
        .start(request)
        .await
        .expect("start with elevation grant");
    assert_eq!(events.lock().as_slice(), ["spawn:Main:pid=100"]);
}

#[tokio::test]
async fn supervisor_crash_restarts_serialized_lifecycle() {
    let events = SharedEvents::default();
    let elevation = Arc::new(ElevationState::new());
    let supervisor = supervisor_with(&events, TargetOs::Linux, elevation);

    let request = SupervisorStartRequest {
        active_profile_id: Some("active".to_string()),
        active_group_id: None,
        main: CoreProcessSpec::new(launch(
            "/tmp/sing-box",
            "run -c config.json --disable-color",
        ))
        .with_may_need_sudo(false),
        pre: None,
        tun_enabled: false,
        kill_switch: false,
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
            "spawn:Main:pid=100",
            "stop:Main:pid=100",
            "spawn:Main:pid=101"
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
            active_group_id: None,
            main: CoreProcessSpec::new(CoreLaunch {
                executable: script,
                arguments: String::new(),
                working_dir: temp_dir.clone(),
            })
            .with_display_log(false)
            .with_may_need_sudo(false),
            pre: None,
            tun_enabled: false,
            kill_switch: false,
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
            active_group_id: None,
            main: CoreProcessSpec::new(launch(
                "/tmp/sing-box",
                "run -c config.json --disable-color",
            )),
            pre: Some(CoreProcessSpec::new(launch(
                "/tmp/sing-box-pre",
                "run -c pre.json --disable-color",
            ))),
            tun_enabled: false,
            kill_switch: false,
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
            "spawn:Main:pid=100",
            "job:assign:Main:pid=100",
            "spawn:Pre:pid=101",
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
            active_group_id: None,
            main: CoreProcessSpec::new(launch(
                "/tmp/sing-box",
                "run -c config.json --disable-color",
            ))
            .with_config_path("/tmp/voya/config.json"),
            pre: Some(
                CoreProcessSpec::new(launch(
                    "/tmp/sing-box-pre",
                    "run -c pre.json --disable-color",
                ))
                .with_config_path("/tmp/voya/pre.json"),
            ),
            tun_enabled: true,
            kill_switch: false,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: false,
            clash_api_port: 0,
            clash_api_secret: None,
        })
        .await
        .expect("native tun start");

    assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
    assert_eq!(
        snapshot.active_tun_backend,
        Some(TunBackend::WindowsService)
    );
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
            active_group_id: None,
            main: CoreProcessSpec::new(launch(
                "/tmp/sing-box",
                "run -c config.json --disable-color",
            ))
            .with_config_path("/tmp/voya/config.json"),
            pre: None,
            tun_enabled: true,
            kill_switch: false,
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
            active_group_id: None,
            main: CoreProcessSpec::new(launch(
                "/tmp/sing-box",
                "run -c config.json --disable-color",
            )),
            pre: None,
            tun_enabled: true,
            kill_switch: false,
            sudo_script_dir: "/tmp/voya/scripts".into(),
            restart_on_crash: false,
            clash_api_port: 0,
            clash_api_secret: None,
        })
        .await
        .expect("sing-box start");

    assert_eq!(events.lock().as_slice(), ["spawn:Main:pid=100",]);
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
            active_group_id: None,
            main: CoreProcessSpec::new(launch(
                "/tmp/sing-box",
                "run -c config.json --disable-color",
            )),
            pre: Some(CoreProcessSpec::new(launch(
                "/tmp/sing-box-pre",
                "run -c pre.json --disable-color",
            ))),
            tun_enabled: true,
            kill_switch: false,
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
            "spawn:Main:pid=100",
            "spawn-fail:Pre",
            "oneshot:SudoKill",
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
        active_group_id: None,
        main: CoreProcessSpec::new(launch(
            "/tmp/sing-box",
            "run -c config.json --disable-color",
        ))
        .with_may_need_sudo(false),
        pre: None,
        tun_enabled: false,
        kill_switch: false,
        sudo_script_dir: "/tmp/voya/scripts".into(),
        restart_on_crash: true,
        clash_api_port: 0,
        clash_api_secret: None,
    }
}

#[tokio::test]
async fn connected_duration_tracks_backend_lifetime_and_resets_on_restart() {
    for target in [TargetOs::Linux, TargetOs::Windows, TargetOs::Macos] {
        let clock = FakeClock::new();
        let events = SharedEvents::default();
        let deps = SupervisorDeps::new(
            Arc::new(FakeRunner::new(events.clone())),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(target)
        .with_clock(Arc::new(clock.clone()))
        .with_native_tun_controller(Arc::new(RecordingNativeTunController { events }));
        let supervisor = CoreSupervisor::spawn(deps);
        assert_eq!(
            supervisor
                .status()
                .await
                .expect("idle")
                .connected_duration_ms,
            None
        );
        let mut request = crash_test_request();
        request.tun_enabled = target != TargetOs::Linux;
        request.main = request.main.with_config_path("/tmp/voya/config.json");
        assert_eq!(
            supervisor
                .start(request.clone())
                .await
                .expect("start")
                .connected_duration_ms,
            Some(0)
        );
        clock.advance(Duration::from_millis(1458000));
        assert_eq!(
            supervisor
                .status()
                .await
                .expect("reopened view")
                .connected_duration_ms,
            Some(1458000)
        );
        assert_eq!(
            supervisor
                .start(request)
                .await
                .expect("restart")
                .connected_duration_ms,
            Some(0)
        );
        clock.advance(Duration::from_secs(2));
        if target == TargetOs::Linux {
            let pid = supervisor
                .status()
                .await
                .expect("running")
                .main_pid
                .expect("pid");
            assert_eq!(
                supervisor
                    .process_exited(pid, Some(1))
                    .await
                    .expect("crash restart")
                    .connected_duration_ms,
                Some(0)
            );
        }
        assert_eq!(
            supervisor.stop().await.expect("stop").connected_duration_ms,
            None
        );
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
            "spawn:Main:pid=100",
            "stop:Main:pid=100",
            "spawn:Main:pid=101",
            "stop:Main:pid=101",
            "spawn:Main:pid=102",
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
        ["spawn:Main:pid=100", "stop:Main:pid=100"]
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
        if supervisor.status().await.expect("status").state == SupervisorConnectionState::Connected
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
        active_group_id: None,
        main: CoreProcessSpec::new(launch(
            "/tmp/sing-box",
            "run -c config.json --disable-color",
        )),
        pre: None,
        tun_enabled: true,
        kill_switch: false,
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

    assert!(matches!(error, SupervisorError::ElevationNotGranted));
    let snapshot = supervisor.status().await.expect("status");
    assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
    assert_eq!(snapshot.main_pid, Some(100));
    assert_eq!(events.lock().as_slice(), ["spawn:Main:pid=100"]);
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

    let status = supervisor.status().await.expect("status");
    assert_eq!(status.connected_duration_ms, None);
    assert_eq!(
        status.state,
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
        .start(crash_test_request())
        .await
        .expect("restart");

    assert_eq!(second.state, SupervisorConnectionState::Connected);
    assert_ne!(first.main_pid, second.main_pid);
    assert_eq!(
        events.lock().as_slice(),
        [
            "spawn:Main:pid=100",
            "stop:Main:pid=100",
            "spawn:Main:pid=101"
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
        .start(native_tun_test_request())
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
                CoreProcessSpec::new(launch(
                    "/tmp/sing-box-pre",
                    "run -c pre.json --disable-color",
                ))
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
            "spawn:Main:pid=100",
            "spawn:Pre:pid=101",
            "stop:Main:pid=100",
            "stop:Pre:pid=101",
            "spawn:Main:pid=102",
            "spawn:Pre:pid=103"
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
        active_group_id: None,
        main: CoreProcessSpec::new(launch(
            "/tmp/sing-box",
            "run -c config.json --disable-color",
        )),
        pre: None,
        tun_enabled: true,
        kill_switch: false,
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

#[tokio::test]
async fn active_tun_backend_tracks_the_running_request_and_clears_after_stop() {
    for (target, expected) in [
        (TargetOs::Macos, TunBackend::MacosPacketTunnel),
        (TargetOs::Windows, TunBackend::WindowsService),
        (TargetOs::Linux, TunBackend::Process),
    ] {
        let events = SharedEvents::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let deps = SupervisorDeps::new(Arc::new(FakeRunner::new(events.clone())), elevation)
            .with_target_os(target)
            .with_native_tun_controller(Arc::new(RecordingNativeTunController { events }));
        let supervisor = CoreSupervisor::spawn(deps);
        assert_eq!(
            supervisor
                .status()
                .await
                .expect("initial")
                .active_tun_backend,
            None
        );
        let local = supervisor
            .start(SupervisorStartRequest {
                tun_enabled: false,
                kill_switch: false,
                ..native_tun_test_request()
            })
            .await
            .expect("local proxy");
        assert_eq!(local.active_tun_backend, None);
        if target == TargetOs::Macos {
            let mut invalid = native_tun_test_request();
            invalid.main.config_path = None;
            assert!(supervisor.start(invalid).await.is_err());
            let retained = supervisor.status().await.expect("retained local proxy");
            assert_eq!(retained.state, SupervisorConnectionState::Connected);
            assert_eq!(retained.active_tun_backend, None);
        }
        let tunnel = supervisor
            .start(native_tun_test_request())
            .await
            .expect("tunnel");
        assert_eq!(tunnel.active_tun_backend, Some(expected));
        assert_eq!(
            supervisor.stop().await.expect("stop").active_tun_backend,
            None
        );
    }
}

fn native_tun_test_request() -> SupervisorStartRequest {
    SupervisorStartRequest {
        active_profile_id: Some("active".to_string()),
        active_group_id: None,
        main: CoreProcessSpec::new(launch(
            "/tmp/sing-box",
            "run -c config.json --disable-color",
        ))
        .with_config_path("/tmp/voya/config.json"),
        pre: None,
        tun_enabled: true,
        kill_switch: false,
        sudo_script_dir: "/tmp/voya/scripts".into(),
        restart_on_crash: true,
        clash_api_port: 0,
        clash_api_secret: None,
    }
}
