//! The eight `voya-app` sinks, fanned into the three channels.
//!
//! `apps/desktop/src-tauri/src/event_sinks.rs` is the same file for Tauri: the
//! sinks are `Arc<dyn Trait>` carrying codes rather than English strings, so
//! neither host translates anything. The only difference here is the
//! destination — a host callback taking a channel name and a JSON payload
//! instead of `AppHandle::emit`.

use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

use voya_app::{
    contract_map::statistics_snapshot_to_contract,
    invalidation,
    logging::process_log_level_to_contract,
    proxy_runtime::{ProxyConnectionsSnapshot, ProxyRuntimeEventSink},
    redaction::{redact_url_userinfo, redact_urls},
    statistics::{StatisticsEventSink, StatisticsSnapshot as AppStatisticsSnapshot},
    subscriptions::{AutoUpdateOutcome, SubscriptionAutoUpdateSink},
    supervisor::{CoreExitEvent, NativeTunExitEvent, SupervisorEventSink},
};
use voya_contracts::{
    AppNotice, AppNoticeLevel, InvalidationScope, LogCode, LogLevel, LogLineBody, LogLineEvent,
    NoticeCode, QueryInvalidation,
};
use voya_platform::process::{ProcessLogLevel, ProcessLogSink, ProcessOutputStream, ProcessRole};

use crate::events::{AppEvent, EventChannel, InvalidateEvent, TransientStreamEvent};

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
    /// Whether a Logs screen is open. Core output is the one stream that costs
    /// something while nobody is reading it, so it is gated at the source.
    log_streaming: AtomicBool,
}

impl HostSinks {
    #[must_use]
    pub fn new(listener: Arc<dyn EventListener>) -> Self {
        Self {
            listener,
            log_streaming: AtomicBool::new(false),
        }
    }

    pub(crate) fn set_log_streaming(&self, enabled: bool) {
        self.log_streaming.store(enabled, Ordering::Relaxed);
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
    pub(crate) fn invalidate(&self, reason: &str, scopes: Vec<InvalidationScope>) {
        self.emit(
            EventChannel::Invalidate,
            &InvalidateEvent {
                keys: scopes
                    .into_iter()
                    .map(|scope| QueryInvalidation {
                        scope,
                        reason: reason.to_string(),
                    })
                    .collect(),
            },
        );
    }

    pub(crate) fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: Option<String>) {
        self.emit(
            EventChannel::App,
            &AppEvent::Notice(AppNotice {
                code,
                detail,
                level,
            }),
        );
    }

    pub(crate) fn log(&self, level: LogLevel, code: LogCode, detail: Option<String>) {
        self.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::LogLines(vec![LogLineEvent {
                id: next_log_line_id(),
                level,
                logged_at_ms: now_ms(),
                body: LogLineBody::App { code, detail },
            }]),
        );
    }
}

impl StatisticsEventSink for HostSinks {
    fn emit_statistics(&self, snapshot: AppStatisticsSnapshot) {
        self.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::Statistics(statistics_snapshot_to_contract(snapshot)),
        );
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

impl SupervisorEventSink for HostSinks {
    fn native_tun_exited(&self, event: NativeTunExitEvent) {
        // The provider went down without the app asking. The user has to be
        // told even with the app in the background, which is what the notice
        // channel is for.
        self.notice(
            AppNoticeLevel::Warning,
            NoticeCode::NativeTunStopped,
            Some(event.message),
        );
    }

    fn core_exited(&self, event: CoreExitEvent) {
        self.notice(
            AppNoticeLevel::Warning,
            NoticeCode::CoreStopped,
            event.exit_code.map(|code| format!("exit code {code}")),
        );
    }
}

impl SubscriptionAutoUpdateSink for HostSinks {
    fn update_completed(&self, outcome: AutoUpdateOutcome) {
        if let Some(error) = &outcome.error {
            // Redacted at the source too; repeated here so a future failure
            // path cannot put a tokenized subscription URL in a toast.
            let error = redact_urls(error);
            self.log(
                LogLevel::Warn,
                LogCode::SubscriptionAutoUpdateFailed {
                    remarks: outcome.remarks.clone(),
                },
                Some(error.clone()),
            );
            // Only the first failure of a streak surfaces as a user notice;
            // retries stay in the log until the subscription recovers.
            if outcome.consecutive_failures == 1 {
                self.notice(
                    AppNoticeLevel::Warning,
                    NoticeCode::SubscriptionAutoUpdateFailed {
                        remarks: outcome.remarks.clone(),
                    },
                    Some(error),
                );
            }
            return;
        }

        let imported = outcome.result.as_ref().map_or(0, |result| result.imported);
        self.log(
            LogLevel::Info,
            LogCode::SubscriptionAutoUpdateFinished {
                remarks: outcome.remarks.clone(),
                imported,
            },
            None,
        );
        self.invalidate(
            "subscription-auto-updated",
            invalidation::subscription_scopes(true, outcome.config_changed),
        );
    }
}

impl ProcessLogSink for HostSinks {
    fn line(
        &self,
        role: ProcessRole,
        _stream: ProcessOutputStream,
        level: ProcessLogLevel,
        line: String,
    ) {
        // Nothing spawns a child process here (see `app::NoProcessRunner`), so
        // this only ever carries the probe core the disconnected speedtest
        // starts in-process. The desktop drops probe lines for the same reason:
        // a latency run over a few hundred nodes would drown the log.
        if role == ProcessRole::Probe || !self.log_streaming.load(Ordering::Relaxed) {
            return;
        }
        self.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::LogLines(vec![LogLineEvent {
                id: next_log_line_id(),
                level: process_log_level_to_contract(level),
                logged_at_ms: now_ms(),
                body: LogLineBody::Core {
                    line: redact_url_userinfo(&line),
                },
            }]),
        );
    }
}

fn next_log_line_id() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// When the app queued the line. Lines are held back while no Logs screen is
/// open, so the time one reaches the view says nothing about when it happened.
fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis() as f64)
        .unwrap_or_default()
}
