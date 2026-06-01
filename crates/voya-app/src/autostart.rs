use std::{
    env, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use voya_core::AppConfig;
use voya_platform::{
    autostart::{
        AutostartAdapter, AutostartArtifact, AutostartError, AutostartRequest, AutostartService,
        StdAutostartAdapter, AUTOSTART_APP_NAME,
    },
    coreinfo::TargetOs,
    process::StdProcessRunner,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AutostartPlatform {
    Windows,
    Linux,
    Macos,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AutostartStatus {
    pub enabled: bool,
    pub platform: AutostartPlatform,
    pub artifact_kind: Option<String>,
    pub artifact_path: Option<String>,
    pub artifact_name: Option<String>,
}

#[derive(Clone)]
pub struct AutostartManager {
    service: AutostartService,
    target_os: TargetOs,
    app_name: String,
}

impl AutostartManager {
    #[must_use]
    pub fn new() -> Self {
        Self::with_service(
            AutostartService::new(Arc::new(StdAutostartAdapter::new(Arc::new(
                StdProcessRunner::new(),
            )))),
            TargetOs::current(),
            AUTOSTART_APP_NAME,
        )
    }

    #[must_use]
    pub fn with_adapter(adapter: Arc<dyn AutostartAdapter>, target_os: TargetOs) -> Self {
        Self::with_service(
            AutostartService::new(adapter),
            target_os,
            AUTOSTART_APP_NAME,
        )
    }

    #[must_use]
    pub fn with_service(
        service: AutostartService,
        target_os: TargetOs,
        app_name: impl Into<String>,
    ) -> Self {
        Self {
            service,
            target_os,
            app_name: app_name.into(),
        }
    }

    pub fn status(&self, config: &AppConfig) -> Result<AutostartStatus, AutostartManagerError> {
        let request = self.request(config.gui_item.auto_run)?;

        Ok(status_from_request(&request))
    }

    pub fn set_enabled(
        &self,
        config: &mut AppConfig,
        enabled: bool,
    ) -> Result<AutostartStatus, AutostartManagerError> {
        let request = self.request(enabled)?;
        self.service.apply(&request)?;
        config.gui_item.auto_run = enabled;

        Ok(status_from_request(&request))
    }

    fn request(&self, enabled: bool) -> Result<AutostartRequest, AutostartManagerError> {
        Ok(AutostartRequest {
            target_os: self.target_os,
            enabled,
            app_name: self.app_name.clone(),
            executable: current_executable()?,
            home_dir: home_dir()?,
        })
    }
}

impl Default for AutostartManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error)]
pub enum AutostartManagerError {
    #[error(transparent)]
    Autostart(#[from] AutostartError),
    #[error("failed to determine current executable path: {0}")]
    CurrentExe(io::Error),
    #[error("failed to determine a home directory for autostart artifacts")]
    HomeDir,
}

fn status_from_request(request: &AutostartRequest) -> AutostartStatus {
    let artifact = request.artifact();

    AutostartStatus {
        enabled: request.enabled,
        platform: autostart_platform(request.target_os),
        artifact_kind: artifact.as_ref().map(artifact_kind).map(str::to_string),
        artifact_path: artifact.as_ref().and_then(artifact_path),
        artifact_name: artifact.as_ref().and_then(artifact_name),
    }
}

fn current_executable() -> Result<PathBuf, AutostartManagerError> {
    env::current_exe().map_err(AutostartManagerError::CurrentExe)
}

fn home_dir() -> Result<PathBuf, AutostartManagerError> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or(AutostartManagerError::HomeDir)
}

const fn autostart_platform(os: TargetOs) -> AutostartPlatform {
    match os {
        TargetOs::Windows => AutostartPlatform::Windows,
        TargetOs::Linux => AutostartPlatform::Linux,
        TargetOs::Macos => AutostartPlatform::Macos,
        TargetOs::Other => AutostartPlatform::Other,
    }
}

fn artifact_kind(artifact: &AutostartArtifact) -> &'static str {
    match artifact {
        AutostartArtifact::WindowsRunRegistry { .. } => "windowsRunRegistry",
        AutostartArtifact::LinuxDesktopFile { .. } => "linuxDesktopFile",
        AutostartArtifact::MacosLaunchAgent { .. } => "macosLaunchAgent",
    }
}

fn artifact_path(artifact: &AutostartArtifact) -> Option<String> {
    match artifact {
        AutostartArtifact::WindowsRunRegistry { key_path, .. } => Some(key_path.clone()),
        AutostartArtifact::LinuxDesktopFile { path }
        | AutostartArtifact::MacosLaunchAgent { path, .. } => Some(path_display(path)),
    }
}

fn artifact_name(artifact: &AutostartArtifact) -> Option<String> {
    match artifact {
        AutostartArtifact::WindowsRunRegistry { value_name, .. } => Some(value_name.clone()),
        AutostartArtifact::LinuxDesktopFile { path } => path
            .file_name()
            .and_then(|value| value.to_str())
            .map(ToString::to_string),
        AutostartArtifact::MacosLaunchAgent { label, .. } => Some(label.clone()),
    }
}

fn path_display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod autostart_app_tests {
    use std::sync::Mutex;

    use voya_platform::autostart::AutostartAdapter;

    use super::*;

    #[derive(Default)]
    struct FakeAutostartAdapter {
        writes: Mutex<u32>,
        registry_sets: Mutex<u32>,
    }

    impl AutostartAdapter for FakeAutostartAdapter {
        fn write_file(&self, _path: &Path, _contents: &str) -> Result<(), AutostartError> {
            *self.writes.lock().expect("writes") += 1;
            Ok(())
        }

        fn remove_file(&self, _path: &Path) -> Result<(), AutostartError> {
            Ok(())
        }

        fn run_command(
            &self,
            _executable: &Path,
            _arguments: &[String],
        ) -> Result<(), AutostartError> {
            Ok(())
        }

        fn set_windows_run_registry(
            &self,
            _key_path: &str,
            _value_name: &str,
            _value: &str,
        ) -> Result<(), AutostartError> {
            *self.registry_sets.lock().expect("registry_sets") += 1;
            Ok(())
        }

        fn delete_windows_run_registry(
            &self,
            _key_path: &str,
            _value_name: &str,
        ) -> Result<(), AutostartError> {
            Ok(())
        }
    }

    #[test]
    fn autostart_manager_updates_config_after_adapter_success() {
        let adapter = Arc::new(FakeAutostartAdapter::default());
        let manager = AutostartManager::with_adapter(adapter.clone(), TargetOs::Linux);
        let mut config = AppConfig::default();

        let status = manager
            .set_enabled(&mut config, true)
            .expect("autostart set");

        assert!(config.gui_item.auto_run);
        assert!(status.enabled);
        assert_eq!(status.platform, AutostartPlatform::Linux);
        assert_eq!(*adapter.writes.lock().expect("writes"), 1);
    }
}
