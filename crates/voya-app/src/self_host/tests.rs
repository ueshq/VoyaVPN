//! The manager against fakes: a recording process runner, a scripted probe
//! service and router, and a network whose busy ports the test chooses.

use std::{
    collections::HashSet,
    fs,
    net::IpAddr,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use futures_util::future::BoxFuture;
use tokio::sync::Notify;
use voya_contracts::{
    AppNoticeLevel, LogCode, LogLevel, NoticeCode, SelfHostConfig, SelfHostPortMappingStatus,
    SelfHostProblem, SelfHostReachability, SelfHostRuntimeStatus, SelfHostState,
};
use voya_db::Database;
use voya_net::{
    portmap::{PortMapper, PortMappingError, PortMappingResult},
    probe::{
        PortProbeOutcome, PortProbeResult, ProbeFamily, ReachabilityProbeError,
        ReachabilityProbeResponse,
    },
};
use voya_platform::{
    coreinfo::{executable_name_for_current_os, TargetOs, CORE_DIR_NAME},
    firewall::FirewallService,
    netif::InterfaceAddress,
    paths::{core_seed_resources_dir, AppPaths},
    process::{
        ProcessError, ProcessExit, ProcessExitHandler, ProcessHandle, ProcessOutput, ProcessRole,
        ProcessRunner, ProcessSpawn,
    },
    test_support::RecordingRunner,
};

use voya_contracts::{SelfHostRecordV1, SelfHostSelfTest, SelfHostSelfTestResult};

use super::{
    HostTunnelState, LocalNetwork, NodeSelfTester, ReachabilityProbe, RestartBackoff, SelfHostDeps,
    SelfHostError, SelfHostEventSink, SelfHostManager,
};

#[cfg(test)]
mod live;

/// Reports every enabled protocol as working, without a core.
struct PassingSelfTest;

impl NodeSelfTester for PassingSelfTest {
    fn run<'a>(
        &'a self,
        _deps: &'a SelfHostDeps,
        record: &'a SelfHostRecordV1,
    ) -> BoxFuture<'a, SelfHostSelfTest> {
        let verdict = |enabled| {
            if enabled {
                SelfHostSelfTestResult::Passed
            } else {
                SelfHostSelfTestResult::Skipped
            }
        };
        let result = SelfHostSelfTest {
            vless: verdict(record.config.vless_enabled),
            shadowsocks: verdict(record.config.shadowsocks_enabled),
        };
        Box::pin(async move { result })
    }
}

#[derive(Default)]
struct RecordingSink {
    changes: Mutex<u32>,
    logs: Mutex<Vec<LogCode>>,
    notices: Mutex<Vec<NoticeCode>>,
}

impl SelfHostEventSink for RecordingSink {
    fn state_changed(&self) {
        *self.changes.lock().expect("changes") += 1;
    }

    fn log(&self, _level: LogLevel, code: LogCode, _detail: Option<String>) {
        self.logs.lock().expect("logs").push(code);
    }

    fn notice(&self, _level: AppNoticeLevel, code: NoticeCode, _detail: Option<String>) {
        self.notices.lock().expect("notices").push(code);
    }
}

/// Answers every probe with the current public address and every port open.
struct FakeProbe {
    ipv4: Mutex<Option<IpAddr>>,
    reachable: bool,
}

impl ReachabilityProbe for FakeProbe {
    fn probe(
        &self,
        family: ProbeFamily,
        ports: Vec<u16>,
    ) -> BoxFuture<'static, Result<ReachabilityProbeResponse, ReachabilityProbeError>> {
        let address = *self.ipv4.lock().expect("address");
        let reachable = self.reachable;
        Box::pin(async move {
            match (family, address) {
                (ProbeFamily::Ipv4, Some(address)) => Ok(ReachabilityProbeResponse {
                    ip: address.to_string(),
                    family,
                    results: ports
                        .into_iter()
                        .map(|port| PortProbeResult {
                            port,
                            reachable,
                            reason: if reachable {
                                PortProbeOutcome::Connected
                            } else {
                                PortProbeOutcome::Timeout
                            },
                            elapsed_ms: 5,
                        })
                        .collect(),
                }),
                _ => Err(ReachabilityProbeError::Status { status: 503 }),
            }
        })
    }

    fn public_address(
        &self,
        family: ProbeFamily,
    ) -> BoxFuture<'static, Result<IpAddr, ReachabilityProbeError>> {
        let address = *self.ipv4.lock().expect("address");
        Box::pin(async move {
            match (family, address) {
                (ProbeFamily::Ipv4, Some(address)) => Ok(address),
                _ => Err(ReachabilityProbeError::Decode("no address".to_string())),
            }
        })
    }
}

/// Holds `spawn` while closed, so a test can look at a start midway. It opens
/// by itself after a while, so a failing test cannot hang on it.
#[derive(Default)]
struct SpawnGate {
    closed: Mutex<bool>,
    opened: Condvar,
}

impl SpawnGate {
    fn set_closed(&self, closed: bool) {
        *self.closed.lock().expect("gate") = closed;
        self.opened.notify_all();
    }

    fn pass(&self) {
        let closed = self.closed.lock().expect("gate");
        let (_closed, _timeout) = self
            .opened
            .wait_timeout_while(closed, Duration::from_secs(5), |closed| *closed)
            .expect("gate");
    }
}

/// The recording runner behind a [`SpawnGate`].
struct GatedRunner {
    runner: RecordingRunner,
    gate: Arc<SpawnGate>,
}

impl ProcessRunner for GatedRunner {
    fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
        self.gate.pass();
        self.runner.spawn(request)
    }

    fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
        self.runner.run_oneshot(request)
    }

    fn stop(&self, handle: &ProcessHandle) -> Result<(), ProcessError> {
        self.runner.stop(handle)
    }

    fn set_exit_handler(&self, handler: Option<Arc<dyn ProcessExitHandler>>) {
        self.runner.set_exit_handler(handler);
    }
}

#[derive(Default)]
struct FakeRouter {
    maps: Mutex<Vec<Vec<u16>>>,
    unmaps: Mutex<Vec<Vec<u16>>>,
    /// While set, `map` answers only once this is notified.
    held: Mutex<Option<Arc<Notify>>>,
}

impl PortMapper for FakeRouter {
    fn map(
        &self,
        ports: Vec<u16>,
        _lease_seconds: u32,
    ) -> BoxFuture<'static, Result<PortMappingResult, PortMappingError>> {
        self.maps.lock().expect("maps").push(ports.clone());
        let held = self.held.lock().expect("held").clone();
        Box::pin(async move {
            if let Some(release) = held {
                release.notified().await;
            }
            Ok(PortMappingResult {
                external_address: "203.0.113.7".parse().ok(),
                mapped: ports,
                refused: Vec::new(),
            })
        })
    }

    fn unmap(&self, ports: Vec<u16>) -> BoxFuture<'static, Result<(), PortMappingError>> {
        self.unmaps.lock().expect("unmaps").push(ports);
        Box::pin(async { Ok(()) })
    }
}

#[derive(Default)]
struct FakeNetwork {
    busy: Mutex<HashSet<u16>>,
}

impl LocalNetwork for FakeNetwork {
    fn interface_addresses(&self) -> Vec<InterfaceAddress> {
        vec![InterfaceAddress {
            interface: "en0".to_string(),
            address: "192.168.1.20".parse().expect("address"),
        }]
    }

    fn port_available(&self, port: u16) -> bool {
        !self.busy.lock().expect("busy").contains(&port)
    }
}

struct NoTunnel;

impl HostTunnelState for NoTunnel {
    fn tunnel_active(&self) -> BoxFuture<'static, bool> {
        Box::pin(async { false })
    }
}

struct Fixture {
    database: Database,
    manager: SelfHostManager,
    runner: RecordingRunner,
    spawn_gate: Arc<SpawnGate>,
    sink: Arc<RecordingSink>,
    probe: Arc<FakeProbe>,
    router: Arc<FakeRouter>,
    network: Arc<FakeNetwork>,
    paths: AppPaths,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_restart_delay(Duration::from_millis(2)).await
    }

    async fn with_restart_delay(initial: Duration) -> Self {
        let paths = AppPaths::new(
            std::env::temp_dir().join(format!("voyavpn-self-host-{}", uuid::Uuid::new_v4())),
        );
        let seed_root = seed_core(&paths);
        let runner = RecordingRunner::default();
        let spawn_gate = Arc::new(SpawnGate::default());
        let sink = Arc::new(RecordingSink::default());
        let probe = Arc::new(FakeProbe {
            ipv4: Mutex::new("203.0.113.7".parse().ok()),
            reachable: true,
        });
        let router = Arc::new(FakeRouter::default());
        let network = Arc::new(FakeNetwork::default());
        let deps = SelfHostDeps {
            paths: paths.clone(),
            core_seed_resource_dir: Some(seed_root),
            runner: Arc::new(GatedRunner {
                runner: runner.clone(),
                gate: Arc::clone(&spawn_gate),
            }),
            target_os: TargetOs::Linux,
            probe: Arc::clone(&probe) as Arc<dyn ReachabilityProbe>,
            port_mapper: Arc::clone(&router) as Arc<dyn PortMapper>,
            firewall: FirewallService::new(Arc::new(RecordingRunner::default()), TargetOs::Linux),
            network: Arc::clone(&network) as Arc<dyn LocalNetwork>,
            host_tunnel: Arc::new(NoTunnel),
            sink: Arc::clone(&sink) as Arc<dyn SelfHostEventSink>,
            self_tester: Arc::new(PassingSelfTest),
            log_level: "warning".to_string(),
            restart_backoff: RestartBackoff {
                initial,
                max: initial * 4,
            },
        };
        let database = Database::connect_in_memory().await.expect("database");
        Self {
            manager: SelfHostManager::spawn(database.clone(), deps),
            database,
            runner,
            spawn_gate,
            sink,
            probe,
            router,
            network,
            paths,
        }
    }

    fn self_host_spawns(&self) -> usize {
        self.runner
            .spawns()
            .iter()
            .filter(|spawn| spawn.role == ProcessRole::SelfHost)
            .count()
    }

    fn config_path(&self) -> PathBuf {
        self.paths.bin_config_file("configSelfHost.json")
    }

    fn notices(&self) -> Vec<NoticeCode> {
        self.sink.notices.lock().expect("notices").clone()
    }

    fn map_calls(&self) -> usize {
        self.router.maps.lock().expect("maps").len()
    }

    /// Reads the state until `done` holds. Every read must answer promptly:
    /// the page polls it while the node starts and checks.
    async fn wait_for_state(&self, done: impl Fn(&SelfHostState) -> bool) -> SelfHostState {
        for _ in 0..500 {
            let state = tokio::time::timeout(Duration::from_secs(1), self.manager.state())
                .await
                .expect("the state answers at once")
                .expect("state");
            if done(&state) {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        panic!("the state never reached the expected shape");
    }
}

fn seed_core(paths: &AppPaths) -> PathBuf {
    let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
    let seed = seed_root
        .join(CORE_DIR_NAME)
        .join(executable_name_for_current_os("sing-box"));
    fs::create_dir_all(seed.parent().expect("seed dir")).expect("seed dir");
    fs::write(&seed, b"seed-sing-box").expect("seed");
    seed_root
}

async fn wait_for_spawns(fixture: &Fixture, count: usize) {
    for _ in 0..500 {
        if fixture.self_host_spawns() >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!(
        "expected {count} self-hosted spawns, saw {}",
        fixture.self_host_spawns()
    );
}

async fn exit(fixture: &Fixture) {
    let pid = fixture
        .runner
        .spawns()
        .len()
        .checked_add(9)
        .and_then(|pid| u32::try_from(pid).ok())
        .expect("pid");
    fixture
        .manager
        .handle_exit(ProcessExit {
            process_id: pid,
            role: ProcessRole::SelfHost,
            exit_code: Some(1),
        })
        .await;
}

#[tokio::test]
async fn enabling_mints_an_identity_and_starts_the_core() {
    let fixture = Fixture::new().await;
    let initial = fixture.manager.state().await.expect("state");
    assert_eq!(initial.runtime.status, SelfHostRuntimeStatus::Stopped);
    assert_eq!(initial.config.vless_port, 0);
    assert!(initial.share_links.is_empty());

    let state = fixture.manager.set_enabled(true).await.expect("enable");
    assert_eq!(state.runtime.status, SelfHostRuntimeStatus::Running);
    assert!(state.config.vless_port >= 1024);
    assert_ne!(state.config.vless_port, state.config.shadowsocks_port);
    assert_eq!(fixture.self_host_spawns(), 1);

    let config = fs::read_to_string(fixture.config_path()).expect("config written");
    assert!(config.contains("\"selfhost-vless\""));
    assert!(config.contains(&format!("\"listen_port\": {}", state.config.vless_port)));
    assert!(config.contains("\"level\": \"warn\""));

    // Ports and keys stick: re-enabling reuses them.
    let again = fixture
        .manager
        .set_enabled(true)
        .await
        .expect("enable again");
    assert_eq!(again.config.vless_port, state.config.vless_port);
    assert_eq!(
        fixture.self_host_spawns(),
        1,
        "a running node is not restarted"
    );
}

#[tokio::test]
async fn disabling_stops_the_core_and_removes_its_config() {
    let fixture = Fixture::new().await;
    fixture.manager.set_enabled(true).await.expect("enable");
    let state = fixture.manager.set_enabled(false).await.expect("disable");
    assert_eq!(state.runtime.status, SelfHostRuntimeStatus::Stopped);
    assert_eq!(fixture.runner.stops().len(), 1);
    assert!(!fixture.config_path().exists());
    assert!(fixture
        .sink
        .logs
        .lock()
        .expect("logs")
        .contains(&LogCode::SelfHostStopped));
}

#[tokio::test]
async fn a_busy_port_fails_the_start_without_spawning() {
    let fixture = Fixture::new().await;
    let state = fixture.manager.set_enabled(true).await.expect("enable");
    fixture.manager.set_enabled(false).await.expect("disable");
    fixture
        .network
        .busy
        .lock()
        .expect("busy")
        .insert(state.config.vless_port);

    let failed = fixture.manager.set_enabled(true).await.expect("enable");
    assert_eq!(failed.runtime.status, SelfHostRuntimeStatus::Failed);
    assert_eq!(failed.runtime.problem, Some(SelfHostProblem::PortInUse));
    assert_eq!(failed.runtime.port, Some(state.config.vless_port));
    assert_eq!(fixture.self_host_spawns(), 1);
}

#[tokio::test]
async fn only_core_settings_restart_a_running_node() {
    let fixture = Fixture::new().await;
    let state = fixture.manager.set_enabled(true).await.expect("enable");

    let cosmetic = SelfHostConfig {
        device_label: "Tokyo".to_string(),
        ..state.config.clone()
    };
    fixture
        .manager
        .save_config(cosmetic.clone())
        .await
        .expect("save");
    assert_eq!(fixture.self_host_spawns(), 1);

    let lan = SelfHostConfig {
        allow_lan_access: true,
        ..cosmetic
    };
    fixture.manager.save_config(lan).await.expect("save");
    assert_eq!(fixture.self_host_spawns(), 2);
    assert_eq!(fixture.runner.stops().len(), 1);
}

#[tokio::test]
async fn invalid_settings_are_rejected_before_anything_changes() {
    let fixture = Fixture::new().await;
    let error = fixture
        .manager
        .save_config(SelfHostConfig {
            enabled: true,
            vless_port: 80,
            ..SelfHostConfig::default()
        })
        .await
        .expect_err("port 80 is privileged");
    assert!(
        matches!(error, SelfHostError::Validation(ref issues) if issues[0].field == "vlessPort")
    );
    assert_eq!(fixture.self_host_spawns(), 0);
}

#[tokio::test]
async fn a_crashed_core_is_restarted_and_then_given_up_on() {
    let fixture = Fixture::new().await;
    fixture.manager.set_enabled(true).await.expect("enable");

    exit(&fixture).await;
    // The restart may already have happened, so the log is what is checked.
    assert!(fixture
        .sink
        .logs
        .lock()
        .expect("logs")
        .contains(&LogCode::SelfHostRetryScheduled {
            attempt: 1,
            delay_ms: 2,
        }));

    for attempt in 2..=6 {
        wait_for_spawns(&fixture, attempt).await;
        exit(&fixture).await;
    }

    let failed = fixture.manager.state().await.expect("state");
    assert_eq!(failed.runtime.status, SelfHostRuntimeStatus::Failed);
    assert_eq!(failed.runtime.problem, Some(SelfHostProblem::CoreExited));
    assert!(fixture.notices().contains(&NoticeCode::SelfHostGaveUp));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(fixture.self_host_spawns(), 6, "a given-up node stays down");
}

#[tokio::test]
async fn a_user_change_cancels_a_pending_restart() {
    // Long enough that the retry cannot fire before the user acts.
    let fixture = Fixture::with_restart_delay(Duration::from_millis(300)).await;
    fixture.manager.set_enabled(true).await.expect("enable");
    exit(&fixture).await;
    let retrying = fixture.manager.state().await.expect("state");
    assert_eq!(retrying.runtime.status, SelfHostRuntimeStatus::Retrying);
    assert_eq!(retrying.runtime.problem, Some(SelfHostProblem::CoreExited));
    assert!(
        !fixture.config_path().exists(),
        "a dead core's config is removed"
    );
    fixture.manager.set_enabled(false).await.expect("disable");
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(fixture.self_host_spawns(), 1);
}

#[tokio::test]
async fn the_network_check_maps_ports_and_fills_the_links() {
    let fixture = Fixture::new().await;
    let enabled = fixture.manager.set_enabled(true).await.expect("enable");
    let state = fixture
        .manager
        .run_environment_check()
        .await
        .expect("check");
    let environment = state.environment.expect("report");
    assert_eq!(
        environment.ipv4.public_address.as_deref(),
        Some("203.0.113.7")
    );
    assert_eq!(
        environment.ipv4.reachability,
        SelfHostReachability::Reachable
    );
    assert_eq!(
        environment.port_mapping.status,
        SelfHostPortMappingStatus::Mapped
    );
    assert_eq!(environment.self_test.vless, SelfHostSelfTestResult::Passed);
    assert_eq!(
        environment.self_test.shadowsocks,
        SelfHostSelfTestResult::Passed
    );
    assert!(!environment.probe_available || environment.ipv4.verified_by_probe);
    assert_eq!(state.share_links.len(), 2, "one link per protocol on IPv4");
    assert!(state.share_links[0].link.contains("203.0.113.7"));
    let maps = fixture.router.maps.lock().expect("maps").clone();
    assert!(maps
        .iter()
        .all(|ports| ports == &vec![enabled.config.vless_port, enabled.config.shadowsocks_port]));

    fixture.manager.set_enabled(false).await.expect("disable");
    assert!(
        !fixture.router.unmaps.lock().expect("unmaps").is_empty(),
        "stopping removes the router forward"
    );
}

#[tokio::test]
async fn a_moved_public_address_is_announced_once() {
    let fixture = Fixture::new().await;
    fixture.manager.set_enabled(true).await.expect("enable");
    fixture.manager.periodic_check().await.expect("check");
    assert!(!fixture
        .notices()
        .contains(&NoticeCode::SelfHostAddressChanged));

    *fixture.probe.ipv4.lock().expect("address") = "198.51.100.9".parse().ok();
    fixture.manager.periodic_check().await.expect("check");
    fixture.manager.periodic_check().await.expect("check");
    let announced = fixture
        .notices()
        .into_iter()
        .filter(|notice| *notice == NoticeCode::SelfHostAddressChanged)
        .count();
    assert_eq!(announced, 1);
    let state = fixture.manager.state().await.expect("state");
    assert!(state.share_links[0].link.contains("198.51.100.9"));
}

#[tokio::test]
async fn rotating_credentials_changes_every_link() {
    let fixture = Fixture::new().await;
    fixture.manager.set_enabled(true).await.expect("enable");
    let before = fixture
        .manager
        .run_environment_check()
        .await
        .expect("check");
    let after = fixture.manager.rotate_credentials().await.expect("rotate");
    assert_eq!(after.share_links.len(), before.share_links.len());
    for (old, new) in before.share_links.iter().zip(&after.share_links) {
        assert_ne!(old.link, new.link);
        assert_eq!(old.port, new.port, "rotation keeps the ports");
    }
    assert_eq!(fixture.self_host_spawns(), 2);
}

#[tokio::test]
async fn stats_are_zero_while_stopped_and_shutdown_stops_the_core() {
    let fixture = Fixture::new().await;
    let stats = fixture.manager.stats().await.expect("stats");
    assert_eq!(stats.active_connections, 0);

    fixture.manager.set_enabled(true).await.expect("enable");
    fixture.manager.shutdown().await;
    assert_eq!(fixture.runner.stops().len(), 1);
    assert!(!fixture.config_path().exists());
    let state = fixture.manager.state().await.expect("state");
    assert_eq!(state.runtime.status, SelfHostRuntimeStatus::Stopped);
    assert!(
        state.config.enabled,
        "quitting keeps the node enabled for next launch"
    );
}

#[tokio::test]
async fn the_state_reads_starting_while_the_core_spawns() {
    let fixture = Fixture::new().await;
    fixture.spawn_gate.set_closed(true);
    let manager = fixture.manager.clone();
    let enabling = tokio::spawn(async move { manager.set_enabled(true).await });

    fixture
        .wait_for_state(|state| state.runtime.status == SelfHostRuntimeStatus::Starting)
        .await;
    assert_eq!(fixture.self_host_spawns(), 0, "the spawn is still held");

    fixture.spawn_gate.set_closed(false);
    let running = enabling.await.expect("enable task").expect("enable");
    assert_eq!(running.runtime.status, SelfHostRuntimeStatus::Running);
    assert_eq!(fixture.self_host_spawns(), 1);
}

#[tokio::test]
async fn a_check_request_during_a_check_shares_its_result() {
    let fixture = Fixture::new().await;
    fixture.manager.set_enabled(true).await.expect("enable");
    // The start asks the watch loop for a check; let that one finish first.
    fixture
        .wait_for_state(|state| state.environment.is_some())
        .await;
    let before = fixture.map_calls();

    let release = Arc::new(Notify::new());
    *fixture.router.held.lock().expect("held") = Some(Arc::clone(&release));
    let manager = fixture.manager.clone();
    let first = tokio::spawn(async move { manager.run_environment_check().await });
    for _ in 0..500 {
        if fixture.map_calls() > before {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert_eq!(
        fixture.map_calls(),
        before + 1,
        "the first check is running"
    );

    let manager = fixture.manager.clone();
    let second = tokio::spawn(async move { manager.run_environment_check().await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    // A second check, if one ran, must not wait on the router too.
    fixture.router.held.lock().expect("held").take();
    release.notify_one();

    first.await.expect("first task").expect("first check");
    let shared = second.await.expect("second task").expect("second check");
    assert!(shared.environment.is_some());
    assert_eq!(
        fixture.map_calls(),
        before + 1,
        "the second request waited for the first check instead of running its own"
    );
}

#[tokio::test]
async fn shutdown_joins_watchers_without_losing_the_in_memory_database() {
    let fixture = Fixture::new().await;
    let initial = fixture.manager.set_enabled(true).await.expect("enabled");
    fixture.manager.shutdown().await;
    // These reads acquire the very same singleton pool after the background
    // loops have been cancelled, exercising the former intermittent table loss.
    for _ in 0..32 {
        let record = fixture
            .database
            .self_host()
            .load()
            .await
            .expect("persisted record");
        assert_eq!(record.config, initial.config);
        tokio::task::yield_now().await;
    }
    fixture.manager.shutdown().await;
    assert_eq!(fixture.runner.stops().len(), 1);
}
