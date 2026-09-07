use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Child, Command},
};

#[cfg(windows)]
use std::{sync::mpsc, thread, time::Duration};

#[path = "voyavpn-tunnel-service/layout.rs"]
mod layout;

use layout::{
    canonicalize_existing_or_parent, describe_paths, ensure_config_files_are_contained,
    is_inside_any, read_config_within_limit, ServiceLayout,
};

#[cfg(windows)]
const SERVICE_NAME: &str = "VoyaVPNTunnelService";
/// Where a service start failure is recorded. An SCM-hosted process has no
/// stderr, so without this the `sing-box check` output — the only thing that
/// explains most TUN start failures — was written to a discarded stream.
#[cfg(windows)]
const SERVICE_ERROR_LOG_NAME: &str = "tunnel-service-error.log";
const STAGED_CONFIG_NAME: &str = "config.json";
const SING_BOX_EXES: &[&str] = if cfg!(windows) {
    &["sing-box.exe", "sing-box-client.exe"]
} else {
    &["sing-box", "sing-box-client"]
};

fn main() {
    if let Err(error) = entry() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn entry() -> Result<(), ServiceError> {
    let args = env::args_os().collect::<Vec<_>>();
    match args.get(1).and_then(|value| value.to_str()) {
        Some("run") => {
            let config = parse_run_config(&args[2..])?;
            run_foreground(config)
        }
        Some("check") => {
            let config = parse_run_config(&args[2..])?;
            let plan = RuntimePlan::from_config_path(&config, ServiceLayout::from_environment()?)?;
            plan.validate()?;
            Ok(())
        }
        Some("--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some(command) if command.starts_with('-') => Err(ServiceError::InvalidArgs(format!(
            "unknown option: {command}"
        ))),
        _ => run_service(args),
    }
}

fn print_help() {
    println!("VoyaVPN Tunnel Service");
    println!("usage:");
    println!("  voyavpn-tunnel-service run --config <config.json>");
    println!("  voyavpn-tunnel-service check --config <config.json>");
    println!("  voyavpn-tunnel-service   # run under the Windows Service Control Manager");
    println!();
    println!("The service only runs the sing-box executable installed next to itself");
    println!("(<service dir>\\sing_box) and only accepts configs from the VoyaVPN");
    println!("app-data or %ProgramData%\\VoyaVPN\\runtime directories.");
}

fn parse_run_config(args: &[std::ffi::OsString]) -> Result<PathBuf, ServiceError> {
    let mut config = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].to_str() {
            Some("--config") => {
                let Some(value) = args.get(index + 1) else {
                    return Err(ServiceError::InvalidArgs(
                        "--config requires a path".to_string(),
                    ));
                };
                config = Some(PathBuf::from(value));
                index += 2;
            }
            Some(other) => {
                return Err(ServiceError::InvalidArgs(format!(
                    "unknown argument: {other}"
                )));
            }
            None => {
                return Err(ServiceError::InvalidArgs(
                    "argument is not valid UTF-8".to_string(),
                ));
            }
        }
    }

    config.ok_or_else(|| ServiceError::InvalidArgs("missing --config <path>".to_string()))
}

#[cfg(windows)]
fn run_service(args: Vec<std::ffi::OsString>) -> Result<(), ServiceError> {
    use windows_service::{
        define_windows_service,
        service::{
            ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl,
            ServiceExitCode, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };

    /// Reported for every non-terminal transition and for a clean stop.
    const SUCCESS_EXIT: ServiceExitCode = ServiceExitCode::Win32(0);

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(arguments: Vec<std::ffi::OsString>) {
        if let Err(error) = run_windows_service(arguments) {
            eprintln!("{error}");
        }
    }

    fn run_windows_service(arguments: Vec<std::ffi::OsString>) -> Result<(), ServiceError> {
        let config_path = service_config_from_args(&arguments)?;
        let (stop_tx, stop_rx) = mpsc::channel();
        let status_handle =
            service_control_handler::register(SERVICE_NAME, move |event| match event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    let _ = stop_tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            })?;

        set_service_status(&status_handle, ServiceState::StartPending, SUCCESS_EXIT)?;
        // Every failure below has to reach the SCM. Returning early out of
        // `run_windows_service` used to leave the service in StartPending until
        // the SCM gave up with a generic 1067, and the exit code was hard-coded
        // to Win32(0), so a crashed core and a clean stop looked identical to
        // the desktop app's `sc.exe query` poll.
        let result = run_windows_session(&status_handle, &config_path, &stop_rx);
        let exit_code = match &result {
            Ok(()) => SUCCESS_EXIT,
            Err(error) => {
                report_service_failure(error);
                ServiceExitCode::ServiceSpecific(error.service_exit_code())
            }
        };
        let _ = set_service_status(&status_handle, ServiceState::Stopped, exit_code);

        result
    }

    fn run_windows_session(
        status_handle: &service_control_handler::ServiceStatusHandle,
        config_path: &Path,
        stop_rx: &mpsc::Receiver<()>,
    ) -> Result<(), ServiceError> {
        let plan = RuntimePlan::from_config_path(config_path, ServiceLayout::from_environment()?)?;
        plan.validate()?;
        let mut child = plan.spawn()?;
        set_service_status(status_handle, ServiceState::Running, SUCCESS_EXIT)?;

        let result = wait_for_child_or_stop(&mut child, stop_rx);
        // Best effort: the core still has to be reaped even if the SCM refuses
        // the transition, and the caller reports the real outcome.
        let _ = set_service_status(status_handle, ServiceState::StopPending, SUCCESS_EXIT);
        stop_child(&mut child);

        result
    }

    /// Record why the service could not start, where a human can find it.
    fn report_service_failure(error: &ServiceError) {
        // The path is derived from the service's own installed location, never
        // from the caller-supplied start argument: this runs as SYSTEM.
        let Ok(layout) = ServiceLayout::from_environment() else {
            return;
        };
        if fs::create_dir_all(&layout.staging_dir).is_err() {
            return;
        }
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since_epoch| since_epoch.as_secs())
            .unwrap_or_default();
        let _ = fs::write(
            layout.staging_dir.join(SERVICE_ERROR_LOG_NAME),
            format!("[{seconds}] {error}\n"),
        );
    }

    fn set_service_status(
        status_handle: &service_control_handler::ServiceStatusHandle,
        state: ServiceState,
        exit_code: ServiceExitCode,
    ) -> Result<(), ServiceError> {
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: if state == ServiceState::Running {
                ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
            } else {
                ServiceControlAccept::empty()
            },
            exit_code,
            checkpoint: 0,
            wait_hint: Duration::from_secs(10),
            process_id: None,
        })?;
        Ok(())
    }

    fn install_service(executable: PathBuf) -> Result<(), ServiceError> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )?;
        let info = ServiceInfo {
            name: SERVICE_NAME.into(),
            display_name: "VoyaVPN Tunnel Service".into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::OnDemand,
            error_control: ServiceErrorControl::Normal,
            executable_path: executable,
            launch_arguments: Vec::new(),
            dependencies: Vec::new(),
            account_name: None,
            account_password: None,
        };
        let _service = manager.create_service(
            &info,
            ServiceAccess::START
                | ServiceAccess::STOP
                | ServiceAccess::QUERY_STATUS
                | ServiceAccess::DELETE,
        )?;
        Ok(())
    }

    let _ = install_service as fn(PathBuf) -> Result<(), ServiceError>;
    let _ = args;
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

#[cfg(not(windows))]
fn run_service(_args: Vec<std::ffi::OsString>) -> Result<(), ServiceError> {
    Err(ServiceError::Unsupported(
        "Windows service mode is only available on Windows".to_string(),
    ))
}

#[cfg(windows)]
fn service_config_from_args(arguments: &[std::ffi::OsString]) -> Result<PathBuf, ServiceError> {
    // `arguments[0]` is the service name supplied by the SCM. The desktop app
    // passes the runtime config path as the single start argument; it is only a
    // selector, because `ServiceLayout` decides which roots are acceptable.
    arguments
        .iter()
        .skip(1)
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            ServiceError::InvalidArgs("service start requires <main-config-path>".to_string())
        })
}

fn run_foreground(config_path: PathBuf) -> Result<(), ServiceError> {
    let plan = RuntimePlan::from_config_path(&config_path, ServiceLayout::from_environment()?)?;
    plan.validate()?;
    let mut child = plan.spawn()?;
    wait_for_child(&mut child)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePlan {
    layout: ServiceLayout,
    source_config_path: PathBuf,
    staged_config_path: PathBuf,
    sing_box_path: PathBuf,
}

impl RuntimePlan {
    fn from_config_path(config_path: &Path, layout: ServiceLayout) -> Result<Self, ServiceError> {
        let source_config_path = absolute_path(config_path)?;
        let sing_box_path = find_sing_box(&layout.core_dir);
        let staged_config_path = layout.staging_dir.join(STAGED_CONFIG_NAME);

        Ok(Self {
            layout,
            source_config_path,
            staged_config_path,
            sing_box_path,
        })
    }

    fn validate(&self) -> Result<(), ServiceError> {
        if !self.sing_box_path.is_file() {
            return Err(ServiceError::MissingSingBox(self.sing_box_path.clone()));
        }
        self.stage()?;
        self.check_config()?;

        Ok(())
    }

    /// Validates the caller-supplied config and copies it into the
    /// service-owned staging directory. After this the executable, the working
    /// directory and the config the core reads all live in administrator-owned
    /// locations, independent of the argument the caller passed.
    fn stage(&self) -> Result<(), ServiceError> {
        if !self.source_config_path.is_absolute() {
            return Err(self.invalid_config("config path must be absolute"));
        }
        if !self.source_config_path.is_file() {
            return Err(self.invalid_config("config file does not exist"));
        }

        fs::create_dir_all(&self.layout.staging_dir).map_err(|source| ServiceError::Staging {
            path: self.layout.staging_dir.clone(),
            source,
        })?;

        let canonical = canonicalize_existing_or_parent(&self.source_config_path)?;
        if !is_inside_any(&canonical, &self.layout.config_dirs) {
            return Err(self.invalid_config(&format!(
                "config must live in one of {}",
                describe_paths(&self.layout.config_dirs)
            )));
        }

        let contents = read_config_within_limit(&self.source_config_path)?;
        let config = serde_json::from_str::<serde_json::Value>(&contents).map_err(|source| {
            ServiceError::InvalidConfigJson {
                path: self.source_config_path.clone(),
                source,
            }
        })?;
        ensure_config_files_are_contained(&config, &self.layout)?;

        // The staging directory is administrator-owned, but removing any stale
        // entry keeps a pre-planted link from redirecting the copy.
        let _ = fs::remove_file(&self.staged_config_path);
        fs::write(&self.staged_config_path, contents.as_bytes()).map_err(|source| {
            ServiceError::Staging {
                path: self.staged_config_path.clone(),
                source,
            }
        })?;

        Ok(())
    }

    fn invalid_config(&self, reason: &str) -> ServiceError {
        ServiceError::InvalidConfigPath {
            path: self.source_config_path.clone(),
            reason: reason.to_string(),
        }
    }

    fn check_config(&self) -> Result<(), ServiceError> {
        let output = Command::new(&self.sing_box_path)
            .arg("check")
            .arg("-c")
            .arg(&self.staged_config_path)
            .current_dir(&self.layout.staging_dir)
            .output()
            .map_err(|source| ServiceError::CheckSingBox {
                executable: self.sing_box_path.clone(),
                source,
            })?;
        if output.status.success() {
            return Ok(());
        }

        Err(ServiceError::SingBoxCheckFailed {
            status_code: output.status.code(),
            output: command_output_text(&output.stdout, &output.stderr),
        })
    }

    fn spawn(&self) -> Result<Child, ServiceError> {
        Command::new(&self.sing_box_path)
            .arg("run")
            .arg("-c")
            .arg(&self.staged_config_path)
            .arg("--disable-color")
            .current_dir(&self.layout.staging_dir)
            .spawn()
            .map_err(|source| ServiceError::SpawnSingBox {
                executable: self.sing_box_path.clone(),
                source,
            })
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, ServiceError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(ServiceError::CurrentDir)
}

/// Resolves the sing-box the service is allowed to launch. The directory comes
/// from `ServiceLayout`, which derives it from the service executable's own
/// installed location, so no caller argument can influence it.
fn find_sing_box(core_dir: &Path) -> PathBuf {
    for executable in SING_BOX_EXES {
        let candidate = core_dir.join(executable);
        if candidate.is_file() {
            return candidate;
        }
    }

    core_dir.join(SING_BOX_EXES[0])
}

fn command_output_text(stdout: &[u8], stderr: &[u8]) -> String {
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

fn wait_for_child(child: &mut Child) -> Result<(), ServiceError> {
    let status = child.wait().map_err(ServiceError::WaitSingBox)?;
    if status.success() {
        Ok(())
    } else {
        Err(ServiceError::SingBoxExited(status.code()))
    }
}

#[cfg(windows)]
fn wait_for_child_or_stop(
    child: &mut Child,
    stop_rx: &mpsc::Receiver<()>,
) -> Result<(), ServiceError> {
    loop {
        if stop_rx.try_recv().is_ok() {
            return Ok(());
        }
        match child.try_wait().map_err(ServiceError::WaitSingBox)? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => return Err(ServiceError::SingBoxExited(status.code())),
            None => thread::sleep(Duration::from_millis(250)),
        }
    }
}

#[cfg(windows)]
fn stop_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[derive(Debug)]
enum ServiceError {
    InvalidArgs(String),
    InvalidConfigPath {
        path: PathBuf,
        reason: String,
    },
    InvalidConfigJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    InvalidConfigOption {
        field: &'static str,
        value: String,
        reason: String,
    },
    ConfigTooLarge {
        path: PathBuf,
        limit: u64,
    },
    Staging {
        path: PathBuf,
        source: io::Error,
    },
    ServiceLocation(io::Error),
    MissingSingBox(PathBuf),
    CheckSingBox {
        executable: PathBuf,
        source: io::Error,
    },
    SingBoxCheckFailed {
        status_code: Option<i32>,
        output: String,
    },
    SpawnSingBox {
        executable: PathBuf,
        source: io::Error,
    },
    WaitSingBox(io::Error),
    SingBoxExited(Option<i32>),
    Canonicalize {
        path: PathBuf,
        source: io::Error,
    },
    CurrentDir(io::Error),
    Unsupported(String),
    #[cfg(windows)]
    WindowsService(windows_service::Error),
}

#[cfg(windows)]
impl ServiceError {
    /// Service-specific exit code reported to the SCM.
    ///
    /// `sc query`/`sc queryex` surfaces this verbatim, so the desktop app's
    /// service poll can tell a rejected config from a missing core from a
    /// crashed sing-box instead of seeing `Stopped` for all of them.
    fn service_exit_code(&self) -> u32 {
        match self {
            Self::InvalidArgs(_) => 1,
            Self::InvalidConfigPath { .. }
            | Self::InvalidConfigJson { .. }
            | Self::InvalidConfigOption { .. }
            | Self::ConfigTooLarge { .. } => 2,
            Self::Staging { .. }
            | Self::Canonicalize { .. }
            | Self::CurrentDir(_)
            | Self::ServiceLocation(_) => 3,
            Self::MissingSingBox(_) => 4,
            Self::CheckSingBox { .. } => 5,
            Self::SingBoxCheckFailed { .. } => 6,
            Self::SpawnSingBox { .. } => 7,
            Self::WaitSingBox(_) => 8,
            Self::SingBoxExited(_) => 9,
            Self::Unsupported(_) => 10,
            Self::WindowsService(_) => 11,
        }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgs(message) => write!(formatter, "{message}"),
            Self::InvalidConfigPath { path, reason } => {
                write!(
                    formatter,
                    "invalid config path {}: {reason}",
                    path.display()
                )
            }
            Self::InvalidConfigJson { path, source } => {
                write!(
                    formatter,
                    "config {} is not valid JSON: {source}",
                    path.display()
                )
            }
            Self::InvalidConfigOption {
                field,
                value,
                reason,
            } => {
                write!(
                    formatter,
                    "config option {field} = {value} is rejected: {reason}"
                )
            }
            Self::ConfigTooLarge { path, limit } => {
                write!(
                    formatter,
                    "config {} is larger than the {limit} byte service limit",
                    path.display()
                )
            }
            Self::Staging { path, source } => {
                write!(
                    formatter,
                    "failed to stage the runtime config at {}: {source}",
                    path.display()
                )
            }
            Self::ServiceLocation(source) => {
                write!(
                    formatter,
                    "failed to resolve the service installation directory: {source}"
                )
            }
            Self::MissingSingBox(path) => {
                write!(
                    formatter,
                    "sing-box executable was not found at {}",
                    path.display()
                )
            }
            Self::CheckSingBox { executable, source } => {
                write!(
                    formatter,
                    "failed to run {} check: {source}",
                    executable.display()
                )
            }
            Self::SingBoxCheckFailed {
                status_code,
                output,
            } => {
                write!(
                    formatter,
                    "sing-box config check failed with status {status_code:?}: {output}"
                )
            }
            Self::SpawnSingBox { executable, source } => {
                write!(
                    formatter,
                    "failed to spawn {}: {source}",
                    executable.display()
                )
            }
            Self::WaitSingBox(source) => write!(formatter, "failed to wait for sing-box: {source}"),
            Self::SingBoxExited(code) => write!(formatter, "sing-box exited with status {code:?}"),
            Self::Canonicalize { path, source } => {
                write!(
                    formatter,
                    "failed to canonicalize {}: {source}",
                    path.display()
                )
            }
            Self::CurrentDir(source) => {
                write!(formatter, "failed to resolve current directory: {source}")
            }
            Self::Unsupported(message) => write!(formatter, "{message}"),
            #[cfg(windows)]
            Self::WindowsService(source) => write!(formatter, "Windows service error: {source}"),
        }
    }
}

impl std::error::Error for ServiceError {}

#[cfg(windows)]
impl From<windows_service::Error> for ServiceError {
    fn from(source: windows_service::Error) -> Self {
        Self::WindowsService(source)
    }
}

#[cfg(test)]
mod tests {
    use super::layout::{
        APP_IDENTIFIER, BIN_CONFIG_DIR, MAX_CONFIG_BYTES, PRODUCT_DIR_NAME, RUNTIME_STAGING_DIR,
        SING_BOX_CORE_DIR,
    };
    use super::*;

    struct Fixture {
        root: PathBuf,
        layout: ServiceLayout,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = env::temp_dir().join(format!(
                "voyavpn-tunnel-service-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_nanos())
            ));
            let staging_dir = root
                .join("ProgramData")
                .join(PRODUCT_DIR_NAME)
                .join(RUNTIME_STAGING_DIR);
            let app_data_root = root.join("Users").join("tester").join(APP_IDENTIFIER);
            let core_dir = root
                .join("ProgramFiles")
                .join(PRODUCT_DIR_NAME)
                .join(SING_BOX_CORE_DIR);
            fs::create_dir_all(&staging_dir).expect("create staging directory");
            fs::create_dir_all(app_data_root.join(BIN_CONFIG_DIR)).expect("create app data dir");
            fs::create_dir_all(&core_dir).expect("create core directory");

            let layout = ServiceLayout {
                core_dir,
                staging_dir: staging_dir.clone(),
                config_dirs: vec![staging_dir, app_data_root.join(BIN_CONFIG_DIR)],
                reference_roots: vec![root.join("ProgramData"), app_data_root],
            };
            Self { root, layout }
        }

        fn app_config(&self, contents: &str) -> PathBuf {
            let path = self
                .root
                .join("Users")
                .join("tester")
                .join(APP_IDENTIFIER)
                .join(BIN_CONFIG_DIR)
                .join("config.json");
            fs::write(&path, contents).expect("write app config");
            path
        }

        fn foreign_config(&self, contents: &str) -> PathBuf {
            let directory = self.root.join("evil").join(BIN_CONFIG_DIR);
            fs::create_dir_all(&directory).expect("create foreign config directory");
            let path = directory.join("config.json");
            fs::write(&path, contents).expect("write foreign config");
            path
        }

        fn plan(&self, config_path: &Path) -> RuntimePlan {
            RuntimePlan::from_config_path(config_path, self.layout.clone()).expect("runtime plan")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    const MINIMAL_CONFIG: &str = r#"{"inbounds":[],"outbounds":[]}"#;

    #[test]
    fn staging_accepts_a_config_from_the_app_data_root() {
        let fixture = Fixture::new("accepts-app-data");
        let config = fixture.app_config(MINIMAL_CONFIG);
        let plan = fixture.plan(&config);

        plan.stage().expect("stage app-data config");

        assert_eq!(
            fs::read_to_string(&plan.staged_config_path).expect("read staged config"),
            MINIMAL_CONFIG
        );
        assert!(plan
            .staged_config_path
            .starts_with(&fixture.layout.staging_dir));
    }

    #[test]
    fn staging_rejects_a_config_outside_every_allowed_root() {
        let fixture = Fixture::new("rejects-foreign-root");
        let config = fixture.foreign_config(MINIMAL_CONFIG);
        let plan = fixture.plan(&config);

        let error = plan.stage().expect_err("foreign config must be rejected");

        assert!(
            matches!(&error, ServiceError::InvalidConfigPath { reason, .. }
                if reason.contains("config must live in one of")),
            "unexpected error: {error}"
        );
        assert!(!plan.staged_config_path.exists());
    }

    #[test]
    fn sing_box_path_never_derives_from_the_caller_config_path() {
        let fixture = Fixture::new("pinned-core");
        let config = fixture.foreign_config(MINIMAL_CONFIG);
        let plan = fixture.plan(&config);

        assert!(plan.sing_box_path.starts_with(&fixture.layout.core_dir));
        assert_eq!(
            plan.sing_box_path,
            fixture.layout.core_dir.join(SING_BOX_EXES[0])
        );
        let config_parent = config.parent().expect("config parent");
        assert!(!plan.sing_box_path.starts_with(config_parent));
        assert!(!plan
            .sing_box_path
            .starts_with(config_parent.parent().expect("app dir")));
    }

    #[test]
    fn staging_rejects_a_log_output_outside_the_runtime_root() {
        let fixture = Fixture::new("rejects-log-output");
        let escape = fixture.root.join("outside.log");
        let config = fixture.app_config(&format!(
            r#"{{"log":{{"output":"{}"}}}}"#,
            escape.display().to_string().replace('\\', "\\\\")
        ));

        let error = fixture
            .plan(&config)
            .stage()
            .expect_err("escaping log output must be rejected");

        assert!(
            matches!(&error, ServiceError::InvalidConfigOption { field, .. } if *field == "log.output"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn staging_rejects_a_relative_cache_path_that_escapes_the_runtime_root() {
        let fixture = Fixture::new("rejects-relative-escape");
        let config = fixture
            .app_config(r#"{"experimental":{"cache_file":{"enabled":true,"path":"../cache.db"}}}"#);

        let error = fixture
            .plan(&config)
            .stage()
            .expect_err("relative escape must be rejected");

        assert!(
            matches!(&error, ServiceError::InvalidConfigOption { field, .. }
                if *field == "experimental.cache_file.path"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn staging_accepts_the_generated_cache_and_local_ruleset_paths() {
        let fixture = Fixture::new("accepts-generated-paths");
        let ruleset_dir = fixture
            .root
            .join("Users")
            .join("tester")
            .join(APP_IDENTIFIER)
            .join("bin")
            .join("srss");
        fs::create_dir_all(&ruleset_dir).expect("create ruleset directory");
        let ruleset = ruleset_dir.join("geosite-cn.srs");
        fs::write(&ruleset, "srs").expect("write ruleset");
        let config = fixture.app_config(&format!(
            r#"{{"experimental":{{"cache_file":{{"enabled":true,"path":"cache.db"}}}},"route":{{"rule_set":[{{"tag":"geosite-cn","type":"local","format":"binary","path":"{}"}}]}}}}"#,
            ruleset.display().to_string().replace('\\', "\\\\")
        ));

        fixture
            .plan(&config)
            .stage()
            .expect("generated config paths must be accepted");
    }

    #[test]
    fn staging_rejects_a_local_ruleset_outside_the_reference_roots() {
        let fixture = Fixture::new("rejects-foreign-ruleset");
        let ruleset = fixture.root.join("geosite-cn.srs");
        fs::write(&ruleset, "srs").expect("write ruleset");
        let config = fixture.app_config(&format!(
            r#"{{"route":{{"rule_set":[{{"tag":"geosite-cn","type":"local","format":"binary","path":"{}"}}]}}}}"#,
            ruleset.display().to_string().replace('\\', "\\\\")
        ));

        let error = fixture
            .plan(&config)
            .stage()
            .expect_err("foreign ruleset must be rejected");

        assert!(
            matches!(&error, ServiceError::InvalidConfigOption { field, .. }
                if *field == "route.rule_set.path"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn staging_rejects_an_oversized_config() {
        let fixture = Fixture::new("rejects-oversized");
        let mut padding = String::from("{\"comment\":\"");
        padding.push_str(&"a".repeat(usize::try_from(MAX_CONFIG_BYTES).unwrap_or(usize::MAX) + 8));
        padding.push_str("\"}");
        let config = fixture.app_config(&padding);

        let error = fixture
            .plan(&config)
            .stage()
            .expect_err("oversized config must be rejected");

        assert!(
            matches!(error, ServiceError::ConfigTooLarge { .. }),
            "oversized config must be reported as too large"
        );
    }

    #[test]
    fn staging_rejects_a_config_that_is_not_json() {
        let fixture = Fixture::new("rejects-non-json");
        let config = fixture.app_config("not json");

        let error = fixture
            .plan(&config)
            .stage()
            .expect_err("non-JSON config must be rejected");

        assert!(matches!(error, ServiceError::InvalidConfigJson { .. }));
    }

    #[test]
    fn validate_reports_a_missing_managed_core_before_touching_the_config() {
        let fixture = Fixture::new("missing-core");
        let config = fixture.foreign_config(MINIMAL_CONFIG);

        let error = fixture
            .plan(&config)
            .validate()
            .expect_err("missing core must be reported");

        assert!(matches!(error, ServiceError::MissingSingBox(path)
            if path == fixture.layout.core_dir.join(SING_BOX_EXES[0])));
    }
}
