//! Process-wide `tracing` subscriber for the desktop shell.
//!
//! The backend states its error-handling contract as "best effort, failures are
//! logged": privileged cleanup, settings compensation and supervisor recovery
//! all fall back to `tracing::warn!`/`error!`. Without a subscriber installed
//! every one of those records is discarded, so those failures leave no trace at
//! all. This module installs the one subscriber the app has:
//!
//! * a rolling file layer under `AppPaths::log_dir()` (`guiLogs`), retained for
//!   [`LOG_FILE_RETENTION_DAYS`] days, with URL userinfo stripped on the way
//!   out, and
//! * a layer that forwards `warn`/`error` records to the `LogLines` transient
//!   stream so they also show up in the Logs panel.
//!
//! Raw core stdout/stderr is deliberately excluded (see
//! [`voya_app::logging::DEFAULT_LOG_FILTER`]): those lines already reach the UI
//! through the redacting `ProcessLogSink`, and persisting them again would both
//! duplicate the panel and write credential-bearing core output to disk.

use std::{
    io::{self, Write as _},
    path::Path,
    str::FromStr as _,
};

use tracing::Subscriber;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    filter::Targets,
    fmt::{self, writer::MakeWriter},
    layer::{Context, SubscriberExt as _},
    registry::LookupSpan,
    util::SubscriberInitExt as _,
    Layer,
};
use voya_app::{
    logging::{
        log_filter_directives, ui_log_level, TracingLineVisitor, LOG_FILE_RETENTION_DAYS,
        LOG_FILTER_ENV_VAR,
    },
    redaction::redact_url_userinfo,
};

use crate::ipc::{commands::queue_log_line, events::LogLineBody};

const LOG_FILE_PREFIX: &str = "voyavpn";
const LOG_FILE_SUFFIX: &str = "log";

/// Install the process-wide subscriber.
///
/// Called at the very top of `setup()` — before the first `tracing::warn!` of
/// the startup sequence — so early failures such as the dirty system-proxy
/// marker recovery are captured. Failures here are reported on stderr only: a
/// missing log file must never keep the app from starting, and `tracing` itself
/// is not available to report its own absence. `eprintln!` is avoided because a
/// Windows GUI build has no stderr handle and would panic on it.
pub(crate) fn install<R>(app: tauri::AppHandle<R>, log_dir: &Path)
where
    R: tauri::Runtime,
{
    let directives = log_filter_directives(
        std::env::var(LOG_FILTER_ENV_VAR)
            .ok()
            .or_else(|| std::env::var("RUST_LOG").ok())
            .as_deref(),
    );
    let Ok(filter) = Targets::from_str(&directives) else {
        report(&format!(
            "ignoring invalid {LOG_FILTER_ENV_VAR} value {directives:?}"
        ));
        return;
    };

    let file_layer = match build_file_appender(log_dir) {
        Ok(appender) => Some(
            fmt::layer()
                .with_ansi(false)
                .with_target(true)
                .with_writer(RedactingWriter(appender)),
        ),
        Err(error) => {
            report(&format!(
                "file logging disabled, could not open {}: {error}",
                log_dir.display()
            ));
            None
        }
    };

    if let Err(error) = tracing_subscriber::registry()
        .with(file_layer)
        .with(LogPanelLayer { app })
        .with(filter)
        .try_init()
    {
        report(&format!(
            "tracing subscriber was already installed: {error}"
        ));
    }
}

/// Best-effort diagnostic for failures that happen before logging exists.
fn report(message: &str) {
    let _ = writeln!(io::stderr(), "VoyaVPN: {message}");
}

fn build_file_appender(
    log_dir: &Path,
) -> Result<RollingFileAppender, tracing_appender::rolling::InitError> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix(LOG_FILE_SUFFIX)
        .max_log_files(LOG_FILE_RETENTION_DAYS)
        .build(log_dir)
}

/// Forwards `warn`/`error` records to the desktop Logs panel.
struct LogPanelLayer<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<S, R> Layer<S> for LogPanelLayer<R>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    R: tauri::Runtime,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let Some(level) = ui_log_level(metadata.level()) else {
            return;
        };

        let mut visitor = TracingLineVisitor::new();
        event.record(&mut visitor);

        // Queued, never emitted from here: an emit failure traced from this
        // layer would re-enter it. The file layer still records the event.
        queue_log_line(
            &self.app,
            level,
            // A `tracing` event: developer diagnostics with a module target,
            // not an app-authored sentence, so it stays raw like core output.
            LogLineBody::Diagnostic {
                line: visitor.into_line(metadata.target()),
            },
        );
    }
}

/// Strips URL userinfo from formatted events before they reach disk.
///
/// The `fmt` layer writes one complete event per `write` call, so redacting per
/// call is equivalent to redacting per line. Backend errors routinely embed the
/// outbound URI that failed, and those carry passwords and UUIDs.
#[derive(Debug, Clone)]
struct RedactingWriter<W>(W);

impl<'writer, W> MakeWriter<'writer> for RedactingWriter<W>
where
    W: MakeWriter<'writer>,
{
    type Writer = RedactingLineWriter<W::Writer>;

    fn make_writer(&'writer self) -> Self::Writer {
        RedactingLineWriter(self.0.make_writer())
    }

    fn make_writer_for(&'writer self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        RedactingLineWriter(self.0.make_writer_for(meta))
    }
}

struct RedactingLineWriter<W>(W);

impl<W: io::Write> io::Write for RedactingLineWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match std::str::from_utf8(buf) {
            Ok(text) => {
                self.0.write_all(redact_url_userinfo(text).as_bytes())?;
                Ok(buf.len())
            }
            Err(_) => self.0.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
