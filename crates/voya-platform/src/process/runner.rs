//! Spawn, own and reap subprocesses, including bounded stop and stdin delivery.
use super::logging::drain_child_pipe;
use super::{
    write_generated_scripts, ProcessError, ProcessExit, ProcessExitHandler, ProcessHandle,
    ProcessLogSink, ProcessOutput, ProcessOutputStream, ProcessRunner, ProcessSpawn, ProcessStdin,
};
use std::{
    collections::HashMap,
    io::Write,
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc, Mutex, Weak},
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
        Self {
            children: Arc::new(Mutex::new(HashMap::new())),
            exit_handler: Mutex::new(None),
            log_sink: Some(log_sink),
        }
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
        if request.stdin.is_some() {
            command.stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }

        if request.display_log {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
        } else {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }

        let mut child = command.spawn().map_err(|source| ProcessError::Spawn {
            executable: request.executable.clone(),
            source,
        })?;

        if let Some(stdin) = &request.stdin {
            write_child_stdin(&mut child, stdin)?;
        }

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

        let handle = ProcessHandle::new(child.id(), request.role);
        let exit_handler = self
            .exit_handler
            .lock()
            .map_err(|_| ProcessError::LockPoisoned("exit_handler"))?
            .clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        {
            let mut children = self
                .children
                .lock()
                .map_err(|_| ProcessError::LockPoisoned("children"))?;
            children.insert(handle.id(), ChildControl { stop_tx });
        }
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

        let mut command = build_command(&request);
        if let Some(stdin) = &request.stdin {
            command.stdin(Stdio::piped());
            let mut child = command.spawn().map_err(|source| ProcessError::Spawn {
                executable: request.executable.clone(),
                source,
            })?;
            write_child_stdin(&mut child, stdin)?;
            let output = child.wait_with_output().map_err(ProcessError::Wait)?;
            return Ok(ProcessOutput {
                status_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let output = command.output().map_err(|source| ProcessError::Spawn {
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
        let child = {
            let mut children = self
                .children
                .lock()
                .map_err(|_| ProcessError::LockPoisoned("children"))?;
            children.remove(&handle.id())
        };
        let Some(child) = child else {
            return Ok(());
        };

        child.stop(handle.id())
    }

    fn set_exit_handler(&self, handler: Option<Arc<dyn ProcessExitHandler>>) {
        let Ok(mut exit_handler) = self.exit_handler.lock() else {
            tracing::warn!("failed to register process exit handler: exit handler lock poisoned");
            return;
        };
        *exit_handler = handler;
    }
}

impl Drop for StdProcessRunner {
    fn drop(&mut self) {
        let children = {
            let mut children = match self.children.lock() {
                Ok(children) => children,
                Err(poisoned) => poisoned.into_inner(),
            };
            std::mem::take(&mut *children)
        };

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
/// The reaper picks the stop request up within one poll interval and then only
/// has to `kill()` and `wait()`, so a healthy stop finishes in milliseconds.
/// The budget exists so a child stuck in an uninterruptible state surfaces as
/// [`ProcessError::StopTimeout`] instead of blocking the caller forever.
const CHILD_STOP_TIMEOUT: Duration = Duration::from_secs(10);

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
    match child.try_wait().map_err(ProcessError::Wait)? {
        Some(_) => Ok(()),
        None => {
            child.kill().map_err(ProcessError::Stop)?;
            let _ = child.wait().map_err(ProcessError::Wait)?;
            Ok(())
        }
    }
}

fn write_child_stdin(child: &mut Child, stdin: &ProcessStdin) -> Result<(), ProcessError> {
    let Some(mut child_stdin) = child.stdin.take() else {
        if let Err(error) = stop_child(child) {
            tracing::warn!(
                ?error,
                "failed to stop child process after stdin pipe was missing"
            );
            return Err(error);
        }
        return Err(ProcessError::MissingStdinPipe);
    };

    child_stdin
        .write_all(stdin.expose_for_process().as_bytes())
        .and_then(|_| child_stdin.write_all(b"\n"))
        .map_err(ProcessError::WriteStdin)
}

fn remove_child_control(children: &Weak<Mutex<HashMap<u32, ChildControl>>>, process_id: u32) {
    let Some(children) = children.upgrade() else {
        return;
    };
    let Ok(mut children) = children.lock() else {
        tracing::warn!(
            pid = process_id,
            "failed to remove exited child process: children lock poisoned"
        );
        return;
    };
    children.remove(&process_id);
}

fn build_command(request: &ProcessSpawn) -> Command {
    let mut command = Command::new(&request.executable);
    command.args(&request.arguments);
    if !request.working_dir.as_os_str().is_empty() {
        command.current_dir(&request.working_dir);
    }
    command.envs(&request.environment);
    apply_windows_creation_flags(&mut command);
    command
}

/// Keep console-subsystem children (the core, `reg`, helper tools) from opening
/// a console window: Windows allocates one for every such child of a
/// GUI-subsystem parent, regardless of the child's redirected stdio.
#[cfg(windows)]
fn apply_windows_creation_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_windows_creation_flags(_command: &mut Command) {}

#[cfg(test)]
mod tests;
