use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use thiserror::Error;
use voya_contracts::{TunBackend, TunProviderDiagnostics, TunProviderState, TunStatus};
use voya_core::AppConfig;

use crate::blocking::{run_blocking, BlockingTaskError};
use voya_platform::{
    coreinfo::TargetOs,
    privilege::ElevationState,
    tun::{
        tun_preflight, NativeTunController, NativeTunDiagnostics, NativeTunProviderState,
        PlatformNativeTunController, PlatformProviderRegistrationResolver,
        ProviderRegistrationResolver, TunBackend as PlatformTunBackend, TunPreflightReport,
        MACOS_PACKET_TUNNEL_BUNDLE_ID,
    },
};

/// How long one macOS PacketTunnel registration probe is reused.
///
/// The probe forks `pluginkit -mAvvv` and canonicalizes the paths it returns —
/// 100-500 ms — and `TunManager` is rebuilt per IPC call, so without a shared
/// memo it ran on every Home mount, twice per connection-mode switch and on
/// every connect/disconnect. The window is deliberately only a few seconds:
/// the whole point of the check is to notice a PlugInKit election made
/// *outside* the app (see the macOS NetworkExtension section of AGENTS.md), so
/// a long TTL would hide exactly the misconfiguration it exists to catch.
const PROVIDER_REGISTRATION_TTL: Duration = Duration::from_secs(5);

/// Short-lived memo of the macOS provider-registration probe.
///
/// Shared across the per-command `TunManager` instances; a manager built
/// without one gets a private, empty cache and therefore probes once, which is
/// what every unit test wants.
#[derive(Debug, Default)]
pub struct ProviderRegistrationCache {
    entry: Mutex<Option<CachedRegistration>>,
}

#[derive(Debug, Clone)]
struct CachedRegistration {
    probed_at: Instant,
    status: ProviderRegistrationStatus,
}

impl ProviderRegistrationCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget the memo so the next status re-probes PlugInKit.
    pub fn invalidate(&self) {
        if let Ok(mut entry) = self.entry.lock() {
            *entry = None;
        }
    }

    fn fresh_at(&self, now: Instant) -> Option<ProviderRegistrationStatus> {
        let entry = self.entry.lock().ok()?;
        let cached = entry.as_ref()?;
        (now.duration_since(cached.probed_at) < PROVIDER_REGISTRATION_TTL)
            .then(|| cached.status.clone())
    }

    fn store(&self, now: Instant, status: &ProviderRegistrationStatus) {
        if let Ok(mut entry) = self.entry.lock() {
            *entry = Some(CachedRegistration {
                probed_at: now,
                status: status.clone(),
            });
        }
    }
}

#[derive(Clone)]
pub struct TunManager {
    elevation: Arc<ElevationState>,
    native_tun: Arc<dyn NativeTunController>,
    provider_resolver: Arc<dyn ProviderRegistrationResolver>,
    registration_cache: Arc<ProviderRegistrationCache>,
    target_os: TargetOs,
}

impl TunManager {
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
            registration_cache: Arc::new(ProviderRegistrationCache::new()),
            target_os,
        }
    }

    /// Share one registration memo across the per-command managers.
    #[must_use]
    pub fn with_provider_registration_cache(
        mut self,
        registration_cache: Arc<ProviderRegistrationCache>,
    ) -> Self {
        self.registration_cache = registration_cache;
        self
    }

    /// A host that owns the tunnel itself — a phone's app, which alone can
    /// start its provider — swaps in its own controller, so status reads ask
    /// the host rather than `voya-platform`.
    #[must_use]
    pub fn with_native_tun_controller(mut self, native_tun: Arc<dyn NativeTunController>) -> Self {
        self.native_tun = native_tun;
        self
    }

    /// Test seam for the macOS provider-registration probe.
    #[cfg(test)]
    #[must_use]
    pub fn with_provider_resolver(
        mut self,
        provider_resolver: Arc<dyn ProviderRegistrationResolver>,
    ) -> Self {
        self.provider_resolver = provider_resolver;
        self
    }

    /// The TUN status, read off the caller's async worker.
    ///
    /// The probe forks `pluginkit`/`sc.exe`/`systemextensionsctl`, and on a
    /// phone asks the host app, which may wait on the system for seconds. The
    /// blocking read stays crate-private so a host cannot call it inline.
    pub async fn status_off_thread(
        &self,
        config: &AppConfig,
    ) -> Result<TunStatus, TunManagerError> {
        let manager = self.clone();
        let enabled = config.tun.enabled;

        run_blocking("TUN status", move || manager.status(enabled)).await?
    }

    /// [`Self::plan_set_enabled`] off the caller's async worker.
    pub async fn plan_set_enabled_off_thread(
        &self,
        enabled: bool,
    ) -> Result<TunStatus, TunManagerError> {
        let manager = self.clone();

        run_blocking("TUN preflight", move || manager.plan_set_enabled(enabled)).await?
    }

    /// `enabled` is the configuration's flag; everything else is probed.
    pub(crate) fn status(&self, enabled: bool) -> Result<TunStatus, TunManagerError> {
        self.status_with_report(enabled, RegistrationFreshness::Cached)
            .map(|(status, _report)| status)
    }

    /// Run the enable/disable preflight without touching the configuration.
    ///
    /// The probe forks `pluginkit`/`systemextensionsctl`/`sc.exe` and loads
    /// NetworkExtension preferences, so it runs off the async runtime, before
    /// any config mutation guard is taken, and the caller then writes
    /// `tun.enabled` itself. The returned status is what the config will
    /// report once applied: nothing but `enabled` ever came from the config,
    /// so the probe needs none.
    pub(crate) fn plan_set_enabled(&self, enabled: bool) -> Result<TunStatus, TunManagerError> {
        // macOS has no other capture path, so TUN stays on there. Refuse before
        // probing: nothing about the machine can change the answer.
        if !enabled && self.platform_backend() == PlatformTunBackend::MacosPacketTunnel {
            return Err(TunManagerError::VpnRequired);
        }
        // Always re-probe here: this is the gate that refuses to enable TUN when
        // PlugInKit elected another bundle's provider, so it must never decide
        // on a memo taken before the user fixed (or broke) the installation.
        let (status, report) = self.status_with_report(enabled, RegistrationFreshness::Probe)?;
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

        Ok(status)
    }

    fn status_with_report(
        &self,
        enabled: bool,
        freshness: RegistrationFreshness,
    ) -> Result<(TunStatus, TunPreflightReport), TunManagerError> {
        let elevation_granted = self.elevation.is_granted();
        let report = tun_preflight(self.target_os, elevation_granted);
        let native_status = self.native_tun.status(report.backend);
        let registration = self.provider_registration(report.backend, freshness);
        let provider_state = tun_provider_state(native_status.provider_state);
        let status = TunStatus {
            enabled,
            backend: tun_backend(report.backend),
            provider_state,
            allow_enable_tun: report.allow_enable_tun && !registration.path_mismatch,
            requires_elevation: report.requires_elevation,
            elevation_granted: report.elevation_granted,
            native_component_ready: native_status.component_ready,
            last_provider_error: native_status.message,
            provider_path_mismatch: registration.path_mismatch,
            resolved_provider_path: registration.resolved_provider_path,
            expected_provider_path: registration.expected_provider_path,
        };

        Ok((status, report))
    }

    pub fn provider_diagnostics(&self) -> Result<TunProviderDiagnostics, TunManagerError> {
        // The user is asking why TUN will not start, so the memo goes: whatever
        // the next status reports has to reflect the same machine state as the
        // diagnostics they are looking at.
        self.registration_cache.invalidate();
        let backend = tun_backend(self.platform_backend());
        Ok(tun_provider_diagnostics_response(
            backend,
            self.native_tun.diagnostics(self.platform_backend()),
        ))
    }

    fn platform_backend(&self) -> PlatformTunBackend {
        voya_platform::tun::tun_backend(self.target_os)
    }

    /// The registration status, from the shared memo when it is still fresh.
    fn provider_registration(
        &self,
        backend: PlatformTunBackend,
        freshness: RegistrationFreshness,
    ) -> ProviderRegistrationStatus {
        if backend != PlatformTunBackend::MacosPacketTunnel {
            return ProviderRegistrationStatus::default();
        }

        let now = Instant::now();
        if freshness == RegistrationFreshness::Cached {
            if let Some(cached) = self.registration_cache.fresh_at(now) {
                return cached;
            }
        }

        let status = self.provider_registration_status(backend);
        self.registration_cache.store(now, &status);
        status
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
            Err(voya_platform::tun::NativeTunError::RegistrationUnavailable) => {
                return ProviderRegistrationStatus {
                    expected_provider_path: expected_label,
                    ..ProviderRegistrationStatus::default()
                };
            }
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

/// Whether a status read may answer from the registration memo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistrationFreshness {
    Cached,
    Probe,
}

#[derive(Debug, Default, Clone)]
struct ProviderRegistrationStatus {
    path_mismatch: bool,
    resolved_provider_path: Option<String>,
    expected_provider_path: Option<String>,
}

pub(crate) const fn tun_backend(backend: PlatformTunBackend) -> TunBackend {
    match backend {
        PlatformTunBackend::Process => TunBackend::Process,
        PlatformTunBackend::MacosPacketTunnel => TunBackend::MacosPacketTunnel,
        PlatformTunBackend::WindowsService => TunBackend::WindowsService,
        PlatformTunBackend::IosPacketTunnel => TunBackend::IosPacketTunnel,
        PlatformTunBackend::AndroidVpnService => TunBackend::AndroidVpnService,
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

#[derive(Debug, Error)]
pub enum TunManagerError {
    #[error("system authorization is required before enabling TUN on Unix")]
    ElevationRequired,
    #[error("TUN mode is not supported on this platform")]
    UnsupportedPlatform,
    #[error(
        "macOS captures traffic only through its PacketTunnel VPN, so TUN cannot be turned off"
    )]
    VpnRequired,
    #[error(
        "macOS PacketTunnel provider path mismatch: expected {expected}, PlugInKit elected {resolved}"
    )]
    ProviderPathMismatch { expected: String, resolved: String },
    #[error(transparent)]
    Task(#[from] BlockingTaskError),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The combined plan+apply step the production paths run separately so the
    /// blocking platform probe never runs under the mutation guard.
    fn set_enabled(
        manager: &TunManager,
        config: &mut AppConfig,
        enabled: bool,
    ) -> Result<TunStatus, TunManagerError> {
        let status = manager.plan_set_enabled(enabled)?;
        config.tun.enabled = enabled;
        Ok(status)
    }

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

    /// Counts PlugInKit probes so the memo can be observed without forking
    /// `pluginkit`, and lets the test re-elect a different bundle mid-run.
    #[derive(Debug)]
    struct CountingProviderResolver {
        expected: std::path::PathBuf,
        resolved: Mutex<Vec<std::path::PathBuf>>,
        probes: Mutex<u32>,
    }

    impl CountingProviderResolver {
        fn new(expected: &str) -> Self {
            Self {
                expected: std::path::PathBuf::from(expected),
                resolved: Mutex::new(vec![std::path::PathBuf::from(expected)]),
                probes: Mutex::new(0),
            }
        }

        fn probes(&self) -> u32 {
            *self.probes.lock().expect("probe count")
        }

        fn elect(&self, path: &str) {
            *self.resolved.lock().expect("resolved paths") = vec![std::path::PathBuf::from(path)];
        }
    }

    impl ProviderRegistrationResolver for CountingProviderResolver {
        fn expected_provider_path(&self, _bundle_id: &str) -> Option<std::path::PathBuf> {
            Some(self.expected.clone())
        }

        fn resolved_provider_paths(
            &self,
            _bundle_id: &str,
        ) -> Result<Vec<std::path::PathBuf>, voya_platform::tun::NativeTunError> {
            *self.probes.lock().expect("probe count") += 1;
            Ok(self.resolved.lock().expect("resolved paths").clone())
        }
    }

    const EXPECTED_PROVIDER: &str =
        "/Applications/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex";
    const OTHER_PROVIDER: &str =
        "/Users/afu/Dev/VoyaVPN/target/release/bundle/macos/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex";

    fn macos_manager(
        resolver: &Arc<CountingProviderResolver>,
        cache: &Arc<ProviderRegistrationCache>,
    ) -> TunManager {
        TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Macos)
            .with_provider_resolver(Arc::clone(resolver) as Arc<dyn ProviderRegistrationResolver>)
            .with_provider_registration_cache(Arc::clone(cache))
    }

    /// `TunManager` is rebuilt per IPC call, so the memo has to live outside it:
    /// the Home screen alone reads the status on mount and after every action.
    #[test]
    fn the_registration_memo_is_shared_across_per_command_managers() {
        let config = AppConfig::default();
        let resolver = Arc::new(CountingProviderResolver::new(EXPECTED_PROVIDER));
        let cache = Arc::new(ProviderRegistrationCache::new());

        for _ in 0..3 {
            macos_manager(&resolver, &cache)
                .status(config.tun.enabled)
                .expect("status");
        }

        assert_eq!(
            resolver.probes(),
            1,
            "pluginkit must not be forked per call"
        );
    }

    #[test]
    fn invalidating_the_registration_memo_forces_a_reprobe() {
        let config = AppConfig::default();
        let resolver = Arc::new(CountingProviderResolver::new(EXPECTED_PROVIDER));
        let cache = Arc::new(ProviderRegistrationCache::new());
        let manager = macos_manager(&resolver, &cache);

        manager.status(config.tun.enabled).expect("first status");
        cache.invalidate();
        manager
            .status(config.tun.enabled)
            .expect("status after invalidation");

        assert_eq!(resolver.probes(), 2);
    }

    /// The memo may make a status read stale, but it must never make the enable
    /// gate stale: a bundle elected after the last status would otherwise be
    /// allowed to start.
    #[test]
    fn enabling_tun_reprobes_instead_of_trusting_the_memo() {
        let mut config = AppConfig::default();
        let resolver = Arc::new(CountingProviderResolver::new(EXPECTED_PROVIDER));
        let cache = Arc::new(ProviderRegistrationCache::new());
        let manager = macos_manager(&resolver, &cache);

        let status = manager.status(config.tun.enabled).expect("status");
        assert!(!status.provider_path_mismatch);

        resolver.elect(OTHER_PROVIDER);
        assert!(matches!(
            set_enabled(&manager, &mut config, true),
            Err(TunManagerError::ProviderPathMismatch { .. })
        ));
        assert_eq!(resolver.probes(), 2);
        assert!(!config.tun.enabled);
        // The fresh probe replaces the memo, so the next status agrees with it.
        assert!(
            manager
                .status(config.tun.enabled)
                .expect("status after the re-election")
                .provider_path_mismatch
        );
        assert_eq!(resolver.probes(), 2);
    }

    #[test]
    fn tun_allow_enable_on_unix_is_tied_to_elevation_grant() {
        let mut config = AppConfig::default();
        let elevation = Arc::new(ElevationState::new());
        let manager = TunManager::with_target_os(Arc::clone(&elevation), TargetOs::Linux);

        let status = manager.status(config.tun.enabled).expect("status");
        assert!(!status.enabled);
        assert!(!status.allow_enable_tun);
        assert!(status.requires_elevation);
        assert!(matches!(
            set_enabled(&manager, &mut config, true),
            Err(TunManagerError::ElevationRequired)
        ));

        elevation.set_granted(true);
        let status = set_enabled(&manager, &mut config, true).expect("enable with elevation grant");
        assert!(status.enabled);
        assert!(status.allow_enable_tun);
        assert!(status.elevation_granted);
    }

    #[test]
    fn tun_disable_does_not_require_elevation() {
        let mut config = AppConfig::default();
        config.tun.enabled = true;
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Linux);

        let status = set_enabled(&manager, &mut config, false).expect("disable");
        assert!(!status.enabled);
        assert!(!config.tun.enabled);
    }

    #[test]
    fn macos_refuses_to_leave_vpn_mode() {
        let mut config = AppConfig::default();
        config.tun.enabled = true;
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Macos);

        assert!(matches!(
            set_enabled(&manager, &mut config, false),
            Err(TunManagerError::VpnRequired)
        ));
        assert!(config.tun.enabled);
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
            .plan_set_enabled(true)
            .expect("plan enable with elevation grant");
        assert!(planned.enabled);
        assert!(!config.tun.enabled);

        config.tun.enabled = true;
        assert!(config.tun.enabled);
        assert_eq!(manager.status(config.tun.enabled).expect("status"), planned);
    }

    #[test]
    fn plan_set_enabled_rejects_without_mutating() {
        let config = AppConfig::default();
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Linux);

        assert!(matches!(
            manager.plan_set_enabled(true),
            Err(TunManagerError::ElevationRequired)
        ));
        assert!(!config.tun.enabled);
    }

    #[test]
    fn tun_windows_preflight_tracks_service_backend_and_install_state() {
        let config = AppConfig::default();
        let manager =
            TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Windows);

        let status = manager.status(config.tun.enabled).expect("status");
        assert!(status.allow_enable_tun);
        assert!(!status.requires_elevation);
        assert_eq!(status.backend, TunBackend::WindowsService);
        assert_eq!(status.provider_state, TunProviderState::MissingComponent);
        assert!(!status.native_component_ready);
    }

    #[test]
    fn tun_macos_preflight_uses_packet_tunnel_without_sudo() {
        let mut config = AppConfig::default();
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Macos);

        let status = set_enabled(&manager, &mut config, true)
            .expect("macOS PacketTunnel setting can be enabled without sudo");

        assert!(status.enabled);
        assert!(status.allow_enable_tun);
        assert!(!status.requires_elevation);
        assert_eq!(status.backend, TunBackend::MacosPacketTunnel);
        assert_eq!(status.provider_state, TunProviderState::MissingComponent);
        assert!(!status.native_component_ready);
    }

    #[test]
    fn tun_macos_provider_path_mismatch_blocks_enable_and_surfaces_paths() {
        let mut config = AppConfig::default();
        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Macos)
            .with_provider_resolver(Arc::new(FakeProviderResolver {
                expected: Some(std::path::PathBuf::from(
                    "/Applications/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex",
                )),
                resolved: vec![std::path::PathBuf::from(
                    "/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/VoyaPacketTunnel.appex",
                )],
            }));

        let status = manager.status(config.tun.enabled).expect("status");
        assert!(status.provider_path_mismatch);
        assert!(!status.allow_enable_tun);
        assert_eq!(
            status.expected_provider_path.as_deref(),
            Some("/Applications/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex")
        );
        assert_eq!(
            status.resolved_provider_path.as_deref(),
            Some("/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/VoyaPacketTunnel.appex")
        );

        assert!(matches!(
            set_enabled(&manager, &mut config, true),
            Err(TunManagerError::ProviderPathMismatch { .. })
        ));
        assert!(!config.tun.enabled);
    }

    /// A phone's tunnel belongs to its host app. Its controller replaces the
    /// platform one here too, or every status read would report the
    /// `voya-platform` refusal as a missing component.
    #[test]
    fn a_host_owned_tunnel_reports_the_host_controller_status() {
        struct RunningHostTunnel;

        impl NativeTunController for RunningHostTunnel {
            fn status(&self, backend: PlatformTunBackend) -> voya_platform::tun::NativeTunStatus {
                voya_platform::tun::NativeTunStatus {
                    backend,
                    provider_state: NativeTunProviderState::Running,
                    component_ready: true,
                    message: None,
                }
            }

            fn start(
                &self,
                _request: voya_platform::tun::NativeTunStartRequest,
            ) -> Result<(), voya_platform::tun::NativeTunError> {
                Ok(())
            }

            fn stop(
                &self,
                _backend: PlatformTunBackend,
            ) -> Result<(), voya_platform::tun::NativeTunError> {
                Ok(())
            }
        }

        let manager = TunManager::with_target_os(Arc::new(ElevationState::new()), TargetOs::Ios)
            .with_native_tun_controller(Arc::new(RunningHostTunnel));

        let status = manager
            .status(AppConfig::default().tun.enabled)
            .expect("status");

        assert_eq!(status.backend, TunBackend::IosPacketTunnel);
        assert_eq!(status.provider_state, TunProviderState::Running);
        assert!(status.native_component_ready);
        assert_eq!(status.last_provider_error, None);
        assert!(status.allow_enable_tun);
    }
}
