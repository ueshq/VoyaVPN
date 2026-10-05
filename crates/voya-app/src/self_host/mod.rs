//! The self-hosted exit node: this device runs a sing-box server that other
//! devices connect to through the links the page hands out.
//!
//! The node is a core of its own, launched beside — never inside — the
//! connection core, so a machine can host while it is disconnected, which is
//! how an overseas exit usually runs (ADR 0011). It owns:
//!
//! - a record in `voya-db` with the settings and the minted credentials, which
//!   never cross IPC;
//! - one supervised sing-box process on a dedicated runner, restarted with
//!   backoff when it exits on its own;
//! - a network check that learns the public addresses, asks the router for a
//!   forward, and has the probe service connect back to the node's ports;
//! - a watch loop that repeats the check, keeps the router lease alive, and
//!   tells the user when the public address moves.

use std::{net::IpAddr, path::PathBuf, sync::Arc, time::Duration};

use crate::{
    config_mutation::ConfigMutationCoordinator,
    supervisor::{CoreSupervisor, SupervisorConnectionState},
};
use futures_util::future::BoxFuture;
use thiserror::Error;
use voya_contracts::{AppNoticeLevel, LogCode, LogLevel, NoticeCode, ValidationIssue};
use voya_core::SingboxConfigError;
use voya_db::DbError;
pub use voya_net::probe::DEFAULT_PROBE_BASE_URL;
use voya_net::{
    clash::ClashError,
    portmap::{PortMapper, UpnpPortMapper},
    probe::{
        ProbeFamily, ReachabilityProbeClient, ReachabilityProbeError, ReachabilityProbeResponse,
    },
};
use voya_platform::{
    coreinfo::{CoreInfoError, TargetOs},
    firewall::{FirewallError, FirewallService},
    netif::{self, InterfaceAddress},
    paths::AppPaths,
    process::{ProcessError, ProcessRunner},
};

mod environment;
mod identity;
mod manager;
mod process;
mod selftest;
mod spec;
mod watch;

pub use manager::SelfHostManager;
pub use selftest::{NodeSelfTester, ProbeCoreSelfTester};

/// Where the node reports what the user should see.
pub trait SelfHostEventSink: Send + Sync {
    /// Status, links or the network report changed.
    fn state_changed(&self);
    fn log(&self, level: LogLevel, code: LogCode, detail: Option<String>);
    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: Option<String>);
}

/// The probe service, behind a trait so the network check can be tested.
pub trait ReachabilityProbe: Send + Sync {
    fn probe(
        &self,
        family: ProbeFamily,
        ports: Vec<u16>,
    ) -> BoxFuture<'static, std::result::Result<ReachabilityProbeResponse, ReachabilityProbeError>>;
    fn public_address(
        &self,
        family: ProbeFamily,
    ) -> BoxFuture<'static, std::result::Result<IpAddr, ReachabilityProbeError>>;
}

impl ReachabilityProbe for ReachabilityProbeClient {
    fn probe(
        &self,
        family: ProbeFamily,
        ports: Vec<u16>,
    ) -> BoxFuture<'static, std::result::Result<ReachabilityProbeResponse, ReachabilityProbeError>>
    {
        let client = self.clone();
        Box::pin(async move { client.probe(family, &ports).await })
    }

    fn public_address(
        &self,
        family: ProbeFamily,
    ) -> BoxFuture<'static, std::result::Result<IpAddr, ReachabilityProbeError>> {
        let client = self.clone();
        Box::pin(async move { client.public_address(family).await })
    }
}

/// The probe service at `base_url`, reached directly per address family.
#[must_use]
pub fn probe_service(base_url: &str) -> Arc<dyn ReachabilityProbe> {
    Arc::new(ReachabilityProbeClient::new(base_url))
}

/// The home router, asked over UPnP IGD.
#[must_use]
pub fn router_port_mapper() -> Arc<dyn PortMapper> {
    Arc::new(UpnpPortMapper)
}

/// This machine's interfaces and ports. Every call blocks briefly.
pub trait LocalNetwork: Send + Sync {
    fn interface_addresses(&self) -> Vec<InterfaceAddress>;
    fn port_available(&self, port: u16) -> bool;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemLocalNetwork;

impl LocalNetwork for SystemLocalNetwork {
    fn interface_addresses(&self) -> Vec<InterfaceAddress> {
        netif::interface_addresses().unwrap_or_else(|error| {
            tracing::warn!(%error, "failed to list network interfaces");
            Vec::new()
        })
    }

    fn port_available(&self, port: u16) -> bool {
        netif::wildcard_port_available(port)
    }
}

/// Whether this device's own tunnel is up. While it is, the tunnel carries
/// the probe request and the answer says nothing about this device.
pub trait HostTunnelState: Send + Sync {
    fn tunnel_active(&self) -> BoxFuture<'static, bool>;
}

/// How long the tunnel check waits for the supervisor's own answer.
const TUNNEL_STATE_TIMEOUT: Duration = Duration::from_secs(1);

/// The tunnel state as the connection core reports it: connected with a TUN
/// backend, or with TUN mode on.
pub struct SupervisorTunnelState {
    pub supervisor: CoreSupervisor,
    pub config: Arc<ConfigMutationCoordinator>,
}

impl HostTunnelState for SupervisorTunnelState {
    fn tunnel_active(&self) -> BoxFuture<'static, bool> {
        let supervisor = self.supervisor.clone();
        let config = Arc::clone(&self.config);
        Box::pin(async move {
            let tun_enabled = config.current_config().tun.enabled;
            // The supervisor answers in turn, and a tunnel start ahead of this
            // read can hold it for many seconds while the network check waits
            // with its lock held. What it publishes without a round trip is
            // enough to answer by once that happens.
            match tokio::time::timeout(TUNNEL_STATE_TIMEOUT, supervisor.status()).await {
                Ok(Ok(snapshot)) => {
                    snapshot.state == SupervisorConnectionState::Connected
                        && (snapshot.active_tun_backend.is_some() || tun_enabled)
                }
                Ok(Err(_)) => false,
                Err(_) => supervisor.is_connected() && tun_enabled,
            }
        })
    }
}

/// Everything the node needs from the machine, injected so tests can fake it.
#[derive(Clone)]
pub struct SelfHostDeps {
    pub paths: AppPaths,
    pub core_seed_resource_dir: Option<PathBuf>,
    /// A runner of its own: the exit handler slot of the connection core's
    /// runner belongs to the supervisor.
    pub runner: Arc<dyn ProcessRunner>,
    pub target_os: TargetOs,
    pub probe: Arc<dyn ReachabilityProbe>,
    pub port_mapper: Arc<dyn PortMapper>,
    pub firewall: FirewallService,
    pub network: Arc<dyn LocalNetwork>,
    pub host_tunnel: Arc<dyn HostTunnelState>,
    pub sink: Arc<dyn SelfHostEventSink>,
    pub self_tester: Arc<dyn NodeSelfTester>,
    /// The app's log level, applied to the node's core as well.
    pub log_level: String,
    pub restart_backoff: RestartBackoff,
}

/// How long to wait before restarting a core that exited on its own: doubling
/// from `initial` up to `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartBackoff {
    pub initial: Duration,
    pub max: Duration,
}

impl Default for RestartBackoff {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Error)]
pub enum SelfHostError {
    #[error("self-hosted node settings are invalid")]
    Validation(Vec<ValidationIssue>),
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("could not draw random bytes for the node's keys: {0}")]
    Random(#[from] getrandom::Error),
    #[error("no free port was found for the self-hosted node")]
    NoFreePort,
    #[error(transparent)]
    CoreInfo(#[from] CoreInfoError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("failed to write the self-hosted config {path}: {source}")]
    WriteConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Config(#[from] SingboxConfigError),
    #[error(transparent)]
    Firewall(#[from] FirewallError),
    #[error(transparent)]
    Clash(#[from] ClashError),
    #[error(transparent)]
    Probe(#[from] ReachabilityProbeError),
    #[error("background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, SelfHostError>;

#[cfg(test)]
mod tests;
