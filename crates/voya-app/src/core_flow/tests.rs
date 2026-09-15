use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use voya_core::{AppConfig, ProfileItem, ProfileProtocol, ProfileTransport, ServerEndpoint};
use voya_db::Database;
use voya_platform::{
    coreinfo::{executable_name_for_current_os, TargetOs, CORE_DIR_NAME, SING_BOX_EXECUTABLES},
    paths::AppPaths,
    privilege::ElevationState,
    sysproxy::SystemProxyService,
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
struct RecordingSink(Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<SystemProxyStatus>>>);

impl RecordingSink {
    fn events(&self) -> Vec<String> {
        self.0.lock().expect("core flow events").clone()
    }

    fn push(&self, event: impl Into<String>) {
        self.0.lock().expect("core flow events").push(event.into());
    }
}

impl CoreFlowSink for RecordingSink {
    fn log(&self, level: CoreFlowLevel, code: LogCode, detail: Option<&str>) {
        // Codes, not sentences: the recording spells the variant the way the
        // wire does, so a reworded locale string can never fail these.
        self.push(format!(
            "log:{level:?}:{}{}",
            code_tag(&code),
            detail
                .map(|detail| format!(":{detail}"))
                .unwrap_or_default()
        ));
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

    fn system_proxy_changed(&self, status: &SystemProxyStatus) {
        self.1.lock().expect("proxy statuses").push(status.clone());
        self.push("sysproxy");
    }

    fn tun_changed(&self, _status: &TunStatus) {
        self.push("tun");
    }

    fn statistics_zero(&self) {
        self.push("statistics:zero");
    }

    fn notice(&self, level: CoreFlowLevel, code: NoticeCode, _detail: &str) {
        self.push(format!("notice:{level:?}:{}", code_tag(&code)));
    }
}

/// The serde tag of a code, plus the parameters that make two uses of the same
/// code distinguishable.
fn code_tag(code: &impl serde::Serialize) -> String {
    let value = serde_json::to_value(code).expect("serialize code");
    let tag = value["code"].as_str().unwrap_or_default().to_string();
    let params = value
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(key, _)| key.as_str() != "code")
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>();
    if params.is_empty() {
        tag
    } else {
        format!("{tag}({})", params.join(","))
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

#[derive(Clone, Default)]
struct ModeTransport {
    requests: Arc<Mutex<Vec<voya_net::clash::ClashHttpRequest>>>,
    events_before_apply: Arc<Mutex<Vec<Vec<String>>>>,
    observe_sink: Option<RecordingSink>,
    fail: bool,
}

impl ClashHttpTransport for ModeTransport {
    fn send_json<'transport>(
        &'transport self,
        request: voya_net::clash::ClashHttpRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = voya_net::clash::Result<serde_json::Value>>
                + Send
                + 'transport,
        >,
    > {
        Box::pin(async move {
            if let Some(sink) = &self.observe_sink {
                self.events_before_apply
                    .lock()
                    .expect("events")
                    .push(sink.events());
            }
            self.requests.lock().expect("mode requests").push(request);
            if self.fail {
                return Err(voya_net::clash::ClashError::Request(
                    "mode unavailable".into(),
                ));
            }
            Ok(serde_json::Value::Null)
        })
    }
}

struct Harness {
    database: Database,
    paths: AppPaths,
    supervisor: CoreSupervisor,
    sink: RecordingSink,
    mode_transport: ModeTransport,
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
            mode_transport: ModeTransport::default(),
        }
    }

    fn flow(&self) -> CoreFlow<'_, ModeTransport> {
        self.flow_with_proxy_runner(RecordingRunner::default())
    }

    fn flow_with_proxy_runner(&self, runner: RecordingRunner) -> CoreFlow<'_, ModeTransport> {
        let system_proxy = SystemProxyManager::with_target_os(
            SystemProxyService::new(Arc::new(runner)),
            self.paths.clone(),
            TargetOs::Linux,
        );
        self.flow_with_proxy_manager(system_proxy)
    }

    fn flow_with_proxy_manager(
        &self,
        system_proxy: SystemProxyManager,
    ) -> CoreFlow<'_, ModeTransport> {
        let tun = TunManager::with_target_os_and_native_tun(
            Arc::new(ElevationState::new()),
            TargetOs::Linux,
            Arc::new(StoppedNativeTun),
        );

        CoreFlow::new(
            RuntimeManager::with_target_os(
                &self.database,
                self.paths.clone(),
                self.supervisor.clone(),
                TargetOs::Linux,
            ),
            system_proxy,
            tun,
            Arc::new(self.sink.clone()),
        )
        .with_proxy_runtime(ProxyRuntimeManager::with_transport(
            self.mode_transport.clone(),
        ))
    }
}

fn active_config() -> AppConfig {
    AppConfig {
        index_id: "active".to_string(),
        ..AppConfig::default()
    }
}

#[tokio::test]
async fn saved_mode_is_applied_before_connect_restart_and_recovery_are_announced() {
    let harness = Harness::new().await;
    let transport = ModeTransport {
        observe_sink: Some(harness.sink.clone()),
        ..ModeTransport::default()
    };
    let flow = harness
        .flow()
        .with_proxy_runtime(ProxyRuntimeManager::with_transport(transport.clone()));
    let mut config = active_config();
    config.proxy_ui_item.traffic_mode = voya_core::TrafficMode::Global;
    let first = flow.connect(&config).await.expect("connect");
    let restarted = flow.restart(&config).await.expect("restart");
    flow.restart_if_connected(&config, CoreFlowReason::Connect)
        .await
        .expect("config restart");
    flow.handle_core_exit(
        &config,
        CoreExitEvent {
            active_profile_id: Some("active".into()),
            process_id: 11,
            exit_code: Some(2),
            outcome: CoreExitOutcome::Restarted {
                attempt: 1,
                snapshot: restarted.clone(),
            },
        },
    )
    .await;
    let requests = transport.requests.lock().expect("requests");
    assert_eq!(requests.len(), 4);
    for request in requests.iter() {
        assert_eq!(request.body, Some(serde_json::json!({ "mode": "global" })));
    }
    assert_eq!(
        requests[0].bearer_token.as_deref(),
        first
            .clash_api_access()
            .secret
            .as_ref()
            .map(crate::supervisor::ClashApiSecret::as_str)
    );
    let observations = transport.events_before_apply.lock().expect("observations");
    for (index, events) in observations.iter().enumerate() {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.starts_with("state:Connected:"))
                .count(),
            index
        );
    }
}

#[tokio::test]
async fn startup_mode_failure_warns_and_keeps_the_saved_preference() {
    let harness = Harness::new().await;
    // Pause after SQLite setup; its worker threads use wall-clock scheduling.
    tokio::time::pause();
    let transport = ModeTransport {
        fail: true,
        ..ModeTransport::default()
    };
    let mut config = active_config();
    config.proxy_ui_item.traffic_mode = voya_core::TrafficMode::Global;
    let snapshot = harness
        .flow()
        .with_proxy_runtime(ProxyRuntimeManager::with_transport(transport))
        .connect(&config)
        .await
        .expect("core still connected");
    assert_eq!(snapshot.state, SupervisorConnectionState::Connected);
    assert_eq!(
        config.proxy_ui_item.traffic_mode,
        voya_core::TrafficMode::Global
    );
    let events = harness.sink.events();
    let warning = events
        .iter()
        .position(|event| event == "notice:Warn:proxyModeSavedRuntimeUpdateFailed")
        .expect("warning");
    let connected = events
        .iter()
        .position(|event| event.starts_with("state:Connected:"))
        .expect("connected");
    assert!(warning < connected);
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
    let executable_name = executable_name_for_current_os(SING_BOX_EXECUTABLES[0]);
    let executable: PathBuf = paths.core_bin_file(CORE_DIR_NAME, executable_name);
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
async fn saved_proxy_settings_are_not_applied_while_disconnected() {
    let harness = Harness::new().await;
    harness
        .flow()
        .reapply_system_proxy_if_connected(&active_config())
        .await
        .expect("disconnected no-op");
    assert!(harness.sink.events().is_empty());
}

#[tokio::test]
async fn saved_proxy_settings_publish_only_the_proxy_status_while_connected() {
    let harness = Harness::new().await;
    let config = active_config();
    harness.flow().connect(&config).await.expect("connect");
    let before = harness.sink.events().len();
    harness
        .flow()
        .reapply_system_proxy_if_connected(&config)
        .await
        .expect("reapply proxy");
    assert_eq!(&harness.sink.events()[before..], ["sysproxy"]);
}

#[tokio::test]
async fn explicit_proxy_application_failure_is_reported_and_keeps_the_core_connected() {
    let harness = Harness::new().await;
    let mut config = active_config();
    harness.flow().connect(&config).await.expect("connect");
    let before = harness.sink.events().len();
    config.system_proxy_item.sys_proxy_type = voya_core::SysProxyType::ForcedChange;
    harness
        .flow_with_proxy_runner(RecordingRunner::default().with_oneshot_output(
            voya_platform::process::ProcessOutput {
                status_code: Some(1),
                stdout: String::new(),
                stderr: "proxy rejected".to_string(),
            },
        ))
        .reapply_system_proxy_if_connected(&config)
        .await
        .expect_err("explicit apply reports failure");
    assert_eq!(
        &harness.sink.events()[before..],
        [
            "sysproxy",
            "notice:Warn:settingsSavedSystemProxyUpdateFailed"
        ]
    );
    assert_eq!(
        harness.supervisor.status().await.expect("status").state,
        SupervisorConnectionState::Connected
    );
}

#[tokio::test]
async fn connect_publishes_the_system_proxy_before_connected_then_tun() {
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
            "log:Info:connecting".to_string(),
            "state:Connecting:profile=active:pid=None".to_string(),
            "log:Info:connected".to_string(),
            "sysproxy".to_string(),
            format!("state:Connected:profile=active:pid={:?}", snapshot.main_pid),
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
        tail.iter().any(|event| event == "sysproxy"),
        "a surviving core must publish its unchanged system proxy: {tail:?}"
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
            "log:Info:disconnecting".to_string(),
            "state:Disconnecting:profile=:pid=None".to_string(),
            "log:Info:disconnected".to_string(),
            "sysproxy".to_string(),
            "state:Disconnected:profile=:pid=None".to_string(),
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
            "log:Error:coreExitGaveUp:Core process 4242 exited with code 1: the core kept exiting after 3 automatic restarts".to_string(),
            "notice:Error:coreStopped".to_string(),
            "sysproxy".to_string(),
            "state:Disconnected:profile=active:pid=None".to_string(),
            "tun".to_string(),
            "statistics:zero".to_string(),
        ]
    );
}

#[tokio::test]
async fn a_restarted_core_refreshes_proxy_state_before_the_snapshot() {
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
                        connected_duration_ms: Some(0),
                        state: SupervisorConnectionState::Connected,
                        active_tun_backend: None,
                        active_profile_id: Some("active".to_string()),
                        active_group_id: None,
                        main_pid: Some(12),
                        pre_pid: None,
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
            "log:Warn:coreExitRestarted(attempt=1):Core process 11 exited with code 2".to_string(),
            "sysproxy".to_string(),
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
            "log:Warn:coreExitRetryScheduled(attempt=2,delayMs=1500):Core process 7 exited"
                .to_string(),
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
        .restart_if_connected(&config, CoreFlowReason::RoutingChanged)
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
        .restart_if_connected(&config, CoreFlowReason::RoutingChanged)
        .await
        .expect("restart succeeds");

    let events = harness.sink.events();
    assert_eq!(
        &events[sink_before..sink_before + 3],
        [
            "log:Info:restartingAfterChange(reason=\"routingChanged\")".to_string(),
            "state:Connecting:profile=active:pid=None".to_string(),
            "log:Info:restartedAfterChange(reason=\"routingChanged\")".to_string(),
        ]
    );
    assert!(events.iter().filter(|event| *event == "sysproxy").count() >= 2);
}

struct CleanupFailure;
impl NativeTunController for CleanupFailure {
    fn status(&self, backend: TunBackend) -> NativeTunStatus {
        NativeTunStatus {
            backend,
            component_ready: true,
            provider_state: NativeTunProviderState::Running,
            message: None,
        }
    }
    fn start(&self, _request: NativeTunStartRequest) -> Result<(), NativeTunError> {
        Err(NativeTunError::StartCleanupFailed {
            start_error: "start failed".into(),
            cleanup_error: "stop failed".into(),
        })
    }
    fn stop(&self, _backend: TunBackend) -> Result<(), NativeTunError> {
        Err(NativeTunError::Bridge {
            action: "stop",
            message: "stop failed".into(),
        })
    }
}

#[tokio::test]
async fn pending_native_cleanup_publishes_the_proxy_state_before_the_pending_state() {
    let mut harness = Harness::new().await;
    harness.supervisor = CoreSupervisor::spawn(
        SupervisorDeps::new(
            Arc::new(RecordingRunner::default()),
            Arc::new(ElevationState::new()),
        )
        .with_target_os(TargetOs::Macos)
        .with_native_tun_controller(Arc::new(CleanupFailure)),
    );
    let manager = SystemProxyManager::with_target_os(
        SystemProxyService::new(Arc::new(RecordingRunner::default())),
        harness.paths.clone(),
        TargetOs::Macos,
    );
    let mut config = active_config();
    config.tun_mode_item.enable_tun = true;
    let error = harness
        .flow_with_proxy_manager(manager)
        .connect(&config)
        .await
        .expect_err("pending cleanup");
    assert!(error.to_string().contains("start failed"));
    assert!(error.to_string().contains("stop failed"));
    let statuses = harness.sink.1.lock().expect("statuses");
    let last = statuses.last().expect("retired state");
    assert!(last.proxy.is_none());
    drop(statuses);
    let events = harness.sink.events();
    let proxy = events
        .iter()
        .position(|event| event == "sysproxy")
        .expect("proxy event");
    let pending = events
        .iter()
        .position(|event| event.starts_with("state:CleanupPending"))
        .expect("pending event");
    assert!(proxy < pending);
}

#[tokio::test]
async fn removing_the_running_node_stops_it_without_selecting_another_node() {
    let harness = Harness::new().await;
    let mut config = active_config();
    let flow = harness.flow();
    flow.connect(&config).await.expect("connect");
    harness
        .database
        .profiles()
        .upsert(&singbox_profile("other"))
        .await
        .expect("another node");
    flow.disconnect_removed_profile(&config)
        .await
        .expect("still exists");
    assert_eq!(
        harness.supervisor.status().await.expect("status").state,
        SupervisorConnectionState::Connected
    );
    crate::profiles::ProfileManager::new(&harness.database)
        .delete_profiles(&mut config, &["active".into()])
        .await
        .expect("delete current");
    assert!(config.index_id.is_empty());
    flow.disconnect_removed_profile(&config)
        .await
        .expect("reconcile");
    let status = harness.supervisor.status().await.expect("status");
    assert_eq!(status.state, SupervisorConnectionState::Disconnected);
    assert!(status.active_profile_id.is_none());
    let events = harness.sink.events();
    assert!(events.contains(&"statistics:zero".into()));
    assert!(events.contains(&"sysproxy".into()));
    assert!(events.contains(&"notice:Warn:activeSelectionRemoved".into()));
    // A repeated notification must not interrupt a newer valid selection.
    config.index_id = "other".into();
    flow.connect(&config).await.expect("select other");
    flow.disconnect_removed_profile(&config)
        .await
        .expect("late reconciliation");
    assert_eq!(
        harness
            .supervisor
            .status()
            .await
            .expect("status")
            .active_profile_id
            .as_deref(),
        Some("other")
    );
    flow.disconnect(&config).await.expect("cleanup");
}
