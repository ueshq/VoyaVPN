use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TunPlatform {
    Windows,
    Linux,
    Macos,
    Ios,
    Android,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TunBackend {
    Process,
    MacosPacketTunnel,
    WindowsService,
    IosPacketTunnel,
    AndroidVpnService,
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TunPreflight {
    pub platform: TunPlatform,
    pub state: TunPreflightState,
    pub notes: Vec<String>,
    pub route_restore_note: String,
    pub windows_cleanup_devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    pub provider_path_mismatch: bool,
    pub resolved_provider_path: Option<String>,
    pub expected_provider_path: Option<String>,
    pub restore_on_disconnect: bool,
    pub preflight: TunPreflight,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TunProviderDiagnostics {
    pub backend: TunBackend,
    pub container_path: Option<String>,
    pub status_path: Option<String>,
    pub log_path: Option<String>,
    pub packaging_mode: Option<String>,
    pub expected_provider_path: Option<String>,
    pub system_extension_state: Option<String>,
    pub registration_paths: Vec<String>,
    pub status_state: Option<String>,
    pub last_error: Option<String>,
    pub provider_bundle_path: Option<String>,
    pub breadcrumbs: Vec<String>,
    pub provider_log_tail: Vec<String>,
    pub host_log_tail: Vec<String>,
    pub message: Option<String>,
}
