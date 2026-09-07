use std::{path::Path, sync::Arc};

use thiserror::Error;
pub use voya_contracts::{
    TunBackend, TunPlatform, TunPreflight, TunPreflightState, TunProviderDiagnostics,
    TunProviderState, TunStatus,
};
use voya_core::AppConfig;
use voya_platform::{
    coreinfo::TargetOs,
    privilege::ElevationState,
    tun::{
        tun_preflight, NativeTunController, NativeTunDiagnostics, NativeTunProviderState,
        PlatformNativeTunController, PlatformProviderRegistrationResolver,
        ProviderRegistrationResolver, TunBackend as PlatformTunBackend, TunPreflightReport,
        TunPreflightState as PlatformTunPreflightState, MACOS_PACKET_TUNNEL_BUNDLE_ID,
    },
};

#[derive(Clone)]
pub struct TunManager {
    elevation: Arc<ElevationState>,
    native_tun: Arc<dyn NativeTunController>,
    provider_resolver: Arc<dyn ProviderRegistrationResolver>,
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
            provider_resolver: Arc::new(PlatformProviderRegistrationResolver),
            target_os,
        }
    }

    #[must_use]
    pub fn with_provider_resolver(
        mut self,
        provider_resolver: Arc<dyn ProviderRegistrationResolver>,
    ) -> Self {
        self.provider_resolver = provider_resolver;
        self
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
        let status = self.plan_set_enabled(config, enabled)?;
        Self::apply_enabled(config, enabled);

        Ok(status)
    }

    /// Run the enable/disable preflight without touching the configuration.
    ///
    /// The probe forks `pluginkit`/`systemextensionsctl`/`sc.exe` and loads
    /// NetworkExtension preferences, so callers holding a config mutation guard
    /// can run this off the async runtime against a snapshot and then apply the
    /// decision with [`Self::apply_enabled`]. The returned status is what the
    /// config will report once applied — only `enabled` is derived from the
    /// config, everything else comes from the platform probe.
    pub fn plan_set_enabled(
        &self,
        config: &AppConfig,
        enabled: bool,
    ) -> Result<TunStatus, TunManagerError> {
        let (status, report) = self.status_with_report(config)?;
        if enabled && status.provider_path_mismatch {
            return Err(TunManagerError::ProviderPathMismatch {
                expected: status.expected_provider_path.unwrap_or_default(),
                resolved: status.resolved_provider_path.unwrap_or_default(),
            });
        }
        if enabled && !status.allow_enable_tun {
            return if report.requires_elevation && !report.elevation_granted {
                Err(TunManagerError::ElevationRequired)
            } else {
                Err(TunManagerError::UnsupportedPlatform)
            };
        }

        Ok(TunStatus { enabled, ..status })
    }

    /// Apply a change already validated by [`Self::plan_set_enabled`].
    pub fn apply_enabled(config: &mut AppConfig, enabled: bool) {
        config.tun_mode_item.enable_tun = enabled;
    }

    fn status_with_report(
        &self,
        config: &AppConfig,
    ) -> Result<(TunStatus, TunPreflightReport), TunManagerError> {
        let elevation_granted = self.elevation.is_granted();
        let report = tun_preflight(self.target_os, elevation_granted);
        let native_status = self.native_tun.status(report.backend);
        let registration = self.provider_registration_status(report.backend);
        let provider_state = tun_provider_state(native_status.provider_state);
        let status = TunStatus {
            enabled: config.tun_mode_item.enable_tun,
            backend: tun_backend(report.backend),
            provider_state,
            allow_enable_tun: report.allow_enable_tun && !registration.path_mismatch,
            requires_elevation: report.requires_elevation,
            elevation_granted: report.elevation_granted,
            needs_vpn_permission: native_status.provider_state
                == NativeTunProviderState::PermissionRequired,
            needs_service_install: report.backend == PlatformTunBackend::WindowsService
                && !native_status.component_ready,
            native_component_ready: native_status.component_ready,
            last_provider_error: native_status.message,
            provider_path_mismatch: registration.path_mismatch,
            resolved_provider_path: registration.resolved_provider_path,
            expected_provider_path: registration.expected_provider_path,
            restore_on_disconnect: self.target_os != TargetOs::Other,
            preflight: tun_preflight_response(&report),
        };

        Ok((status, report))
    }

    pub fn provider_diagnostics(&self) -> Result<TunProviderDiagnostics, TunManagerError> {
        let backend = tun_backend(self.platform_backend());
        Ok(tun_provider_diagnostics_response(
            backend,
            self.native_tun.diagnostics(self.platform_backend()),
        ))
    }

    fn platform_backend(&self) -> PlatformTunBackend {
        voya_platform::tun::tun_backend(self.target_os)
    }

    fn provider_registration_status(
        &self,
        backend: PlatformTunBackend,
    ) -> ProviderRegistrationStatus {
        if backend != PlatformTunBackend::MacosPacketTunnel {
            return ProviderRegistrationStatus::default();
        }

        let expected = self
            .provider_resolver
            .expected_provider_path(MACOS_PACKET_TUNNEL_BUNDLE_ID);
        let Some(expected) = expected else {
            return ProviderRegistrationStatus::default();
        };
        let expected_label = Some(expected.display().to_string());
        let resolved = match self
            .provider_resolver
            .resolved_provider_paths(MACOS_PACKET_TUNNEL_BUNDLE_ID)
        {
            Ok(paths) => paths,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "failed to resolve macOS PacketTunnel provider path; continuing"
                );
                return ProviderRegistrationStatus {
                    expected_provider_path: expected_label,
                    ..ProviderRegistrationStatus::default()
                };
            }
        };
        if resolved.is_empty() {
            return ProviderRegistrationStatus {
                expected_provider_path: expected_label,
                ..ProviderRegistrationStatus::default()
            };
        }

        if resolved
            .iter()
            .any(|path| provider_paths_equivalent(path, &expected))
        {
            return ProviderRegistrationStatus {
                expected_provider_path: expected_label,
                resolved_provider_path: resolved.first().map(|path| path.display().to_string()),
                ..ProviderRegistrationStatus::default()
            };
        }

        ProviderRegistrationStatus {
            path_mismatch: true,
            expected_provider_path: expected_label,
            resolved_provider_path: resolved.first().map(|path| path.display().to_string()),
        }
    }
}

#[derive(Debug, Default)]
struct ProviderRegistrationStatus {
    path_mismatch: bool,
    resolved_provider_path: Option<String>,
    expected_provider_path: Option<String>,
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

fn tun_provider_diagnostics_response(
    backend: TunBackend,
    diagnostics: NativeTunDiagnostics,
) -> TunProviderDiagnostics {
    let status = diagnostics.status.unwrap_or_default();
    TunProviderDiagnostics {
        backend,
        container_path: diagnostics
            .container_path
            .map(|path| path.display().to_string()),
        status_path: diagnostics
            .status_path
            .map(|path| path.display().to_string()),
        log_path: diagnostics.log_path.map(|path| path.display().to_string()),
        packaging_mode: diagnostics.packaging_mode,
        expected_provider_path: diagnostics
            .expected_provider_path
            .map(|path| path.display().to_string()),
        system_extension_state: diagnostics.system_extension_state,
        registration_paths: diagnostics.registration_paths,
        status_state: status.state,
        last_error: status.last_error,
        provider_bundle_path: status.provider_bundle_path,
        breadcrumbs: status.breadcrumbs,
        provider_log_tail: diagnostics.provider_log_tail,
        host_log_tail: diagnostics.host_log_tail,
        message: diagnostics.message,
    }
}

fn provider_paths_equivalent(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
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
    #[error(
        "macOS PacketTunnel provider path mismatch: expected {expected}, PlugInKit elected {resolved}"
    )]
    ProviderPathMismatch { expected: String, resolved: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct FakeProviderResolver {
        expected: Option<std::path::PathBuf>,
        resolved: Vec<std::path::PathBuf>,
    }

    impl ProviderRegistrationResolver for FakeProviderResolver {
        fn expected_provider_path(&self, _bundle_id: &str) -> Option<std::path::PathBuf> {
            self.expected.clone()
        }

        fn resolved_provider_paths(
            &self,
            _bundle_id: &str,
        ) -> Result<Vec<std::path::PathBuf>, voya_platform::tun::NativeTunError> {
            Ok(self.resolved.clone())
        }
    }

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

    /// The status probe forks OS helpers, so command handlers run it off the
    /// async runtime against a config snapshot and apply the flag afterwards.
    /// The planned status must match what the applied config reports.
    #[test]
    fn plan_set_enabled_reports_the_post_apply_status_without_mutating() {
        let mut config = AppConfig::default();
        let elevation = Arc::new(ElevationState::new());
        elevation.set_granted(true);
        let manager = TunManager::with_target_os(elevation, TargetOs::Linux);

        let planned = manager
            .plan_set_enabled(&config, true)
            .expect("plan enable with elevation grant");
        assert!(planned.enabled);
        assert!(!config.tun_mode_item.enable_tun);

        TunManager::apply_enabled(&mut config, true);
        assert!(config.tun_mode_item.enable_tun);
        assert_eq!(manager.status(&config).expect("status"), planned);
    }

    #[test]
    fn plan_set_enabled_rejects_without_mutating() {
        let config = AppConfig::default();
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Linux);

        assert!(matches!(
            manager.plan_set_enabled(&config, true),
            Err(TunManagerError::ElevationRequired)
        ));
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

    #[test]
    fn tun_macos_provider_path_mismatch_blocks_enable_and_surfaces_paths() {
        let mut config = AppConfig::default();
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Macos)
            .with_provider_resolver(Arc::new(FakeProviderResolver {
                expected: Some(std::path::PathBuf::from(
                    "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
                )),
                resolved: vec![std::path::PathBuf::from(
                    "/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
                )],
            }));

        let status = manager.status(&config).expect("status");
        assert!(status.provider_path_mismatch);
        assert!(!status.allow_enable_tun);
        assert_eq!(
            status.expected_provider_path.as_deref(),
            Some(
                "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex"
            )
        );
        assert_eq!(
            status.resolved_provider_path.as_deref(),
            Some("/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex")
        );

        assert!(matches!(
            manager.set_enabled(&mut config, true),
            Err(TunManagerError::ProviderPathMismatch { .. })
        ));
        assert!(!config.tun_mode_item.enable_tun);
    }
}
