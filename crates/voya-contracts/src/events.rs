use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{CoreType, ServerStatItem, TunBackend, TunProviderState};

/// One frontend query cache, named on the wire.
///
/// Replaces the free-string key list the shell used to build by hand. The
/// variants are generated into `apps/desktop/src/ipc/bindings.ts`, and
/// `apps/desktop/src/ipc/query-keys.ts` maps each one onto the `queryKey`
/// array its `useQuery` really uses, so a variant added here fails the frontend
/// typecheck until the map is extended. Only caches an emitter can actually
/// invalidate belong here; a query whose data no command mutates
/// (`process-candidates`, `profile-share-qr`) deliberately has no variant.
///
/// Internally tagged rather than a bare string so a future scope that has to
/// name one row (`{ kind: "profile", id }`) is an additive change instead of a
/// rewrite of the wire shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum InvalidationScope {
    /// `["profiles"]` — the profile list, every filter slice of it.
    Profiles,
    /// `["group-child-candidates"]` — the policy-group / chain child picker.
    GroupChildCandidates,
    /// `["subscriptions"]`
    Subscriptions,
    /// `["subscription-metadata"]`
    SubscriptionMetadata,
    /// `["routings"]`
    Routings,
    /// `["dns"]`
    Dns,
    /// `["app-settings"]` — the whole settings bundle projected from `AppConfig`.
    AppSettings,
    /// `["ui-preferences"]` — the appearance slice the shell reads on its own.
    UiPreferences,
    /// `["connection-mode"]` — TUN / system-proxy mode and its availability.
    ConnectionMode,
    /// `["proxy-groups"]`
    ProxyGroups,
    /// `["proxy-connections"]`
    ProxyConnections,
}

/// One invalidated cache plus the change that invalidated it.
///
/// `reason` stays a free string on purpose: it is diagnostic (it names the
/// command, and shows up in logs), never part of the contract the frontend
/// switches on.
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryInvalidation {
    pub scope: InvalidationScope,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogLineEvent {
    pub id: u32,
    pub level: LogLevel,
    pub line: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CoreState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CoreStateEvent {
    pub state: CoreState,
    pub active_profile_id: Option<String>,
    pub main_pid: Option<u32>,
    pub pre_pid: Option<u32>,
    pub running_core_type: Option<CoreType>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatisticsSnapshot {
    pub active_profile_id: Option<String>,
    pub proxy_upload_bytes_per_second: f64,
    pub proxy_download_bytes_per_second: f64,
    pub direct_upload_bytes_per_second: f64,
    pub direct_download_bytes_per_second: f64,
    pub upload_bytes_per_second: f64,
    pub download_bytes_per_second: f64,
    pub server_stat: Option<ServerStatItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SysProxyMode {
    Unchanged,
    ForcedChange,
    ForcedClear,
    Pac,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SysProxyChanged {
    pub requested_mode: SysProxyMode,
    pub effective_mode: SysProxyMode,
    pub pac_available: bool,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TunChanged {
    pub enabled: bool,
    pub backend: TunBackend,
    pub provider_state: TunProviderState,
    pub native_component_ready: bool,
    pub last_provider_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppNoticeLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppNotice {
    pub level: AppNoticeLevel,
    pub title: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ShellTabTarget {
    Profiles,
    ProxyGroups,
    ProxyConnections,
    Logs,
}
