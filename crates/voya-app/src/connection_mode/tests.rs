use std::{
    fs,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use voya_db::Database;
use voya_platform::{
    coreinfo::TargetOs,
    paths::AppPaths,
    privilege::ElevationState,
    process::ProcessOutput,
    sysproxy::SystemProxyService,
    test_support::RecordingRunner,
    tun::{
        NativeTunController, NativeTunError, NativeTunProviderState, NativeTunStartRequest,
        NativeTunStatus, TunBackend,
    },
};

use super::*;

static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn config_with(sys_proxy: SysProxyType, tun: bool) -> AppConfig {
    let mut config = AppConfig::default();
    config.system_proxy_item.sys_proxy_type = sys_proxy;
    config.tun_mode_item.enable_tun = tun;
    config
}

#[test]
fn derivation_covers_every_primitive_combination() {
    for (sys_proxy, tun, expected) in [
        (
            SysProxyType::ForcedClear,
            false,
            ConnectionMode::SystemProxy,
        ),
        (SysProxyType::Unchanged, false, ConnectionMode::SystemProxy),
        (
            SysProxyType::ForcedChange,
            false,
            ConnectionMode::SystemProxy,
        ),
        (SysProxyType::ForcedClear, true, ConnectionMode::Vpn),
        (SysProxyType::Unchanged, true, ConnectionMode::Vpn),
        (SysProxyType::ForcedChange, true, ConnectionMode::Vpn),
    ] {
        assert_eq!(
            derive_connection_mode(&config_with(sys_proxy, tun)),
            expected,
            "{sys_proxy:?} tun={tun}"
        );
    }
}

#[test]
fn apply_system_proxy_selects_forced_change() {
    let mut config = config_with(SysProxyType::ForcedClear, true);
    apply_connection_mode(&mut config, ConnectionMode::SystemProxy);
    assert!(!config.tun_mode_item.enable_tun);
    assert_eq!(
        config.system_proxy_item.sys_proxy_type,
        SysProxyType::ForcedChange
    );
}

#[test]
fn apply_vpn_preserves_stored_system_proxy_type() {
    let mut config = config_with(SysProxyType::ForcedChange, false);
    apply_connection_mode(&mut config, ConnectionMode::Vpn);
    assert!(config.tun_mode_item.enable_tun);
    assert_eq!(
        config.system_proxy_item.sys_proxy_type,
        SysProxyType::ForcedChange
    );
}

#[test]
fn round_trip_apply_then_derive_is_stable() {
    for mode in [ConnectionMode::SystemProxy, ConnectionMode::Vpn] {
        let mut config = AppConfig::default();
        apply_connection_mode(&mut config, mode);
        assert_eq!(derive_connection_mode(&config), mode);
    }
}

#[test]
fn status_reports_the_derived_mode_and_process_rule_effect() {
    let config = config_with(SysProxyType::ForcedChange, false);
    let tun = disabled_tun_status();

    let status = connection_mode_status(&config, &tun);
    assert_eq!(status.mode, ConnectionMode::SystemProxy);
    assert!(!status.process_rules_effective);
}

#[tokio::test]
async fn a_disconnected_mode_switch_persists_the_mode_without_touching_the_machine() {
    let harness = Harness::new().await;

    let outcome = harness
        .manager()
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::SystemProxy,
            SupervisorConnectionState::Disconnected,
        )
        .await
        .expect("a mode switch never needs a running core");

    assert_eq!(
        harness
            .coordinator
            .current_config()
            .system_proxy_item
            .sys_proxy_type,
        SysProxyType::ForcedChange,
        "the mode is always persisted"
    );
    assert!(
        !outcome.system_proxy_applied,
        "pointing the machine at a port nothing is listening on blackholes every request"
    );
    assert!(
        harness.runner.oneshots().is_empty(),
        "no OS proxy command may run while the core is down: {:?}",
        harness.runner.oneshots()
    );
    assert_eq!(
        outcome.system_proxy_status.requested_type,
        SysProxyType::ForcedChange,
        "the planned status keeps the UI truthful"
    );
    assert_eq!(outcome.status.mode, ConnectionMode::SystemProxy);
    assert!(!outcome.tun_flag_changed);
    assert_eq!(
        harness.sink.events(),
        ["sysproxy:ForcedChange", "tun:false", "tray"]
    );
}

#[tokio::test]
async fn a_connected_mode_switch_applies_the_machine_proxy() {
    let harness = Harness::new().await;

    let outcome = harness
        .manager()
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::SystemProxy,
            SupervisorConnectionState::Connected,
        )
        .await
        .expect("mode switch");

    assert!(outcome.system_proxy_applied);
    assert!(
        !harness.runner.oneshots().is_empty(),
        "a running core must get the OS proxy it expects"
    );
}

#[tokio::test]
async fn entering_vpn_without_authorization_leaves_the_configuration_alone() {
    let harness = Harness::new().await;

    let error = harness
        .manager()
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::Vpn,
            SupervisorConnectionState::Connected,
        )
        .await
        .expect_err("TUN needs an elevation grant on Linux");

    assert!(matches!(
        error,
        ConnectionModeError::Tun(TunManagerError::ElevationRequired)
    ));
    assert!(
        !harness
            .coordinator
            .current_config()
            .tun_mode_item
            .enable_tun,
        "a failed preflight must not persist the mode"
    );
    assert!(harness.sink.events().is_empty());
}

#[tokio::test]
async fn entering_vpn_reports_the_tun_flag_change_that_forces_a_restart() {
    let harness = Harness::new().await;
    harness.elevation.set_granted(true);

    let outcome = harness
        .manager()
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::Vpn,
            SupervisorConnectionState::Disconnected,
        )
        .await
        .expect("VPN mode");

    assert!(outcome.tun_flag_changed);
    assert!(outcome.tun_status.enabled);
    assert!(
        harness
            .coordinator
            .current_config()
            .tun_mode_item
            .enable_tun
    );
    assert_eq!(outcome.status.mode, ConnectionMode::Vpn);
    assert!(outcome.status.process_rules_effective);
}

#[tokio::test]
async fn a_failed_apply_rolls_the_persisted_mode_back() {
    let harness = Harness::with_runner(RecordingRunner::default().with_oneshot_output(
        ProcessOutput {
            status_code: Some(1),
            stdout: String::new(),
            stderr: "gsettings: no such schema".to_string(),
        },
    ))
    .await;

    let error = harness
        .manager()
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::SystemProxy,
            SupervisorConnectionState::Connected,
        )
        .await
        .expect_err("the desktop refused the proxy change");

    assert!(matches!(
        error,
        ConnectionModeError::SystemProxyRolledBack { .. }
    ));
    assert_eq!(
        harness
            .coordinator
            .current_config()
            .system_proxy_item
            .sys_proxy_type,
        SysProxyType::ForcedClear,
        "a mode the machine refused must not survive in the database"
    );
    assert!(
        harness.sink.events().is_empty(),
        "a rolled back transaction must not announce the new mode"
    );
}

#[test]
fn macos_offers_neither_the_system_proxy_nor_process_rules() {
    let config = config_with(SysProxyType::Unchanged, true);
    let macos = TunStatus {
        backend: voya_contracts::TunBackend::MacosPacketTunnel,
        enabled: true,
        ..disabled_tun_status()
    };

    let status = connection_mode_status(&config, &macos);
    assert_eq!(status.mode, ConnectionMode::Vpn);
    assert!(!status.system_proxy_available);
    assert!(!status.process_rules_supported);
    assert!(
        !status.process_rules_effective,
        "the NetworkExtension tunnel cannot match processes even in VPN mode"
    );

    let linux = connection_mode_status(&config, &disabled_tun_status());
    assert!(linux.system_proxy_available);
    assert!(linux.process_rules_supported);
    assert!(linux.process_rules_effective);
}

#[test]
fn fresh_installs_start_in_the_native_vpn_where_one_ships() {
    for (target_os, expected) in [
        (TargetOs::Windows, true),
        (TargetOs::Macos, true),
        (TargetOs::Linux, false),
    ] {
        let mut config = AppConfig::default();
        seed_platform_connection_defaults(&mut config, target_os);
        assert_eq!(config.tun_mode_item.enable_tun, expected, "{target_os:?}");
    }
}

#[test]
fn macos_configurations_always_load_in_vpn_mode() {
    let mut config = config_with(SysProxyType::ForcedChange, false);
    assert!(enforce_platform_connection_mode(
        &mut config,
        TargetOs::Macos
    ));
    assert!(config.tun_mode_item.enable_tun);
    assert_eq!(
        config.system_proxy_item.sys_proxy_type,
        SysProxyType::Unchanged
    );
    assert!(!enforce_platform_connection_mode(
        &mut config,
        TargetOs::Macos
    ));

    for target_os in [TargetOs::Windows, TargetOs::Linux] {
        let mut config = config_with(SysProxyType::ForcedChange, false);
        assert!(!enforce_platform_connection_mode(&mut config, target_os));
        assert!(!config.tun_mode_item.enable_tun, "{target_os:?}");
    }
}

#[tokio::test]
async fn leaving_vpn_mode_is_refused_on_macos_without_touching_anything() {
    let harness = Harness::new().await;

    let error = harness
        .manager_for(TargetOs::Macos)
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::SystemProxy,
            SupervisorConnectionState::Connected,
        )
        .await
        .expect_err("macOS has no system proxy mode");

    assert!(matches!(
        error,
        ConnectionModeError::Tun(TunManagerError::VpnRequired)
    ));
    assert_eq!(
        harness
            .coordinator
            .current_config()
            .system_proxy_item
            .sys_proxy_type,
        SysProxyType::ForcedClear,
        "a refused mode must not be persisted"
    );
    assert!(harness.runner.oneshots().is_empty());
    assert!(harness.sink.events().is_empty());
}

struct Harness {
    coordinator: ConfigMutationCoordinator,
    elevation: Arc<ElevationState>,
    paths: AppPaths,
    runner: Arc<RecordingRunner>,
    sink: RecordingSink,
}

impl Harness {
    async fn new() -> Self {
        Self::with_runner(RecordingRunner::default()).await
    }

    async fn with_runner(runner: RecordingRunner) -> Self {
        let database = Database::connect_in_memory()
            .await
            .expect("in-memory database");
        let paths = temp_paths();
        paths.ensure_dirs().expect("runtime dirs");

        Self {
            coordinator: ConfigMutationCoordinator::new(
                database,
                Arc::new(RwLock::new(config_with(SysProxyType::ForcedClear, false))),
            ),
            elevation: Arc::new(ElevationState::new()),
            paths,
            runner: Arc::new(runner),
            sink: RecordingSink::default(),
        }
    }

    fn manager(&self) -> ConnectionModeManager {
        self.manager_for(TargetOs::Linux)
    }

    fn manager_for(&self, target_os: TargetOs) -> ConnectionModeManager {
        ConnectionModeManager::new(
            SystemProxyManager::with_target_os(
                SystemProxyService::new(Arc::clone(&self.runner) as Arc<_>),
                self.paths.clone(),
                target_os,
            ),
            TunManager::with_target_os_and_native_tun(
                Arc::clone(&self.elevation),
                target_os,
                Arc::new(StoppedNativeTun),
            ),
            Arc::new(self.sink.clone()),
        )
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.paths.app_dir());
    }
}

#[derive(Clone, Default)]
struct RecordingSink(Arc<Mutex<Vec<String>>>);

impl RecordingSink {
    fn events(&self) -> Vec<String> {
        self.0.lock().expect("connection mode events").clone()
    }

    fn push(&self, event: impl Into<String>) {
        self.0
            .lock()
            .expect("connection mode events")
            .push(event.into());
    }
}

impl ConnectionModeSink for RecordingSink {
    fn system_proxy_changed(&self, status: &SystemProxyStatus) {
        self.push(format!("sysproxy:{:?}", status.effective_type));
    }

    fn tun_changed(&self, status: &TunStatus) {
        self.push(format!("tun:{}", status.enabled));
    }

    fn tray_refresh(&self) {
        self.push("tray");
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

fn disabled_tun_status() -> TunStatus {
    TunManager::with_target_os_and_native_tun(
        Arc::new(ElevationState::new()),
        TargetOs::Linux,
        Arc::new(StoppedNativeTun),
    )
    .status(&AppConfig::default())
    .expect("TUN status")
}

fn temp_paths() -> AppPaths {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let counter = TEMP_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
    AppPaths::new(std::env::temp_dir().join(format!(
        "voyavpn-connection-mode-tests/{}-{nanos}-{counter}",
        std::process::id()
    )))
}
