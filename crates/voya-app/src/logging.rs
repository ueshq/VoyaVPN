//! Shared pieces of the desktop logging pipeline.
//!
//! The `tracing` subscriber itself is Tauri-specific and lives in the shell,
//! but everything the shell needs that can be tested — severity mapping, the
//! event-to-line formatting used for the Logs panel and the default filter
//! directives — lives here.

use std::fmt::{self, Write as _};

use tracing::{
    field::{Field, Visit},
    Level,
};
use voya_contracts::LogLevel;

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
pub fn log_filter_directives(override_value: Option<&str>) -> String {
    match override_value.map(str::trim) {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => DEFAULT_LOG_FILTER.to_string(),
    }
}

/// Severity a `tracing` event should carry into the desktop Logs panel.
///
/// Only `warn` and `error` are forwarded: the panel is a user-facing surface
/// and the backend's routine `info`/`debug` chatter belongs in the file log.
#[must_use]
pub fn ui_log_level(level: &Level) -> Option<LogLevel> {
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
pub struct TracingLineVisitor {
    message: String,
    fields: String,
}

impl TracingLineVisitor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the visited event as `[target] message field=value`.
    ///
    /// URL userinfo is stripped because backend errors routinely embed the
    /// outbound URI that failed, and those carry passwords and UUIDs.
    #[must_use]
    pub fn into_line(self, target: &str) -> String {
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

        redact_url_userinfo(line.trim_end())
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
