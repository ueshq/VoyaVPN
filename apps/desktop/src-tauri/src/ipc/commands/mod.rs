mod screen_qr;
use std::collections::BTreeSet;

use crate::ipc::events::Emit;
use voya_app::autostart::AutostartManager;
use voya_app::config_mutation::{AppConfig, CommittedMutation};
use voya_app::contract_map::{
    core_info_error, core_seed_install_result, input_text_error, profile_details_to_contract,
    profile_from_contract, profile_summary_listing_to_contract, runtime_status_event,
    runtime_status_response, simple_dns_from_contract, simple_dns_to_contract,
    subscription_from_contract, subscription_metadata_to_contract, subscription_to_contract,
    system_proxy_status_to_contract, traffic_mode_from_contract, traffic_mode_to_contract,
};
use voya_app::input_safety;
use voya_app::invalidation;
use voya_app::profiles::ProfileManager;
use voya_app::runtime::RuntimeManager;
use voya_app::subscriptions::SubscriptionManager;
use voya_app::supervisor::{SupervisorConnectionState, SupervisorSnapshot};
use voya_app::sysproxy::runtime_proxy_url as app_runtime_proxy_url;
use voya_app::tun::TunManager;
use voya_contracts::{
    AppError, AppErrorSubsystem, AppNotice, AppNoticeLevel, AppSettings, AppUpdaterState,
    AppUpdaterStatus, AppearanceSettings, CoreSeedInstallResult, CoreSeedInstallStatus,
    DnsSettings as DnsSettingsContract, ExportProfilesResult,
    ImportProfilesResult as ImportProfilesContract, InvalidationScope, LogCode,
    MoveAction as ContractMoveAction, NoticeCode, Profile as ProfileContract, ProfileDetails,
    ProfileSummaryListing, ProxyConnectionsSnapshot, ProxyMonitorStatus, QrCodeImage, QrScanResult,
    ResourceUpdateFile, Routing as RoutingContract, RoutingRule as RoutingRuleContract,
    RuntimeStatusResponse, SpeedtestRunResult, SpeedtestStatus,
    Subscription as SubscriptionContract, SubscriptionMetadata as SubscriptionMetadataContract,
    SubscriptionUpdateResult as SubscriptionUpdateContract, SystemProxyStatusResponse,
    TunProviderDiagnostics, TunStatus,
};
use voya_platform::{
    coreinfo::{copy_seed_core_asset, discover_packaged_seed_executable, TargetOs},
    sysproxy::SystemProxyStatus,
};

use super::events::next_log_line_id;
use crate::AppState;
use voya_contracts::{
    AppEvent, CoreState, InvalidateEvent, LogLevel, LogLineBody, LogLineEvent, QueryInvalidation,
    TransientStreamEvent,
};

const IPC_ID_MAX_CHARS: usize = 128;
const IPC_PROXY_URL_MAX_CHARS: usize = 2048;
const IPC_QR_CONTENT_MAX_CHARS: usize = 4096;
/// The base64 of one grey pixel per byte at the largest picture voya-app will
/// decode. `decode_image` rejects a payload that disagrees with the width and
/// height it was given, but only after Tauri has already materialized the
/// whole string, so the ceiling is enforced here as well as there.
const IPC_QR_IMAGE_MAX_BASE64_CHARS: usize = (voya_app::qr::QR_IMAGE_MAX_SIDE as usize)
    .pow(2)
    .div_ceil(3)
    * 4;
const IPC_LIST_MAX_ITEMS: usize = 1024;

mod app;
mod clipboard;
mod connection;
mod connection_mode;
mod dns;
mod logs;
mod platform;
mod policy_groups;
mod post_commit;
mod profiles;
mod proxy;
mod routing;
mod runtime;
mod self_host;
mod speedtest;
mod subscriptions;
mod support;
mod sysproxy;
mod tray_actions;
mod tun;
mod updates;

pub use app::*;
pub use clipboard::*;
pub use connection::*;
pub use dns::*;
pub use logs::*;
pub use platform::*;
pub use policy_groups::*;
pub use profiles::*;
pub use proxy::*;
pub use routing::*;
pub use runtime::*;
pub use self_host::*;
pub use speedtest::*;
pub use subscriptions::*;
pub use sysproxy::*;
pub use tray_actions::*;
pub use tun::*;
pub use updates::*;

pub(crate) use post_commit::{disconnect_removed_profile, emit_invalidation};
pub(crate) use runtime::core_flow;
pub(crate) use support::{emit_app_log, emit_core_log, emit_or_warn, queue_log_line};
