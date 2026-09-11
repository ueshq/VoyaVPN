use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
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

/// Supervises OS child processes.
///
/// **Every method blocks the calling thread**, some of them for seconds:
/// `run_oneshot` waits for the child to exit (the sudo-kill helper sleeps for
/// at least a second, and an elevation prompt only returns once the user
/// answers it) and `stop` waits for the child to be signalled and reaped, up to
/// `CHILD_STOP_TIMEOUT`. Async callers must therefore go through
/// `tokio::task::spawn_blocking` or a dedicated OS thread, and a Tauri command
/// reaching them must be `async` or it freezes the window from the UI thread.
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

mod logging;
mod runner;
pub use logging::{
    classify_core_log_line, ProcessLogLevel, ProcessLogSink, ProcessOutputStream,
    CORE_OUTPUT_TARGET,
};
pub use runner::StdProcessRunner;

mod scripts;
pub use scripts::write_generated_scripts;

mod job;
pub use job::{
    JobAssignedRunner, NoopProcessJobFactory, PlatformProcessJobFactory, ProcessJob,
    ProcessJobFactory,
};

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
    #[error("timed out after {timeout_ms}ms waiting for process {process_id} to stop")]
    StopTimeout { process_id: u32, timeout_ms: u128 },
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

/// Combine process diagnostics, preserving both streams when present.
#[must_use]
pub fn command_output_text(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    if stderr.trim().is_empty() {
        stdout.into_owned()
    } else if stdout.trim().is_empty() {
        stderr.into_owned()
    } else {
        format!("{stdout}\n{stderr}")
    }
}
