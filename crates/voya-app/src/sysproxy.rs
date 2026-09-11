use std::{
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use voya_core::{AppConfig, InboundProtocol, SysProxyType};
use voya_platform::{
    coreinfo::TargetOs,
    filesystem,
    paths::{AppPaths, PathError},
    sysproxy::{
        system_proxy_management, SystemProxyError, SystemProxyManagement, SystemProxyObservation,
        SystemProxyRequest, SystemProxyService, SystemProxyStatus,
    },
    tun::{tun_backend, TunBackend},
};

const SYSPROXY_SCRIPT_DIR_NAME: &str = "sysproxy";
const SYSPROXY_DIRTY_MARKER_FILE_NAME: &str = "proxy-dirty";
const SYSPROXY_DIRTY_MARKER_CONTENTS: &[u8] = b"dirty\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualProxyExitWarning {
    LocalProxy,
    Unknown,
}

#[derive(Clone)]
pub struct SystemProxyManager {
    service: SystemProxyService,
    paths: AppPaths,
    target_os: TargetOs,
}

impl SystemProxyManager {
    #[must_use]
    pub fn new(service: SystemProxyService, paths: AppPaths) -> Self {
        Self::with_target_os(service, paths, TargetOs::current())
    }

    #[must_use]
    pub const fn with_target_os(
        service: SystemProxyService,
        paths: AppPaths,
        target_os: TargetOs,
    ) -> Self {
        Self {
            service,
            paths,
            target_os,
        }
    }

    /// Apply the connected core's effective proxy policy for this platform.
    pub fn apply_runtime_config(
        &self,
        config: &AppConfig,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        let runtime = runtime_system_proxy_config(config, false, self.target_os);
        self.apply_config(&runtime.config, runtime.force_disable)
    }

    /// Inspect the effective runtime policy without changing the OS proxy.
    pub fn runtime_status(
        &self,
        config: &AppConfig,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        let runtime = runtime_system_proxy_config(config, false, self.target_os);
        self.status_with_force_disable(&runtime.config, runtime.force_disable)
    }

    pub fn status(&self, config: &AppConfig) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        self.status_with_force_disable(config, false)
    }

    pub fn status_with_force_disable(
        &self,
        config: &AppConfig,
        force_disable: bool,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        let request = self.request(config, force_disable)?;
        let mut status = self.service.status(&request)?;
        Self::decorate_manual_status(&mut status);
        Ok(status)
    }

    pub fn apply_config(
        &self,
        config: &AppConfig,
        force_disable: bool,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        let request = self.request(config, force_disable)?;

        if request_sets_local_proxy(&request) {
            self.write_dirty_marker()?;
        }

        let mut status = self.service.apply(&request)?;
        Self::decorate_manual_status(&mut status);
        if status.management == SystemProxyManagement::Automatic
            && status.effective_type == SysProxyType::ForcedClear
        {
            self.clear_dirty_marker()?;
        }

        Ok(status)
    }

    pub fn restore(
        &self,
        config: &AppConfig,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        self.apply_config(config, true)
    }

    pub fn restore_dirty_proxy_if_needed(
        &self,
        config: &AppConfig,
    ) -> Result<bool, SystemProxyManagerError> {
        if system_proxy_management(self.target_os) == SystemProxyManagement::Manual {
            return Ok(false);
        }
        if !self.dirty_marker_exists()? {
            return Ok(false);
        }

        let mut request = self.request(config, false)?;
        request.item.sys_proxy_type = SysProxyType::ForcedClear;
        let mut status = self.service.apply(&request)?;
        Self::decorate_manual_status(&mut status);
        if status.management == SystemProxyManagement::Automatic
            && status.effective_type == SysProxyType::ForcedClear
        {
            self.clear_dirty_marker()?;
        }

        Ok(true)
    }

    fn decorate_manual_status(status: &mut SystemProxyStatus) {
        if status.management == SystemProxyManagement::Manual {
            status.manual_cleanup_required = matches!(
                status.observation,
                SystemProxyObservation::LocalProxy | SystemProxyObservation::Unknown
            );
        }
    }

    /// Every exit request observes current system settings before prompting.
    pub fn manual_exit_warning(
        &self,
        config: &AppConfig,
    ) -> Result<Option<ManualProxyExitWarning>, SystemProxyManagerError> {
        let status = self.status(config)?;
        if status.management != SystemProxyManagement::Manual {
            return Ok(None);
        }
        Ok(match status.observation {
            SystemProxyObservation::LocalProxy => Some(ManualProxyExitWarning::LocalProxy),
            SystemProxyObservation::Unknown => Some(ManualProxyExitWarning::Unknown),
            SystemProxyObservation::Clear | SystemProxyObservation::OtherProxy => None,
        })
    }

    pub fn open_network_settings(&self) -> Result<(), SystemProxyManagerError> {
        voya_platform::sysproxy::open_network_settings().map_err(Into::into)
    }

    pub fn stop_pac(&self) {
        self.service.stop_pac();
    }

    /// A failed operation/read cannot advertise an endpoint or claim the OS
    /// proxy was restored. This fallback itself performs no fallible I/O.
    pub fn unavailable_status(&self, config: &AppConfig) -> SystemProxyStatus {
        let management = system_proxy_management(self.target_os);
        SystemProxyStatus {
            management,
            observation: SystemProxyObservation::Unknown,
            manual_cleanup_required: management == SystemProxyManagement::Manual,
            requested_type: config.system_proxy_item.sys_proxy_type,
            effective_type: SysProxyType::Unchanged,
            target_os: self.target_os,
            pac_available: voya_platform::sysproxy::pac_available(self.target_os),
            proxy: None,
            exceptions: String::new(),
            pac_url: None,
        }
    }

    fn request(
        &self,
        config: &AppConfig,
        force_disable: bool,
    ) -> Result<SystemProxyRequest, SystemProxyManagerError> {
        self.paths.ensure_dirs()?;
        let socks_port = config
            .inbound
            .first()
            .map_or(voya_core::DEFAULT_LOCAL_PORT, |inbound| inbound.local_port);
        let pac_port = socks_port + InboundProtocol::pac.port_offset();

        Ok(SystemProxyRequest {
            target_os: self.target_os,
            item: config.system_proxy_item.clone(),
            force_disable,
            socks_port,
            pac_port,
            config_dir: self.paths.config_dir().to_path_buf(),
            script_dir: self.paths.temp_dir().join(SYSPROXY_SCRIPT_DIR_NAME),
            pac_url_nonce: current_tick_string(),
        })
    }

    fn dirty_marker_path(&self) -> PathBuf {
        self.paths.config_file(SYSPROXY_DIRTY_MARKER_FILE_NAME)
    }

    fn dirty_marker_exists(&self) -> Result<bool, SystemProxyManagerError> {
        let path = self.dirty_marker_path();
        filesystem::file_exists(&path)
            .map_err(|source| SystemProxyManagerError::DirtyMarkerInspect { path, source })
    }

    fn write_dirty_marker(&self) -> Result<(), SystemProxyManagerError> {
        self.paths.ensure_dirs()?;
        let path = self.dirty_marker_path();
        filesystem::write_file_with_parent(&path, SYSPROXY_DIRTY_MARKER_CONTENTS)
            .map_err(|source| SystemProxyManagerError::DirtyMarkerWrite { path, source })
    }

    fn clear_dirty_marker(&self) -> Result<(), SystemProxyManagerError> {
        let path = self.dirty_marker_path();
        filesystem::remove_file_if_exists(&path)
            .map_err(|source| SystemProxyManagerError::DirtyMarkerRemove { path, source })
    }
}

#[derive(Debug, Error)]
pub enum SystemProxyManagerError {
    #[error("PAC mode is only available on Windows or macOS, not {0:?}")]
    PacUnavailable(TargetOs),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    SystemProxy(#[from] SystemProxyError),
    #[error("failed to inspect system proxy dirty marker {path}: {source}")]
    DirtyMarkerInspect { path: PathBuf, source: io::Error },
    #[error("failed to write system proxy dirty marker {path}: {source}")]
    DirtyMarkerWrite { path: PathBuf, source: io::Error },
    #[error("failed to remove system proxy dirty marker {path}: {source}")]
    DirtyMarkerRemove { path: PathBuf, source: io::Error },
}

#[derive(Debug, Clone)]
pub struct RuntimeSystemProxyConfig {
    pub config: AppConfig,
    pub force_disable: bool,
}

#[must_use]
pub fn runtime_system_proxy_config(
    config: &AppConfig,
    force_disable: bool,
    target_os: TargetOs,
) -> RuntimeSystemProxyConfig {
    let mut runtime = RuntimeSystemProxyConfig {
        config: config.clone(),
        force_disable,
    };

    if force_disable {
        return runtime;
    }
    if should_disable_native_tun_system_proxy(config, target_os) {
        runtime.force_disable = true;
    } else if should_apply_tun_system_proxy_fallback(config, target_os) {
        runtime.config.system_proxy_item.sys_proxy_type = SysProxyType::ForcedChange;
    }
    runtime
}

#[must_use]
pub fn should_disable_native_tun_system_proxy(config: &AppConfig, target_os: TargetOs) -> bool {
    config.tun_mode_item.enable_tun && tun_backend(target_os).is_native()
}

#[must_use]
pub fn should_apply_tun_system_proxy_fallback(config: &AppConfig, target_os: TargetOs) -> bool {
    config.tun_mode_item.enable_tun
        && config.system_proxy_item.sys_proxy_type == SysProxyType::ForcedClear
        && tun_backend(target_os) == TunBackend::Process
}

#[must_use]
pub fn runtime_proxy_url(
    prefer_proxy: bool,
    proxy_url: Option<String>,
    config: &AppConfig,
    target_os: TargetOs,
) -> Option<String> {
    let explicit = proxy_url.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });

    if !prefer_proxy {
        return explicit;
    }
    explicit.or_else(|| runtime_default_proxy_url(config, target_os))
}

#[must_use]
pub fn runtime_default_proxy_url(config: &AppConfig, target_os: TargetOs) -> Option<String> {
    if config.tun_mode_item.enable_tun && tun_backend(target_os).is_native() {
        return None;
    }

    let port = config
        .inbound
        .first()
        .map_or(voya_core::DEFAULT_LOCAL_PORT, |inbound| inbound.local_port);
    (1..=65_535)
        .contains(&port)
        .then(|| format!("http://127.0.0.1:{port}"))
}

fn current_tick_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
        .to_string()
}

fn request_sets_local_proxy(request: &SystemProxyRequest) -> bool {
    if request.force_disable
        || system_proxy_management(request.target_os) != SystemProxyManagement::Automatic
    {
        return false;
    }

    matches!(
        (request.item.sys_proxy_type, request.target_os),
        (
            SysProxyType::ForcedChange,
            TargetOs::Windows | TargetOs::Linux | TargetOs::Macos
        ) | (SysProxyType::Pac, TargetOs::Windows | TargetOs::Macos)
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use voya_platform::{
        sysproxy::{PacManager, PacStartConfig},
        test_support::RecordingRunner,
    };

    use super::*;

    struct TestObserver(SystemProxyObservation);
    impl voya_platform::sysproxy::SystemProxyObserver for TestObserver {
        fn observe(&self) -> SystemProxyObservation {
            self.0
        }
    }

    #[test]
    fn exit_recheck_warns_only_for_local_or_unverified_proxy_settings() {
        for (observation, warning) in [
            (SystemProxyObservation::Clear, None),
            (SystemProxyObservation::OtherProxy, None),
            (
                SystemProxyObservation::LocalProxy,
                Some(ManualProxyExitWarning::LocalProxy),
            ),
            (
                SystemProxyObservation::Unknown,
                Some(ManualProxyExitWarning::Unknown),
            ),
        ] {
            for legacy_marker in [false, true] {
                let app_dir = unique_app_dir("exit-recheck");
                let runner = Arc::new(RecordingRunner::default());
                let pac = Arc::new(RecordingPac::default());
                let service = SystemProxyService::new(runner.clone(), pac.clone())
                    .with_observer(Arc::new(TestObserver(observation)));
                let manager = SystemProxyManager::with_target_os(
                    service,
                    AppPaths::new(app_dir.clone()),
                    TargetOs::Macos,
                );
                if legacy_marker {
                    manager.write_dirty_marker().expect("legacy marker");
                }
                assert_eq!(
                    manager
                        .manual_exit_warning(&AppConfig::default())
                        .expect("exit recheck"),
                    warning,
                    "{observation:?}, legacy marker: {legacy_marker}",
                );
                assert_eq!(manager.dirty_marker_path().exists(), legacy_marker,);
                // Checking before confirmation must keep the connection and
                // PAC running and must never write system network settings.
                assert!(runner.oneshots().is_empty());
                assert_eq!(*pac.starts.lock().expect("starts"), 0);
                assert_eq!(*pac.stops.lock().expect("stops"), 0);
                let _ = fs::remove_dir_all(app_dir);
            }
        }
    }

    #[test]
    fn exit_rechecks_settings_again_after_the_user_returns_from_network_settings() {
        struct MutableObserver(Mutex<SystemProxyObservation>);
        impl voya_platform::sysproxy::SystemProxyObserver for MutableObserver {
            fn observe(&self) -> SystemProxyObservation {
                *self.0.lock().expect("observation")
            }
        }
        let app_dir = unique_app_dir("exit-retry");
        let observer = Arc::new(MutableObserver(Mutex::new(
            SystemProxyObservation::LocalProxy,
        )));
        let service = SystemProxyService::new(
            Arc::new(RecordingRunner::default()),
            Arc::new(RecordingPac::default()),
        )
        .with_observer(observer.clone());
        let manager = SystemProxyManager::with_target_os(
            service,
            AppPaths::new(app_dir.clone()),
            TargetOs::Macos,
        );
        let config = AppConfig::default();
        assert_eq!(
            manager.manual_exit_warning(&config).expect("first exit"),
            Some(ManualProxyExitWarning::LocalProxy),
        );
        *observer.0.lock().expect("observation") = SystemProxyObservation::Clear;
        assert_eq!(manager.manual_exit_warning(&config).expect("retry"), None);
        assert!(!manager.dirty_marker_path().exists());
        let _ = fs::remove_dir_all(app_dir);
    }

    #[test]
    fn manual_proxy_status_ignores_historical_markers() {
        for observation in [
            SystemProxyObservation::Unknown,
            SystemProxyObservation::LocalProxy,
            SystemProxyObservation::Clear,
            SystemProxyObservation::OtherProxy,
        ] {
            let app_dir = unique_app_dir("manual-recheck");
            let runner = Arc::new(RecordingRunner::default());
            let service =
                SystemProxyService::new(runner.clone(), Arc::new(RecordingPac::default()))
                    .with_observer(Arc::new(TestObserver(observation)));
            let manager = SystemProxyManager::with_target_os(
                service,
                AppPaths::new(app_dir.clone()),
                TargetOs::Macos,
            );
            let config = AppConfig::default();
            manager.write_dirty_marker().expect("legacy marker");
            assert!(!manager
                .restore_dirty_proxy_if_needed(&config)
                .expect("startup"));
            let pending = matches!(
                observation,
                SystemProxyObservation::Unknown | SystemProxyObservation::LocalProxy
            );
            for status in [manager.restore(&config), manager.status(&config)] {
                assert_eq!(
                    status.expect("observation").manual_cleanup_required,
                    pending
                );
            }
            assert_eq!(
                fs::read(manager.dirty_marker_path()).expect("untouched marker"),
                SYSPROXY_DIRTY_MARKER_CONTENTS
            );
            assert!(runner.oneshots().is_empty());
            let _ = fs::remove_dir_all(app_dir);
        }
    }

    #[derive(Default)]
    struct RecordingPac {
        starts: Mutex<u32>,
        stops: Mutex<u32>,
    }

    impl PacManager for RecordingPac {
        fn start(&self, _config: PacStartConfig) -> Result<(), SystemProxyError> {
            *self.starts.lock().expect("starts") += 1;
            Ok(())
        }

        fn stop(&self) {
            *self.stops.lock().expect("stops") += 1;
        }

        fn is_supported(&self) -> bool {
            true
        }
        fn is_running(&self) -> bool {
            true
        }
    }

    fn manager(
        target_os: TargetOs,
        runner: Arc<RecordingRunner>,
        pac: Arc<RecordingPac>,
    ) -> SystemProxyManager {
        manager_with_app_dir(target_os, runner, pac, unique_app_dir("default"))
    }

    fn manager_with_app_dir(
        target_os: TargetOs,
        runner: Arc<RecordingRunner>,
        pac: Arc<RecordingPac>,
        app_dir: PathBuf,
    ) -> SystemProxyManager {
        let service = SystemProxyService::new(runner, pac)
            .with_observer(Arc::new(TestObserver(SystemProxyObservation::Unknown)));
        SystemProxyManager::with_target_os(service, AppPaths::new(app_dir), target_os)
    }

    fn unique_app_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "voya-app-sysproxy-{name}-{}-{}",
            std::process::id(),
            current_tick_string()
        ))
    }

    #[test]
    fn sysproxy_manager_accepts_macos_pac_without_writing_dirty_marker() {
        let app_dir = unique_app_dir("macos-pac-dirty");
        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(RecordingPac::default());
        let manager =
            manager_with_app_dir(TargetOs::Macos, runner, Arc::clone(&pac), app_dir.clone());
        let mut config = AppConfig::default();
        config.system_proxy_item.sys_proxy_type = SysProxyType::Pac;

        let status = manager.apply_config(&config, false).expect("macos pac");

        assert_eq!(status.requested_type, SysProxyType::Pac);
        assert_eq!(status.effective_type, SysProxyType::Unchanged);
        assert!(!manager.dirty_marker_path().is_file());
        assert_eq!(*pac.starts.lock().expect("starts"), 1);
        let _ = fs::remove_dir_all(app_dir);
    }

    #[test]
    fn sysproxy_manager_restore_forces_clear_without_changing_requested_mode() {
        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(RecordingPac::default());
        let manager = manager(TargetOs::Windows, runner, pac);
        let mut config = AppConfig::default();
        config.system_proxy_item.sys_proxy_type = SysProxyType::ForcedChange;

        let status = manager.restore(&config).expect("restore");

        assert_eq!(status.requested_type, SysProxyType::ForcedChange);
        assert_eq!(status.effective_type, SysProxyType::ForcedClear);
        assert_eq!(
            config.system_proxy_item.sys_proxy_type,
            SysProxyType::ForcedChange
        );
    }

    #[test]
    fn sysproxy_manager_apply_sets_dirty_marker_for_local_proxy() {
        let app_dir = unique_app_dir("apply-dirty");
        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(RecordingPac::default());
        let manager = manager_with_app_dir(TargetOs::Windows, runner, pac, app_dir.clone());
        let mut config = AppConfig::default();
        config.system_proxy_item.sys_proxy_type = SysProxyType::ForcedChange;

        manager.apply_config(&config, false).expect("apply proxy");

        assert!(manager.dirty_marker_path().is_file());
        let _ = fs::remove_dir_all(app_dir);
    }

    #[test]
    fn sysproxy_manager_restore_clears_dirty_marker() {
        let app_dir = unique_app_dir("restore-clean");
        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(RecordingPac::default());
        let manager = manager_with_app_dir(TargetOs::Windows, runner, pac, app_dir.clone());
        let mut config = AppConfig::default();
        config.system_proxy_item.sys_proxy_type = SysProxyType::ForcedChange;

        manager.apply_config(&config, false).expect("apply proxy");
        manager.restore(&config).expect("restore proxy");

        assert!(!manager.dirty_marker_path().exists());
        let _ = fs::remove_dir_all(app_dir);
    }

    #[test]
    fn sysproxy_manager_startup_recovery_forces_clear_when_marker_exists() {
        let app_dir = unique_app_dir("startup-recover");
        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(RecordingPac::default());
        let manager =
            manager_with_app_dir(TargetOs::Windows, Arc::clone(&runner), pac, app_dir.clone());
        let mut config = AppConfig::default();
        config.system_proxy_item.sys_proxy_type = SysProxyType::Unchanged;
        manager.write_dirty_marker().expect("dirty marker");

        let restored = manager
            .restore_dirty_proxy_if_needed(&config)
            .expect("startup recovery");

        assert!(restored);
        assert!(!manager.dirty_marker_path().exists());
        assert_eq!(runner.oneshots().len(), 4);
        let _ = fs::remove_dir_all(app_dir);
    }

    #[test]
    fn sysproxy_manager_startup_recovery_noops_without_marker() {
        let app_dir = unique_app_dir("startup-clean");
        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(RecordingPac::default());
        let manager =
            manager_with_app_dir(TargetOs::Windows, Arc::clone(&runner), pac, app_dir.clone());
        let config = AppConfig::default();

        let restored = manager
            .restore_dirty_proxy_if_needed(&config)
            .expect("startup recovery");

        assert!(!restored);
        assert!(runner.oneshots().is_empty());
        let _ = fs::remove_dir_all(app_dir);
    }

    #[test]
    fn runtime_proxy_policy_separates_native_tun_and_process_fallback() {
        let mut config = AppConfig::default();
        config.tun_mode_item.enable_tun = true;

        let native = runtime_system_proxy_config(&config, false, TargetOs::Macos);
        assert!(native.force_disable);
        assert!(runtime_default_proxy_url(&config, TargetOs::Macos).is_none());

        let process = runtime_system_proxy_config(&config, false, TargetOs::Linux);
        assert!(!process.force_disable);
        assert_eq!(
            process.config.system_proxy_item.sys_proxy_type,
            SysProxyType::ForcedChange
        );
        assert_eq!(
            runtime_proxy_url(true, None, &config, TargetOs::Linux).as_deref(),
            Some("http://127.0.0.1:10808")
        );
    }
}
