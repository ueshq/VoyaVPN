use std::{
    fs, io,
    io::Write,
    net::{IpAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use thiserror::Error;
use voya_core::{SysProxyType, SystemProxyItem};

use crate::{
    coreinfo::TargetOs,
    process::{
        GeneratedScript, ProcessError, ProcessOutput, ProcessRole, ProcessRunner, ProcessSpawn,
    },
};

pub const LOOPBACK: &str = "127.0.0.1";
pub const PAC_FILE_NAME: &str = "pac.txt";
pub const DEFAULT_PAC_TEMPLATE: &str = r#"var proxy = '__PROXY__';
function FindProxyForURL(url, host) {
  if (isPlainHostName(host) || shExpMatch(host, "localhost")) {
    return "DIRECT";
  }
  return proxy;
}
"#;

const LOCAL_EXCEPTIONS: &str = "<local>";
const WINDOWS_INTERNET_SETTINGS_REG_PATH: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
const PAC_ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const PAC_ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(50);
const LINUX_PROXY_SCRIPT_NAME: &str = "proxy_set_linux.sh";
mod manual;
pub use manual::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProxyRequest {
    pub target_os: TargetOs,
    pub item: SystemProxyItem,
    pub force_disable: bool,
    pub socks_port: i32,
    pub pac_port: i32,
    pub config_dir: PathBuf,
    pub script_dir: PathBuf,
    pub pac_url_nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProxyStatus {
    pub management: SystemProxyManagement,
    pub observation: SystemProxyObservation,
    pub manual_cleanup_required: bool,
    pub requested_type: SysProxyType,
    pub effective_type: SysProxyType,
    pub target_os: TargetOs,
    pub pac_available: bool,
    pub proxy: Option<String>,
    pub exceptions: String,
    pub pac_url: Option<String>,
}

impl SystemProxyStatus {
    fn from_request_with_exceptions(
        request: &SystemProxyRequest,
        effective_type: SysProxyType,
        exceptions: String,
    ) -> Self {
        Self {
            management: system_proxy_management(request.target_os),
            observation: SystemProxyObservation::Unknown,
            manual_cleanup_required: false,
            requested_type: request.item.sys_proxy_type,
            effective_type,
            target_os: request.target_os,
            pac_available: pac_available(request.target_os),
            proxy: None,
            exceptions,
            pac_url: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProxySettings {
    pub proxy: String,
    pub exceptions: String,
    pub option_type: WindowsProxyOption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsProxyOption {
    Direct = 1,
    NamedProxy = 2,
    PacUrl = 4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemProxyAction {
    Noop,
    WindowsSetProxy(WindowsProxySettings),
    WindowsClear,
    WindowsSetPac {
        pac_url: String,
    },
    LinuxSet {
        script: ScriptInvocation,
        host: String,
        port: i32,
        exceptions: String,
    },
    LinuxClear {
        script: ScriptInvocation,
    },
    UnsupportedPac,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptInvocation {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub generated_script: Option<GeneratedScript>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProxyPlan {
    pub action: SystemProxyAction,
    pub status: SystemProxyStatus,
}

#[derive(Clone)]
pub struct SystemProxyService {
    observer: Arc<dyn SystemProxyObserver>,
    manual_runtime: Arc<Mutex<ManualRuntime>>,
    runner: Arc<dyn ProcessRunner>,
    pac_manager: Arc<dyn PacManager>,
}

impl SystemProxyService {
    #[must_use]
    pub fn new(runner: Arc<dyn ProcessRunner>, pac_manager: Arc<dyn PacManager>) -> Self {
        Self {
            runner,
            pac_manager,
            observer: Arc::new(PlatformSystemProxyObserver),
            manual_runtime: Arc::default(),
        }
    }

    pub fn apply(
        &self,
        request: &SystemProxyRequest,
    ) -> Result<SystemProxyStatus, SystemProxyError> {
        if system_proxy_management(request.target_os) == SystemProxyManagement::Manual {
            return self.apply_manual(request);
        }
        let plan = plan_system_proxy(request)?;

        if plan.status.effective_type != SysProxyType::Pac {
            self.pac_manager.stop();
        }

        match &plan.action {
            SystemProxyAction::Noop | SystemProxyAction::UnsupportedPac => {}
            SystemProxyAction::WindowsSetProxy(settings) => {
                apply_windows_proxy(&*self.runner, settings)?;
            }
            SystemProxyAction::WindowsClear => {
                apply_windows_clear(&*self.runner)?;
            }
            SystemProxyAction::WindowsSetPac { pac_url } => {
                self.pac_manager.start(PacStartConfig {
                    http_port: request.socks_port,
                    pac_port: request.pac_port,
                    config_dir: request.config_dir.clone(),
                    custom_pac_path: request.item.custom_system_proxy_pac_path.clone(),
                })?;
                apply_windows_proxy(
                    &*self.runner,
                    &WindowsProxySettings {
                        proxy: pac_url.clone(),
                        exceptions: String::new(),
                        option_type: WindowsProxyOption::PacUrl,
                    },
                )?;
            }
            SystemProxyAction::LinuxSet { script, .. }
            | SystemProxyAction::LinuxClear { script } => {
                run_script(&*self.runner, script)?;
            }
        }

        Ok(plan.status)
    }

    pub fn stop_pac(&self) {
        self.clear_manual_runtime();
        self.pac_manager.stop();
    }
}

#[must_use]
pub fn platform_pac_manager() -> Arc<dyn PacManager> {
    #[cfg(any(windows, target_os = "macos"))]
    {
        Arc::new(LocalPacManager::default())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Arc::new(UnsupportedPacManager)
    }
}

pub trait PacManager: Send + Sync {
    fn start(&self, config: PacStartConfig) -> Result<(), SystemProxyError>;
    fn stop(&self);
    fn is_supported(&self) -> bool;
    fn is_running(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacStartConfig {
    pub http_port: i32,
    pub pac_port: i32,
    pub config_dir: PathBuf,
    pub custom_pac_path: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedPacManager;

impl PacManager for UnsupportedPacManager {
    fn start(&self, _config: PacStartConfig) -> Result<(), SystemProxyError> {
        Err(SystemProxyError::PacUnsupported(TargetOs::current()))
    }

    fn stop(&self) {}

    fn is_supported(&self) -> bool {
        false
    }
    fn is_running(&self) -> bool {
        false
    }
}

#[derive(Debug, Default)]
pub struct LocalPacManager {
    state: Mutex<Option<RunningPacServer>>,
}

impl PacManager for LocalPacManager {
    fn start(&self, config: PacStartConfig) -> Result<(), SystemProxyError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| SystemProxyError::LockPoisoned("pac manager"))?;

        // A server whose accept loop has exited leaves the OS pointing at a
        // closed port, so a finished thread must restart even on the same ports.
        let needs_restart = guard.as_ref().is_none_or(|running| {
            running.http_port != config.http_port
                || running.pac_port != config.pac_port
                || !running.is_alive()
        });
        if !needs_restart {
            return Ok(());
        }

        if let Some(mut running) = guard.take() {
            running.stop();
        }

        let content = pac_http_response(&config)?;
        let listener =
            TcpListener::bind((LOOPBACK, to_u16_port(config.pac_port)?)).map_err(|source| {
                SystemProxyError::PacListen {
                    port: config.pac_port,
                    source,
                }
            })?;
        listener
            .set_nonblocking(true)
            .map_err(SystemProxyError::PacSetNonblocking)?;

        let running = RunningPacServer::spawn(config.http_port, config.pac_port, listener, content);
        *guard = Some(running);

        Ok(())
    }

    fn stop(&self) {
        if let Ok(mut guard) = self.state.lock() {
            if let Some(mut running) = guard.take() {
                running.stop();
            }
        }
    }

    fn is_supported(&self) -> bool {
        cfg!(any(windows, target_os = "macos"))
    }
    fn is_running(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.as_ref().is_some_and(RunningPacServer::is_alive))
    }
}

#[derive(Debug)]
struct RunningPacServer {
    http_port: i32,
    pac_port: i32,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl RunningPacServer {
    fn spawn(http_port: i32, pac_port: i32, listener: TcpListener, content: Vec<u8>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        let thread = thread::spawn(move || {
            let mut reported_error = false;
            while thread_running.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => write_pac_response(stream, &content),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(PAC_ACCEPT_POLL_INTERVAL);
                    }
                    // A client that resets before it is accepted is routine
                    // (WSAECONNRESET on Windows, ECONNABORTED on macOS).
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::ConnectionReset
                                | io::ErrorKind::ConnectionAborted
                                | io::ErrorKind::Interrupted
                        ) => {}
                    Err(error) => {
                        // Never drop the listener while the OS still points at
                        // this PAC URL: every client would silently fall back to
                        // DIRECT while the app reports PAC mode as active.
                        if !reported_error {
                            reported_error = true;
                            tracing::warn!(
                                ?error,
                                port = pac_port,
                                "PAC listener accept failed; retrying"
                            );
                        }
                        thread::sleep(PAC_ACCEPT_ERROR_BACKOFF);
                    }
                }
            }
        });

        Self {
            http_port,
            pac_port,
            running,
            thread: Some(thread),
        }
    }

    /// Whether the accept loop is still running.
    fn is_alive(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for RunningPacServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[must_use]
pub const fn pac_available(target_os: TargetOs) -> bool {
    matches!(target_os, TargetOs::Windows | TargetOs::Macos)
}

pub fn plan_system_proxy(
    request: &SystemProxyRequest,
) -> Result<SystemProxyPlan, SystemProxyError> {
    if request.socks_port <= 0 {
        return Err(SystemProxyError::InvalidPort(request.socks_port));
    }
    if request.pac_port <= 0 {
        return Err(SystemProxyError::InvalidPort(request.pac_port));
    }

    let exception_entries = validated_proxy_exceptions(&request.item.system_proxy_exceptions)?;
    let normalized_exceptions = exceptions_to_csv(&exception_entries);
    let effective_type = effective_type(request.item.sys_proxy_type, request.force_disable);
    let mut status = SystemProxyStatus::from_request_with_exceptions(
        request,
        effective_type,
        normalized_exceptions.clone(),
    );
    if status.management == SystemProxyManagement::Manual {
        status.effective_type = SysProxyType::Unchanged;
        return Ok(SystemProxyPlan {
            action: SystemProxyAction::Noop,
            status,
        });
    }
    let action = match (effective_type, request.target_os) {
        (SysProxyType::ForcedChange, TargetOs::Windows) => {
            let settings = build_windows_proxy_settings_with_exceptions(
                &request.item,
                request.socks_port,
                &exception_entries,
            );
            status.proxy = Some(settings.proxy.clone());
            status.exceptions.clone_from(&settings.exceptions);
            SystemProxyAction::WindowsSetProxy(settings)
        }
        (SysProxyType::ForcedChange, TargetOs::Linux) => {
            let exceptions = normalized_exceptions.clone();
            SystemProxyAction::LinuxSet {
                script: linux_script_invocation(
                    request,
                    "manual",
                    Some((LOOPBACK, request.socks_port, &exceptions)),
                ),
                host: LOOPBACK.to_string(),
                port: request.socks_port,
                exceptions,
            }
        }
        (_, TargetOs::Macos) => SystemProxyAction::Noop,
        (SysProxyType::ForcedChange, TargetOs::Other) => {
            return Err(SystemProxyError::UnsupportedPlatform(TargetOs::Other));
        }
        (SysProxyType::ForcedClear, TargetOs::Windows) => SystemProxyAction::WindowsClear,
        (SysProxyType::ForcedClear, TargetOs::Linux) => SystemProxyAction::LinuxClear {
            script: linux_script_invocation(request, "none", None),
        },
        (SysProxyType::ForcedClear, TargetOs::Other) => {
            return Err(SystemProxyError::UnsupportedPlatform(TargetOs::Other));
        }
        (SysProxyType::Unchanged, _) => SystemProxyAction::Noop,
        (SysProxyType::Pac, TargetOs::Windows) => {
            let pac_url = pac_url(request);
            status.proxy = Some(pac_url.clone());
            status.pac_url = Some(pac_url.clone());
            status.exceptions.clear();
            SystemProxyAction::WindowsSetPac { pac_url }
        }
        (SysProxyType::Pac, TargetOs::Other) => {
            return Err(SystemProxyError::UnsupportedPlatform(TargetOs::Other));
        }
        (SysProxyType::Pac, _) => {
            status.effective_type = SysProxyType::Unchanged;
            SystemProxyAction::UnsupportedPac
        }
    };

    Ok(SystemProxyPlan { action, status })
}

fn pac_url(request: &SystemProxyRequest) -> String {
    format!(
        "http://{}:{}/pac?t={}",
        LOOPBACK, request.pac_port, request.pac_url_nonce
    )
}

pub fn build_windows_proxy_settings(
    item: &SystemProxyItem,
    port: i32,
) -> Result<WindowsProxySettings, SystemProxyError> {
    let exception_entries = validated_proxy_exceptions(&item.system_proxy_exceptions)?;
    Ok(build_windows_proxy_settings_with_exceptions(
        item,
        port,
        &exception_entries,
    ))
}

fn build_windows_proxy_settings_with_exceptions(
    item: &SystemProxyItem,
    port: i32,
    exception_entries: &[String],
) -> WindowsProxySettings {
    let exceptions = windows_exceptions(item, exception_entries);
    let proxy = if item.system_proxy_advanced_protocol.trim().is_empty() {
        format!("{LOOPBACK}:{port}")
    } else {
        item.system_proxy_advanced_protocol
            .replace("{ip}", LOOPBACK)
            .replace("{http_port}", &port.to_string())
            .replace("{socks_port}", &port.to_string())
    };

    WindowsProxySettings {
        proxy,
        exceptions,
        option_type: WindowsProxyOption::NamedProxy,
    }
}

fn effective_type(proxy_type: SysProxyType, force_disable: bool) -> SysProxyType {
    if force_disable && proxy_type != SysProxyType::Unchanged {
        SysProxyType::ForcedClear
    } else {
        proxy_type
    }
}

fn validated_proxy_exceptions(input: &str) -> Result<Vec<String>, SystemProxyError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    trimmed
        .split(',')
        .map(|raw| {
            let value = raw.trim();
            validate_proxy_exception(value)?;
            Ok(value.to_string())
        })
        .collect()
}

fn validate_proxy_exception(value: &str) -> Result<(), SystemProxyError> {
    if value.is_empty() {
        return Err(SystemProxyError::InvalidProxyException {
            value: value.to_string(),
            reason: "empty exception entry",
        });
    }
    if contains_forbidden_proxy_exception_char(value) {
        return Err(SystemProxyError::InvalidProxyException {
            value: value.to_string(),
            reason: "contains forbidden shell or gsettings metacharacters",
        });
    }
    if value.contains('/') {
        return validate_cidr_exception(value);
    }
    if value.parse::<IpAddr>().is_ok() || is_valid_hostname(value) {
        return Ok(());
    }

    Err(SystemProxyError::InvalidProxyException {
        value: value.to_string(),
        reason: "expected a hostname, IP address, or CIDR range",
    })
}

fn validate_cidr_exception(value: &str) -> Result<(), SystemProxyError> {
    let Some((ip, prefix)) = value.split_once('/') else {
        return Err(SystemProxyError::InvalidProxyException {
            value: value.to_string(),
            reason: "expected a CIDR range",
        });
    };
    if prefix.is_empty() || !prefix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SystemProxyError::InvalidProxyException {
            value: value.to_string(),
            reason: "expected a numeric CIDR prefix length",
        });
    }

    let Ok(ip) = ip.parse::<IpAddr>() else {
        return Err(SystemProxyError::InvalidProxyException {
            value: value.to_string(),
            reason: "expected a CIDR IP address",
        });
    };
    let Ok(prefix) = prefix.parse::<u8>() else {
        return Err(SystemProxyError::InvalidProxyException {
            value: value.to_string(),
            reason: "CIDR prefix length is out of range",
        });
    };
    let max_prefix = match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if prefix > max_prefix {
        return Err(SystemProxyError::InvalidProxyException {
            value: value.to_string(),
            reason: "CIDR prefix length is out of range",
        });
    }

    Ok(())
}

fn contains_forbidden_proxy_exception_char(value: &str) -> bool {
    value.bytes().any(|byte| {
        matches!(
            byte,
            b'\''
                | b'"'
                | b'`'
                | b'$'
                | b'\\'
                | b';'
                | b'|'
                | b'&'
                | b'('
                | b')'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'<'
                | b'>'
                | b'!'
                | b'*'
                | b'?'
                | b'~'
        ) || byte.is_ascii_whitespace()
            || !byte.is_ascii()
    })
}

fn is_valid_hostname(value: &str) -> bool {
    let hostname = if let Some(stripped) = value.strip_suffix('.') {
        stripped
    } else {
        value
    };
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }

    hostname.split('.').all(is_valid_hostname_label)
}

fn is_valid_hostname_label(label: &str) -> bool {
    if label.is_empty() || label.len() > 63 {
        return false;
    }

    let mut bytes = label.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }

    let mut last = first;
    for byte in bytes {
        if !(byte.is_ascii_alphanumeric() || byte == b'-') {
            return false;
        }
        last = byte;
    }

    last.is_ascii_alphanumeric()
}

fn exceptions_to_csv(entries: &[String]) -> String {
    entries.join(",")
}

fn windows_exceptions(item: &SystemProxyItem, exception_entries: &[String]) -> String {
    let exceptions = exception_entries.join(";");
    if item.not_proxy_local_address && exceptions.is_empty() {
        LOCAL_EXCEPTIONS.to_string()
    } else if item.not_proxy_local_address {
        format!("{LOCAL_EXCEPTIONS};{exceptions}")
    } else {
        exceptions
    }
}

mod backend;
use backend::*;
#[derive(Debug, Error)]
pub enum SystemProxyError {
    #[error("manual proxy runtime state is unavailable")]
    ManualState,
    #[error("could not open macOS Network settings")]
    OpenNetworkSettings,
    #[error("invalid system proxy port {0}")]
    InvalidPort(i32),
    #[error("system proxy is not supported on {0:?}")]
    UnsupportedPlatform(TargetOs),
    #[error("invalid system proxy exception {value:?}: {reason}")]
    InvalidProxyException { value: String, reason: &'static str },
    #[error("PAC mode is only supported on Windows or macOS, not {0:?}")]
    PacUnsupported(TargetOs),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("{context} failed with status {status_code:?}: {stderr}")]
    CommandFailed {
        context: &'static str,
        status_code: Option<i32>,
        stderr: String,
    },
    #[error("failed to listen for PAC requests on port {port}: {source}")]
    PacListen { port: i32, source: io::Error },
    #[error("failed to set PAC listener to nonblocking mode: {0}")]
    PacSetNonblocking(io::Error),
    #[error("failed to read PAC file {path}: {source}")]
    PacRead { path: PathBuf, source: io::Error },
    #[error("failed to write PAC file {path}: {source}")]
    PacWrite { path: PathBuf, source: io::Error },
    #[error("lock poisoned: {0}")]
    LockPoisoned(&'static str),
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use voya_core::DEFAULT_SYSTEM_PROXY_EXCEPTIONS;

    use crate::test_support::RecordingRunner;

    use super::*;

    #[derive(Default)]
    struct FakePacManager {
        starts: Mutex<Vec<PacStartConfig>>,
        stops: Mutex<u32>,
    }

    impl PacManager for FakePacManager {
        fn start(&self, config: PacStartConfig) -> Result<(), SystemProxyError> {
            self.starts.lock().expect("starts").push(config);
            Ok(())
        }

        fn stop(&self) {
            *self.stops.lock().expect("stops") += 1;
        }

        fn is_supported(&self) -> bool {
            true
        }
        fn is_running(&self) -> bool {
            true
        }
    }

    fn request(target_os: TargetOs, proxy_type: SysProxyType) -> SystemProxyRequest {
        SystemProxyRequest {
            target_os,
            item: SystemProxyItem {
                sys_proxy_type: proxy_type,
                system_proxy_exceptions: DEFAULT_SYSTEM_PROXY_EXCEPTIONS.to_string(),
                not_proxy_local_address: true,
                ..SystemProxyItem::default()
            },
            force_disable: false,
            socks_port: 10808,
            pac_port: 10811,
            config_dir: "/tmp/voya/config".into(),
            script_dir: "/tmp/voya/scripts".into(),
            pac_url_nonce: "123".to_string(),
        }
    }

    #[test]
    fn sysproxy_windows_advanced_template_uses_socks_port_and_local_exceptions() {
        let item = SystemProxyItem {
            system_proxy_exceptions: "localhost, 10.0.0.0/8".to_string(),
            not_proxy_local_address: true,
            system_proxy_advanced_protocol:
                "http={ip}:{http_port};https={ip}:{http_port};socks={ip}:{socks_port}".to_string(),
            ..SystemProxyItem::default()
        };

        let settings = build_windows_proxy_settings(&item, 2080).expect("windows settings");

        assert_eq!(
            settings.proxy,
            "http=127.0.0.1:2080;https=127.0.0.1:2080;socks=127.0.0.1:2080"
        );
        assert_eq!(settings.exceptions, "<local>;localhost;10.0.0.0/8");
    }

    #[test]
    fn sysproxy_rejects_unsafe_proxy_exceptions() {
        for value in [
            "localhost,'direct'",
            "localhost,$(id)",
            "localhost;example.com",
            "bad host",
            "*.example.com",
            "10.0.0.0/33",
            "example.com/24",
        ] {
            let mut request = request(TargetOs::Linux, SysProxyType::ForcedChange);
            request.item.system_proxy_exceptions = value.to_string();

            let error = plan_system_proxy(&request).expect_err("unsafe exception should fail");

            assert!(matches!(
                error,
                SystemProxyError::InvalidProxyException { .. }
            ));
        }
    }

    #[test]
    fn sysproxy_allows_hostname_ip_and_cidr_exceptions() {
        let mut request = request(TargetOs::Linux, SysProxyType::ForcedChange);
        request.item.system_proxy_exceptions =
            "localhost,example.internal,127.0.0.1,10.0.0.0/8,::1,fd00::/8".to_string();

        let plan = plan_system_proxy(&request).expect("valid exceptions");

        let SystemProxyAction::LinuxSet {
            exceptions, script, ..
        } = plan.action
        else {
            panic!("expected linux set");
        };
        assert_eq!(
            exceptions,
            "localhost,example.internal,127.0.0.1,10.0.0.0/8,::1,fd00::/8"
        );
        assert_eq!(
            script.arguments,
            [
                "manual",
                LOOPBACK,
                "10808",
                "localhost,example.internal,127.0.0.1,10.0.0.0/8,::1,fd00::/8"
            ]
        );
    }

    #[test]
    fn sysproxy_other_platform_forced_modes_are_errors() {
        for proxy_type in [SysProxyType::ForcedChange, SysProxyType::ForcedClear] {
            let error = plan_system_proxy(&request(TargetOs::Other, proxy_type))
                .expect_err("unsupported platform should fail");

            assert!(matches!(
                error,
                SystemProxyError::UnsupportedPlatform(TargetOs::Other)
            ));
        }

        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(FakePacManager::default());
        let service = SystemProxyService::new(runner.clone(), pac);
        let error = service
            .apply(&request(TargetOs::Other, SysProxyType::ForcedChange))
            .expect_err("unsupported platform apply should fail");

        assert!(matches!(
            error,
            SystemProxyError::UnsupportedPlatform(TargetOs::Other)
        ));
        assert!(runner.oneshots().is_empty());
    }

    #[test]
    fn sysproxy_force_disable_clears_forced_modes_but_preserves_unchanged() {
        let mut forced = request(TargetOs::Windows, SysProxyType::ForcedChange);
        forced.force_disable = true;
        let forced_plan = plan_system_proxy(&forced).expect("forced plan");
        assert_eq!(forced_plan.status.effective_type, SysProxyType::ForcedClear);
        assert!(matches!(
            forced_plan.action,
            SystemProxyAction::WindowsClear
        ));

        let mut unchanged = request(TargetOs::Windows, SysProxyType::Unchanged);
        unchanged.force_disable = true;
        let unchanged_plan = plan_system_proxy(&unchanged).expect("unchanged plan");
        assert_eq!(
            unchanged_plan.status.effective_type,
            SysProxyType::Unchanged
        );
        assert!(matches!(unchanged_plan.action, SystemProxyAction::Noop));
    }

    #[test]
    fn sysproxy_pac_availability_matches_supported_platforms() {
        assert!(pac_available(TargetOs::Windows));
        assert!(pac_available(TargetOs::Macos));
        assert!(!pac_available(TargetOs::Linux));
        assert!(!pac_available(TargetOs::Other));
    }

    #[test]
    fn sysproxy_pac_is_windows_and_macos_only_and_stops_when_switching_away() {
        let linux_pac = plan_system_proxy(&request(TargetOs::Linux, SysProxyType::Pac))
            .expect("linux pac plan");
        assert_eq!(linux_pac.status.effective_type, SysProxyType::Unchanged);
        assert!(matches!(
            linux_pac.action,
            SystemProxyAction::UnsupportedPac
        ));

        let macos_pac =
            plan_system_proxy(&request(TargetOs::Macos, SysProxyType::Pac)).expect("manual plan");
        assert_eq!(macos_pac.status.effective_type, SysProxyType::Unchanged);
        assert_eq!(macos_pac.status.pac_url, None);
        assert!(matches!(macos_pac.action, SystemProxyAction::Noop));

        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(FakePacManager::default());
        let service = SystemProxyService::new(runner, pac.clone());
        service
            .apply(&request(TargetOs::Macos, SysProxyType::ForcedClear))
            .expect("macos clear");
        service
            .apply(&request(TargetOs::Macos, SysProxyType::ForcedChange))
            .expect("macos set");
        service
            .apply(&request(TargetOs::Macos, SysProxyType::Unchanged))
            .expect("macos unchanged");
        assert_eq!(*pac.stops.lock().expect("stops"), 3);
    }

    #[test]
    fn sysproxy_linux_script_arguments_match_reference_shape() {
        let linux = plan_system_proxy(&request(TargetOs::Linux, SysProxyType::ForcedChange))
            .expect("linux plan");
        let SystemProxyAction::LinuxSet { script, .. } = linux.action else {
            panic!("expected linux set");
        };
        assert_eq!(
            script.arguments,
            ["manual", LOOPBACK, "10808", DEFAULT_SYSTEM_PROXY_EXCEPTIONS]
        );
    }

    #[test]
    fn sysproxy_managed_scripts_are_generated_even_when_existing_file_is_present() {
        let root = unique_temp_root("sysproxy-managed-script");
        let script_dir = root.join("guiTemps").join("sysproxy");
        fs::create_dir_all(&script_dir).expect("create script directory");
        let script_path = script_dir.join(LINUX_PROXY_SCRIPT_NAME);
        fs::write(&script_path, "stale").expect("write stale script");

        let mut request = request(TargetOs::Linux, SysProxyType::ForcedChange);
        request.script_dir = script_dir.clone();
        let plan = plan_system_proxy(&request).expect("linux plan");
        let SystemProxyAction::LinuxSet { script, .. } = plan.action else {
            panic!("expected linux set");
        };
        let generated = script.generated_script.expect("managed script");

        assert_eq!(script.executable, script_path);
        assert_eq!(generated.directory, script_dir);
        assert_eq!(generated.path, script.executable);
        assert_eq!(generated.contents, LINUX_PROXY_SCRIPT);
        assert!(generated.executable);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sysproxy_custom_script_path_is_not_rewritten_as_managed_script() {
        let root = unique_temp_root("sysproxy-custom-script");
        fs::create_dir_all(&root).expect("create script directory");
        let custom_script = root.join("custom.sh");
        fs::write(&custom_script, "#!/bin/sh\n").expect("write custom script");

        let mut request = request(TargetOs::Linux, SysProxyType::ForcedChange);
        request.item.custom_system_proxy_script_path =
            Some(custom_script.to_string_lossy().into_owned());
        let plan = plan_system_proxy(&request).expect("linux plan");
        let SystemProxyAction::LinuxSet { script, .. } = plan.action else {
            panic!("expected linux set");
        };

        assert_eq!(script.executable, custom_script);
        assert!(script.generated_script.is_none());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sysproxy_service_starts_windows_pac_and_sets_autoconfig_url() {
        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(FakePacManager::default());
        let service = SystemProxyService::new(runner.clone(), pac.clone());

        let status = service
            .apply(&request(TargetOs::Windows, SysProxyType::Pac))
            .expect("pac");

        assert_eq!(status.effective_type, SysProxyType::Pac);
        assert_eq!(
            status.pac_url.as_deref(),
            Some("http://127.0.0.1:10811/pac?t=123")
        );
        assert_eq!(pac.starts.lock().expect("starts").len(), 1);
        assert!(runner
            .oneshots()
            .iter()
            .any(|spawn| spawn.arguments.iter().any(|arg| arg == "AutoConfigURL")));
    }

    #[test]
    fn sysproxy_pac_manager_respawns_a_server_whose_accept_loop_stopped() {
        let root = unique_temp_root("pac-respawn");
        fs::create_dir_all(&root).expect("create pac config directory");
        let manager = LocalPacManager::default();
        let config = PacStartConfig {
            http_port: 10808,
            pac_port: free_local_port(),
            config_dir: root.clone(),
            custom_pac_path: None,
        };

        manager.start(config.clone()).expect("start pac server");
        assert!(fetch_pac(config.pac_port).contains("FindProxyForURL"));

        {
            let mut guard = manager.state.lock().expect("pac state");
            let running = guard.as_mut().expect("running pac server");
            running.running.store(false, Ordering::Relaxed);
            if let Some(thread) = running.thread.take() {
                let _ = thread.join();
            }
        }

        manager.start(config.clone()).expect("restart pac server");

        assert!(
            fetch_pac(config.pac_port).contains("FindProxyForURL"),
            "a stopped accept loop must be respawned instead of leaving the PAC url dead"
        );

        manager.stop();
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn sysproxy_pac_server_keeps_serving_after_a_client_reset() {
        let root = unique_temp_root("pac-reset");
        fs::create_dir_all(&root).expect("create pac config directory");
        let manager = LocalPacManager::default();
        let config = PacStartConfig {
            http_port: 10808,
            pac_port: free_local_port(),
            config_dir: root.clone(),
            custom_pac_path: None,
        };
        manager.start(config.clone()).expect("start pac server");

        reset_client_connection(config.pac_port);

        assert!(
            fetch_pac(config.pac_port).contains("FindProxyForURL"),
            "an aborted connection must not take the PAC listener down"
        );

        manager.stop();
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    fn reset_client_connection(port: i32) {
        use std::os::fd::AsRawFd;

        let stream = TcpStream::connect((LOOPBACK, u16::try_from(port).expect("pac port")))
            .expect("connect to pac server");
        let linger = libc::linger {
            l_onoff: 1,
            l_linger: 0,
        };
        // SAFETY: `setsockopt` reads `size_of::<libc::linger>()` bytes from the
        // pointer, which points at the live local `linger` value, and `stream`
        // owns the descriptor for the whole call.
        let result = unsafe {
            libc::setsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_LINGER,
                std::ptr::addr_of!(linger).cast(),
                size_of::<libc::linger>() as libc::socklen_t,
            )
        };

        assert_eq!(result, 0, "failed to arm SO_LINGER for the aborted client");
        drop(stream);
    }

    fn fetch_pac(port: i32) -> String {
        use std::io::Read;

        let mut stream = TcpStream::connect((LOOPBACK, u16::try_from(port).expect("pac port")))
            .expect("connect to pac server");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        stream
            .write_all(b"GET /pac HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
            .expect("send pac request");
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        response
    }

    fn free_local_port() -> i32 {
        let listener = TcpListener::bind((LOOPBACK, 0)).expect("bind an ephemeral port");
        let port = listener.local_addr().expect("local address").port();
        drop(listener);
        i32::from(port)
    }

    fn unique_temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "voyavpn-sysproxy-{name}-{}-{}",
            std::process::id(),
            monotonic_nanos()
        ))
    }

    fn monotonic_nanos() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    }
}
