use std::collections::BTreeSet;

use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;
use voya_app::autostart::AutostartManager;
use voya_app::certificates::{
    calculate_certificate_sha256 as calculate_certificate_sha256_impl,
    fetch_certificate as fetch_certificate_impl,
};
use voya_app::config_mutation::{CommittedMutation, UnitOfWork};
use voya_app::contract_map::{
    certificate_error, core_info_error, core_seed_install_result, dns_from_contract,
    dns_to_contract, group_child_to_contract, group_preview_to_contract,
    import_profiles_to_contract, input_text_error, move_action_from_contract,
    profile_from_contract, profile_list_to_contract, profile_listing_to_contract,
    routing_from_contract, routing_to_contract, rule_from_contract, runtime_status_event,
    runtime_status_response, subscription_from_contract, subscription_metadata_to_contract,
    subscription_to_contract, subscription_update_to_contract, traffic_mode_from_contract,
    traffic_mode_to_contract,
};
use voya_app::dns::DnsManager;
use voya_app::groups::GroupManager;
use voya_app::input_safety;
use voya_app::invalidation;
use voya_app::profiles::ProfileManager;
use voya_app::qr::QrCodeManager;
use voya_app::routing::RoutingManager;
use voya_app::runtime::RuntimeManager;
use voya_app::services::AppConfig;
use voya_app::settings_flow::SettingsSaveOutcome;
use voya_app::settings_save::{SettingsRuntimeAction, SettingsSideEffectAdapter};
use voya_app::speedtest::SpeedtestManager;
use voya_app::subscriptions::SubscriptionManager;
use voya_app::supervisor::{SupervisorConnectionState, SupervisorSnapshot};
use voya_app::sysproxy::runtime_proxy_url as app_runtime_proxy_url;
use voya_app::tun::TunManager;
use voya_app::updates::UpdateManager;
use voya_contracts::{
    AppError, AppErrorSubsystem, AppNotice, AppNoticeLevel, AppSettingsV1, AppUpdaterState,
    AppUpdaterStatus, AppearanceSettings, CertificateFetchRequest, CertificateFetchResult,
    CoreFlowReason, CoreSeedInstallResult, CoreSeedInstallStatus, CoreType as ContractCoreType,
    DnsSettings as DnsSettingsContract, ExportProfilesFormat, ExportProfilesRequest,
    ExportProfilesResult, GroupChildCandidate as GroupChildContract,
    GroupPreview as GroupPreviewContract, ImportProfilesResult as ImportProfilesContract,
    InvalidationScope, LogCode, MoveAction as ContractMoveAction, NoticeCode,
    Profile as ProfileContract, ProfileListEntry, ProfileListing, ProxyConnectionsSnapshot,
    ProxyDelayTestResult, ProxyGroupsSnapshot, ProxyMonitorStatus, QrCodeImage, QrScanResult,
    ResourceUpdateFile, Routing as RoutingContract, RoutingRule as RoutingRuleContract,
    RuntimeStatusResponse, SpeedtestResult, SpeedtestRunResult, SpeedtestStatus,
    Subscription as SubscriptionContract, SubscriptionMetadata as SubscriptionMetadataContract,
    SubscriptionUpdateResult as SubscriptionUpdateContract, SystemProxyStatusResponse,
    SystemProxyType, TunProviderDiagnostics, TunStatus,
};
use voya_platform::{
    coreinfo::{
        copy_seed_core_asset, discover_packaged_seed_executable, get_core_info, CoreInfoError,
        TargetOs,
    },
    sysproxy::SystemProxyStatus,
};

use super::events::{
    next_log_line_id, AppEvent, CoreState, InvalidateEvent, LogLevel, LogLineBody, LogLineEvent,
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

mod app;
mod connection;
mod connection_mode;
mod core_flow;
mod dns;
mod groups;
mod lifecycle;
mod platform;
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
pub use profiles::*;
pub use proxy::*;
pub use routing::*;
pub use runtime::*;
pub use speedtest::*;
pub use subscriptions::*;
pub use sysproxy::*;
pub use tun::*;
pub use updates::*;

pub(crate) use core_flow::core_flow;
pub(crate) use lifecycle::emit_subscription_invalidation;
pub(crate) use support::{emit_app_log, emit_core_log};
