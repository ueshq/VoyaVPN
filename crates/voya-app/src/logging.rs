//! The pieces of the logging pipeline both hosts share.
//!
//! Each host installs its own `tracing` subscriber — the desktop adds a file
//! layer, a phone has none — but what reaches the Logs panel and how is the
//! same on both: the filter directives, the severity gate, the event-to-line
//! formatting, the layer that forwards a record, and the line it becomes.

use std::{
    fmt::{self, Write as _},
    str::FromStr as _,
    sync::atomic::{AtomicU32, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use tracing::{
    field::{Field, Visit},
    Level, Subscriber,
};
use tracing_subscriber::{filter::Targets, layer::Context, registry::LookupSpan, Layer};
use voya_contracts::{LogLevel, LogLineBody, LogLineEvent};

use crate::redaction::redact_url_userinfo;

/// Environment variable that overrides [`log_filter_directives`].
pub const LOG_FILTER_ENV_VAR: &str = "VOYAVPN_LOG";

/// Number of daily log files retained on disk.
pub const LOG_FILE_RETENTION_DAYS: usize = 7;

/// Filter directives used when [`LOG_FILTER_ENV_VAR`] is unset.
///
/// Third-party crates are limited to `warn` so the file log stays readable,
/// while the workspace crates report at `info`. Raw core output is off by
/// default: those lines already reach the UI through the redacting
/// `ProcessLogSink`, so persisting them again would both duplicate the Logs
/// panel and write credential-bearing core output to disk.
pub const DEFAULT_LOG_FILTER: &str = concat!(
    "warn,",
    "voya_app=info,",
    "voya_core=info,",
    "voya_db=info,",
    "voya_net=info,",
    "voya_platform=info,",
    "voyavpn=info,",
    "voyavpn_lib=info,",
    "voya::core_output=off",
);

/// Resolve the filter directives for the process.
///
/// An explicit, non-blank override always wins so a user chasing a bug can ask
/// for `debug` (or re-enable `voya_platform::process::CORE_OUTPUT_TARGET`)
/// without a rebuild.
#[must_use]
pub(crate) fn log_filter_directives(override_value: Option<&str>) -> String {
    match override_value.map(str::trim) {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => DEFAULT_LOG_FILTER.to_string(),
    }
}

/// The filter this process runs with: [`LOG_FILTER_ENV_VAR`], then `RUST_LOG`,
/// then the default.
///
/// Directives that do not parse fall back to the default and are handed back
/// for the host to report. Installing no subscriber at all for them — which is
/// what both hosts did — turned a typo in the variable into an app with no
/// file log and an empty Logs panel.
#[must_use]
pub fn env_log_filter() -> (Targets, Option<String>) {
    let directives = log_filter_directives(
        std::env::var(LOG_FILTER_ENV_VAR)
            .ok()
            .or_else(|| std::env::var("RUST_LOG").ok())
            .as_deref(),
    );
    parse_log_filter(directives)
}

fn parse_log_filter(directives: String) -> (Targets, Option<String>) {
    match Targets::from_str(&directives) {
        Ok(filter) => (filter, None),
        Err(_) => (
            Targets::from_str(DEFAULT_LOG_FILTER).unwrap_or_default(),
            Some(directives),
        ),
    }
}

/// One Logs-panel line, stamped now.
///
/// The id is unique within the process, whoever produced the line, so the
/// frontend can key rows on it. The time is when the line was queued, in whole
/// milliseconds: lines are held back while no Logs panel is open, so the
/// moment one reaches the view says nothing about when it happened.
#[must_use]
pub fn new_log_line(level: LogLevel, body: LogLineBody) -> LogLineEvent {
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);

    LogLineEvent {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        level,
        logged_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_millis() as f64)
            .unwrap_or_default(),
        body,
    }
}

/// Forwards `warn`/`error` records to the Logs panel through `queue`.
///
/// `queue` must only queue, never emit: an emit failure traced from this layer
/// would re-enter it.
pub struct LogPanelLayer<Q> {
    queue: Q,
}

impl<Q> LogPanelLayer<Q>
where
    Q: Fn(LogLevel, LogLineBody) + Send + Sync + 'static,
{
    pub const fn new(queue: Q) -> Self {
        Self { queue }
    }
}

impl<S, Q> Layer<S> for LogPanelLayer<Q>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    Q: Fn(LogLevel, LogLineBody) + Send + Sync + 'static,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let Some(level) = ui_log_level(metadata.level()) else {
            return;
        };

        let mut visitor = TracingLineVisitor::new();
        event.record(&mut visitor);
        (self.queue)(
            level,
            // A `tracing` event: developer diagnostics with a module target,
            // not an app-authored sentence, so it stays raw like core output.
            LogLineBody::Diagnostic {
                line: visitor.into_line(metadata.target()),
            },
        );
    }
}

/// Severity a `tracing` event should carry into the Logs panel.
///
/// Only `warn` and `error` are forwarded: the panel is a user-facing surface
/// and the backend's routine `info`/`debug` chatter belongs in the file log.
fn ui_log_level(level: &Level) -> Option<LogLevel> {
    if *level == Level::ERROR {
        Some(LogLevel::Error)
    } else if *level == Level::WARN {
        Some(LogLevel::Warn)
    } else {
        None
    }
}

/// Flattens a `tracing` event's message and structured fields into one line.
///
/// `tracing` keeps the message and the `key = value` pairs as separate fields;
/// the Logs panel takes a single string, so they are joined here in a stable
/// order (message first, then the remaining fields as recorded).
#[derive(Debug, Default)]
struct TracingLineVisitor {
    message: String,
    fields: String,
}

impl TracingLineVisitor {
    fn new() -> Self {
        Self::default()
    }

    /// Render the visited event as `[target] message field=value`.
    ///
    /// URL userinfo is stripped because backend errors routinely embed the
    /// outbound URI that failed, and those carry passwords and UUIDs.
    fn into_line(self, target: &str) -> String {
        let mut line = String::with_capacity(
            target.len() + self.message.len() + self.fields.len() + LINE_PADDING,
        );
        if !target.is_empty() {
            let _ = write!(line, "[{target}] ");
        }
        line.push_str(&self.message);
        if !self.fields.is_empty() {
            if !self.message.is_empty() {
                line.push(' ');
            }
            line.push_str(&self.fields);
        }

        redact_url_userinfo(line.trim_end()).into_owned()
    }

    fn push_field(&mut self, name: &str, value: &str) {
        if name == MESSAGE_FIELD {
            self.message = value.to_string();
            return;
        }
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        let _ = write!(self.fields, "{name}={value}");
    }
}

impl Visit for TracingLineVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.push_field(field.name(), &format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.push_field(field.name(), value);
    }
}

const MESSAGE_FIELD: &str = "message";
const LINE_PADDING: usize = 8;

#[cfg(test)]
mod tests {
    use voya_platform::process::CORE_OUTPUT_TARGET;

    use super::*;

    /// The layer both hosts install: a `warn` reaches the queue as one
    /// redacted diagnostic line, and routine `info` chatter does not.
    #[test]
    fn the_panel_layer_forwards_warnings_redacted_and_drops_info() {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::layer::SubscriberExt as _;

        let queued: Arc<Mutex<Vec<(LogLevel, LogLineBody)>>> = Arc::default();
        let subscriber = tracing_subscriber::registry().with(LogPanelLayer::new({
            let queued = Arc::clone(&queued);
            move |level, body| queued.lock().expect("queue lock").push((level, body))
        }));
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!("dial failed for https://alice:secret@example.test/sub");
            tracing::info!("routine chatter must not reach the panel");
        });

        let queued = queued.lock().expect("queue lock");
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, LogLevel::Warn);
        assert!(matches!(
            &queued[0].1,
            LogLineBody::Diagnostic { line }
                if line.ends_with("dial failed for https://<redacted>@example.test/sub")
        ));
    }

    #[test]
    fn a_log_line_gets_the_next_id_and_a_whole_millisecond_stamp() {
        let body = || LogLineBody::Core {
            line: "started".to_string(),
        };
        let first = new_log_line(LogLevel::Info, body());
        let second = new_log_line(LogLevel::Info, body());

        assert!(second.id > first.id);
        assert!(first.logged_at_ms > 0.0);
        assert!(first.logged_at_ms.fract().abs() < f64::EPSILON);
    }

    /// The fallback itself has to parse, or a bad variable would install a
    /// filter that lets nothing through.
    #[test]
    fn unparsable_directives_fall_back_to_the_default_filter() {
        let (filter, rejected) = parse_log_filter("info,voya=notalevel".to_string());

        assert_eq!(rejected.as_deref(), Some("info,voya=notalevel"));
        assert_eq!(
            filter.to_string(),
            Targets::from_str(DEFAULT_LOG_FILTER)
                .expect("the default filter parses")
                .to_string()
        );
        assert_eq!(parse_log_filter("warn".to_string()).1, None);
    }

    #[test]
    fn default_filter_silences_raw_core_output() {
        assert!(DEFAULT_LOG_FILTER.contains(&format!("{CORE_OUTPUT_TARGET}=off")));
        assert_eq!(log_filter_directives(None), DEFAULT_LOG_FILTER);
        assert_eq!(log_filter_directives(Some("   ")), DEFAULT_LOG_FILTER);
    }

    #[test]
    fn explicit_filter_override_wins() {
        assert_eq!(
            log_filter_directives(Some(" debug,voya::core_output=trace ")),
            "debug,voya::core_output=trace"
        );
    }

    #[test]
    fn only_warn_and_error_reach_the_log_panel() {
        assert!(matches!(ui_log_level(&Level::ERROR), Some(LogLevel::Error)));
        assert!(matches!(ui_log_level(&Level::WARN), Some(LogLevel::Warn)));
        assert!(ui_log_level(&Level::INFO).is_none());
        assert!(ui_log_level(&Level::DEBUG).is_none());
        assert!(ui_log_level(&Level::TRACE).is_none());
    }

    #[test]
    fn visitor_joins_message_and_fields() {
        let mut visitor = TracingLineVisitor::new();
        record_message(&mut visitor, "failed to revoke TUN elevation on exit");
        visitor.push_field("error", "PermissionDenied");
        visitor.push_field("attempt", "2");

        assert_eq!(
            visitor.into_line("voya_app::elevation"),
            concat!(
                "[voya_app::elevation] failed to revoke TUN elevation on exit ",
                "error=PermissionDenied attempt=2"
            )
        );
    }

    #[test]
    fn visitor_renders_field_only_events() {
        let mut visitor = TracingLineVisitor::new();
        visitor.push_field("error", "Timeout");

        assert_eq!(visitor.into_line("voya_net"), "[voya_net] error=Timeout");
    }

    #[test]
    fn visitor_redacts_url_userinfo() {
        let mut visitor = TracingLineVisitor::new();
        record_message(
            &mut visitor,
            "dial failed for https://alice:secret@example.test/sub",
        );

        let line = visitor.into_line("voya_net::download");
        assert!(line.contains("https://<redacted>@example.test/sub"));
        assert!(!line.contains("alice:secret"));
    }

    #[test]
    fn visitor_keeps_the_target_out_when_empty() {
        let mut visitor = TracingLineVisitor::new();
        record_message(&mut visitor, "plain");
        assert_eq!(visitor.into_line(""), "plain");
    }

    /// Records a `message` field the way `tracing::warn!("...")` does, without
    /// needing a live subscriber.
    fn record_message(visitor: &mut TracingLineVisitor, message: &str) {
        visitor.push_field(MESSAGE_FIELD, message);
    }
}
