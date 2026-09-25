use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    LogLineBody, NoticeCode, ProxyConnectionsSnapshot, ProxyMonitorStatus, RuntimeStatusResponse,
    ServerStatItem, SpeedtestResult, SystemProxyStatusResponse, TunStatus,
};

/// One frontend query cache, named on the wire.
///
/// Replaces the free-string key list the shell used to build by hand. The
/// variants are generated into `apps/desktop/src/ipc/bindings.ts`, and
/// `packages/client/src/query-keys.ts` maps each one onto the `queryKey`
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
    /// `["proxy-connections"]`
    ProxyConnections,
    /// `["profiles", "policy-groups"]` — stored groups and their resolved members.
    PolicyGroups,
    /// `["policy-group-runtime"]` — the running group's current member and delays.
    PolicyGroupRuntime,
    /// `["self-host"]` — the self-hosted node's settings, status, links and
    /// last network check.
    SelfHost,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// One line for the Logs panel.
///
/// `body` says who wrote it: the core process (raw passthrough) or the app
/// itself (a [`crate::LogCode`] the frontend translates). It used to be a
/// single `line: String`, which meant every sentence the app logged reached
/// the panel in English whatever the interface language was.
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogLineEvent {
    pub id: u32,
    /// When the app queued the line, in milliseconds since the Unix epoch.
    /// Lines are held back while no Logs panel is open, so the time they
    /// reach the webview says nothing about when they happened.
    pub logged_at_ms: f64,
    pub level: LogLevel,
    pub body: LogLineBody,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppNoticeLevel {
    Info,
    Warning,
    Error,
}

/// One toast.
///
/// `code` replaced the prose `title` the shell used to spell out at each of its
/// call sites: those titles were English literals that `pnpm check:i18n` could
/// not see, and the toast is the most visible text the backend produces.
/// `detail` stays an untranslated diagnostic — the error behind the notice.
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppNotice {
    pub level: AppNoticeLevel,
    pub code: NoticeCode,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ShellTabTarget {
    Profiles,
    ProxyConnections,
    Logs,
}

// The three event channels. Each host puts these payloads on the wire under
// its own channel names (`invalidate-event`, `transient-stream-event`,
// `app-event`); the desktop wraps them for tauri-specta, a phone serializes
// them as they are. Declaring them once here is what keeps the two hosts'
// payloads from drifting.

/// Query caches the frontend must drop.
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InvalidateEvent {
    pub keys: Vec<QueryInvalidation>,
}

/// Live state that is not a query cache.
///
/// The three status variants carry the very structs their commands return, so
/// the frontend stores an event payload and a command result interchangeably.
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(tag = "kind", content = "payload", rename_all = "camelCase")]
pub enum TransientStreamEvent {
    /// Logs-panel lines in the order they were written, delivered at most
    /// once per batch window instead of one event per line.
    LogLines(Vec<LogLineEvent>),
    CoreState(RuntimeStatusResponse),
    Statistics(StatisticsSnapshot),
    SysProxyChanged(SystemProxyStatusResponse),
    TunChanged(TunStatus),
    ProxyMonitorStatus(ProxyMonitorStatus),
    ProxyConnections(ProxyConnectionsSnapshot),
    /// Speedtest results in the order they were reached. A run's start and a
    /// cancel settle every selected node at once and arrive as one event; a
    /// measured node arrives on its own.
    SpeedtestResults(Vec<SpeedtestResult>),
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(tag = "kind", content = "payload", rename_all = "camelCase")]
pub enum AppEvent {
    Notice(AppNotice),
    SelectTab(ShellTabTarget),
    /// The main window was closed while the close action is "ask".
    CloseRequested,
}
