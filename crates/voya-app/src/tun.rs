use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use voya_core::AppConfig;
use voya_platform::{
    coreinfo::TargetOs,
    privilege::ElevationState,
    tun::{
        tun_preflight, NativeTunController, NativeTunProviderState, PlatformNativeTunController,
        TunBackend as PlatformTunBackend, TunPreflightReport,
        TunPreflightState as PlatformTunPreflightState,
    },
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
pub enum TunBackend {
    Process,
    MacosPacketTunnel,
    WindowsService,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TunProviderState {
    NotApplicable,
    MissingComponent,
    PermissionRequired,
    Stopped,
    Starting,
    Running,
    Error,
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
    pub backend: TunBackend,
    pub provider_state: TunProviderState,
    pub allow_enable_tun: bool,
    pub requires_elevation: bool,
    pub elevation_granted: bool,
    pub needs_vpn_permission: bool,
    pub needs_service_install: bool,
    pub native_component_ready: bool,
    pub last_provider_error: Option<String>,
    pub restore_on_disconnect: bool,
    pub preflight: TunPreflight,
}

#[derive(Clone)]
pub struct TunManager {
    elevation: Arc<ElevationState>,
    native_tun: Arc<dyn NativeTunController>,
    target_os: TargetOs,
}

impl TunManager {
    #[must_use]
    pub fn new(elevation: Arc<ElevationState>) -> Self {
        Self::with_target_os_and_native_tun(
            elevation,
            TargetOs::current(),
            Arc::new(PlatformNativeTunController),
        )
    }

    #[must_use]
    pub fn with_target_os(elevation: Arc<ElevationState>, target_os: TargetOs) -> Self {
        Self::with_target_os_and_native_tun(
            elevation,
            target_os,
            Arc::new(PlatformNativeTunController),
        )
    }

    #[must_use]
    pub fn with_target_os_and_native_tun(
        elevation: Arc<ElevationState>,
        target_os: TargetOs,
        native_tun: Arc<dyn NativeTunController>,
    ) -> Self {
        Self {
            elevation,
            native_tun,
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
        let native_status = self.native_tun.status(report.backend);
        let provider_state = tun_provider_state(native_status.provider_state);
        let status = TunStatus {
            enabled: config.tun_mode_item.enable_tun,
            backend: tun_backend(report.backend),
            provider_state,
            allow_enable_tun: report.allow_enable_tun,
            requires_elevation: report.requires_elevation,
            elevation_granted: report.elevation_granted,
            needs_vpn_permission: native_status.provider_state
                == NativeTunProviderState::PermissionRequired,
            needs_service_install: report.backend == PlatformTunBackend::WindowsService
                && !native_status.component_ready,
            native_component_ready: native_status.component_ready,
            last_provider_error: native_status.message,
            restore_on_disconnect: self.target_os != TargetOs::Other,
            preflight: tun_preflight_response(&report),
        };

        Ok((status, report))
    }
}

const fn tun_backend(backend: PlatformTunBackend) -> TunBackend {
    match backend {
        PlatformTunBackend::Process => TunBackend::Process,
        PlatformTunBackend::MacosPacketTunnel => TunBackend::MacosPacketTunnel,
        PlatformTunBackend::WindowsService => TunBackend::WindowsService,
        PlatformTunBackend::Unsupported => TunBackend::Unsupported,
    }
}

const fn tun_provider_state(state: NativeTunProviderState) -> TunProviderState {
    match state {
        NativeTunProviderState::NotApplicable => TunProviderState::NotApplicable,
        NativeTunProviderState::MissingComponent => TunProviderState::MissingComponent,
        NativeTunProviderState::PermissionRequired => TunProviderState::PermissionRequired,
        NativeTunProviderState::Stopped => TunProviderState::Stopped,
        NativeTunProviderState::Starting => TunProviderState::Starting,
        NativeTunProviderState::Running => TunProviderState::Running,
        NativeTunProviderState::Error => TunProviderState::Error,
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
    fn tun_windows_preflight_tracks_service_backend_and_install_state() {
        let config = AppConfig::default();
        let manager =
            TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Windows);

        let status = manager.status(&config).expect("status");
        assert!(status.allow_enable_tun);
        assert!(!status.requires_elevation);
        assert_eq!(status.backend, TunBackend::WindowsService);
        assert_eq!(status.provider_state, TunProviderState::MissingComponent);
        assert!(status.needs_service_install);
        assert!(!status.native_component_ready);
        assert_eq!(status.preflight.state, TunPreflightState::Ready);
        assert_eq!(
            status.preflight.windows_cleanup_devices,
            ["wintunsingbox_tun".to_string()]
        );
        assert!(status
            .preflight
            .route_restore_note
            .contains("VoyaVPN Service"));
    }

    #[test]
    fn tun_macos_preflight_uses_packet_tunnel_without_sudo() {
        let mut config = AppConfig::default();
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Macos);

        let status = manager
            .set_enabled(&mut config, true)
            .expect("macOS PacketTunnel setting can be enabled without sudo");

        assert!(status.enabled);
        assert!(status.allow_enable_tun);
        assert!(!status.requires_elevation);
        assert_eq!(status.backend, TunBackend::MacosPacketTunnel);
        assert_eq!(status.provider_state, TunProviderState::MissingComponent);
        assert!(!status.native_component_ready);
        assert_eq!(status.preflight.state, TunPreflightState::Ready);
    }
}
