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
    paths::AppPaths,
    privilege::ElevationState,
    process::ProcessOutput,
    sysproxy::{PacManager, PacStartConfig, SystemProxyError, SystemProxyService},
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
    for (sys_proxy, tun, expected_mode, expected_pac) in [
        (
            SysProxyType::ForcedClear,
            false,
            ConnectionMode::ProxyOnly,
            false,
        ),
        (
            SysProxyType::Unchanged,
            false,
            ConnectionMode::ProxyOnly,
            false,
        ),
        (
            SysProxyType::ForcedChange,
            false,
            ConnectionMode::SystemProxy,
            false,
        ),
        (SysProxyType::Pac, false, ConnectionMode::SystemProxy, true),
        (SysProxyType::ForcedClear, true, ConnectionMode::Vpn, false),
        (SysProxyType::Pac, true, ConnectionMode::Vpn, true),
    ] {
        let (mode, pac) = derive_connection_mode(&config_with(sys_proxy, tun));
        assert_eq!(
            (mode, pac),
            (expected_mode, expected_pac),
            "{sys_proxy:?} tun={tun}"
        );
    }
}

#[test]
fn apply_proxy_only_clears_both_primitives() {
    let mut config = config_with(SysProxyType::Pac, true);
    apply_connection_mode(&mut config, ConnectionMode::ProxyOnly, false);
    assert!(!config.tun_mode_item.enable_tun);
    assert_eq!(
        config.system_proxy_item.sys_proxy_type,
        SysProxyType::ForcedClear
    );
}

#[test]
fn apply_system_proxy_selects_pac_flavor() {
    let mut config = config_with(SysProxyType::ForcedClear, true);
    apply_connection_mode(&mut config, ConnectionMode::SystemProxy, true);
    assert!(!config.tun_mode_item.enable_tun);
    assert_eq!(config.system_proxy_item.sys_proxy_type, SysProxyType::Pac);

    apply_connection_mode(&mut config, ConnectionMode::SystemProxy, false);
    assert_eq!(
        config.system_proxy_item.sys_proxy_type,
        SysProxyType::ForcedChange
    );
}

#[test]
fn apply_vpn_preserves_stored_system_proxy_type() {
    let mut config = config_with(SysProxyType::ForcedChange, false);
    apply_connection_mode(&mut config, ConnectionMode::Vpn, false);
    assert!(config.tun_mode_item.enable_tun);
    assert_eq!(
        config.system_proxy_item.sys_proxy_type,
        SysProxyType::ForcedChange
    );
}

#[test]
fn round_trip_apply_then_derive_is_stable() {
    for (mode, pac) in [
        (ConnectionMode::ProxyOnly, false),
        (ConnectionMode::SystemProxy, false),
        (ConnectionMode::SystemProxy, true),
        (ConnectionMode::Vpn, false),
    ] {
        let mut config = AppConfig::default();
        apply_connection_mode(&mut config, mode, pac);
        let (derived_mode, derived_pac) = derive_connection_mode(&config);
        assert_eq!(derived_mode, mode);
        if mode == ConnectionMode::SystemProxy {
            assert_eq!(derived_pac, pac);
        }
    }
}

#[test]
fn status_reports_platform_pac_availability_from_the_platform_rule() {
    let config = config_with(SysProxyType::ForcedChange, false);
    let tun = disabled_tun_status();

    assert!(!connection_mode_status(&config, &tun, TargetOs::Linux).pac_available);
    assert!(connection_mode_status(&config, &tun, TargetOs::Macos).pac_available);
    assert!(connection_mode_status(&config, &tun, TargetOs::Windows).pac_available);
    assert_eq!(
        connection_mode_status(&config, &tun, TargetOs::Linux).mode,
        ConnectionMode::SystemProxy
    );
    assert!(!connection_mode_status(&config, &tun, TargetOs::Linux).process_rules_effective);
}

#[tokio::test]
async fn a_disconnected_mode_switch_persists_the_mode_without_touching_the_machine() {
    let harness = Harness::new().await;

    let outcome = harness
        .manager()
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::SystemProxy,
            None,
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
            None,
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
async fn pac_on_a_platform_without_pac_is_rejected_before_anything_is_written() {
    let harness = Harness::new().await;

    let error = harness
        .manager()
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::SystemProxy,
            Some(true),
            SupervisorConnectionState::Connected,
        )
        .await
        .expect_err("PAC is Windows/macOS only");

    assert!(matches!(
        error,
        ConnectionModeError::SystemProxy(SystemProxyManagerError::PacUnavailable(TargetOs::Linux))
    ));
    assert_eq!(
        harness
            .coordinator
            .current_config()
            .system_proxy_item
            .sys_proxy_type,
        SysProxyType::ForcedClear
    );
    assert!(harness.runner.oneshots().is_empty());
    assert!(harness.sink.events().is_empty());
}

#[tokio::test]
async fn entering_vpn_without_authorization_leaves_the_configuration_alone() {
    let harness = Harness::new().await;

    let error = harness
        .manager()
        .set_connection_mode(
            &harness.coordinator,
            ConnectionMode::Vpn,
            None,
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
            None,
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
            None,
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

#[tokio::test]
async fn setting_only_the_system_proxy_flavor_persists_while_disconnected() {
    let harness = Harness::new().await;

    let status = harness
        .manager()
        .set_system_proxy_mode(
            &harness.coordinator,
            SysProxyType::ForcedChange,
            SupervisorConnectionState::Disconnected,
        )
        .await
        .expect("system proxy mode");

    assert_eq!(status.requested_type, SysProxyType::ForcedChange);
    assert_eq!(
        harness
            .coordinator
            .current_config()
            .system_proxy_item
            .sys_proxy_type,
        SysProxyType::ForcedChange
    );
    assert!(harness.runner.oneshots().is_empty());
    assert!(
        !harness
            .coordinator
            .current_config()
            .tun_mode_item
            .enable_tun,
        "the system proxy flavor never touches TUN"
    );
    assert_eq!(harness.sink.events(), ["sysproxy:ForcedChange", "tray"]);
}

#[tokio::test]
async fn setting_pac_as_the_system_proxy_flavor_is_rejected_off_platform() {
    let harness = Harness::new().await;

    let error = harness
        .manager()
        .set_system_proxy_mode(
            &harness.coordinator,
            SysProxyType::Pac,
            SupervisorConnectionState::Connected,
        )
        .await
        .expect_err("PAC is Windows/macOS only");

    assert!(matches!(
        error,
        ConnectionModeError::SystemProxy(SystemProxyManagerError::PacUnavailable(TargetOs::Linux))
    ));
    assert_eq!(
        harness
            .coordinator
            .current_config()
            .system_proxy_item
            .sys_proxy_type,
        SysProxyType::ForcedClear
    );
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
                Arc::new(RwLock::new(AppConfig::default())),
            ),
            elevation: Arc::new(ElevationState::new()),
            paths,
            runner: Arc::new(runner),
            sink: RecordingSink::default(),
        }
    }

    fn manager(&self) -> ConnectionModeManager {
        ConnectionModeManager::with_target_os(
            SystemProxyManager::with_target_os(
                SystemProxyService::new(Arc::clone(&self.runner) as Arc<_>, Arc::new(SilentPac)),
                self.paths.clone(),
                TargetOs::Linux,
            ),
            TunManager::with_target_os_and_native_tun(
                Arc::clone(&self.elevation),
                TargetOs::Linux,
                Arc::new(StoppedNativeTun),
            ),
            Arc::new(self.sink.clone()),
            TargetOs::Linux,
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

struct SilentPac;

impl PacManager for SilentPac {
    fn start(&self, _config: PacStartConfig) -> Result<(), SystemProxyError> {
        Ok(())
    }

    fn stop(&self) {}

    fn is_supported(&self) -> bool {
        false
    }
    fn is_running(&self) -> bool {
        true
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
