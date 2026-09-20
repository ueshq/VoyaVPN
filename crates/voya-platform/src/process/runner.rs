//! Spawn, own and reap subprocesses, including bounded stop.
use super::logging::drain_child_pipe;
use super::{
    hidden_command, write_generated_scripts, ProcessError, ProcessExit, ProcessExitHandler,
    ProcessHandle, ProcessLogSink, ProcessOutput, ProcessOutputStream, ProcessRunner, ProcessSpawn,
};
use std::{
    collections::HashMap,
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc, Mutex, MutexGuard, Weak},
    thread,
    time::Duration,
};

pub struct StdProcessRunner {
    children: Arc<Mutex<HashMap<u32, ChildControl>>>,
    exit_handler: Mutex<Option<Arc<dyn ProcessExitHandler>>>,
    log_sink: Option<Arc<dyn ProcessLogSink>>,
}

impl StdProcessRunner {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_log_sink(log_sink: Arc<dyn ProcessLogSink>) -> Self {
        // `Drop` rules out `..Self::default()`, so the field is set afterwards.
        let mut runner = Self::default();
        runner.log_sink = Some(log_sink);
        runner
    }
}

impl Default for StdProcessRunner {
    fn default() -> Self {
        Self {
            children: Arc::new(Mutex::new(HashMap::new())),
            exit_handler: Mutex::new(None),
            log_sink: None,
        }
    }
}

impl ProcessRunner for StdProcessRunner {
    fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
        write_generated_scripts(&request.generated_scripts)?;

        let mut command = build_command(&request);
        command.stdin(Stdio::null());

        if request.display_log {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
        } else {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }

        let mut child = command.spawn().map_err(|source| ProcessError::Spawn {
            executable: request.executable.clone(),
            source,
        })?;

        if request.display_log {
            drain_child_pipe(
                child.stdout.take(),
                request.role,
                ProcessOutputStream::Stdout,
                self.log_sink.clone(),
            );
            drain_child_pipe(
                child.stderr.take(),
                request.role,
                ProcessOutputStream::Stderr,
                self.log_sink.clone(),
            );
        }

        // The child is running from here on, so nothing below may return early:
        // dropping a `Child` neither kills nor reaps it, and an elevated core
        // left that way holds the TUN device with no stop path.
        let handle = ProcessHandle::new(child.id(), request.role);
        let exit_handler = lock_ignoring_poison(&self.exit_handler).clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        lock_ignoring_poison(&self.children).insert(handle.id(), ChildControl { stop_tx });
        spawn_child_reaper(
            handle.clone(),
            child,
            stop_rx,
            exit_handler,
            Arc::downgrade(&self.children),
        );

        Ok(handle)
    }

    fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
        write_generated_scripts(&request.generated_scripts)?;

        let output = build_command(&request)
            .output()
            .map_err(|source| ProcessError::Spawn {
                executable: request.executable.clone(),
                source,
            })?;
        Ok(ProcessOutput {
            status_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn stop(&self, handle: &ProcessHandle) -> Result<(), ProcessError> {
        let child = lock_ignoring_poison(&self.children).remove(&handle.id());
        let Some(child) = child else {
            return Ok(());
        };

        child.stop(handle.id())
    }

    fn set_exit_handler(&self, handler: Option<Arc<dyn ProcessExitHandler>>) {
        let mut slot = lock_ignoring_poison(&self.exit_handler);
        if let (Some(current), Some(next)) = (slot.as_ref(), handler.as_ref()) {
            // Replacing a live handler silently takes process exits away from
            // its owner — the supervisor's crash restart, typically. A second
            // subsystem needs a runner of its own, as bootstrap gives the
            // speedtest and the self-hosted node.
            let same = Arc::ptr_eq(current, next);
            if !same {
                tracing::error!(
                    "process exit handler replaced; give each subsystem its own runner"
                );
            }
            debug_assert!(same, "a runner has one exit-handler slot and it is taken");
        }
        *slot = handler;
    }
}

impl Drop for StdProcessRunner {
    fn drop(&mut self) {
        let children = std::mem::take(&mut *lock_ignoring_poison(&self.children));

        for (pid, child) in children {
            if let Err(error) = child.stop(pid) {
                tracing::warn!(
                    pid,
                    ?error,
                    "failed to stop child process during runner drop"
                );
            }
        }
    }
}

/// Both guarded values stay consistent through a panic: the child map still
/// holds the controls it exists to stop, and the handler slot is one `Option`.
/// Refusing the lock would strand running children instead.
fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct ChildControl {
    stop_tx: mpsc::Sender<ChildCommand>,
}

impl ChildControl {
    fn stop(&self, process_id: u32) -> Result<(), ProcessError> {
        self.stop_with_timeout(process_id, CHILD_STOP_TIMEOUT)
    }

    /// Asks the reaper thread to signal and wait for the child.
    ///
    /// The wait is bounded: callers reach this from a tokio worker and from the
    /// Tauri main thread, and an unbounded `recv()` would pin either of them for
    /// as long as the child refuses to die.
    fn stop_with_timeout(&self, process_id: u32, timeout: Duration) -> Result<(), ProcessError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self
            .stop_tx
            .send(ChildCommand::Stop { reply: reply_tx })
            .is_err()
        {
            // The reaper already returned, which it only does after the child
            // has exited or been killed.
            return Ok(());
        }

        match reply_rx.recv_timeout(timeout) {
            Ok(result) => result,
            // Same reasoning: the reaper dropped the reply channel on its way
            // out, so the child is no longer running.
            Err(mpsc::RecvTimeoutError::Disconnected) => Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ProcessError::StopTimeout {
                process_id,
                timeout_ms: timeout.as_millis(),
            }),
        }
    }
}

enum ChildCommand {
    Stop {
        reply: mpsc::Sender<Result<(), ProcessError>>,
    },
}

const CHILD_REAPER_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Upper bound on [`ProcessRunner::stop`].
///
/// The reaper picks the stop request up within one poll interval, and a
/// healthy child exits on `SIGTERM` in milliseconds; one that ignores it is
/// killed after `CHILD_TERM_GRACE` (unix). The budget exists so a child stuck in an
/// uninterruptible state surfaces as [`ProcessError::StopTimeout`] instead of
/// blocking the caller forever.
const CHILD_STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a child gets to exit on `SIGTERM` before it is killed: sing-box
/// closes its inbounds and cache file on the way out, which a kill leaves to
/// the next start. The launcher's kill verb gives elevated cores the same.
#[cfg(unix)]
const CHILD_TERM_GRACE: Duration = Duration::from_millis(1500);
#[cfg(unix)]
const CHILD_TERM_POLL_INTERVAL: Duration = Duration::from_millis(20);

fn spawn_child_reaper(
    handle: ProcessHandle,
    child: Child,
    stop_rx: mpsc::Receiver<ChildCommand>,
    exit_handler: Option<Arc<dyn ProcessExitHandler>>,
    children: Weak<Mutex<HashMap<u32, ChildControl>>>,
) {
    thread::spawn(move || {
        run_child_reaper(handle, child, stop_rx, exit_handler, children);
    });
}

fn run_child_reaper(
    handle: ProcessHandle,
    mut child: Child,
    stop_rx: mpsc::Receiver<ChildCommand>,
    exit_handler: Option<Arc<dyn ProcessExitHandler>>,
    children: Weak<Mutex<HashMap<u32, ChildControl>>>,
) {
    loop {
        match stop_rx.recv_timeout(CHILD_REAPER_POLL_INTERVAL) {
            Ok(ChildCommand::Stop { reply }) => {
                let _ = reply.send(stop_child(&mut child));
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Err(error) = stop_child(&mut child) {
                    tracing::warn!(
                        pid = handle.id(),
                        ?error,
                        "failed to stop child process after control channel closed"
                    );
                }
                return;
            }
        }

        match child.try_wait().map_err(ProcessError::Wait) {
            Ok(Some(status)) => {
                remove_child_control(&children, handle.id());
                if let Some(exit_handler) = exit_handler {
                    exit_handler.process_exited(ProcessExit {
                        process_id: handle.id(),
                        role: handle.role(),
                        exit_code: status.code(),
                    });
                }
                return;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    pid = handle.id(),
                    ?error,
                    "failed to wait for child process exit"
                );
                remove_child_control(&children, handle.id());
                if let Some(exit_handler) = exit_handler {
                    exit_handler.process_exited(ProcessExit {
                        process_id: handle.id(),
                        role: handle.role(),
                        exit_code: None,
                    });
                }
                return;
            }
        }
    }
}

fn stop_child(child: &mut Child) -> Result<(), ProcessError> {
    if child.try_wait().map_err(ProcessError::Wait)?.is_some() {
        return Ok(());
    }
    #[cfg(unix)]
    if terminate(child)? {
        return Ok(());
    }
    child.kill().map_err(ProcessError::Stop)?;
    child.wait().map_err(ProcessError::Wait)?;
    Ok(())
}

/// Sends `SIGTERM` and waits up to [`CHILD_TERM_GRACE`]; returns whether the
/// child exited (and was reaped) in time.
#[cfg(unix)]
fn terminate(child: &mut Child) -> Result<bool, ProcessError> {
    let Ok(pid) = libc::pid_t::try_from(child.id()) else {
        return Ok(false);
    };
    // SAFETY: `kill` takes no pointers. The child is unreaped (only this
    // thread waits it), so `pid` still names it and cannot have been reused.
    if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
        return Ok(false);
    }
    let deadline = std::time::Instant::now() + CHILD_TERM_GRACE;
    loop {
        if child.try_wait().map_err(ProcessError::Wait)?.is_some() {
            return Ok(true);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(CHILD_TERM_POLL_INTERVAL);
    }
}

fn remove_child_control(children: &Weak<Mutex<HashMap<u32, ChildControl>>>, process_id: u32) {
    let Some(children) = children.upgrade() else {
        return;
    };
    lock_ignoring_poison(&children).remove(&process_id);
}

fn build_command(request: &ProcessSpawn) -> Command {
    let mut command = hidden_command(&request.executable);
    command.args(&request.arguments);
    if !request.working_dir.as_os_str().is_empty() {
        command.current_dir(&request.working_dir);
    }
    command.envs(&request.environment);
    command
}

#[cfg(test)]
mod tests;
