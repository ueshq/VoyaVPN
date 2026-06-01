use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use voya_core::{AppConfig, InboundProtocol, SysProxyType};
use voya_platform::{
    coreinfo::TargetOs,
    paths::{AppPaths, PathError},
    sysproxy::{SystemProxyError, SystemProxyRequest, SystemProxyService, SystemProxyStatus},
};

const SYSPROXY_SCRIPT_DIR_NAME: &str = "sysproxy";

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

        Ok(self.service.apply(&request)?)
    }

    pub fn restore(
        &self,
        config: &AppConfig,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        self.apply_config(config, true)
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
}

#[derive(Debug, Error)]
pub enum SystemProxyManagerError {
    #[error("PAC mode is only available on Windows, not {0:?}")]
    PacUnavailable(TargetOs),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    SystemProxy(#[from] SystemProxyError),
}

fn current_tick_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

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
        let service = SystemProxyService::new(runner, pac);
        SystemProxyManager::with_target_os(
            service,
            AppPaths::new("/tmp/voya-app-sysproxy", StorageMode::Portable),
            target_os,
        )
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
}
