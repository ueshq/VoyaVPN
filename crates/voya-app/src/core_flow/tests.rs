use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use voya_core::{
    AppConfig, CoreType, ProfileItem, ProfileProtocol, ProfileTransport, ServerEndpoint,
};
use voya_db::Database;
use voya_platform::{
    coreinfo::{core_type_dir_name, executable_name_for_current_os, get_core_info, TargetOs},
    paths::AppPaths,
    privilege::ElevationState,
    sysproxy::{PacManager, PacStartConfig, SystemProxyError, SystemProxyService},
    test_support::RecordingRunner,
    tun::{
        NativeTunController, NativeTunError, NativeTunProviderState, NativeTunStartRequest,
        NativeTunStatus, TunBackend,
    },
};

use super::*;
use crate::supervisor::{
    CoreExitGiveUp, CoreSupervisor, SupervisorConnectionState, SupervisorDeps,
};

static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Default)]
struct RecordingSink(Arc<Mutex<Vec<String>>>);

impl RecordingSink {
    fn events(&self) -> Vec<String> {
        self.0.lock().expect("core flow events").clone()
    }

    fn push(&self, event: impl Into<String>) {
        self.0.lock().expect("core flow events").push(event.into());
    }
}

impl CoreFlowSink for RecordingSink {
    fn log(&self, level: CoreFlowLevel, message: &str) {
        self.push(format!("log:{level:?}:{message}"));
    }

    fn core_state(
        &self,
        state: CoreFlowState,
        active_profile_id: Option<String>,
        snapshot: Option<&SupervisorSnapshot>,
    ) {
        self.push(format!(
            "state:{state:?}:profile={}:pid={:?}",
            active_profile_id
                .or_else(|| snapshot.and_then(|snapshot| snapshot.active_profile_id.clone()))
                .unwrap_or_default(),
            snapshot.and_then(|snapshot| snapshot.main_pid)
        ));
    }

    fn system_proxy_changed(&self, _status: &SystemProxyStatus) {
        self.push("sysproxy");
    }

    fn tun_changed(&self, _status: &TunStatus) {
        self.push("tun");
    }

    fn statistics_zero(&self) {
        self.push("statistics:zero");
    }

    fn notice(&self, level: CoreFlowLevel, title: &str, _message: &str) {
        self.push(format!("notice:{level:?}:{title}"));
    }
}

#[derive(Default)]
struct SilentPac;

impl PacManager for SilentPac {
    fn start(&self, _config: PacStartConfig) -> Result<(), SystemProxyError> {
        Ok(())
    }

    fn stop(&self) {}

    fn is_supported(&self) -> bool {
        false
    }
}

struct StoppedNativeTun;

impl NativeTunController for StoppedNativeTun {
    fn status(&self, backend: TunBackend) -> NativeTunStatus {
        NativeTunStatus {
            backend,
            provider_state: NativeTunProviderState::Stopped,
            component_ready: true,
            message: None,
        }
    }

    fn start(&self, _request: NativeTunStartRequest) -> Result<(), NativeTunError> {
        Ok(())
    }

    fn stop(&self, _backend: TunBackend) -> Result<(), NativeTunError> {
        Ok(())
    }
}

struct Harness {
    database: Database,
    paths: AppPaths,
    supervisor: CoreSupervisor,
    sink: RecordingSink,
}

impl Harness {
    async fn new() -> Self {
        let database = Database::connect_in_memory()
            .await
            .expect("in-memory database");
        let paths = temp_paths();
        paths.ensure_dirs().expect("runtime dirs");
        write_fake_core_executable(&paths);
        database
            .profiles()
            .upsert(&singbox_profile("active"))
            .await
            .expect("seed profile");

        let supervisor = CoreSupervisor::spawn(
            SupervisorDeps::new(
                Arc::new(RecordingRunner::default()),
                Arc::new(ElevationState::new()),
            )
            .with_target_os(TargetOs::Linux),
        );

        Self {
            database,
            paths,
            supervisor,
            sink: RecordingSink::default(),
        }
    }

    fn flow(&self) -> CoreFlow<'_> {
        let system_proxy = SystemProxyManager::with_target_os(
            SystemProxyService::new(Arc::new(RecordingRunner::default()), Arc::new(SilentPac)),
            self.paths.clone(),
            TargetOs::Linux,
        );
        let tun = TunManager::with_target_os_and_native_tun(
            Arc::new(ElevationState::new()),
            TargetOs::Linux,
            Arc::new(StoppedNativeTun),
        );

        CoreFlow::with_target_os(
            RuntimeManager::with_target_os(
                &self.database,
                self.paths.clone(),
                self.supervisor.clone(),
                TargetOs::Linux,
            ),
            system_proxy,
            tun,
            Arc::new(self.sink.clone()),
            TargetOs::Linux,
        )
    }
}

fn active_config() -> AppConfig {
    AppConfig {
        index_id: "active".to_string(),
        ..AppConfig::default()
    }
}

fn temp_paths() -> AppPaths {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let counter = TEMP_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
    AppPaths::new(
        std::env::temp_dir()
            .join("voyavpn-core-flow-tests")
            .join(format!("{}-{nanos}-{counter}", std::process::id())),
    )
}

fn write_fake_core_executable(paths: &AppPaths) {
    let core_info = get_core_info(CoreType::sing_box).expect("core info");
    let executable_name = executable_name_for_current_os(core_info.executable_names()[0]);
    let executable: PathBuf =
        paths.core_bin_file(core_type_dir_name(CoreType::sing_box), executable_name);
    fs::create_dir_all(executable.parent().expect("core dir")).expect("core dir");
    fs::write(executable, b"fake").expect("fake core");
}

fn singbox_profile(index_id: &str) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        remarks: "Core flow".to_string(),
        protocol: ProfileProtocol::Vless {
            server: ServerEndpoint {
                address: "example.test".to_string(),
                port: 443,
            },
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            flow: None,
            encryption: Some("none".to_string()),
        },
        transport: Some(ProfileTransport::Tcp {
            header: None,
            host: None,
            path: None,
        }),
        ..ProfileItem::default()
    }
}

#[tokio::test]
async fn connect_reports_connected_then_the_system_proxy_then_tun() {
    let harness = Harness::new().await;
    let config = active_config();

    let snapshot = harness
        .flow()
        .connect(&config)
        .await
        .expect("connect succeeds");

    assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
    let events = harness.sink.events();
    assert_eq!(
        events,
        [
            "log:Info:Connecting active profile".to_string(),
            "state:Connecting:profile=active:pid=None".to_string(),
            "log:Info:Core supervisor started".to_string(),
            format!("state:Connected:profile=active:pid={:?}", snapshot.main_pid),
            "sysproxy".to_string(),
            "tun".to_string(),
        ]
    );
}

#[tokio::test]
async fn a_failure_before_the_supervisor_leaves_the_previous_core_connected() {
    let harness = Harness::new().await;
    let config = active_config();
    harness
        .flow()
        .connect(&config)
        .await
        .expect("connect succeeds");

    // The active profile disappears (a subscription update, a manual delete)
    // before a config-change restart: `restart` fails while resolving it, long
    // before the supervisor is asked to stop anything.
    let missing = AppConfig {
        index_id: "gone".to_string(),
        ..AppConfig::default()
    };
    let sink_before = harness.sink.events().len();

    let error = harness
        .flow()
        .restart(&missing)
        .await
        .expect_err("the profile is gone");

    assert!(matches!(error, RuntimeError::ActiveProfileNotFound(_)));
    assert_eq!(
        harness.supervisor.status().await.expect("status").state,
        SupervisorConnectionState::Connected,
        "the previous core must survive a failure that never reached the supervisor"
    );
    let events = harness.sink.events();
    let tail = &events[sink_before..];
    assert!(
        tail.iter()
            .any(|event| event.starts_with("state:Connected")),
        "expected a Connected state, got {tail:?}"
    );
    assert!(
        !tail
            .iter()
            .any(|event| event.starts_with("state:Disconnected")),
        "a surviving core must never be reported as disconnected: {tail:?}"
    );
    assert!(
        !tail.iter().any(|event| event == "sysproxy"),
        "a surviving core must keep its system proxy: {tail:?}"
    );
}

#[tokio::test]
async fn disconnect_restores_the_system_proxy_and_zeroes_statistics() {
    let harness = Harness::new().await;
    let config = active_config();
    harness
        .flow()
        .connect(&config)
        .await
        .expect("connect succeeds");
    let sink_before = harness.sink.events().len();

    harness
        .flow()
        .disconnect(&config)
        .await
        .expect("disconnect succeeds");

    let events = harness.sink.events();
    assert_eq!(
        &events[sink_before..],
        [
            "log:Info:Disconnecting core supervisor".to_string(),
            "state:Disconnecting:profile=:pid=None".to_string(),
            "log:Info:Core supervisor stopped".to_string(),
            "state:Disconnected:profile=:pid=None".to_string(),
            "sysproxy".to_string(),
            "tun".to_string(),
            "statistics:zero".to_string(),
        ]
    );
}

#[tokio::test]
async fn giving_up_on_a_crashed_core_disconnects_and_restores_the_system_proxy() {
    let harness = Harness::new().await;
    let config = active_config();

    harness
        .flow()
        .handle_core_exit(
            &config,
            CoreExitEvent {
                active_profile_id: Some("active".to_string()),
                process_id: 4242,
                exit_code: Some(1),
                outcome: CoreExitOutcome::GaveUp(CoreExitGiveUp::CrashLoop { restarts: 3 }),
            },
        )
        .await;

    assert_eq!(
        harness.sink.events(),
        [
            "log:Error:Core process 4242 exited with code 1: the core kept exiting after 3 automatic restarts".to_string(),
            "notice:Error:Core stopped".to_string(),
            "state:Disconnected:profile=active:pid=None".to_string(),
            "sysproxy".to_string(),
            "tun".to_string(),
            "statistics:zero".to_string(),
        ]
    );
}

#[tokio::test]
async fn a_restarted_core_refreshes_the_snapshot_without_touching_the_system_proxy() {
    let harness = Harness::new().await;
    let config = active_config();

    harness
        .flow()
        .handle_core_exit(
            &config,
            CoreExitEvent {
                active_profile_id: Some("active".to_string()),
                process_id: 11,
                exit_code: Some(2),
                outcome: CoreExitOutcome::Restarted {
                    attempt: 1,
                    snapshot: SupervisorSnapshot {
                        state: SupervisorConnectionState::Connected,
                        active_profile_id: Some("active".to_string()),
                        main_pid: Some(12),
                        pre_pid: None,
                        running_core_type: Some(CoreType::sing_box),
                        clash_api_port: None,
                        clash_api_secret: None,
                    },
                },
            },
        )
        .await;

    assert_eq!(
        harness.sink.events(),
        [
            "log:Warn:Core process 11 exited with code 2; restarted the core (attempt 1)"
                .to_string(),
            "state:Connected:profile=active:pid=Some(12)".to_string(),
        ]
    );
}

#[tokio::test]
async fn a_scheduled_restart_is_reported_as_connecting() {
    let harness = Harness::new().await;
    let config = active_config();

    harness
        .flow()
        .handle_core_exit(
            &config,
            CoreExitEvent {
                active_profile_id: Some("active".to_string()),
                process_id: 7,
                exit_code: None,
                outcome: CoreExitOutcome::RestartScheduled {
                    attempt: 2,
                    delay: Duration::from_millis(1500),
                },
            },
        )
        .await;

    assert_eq!(
        harness.sink.events(),
        [
            "log:Warn:Core process 7 exited; retrying in 1500 ms (attempt 2)".to_string(),
            "state:Connecting:profile=active:pid=None".to_string(),
        ]
    );
}

#[tokio::test]
async fn restart_if_connected_stays_silent_while_the_core_is_down() {
    let harness = Harness::new().await;
    let config = active_config();

    harness
        .flow()
        .restart_if_connected(&config, "Routing changed")
        .await
        .expect("no restart is needed");

    assert!(harness.sink.events().is_empty());
}

#[tokio::test]
async fn restart_if_connected_restarts_a_running_core() {
    let harness = Harness::new().await;
    let config = active_config();
    harness
        .flow()
        .connect(&config)
        .await
        .expect("connect succeeds");
    let sink_before = harness.sink.events().len();

    harness
        .flow()
        .restart_if_connected(&config, "Routing changed")
        .await
        .expect("restart succeeds");

    let events = harness.sink.events();
    assert_eq!(
        &events[sink_before..sink_before + 3],
        [
            "log:Info:Routing changed; restarting core".to_string(),
            "state:Connecting:profile=active:pid=None".to_string(),
            "log:Info:Core supervisor restarted after Routing changed".to_string(),
        ]
    );
    assert!(events.iter().filter(|event| *event == "sysproxy").count() >= 2);
}
