use std::{io, path::PathBuf};

use thiserror::Error;
use voya_core::{AppConfig, SysProxyType};
use voya_platform::{
    coreinfo::TargetOs,
    filesystem,
    paths::{AppPaths, PathError},
    sysproxy::{
        system_proxy_management, SystemProxyError, SystemProxyManagement, SystemProxyRequest,
        SystemProxyService, SystemProxyStatus,
    },
    tun::{tun_backend, TunBackend},
};

const SYSPROXY_SCRIPT_DIR_NAME: &str = "sysproxy";
const SYSPROXY_DIRTY_MARKER_FILE_NAME: &str = "proxy-dirty";
const SYSPROXY_DIRTY_MARKER_CONTENTS: &[u8] = b"dirty\n";

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
        self.apply_request(&self.runtime_request(config)?)
    }

    /// Inspect the effective runtime policy without changing the OS proxy.
    pub fn runtime_status(
        &self,
        config: &AppConfig,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        Ok(self.service.status(&self.runtime_request(config)?)?)
    }

    fn runtime_request(
        &self,
        config: &AppConfig,
    ) -> Result<SystemProxyRequest, SystemProxyManagerError> {
        let policy = runtime_system_proxy_policy(config, self.target_os);
        let mut request = self.request(config, policy.force_disable)?;
        request.item.mode = policy.mode;
        Ok(request)
    }

    fn apply_config(
        &self,
        config: &AppConfig,
        force_disable: bool,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        self.apply_request(&self.request(config, force_disable)?)
    }

    fn apply_request(
        &self,
        request: &SystemProxyRequest,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        if request_sets_local_proxy(request) {
            self.write_dirty_marker()?;
        }

        let status = self.service.apply(request)?;
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
        if system_proxy_management(self.target_os) != SystemProxyManagement::Automatic {
            return Ok(false);
        }
        if !self.dirty_marker_exists()? {
            return Ok(false);
        }

        let mut request = self.request(config, false)?;
        request.item.mode = SysProxyType::ForcedClear;
        let status = self.service.apply(&request)?;
        if status.effective_type == SysProxyType::ForcedClear {
            self.clear_dirty_marker()?;
        }

        Ok(true)
    }

    /// A failed operation/read cannot advertise an endpoint or claim the OS
    /// proxy was restored. This fallback itself performs no fallible I/O.
    pub fn unavailable_status(&self, config: &AppConfig) -> SystemProxyStatus {
        SystemProxyStatus {
            management: system_proxy_management(self.target_os),
            requested_type: config.system_proxy.mode,
            effective_type: SysProxyType::Unchanged,
            target_os: self.target_os,
            proxy: None,
            exceptions: String::new(),
        }
    }

    fn request(
        &self,
        config: &AppConfig,
        force_disable: bool,
    ) -> Result<SystemProxyRequest, SystemProxyManagerError> {
        self.paths.ensure_dirs()?;
        Ok(SystemProxyRequest {
            target_os: self.target_os,
            item: config.system_proxy.clone(),
            force_disable,
            socks_port: config.local_port(),
            script_dir: self.paths.temp_dir().join(SYSPROXY_SCRIPT_DIR_NAME),
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

/// What the connected core does to the OS proxy on this platform.
struct RuntimeProxyPolicy {
    mode: SysProxyType,
    force_disable: bool,
}

fn runtime_system_proxy_policy(config: &AppConfig, target_os: TargetOs) -> RuntimeProxyPolicy {
    let force_disable = should_disable_native_tun_system_proxy(config, target_os);
    let mode = if !force_disable && should_apply_tun_system_proxy_fallback(config, target_os) {
        SysProxyType::ForcedChange
    } else {
        config.system_proxy.mode
    };
    RuntimeProxyPolicy {
        mode,
        force_disable,
    }
}

#[must_use]
fn should_disable_native_tun_system_proxy(config: &AppConfig, target_os: TargetOs) -> bool {
    config.tun.enabled && tun_backend(target_os).is_native()
}

#[must_use]
fn should_apply_tun_system_proxy_fallback(config: &AppConfig, target_os: TargetOs) -> bool {
    config.tun.enabled
        && config.system_proxy.mode == SysProxyType::ForcedClear
        && tun_backend(target_os) == TunBackend::Process
}

#[must_use]
pub(crate) fn runtime_default_proxy_url(config: &AppConfig, target_os: TargetOs) -> Option<String> {
    if config.tun.enabled && tun_backend(target_os).is_native() {
        return None;
    }

    let port = config.local_port();
    (1..=65_535)
        .contains(&port)
        .then(|| format!("http://127.0.0.1:{port}"))
}

fn request_sets_local_proxy(request: &SystemProxyRequest) -> bool {
    if request.force_disable
        || system_proxy_management(request.target_os) != SystemProxyManagement::Automatic
    {
        return false;
    }

    request.item.mode == SysProxyType::ForcedChange
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc};

    use voya_platform::test_support::RecordingRunner;

    use super::*;

    fn manager(target_os: TargetOs, runner: Arc<RecordingRunner>) -> SystemProxyManager {
        manager_with_app_dir(target_os, runner, unique_app_dir("default"))
    }

    fn manager_with_app_dir(
        target_os: TargetOs,
        runner: Arc<RecordingRunner>,
        app_dir: PathBuf,
    ) -> SystemProxyManager {
        let service = SystemProxyService::new(runner);
        SystemProxyManager::with_target_os(service, AppPaths::new(app_dir), target_os)
    }

    fn unique_app_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "voya-app-sysproxy-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        ))
    }

    #[test]
    fn sysproxy_manager_restore_forces_clear_without_changing_requested_mode() {
        let runner = Arc::new(RecordingRunner::default());
        let manager = manager(TargetOs::Windows, runner);
        let mut config = AppConfig::default();
        config.system_proxy.mode = SysProxyType::ForcedChange;

        let status = manager.restore(&config).expect("restore");

        assert_eq!(status.requested_type, SysProxyType::ForcedChange);
        assert_eq!(status.effective_type, SysProxyType::ForcedClear);
        assert_eq!(config.system_proxy.mode, SysProxyType::ForcedChange);
    }

    #[test]
    fn sysproxy_manager_apply_sets_dirty_marker_for_local_proxy() {
        let app_dir = unique_app_dir("apply-dirty");
        let runner = Arc::new(RecordingRunner::default());
        let manager = manager_with_app_dir(TargetOs::Windows, runner, app_dir.clone());
        let mut config = AppConfig::default();
        config.system_proxy.mode = SysProxyType::ForcedChange;

        manager.apply_config(&config, false).expect("apply proxy");

        assert!(manager.dirty_marker_path().is_file());
        let _ = fs::remove_dir_all(app_dir);
    }

    #[test]
    fn sysproxy_manager_restore_clears_dirty_marker() {
        let app_dir = unique_app_dir("restore-clean");
        let runner = Arc::new(RecordingRunner::default());
        let manager = manager_with_app_dir(TargetOs::Windows, runner, app_dir.clone());
        let mut config = AppConfig::default();
        config.system_proxy.mode = SysProxyType::ForcedChange;

        manager.apply_config(&config, false).expect("apply proxy");
        manager.restore(&config).expect("restore proxy");

        assert!(!manager.dirty_marker_path().exists());
        let _ = fs::remove_dir_all(app_dir);
    }

    #[test]
    fn sysproxy_manager_startup_recovery_forces_clear_when_marker_exists() {
        let app_dir = unique_app_dir("startup-recover");
        let runner = Arc::new(RecordingRunner::default());
        let manager = manager_with_app_dir(TargetOs::Windows, Arc::clone(&runner), app_dir.clone());
        let mut config = AppConfig::default();
        config.system_proxy.mode = SysProxyType::Unchanged;
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
        let manager = manager_with_app_dir(TargetOs::Windows, Arc::clone(&runner), app_dir.clone());
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
        config.tun.enabled = true;

        let native = runtime_system_proxy_policy(&config, TargetOs::Macos);
        assert!(native.force_disable);
        assert!(runtime_default_proxy_url(&config, TargetOs::Macos).is_none());
        assert!(
            !manager(TargetOs::Macos, Arc::new(RecordingRunner::default()))
                .restore_dirty_proxy_if_needed(&AppConfig::default())
                .expect("macOS has no system proxy to recover")
        );

        let process = runtime_system_proxy_policy(&config, TargetOs::Linux);
        assert!(!process.force_disable);
        assert_eq!(process.mode, SysProxyType::ForcedChange);
        assert_eq!(
            runtime_default_proxy_url(&config, TargetOs::Linux).as_deref(),
            Some("http://127.0.0.1:10808")
        );
    }
}
