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
    /// One line of child output; `level` is [`classify_core_log_line`]'s
    /// reading of it, so a sink need not parse the line again.
    fn line(
        &self,
        role: ProcessRole,
        stream: ProcessOutputStream,
        level: ProcessLogLevel,
        line: String,
    );
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
        for_each_output_line(pipe, |line| {
            let level = classify_core_log_line(&line);
            match level {
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
            if let Some(log_sink) = &log_sink {
                log_sink.line(role, stream, level, line);
            }
        });
    });
}

/// Hands `pipe` to `on_line` one line at a time until it reaches end of file.
///
/// The pipe has to be read to the end whatever is in it. A line that is not
/// UTF-8 — a sniffed host name, a node remark — is decoded lossily rather than
/// treated as the end of the stream: giving up there would drop the pipe, and
/// the child's next write to it is then a `SIGPIPE` that takes the core down.
fn for_each_output_line(pipe: impl io::Read, mut on_line: impl FnMut(String)) {
    let mut reader = io::BufReader::new(pipe);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
        if buffer.last() == Some(&b'\n') {
            buffer.pop();
            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }
        }
        on_line(String::from_utf8_lossy(&buffer).into_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::for_each_output_line;

    #[test]
    fn output_is_read_to_the_end_past_a_line_that_is_not_utf8() {
        let output: &[u8] = b"first\r\n\xff\xfe broken\nlast";
        let mut lines = Vec::new();

        for_each_output_line(output, |line| lines.push(line));

        assert_eq!(lines, ["first", "\u{fffd}\u{fffd} broken", "last"]);
    }
}
