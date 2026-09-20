//! The three event channels, declared a second time.
//!
//! The shell's copies in `apps/desktop/src-tauri/src/ipc/events.rs` carry
//! `tauri_specta::Event`, which is what makes them the source of the generated
//! TypeScript. This host cannot reuse them — it must not depend on the shell,
//! and `Event` is a foreign trait it could not implement for foreign types
//! either — so it declares the same payloads with the same serde attributes.
//!
//! What keeps the two from drifting is not discipline but
//! `packages/contracts/events.json`, generated from the very `bindings.ts` the
//! frontend's types come from, and the test at the bottom of this file.

use serde::Serialize;
use voya_contracts::{
    AppNotice, LogLineEvent, ProxyConnectionsSnapshot, ProxyMonitorStatus, QueryInvalidation,
    RuntimeStatusResponse, ShellTabTarget, SpeedtestResult, StatisticsSnapshot,
    SystemProxyStatusResponse, TunStatus,
};

/// The wire name of a channel, as the frontend subscribes to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventChannel {
    App,
    Invalidate,
    TransientStream,
}

impl EventChannel {
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::App => "app-event",
            Self::Invalidate => "invalidate-event",
            Self::TransientStream => "transient-stream-event",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidateEvent {
    pub keys: Vec<QueryInvalidation>,
}

/// Live state that is not a query cache.
///
/// The three status variants carry the very structs their commands return, so
/// the frontend stores an event payload and a command result interchangeably.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "camelCase")]
pub enum TransientStreamEvent {
    LogLines(Vec<LogLineEvent>),
    CoreState(RuntimeStatusResponse),
    Statistics(StatisticsSnapshot),
    SysProxyChanged(SystemProxyStatusResponse),
    TunChanged(TunStatus),
    ProxyMonitorStatus(ProxyMonitorStatus),
    ProxyConnections(ProxyConnectionsSnapshot),
    SpeedtestResults(Vec<SpeedtestResult>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "camelCase")]
pub enum AppEvent {
    Notice(AppNotice),
    SelectTab(ShellTabTarget),
    CloseRequested,
}

#[cfg(test)]
mod tests;
