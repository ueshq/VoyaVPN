use super::ProcessRole;
use std::{
    io::{self, BufRead},
    sync::Arc,
    thread,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOutputStream {
    Stdout,
    Stderr,
}

pub trait ProcessLogSink: Send + Sync {
    fn line(&self, role: ProcessRole, stream: ProcessOutputStream, line: String);
}

/// Severity of a single core log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Tracing target carrying raw child-process output lines.
///
/// These lines are unredacted core output and are already delivered to the
/// desktop log panel through [`ProcessLogSink`], which redacts them. A
/// subscriber that persists events must therefore opt in to this target
/// deliberately instead of picking it up through a module-path filter.
pub const CORE_OUTPUT_TARGET: &str = "voya::core_output";

/// Number of leading whitespace-separated tokens searched for a level token.
///
/// sing-box prefixes its level with a timezone offset, a date and a time, so the
/// level is always within the first few tokens; scanning the whole line would
/// misclassify messages that merely mention a level name.
const CORE_LOG_LEVEL_SCAN_TOKENS: usize = 8;

/// Classify a core log line by the level token the core itself wrote.
///
/// The stream a line arrives on carries no severity: sing-box writes every level
/// (TRACE..PANIC) to stderr unless `log.output` is configured, so deriving the
/// level from `ProcessOutputStream` would mark every core line as a warning.
/// Lines without a recognizable level token are treated as `Info`.
#[must_use]
pub fn classify_core_log_line(line: &str) -> ProcessLogLevel {
    line.split_whitespace()
        .take(CORE_LOG_LEVEL_SCAN_TOKENS)
        .find_map(|token| {
            let token = token.trim_matches(|character: char| !character.is_ascii_alphabetic());
            match token.to_ascii_uppercase().as_str() {
                "TRACE" => Some(ProcessLogLevel::Trace),
                "DEBUG" => Some(ProcessLogLevel::Debug),
                "INFO" => Some(ProcessLogLevel::Info),
                "WARN" | "WARNING" => Some(ProcessLogLevel::Warn),
                "ERROR" | "FATAL" | "PANIC" => Some(ProcessLogLevel::Error),
                _ => None,
            }
        })
        .unwrap_or(ProcessLogLevel::Info)
}

pub(super) fn drain_child_pipe<T>(
    pipe: Option<T>,
    role: ProcessRole,
    stream: ProcessOutputStream,
    log_sink: Option<Arc<dyn ProcessLogSink>>,
) where
    T: io::Read + Send + 'static,
{
    let Some(pipe) = pipe else {
        return;
    };

    thread::spawn(move || {
        let reader = io::BufReader::new(pipe);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(log_sink) = &log_sink {
                log_sink.line(role, stream, line.clone());
            }
            match classify_core_log_line(&line) {
                ProcessLogLevel::Trace | ProcessLogLevel::Debug => {
                    tracing::debug!(target: CORE_OUTPUT_TARGET, ?role, ?stream, "{line}");
                }
                ProcessLogLevel::Info => {
                    tracing::info!(target: CORE_OUTPUT_TARGET, ?role, ?stream, "{line}");
                }
                ProcessLogLevel::Warn => {
                    tracing::warn!(target: CORE_OUTPUT_TARGET, ?role, ?stream, "{line}");
                }
                ProcessLogLevel::Error => {
                    tracing::error!(target: CORE_OUTPUT_TARGET, ?role, ?stream, "{line}");
                }
            }
        }
    });
}
