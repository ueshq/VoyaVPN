//! Requests, snapshots and failures exposed by the supervisor facade.
use super::CoreExitEvent;
use std::{fmt, path::PathBuf};
use thiserror::Error;
use voya_platform::{
    coreinfo::CoreLaunch,
    process::{ProcessError, ProcessRole},
    tun::{NativeTunError, TunBackend},
};

/// Bearer token the running core's Clash API requires.
///
/// sing-box's Clash API listens on loopback, which is *not* a trust boundary:
/// any local process, and any web page a browser can be pointed at, could
/// otherwise read the live connection list, switch every route to direct or pin
/// a node. A fresh token is minted per core launch, written into the generated
/// `experimental.clash_api.secret`, and demanded of every REST call and
/// websocket upgrade.
///
/// The value is deliberately opaque: `Debug` redacts it so a token can never
/// reach a tracing field, a log file or a crash report, and reading it back
/// takes the explicit [`ClashApiSecret::as_str`].
#[derive(Clone, PartialEq, Eq)]
pub struct ClashApiSecret(String);

impl fmt::Debug for ClashApiSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClashApiSecret(<redacted>)")
    }
}

impl ClashApiSecret {
    /// Mints a token for one core launch.
    ///
    /// Two v4 UUIDs give 244 random bits from the platform CSPRNG (each carries
    /// 122; six bits are version and variant markers). `uuid` is already a
    /// workspace dependency, so this adds no new supply-chain surface.
    #[must_use]
    pub fn generate() -> Self {
        Self(format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_token(self) -> String {
        self.0
    }
}

/// Everything a Clash API client needs to reach the core that is *running*.
///
/// Port and token are minted together by one core launch and are useless apart:
/// a stale token against a restarted core is a 401, and the port alone is a
/// 401 too. Carrying them as one value keeps every caller from re-deriving
/// either half.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClashApiAccess {
    pub port: Option<u16>,
    pub secret: Option<ClashApiSecret>,
}

impl ClashApiAccess {
    #[must_use]
    pub const fn new(port: Option<u16>, secret: Option<ClashApiSecret>) -> Self {
        Self { port, secret }
    }

    /// Access to a core on `port` that needs no token. Test and preview helper.
    #[must_use]
    pub const fn unauthenticated(port: u16) -> Self {
        Self {
            port: Some(port),
            secret: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreProcessSpec {
    /// How to start sing-box as a child process. `None` for a config only a
    /// native TUN backend runs: the macOS PacketTunnel carries sing-box itself
    /// and the connection never launches a child core.
    pub launch: Option<CoreLaunch>,
    pub config_path: Option<PathBuf>,
    pub display_log: bool,
    pub may_need_sudo: bool,
}

impl CoreProcessSpec {
    #[must_use]
    pub const fn new(launch: CoreLaunch) -> Self {
        Self {
            launch: Some(launch),
            config_path: None,
            display_log: true,
            may_need_sudo: true,
        }
    }

    /// A config for a native TUN backend, which starts no child process.
    #[must_use]
    pub const fn native_tun() -> Self {
        Self {
            launch: None,
            config_path: None,
            display_log: true,
            may_need_sudo: false,
        }
    }

    #[must_use]
    pub fn with_config_path(mut self, config_path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(config_path.into());
        self
    }

    #[must_use]
    pub const fn with_display_log(mut self, display_log: bool) -> Self {
        self.display_log = display_log;
        self
    }

    #[must_use]
    pub const fn with_may_need_sudo(mut self, may_need_sudo: bool) -> Self {
        self.may_need_sudo = may_need_sudo;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorStartRequest {
    pub active_profile_id: Option<String>,
    /// The policy group this launch runs, when a group rather than a node is active.
    pub active_group_id: Option<String>,
    pub main: CoreProcessSpec,
    pub pre: Option<CoreProcessSpec>,
    pub tun_enabled: bool,
    /// Keep traffic from leaving outside the tunnel. sing-box enforces it
    /// through the generated `strict_route` on Windows and Linux; the macOS
    /// PacketTunnel applies it to the VPN configuration instead.
    pub kill_switch: bool,
    pub sudo_script_dir: PathBuf,
    pub restart_on_crash: bool,
    /// Clash API port of the *main* generated config.
    ///
    /// This cannot be recomputed from `AppConfig`: on a pre-socks topology the
    /// builder clears `is_tun_enabled` on the main context, so the main process
    /// listens on `api2` while the pre-socks one takes `api2 + 1`. Deriving it
    /// from the TUN setting instead would point every client at the pre-socks
    /// process, which has no selector or per-node statistics.
    pub clash_api_port: i32,
    /// Bearer token the *main* generated config wrote into
    /// `experimental.clash_api.secret`, if the caller minted one.
    pub clash_api_secret: Option<ClashApiSecret>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorConnectionState {
    CleanupPending,
    Disconnected,
    Connected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorSnapshot {
    /// Elapsed time since this running core successfully connected.
    pub connected_duration_ms: Option<u64>,
    pub state: SupervisorConnectionState,
    pub active_tun_backend: Option<TunBackend>,
    pub active_profile_id: Option<String>,
    pub active_group_id: Option<String>,
    pub main_pid: Option<u32>,
    pub pre_pid: Option<u32>,
    /// Clash API port the running main config actually listens on.
    pub clash_api_port: Option<i32>,
    /// Bearer token the running main config demands on that port.
    pub clash_api_secret: Option<ClashApiSecret>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTunExitEvent {
    pub active_profile_id: Option<String>,
    pub backend: TunBackend,
    pub message: String,
}

pub trait SupervisorEventSink: Send + Sync {
    fn native_tun_exited(&self, event: NativeTunExitEvent);

    /// A tracked core process exited and the supervisor has decided what to do
    /// about it. The default ignores the event so sinks that only care about
    /// the native TUN backend keep compiling.
    fn core_exited(&self, _event: CoreExitEvent) {}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSupervisorEventSink;

impl SupervisorEventSink for NoopSupervisorEventSink {
    fn native_tun_exited(&self, _event: NativeTunExitEvent) {}
}

impl SupervisorSnapshot {
    /// The policy group the connected core is using, if any.
    ///
    /// Shared by both hosts so "which group is running" cannot drift: a
    /// disconnected snapshot never names a group even if one is still recorded.
    #[must_use]
    pub fn running_group_id(&self) -> Option<String> {
        self.active_group_id
            .clone()
            .filter(|_| self.state == SupervisorConnectionState::Connected)
    }

    #[must_use]
    pub const fn disconnected() -> Self {
        Self {
            state: SupervisorConnectionState::Disconnected,
            connected_duration_ms: None,
            active_tun_backend: None,
            active_profile_id: None,
            active_group_id: None,
            main_pid: None,
            pre_pid: None,
            clash_api_port: None,
            clash_api_secret: None,
        }
    }

    /// How to reach the running core's Clash API.
    ///
    /// Both halves are cleared while nothing is running, so a caller that hands
    /// this straight to a client dials nothing instead of dialling a port the
    /// previous core no longer owns.
    #[must_use]
    pub fn clash_api_access(&self) -> ClashApiAccess {
        ClashApiAccess {
            port: self
                .clash_api_port
                .and_then(|port| u16::try_from(port).ok()),
            secret: self.clash_api_secret.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("supervisor command channel is closed")]
    CommandChannelClosed,
    #[error("supervisor response channel was dropped")]
    ResponseDropped,
    #[error("system authorization is required before spawning elevated sing_box")]
    ElevationNotGranted,
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    NativeTun(#[from] NativeTunError),
    #[error("process job error: {0}")]
    Job(String),
    #[error("elevation error: {0}")]
    Elevation(String),
    #[error("sudo kill target pid {pid} does not match a tracked elevated process")]
    UnknownSudoKillTarget { pid: u32 },
    #[error("missing runtime config path for native TUN {role:?} process")]
    MissingNativeTunConfigPath { role: ProcessRole },
    #[error("no sing-box executable to launch the {role:?} process with")]
    MissingCoreLaunch { role: ProcessRole },
    #[error("sudo kill for pid {pid} failed with status {status_code:?}: {stderr}")]
    SudoKillFailed {
        pid: u32,
        status_code: Option<i32>,
        stderr: String,
    },
}

impl From<voya_platform::elevation::ElevationError> for SupervisorError {
    fn from(error: voya_platform::elevation::ElevationError) -> Self {
        Self::Elevation(error.to_string())
    }
}
