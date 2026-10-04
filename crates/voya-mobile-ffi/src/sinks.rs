//! Where this host's events leave: every sink and command outcome, fanned
//! into the three channels.
//!
//! `apps/desktop/src-tauri/src/event_sinks.rs` is the same file for Tauri: the
//! sinks are `Arc<dyn Trait>` carrying codes rather than English strings, so
//! neither host translates anything. The only difference here is the
//! destination — a host callback taking a channel name and a JSON payload
//! instead of `AppHandle::emit`.

use std::sync::Arc;

use voya_app::{
    connection_mode::ConnectionModeSink, invalidation, proxy_runtime::ProxyRuntimeEventSink,
};
use voya_contracts::{
    AppNoticeLevel, LogCode, LogLevel, LogLineBody, LogLineEvent, NoticeCode,
    ProxyConnectionsSnapshot, TunStatus,
};

use voya_platform::sysproxy::SystemProxyStatus;

use crate::events::{AppEvent, EventChannel, TransientStreamEvent};

/// Where events go once this host has encoded them.
///
/// `channel` is the wire name the frontend subscribes to, and `payload_json` is
/// the same JSON the Tauri shell would have emitted. A listener never reports
/// failure: by the time it is called the change has already happened, and a
/// view that is tearing down must not turn that into a failed command.
#[uniffi::export(with_foreign)]
pub trait EventListener: Send + Sync {
    fn on_event(&self, channel: String, payload_json: String);
}

pub struct HostSinks {
    listener: Arc<dyn EventListener>,
}

impl HostSinks {
    #[must_use]
    pub fn new(listener: Arc<dyn EventListener>) -> Self {
        Self { listener }
    }

    pub(crate) fn emit<T: serde::Serialize>(&self, channel: EventChannel, payload: &T) {
        match serde_json::to_string(payload) {
            Ok(payload_json) => self
                .listener
                .on_event(channel.wire_name().to_string(), payload_json),
            Err(error) => {
                tracing::warn!(%error, channel = channel.wire_name(), "an event could not be encoded")
            }
        }
    }

    /// Announces a committed change, exactly as `voya_app::invalidation` says.
    pub(crate) fn invalidate(&self, reason: &str, bundle: invalidation::InvalidationBundle) {
        self.emit(
            EventChannel::Invalidate,
            &invalidation::invalidate_event(reason, bundle.1),
        );
    }

    /// A notice raised after a change was committed, and the log line a
    /// failure owes the Logs page — the same policy the desktop applies.
    pub(crate) fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str) {
        let effects = voya_app::post_commit::notice_effects(level, code, detail);
        if let Some(log_level) = effects.log_level {
            self.log(
                log_level,
                voya_app::post_commit::POST_COMMIT_LOG_CODE,
                Some(detail.to_string()),
            );
        }
        self.emit(EventChannel::App, &AppEvent::Notice(effects.notice));
    }

    /// An app-authored line. Queued through the shared batcher — like every
    /// other line — so the Logs page's batching and hold-back apply to it.
    pub(crate) fn log(&self, level: LogLevel, code: LogCode, detail: Option<String>) {
        crate::logging::queue_log_line(level, LogLineBody::App { code, detail });
    }

    /// A flushed batch, delivered by the logging pipeline's flusher.
    pub(crate) fn emit_log_lines(&self, batch: Vec<LogLineEvent>) {
        self.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::LogLines(batch),
        );
    }
}

/// What a mode switch and a settled connection both announce, so the core
/// flow's sink forwards its two matching callbacks here.
impl ConnectionModeSink for HostSinks {
    fn system_proxy_changed(&self, status: &SystemProxyStatus) {
        self.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::SysProxyChanged(
                voya_app::contract_map::system_proxy_status_to_contract(status.clone()),
            ),
        );
    }

    fn tun_changed(&self, status: &TunStatus) {
        self.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::TunChanged(status.clone()),
        );
    }

    fn tray_refresh(&self) {
        // No tray on a phone; the tab bar is rendered from the same stores the
        // events above already move.
    }
}

impl ProxyRuntimeEventSink for HostSinks {
    fn emit_connections(&self, event: ProxyConnectionsSnapshot) {
        self.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::ProxyConnections(event),
        );
    }
}
