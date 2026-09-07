use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[cfg(any(target_os = "macos", windows))]
use std::process::Command;

use thiserror::Error;

use crate::coreinfo::{CoreLaunch, TargetOs};

pub const MACOS_PACKET_TUNNEL_BUNDLE_ID: &str = "app.voyavpn.desktop.PacketTunnel";
pub const WINDOWS_TUN_SERVICE_NAME: &str = "VoyaVPNTunnelService";
pub const MACOS_PACKET_TUNNEL_START_TIMEOUT_MS: i64 = 20_000;
#[cfg(target_os = "macos")]
const MACOS_PACKET_TUNNEL_APPEX_NAME: &str = "app.voyavpn.desktop.PacketTunnel.appex";
#[cfg(target_os = "macos")]
const MACOS_PACKET_TUNNEL_SYSEX_NAME: &str = "app.voyavpn.desktop.PacketTunnel.systemextension";
const MACOS_PROVIDER_STATUS_RELATIVE_PATH: &str =
    "Library/Application Support/VoyaVPN/packet-tunnel-status.json";
const MACOS_PROVIDER_LOG_RELATIVE_PATH: &str = "Library/Application Support/VoyaVPN/provider.log";
const PROVIDER_LOG_TAIL_LINES: usize = 200;

#[cfg(any(target_os = "macos", windows, test))]
fn command_output_text(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    if stderr.trim().is_empty() {
        stdout.into_owned()
    } else if stdout.trim().is_empty() {
        stderr.into_owned()
    } else {
        format!("{stdout}\n{stderr}")
    }
}

pub const WINDOWS_TUN_DEVICES: &[WindowsTunDevice] = &[WindowsTunDevice {
    name: "wintunsingbox_tun",
    guid: "b738a021-9842-444c-10b0-a4e3f65ab5b6",
}];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsTunDevice {
    pub name: &'static str,
    pub guid: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunBackend {
    Process,
    MacosPacketTunnel,
    WindowsService,
    Unsupported,
}

impl TunBackend {
    #[must_use]
    pub const fn is_native(self) -> bool {
        matches!(self, Self::MacosPacketTunnel | Self::WindowsService)
    }
}

#[must_use]
pub const fn tun_backend(os: TargetOs) -> TunBackend {
    match os {
        TargetOs::Windows => TunBackend::WindowsService,
        TargetOs::Linux => TunBackend::Process,
        TargetOs::Macos => TunBackend::MacosPacketTunnel,
        TargetOs::Other => TunBackend::Unsupported,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunPreflightState {
    Ready,
    NeedsElevation,
    ManualCheck,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunPreflightReport {
    pub os: TargetOs,
    pub backend: TunBackend,
    pub state: TunPreflightState,
    pub allow_enable_tun: bool,
    pub requires_elevation: bool,
    pub elevation_granted: bool,
    pub notes: Vec<String>,
    pub route_restore_note: String,
    pub windows_cleanup_devices: Vec<WindowsTunDevice>,
}

#[must_use]
pub const fn allow_enable_tun(os: TargetOs, elevation_granted: bool) -> bool {
    match tun_backend(os) {
        TunBackend::Process => elevation_granted,
        TunBackend::MacosPacketTunnel | TunBackend::WindowsService => true,
        TunBackend::Unsupported => false,
    }
}

#[must_use]
pub fn tun_preflight(os: TargetOs, elevation_granted: bool) -> TunPreflightReport {
    let backend = tun_backend(os);
    let requires_elevation = matches!(backend, TunBackend::Process);
    let allow_enable_tun = allow_enable_tun(os, elevation_granted);
    let state = match backend {
        TunBackend::Process if elevation_granted => TunPreflightState::Ready,
        TunBackend::Process => TunPreflightState::NeedsElevation,
        TunBackend::MacosPacketTunnel | TunBackend::WindowsService => TunPreflightState::Ready,
        TunBackend::Unsupported => TunPreflightState::Unsupported,
    };

    TunPreflightReport {
        os,
        backend,
        state,
        allow_enable_tun,
        requires_elevation,
        elevation_granted,
        notes: tun_preflight_notes(os, elevation_granted),
        route_restore_note: route_restore_note(os).to_string(),
        windows_cleanup_devices: if os == TargetOs::Windows {
            WINDOWS_TUN_DEVICES.to_vec()
        } else {
            Vec::new()
        },
    }
}

fn tun_preflight_notes(os: TargetOs, elevation_granted: bool) -> Vec<String> {
    match tun_backend(os) {
        TunBackend::WindowsService => vec![
            "Windows transparent proxy is owned by the VoyaVPN Service, which runs sing-box and Wintun outside the desktop UI process."
                .to_string(),
            "The desktop app only writes the runtime config and asks the service to start or stop the tunnel."
                .to_string(),
        ],
        TunBackend::MacosPacketTunnel => vec![
            "macOS transparent proxy is owned by a Network Extension PacketTunnel provider, matching the system VPN model used by V2Box."
                .to_string(),
            "The desktop app only writes the runtime config and asks macOS to start or stop the VPN profile."
                .to_string(),
        ],
        TunBackend::Process if elevation_granted => vec![
            "Unix TUN start runs the core through the root-owned elevation launcher granted at enable time."
                .to_string(),
            "The elevated process is killed first during disconnect before regular process teardown."
                .to_string(),
        ],
        TunBackend::Process => vec![
            "Unix TUN start requires a one-time native authorization before enabling TUN; no admin password is stored."
                .to_string(),
        ],
        TunBackend::Unsupported => {
            vec!["TUN mode is not supported on this platform yet.".to_string()]
        }
    }
}

fn route_restore_note(os: TargetOs) -> &'static str {
    match tun_backend(os) {
        TunBackend::WindowsService => {
            "Disconnect asks the VoyaVPN Service to stop sing-box so Wintun routes and DNS state are restored by the service-owned lifecycle."
        }
        TunBackend::MacosPacketTunnel => {
            "Disconnect asks macOS to stop the PacketTunnel VPN profile so routes and DNS state are restored by NetworkExtension."
        }
        TunBackend::Process => {
            "Disconnect runs sudo kill for elevated TUN cores before normal teardown so core-owned routes can be restored by process exit."
        }
        TunBackend::Unsupported => "No route mutation is attempted on unsupported platforms.",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTunProviderState {
    NotApplicable,
    MissingComponent,
    PermissionRequired,
    Stopped,
    Starting,
    Running,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTunStatus {
    pub backend: TunBackend,
    pub provider_state: NativeTunProviderState,
    pub component_ready: bool,
    pub message: Option<String>,
}

impl NativeTunStatus {
    #[must_use]
    pub fn not_applicable(backend: TunBackend) -> Self {
        Self {
            backend,
            provider_state: NativeTunProviderState::NotApplicable,
            component_ready: true,
            message: None,
        }
    }

    #[must_use]
    pub fn missing_component(backend: TunBackend, message: impl Into<String>) -> Self {
        Self {
            backend,
            provider_state: NativeTunProviderState::MissingComponent,
            component_ready: false,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeTunProviderStatusFile {
    pub state: Option<String>,
    pub last_error: Option<String>,
    pub provider_bundle_path: Option<String>,
    pub breadcrumbs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTunDiagnostics {
    pub backend: TunBackend,
    pub container_path: Option<PathBuf>,
    pub status_path: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
    pub packaging_mode: Option<String>,
    pub expected_provider_path: Option<PathBuf>,
    pub system_extension_state: Option<String>,
    pub registration_paths: Vec<String>,
    pub status: Option<NativeTunProviderStatusFile>,
    pub provider_log_tail: Vec<String>,
    pub host_log_tail: Vec<String>,
    pub message: Option<String>,
}

impl NativeTunDiagnostics {
    #[must_use]
    pub fn empty(backend: TunBackend) -> Self {
        Self {
            backend,
            container_path: None,
            status_path: None,
            log_path: None,
            packaging_mode: None,
            expected_provider_path: None,
            system_extension_state: None,
            registration_paths: Vec::new(),
            status: None,
            provider_log_tail: Vec::new(),
            host_log_tail: Vec::new(),
            message: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTunStartRequest {
    pub backend: TunBackend,
    pub active_profile_id: Option<String>,
    pub main_launch: CoreLaunch,
    pub pre_launch: Option<CoreLaunch>,
    pub main_config_path: PathBuf,
    pub pre_config_path: Option<PathBuf>,
}

pub trait NativeTunController: Send + Sync {
    fn status(&self, backend: TunBackend) -> NativeTunStatus;
    fn start(&self, request: NativeTunStartRequest) -> Result<(), NativeTunError>;
    fn stop(&self, backend: TunBackend) -> Result<(), NativeTunError>;

    fn diagnostics(&self, backend: TunBackend) -> NativeTunDiagnostics {
        NativeTunDiagnostics::empty(backend)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopNativeTunController;

impl NativeTunController for NoopNativeTunController {
    fn status(&self, backend: TunBackend) -> NativeTunStatus {
        NativeTunStatus::not_applicable(backend)
    }

    fn start(&self, request: NativeTunStartRequest) -> Result<(), NativeTunError> {
        if request.backend.is_native() {
            return Err(NativeTunError::ControllerUnavailable {
                backend: request.backend,
                message: "native tunnel controller is not installed in this test runtime"
                    .to_string(),
            });
        }

        Ok(())
    }

    fn stop(&self, _backend: TunBackend) -> Result<(), NativeTunError> {
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformNativeTunController;

impl NativeTunController for PlatformNativeTunController {
    fn status(&self, backend: TunBackend) -> NativeTunStatus {
        platform_native_tun_status(backend)
    }

    fn start(&self, request: NativeTunStartRequest) -> Result<(), NativeTunError> {
        platform_native_tun_start(request)
    }

    fn stop(&self, backend: TunBackend) -> Result<(), NativeTunError> {
        platform_native_tun_stop(backend)
    }

    fn diagnostics(&self, backend: TunBackend) -> NativeTunDiagnostics {
        platform_native_tun_diagnostics(backend)
    }
}

pub trait ProviderRegistrationResolver: Send + Sync {
    fn expected_provider_path(&self, bundle_id: &str) -> Option<PathBuf>;
    fn resolved_provider_paths(&self, bundle_id: &str) -> Result<Vec<PathBuf>, NativeTunError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProviderRegistrationResolver;

impl ProviderRegistrationResolver for NoopProviderRegistrationResolver {
    fn expected_provider_path(&self, _bundle_id: &str) -> Option<PathBuf> {
        None
    }

    fn resolved_provider_paths(&self, _bundle_id: &str) -> Result<Vec<PathBuf>, NativeTunError> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformProviderRegistrationResolver;

impl ProviderRegistrationResolver for PlatformProviderRegistrationResolver {
    fn expected_provider_path(&self, bundle_id: &str) -> Option<PathBuf> {
        if bundle_id == MACOS_PACKET_TUNNEL_BUNDLE_ID {
            return macos_packet_tunnel_appex_path().filter(|path| path.exists());
        }
        None
    }

    fn resolved_provider_paths(&self, bundle_id: &str) -> Result<Vec<PathBuf>, NativeTunError> {
        platform_provider_registration_paths(bundle_id)
    }
}

fn platform_native_tun_status(backend: TunBackend) -> NativeTunStatus {
    match backend {
        TunBackend::Process => NativeTunStatus::not_applicable(backend),
        TunBackend::MacosPacketTunnel => macos_packet_tunnel_status(),
        TunBackend::WindowsService => windows_service_status(),
        TunBackend::Unsupported => NativeTunStatus::missing_component(
            backend,
            "no native TUN backend is available for this platform",
        ),
    }
}

fn platform_native_tun_diagnostics(backend: TunBackend) -> NativeTunDiagnostics {
    match backend {
        TunBackend::MacosPacketTunnel => macos_packet_tunnel_diagnostics(),
        _ => NativeTunDiagnostics::empty(backend),
    }
}

fn platform_native_tun_start(request: NativeTunStartRequest) -> Result<(), NativeTunError> {
    if request.backend.is_native() {
        ensure_native_tun_config_has_tun_inbound(request.backend, &request.main_config_path)?;
    }

    match request.backend {
        TunBackend::Process => Ok(()),
        TunBackend::MacosPacketTunnel => start_macos_packet_tunnel(&request),
        TunBackend::WindowsService => start_windows_tun_service(&request),
        TunBackend::Unsupported => Err(NativeTunError::UnsupportedBackend(request.backend)),
    }
}

fn ensure_native_tun_config_has_tun_inbound(
    backend: TunBackend,
    config_path: &Path,
) -> Result<(), NativeTunError> {
    let config_text =
        fs::read_to_string(config_path).map_err(|source| NativeTunError::Command {
            action: "read native TUN config",
            source,
        })?;
    let config: serde_json::Value =
        serde_json::from_str(&config_text).map_err(|source| NativeTunError::InvalidRequest {
            backend,
            message: format!("native TUN config is not valid JSON: {source}"),
        })?;

    let has_tun_inbound = config
        .get("inbounds")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|inbounds| {
            inbounds.iter().any(|inbound| {
                inbound.get("type").and_then(serde_json::Value::as_str) == Some("tun")
            })
        });

    if has_tun_inbound {
        return Ok(());
    }

    Err(NativeTunError::InvalidRequest {
        backend,
        message: "native TUN config does not contain a tun inbound; reconnect from the TUN toggle or regenerate runtime config"
            .to_string(),
    })
}

fn platform_native_tun_stop(backend: TunBackend) -> Result<(), NativeTunError> {
    match backend {
        TunBackend::Process => Ok(()),
        TunBackend::MacosPacketTunnel => stop_macos_packet_tunnel(),
        TunBackend::WindowsService => stop_windows_tun_service(),
        TunBackend::Unsupported => Err(NativeTunError::UnsupportedBackend(backend)),
    }
}

mod macos;
pub use macos::{
    ensure_macos_provider_path_matches, parse_pluginkit_matches, parse_provider_status_json,
    parse_systemextensionsctl_matches, parse_systemextensionsctl_state,
};
use macos::{
    macos_packet_tunnel_appex_path, macos_packet_tunnel_diagnostics, macos_packet_tunnel_status,
    platform_provider_registration_paths, start_macos_packet_tunnel, stop_macos_packet_tunnel,
};

// `self::` keeps this resolving to the module below even if the `windows`
// crate is ever added as a dependency.
mod windows;
#[cfg(windows)]
use self::windows::windows_command;
use self::windows::{start_windows_tun_service, stop_windows_tun_service, windows_service_status};

pub trait TunCleaner: Send + Sync {
    fn cleanup_before_start(&self) -> Result<(), TunCleanupError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTunCleaner;

impl TunCleaner for NoopTunCleaner {
    fn cleanup_before_start(&self) -> Result<(), TunCleanupError> {
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformTunCleaner;

impl TunCleaner for PlatformTunCleaner {
    fn cleanup_before_start(&self) -> Result<(), TunCleanupError> {
        platform_cleanup_before_start()
    }
}

#[cfg(windows)]
fn platform_cleanup_before_start() -> Result<(), TunCleanupError> {
    for device in WINDOWS_TUN_DEVICES {
        let output = windows_command(r"C:\Windows\System32\pnputil.exe")
            .args([
                "/remove-device",
                &format!(r"SWD\Wintun\{{{}}}", device.guid),
            ])
            .output()
            .map_err(TunCleanupError::Command)?;

        windows_cleanup_result(
            device,
            output.status.success(),
            output.status.code(),
            &output.stderr,
        )?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn platform_cleanup_before_start() -> Result<(), TunCleanupError> {
    Ok(())
}

#[cfg(any(windows, test))]
fn windows_cleanup_result(
    device: &WindowsTunDevice,
    success: bool,
    status_code: Option<i32>,
    stderr: &[u8],
) -> Result<(), TunCleanupError> {
    if success {
        return Ok(());
    }

    Err(TunCleanupError::CommandFailed {
        device: device.name,
        status_code,
        stderr: String::from_utf8_lossy(stderr).into_owned(),
    })
}

#[derive(Debug, Error)]
pub enum TunCleanupError {
    #[error("failed to run Windows TUN cleanup command: {0}")]
    Command(io::Error),
    #[error(
        "Windows TUN cleanup command failed for {device} with status {status_code:?}: {stderr}"
    )]
    CommandFailed {
        device: &'static str,
        status_code: Option<i32>,
        stderr: String,
    },
}

#[derive(Debug, Error)]
pub enum NativeTunError {
    #[error("native TUN backend {0:?} is not supported")]
    UnsupportedBackend(TunBackend),
    #[error("native TUN component is missing for {backend:?}: {message}")]
    ComponentMissing {
        backend: TunBackend,
        message: String,
    },
    #[error("native TUN controller is unavailable for {backend:?}: {message}")]
    ControllerUnavailable {
        backend: TunBackend,
        message: String,
    },
    #[error("native TUN request is invalid for {backend:?}: {message}")]
    InvalidRequest {
        backend: TunBackend,
        message: String,
    },
    #[error("native TUN permission is required for {backend:?}: {message}")]
    PermissionRequired {
        backend: TunBackend,
        message: String,
    },
    #[error(
        "macOS PacketTunnel provider path mismatch: expected {expected}, PlugInKit elected {resolved}",
        expected = expected.display(),
        resolved = resolved.display()
    )]
    ProviderPathMismatch {
        expected: PathBuf,
        resolved: PathBuf,
    },
    #[error("failed to {action}: {source}")]
    Command {
        action: &'static str,
        source: io::Error,
    },
    #[error("{action} failed with status {status_code:?}: {output}")]
    CommandFailed {
        action: &'static str,
        status_code: Option<i32>,
        output: String,
    },
}

#[cfg(test)]
mod tests {
    use super::macos::parse_macos_provider_state;
    use super::*;

    #[test]
    fn process_windows_tun_cleanup_abstraction_names_reference_devices() {
        assert_eq!(WINDOWS_TUN_DEVICES.len(), 1);
        assert_eq!(WINDOWS_TUN_DEVICES[0].name, "wintunsingbox_tun");
        assert!(WINDOWS_TUN_DEVICES
            .iter()
            .all(|device| device.guid.len() == 36));
    }

    #[test]
    fn process_noop_tun_cleaner_is_deterministic_for_tests() {
        NoopTunCleaner.cleanup_before_start().expect("noop cleanup");
    }

    #[test]
    fn native_tun_config_validation_requires_tun_inbound() {
        let path = temp_config_path("native-tun-missing.json");
        fs::write(
            &path,
            r#"{"inbounds":[{"type":"mixed","listen":"127.0.0.1","listen_port":10808}]}"#,
        )
        .expect("write temp config");

        let error =
            ensure_native_tun_config_has_tun_inbound(TunBackend::MacosPacketTunnel, path.as_path())
                .expect_err("missing tun inbound should fail");

        assert!(matches!(
            error,
            NativeTunError::InvalidRequest {
                backend: TunBackend::MacosPacketTunnel,
                ref message,
            } if message.contains("does not contain a tun inbound")
        ));

        fs::remove_file(path).expect("remove temp config");
    }

    #[test]
    fn native_tun_config_validation_accepts_tun_inbound() {
        let path = temp_config_path("native-tun-valid.json");
        fs::write(
            &path,
            r#"{"inbounds":[{"type":"mixed"},{"type":"tun","auto_route":true}]}"#,
        )
        .expect("write temp config");

        ensure_native_tun_config_has_tun_inbound(TunBackend::WindowsService, path.as_path())
            .expect("tun inbound should pass");

        fs::remove_file(path).expect("remove temp config");
    }

    #[test]
    fn process_windows_tun_cleanup_failure_is_returned() {
        let error = windows_cleanup_result(
            &WINDOWS_TUN_DEVICES[0],
            false,
            Some(1),
            b"device removal failed",
        )
        .expect_err("failed cleanup should propagate");

        assert!(matches!(
            error,
            TunCleanupError::CommandFailed {
                device: "wintunsingbox_tun",
                status_code: Some(1),
                ref stderr,
            } if stderr == "device removal failed"
        ));
    }

    #[test]
    fn tun_allow_enable_matches_platform_backend() {
        assert!(!allow_enable_tun(TargetOs::Linux, false));
        assert!(allow_enable_tun(TargetOs::Linux, true));
        assert!(allow_enable_tun(TargetOs::Macos, false));
        assert!(allow_enable_tun(TargetOs::Macos, true));
        assert!(allow_enable_tun(TargetOs::Windows, false));
    }

    #[test]
    fn tun_backend_selects_native_macos_and_windows() {
        assert_eq!(tun_backend(TargetOs::Macos), TunBackend::MacosPacketTunnel);
        assert_eq!(tun_backend(TargetOs::Windows), TunBackend::WindowsService);
        assert_eq!(tun_backend(TargetOs::Linux), TunBackend::Process);
        assert_eq!(tun_backend(TargetOs::Other), TunBackend::Unsupported);
    }

    #[test]
    fn macos_provider_status_parser_maps_known_states() {
        assert_eq!(
            parse_macos_provider_state("running\n"),
            NativeTunProviderState::Running
        );
        assert_eq!(
            parse_macos_provider_state("permissionRequired"),
            NativeTunProviderState::PermissionRequired
        );
        assert_eq!(
            parse_macos_provider_state("unexpected"),
            NativeTunProviderState::Error
        );
    }

    #[test]
    fn command_output_combines_stdout_and_stderr_without_losing_context() {
        assert_eq!(command_output_text(b"stdout", b""), "stdout");
        assert_eq!(command_output_text(b"", b"stderr"), "stderr");
        assert_eq!(command_output_text(b"stdout", b"stderr"), "stdout\nstderr");
    }

    #[test]
    fn provider_status_json_parser_extracts_status_error_and_breadcrumbs() {
        let status = parse_provider_status_json(
            r#"{
              "state":"failed",
              "lastError":"sing-box runtime unavailable",
              "providerBundlePath":"/tmp/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
              "breadcrumbs":["starting","failed: sing-box runtime unavailable"]
            }"#,
        )
        .expect("provider status JSON should parse");

        assert_eq!(status.state.as_deref(), Some("failed"));
        assert_eq!(
            status.last_error.as_deref(),
            Some("sing-box runtime unavailable")
        );
        assert_eq!(
            status.provider_bundle_path.as_deref(),
            Some("/tmp/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex")
        );
        assert_eq!(
            status.breadcrumbs,
            ["starting", "failed: sing-box runtime unavailable"]
        );
    }

    #[test]
    fn pluginkit_parser_extracts_unique_appex_paths_from_verbose_output() {
        let output = r#"
+    app.voyavpn.desktop.PacketTunnel(0.1)
        Path = /Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex
        UUID = 11111111-1111-1111-1111-111111111111
-    app.voyavpn.desktop.PacketTunnel(0.1)
        Path = "/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex"
        Path = /Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex
"#;

        let matches = parse_pluginkit_matches(output);

        assert_eq!(
            matches,
            [
                PathBuf::from(
                    "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex"
                ),
                PathBuf::from(
                    "/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex"
                )
            ]
        );
    }

    #[test]
    fn systemextensionsctl_parser_extracts_state_lines() {
        let output = r#"
2 extension(s)
--- com.apple.system_extension.network_extension
enabled active teamID bundleID (version) name [state]
* * 4LUKJ56532 app.voyavpn.desktop.PacketTunnel (0.1.0/1) VoyaVPN PacketTunnel [activated enabled]
    4LUKJ56532 app.example.Other (1/1) Other [activated waiting for user]
"#;

        assert_eq!(
            parse_systemextensionsctl_matches(output, MACOS_PACKET_TUNNEL_BUNDLE_ID),
            ["* * 4LUKJ56532 app.voyavpn.desktop.PacketTunnel (0.1.0/1) VoyaVPN PacketTunnel [activated enabled]"]
        );
        assert_eq!(
            parse_systemextensionsctl_state(output, MACOS_PACKET_TUNNEL_BUNDLE_ID).as_deref(),
            Some("activated enabled")
        );
    }

    #[derive(Debug)]
    struct StaticProviderRegistrationResolver {
        expected: Option<PathBuf>,
        resolved: Result<Vec<PathBuf>, NativeTunError>,
    }

    impl ProviderRegistrationResolver for StaticProviderRegistrationResolver {
        fn expected_provider_path(&self, _bundle_id: &str) -> Option<PathBuf> {
            self.expected.clone()
        }

        fn resolved_provider_paths(
            &self,
            _bundle_id: &str,
        ) -> Result<Vec<PathBuf>, NativeTunError> {
            match &self.resolved {
                Ok(paths) => Ok(paths.clone()),
                Err(NativeTunError::CommandFailed {
                    action,
                    status_code,
                    output,
                }) => Err(NativeTunError::CommandFailed {
                    action,
                    status_code: *status_code,
                    output: output.clone(),
                }),
                Err(error) => panic!("unexpected resolver error in test: {error}"),
            }
        }
    }

    #[test]
    fn provider_path_precheck_fails_open_on_empty_or_query_error() {
        ensure_macos_provider_path_matches(&StaticProviderRegistrationResolver {
            expected: Some(PathBuf::from(
                "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
            )),
            resolved: Ok(Vec::new()),
        })
        .expect("empty PlugInKit result should fail open");

        ensure_macos_provider_path_matches(&StaticProviderRegistrationResolver {
            expected: Some(PathBuf::from(
                "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
            )),
            resolved: Err(NativeTunError::CommandFailed {
                action: "query macOS PacketTunnel provider registration",
                status_code: Some(1),
                output: "pluginkit unavailable".to_string(),
            }),
        })
        .expect("PlugInKit query failure should fail open");
    }

    #[test]
    fn provider_path_precheck_blocks_clear_mismatch() {
        let error = ensure_macos_provider_path_matches(&StaticProviderRegistrationResolver {
            expected: Some(PathBuf::from("/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex")),
            resolved: Ok(vec![PathBuf::from("/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex")]),
        })
        .expect_err("mismatch should fail");

        assert!(matches!(error, NativeTunError::ProviderPathMismatch { .. }));
    }

    #[test]
    fn tun_preflight_reports_backend_restore_notes_by_platform() {
        let windows = tun_preflight(TargetOs::Windows, false);
        assert_eq!(windows.backend, TunBackend::WindowsService);
        assert_eq!(windows.state, TunPreflightState::Ready);
        assert_eq!(windows.windows_cleanup_devices, WINDOWS_TUN_DEVICES);
        assert!(windows.route_restore_note.contains("VoyaVPN Service"));

        let macos = tun_preflight(TargetOs::Macos, false);
        assert_eq!(macos.backend, TunBackend::MacosPacketTunnel);
        assert_eq!(macos.state, TunPreflightState::Ready);
        assert!(!macos.requires_elevation);
        assert!(macos.route_restore_note.contains("PacketTunnel"));

        let linux_missing = tun_preflight(TargetOs::Linux, false);
        assert_eq!(linux_missing.backend, TunBackend::Process);
        assert_eq!(linux_missing.state, TunPreflightState::NeedsElevation);
        assert!(!linux_missing.allow_enable_tun);

        let linux_ready = tun_preflight(TargetOs::Linux, true);
        assert_eq!(linux_ready.state, TunPreflightState::Ready);
        assert!(linux_ready.allow_enable_tun);
        assert!(linux_ready.route_restore_note.contains("sudo kill"));
    }

    fn temp_config_path(file_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("voyavpn-{file_name}-{}", std::process::id()))
    }
}
