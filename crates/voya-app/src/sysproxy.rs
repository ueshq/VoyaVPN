use std::{
    fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use voya_core::{AppConfig, InboundProtocol, SysProxyType};
use voya_platform::{
    coreinfo::TargetOs,
    paths::{AppPaths, PathError},
    sysproxy::{SystemProxyError, SystemProxyRequest, SystemProxyService, SystemProxyStatus},
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

    pub fn status(&self, config: &AppConfig) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        let request = self.request(config, false)?;
        voya_platform::sysproxy::plan_system_proxy(&request)
            .map(|plan| plan.status)
            .map_err(Into::into)
    }

    pub fn set_mode(
        &self,
        config: &mut AppConfig,
        mode: SysProxyType,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        if mode == SysProxyType::Pac && self.target_os != TargetOs::Windows {
            return Err(SystemProxyManagerError::PacUnavailable(self.target_os));
        }

        config.system_proxy_item.sys_proxy_type = mode;
        self.apply_config(config, false)
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

        let status = self.service.apply(&request)?;
        if status.effective_type == SysProxyType::ForcedClear {
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
        if !self.dirty_marker_exists()? {
            return Ok(false);
        }

        let mut request = self.request(config, false)?;
        request.item.sys_proxy_type = SysProxyType::ForcedClear;
        let status = self.service.apply(&request)?;
        if status.effective_type == SysProxyType::ForcedClear {
            self.clear_dirty_marker()?;
        }

        Ok(true)
    }

    pub fn stop_pac(&self) {
        self.service.stop_pac();
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
        let pac_port = socks_port + InboundProtocol::pac.as_i32();

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
        match fs::metadata(&path) {
            Ok(_) => Ok(true),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(SystemProxyManagerError::DirtyMarkerInspect { path, source }),
        }
    }

    fn write_dirty_marker(&self) -> Result<(), SystemProxyManagerError> {
        self.paths.ensure_dirs()?;
        let path = self.dirty_marker_path();
        fs::write(&path, SYSPROXY_DIRTY_MARKER_CONTENTS)
            .map_err(|source| SystemProxyManagerError::DirtyMarkerWrite { path, source })
    }

    fn clear_dirty_marker(&self) -> Result<(), SystemProxyManagerError> {
        let path = self.dirty_marker_path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(SystemProxyManagerError::DirtyMarkerRemove { path, source }),
        }
    }
}

#[derive(Debug, Error)]
pub enum SystemProxyManagerError {
    #[error("PAC mode is only available on Windows, not {0:?}")]
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

fn current_tick_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
        .to_string()
}

fn request_sets_local_proxy(request: &SystemProxyRequest) -> bool {
    if request.force_disable {
        return false;
    }

    matches!(
        (request.item.sys_proxy_type, request.target_os),
        (
            SysProxyType::ForcedChange,
            TargetOs::Windows | TargetOs::Linux | TargetOs::Macos
        ) | (SysProxyType::Pac, TargetOs::Windows)
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
        paths::StorageMode,
        process::{ProcessError, ProcessHandle, ProcessOutput, ProcessRunner, ProcessSpawn},
        sysproxy::{PacManager, PacStartConfig},
    };

    use super::*;

    #[derive(Default)]
    struct RecordingRunner {
        spawns: Mutex<Vec<ProcessSpawn>>,
    }

    impl ProcessRunner for RecordingRunner {
        fn spawn(&self, _request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
            unreachable!("sysproxy app tests only use oneshot commands")
        }

        fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
            self.spawns.lock().expect("spawns").push(request);
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn stop(&self, _handle: &ProcessHandle) -> Result<(), ProcessError> {
            unreachable!("sysproxy app tests only use oneshot commands")
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
        let service = SystemProxyService::new(runner, pac);
        SystemProxyManager::with_target_os(
            service,
            AppPaths::new(app_dir, StorageMode::Portable),
            target_os,
        )
    }

    fn unique_app_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "voya-app-sysproxy-{name}-{}-{}",
            std::process::id(),
            current_tick_string()
        ))
    }

    #[test]
    fn sysproxy_manager_rejects_pac_mode_off_windows() {
        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(RecordingPac::default());
        let manager = manager(TargetOs::Macos, runner, pac);
        let mut config = AppConfig::default();

        let error = manager
            .set_mode(&mut config, SysProxyType::Pac)
            .expect_err("pac should be hidden and rejected off Windows");

        assert!(matches!(
            error,
            SystemProxyManagerError::PacUnavailable(TargetOs::Macos)
        ));
        assert_eq!(
            config.system_proxy_item.sys_proxy_type,
            SysProxyType::ForcedClear
        );
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
        assert_eq!(runner.spawns.lock().expect("spawns").len(), 4);
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
        assert!(runner.spawns.lock().expect("spawns").is_empty());
        let _ = fs::remove_dir_all(app_dir);
    }
}
