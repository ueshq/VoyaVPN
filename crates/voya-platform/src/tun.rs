use std::{io, path::PathBuf};

#[cfg(any(windows, target_os = "macos"))]
use std::process::Command;

use thiserror::Error;

use crate::coreinfo::{CoreLaunch, TargetOs};

pub const MACOS_PACKET_TUNNEL_BUNDLE_ID: &str = "app.voyavpn.desktop.PacketTunnel";
pub const MACOS_TUNNEL_HELPER_NAME: &str = "voyavpn-macos-tunnelctl";
pub const WINDOWS_TUN_SERVICE_NAME: &str = "VoyaVPNTunnelService";

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

fn platform_native_tun_start(request: NativeTunStartRequest) -> Result<(), NativeTunError> {
    match request.backend {
        TunBackend::Process => Ok(()),
        TunBackend::MacosPacketTunnel => start_macos_packet_tunnel(&request),
        TunBackend::WindowsService => start_windows_tun_service(&request),
        TunBackend::Unsupported => Err(NativeTunError::UnsupportedBackend(request.backend)),
    }
}

fn platform_native_tun_stop(backend: TunBackend) -> Result<(), NativeTunError> {
    match backend {
        TunBackend::Process => Ok(()),
        TunBackend::MacosPacketTunnel => stop_macos_packet_tunnel(),
        TunBackend::WindowsService => stop_windows_tun_service(),
        TunBackend::Unsupported => Err(NativeTunError::UnsupportedBackend(backend)),
    }
}

fn macos_packet_tunnel_status() -> NativeTunStatus {
    if let Err(status) = require_bundled_component(
        macos_packet_tunnel_extension_path(),
        "PacketTunnel extension is not bundled in this build",
    ) {
        return status;
    }
    let helper_path = match require_bundled_component(
        macos_tunnel_helper_path(),
        "PacketTunnel helper is not bundled in this build",
    ) {
        Ok(path) => path,
        Err(status) => return status,
    };

    match macos_tunnel_helper_status(&helper_path) {
        Ok(provider_state) => NativeTunStatus {
            backend: TunBackend::MacosPacketTunnel,
            provider_state,
            component_ready: true,
            message: None,
        },
        Err(error) => NativeTunStatus {
            backend: TunBackend::MacosPacketTunnel,
            provider_state: NativeTunProviderState::Error,
            component_ready: true,
            message: Some(error.to_string()),
        },
    }
}

/// Resolve a bundled macOS PacketTunnel component path, returning a
/// `missing_component` status when it is absent from the build.
fn require_bundled_component(
    path: Option<PathBuf>,
    missing_message: &'static str,
) -> Result<PathBuf, NativeTunStatus> {
    match path {
        Some(path) if path.exists() => Ok(path),
        _ => Err(NativeTunStatus::missing_component(
            TunBackend::MacosPacketTunnel,
            missing_message,
        )),
    }
}

#[cfg(target_os = "macos")]
fn macos_tunnel_helper_path() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let macos_dir = executable.parent()?;
    Some(macos_dir.join(MACOS_TUNNEL_HELPER_NAME))
}

#[cfg(not(target_os = "macos"))]
fn macos_tunnel_helper_path() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn macos_tunnel_helper_status(
    helper_path: &std::path::Path,
) -> Result<NativeTunProviderState, NativeTunError> {
    let output = Command::new(helper_path)
        .arg("status")
        .output()
        .map_err(|source| NativeTunError::Command {
            action: "query macOS PacketTunnel status",
            source,
        })?;
    if !output.status.success() {
        return Err(NativeTunError::CommandFailed {
            action: "query macOS PacketTunnel status",
            status_code: output.status.code(),
            output: command_output_text(&output.stdout, &output.stderr),
        });
    }

    Ok(parse_macos_provider_state(&command_output_text(
        &output.stdout,
        &output.stderr,
    )))
}

#[cfg(not(target_os = "macos"))]
fn macos_tunnel_helper_status(
    _helper_path: &std::path::Path,
) -> Result<NativeTunProviderState, NativeTunError> {
    Err(NativeTunError::ComponentMissing {
        backend: TunBackend::MacosPacketTunnel,
        message: "PacketTunnel helper is not available on this platform".to_string(),
    })
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_provider_state(output: &str) -> NativeTunProviderState {
    match output.trim() {
        "running" => NativeTunProviderState::Running,
        "starting" => NativeTunProviderState::Starting,
        "stopped" => NativeTunProviderState::Stopped,
        "permissionRequired" => NativeTunProviderState::PermissionRequired,
        "missingComponent" => NativeTunProviderState::MissingComponent,
        "notApplicable" => NativeTunProviderState::NotApplicable,
        _ => NativeTunProviderState::Error,
    }
}

#[cfg(any(windows, target_os = "macos"))]
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

#[cfg(target_os = "macos")]
fn macos_packet_tunnel_extension_path() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let contents_dir = executable
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "Contents"))?;

    Some(
        contents_dir
            .join("PlugIns")
            .join(format!("{MACOS_PACKET_TUNNEL_BUNDLE_ID}.appex")),
    )
}

#[cfg(not(target_os = "macos"))]
fn macos_packet_tunnel_extension_path() -> Option<PathBuf> {
    None
}

fn start_macos_packet_tunnel(request: &NativeTunStartRequest) -> Result<(), NativeTunError> {
    let status = macos_packet_tunnel_status();
    if !status.component_ready {
        return Err(NativeTunError::ComponentMissing {
            backend: TunBackend::MacosPacketTunnel,
            message: status
                .message
                .unwrap_or_else(|| "PacketTunnel extension is missing".to_string()),
        });
    }

    start_macos_packet_tunnel_with_helper(request)
}

#[cfg(target_os = "macos")]
fn start_macos_packet_tunnel_with_helper(
    request: &NativeTunStartRequest,
) -> Result<(), NativeTunError> {
    let Some(helper_path) = macos_tunnel_helper_path() else {
        return Err(NativeTunError::ComponentMissing {
            backend: TunBackend::MacosPacketTunnel,
            message: "PacketTunnel helper is missing".to_string(),
        });
    };
    let mut command = Command::new(helper_path);
    command
        .arg("start")
        .arg("--config")
        .arg(&request.main_config_path);
    if let Some(active_profile_id) = &request.active_profile_id {
        command.arg("--profile").arg(active_profile_id);
    }
    let output = command.output().map_err(|source| NativeTunError::Command {
        action: "start macOS PacketTunnel",
        source,
    })?;
    if output.status.success() {
        return Ok(());
    }

    Err(NativeTunError::CommandFailed {
        action: "start macOS PacketTunnel",
        status_code: output.status.code(),
        output: command_output_text(&output.stdout, &output.stderr),
    })
}

#[cfg(not(target_os = "macos"))]
fn start_macos_packet_tunnel_with_helper(
    _request: &NativeTunStartRequest,
) -> Result<(), NativeTunError> {
    Err(NativeTunError::ComponentMissing {
        backend: TunBackend::MacosPacketTunnel,
        message: "PacketTunnel helper is not available on this platform".to_string(),
    })
}

fn stop_macos_packet_tunnel() -> Result<(), NativeTunError> {
    stop_macos_packet_tunnel_with_helper()
}

#[cfg(target_os = "macos")]
fn stop_macos_packet_tunnel_with_helper() -> Result<(), NativeTunError> {
    let Some(helper_path) = macos_tunnel_helper_path() else {
        return Ok(());
    };
    if !helper_path.exists() {
        return Ok(());
    }
    let output = Command::new(helper_path)
        .arg("stop")
        .output()
        .map_err(|source| NativeTunError::Command {
            action: "stop macOS PacketTunnel",
            source,
        })?;
    if output.status.success() {
        return Ok(());
    }

    Err(NativeTunError::CommandFailed {
        action: "stop macOS PacketTunnel",
        status_code: output.status.code(),
        output: command_output_text(&output.stdout, &output.stderr),
    })
}

#[cfg(not(target_os = "macos"))]
fn stop_macos_packet_tunnel_with_helper() -> Result<(), NativeTunError> {
    Ok(())
}

#[cfg(windows)]
fn windows_service_status() -> NativeTunStatus {
    let output = match Command::new(r"C:\Windows\System32\sc.exe")
        .args(["query", WINDOWS_TUN_SERVICE_NAME])
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return NativeTunStatus {
                backend: TunBackend::WindowsService,
                provider_state: NativeTunProviderState::Error,
                component_ready: false,
                message: Some(format!("failed to query Windows service: {error}")),
            };
        }
    };

    let text = command_output_text(&output.stdout, &output.stderr);
    if !output.status.success() {
        return NativeTunStatus::missing_component(
            TunBackend::WindowsService,
            if text.trim().is_empty() {
                format!("Windows service {WINDOWS_TUN_SERVICE_NAME} is not installed")
            } else {
                text
            },
        );
    }

    let provider_state = if text.contains("RUNNING") {
        NativeTunProviderState::Running
    } else if text.contains("START_PENDING") || text.contains("STOP_PENDING") {
        NativeTunProviderState::Starting
    } else {
        NativeTunProviderState::Stopped
    };

    NativeTunStatus {
        backend: TunBackend::WindowsService,
        provider_state,
        component_ready: true,
        message: None,
    }
}

#[cfg(not(windows))]
fn windows_service_status() -> NativeTunStatus {
    NativeTunStatus::missing_component(
        TunBackend::WindowsService,
        format!("Windows service {WINDOWS_TUN_SERVICE_NAME} is not installed in this build"),
    )
}

#[cfg(windows)]
fn start_windows_tun_service(request: &NativeTunStartRequest) -> Result<(), NativeTunError> {
    let output = Command::new(r"C:\Windows\System32\sc.exe")
        .arg("start")
        .arg(WINDOWS_TUN_SERVICE_NAME)
        .arg(request.main_config_path.to_string_lossy().as_ref())
        .output()
        .map_err(|source| NativeTunError::Command {
            action: "start Windows tunnel service",
            source,
        })?;

    windows_service_command_result("start Windows tunnel service", output)
}

#[cfg(not(windows))]
fn start_windows_tun_service(_request: &NativeTunStartRequest) -> Result<(), NativeTunError> {
    Err(NativeTunError::ComponentMissing {
        backend: TunBackend::WindowsService,
        message: format!("Windows service {WINDOWS_TUN_SERVICE_NAME} is not available"),
    })
}

#[cfg(windows)]
fn stop_windows_tun_service() -> Result<(), NativeTunError> {
    let output = Command::new(r"C:\Windows\System32\sc.exe")
        .args(["stop", WINDOWS_TUN_SERVICE_NAME])
        .output()
        .map_err(|source| NativeTunError::Command {
            action: "stop Windows tunnel service",
            source,
        })?;

    windows_service_command_result("stop Windows tunnel service", output)
}

#[cfg(not(windows))]
fn stop_windows_tun_service() -> Result<(), NativeTunError> {
    Ok(())
}

#[cfg(windows)]
fn windows_service_command_result(
    action: &'static str,
    output: std::process::Output,
) -> Result<(), NativeTunError> {
    if output.status.success() {
        return Ok(());
    }

    Err(NativeTunError::CommandFailed {
        action,
        status_code: output.status.code(),
        output: command_output_text(&output.stdout, &output.stderr),
    })
}

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
        let output = Command::new(r"C:\Windows\System32\pnputil.exe")
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
    fn macos_helper_status_parser_maps_known_states() {
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
}
