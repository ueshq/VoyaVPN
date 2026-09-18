//! The manager against a real sing-box and the real network: a node that
//! starts, serves its own self-test client over loopback, restarts on a
//! settings change, and stops.
//!
//! Opt-in, like the golden acceptance checks: set `VOYA_SINGBOX_BIN` to a
//! sing-box binary (the seed under `apps/desktop/src-tauri/resources`) and
//! allow outbound HTTPS to www.cloudflare.com.

use std::{
    net::{Ipv4Addr, TcpStream},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use futures_util::future::BoxFuture;
use voya_contracts::{SelfHostConfig, SelfHostRuntimeStatus, SelfHostSelfTestResult};
use voya_db::Database;
use voya_net::{
    portmap::{PortMapper, PortMappingError, PortMappingResult},
    probe::{ProbeFamily, ReachabilityProbeError, ReachabilityProbeResponse},
};
use voya_platform::{
    coreinfo::{executable_name_for_current_os, TargetOs, CORE_DIR_NAME},
    firewall::FirewallService,
    paths::{core_seed_resources_dir, AppPaths},
    process::StdProcessRunner,
};

use super::{NoTunnel, RecordingSink};
use crate::self_host::{
    ProbeCoreSelfTester, ReachabilityProbe, RestartBackoff, SelfHostDeps, SelfHostEventSink,
    SelfHostManager, SystemLocalNetwork,
};

/// The probe service is not part of this test.
struct NoProbeService;

impl ReachabilityProbe for NoProbeService {
    fn probe(
        &self,
        _family: ProbeFamily,
        _ports: Vec<u16>,
    ) -> BoxFuture<'static, Result<ReachabilityProbeResponse, ReachabilityProbeError>> {
        Box::pin(async { Err(ReachabilityProbeError::Status { status: 503 }) })
    }

    fn public_address(
        &self,
        _family: ProbeFamily,
    ) -> BoxFuture<'static, Result<std::net::IpAddr, ReachabilityProbeError>> {
        Box::pin(async { Err(ReachabilityProbeError::Status { status: 503 }) })
    }
}

struct NoRouter;

impl PortMapper for NoRouter {
    fn map(
        &self,
        _ports: Vec<u16>,
        _lease_seconds: u32,
    ) -> BoxFuture<'static, Result<PortMappingResult, PortMappingError>> {
        Box::pin(async { Err(PortMappingError::NoGateway("test".to_string())) })
    }

    fn unmap(&self, _ports: Vec<u16>) -> BoxFuture<'static, Result<(), PortMappingError>> {
        Box::pin(async { Ok(()) })
    }
}

fn seed(binary: &PathBuf, paths: &AppPaths) -> PathBuf {
    let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
    let target = seed_root
        .join(CORE_DIR_NAME)
        .join(executable_name_for_current_os("sing-box"));
    std::fs::create_dir_all(target.parent().expect("seed dir")).expect("seed dir");
    std::fs::copy(binary, &target).expect("copy sing-box");
    seed_root
}

fn wait_for_listener(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("the node never listened on {port}");
}

#[tokio::test]
async fn a_live_node_serves_its_own_self_test_client() {
    let Some(binary) = std::env::var_os("VOYA_SINGBOX_BIN").map(PathBuf::from) else {
        println!("live self-hosted node test skipped: set VOYA_SINGBOX_BIN");
        return;
    };
    let paths = AppPaths::new(
        std::env::temp_dir().join(format!("voyavpn-self-host-live-{}", uuid::Uuid::new_v4())),
    );
    let seed_root = seed(&binary, &paths);
    let sink = Arc::new(RecordingSink::default());
    let deps = SelfHostDeps {
        paths: paths.clone(),
        core_seed_resource_dir: Some(seed_root),
        runner: Arc::new(StdProcessRunner::new()),
        target_os: TargetOs::current(),
        probe: Arc::new(NoProbeService),
        port_mapper: Arc::new(NoRouter),
        firewall: FirewallService::new(Arc::new(StdProcessRunner::new()), TargetOs::Linux),
        network: Arc::new(SystemLocalNetwork),
        host_tunnel: Arc::new(NoTunnel),
        sink: Arc::clone(&sink) as Arc<dyn SelfHostEventSink>,
        self_tester: Arc::new(ProbeCoreSelfTester),
        log_level: "warn".to_string(),
        restart_backoff: RestartBackoff::default(),
    };
    let manager =
        SelfHostManager::spawn(Database::connect_in_memory().await.expect("database"), deps);

    let enabled = manager.set_enabled(true).await.expect("enable");
    assert_eq!(
        enabled.runtime.status,
        SelfHostRuntimeStatus::Running,
        "{enabled:?}"
    );
    wait_for_listener(enabled.config.vless_port);
    wait_for_listener(enabled.config.shadowsocks_port);

    let checked = manager.run_environment_check().await.expect("check");
    let self_test = checked.environment.expect("report").self_test;
    assert_eq!(self_test.vless, SelfHostSelfTestResult::Passed);
    assert_eq!(self_test.shadowsocks, SelfHostSelfTestResult::Passed);

    // As of 2026-09 this site answers TLS 1.3 but cannot be borrowed by
    // REALITY: the self-test is what tells the user.
    let restarted = manager
        .save_config(SelfHostConfig {
            reality_server_name: "www.microsoft.com".to_string(),
            ..enabled.config.clone()
        })
        .await
        .expect("save");
    wait_for_listener(restarted.config.vless_port);
    let rechecked = manager.run_environment_check().await.expect("check");
    let self_test = rechecked.environment.expect("report").self_test;
    assert_eq!(self_test.vless, SelfHostSelfTestResult::Failed);
    assert_eq!(self_test.shadowsocks, SelfHostSelfTestResult::Passed);

    manager.shutdown().await;
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, enabled.config.vless_port)).is_err());
}
