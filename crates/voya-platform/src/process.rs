use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc, Mutex, Weak},
    thread,
    time::Duration,
};
#[cfg(unix)]
use std::{
    fs::OpenOptions,
    io::{Seek, SeekFrom},
};

use thiserror::Error;
use zeroize::Zeroizing;

use crate::coreinfo::CoreLaunch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRole {
    Main,
    Pre,
    SudoKill,
    SysProxy,
    Probe,
    Autostart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessHandle {
    id: u32,
    role: ProcessRole,
}

impl ProcessHandle {
    #[must_use]
    pub const fn new(id: u32, role: ProcessRole) -> Self {
        Self { id, role }
    }

    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    #[must_use]
    pub const fn role(&self) -> ProcessRole {
        self.role
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProcessStdin(Zeroizing<String>);

impl ProcessStdin {
    #[must_use]
    pub fn new(secret: Zeroizing<String>) -> Self {
        Self(secret)
    }

    #[must_use]
    pub fn expose_for_process(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for ProcessStdin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProcessStdin(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedScript {
    pub directory: PathBuf,
    pub path: PathBuf,
    pub contents: String,
    pub executable: bool,
}

impl GeneratedScript {
    #[must_use]
    pub fn new(
        directory: impl Into<PathBuf>,
        path: impl Into<PathBuf>,
        contents: impl Into<String>,
        executable: bool,
    ) -> Self {
        Self {
            directory: directory.into(),
            path: path.into(),
            contents: contents.into(),
            executable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpawn {
    pub role: ProcessRole,
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_dir: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub display_log: bool,
    pub stdin: Option<ProcessStdin>,
    pub generated_scripts: Vec<GeneratedScript>,
}

impl ProcessSpawn {
    #[must_use]
    pub fn new(role: ProcessRole, executable: impl Into<PathBuf>) -> Self {
        Self {
            role,
            executable: executable.into(),
            arguments: Vec::new(),
            working_dir: PathBuf::new(),
            environment: BTreeMap::new(),
            display_log: true,
            stdin: None,
            generated_scripts: Vec::new(),
        }
    }

    pub fn from_core_launch(
        role: ProcessRole,
        launch: &CoreLaunch,
        display_log: bool,
    ) -> Result<Self, ProcessError> {
        Ok(Self {
            role,
            executable: launch.executable.clone(),
            arguments: split_command_line(&launch.arguments)?,
            working_dir: launch.working_dir.clone(),
            environment: launch.environment.clone(),
            display_log,
            stdin: None,
            generated_scripts: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_arguments(mut self, arguments: impl IntoIterator<Item = String>) -> Self {
        self.arguments = arguments.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_working_dir(mut self, working_dir: impl Into<PathBuf>) -> Self {
        self.working_dir = working_dir.into();
        self
    }

    #[must_use]
    pub fn with_environment(mut self, environment: BTreeMap<String, String>) -> Self {
        self.environment = environment;
        self
    }

    #[must_use]
    pub fn with_display_log(mut self, display_log: bool) -> Self {
        self.display_log = display_log;
        self
    }

    #[must_use]
    pub fn with_generated_script(mut self, script: GeneratedScript) -> Self {
        self.generated_scripts.push(script);
        self
    }

    #[must_use]
    pub fn has_stdin(&self) -> bool {
        self.stdin.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait ProcessRunner: Send + Sync {
    fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError>;
    fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError>;
    fn stop(&self, handle: &ProcessHandle) -> Result<(), ProcessError>;
    fn set_exit_handler(&self, _handler: Option<Arc<dyn ProcessExitHandler>>) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessExit {
    pub process_id: u32,
    pub role: ProcessRole,
    pub exit_code: Option<i32>,
}

pub trait ProcessExitHandler: Send + Sync {
    fn process_exited(&self, exit: ProcessExit);
}

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

        child.stop()
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
            if let Err(error) = child.stop() {
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
    fn stop(&self) -> Result<(), ProcessError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self
            .stop_tx
            .send(ChildCommand::Stop { reply: reply_tx })
            .is_err()
        {
            return Ok(());
        }

        match reply_rx.recv() {
            Ok(result) => result,
            Err(_) => Ok(()),
        }
    }
}

enum ChildCommand {
    Stop {
        reply: mpsc::Sender<Result<(), ProcessError>>,
    },
}

const CHILD_REAPER_POLL_INTERVAL: Duration = Duration::from_millis(100);

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

fn drain_child_pipe<T>(
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

mod scripts;
pub use scripts::write_generated_scripts;

mod job;
pub use job::{NoopProcessJobFactory, PlatformProcessJobFactory, ProcessJob, ProcessJobFactory};

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("failed to spawn process {executable}: {source}")]
    Spawn {
        executable: PathBuf,
        source: io::Error,
    },
    #[error("failed to write process stdin: {0}")]
    WriteStdin(io::Error),
    #[error("process stdin pipe was unavailable")]
    MissingStdinPipe,
    #[error("failed while waiting for process: {0}")]
    Wait(io::Error),
    #[error("failed to stop process: {0}")]
    Stop(io::Error),
    #[error("failed to write generated script {path}: {source}")]
    WriteGeneratedScript { path: PathBuf, source: io::Error },
    #[error("generated script path {path} is outside managed directory {directory}")]
    GeneratedScriptPathOutsideDirectory { path: PathBuf, directory: PathBuf },
    #[error("insecure generated script path {path}: {reason}")]
    InsecureGeneratedScriptPath { path: PathBuf, reason: &'static str },
    #[error("insecure generated script directory {path}: {reason}")]
    InsecureGeneratedScriptDirectory { path: PathBuf, reason: &'static str },
    #[error("failed to parse command line: {0}")]
    ArgumentParse(String),
    #[error("process lock poisoned: {0}")]
    LockPoisoned(&'static str),
    #[error("process job error: {0}")]
    Job(String),
}

pub fn split_command_line(input: &str) -> Result<Vec<String>, ProcessError> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut saw_token = false;

    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            saw_token = true;
            continue;
        }

        if character == '\\' {
            escaped = true;
            saw_token = true;
            continue;
        }

        match quote {
            Some(active_quote) if character == active_quote => {
                quote = None;
                saw_token = true;
            }
            Some(_) => {
                current.push(character);
                saw_token = true;
            }
            None if character == '\'' || character == '"' => {
                quote = Some(character);
                saw_token = true;
            }
            None if character.is_whitespace() => {
                if saw_token {
                    args.push(std::mem::take(&mut current));
                    saw_token = false;
                }
            }
            None => {
                current.push(character);
                saw_token = true;
            }
        }
    }

    if escaped {
        current.push('\\');
    }

    if let Some(active_quote) = quote {
        return Err(ProcessError::ArgumentParse(format!(
            "unterminated quote {active_quote}"
        )));
    }

    if saw_token {
        args.push(current);
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    #[cfg(unix)]
    use std::{sync::mpsc, time::Duration};

    use crate::test_support::RecordingRunner;

    use super::*;

    #[test]
    fn process_split_command_line_preserves_quoted_config_paths() {
        let args = split_command_line("run -c \"/tmp/Voya VPN/config.json\" --disable-color")
            .expect("arguments split");

        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "-c".to_string(),
                "/tmp/Voya VPN/config.json".to_string(),
                "--disable-color".to_string(),
            ]
        );
    }

    #[test]
    fn process_core_log_level_comes_from_the_line_not_the_stream() {
        assert_eq!(
            classify_core_log_line("+0800 2026-09-07 10:11:12 INFO router: loaded rules"),
            ProcessLogLevel::Info
        );
        assert_eq!(
            classify_core_log_line("+0800 2026-09-07 10:11:12 DEBUG dns: cached answer"),
            ProcessLogLevel::Debug
        );
        assert_eq!(
            classify_core_log_line("2026-09-07 10:11:12 TRACE inbound: packet"),
            ProcessLogLevel::Trace
        );
        assert_eq!(
            classify_core_log_line("WARN outbound: connection reset"),
            ProcessLogLevel::Warn
        );
        assert_eq!(
            classify_core_log_line("+0800 2026-09-07 10:11:12 FATAL start service"),
            ProcessLogLevel::Error
        );
        assert_eq!(
            classify_core_log_line("ERROR[0001] listen tcp: address already in use"),
            ProcessLogLevel::Error
        );
        assert_eq!(
            classify_core_log_line("plain line without a level token"),
            ProcessLogLevel::Info
        );
        assert_eq!(
            classify_core_log_line("+0800 2026-09-07 10:11:12 INFO dial info.example.com WARN"),
            ProcessLogLevel::Info,
            "the first level token in the line wins"
        );
    }

    #[test]
    fn process_stdin_debug_does_not_expose_secret() {
        let stdin = ProcessStdin::new(Zeroizing::new("secret-password".to_string()));
        assert_eq!(format!("{stdin:?}"), "ProcessStdin(<redacted>)");
    }

    #[cfg(unix)]
    #[test]
    fn process_generated_script_rewrites_existing_file_and_locks_down_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_temp_root("generated-script-rewrite");
        let directory = root.join("guiTemps").join("sudo");
        fs::create_dir_all(&directory).expect("create script directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o777))
            .expect("make directory too broad");
        let script_path = directory.join("run_as_sudo.sh");
        fs::write(&script_path, "stale").expect("write stale script");
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
            .expect("make script too broad");

        write_generated_scripts(&[GeneratedScript::new(
            directory.clone(),
            script_path.clone(),
            "#!/bin/sh\nexit 0\n",
            true,
        )])
        .expect("rewrite generated script");

        assert_eq!(
            fs::read_to_string(&script_path).expect("read script"),
            "#!/bin/sh\nexit 0\n"
        );
        assert_eq!(
            fs::metadata(&directory)
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&script_path)
                .expect("script metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn process_generated_script_rejects_paths_outside_managed_directory() {
        let root = unique_temp_root("generated-script-outside");
        let directory = root.join("guiTemps").join("sudo");
        let outside = root.join("guiTemps").join("sysproxy");
        fs::create_dir_all(&directory).expect("create script directory");
        fs::create_dir_all(&outside).expect("create outside directory");
        let script_path = outside.join("run_as_sudo.sh");

        let error = write_generated_scripts(&[GeneratedScript::new(
            directory.clone(),
            script_path.clone(),
            "#!/bin/sh\n",
            true,
        )])
        .expect_err("outside path should fail");

        assert!(matches!(
            error,
            ProcessError::GeneratedScriptPathOutsideDirectory { path, directory: managed }
                if path == script_path && managed.ends_with("sudo")
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn process_generated_script_rejects_symbolic_link_targets() {
        let root = unique_temp_root("generated-script-symlink");
        let directory = root.join("guiTemps").join("sysproxy");
        fs::create_dir_all(&directory).expect("create script directory");
        let outside = root.join("outside.sh");
        fs::write(&outside, "outside").expect("write outside target");
        let script_path = directory.join("proxy_set_linux.sh");
        std::os::unix::fs::symlink(&outside, &script_path).expect("create script symlink");

        let error = write_generated_scripts(&[GeneratedScript::new(
            directory,
            script_path,
            "#!/bin/sh\n",
            true,
        )])
        .expect_err("symlink path should fail");

        assert!(matches!(
            error,
            ProcessError::InsecureGeneratedScriptPath { reason, .. }
                if reason.contains("symbolic link")
        ));
        assert_eq!(
            fs::read_to_string(&outside).expect("read outside target"),
            "outside"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn process_generated_script_rejects_hard_linked_targets() {
        let root = unique_temp_root("generated-script-hardlink");
        let directory = root.join("guiTemps").join("sudo");
        fs::create_dir_all(&directory).expect("create script directory");
        let outside = root.join("outside.sh");
        fs::write(&outside, "outside").expect("write outside target");
        let script_path = directory.join("run_as_sudo.sh");
        fs::hard_link(&outside, &script_path).expect("create hard link");

        let error = write_generated_scripts(&[GeneratedScript::new(
            directory,
            script_path,
            "#!/bin/sh\n",
            true,
        )])
        .expect_err("hard linked path should fail");

        assert!(
            matches!(
                &error,
                ProcessError::InsecureGeneratedScriptPath { reason, .. }
                    if reason.contains("hard links")
            ),
            "unexpected error: {error}"
        );
        assert_eq!(
            fs::read_to_string(&outside).expect("read outside target"),
            "outside"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn process_generated_script_rejects_non_regular_targets() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};

        let root = unique_temp_root("generated-script-fifo");
        let directory = root.join("guiTemps").join("sysproxy");
        fs::create_dir_all(&directory).expect("create script directory");
        let script_path = directory.join("proxy_set_linux.sh");
        let raw_path = CString::new(script_path.as_os_str().as_bytes()).expect("fifo path");
        // SAFETY: `raw_path` stays alive for the call and `mkfifo` only reads
        // the NUL-terminated path it is given.
        let created = unsafe { libc::mkfifo(raw_path.as_ptr(), 0o600) };
        assert_eq!(created, 0, "mkfifo should create the test target");

        let error = write_generated_scripts(&[GeneratedScript::new(
            directory,
            script_path,
            "#!/bin/sh\n",
            true,
        )])
        .expect_err("non-regular path should fail");

        assert!(
            matches!(
                &error,
                ProcessError::InsecureGeneratedScriptPath { reason, .. }
                    if reason.contains("not a regular file")
            ),
            "unexpected error: {error}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn process_runner_trait_supports_fake_process_runner() {
        let runner = RecordingRunner::default();
        let spawn = ProcessSpawn::new(ProcessRole::Main, "/bin/echo")
            .with_arguments(["hello".to_string()])
            .with_working_dir("/tmp")
            .with_environment(BTreeMap::new());
        let handle = runner.spawn(spawn).expect("spawn");
        runner.stop(&handle).expect("stop");

        assert_eq!(runner.events().as_slice(), ["spawn:Main", "stop:Main"]);
    }

    #[cfg(unix)]
    struct RecordingExitHandler {
        tx: mpsc::Sender<ProcessExit>,
    }

    #[cfg(unix)]
    impl ProcessExitHandler for RecordingExitHandler {
        fn process_exited(&self, exit: ProcessExit) {
            let _ = self.tx.send(exit);
        }
    }

    #[cfg(unix)]
    #[test]
    fn std_process_runner_reports_and_reaps_exited_children() {
        let (tx, rx) = mpsc::channel();
        let runner = StdProcessRunner::new();
        runner.set_exit_handler(Some(Arc::new(RecordingExitHandler { tx })));

        let handle = runner
            .spawn(
                ProcessSpawn::new(ProcessRole::Probe, "/bin/sh")
                    .with_arguments(["-c".to_string(), "exit 7".to_string()])
                    .with_display_log(false),
            )
            .expect("spawn shell");
        let pid = handle.id();

        let exit = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("process exit event");

        assert_eq!(
            exit,
            ProcessExit {
                process_id: pid,
                role: ProcessRole::Probe,
                exit_code: Some(7),
            }
        );
        assert!(!process_is_running(pid));
        runner.stop(&handle).expect("stop reaped child");
    }

    #[cfg(unix)]
    #[test]
    fn std_process_runner_drop_kills_tracked_children() {
        let Some(sleep) = ["/bin/sleep", "/usr/bin/sleep"]
            .into_iter()
            .find(|path| Path::new(path).exists())
        else {
            return;
        };

        let runner = StdProcessRunner::new();
        let handle = runner
            .spawn(
                ProcessSpawn::new(ProcessRole::Probe, sleep)
                    .with_arguments(["30".to_string()])
                    .with_display_log(false),
            )
            .expect("spawn sleep");
        let pid = handle.id();

        drop(runner);

        assert!(!process_is_running(pid));
    }

    #[cfg(unix)]
    #[test]
    fn process_stdin_missing_pipe_fails_and_kills_child() {
        let Some(sleep) = ["/bin/sleep", "/usr/bin/sleep"]
            .into_iter()
            .find(|path| Path::new(path).exists())
        else {
            return;
        };

        let mut child = Command::new(sleep)
            .arg("30")
            .stdin(Stdio::null())
            .spawn()
            .expect("spawn sleep without stdin pipe");
        let pid = child.id();
        let stdin = ProcessStdin::new(Zeroizing::new("secret-password".to_string()));

        let error = write_child_stdin(&mut child, &stdin).expect_err("missing pipe should fail");

        assert!(matches!(error, ProcessError::MissingStdinPipe));
        assert!(!process_is_running(pid));
    }

    #[cfg(unix)]
    fn process_is_running(pid: u32) -> bool {
        let pid = pid.to_string();
        Command::new("kill")
            .args(["-0", pid.as_str()])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    fn unique_temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "voyavpn-process-{name}-{}-{}",
            std::process::id(),
            monotonic_nanos()
        ))
    }

    #[cfg(unix)]
    fn monotonic_nanos() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    }
}
