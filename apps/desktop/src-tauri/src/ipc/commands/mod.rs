use std::collections::BTreeSet;

use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;
use voya_app::autostart::{AutostartManager, AutostartManagerError};
use voya_app::certificates::{
    calculate_certificate_sha256 as calculate_certificate_sha256_impl,
    fetch_certificate as fetch_certificate_impl, CertificateError,
};
use voya_app::config_mutation::{ConfigMutationError, ConfigMutationGuard};
use voya_app::contract_map::{
    dns_from_contract, dns_to_contract, group_child_to_contract, group_preview_to_contract,
    import_profiles_to_contract, move_action_from_contract, profile_dedupe_to_contract,
    profile_from_contract, profile_list_to_contract, profile_sort_key_from_contract,
    routing_from_contract, routing_to_contract, rule_from_contract, subscription_from_contract,
    subscription_metadata_to_contract, subscription_to_contract, subscription_update_to_contract,
};
use voya_app::dns::DnsManagerError;
use voya_app::elevation::ElevationError;
use voya_app::exports::ExportManagerError;
use voya_app::groups::{GroupManager, GroupManagerError};
use voya_app::hotkeys::{
    HotkeyManager, HotkeyManagerError, HotkeyRegistrar, HotkeyStatus, ShowWindowShortcutBinding,
};
use voya_app::input_safety::{self, InputSafetyError};
use voya_app::presets::{PresetManager, PresetManagerError};
use voya_app::profiles::{ProfileManager, ProfileManagerError};
use voya_app::proxy_runtime::{ProxyRuntimeError, ProxyRuntimeManager};
use voya_app::qr::{QrCodeError, QrCodeManager};
use voya_app::routing::{RoutingManager, RoutingManagerError};
use voya_app::runtime::{RuntimeError, RuntimeManager};
use voya_app::services::{AppConfig, CoreType, SysProxyType, TrafficMode};
use voya_app::settings_save::{
    apply_settings_side_effects, compensate_settings_side_effects,
    saved_config_requires_runtime_restart, settings_runtime_action, validate_app_settings,
    SettingsRuntimeAction, SettingsSideEffectAdapter,
};
use voya_app::speedtest::{SpeedtestError, SpeedtestManager};
use voya_app::subscriptions::{SubscriptionManager, SubscriptionManagerError};
use voya_app::supervisor::{SupervisorConnectionState, SupervisorSnapshot};
use voya_app::sysproxy::{
    runtime_proxy_url as app_runtime_proxy_url,
    runtime_system_proxy_config as app_runtime_system_proxy_config, SystemProxyManagerError,
};
use voya_app::tun::{TunManager, TunManagerError};
use voya_app::updates::{UpdateManager, UpdateManagerError};
use voya_contracts::{
    AppError, AppNotice, AppNoticeLevel, AppSettingsV1, AppUpdaterState, AppUpdaterStatus,
    AppearanceSettings, CertificateFetchRequest, CertificateFetchResult,
    ConfigTemplateImportOptions, ConfigTemplateImportResult, ConfigTemplateSelection,
    CoreSeedInstallResult, CoreSeedInstallStatus, CoreType as ContractCoreType, DnsCommandError,
    DnsSettings as DnsSettingsContract, ExportProfilesFormat, ExportProfilesRequest,
    ExportProfilesResult, GroupChildCandidate as GroupChildContract,
    GroupPreview as GroupPreviewContract, ImportProfilesResult as ImportProfilesContract,
    MissingCoreError, MoveAction as ContractMoveAction, Profile as ProfileContract,
    ProfileDedupeResult as ProfileDedupeContract, ProfileListEntry,
    ProfileSortKey as ProfileSortContract, ProxyConnectionsSnapshot, ProxyDelayTestResult,
    ProxyGroupsSnapshot, ProxyMonitorStatus, QrCodeImage, QrScanResult, ResourceUpdateFile,
    Routing as RoutingContract, RoutingRule as RoutingRuleContract, RuntimeConnectionState,
    RuntimeStatusResponse, SpeedTestResult, SpeedtestRunResult, SpeedtestStatus,
    Subscription as SubscriptionContract, SubscriptionMetadata as SubscriptionMetadataContract,
    SubscriptionUpdateResult as SubscriptionUpdateContract, SysProxyType as ContractSysProxyType,
    SystemProxyStatusResponse, TunProviderDiagnostics, TunStatus,
};
use voya_platform::{
    coreinfo::{
        copy_seed_core_asset, discover_packaged_seed_executable, get_core_info, CoreInfoError,
        CoreSeedCopyOutcome, CoreSeedCopyStatus, TargetOs,
    },
    sysproxy::SystemProxyStatus,
};

use super::events::{
    next_log_line_id, AppEvent, CoreState, CoreStateEvent, InvalidateEvent, LogLevel, LogLineEvent,
    QueryInvalidation, TransientStreamEvent,
};
use crate::AppState;

const IPC_ID_MAX_CHARS: usize = 128;
const IPC_NAME_MAX_CHARS: usize = 256;
const IPC_FILTER_MAX_CHARS: usize = 256;
const IPC_PATH_MAX_CHARS: usize = 4096;
const IPC_PROXY_URL_MAX_CHARS: usize = 2048;
const IPC_QR_CONTENT_MAX_CHARS: usize = 4096;
const IPC_LIST_MAX_ITEMS: usize = 1024;
const MISSING_CORE_SEARCH_DIR_LABEL: &str = "application core directory";

mod app;
mod connection;
mod connection_mode;
mod core_flow;
mod dns;
mod groups;
mod lifecycle;
mod platform;
mod presets;
mod profiles;
mod proxy;
mod routing;
mod runtime;
mod speedtest;
mod subscriptions;
mod support;
mod sysproxy;
mod tun;
mod updates;

pub use app::*;
pub use connection::*;
pub use dns::*;
pub use groups::*;
pub use platform::*;
pub use presets::*;
pub use profiles::*;
pub use proxy::*;
pub use routing::*;
pub use runtime::*;
pub use speedtest::*;
pub use subscriptions::*;
pub use sysproxy::*;
pub use tun::*;
pub use updates::*;

pub(crate) use app::register_show_window_shortcut_for_config;
pub(crate) use core_flow::core_flow;
pub(crate) use support::emit_runtime_log;
