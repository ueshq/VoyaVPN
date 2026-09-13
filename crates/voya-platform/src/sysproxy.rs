use std::{net::IpAddr, path::PathBuf, sync::Arc};

use thiserror::Error;
use voya_core::{SysProxyType, SystemProxyItem};

use crate::{
    coreinfo::TargetOs,
    process::{
        GeneratedScript, ProcessError, ProcessOutput, ProcessRole, ProcessRunner, ProcessSpawn,
    },
};

pub const LOOPBACK: &str = "127.0.0.1";

const LOCAL_EXCEPTIONS: &str = "<local>";
const WINDOWS_INTERNET_SETTINGS_REG_PATH: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
const LINUX_PROXY_SCRIPT_NAME: &str = "proxy_set_linux.sh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemProxyManagement {
    Automatic,
    Unsupported,
}

/// Whether the app drives the OS proxy on this platform. macOS captures traffic
/// only through its PacketTunnel VPN, so it has no system-proxy mode at all.
#[must_use]
pub const fn system_proxy_management(os: TargetOs) -> SystemProxyManagement {
    match os {
        TargetOs::Windows | TargetOs::Linux => SystemProxyManagement::Automatic,
        TargetOs::Macos | TargetOs::Other => SystemProxyManagement::Unsupported,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProxyRequest {
    pub target_os: TargetOs,
    pub item: SystemProxyItem,
    pub force_disable: bool,
    pub socks_port: i32,
    pub script_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProxyStatus {
    pub management: SystemProxyManagement,
    pub requested_type: SysProxyType,
    pub effective_type: SysProxyType,
    pub target_os: TargetOs,
    pub proxy: Option<String>,
    pub exceptions: String,
}

impl SystemProxyStatus {
    fn from_request_with_exceptions(
        request: &SystemProxyRequest,
        effective_type: SysProxyType,
        exceptions: String,
    ) -> Self {
        Self {
            management: system_proxy_management(request.target_os),
            requested_type: request.item.sys_proxy_type,
            effective_type,
            target_os: request.target_os,
            proxy: None,
            exceptions,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemProxyAction {
    Noop,
    WindowsSetProxy(WindowsProxySettings),
    WindowsClear,
    LinuxSet {
        script: ScriptInvocation,
        host: String,
        port: i32,
        exceptions: String,
    },
    LinuxClear {
        script: ScriptInvocation,
    },
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
}

impl SystemProxyService {
    #[must_use]
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }

    pub fn apply(
        &self,
        request: &SystemProxyRequest,
    ) -> Result<SystemProxyStatus, SystemProxyError> {
        let plan = plan_system_proxy(request)?;

        match &plan.action {
            SystemProxyAction::Noop => {}
            SystemProxyAction::WindowsSetProxy(settings) => {
                apply_windows_proxy(&*self.runner, settings)?;
            }
            SystemProxyAction::WindowsClear => {
                apply_windows_clear(&*self.runner)?;
            }
            SystemProxyAction::LinuxSet { script, .. }
            | SystemProxyAction::LinuxClear { script } => {
                run_script(&*self.runner, script)?;
            }
        }

        Ok(plan.status)
    }

    /// The plan for a request, without touching the OS.
    pub fn status(
        &self,
        request: &SystemProxyRequest,
    ) -> Result<SystemProxyStatus, SystemProxyError> {
        plan_system_proxy(request).map(|plan| plan.status)
    }
}

pub fn plan_system_proxy(
    request: &SystemProxyRequest,
) -> Result<SystemProxyPlan, SystemProxyError> {
    if request.socks_port <= 0 {
        return Err(SystemProxyError::InvalidPort(request.socks_port));
    }

    let exception_entries = validated_proxy_exceptions(&request.item.system_proxy_exceptions)?;
    let normalized_exceptions = exceptions_to_csv(&exception_entries);
    let effective_type = effective_type(request.item.sys_proxy_type, request.force_disable);
    let mut status = SystemProxyStatus::from_request_with_exceptions(
        request,
        effective_type,
        normalized_exceptions.clone(),
    );
    if request.target_os == TargetOs::Macos {
        // No mode to apply: the PacketTunnel VPN is the only capture path.
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
    };

    Ok(SystemProxyPlan { action, status })
}

fn build_windows_proxy_settings_with_exceptions(
    item: &SystemProxyItem,
    port: i32,
    exception_entries: &[String],
) -> WindowsProxySettings {
    WindowsProxySettings {
        proxy: format!("{LOOPBACK}:{port}"),
        exceptions: windows_exceptions(item, exception_entries),
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
    #[error("invalid system proxy port {0}")]
    InvalidPort(i32),
    #[error("system proxy is not supported on {0:?}")]
    UnsupportedPlatform(TargetOs),
    #[error("invalid system proxy exception {value:?}: {reason}")]
    InvalidProxyException { value: String, reason: &'static str },
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("{context} failed with status {status_code:?}: {stderr}")]
    CommandFailed {
        context: &'static str,
        status_code: Option<i32>,
        stderr: String,
    },
    #[error("lock poisoned: {0}")]
    LockPoisoned(&'static str),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use voya_core::DEFAULT_SYSTEM_PROXY_EXCEPTIONS;

    use crate::test_support::RecordingRunner;

    use super::*;

    fn request(target_os: TargetOs, proxy_type: SysProxyType) -> SystemProxyRequest {
        SystemProxyRequest {
            target_os,
            item: SystemProxyItem {
                sys_proxy_type: proxy_type,
                system_proxy_exceptions: DEFAULT_SYSTEM_PROXY_EXCEPTIONS.to_string(),
                not_proxy_local_address: true,
            },
            force_disable: false,
            socks_port: 10808,
            script_dir: "/tmp/voya/scripts".into(),
        }
    }

    #[test]
    fn sysproxy_windows_proxy_uses_socks_port_and_local_exceptions() {
        let item = SystemProxyItem {
            system_proxy_exceptions: "localhost, 10.0.0.0/8".to_string(),
            not_proxy_local_address: true,
            ..SystemProxyItem::default()
        };

        let mut request = request(TargetOs::Windows, SysProxyType::ForcedChange);
        request.item = item;
        request.item.sys_proxy_type = SysProxyType::ForcedChange;
        request.socks_port = 2080;
        let plan = plan_system_proxy(&request).expect("windows proxy plan");
        let SystemProxyAction::WindowsSetProxy(settings) = plan.action else {
            panic!("expected Windows proxy settings");
        };

        assert_eq!(settings.proxy, "127.0.0.1:2080");
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
        let service = SystemProxyService::new(runner.clone());
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
