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
const LINUX_PROXY_SCRIPT_NAME: &str = "proxy_set_linux.sh";
const MACOS_PROXY_SCRIPT_NAME: &str = "proxy_set_osx.sh";

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
    pub requested_type: SysProxyType,
    pub effective_type: SysProxyType,
    pub target_os: TargetOs,
    pub pac_available: bool,
    pub proxy: Option<String>,
    pub exceptions: String,
    pub pac_url: Option<String>,
}

impl SystemProxyStatus {
    pub fn from_request(
        request: &SystemProxyRequest,
        effective_type: SysProxyType,
    ) -> Result<Self, SystemProxyError> {
        let exceptions = validated_proxy_exceptions(&request.item.system_proxy_exceptions)?;
        Ok(Self::from_request_with_exceptions(
            request,
            effective_type,
            exceptions_to_csv(&exceptions),
        ))
    }

    fn from_request_with_exceptions(
        request: &SystemProxyRequest,
        effective_type: SysProxyType,
        exceptions: String,
    ) -> Self {
        Self {
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
    MacosSet {
        script: ScriptInvocation,
        host: String,
        port: i32,
        exceptions: String,
    },
    MacosClear {
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
    runner: Arc<dyn ProcessRunner>,
    pac_manager: Arc<dyn PacManager>,
}

impl SystemProxyService {
    #[must_use]
    pub fn new(runner: Arc<dyn ProcessRunner>, pac_manager: Arc<dyn PacManager>) -> Self {
        Self {
            runner,
            pac_manager,
        }
    }

    pub fn apply(
        &self,
        request: &SystemProxyRequest,
    ) -> Result<SystemProxyStatus, SystemProxyError> {
        let plan = plan_system_proxy(request)?;

        if request.target_os == TargetOs::Windows && plan.status.effective_type != SysProxyType::Pac
        {
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
            | SystemProxyAction::LinuxClear { script }
            | SystemProxyAction::MacosSet { script, .. }
            | SystemProxyAction::MacosClear { script } => {
                run_script(&*self.runner, script)?;
            }
        }

        Ok(plan.status)
    }

    pub fn stop_pac(&self) {
        self.pac_manager.stop();
    }
}

#[must_use]
pub fn platform_pac_manager() -> Arc<dyn PacManager> {
    #[cfg(windows)]
    {
        Arc::new(WindowsPacManager::default())
    }
    #[cfg(not(windows))]
    {
        Arc::new(UnsupportedPacManager)
    }
}

pub trait PacManager: Send + Sync {
    fn start(&self, config: PacStartConfig) -> Result<(), SystemProxyError>;
    fn stop(&self);
    fn is_supported(&self) -> bool;
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
}

#[derive(Debug, Default)]
pub struct WindowsPacManager {
    state: Mutex<Option<RunningPacServer>>,
}

impl PacManager for WindowsPacManager {
    fn start(&self, config: PacStartConfig) -> Result<(), SystemProxyError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| SystemProxyError::LockPoisoned("pac manager"))?;

        let needs_restart = guard.as_ref().is_none_or(|running| {
            running.http_port != config.http_port || running.pac_port != config.pac_port
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
        cfg!(windows)
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
            while thread_running.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => write_pac_response(stream, &content),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
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
    matches!(target_os, TargetOs::Windows)
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
        (SysProxyType::ForcedChange, TargetOs::Macos) => {
            let exceptions = normalized_exceptions.clone();
            SystemProxyAction::MacosSet {
                script: macos_script_invocation(
                    request,
                    "set",
                    Some((LOOPBACK, request.socks_port, &exceptions)),
                ),
                host: LOOPBACK.to_string(),
                port: request.socks_port,
                exceptions,
            }
        }
        (SysProxyType::ForcedChange, TargetOs::Other) => {
            return Err(SystemProxyError::UnsupportedPlatform(TargetOs::Other));
        }
        (SysProxyType::ForcedClear, TargetOs::Windows) => SystemProxyAction::WindowsClear,
        (SysProxyType::ForcedClear, TargetOs::Linux) => SystemProxyAction::LinuxClear {
            script: linux_script_invocation(request, "none", None),
        },
        (SysProxyType::ForcedClear, TargetOs::Macos) => SystemProxyAction::MacosClear {
            script: macos_script_invocation(request, "clear", None),
        },
        (SysProxyType::ForcedClear, TargetOs::Other) => {
            return Err(SystemProxyError::UnsupportedPlatform(TargetOs::Other));
        }
        (SysProxyType::Unchanged, _) => SystemProxyAction::Noop,
        (SysProxyType::Pac, TargetOs::Windows) => {
            let pac_url = format!(
                "http://{}:{}/pac?t={}",
                LOOPBACK, request.pac_port, request.pac_url_nonce
            );
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

fn custom_script_path(item: &SystemProxyItem) -> Option<PathBuf> {
    item.custom_system_proxy_script_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .filter(|path| path.exists())
}

fn linux_script_invocation(
    request: &SystemProxyRequest,
    mode: &str,
    manual: Option<(&str, i32, &str)>,
) -> ScriptInvocation {
    let (executable, generated_script) =
        if let Some(custom_script) = custom_script_path(&request.item) {
            (custom_script, None)
        } else {
            let executable = request.script_dir.join(LINUX_PROXY_SCRIPT_NAME);
            (
                executable.clone(),
                Some(GeneratedScript::new(
                    request.script_dir.clone(),
                    executable,
                    LINUX_PROXY_SCRIPT,
                    true,
                )),
            )
        };
    let mut arguments = vec![mode.to_string()];
    if let Some((host, port, exceptions)) = manual {
        arguments.push(host.to_string());
        arguments.push(port.to_string());
        arguments.push(exceptions.to_string());
    }

    ScriptInvocation {
        executable,
        arguments,
        generated_script,
    }
}

fn macos_script_invocation(
    request: &SystemProxyRequest,
    mode: &str,
    manual: Option<(&str, i32, &str)>,
) -> ScriptInvocation {
    let (executable, generated_script) =
        if let Some(custom_script) = custom_script_path(&request.item) {
            (custom_script, None)
        } else {
            let executable = request.script_dir.join(MACOS_PROXY_SCRIPT_NAME);
            (
                executable.clone(),
                Some(GeneratedScript::new(
                    request.script_dir.clone(),
                    executable,
                    MACOS_PROXY_SCRIPT,
                    true,
                )),
            )
        };
    let mut arguments = vec![mode.to_string()];
    if let Some((host, port, exceptions)) = manual {
        arguments.push(host.to_string());
        arguments.push(port.to_string());
        arguments.extend(
            exceptions
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
        );
    }

    ScriptInvocation {
        executable,
        arguments,
        generated_script,
    }
}

fn run_script(
    runner: &dyn ProcessRunner,
    script: &ScriptInvocation,
) -> Result<(), SystemProxyError> {
    let mut spawn = ProcessSpawn::new(ProcessRole::SysProxy, &script.executable)
        .with_arguments(script.arguments.clone());
    if let Some(generated_script) = script.generated_script.clone() {
        spawn = spawn.with_generated_script(generated_script);
    }
    ensure_success(runner.run_oneshot(spawn)?, "system proxy script")
}

fn apply_windows_clear(runner: &dyn ProcessRunner) -> Result<(), SystemProxyError> {
    apply_windows_proxy(
        runner,
        &WindowsProxySettings {
            proxy: String::new(),
            exceptions: String::new(),
            option_type: WindowsProxyOption::Direct,
        },
    )
}

fn apply_windows_proxy(
    runner: &dyn ProcessRunner,
    settings: &WindowsProxySettings,
) -> Result<(), SystemProxyError> {
    for command in windows_registry_commands(settings) {
        ensure_success(
            runner.run_oneshot(command)?,
            "windows registry proxy command",
        )?;
    }
    refresh_windows_internet_settings();
    Ok(())
}

fn windows_registry_commands(settings: &WindowsProxySettings) -> Vec<ProcessSpawn> {
    match settings.option_type {
        WindowsProxyOption::Direct => vec![
            registry_set_dword("ProxyEnable", 0),
            registry_set_string("ProxyServer", ""),
            registry_set_string("ProxyOverride", ""),
            registry_set_string("AutoConfigURL", ""),
        ],
        WindowsProxyOption::NamedProxy => vec![
            registry_set_dword("ProxyEnable", 1),
            registry_set_string("ProxyServer", &settings.proxy),
            registry_set_string("ProxyOverride", &settings.exceptions),
            registry_set_string("AutoConfigURL", ""),
        ],
        WindowsProxyOption::PacUrl => vec![
            registry_set_dword("ProxyEnable", 0),
            registry_set_string("ProxyServer", ""),
            registry_set_string("ProxyOverride", ""),
            registry_set_string("AutoConfigURL", &settings.proxy),
        ],
    }
}

fn registry_set_dword(name: &str, value: u32) -> ProcessSpawn {
    registry_set(name, "REG_DWORD", &value.to_string())
}

fn registry_set_string(name: &str, value: &str) -> ProcessSpawn {
    registry_set(name, "REG_SZ", value)
}

fn registry_set(name: &str, value_type: &str, value: &str) -> ProcessSpawn {
    ProcessSpawn::new(ProcessRole::SysProxy, "reg").with_arguments([
        "add".to_string(),
        WINDOWS_INTERNET_SETTINGS_REG_PATH.to_string(),
        "/v".to_string(),
        name.to_string(),
        "/t".to_string(),
        value_type.to_string(),
        "/d".to_string(),
        value.to_string(),
        "/f".to_string(),
    ])
}

#[cfg(windows)]
fn refresh_windows_internet_settings() {
    use std::ffi::c_void;

    const INTERNET_OPTION_REFRESH: u32 = 37;
    const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;

    extern "system" {
        fn InternetSetOptionW(
            internet: *mut c_void,
            option: u32,
            buffer: *mut c_void,
            buffer_length: u32,
        ) -> i32;
    }

    unsafe {
        let _ = InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        let _ = InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
    }
}

#[cfg(not(windows))]
fn refresh_windows_internet_settings() {}

fn ensure_success(output: ProcessOutput, context: &'static str) -> Result<(), SystemProxyError> {
    if output.status_code == Some(0) {
        Ok(())
    } else {
        Err(SystemProxyError::CommandFailed {
            context,
            status_code: output.status_code,
            stderr: output.stderr,
        })
    }
}

fn pac_http_response(config: &PacStartConfig) -> Result<Vec<u8>, SystemProxyError> {
    let pac_text = load_pac_text(config)?.replace(
        "__PROXY__",
        &format!("PROXY {LOOPBACK}:{};DIRECT;", config.http_port),
    );
    let mut response = String::new();
    response.push_str("HTTP/1.0 200 OK\r\n");
    response.push_str("Content-type:application/x-ns-proxy-autoconfig\r\n");
    response.push_str("Connection:close\r\n");
    response.push_str(&format!("Content-Length:{}\r\n", pac_text.len()));
    response.push_str("\r\n");
    response.push_str(&pac_text);

    Ok(response.into_bytes())
}

fn load_pac_text(config: &PacStartConfig) -> Result<String, SystemProxyError> {
    if let Some(custom) = config
        .custom_pac_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .filter(|path| path.exists())
    {
        return fs::read_to_string(&custom).map_err(|source| SystemProxyError::PacRead {
            path: custom,
            source,
        });
    }

    fs::create_dir_all(&config.config_dir).map_err(|source| SystemProxyError::PacWrite {
        path: config.config_dir.clone(),
        source,
    })?;
    let path = config.config_dir.join(PAC_FILE_NAME);
    if !path.exists() {
        fs::write(&path, DEFAULT_PAC_TEMPLATE).map_err(|source| SystemProxyError::PacWrite {
            path: path.clone(),
            source,
        })?;
    }

    fs::read_to_string(&path).map_err(|source| SystemProxyError::PacRead { path, source })
}

fn write_pac_response(mut stream: TcpStream, content: &[u8]) {
    let _ = stream.write_all(content);
    let _ = stream.flush();
}

fn to_u16_port(port: i32) -> Result<u16, SystemProxyError> {
    u16::try_from(port).map_err(|_| SystemProxyError::InvalidPort(port))
}

const LINUX_PROXY_SCRIPT: &str = r#"#!/bin/sh
mode="$1"
host="$2"
port="$3"
ignore_hosts="$4"

array_from_csv() {
  if [ -z "$1" ]; then
    printf "[]"
    return
  fi
  old_ifs="$IFS"
  IFS=","
  result=""
  for value in $1; do
    trimmed="$(printf "%s" "$value" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
    if [ -n "$trimmed" ]; then
      if [ -n "$result" ]; then
        result="$result,"
      fi
      result="$result'$trimmed'"
    fi
  done
  IFS="$old_ifs"
  printf "[%s]" "$result"
}

set_gnome() {
  if ! command -v gsettings >/dev/null 2>&1; then
    return
  fi
  gsettings set org.gnome.system.proxy mode "$mode"
  if [ "$mode" = "manual" ]; then
    for proto in http https ftp socks; do
      gsettings set "org.gnome.system.proxy.$proto" host "$host"
      gsettings set "org.gnome.system.proxy.$proto" port "$port"
    done
    gsettings set org.gnome.system.proxy ignore-hosts "$(array_from_csv "$ignore_hosts")"
  fi
}

set_kde() {
  if command -v kwriteconfig6 >/dev/null 2>&1; then
    kwriteconfig=kwriteconfig6
  elif command -v kwriteconfig5 >/dev/null 2>&1; then
    kwriteconfig=kwriteconfig5
  else
    return
  fi
  if [ "$mode" = "manual" ]; then
    "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key ProxyType 1
    "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key httpProxy "http://$host:$port"
    "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key httpsProxy "http://$host:$port"
    "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key ftpProxy "http://$host:$port"
    "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key socksProxy "http://$host:$port"
    "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key NoProxyFor "$ignore_hosts"
  else
    "$kwriteconfig" --file kioslaverc --group "Proxy Settings" --key ProxyType 0
  fi
  dbus-send --type=signal /KIO/Scheduler org.kde.KIO.Scheduler.reparseSlaveConfiguration string:"" >/dev/null 2>&1 || true
}

if [ "$mode" != "manual" ] && [ "$mode" != "none" ]; then
  echo "Usage: $0 manual <host> <port> <ignore_hosts> | none" >&2
  exit 1
fi

set_gnome
set_kde
"#;

const MACOS_PROXY_SCRIPT: &str = r#"#!/bin/sh
mode="$1"
host="$2"
port="$3"
shift 3 2>/dev/null || true

services="$(networksetup -listallnetworkservices | grep -v '^\*')"
printf "%s\n" "$services" | while IFS= read -r service; do
  [ -z "$service" ] && continue
  if [ "$mode" = "set" ]; then
    networksetup -setwebproxy "$service" "$host" "$port"
    networksetup -setsecurewebproxy "$service" "$host" "$port"
    networksetup -setsocksfirewallproxy "$service" "$host" "$port"
    networksetup -setproxybypassdomains "$service" "$@"
  elif [ "$mode" = "clear" ]; then
    networksetup -setwebproxystate "$service" off
    networksetup -setsecurewebproxystate "$service" off
    networksetup -setsocksfirewallproxystate "$service" off
  else
    echo "Usage: $0 set <host> <port> [bypass...] | clear" >&2
    exit 1
  fi
done
"#;

#[derive(Debug, Error)]
pub enum SystemProxyError {
    #[error("invalid system proxy port {0}")]
    InvalidPort(i32),
    #[error("system proxy is not supported on {0:?}")]
    UnsupportedPlatform(TargetOs),
    #[error("invalid system proxy exception {value:?}: {reason}")]
    InvalidProxyException { value: String, reason: &'static str },
    #[error("PAC mode is only supported on Windows, not {0:?}")]
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
    use std::sync::{Mutex, MutexGuard};

    use voya_core::DEFAULT_SYSTEM_PROXY_EXCEPTIONS;

    use super::*;

    #[derive(Default)]
    struct RecordingRunner {
        spawns: Mutex<Vec<ProcessSpawn>>,
    }

    impl RecordingRunner {
        fn lock(&self) -> MutexGuard<'_, Vec<ProcessSpawn>> {
            self.spawns.lock().expect("spawns")
        }
    }

    impl ProcessRunner for RecordingRunner {
        fn spawn(
            &self,
            _request: ProcessSpawn,
        ) -> Result<crate::process::ProcessHandle, ProcessError> {
            unreachable!("sysproxy tests only use oneshot commands")
        }

        fn run_oneshot(&self, request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
            self.spawns.lock().expect("spawns").push(request);
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn stop(&self, _handle: &crate::process::ProcessHandle) -> Result<(), ProcessError> {
            unreachable!("sysproxy tests only use oneshot commands")
        }
    }

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
        assert!(runner.lock().is_empty());
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
    fn sysproxy_pac_is_windows_only_and_stops_when_switching_away() {
        let linux_pac = plan_system_proxy(&request(TargetOs::Linux, SysProxyType::Pac))
            .expect("linux pac plan");
        assert_eq!(linux_pac.status.effective_type, SysProxyType::Unchanged);
        assert!(matches!(
            linux_pac.action,
            SystemProxyAction::UnsupportedPac
        ));

        let runner = Arc::new(RecordingRunner::default());
        let pac = Arc::new(FakePacManager::default());
        let service = SystemProxyService::new(runner, pac.clone());
        service
            .apply(&request(TargetOs::Windows, SysProxyType::ForcedClear))
            .expect("clear");
        assert_eq!(*pac.stops.lock().expect("stops"), 1);
    }

    #[test]
    fn sysproxy_linux_and_macos_script_arguments_match_reference_shape() {
        let linux = plan_system_proxy(&request(TargetOs::Linux, SysProxyType::ForcedChange))
            .expect("linux plan");
        let SystemProxyAction::LinuxSet { script, .. } = linux.action else {
            panic!("expected linux set");
        };
        assert_eq!(
            script.arguments,
            ["manual", LOOPBACK, "10808", DEFAULT_SYSTEM_PROXY_EXCEPTIONS]
        );

        let macos = plan_system_proxy(&request(TargetOs::Macos, SysProxyType::ForcedChange))
            .expect("macos plan");
        let SystemProxyAction::MacosSet { script, .. } = macos.action else {
            panic!("expected macos set");
        };
        assert_eq!(
            script.arguments,
            ["set", LOOPBACK, "10808", "localhost", "127.0.0.0/8", "::1"]
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
            .lock()
            .iter()
            .any(|spawn| spawn.arguments.iter().any(|arg| arg == "AutoConfigURL")));
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
