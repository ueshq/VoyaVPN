use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use voya_core::AppConfig;
use voya_platform::{
    coreinfo::TargetOs,
    privilege::ElevationState,
    tun::{tun_preflight, TunPreflightReport, TunPreflightState as PlatformTunPreflightState},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TunPlatform {
    Windows,
    Linux,
    Macos,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TunPreflightState {
    Ready,
    NeedsElevation,
    ManualCheck,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TunPreflight {
    pub platform: TunPlatform,
    pub state: TunPreflightState,
    pub notes: Vec<String>,
    pub route_restore_note: String,
    pub windows_cleanup_devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TunStatus {
    pub enabled: bool,
    pub allow_enable_tun: bool,
    pub requires_elevation: bool,
    pub elevation_granted: bool,
    pub restore_on_disconnect: bool,
    pub preflight: TunPreflight,
}

#[derive(Debug, Clone)]
pub struct TunManager {
    elevation: Arc<ElevationState>,
    target_os: TargetOs,
}

impl TunManager {
    #[must_use]
    pub fn new(elevation: Arc<ElevationState>) -> Self {
        Self::with_target_os(elevation, TargetOs::current())
    }

    #[must_use]
    pub fn with_target_os(elevation: Arc<ElevationState>, target_os: TargetOs) -> Self {
        Self {
            elevation,
            target_os,
        }
    }

    pub fn status(&self, config: &AppConfig) -> Result<TunStatus, TunManagerError> {
        self.status_with_report(config)
            .map(|(status, _report)| status)
    }

    pub fn set_enabled(
        &self,
        config: &mut AppConfig,
        enabled: bool,
    ) -> Result<TunStatus, TunManagerError> {
        let (status, report) = self.status_with_report(config)?;
        if enabled && !status.allow_enable_tun {
            return if report.requires_elevation && !report.elevation_granted {
                Err(TunManagerError::ElevationRequired)
            } else {
                Err(TunManagerError::UnsupportedPlatform)
            };
        }

        config.tun_mode_item.enable_tun = enabled;
        self.status(config)
    }

    fn status_with_report(
        &self,
        config: &AppConfig,
    ) -> Result<(TunStatus, TunPreflightReport), TunManagerError> {
        let elevation_granted = self.elevation.is_granted();
        let report = tun_preflight(self.target_os, elevation_granted);
        let status = TunStatus {
            enabled: config.tun_mode_item.enable_tun,
            allow_enable_tun: report.allow_enable_tun,
            requires_elevation: report.requires_elevation,
            elevation_granted: report.elevation_granted,
            restore_on_disconnect: self.target_os != TargetOs::Other,
            preflight: tun_preflight_response(&report),
        };

        Ok((status, report))
    }
}

fn tun_preflight_response(report: &TunPreflightReport) -> TunPreflight {
    TunPreflight {
        platform: tun_platform(report.os),
        state: tun_preflight_state(report.state),
        notes: report.notes.clone(),
        route_restore_note: report.route_restore_note.clone(),
        windows_cleanup_devices: report
            .windows_cleanup_devices
            .iter()
            .map(|device| device.name.to_string())
            .collect(),
    }
}

const fn tun_platform(os: TargetOs) -> TunPlatform {
    match os {
        TargetOs::Windows => TunPlatform::Windows,
        TargetOs::Linux => TunPlatform::Linux,
        TargetOs::Macos => TunPlatform::Macos,
        TargetOs::Other => TunPlatform::Other,
    }
}

const fn tun_preflight_state(state: PlatformTunPreflightState) -> TunPreflightState {
    match state {
        PlatformTunPreflightState::Ready => TunPreflightState::Ready,
        PlatformTunPreflightState::NeedsElevation => TunPreflightState::NeedsElevation,
        PlatformTunPreflightState::ManualCheck => TunPreflightState::ManualCheck,
        PlatformTunPreflightState::Unsupported => TunPreflightState::Unsupported,
    }
}

#[derive(Debug, Error)]
pub enum TunManagerError {
    #[error("system authorization is required before enabling TUN on Unix")]
    ElevationRequired,
    #[error("TUN mode is not supported on this platform")]
    UnsupportedPlatform,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tun_allow_enable_on_unix_is_tied_to_elevation_grant() {
        let mut config = AppConfig::default();
        let elevation = Arc::new(ElevationState::new());
        let manager = TunManager::with_target_os(Arc::clone(&elevation), TargetOs::Linux);

        let status = manager.status(&config).expect("status");
        assert!(!status.enabled);
        assert!(!status.allow_enable_tun);
        assert!(status.requires_elevation);
        assert_eq!(status.preflight.state, TunPreflightState::NeedsElevation);
        assert!(matches!(
            manager.set_enabled(&mut config, true),
            Err(TunManagerError::ElevationRequired)
        ));

        elevation.set_granted(true);
        let status = manager
            .set_enabled(&mut config, true)
            .expect("enable with elevation grant");
        assert!(status.enabled);
        assert!(status.allow_enable_tun);
        assert!(status.elevation_granted);
        assert_eq!(status.preflight.state, TunPreflightState::Ready);
    }

    #[test]
    fn tun_disable_does_not_require_elevation() {
        let mut config = AppConfig::default();
        config.tun_mode_item.enable_tun = true;
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Macos);

        let status = manager.set_enabled(&mut config, false).expect("disable");
        assert!(!status.enabled);
        assert!(!config.tun_mode_item.enable_tun);
    }

    #[test]
    fn tun_windows_preflight_tracks_cleanup_devices_and_manual_driver_smoke() {
        let config = AppConfig::default();
        let manager =
            TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Windows);

        let status = manager.status(&config).expect("status");
        assert!(status.allow_enable_tun);
        assert!(!status.requires_elevation);
        assert_eq!(status.preflight.state, TunPreflightState::ManualCheck);
        assert_eq!(
            status.preflight.windows_cleanup_devices,
            ["wintunsingbox_tun".to_string(), "xray_tun".to_string()]
        );
        assert!(status
            .preflight
            .route_restore_note
            .contains("manual OS smoke"));
    }
}
