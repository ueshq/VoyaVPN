use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
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
    pub path: PathBuf,
    pub contents: String,
    pub executable: bool,
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
    pub fn with_stdin(mut self, stdin: ProcessStdin) -> Self {
        self.stdin = Some(stdin);
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOutputStream {
    Stdout,
    Stderr,
}

pub trait ProcessLogSink: Send + Sync {
    fn line(&self, role: ProcessRole, stream: ProcessOutputStream, line: String);
}

pub struct StdProcessRunner {
    children: Mutex<HashMap<u32, Child>>,
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
            children: Mutex::new(HashMap::new()),
            log_sink: Some(log_sink),
        }
    }
}

impl Default for StdProcessRunner {
    fn default() -> Self {
        Self {
            children: Mutex::new(HashMap::new()),
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
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin
                    .write_all(stdin.expose_for_process().as_bytes())
                    .and_then(|_| child_stdin.write_all(b"\n"))
                    .map_err(ProcessError::WriteStdin)?;
            }
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
        let mut children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned("children"))?;
        children.insert(handle.id(), child);

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
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin
                    .write_all(stdin.expose_for_process().as_bytes())
                    .and_then(|_| child_stdin.write_all(b"\n"))
                    .map_err(ProcessError::WriteStdin)?;
            }
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
        let mut children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned("children"))?;
        let Some(mut child) = children.remove(&handle.id()) else {
            return Ok(());
        };

        match child.try_wait().map_err(ProcessError::Wait)? {
            Some(_) => Ok(()),
            None => {
                child.kill().map_err(ProcessError::Stop)?;
                let _ = child.wait().map_err(ProcessError::Wait)?;
                Ok(())
            }
        }
    }
}

fn build_command(request: &ProcessSpawn) -> Command {
    let mut command = Command::new(&request.executable);
    command.args(&request.arguments);
    if !request.working_dir.as_os_str().is_empty() {
        command.current_dir(&request.working_dir);
    }
    command.envs(&request.environment);
    command
}

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
            if stream == ProcessOutputStream::Stderr {
                tracing::warn!(?role, "{line}");
            } else {
                tracing::info!(?role, "{line}");
            }
        }
    });
}

fn write_generated_scripts(scripts: &[GeneratedScript]) -> Result<(), ProcessError> {
    for script in scripts {
        if let Some(parent) = script.path.parent() {
            fs::create_dir_all(parent).map_err(|source| ProcessError::WriteGeneratedScript {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&script.path, &script.contents).map_err(|source| {
            ProcessError::WriteGeneratedScript {
                path: script.path.clone(),
                source,
            }
        })?;
        if script.executable {
            make_executable(&script.path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), ProcessError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| ProcessError::WriteGeneratedScript {
        path: path.to_path_buf(),
        source,
    })?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|source| ProcessError::WriteGeneratedScript {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn make_executable(path: &Path) -> Result<(), ProcessError> {
    let _ = path;
    Ok(())
}

pub trait ProcessJob: Send {
    fn assign(&mut self, handle: &ProcessHandle) -> Result<(), ProcessError>;
}

pub trait ProcessJobFactory: Send + Sync {
    fn create_job(&self) -> Result<Option<Box<dyn ProcessJob>>, ProcessError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProcessJobFactory;

impl ProcessJobFactory for NoopProcessJobFactory {
    fn create_job(&self) -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
        Ok(None)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformProcessJobFactory;

impl ProcessJobFactory for PlatformProcessJobFactory {
    fn create_job(&self) -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
        platform_process_job()
    }
}

#[cfg(windows)]
fn platform_process_job() -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
    windows_job::WindowsProcessJob::new().map(|job| Some(Box::new(job) as Box<dyn ProcessJob>))
}

#[cfg(not(windows))]
fn platform_process_job() -> Result<Option<Box<dyn ProcessJob>>, ProcessError> {
    Ok(None)
}

#[cfg(windows)]
mod windows_job {
    use std::{ffi::c_void, mem, ptr};

    use super::{ProcessError, ProcessHandle, ProcessJob};

    type Handle = *mut c_void;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_SET_QUOTA: u32 = 0x0100;

    #[repr(C)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    struct JobObjectBasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    struct JobObjectExtendedLimitInformation {
        basic_limit_information: JobObjectBasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    extern "system" {
        fn CreateJobObjectW(attributes: Handle, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            info_class: u32,
            info: *const c_void,
            info_length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
    }

    pub struct WindowsProcessJob {
        handle: Handle,
    }

    unsafe impl Send for WindowsProcessJob {}

    impl WindowsProcessJob {
        pub fn new() -> Result<Self, ProcessError> {
            unsafe {
                let handle = CreateJobObjectW(ptr::null_mut(), ptr::null());
                if handle.is_null() {
                    return Err(ProcessError::Job("CreateJobObjectW failed".to_string()));
                }

                let mut info: JobObjectExtendedLimitInformation = mem::zeroed();
                info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                    (&info as *const JobObjectExtendedLimitInformation).cast::<c_void>(),
                    mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
                );
                if ok == 0 {
                    let _ = CloseHandle(handle);
                    return Err(ProcessError::Job(
                        "SetInformationJobObject failed".to_string(),
                    ));
                }

                Ok(Self { handle })
            }
        }
    }

    impl ProcessJob for WindowsProcessJob {
        fn assign(&mut self, handle: &ProcessHandle) -> Result<(), ProcessError> {
            unsafe {
                let process = OpenProcess(PROCESS_TERMINATE | PROCESS_SET_QUOTA, 0, handle.id());
                if process.is_null() {
                    return Err(ProcessError::Job(format!(
                        "OpenProcess failed for pid {}",
                        handle.id()
                    )));
                }

                let ok = AssignProcessToJobObject(self.handle, process);
                let _ = CloseHandle(process);
                if ok == 0 {
                    return Err(ProcessError::Job(format!(
                        "AssignProcessToJobObject failed for pid {}",
                        handle.id()
                    )));
                }
            }
            Ok(())
        }
    }

    impl Drop for WindowsProcessJob {
        fn drop(&mut self) {
            unsafe {
                if !self.handle.is_null() {
                    let _ = CloseHandle(self.handle);
                    self.handle = ptr::null_mut();
                }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("failed to spawn process {executable}: {source}")]
    Spawn {
        executable: PathBuf,
        source: io::Error,
    },
    #[error("failed to write process stdin: {0}")]
    WriteStdin(io::Error),
    #[error("failed while waiting for process: {0}")]
    Wait(io::Error),
    #[error("failed to stop process: {0}")]
    Stop(io::Error),
    #[error("failed to write generated script {path}: {source}")]
    WriteGeneratedScript { path: PathBuf, source: io::Error },
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
    use std::{collections::BTreeMap, sync::Mutex};

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
    fn process_stdin_debug_does_not_expose_secret() {
        let stdin = ProcessStdin::new(Zeroizing::new("secret-password".to_string()));
        assert_eq!(format!("{stdin:?}"), "ProcessStdin(<redacted>)");
    }

    #[derive(Default)]
    struct RecordingRunner {
        events: Mutex<Vec<String>>,
    }

    impl ProcessRunner for RecordingRunner {
        fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
            self.events
                .lock()
                .expect("events")
                .push(format!("spawn:{:?}", request.role));
            Ok(ProcessHandle::new(1, request.role))
        }

        fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
            self.events
                .lock()
                .expect("events")
                .push(format!("oneshot:{:?}", request.role));
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn stop(&self, handle: &ProcessHandle) -> Result<(), ProcessError> {
            self.events
                .lock()
                .expect("events")
                .push(format!("stop:{:?}", handle.role()));
            Ok(())
        }
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

        assert_eq!(
            runner.events.lock().expect("events").as_slice(),
            ["spawn:Main", "stop:Main"]
        );
    }
}
