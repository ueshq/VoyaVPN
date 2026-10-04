//! `Command::output` with a deadline.
use std::{
    io::{self, Read},
    process::{Command, ExitStatus, Output, Stdio},
    sync::{mpsc, Arc, Mutex, PoisonError},
    thread,
    time::{Duration, Instant},
};

/// The bound for an OS helper that asks the user nothing — `reg`, `sc`,
/// `gsettings`, `pluginkit`, `codesign`. They answer in milliseconds; this is
/// only there so one that never answers cannot hold its caller for good.
pub(crate) const HELPER_TIMEOUT: Duration = Duration::from_secs(30);

const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// How long an exited child's pipes are given to reach end-of-file. They do so
/// at once unless a grandchild inherited them; that one may hold them open for
/// as long as it lives, so past this the output read so far is the answer.
const PIPE_DRAIN_GRACE: Duration = Duration::from_secs(1);

/// Runs `command` to completion and collects its output, like
/// [`Command::output`], but kills the child and fails with
/// [`io::ErrorKind::TimedOut`] once `timeout` has passed.
pub(crate) fn output_with_timeout(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Both pipes are drained while the child runs: a child that fills one
    // blocks on its next write and would never exit.
    let stdout = PipeReader::start(child.stdout.take());
    let stderr = PipeReader::start(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                // Not knowing whether it exited is no reason to leave it
                // running, or unreaped, behind a caller that has given up.
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
        if Instant::now() >= deadline {
            // The readers are left to finish on their own: a grandchild that
            // inherited the pipes can keep them open past the kill.
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("the process did not exit within {}s", timeout.as_secs()),
            ));
        }
        thread::sleep(POLL_INTERVAL);
    };

    Ok(collect(status, stdout, stderr))
}

fn collect(status: ExitStatus, stdout: PipeReader, stderr: PipeReader) -> Output {
    let drained_by = Instant::now() + PIPE_DRAIN_GRACE;
    Output {
        status,
        stdout: stdout.finish(drained_by),
        stderr: stderr.finish(drained_by),
    }
}

/// One of the child's pipes, read on its own thread into a buffer the caller
/// can take whether or not the pipe ever closes.
struct PipeReader {
    bytes: Arc<Mutex<Vec<u8>>>,
    /// Disconnects when the reader thread ends: end-of-file or a read error.
    ended: mpsc::Receiver<()>,
}

impl PipeReader {
    fn start(pipe: Option<impl Read + Send + 'static>) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let (ending, ended) = mpsc::channel::<()>();
        let sink = Arc::clone(&bytes);
        thread::spawn(move || {
            let _ending = ending;
            let Some(mut pipe) = pipe else { return };
            let mut chunk = [0_u8; 8192];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) => return,
                    Ok(read) => sink
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .extend_from_slice(&chunk[..read]),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(_) => return,
                }
            }
        });

        Self { bytes, ended }
    }

    /// What the pipe delivered, waiting until `deadline` for its end.
    fn finish(self, deadline: Instant) -> Vec<u8> {
        // `Err` either way — the reader ended, or the wait ran out.
        let _ = self
            .ended
            .recv_timeout(deadline.saturating_duration_since(Instant::now()));

        std::mem::take(&mut *self.bytes.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

#[cfg(test)]
mod tests {
    // Paths are spelled out instead of imported: both tests need a shell, so
    // on Windows this module is empty and a glob import would be unused.
    #[cfg(unix)]
    #[test]
    fn output_is_collected_from_a_process_that_exits_in_time() {
        let output = super::output_with_timeout(
            std::process::Command::new("/bin/sh")
                .args(["-c", "printf out; printf err >&2; exit 3"]),
            std::time::Duration::from_secs(10),
        )
        .expect("the helper runs");

        assert_eq!(output.status.code(), Some(3));
        assert_eq!(output.stdout, b"out");
        assert_eq!(output.stderr, b"err");
    }

    #[cfg(unix)]
    #[test]
    fn a_process_that_never_exits_is_killed_at_the_deadline() {
        let started = std::time::Instant::now();

        let error = super::output_with_timeout(
            std::process::Command::new("/bin/sh").args(["-c", "sleep 30"]),
            std::time::Duration::from_millis(100),
        )
        .expect_err("the helper is cut off");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn a_grandchild_holding_the_pipes_does_not_hold_the_caller() {
        let started = std::time::Instant::now();

        // The shell exits at once; the `sleep` it leaves behind inherited both
        // pipes and keeps them open.
        let output = super::output_with_timeout(
            std::process::Command::new("/bin/sh").args(["-c", "printf out; sleep 30 &"]),
            std::time::Duration::from_secs(20),
        )
        .expect("the helper runs");

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, b"out");
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }
}
