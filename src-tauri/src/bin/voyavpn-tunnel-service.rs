use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Child, Command},
};

#[cfg(windows)]
use std::{sync::mpsc, thread, time::Duration};

#[cfg(windows)]
const SERVICE_NAME: &str = "VoyaVPNTunnelService";
const SING_BOX_CORE_DIR: &str = "sing_box";
const BIN_CONFIG_DIR: &str = "binConfigs";
const BIN_DIR: &str = "bin";
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
            let plan = RuntimePlan::from_config_path(&config)?;
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

        set_service_status(&status_handle, ServiceState::StartPending)?;
        let plan = RuntimePlan::from_config_path(&config_path)?;
        plan.validate()?;
        let mut child = plan.spawn()?;
        set_service_status(&status_handle, ServiceState::Running)?;

        let result = wait_for_child_or_stop(&mut child, &stop_rx);
        set_service_status(&status_handle, ServiceState::StopPending)?;
        stop_child(&mut child);
        set_service_status(&status_handle, ServiceState::Stopped)?;
        result
    }

    fn set_service_status(
        status_handle: &windows_service::service_control_handler::ServiceStatusHandle,
        state: windows_service::service::ServiceState,
    ) -> Result<(), ServiceError> {
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: if state == ServiceState::Running {
                ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
            } else {
                ServiceControlAccept::empty()
            },
            exit_code: ServiceExitCode::Win32(0),
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
            start_type: ServiceStartType::DemandStart,
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
    for value in arguments {
        let path = PathBuf::from(value);
        if path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some(BIN_CONFIG_DIR)
        {
            return Ok(path);
        }
    }

    Err(ServiceError::InvalidArgs(
        "service start requires <main-config-path>".to_string(),
    ))
}

fn run_foreground(config_path: PathBuf) -> Result<(), ServiceError> {
    let plan = RuntimePlan::from_config_path(&config_path)?;
    plan.validate()?;
    let mut child = plan.spawn()?;
    wait_for_child(&mut child)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePlan {
    app_dir: PathBuf,
    config_path: PathBuf,
    sing_box_path: PathBuf,
}

impl RuntimePlan {
    fn from_config_path(config_path: &Path) -> Result<Self, ServiceError> {
        let config_path = absolute_path(config_path)?;
        let app_dir = app_dir_from_config_path(&config_path)?;
        let sing_box_path = find_sing_box(&app_dir)?;

        Ok(Self {
            app_dir,
            config_path,
            sing_box_path,
        })
    }

    fn validate(&self) -> Result<(), ServiceError> {
        if !self.config_path.is_absolute() {
            return Err(ServiceError::InvalidConfigPath {
                path: self.config_path.clone(),
                reason: "config path must be absolute".to_string(),
            });
        }
        if !self.config_path.is_file() {
            return Err(ServiceError::InvalidConfigPath {
                path: self.config_path.clone(),
                reason: "config file does not exist".to_string(),
            });
        }
        let bin_config_dir = self.app_dir.join(BIN_CONFIG_DIR);
        if !is_path_inside(&self.config_path, &bin_config_dir)? {
            return Err(ServiceError::InvalidConfigPath {
                path: self.config_path.clone(),
                reason: format!("config must live under {}", bin_config_dir.display()),
            });
        }
        if !self.sing_box_path.is_file() {
            return Err(ServiceError::MissingSingBox(self.sing_box_path.clone()));
        }
        self.check_config()?;

        Ok(())
    }

    fn check_config(&self) -> Result<(), ServiceError> {
        let output = Command::new(&self.sing_box_path)
            .arg("check")
            .arg("-c")
            .arg(&self.config_path)
            .current_dir(self.config_path.parent().unwrap_or(&self.app_dir))
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
            .arg(&self.config_path)
            .arg("--disable-color")
            .current_dir(self.config_path.parent().unwrap_or(&self.app_dir))
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

fn app_dir_from_config_path(config_path: &Path) -> Result<PathBuf, ServiceError> {
    let bin_config_dir = config_path
        .parent()
        .ok_or_else(|| ServiceError::InvalidConfigPath {
            path: config_path.to_path_buf(),
            reason: "config path has no parent directory".to_string(),
        })?;
    if bin_config_dir.file_name().and_then(|name| name.to_str()) != Some(BIN_CONFIG_DIR) {
        return Err(ServiceError::InvalidConfigPath {
            path: config_path.to_path_buf(),
            reason: format!("config parent directory must be {BIN_CONFIG_DIR}"),
        });
    }
    bin_config_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| ServiceError::InvalidConfigPath {
            path: config_path.to_path_buf(),
            reason: "config path is not inside an app directory".to_string(),
        })
}

fn find_sing_box(app_dir: &Path) -> Result<PathBuf, ServiceError> {
    let core_dir = app_dir.join(BIN_DIR).join(SING_BOX_CORE_DIR);
    for executable in SING_BOX_EXES {
        let candidate = core_dir.join(executable);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Ok(core_dir.join(SING_BOX_EXES[0]))
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

fn is_path_inside(path: &Path, base: &Path) -> Result<bool, ServiceError> {
    let path = canonicalize_existing_or_parent(path)?;
    let base = canonicalize_existing_or_parent(base)?;
    Ok(path.starts_with(base))
}

fn canonicalize_existing_or_parent(path: &Path) -> Result<PathBuf, ServiceError> {
    match fs::canonicalize(path) {
        Ok(canonical) => Ok(canonical),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let Some(parent) = path.parent() else {
                return Err(ServiceError::Canonicalize {
                    path: path.to_path_buf(),
                    source: error,
                });
            };
            fs::canonicalize(parent).map_err(|source| ServiceError::Canonicalize {
                path: parent.to_path_buf(),
                source,
            })
        }
        Err(source) => Err(ServiceError::Canonicalize {
            path: path.to_path_buf(),
            source,
        }),
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
    use super::*;

    #[test]
    fn app_dir_is_resolved_from_bin_config_path() {
        let app_dir = env::temp_dir().join("VoyaVPN");
        let config = app_dir.join(BIN_CONFIG_DIR).join("config.json");

        assert_eq!(app_dir_from_config_path(&config).expect("app dir"), app_dir);
    }

    #[test]
    fn app_dir_rejects_config_outside_bin_config_dir() {
        let config = env::temp_dir().join("VoyaVPN").join("config.json");
        let error = app_dir_from_config_path(&config).expect_err("invalid config path");

        assert!(matches!(
            error,
            ServiceError::InvalidConfigPath { reason, .. }
                if reason.contains(BIN_CONFIG_DIR)
        ));
    }

    #[test]
    fn sing_box_path_uses_app_data_core_directory() {
        let app_dir = Path::new("/tmp/VoyaVPN");
        assert_eq!(
            find_sing_box(app_dir).expect("path"),
            app_dir
                .join(BIN_DIR)
                .join(SING_BOX_CORE_DIR)
                .join(SING_BOX_EXES[0])
        );
    }
}
