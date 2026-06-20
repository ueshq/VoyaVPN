use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;
use voya_app::{
    clash::{ClashConnectionsSnapshot, ClashMonitorStatus, ClashTrafficEvent},
    speedtest::SpeedTestResult,
};
use voya_core::{CoreType, ServerStatItem};

static NEXT_LOG_LINE_ID: AtomicU32 = AtomicU32::new(1);

pub fn next_log_line_id() -> u32 {
    NEXT_LOG_LINE_ID.fetch_add(1, Ordering::Relaxed)
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DemoRequest {
    pub message: String,
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DemoResponse {
    pub echoed_message: String,
    pub message_length: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueryInvalidation {
    pub query_key: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct InvalidateEvent {
    pub keys: Vec<QueryInvalidation>,
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct CoreStateEvent {
    pub state: CoreState,
    pub active_profile_id: Option<String>,
    pub main_pid: Option<u32>,
    pub pre_pid: Option<u32>,
    pub running_core_type: Option<CoreType>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct SysProxyChanged {
    pub requested_mode: SysProxyMode,
    pub effective_mode: SysProxyMode,
    pub pac_available: bool,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TunChanged {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type, Event)]
#[serde(tag = "kind", content = "payload", rename_all = "camelCase")]
pub enum TransientStreamEvent {
    LogLine(LogLineEvent),
    CoreState(CoreStateEvent),
    Statistics(StatisticsSnapshot),
    SysProxyChanged(SysProxyChanged),
    TunChanged(TunChanged),
    ClashMonitorStatus(ClashMonitorStatus),
    ClashTraffic(ClashTrafficEvent),
    ClashConnections(ClashConnectionsSnapshot),
    SpeedtestResult(SpeedTestResult),
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppNoticeLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppNotice {
    pub level: AppNoticeLevel,
    pub title: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ShellTabTarget {
    Profiles,
    ClashProxies,
    ClashConnections,
    Logs,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type, Event)]
#[serde(tag = "kind", content = "payload", rename_all = "camelCase")]
pub enum AppEvent {
    Notice(AppNotice),
    SelectTab(ShellTabTarget),
}
