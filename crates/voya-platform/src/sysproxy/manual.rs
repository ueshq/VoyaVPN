use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemProxyManagement {
    Automatic,
    Manual,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemProxyObservation {
    Unknown,
    Clear,
    LocalProxy,
    OtherProxy,
}

#[must_use]
pub const fn system_proxy_management(os: TargetOs) -> SystemProxyManagement {
    match os {
        TargetOs::Macos => SystemProxyManagement::Manual,
        TargetOs::Windows | TargetOs::Linux => SystemProxyManagement::Automatic,
        TargetOs::Other => SystemProxyManagement::Unsupported,
    }
}

pub trait SystemProxyObserver: Send + Sync {
    fn observe(&self) -> SystemProxyObservation;
}

pub struct PlatformSystemProxyObserver;

impl SystemProxyObserver for PlatformSystemProxyObserver {
    fn observe(&self) -> SystemProxyObservation {
        #[cfg(target_os = "macos")]
        {
            parse_observation(&native::observation())
        }
        #[cfg(not(target_os = "macos"))]
        {
            SystemProxyObservation::Unknown
        }
    }
}

#[derive(Default)]
pub(super) struct ManualRuntime {
    port: Option<i32>,
}

impl SystemProxyService {
    #[must_use]
    pub fn with_observer(mut self, observer: Arc<dyn SystemProxyObserver>) -> Self {
        self.observer = observer;
        self
    }

    pub fn status(
        &self,
        request: &SystemProxyRequest,
    ) -> Result<SystemProxyStatus, SystemProxyError> {
        let mut status = plan_system_proxy(request)?.status;
        if status.management == SystemProxyManagement::Manual {
            status.observation = self.observer.observe();
            let runtime = self
                .manual_runtime
                .lock()
                .map_err(|_| SystemProxyError::ManualState)?;
            status.proxy = runtime.port.map(|port| format!("{LOOPBACK}:{port}"));
        }
        Ok(status)
    }

    pub(super) fn apply_manual(
        &self,
        request: &SystemProxyRequest,
    ) -> Result<SystemProxyStatus, SystemProxyError> {
        // Validate before recording the endpoint the UI will advertise.
        plan_system_proxy(request)?;
        {
            let mut runtime = self
                .manual_runtime
                .lock()
                .map_err(|_| SystemProxyError::ManualState)?;
            if request.force_disable {
                *runtime = ManualRuntime::default();
            } else {
                runtime.port = Some(request.socks_port);
            }
        }
        self.status(request)
    }

    pub(super) fn clear_manual_runtime(&self) {
        if let Ok(mut runtime) = self.manual_runtime.lock() {
            *runtime = ManualRuntime::default();
        }
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_observation(json: &str) -> SystemProxyObservation {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return SystemProxyObservation::Unknown;
    };
    let Some(servers) = value.get("servers").and_then(serde_json::Value::as_array) else {
        return SystemProxyObservation::Unknown;
    };
    let Some(urls) = value.get("pacURLs").and_then(serde_json::Value::as_array) else {
        return SystemProxyObservation::Unknown;
    };
    let hosts: Vec<_> = servers
        .iter()
        .map(|value| value.as_str().and_then(proxy_host))
        .chain(urls.iter().map(|value| value.as_str().and_then(pac_host)))
        .collect();
    if hosts.iter().flatten().any(is_loopback_host) {
        return SystemProxyObservation::LocalProxy;
    }
    if value.get("known").and_then(serde_json::Value::as_bool) != Some(true)
        || hosts.iter().any(Option::is_none)
    {
        return SystemProxyObservation::Unknown;
    }
    if servers.is_empty() && urls.is_empty() {
        SystemProxyObservation::Clear
    } else {
        SystemProxyObservation::OtherProxy
    }
}

// A malformed PAC setting cannot be used as evidence to retire a dirty marker.
// Inspect only its authority, dropping user info without exposing credentials.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn pac_host(input: &str) -> Option<url::Host> {
    let input = input.trim();
    let (_, authority) = input.split_once("://")?;
    if authority.is_empty()
        || authority.starts_with(['/', '\\'])
        || input.chars().any(char::is_whitespace)
    {
        return None;
    }
    let parsed = url::Url::parse(input).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(())?;
    parsed.host().map(|host| host.to_owned())
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn proxy_host(input: &str) -> Option<url::Host> {
    let input = input.trim();
    if let Ok(ip) = input.parse::<IpAddr>() {
        return Some(match ip {
            IpAddr::V4(ip) => url::Host::Ipv4(ip),
            IpAddr::V6(ip) => url::Host::Ipv6(ip),
        });
    }
    url::Host::parse(input).ok()
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn is_loopback_host(host: &url::Host) -> bool {
    match host {
        url::Host::Domain(domain) => domain
            .trim_end_matches('.')
            .eq_ignore_ascii_case("localhost"),
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => {
            ip.is_loopback() || ip.to_ipv4_mapped().is_some_and(|ip| ip.is_loopback())
        }
    }
}

pub fn open_network_settings() -> Result<(), SystemProxyError> {
    #[cfg(target_os = "macos")]
    {
        native::open_settings()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(SystemProxyError::UnsupportedPlatform(TargetOs::current()))
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::ffi::CStr;
    // SAFETY: linked in this crate's build.rs; returned strings are uniquely
    // owned and released using the matching allocator in the native module.
    unsafe extern "C" {
        fn voya_macos_proxy_observation() -> *mut libc::c_char;
        fn voya_macos_proxy_free(value: *mut libc::c_char);
        fn voya_macos_open_network_settings() -> i32;
    }
    pub(super) fn observation() -> String {
        // SAFETY: no arguments, nullable owned C string; copy before freeing.
        unsafe {
            let ptr = voya_macos_proxy_observation();
            if ptr.is_null() {
                return String::new();
            }
            let text = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            voya_macos_proxy_free(ptr);
            text
        }
    }
    pub(super) fn open_settings() -> Result<(), SystemProxyError> {
        // SAFETY: native function takes no arguments and returns a status code.
        if unsafe { voya_macos_open_network_settings() } == 0 {
            Ok(())
        } else {
            Err(SystemProxyError::OpenNetworkSettings)
        }
    }
}

#[cfg(test)]
mod tests;
