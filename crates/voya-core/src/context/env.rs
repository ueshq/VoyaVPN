//! What config generation is told about the host: the platform, how TUN is
//! delivered there, and the environment trait the context builder reads
//! everything else through.

use std::collections::{BTreeMap, BTreeSet};

use super::ContextPolicyGroup;
use crate::{AppConfig, InboundProtocol, ProfileItem, RoutingItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreGenPlatform {
    Windows,
    MacOS,
    Linux,
}

impl CoreGenPlatform {
    #[must_use]
    pub const fn is_windows(self) -> bool {
        matches!(self, Self::Windows)
    }

    #[must_use]
    pub const fn is_macos(self) -> bool {
        matches!(self, Self::MacOS)
    }
}

/// How TUN is delivered on the host, injected as a platform fact.
///
/// ADR 0005 moved macOS to a single in-process NetworkExtension config, so the
/// topology can no longer be inferred from [`CoreGenPlatform`] alone: Windows
/// (service backend) and macOS (PacketTunnel provider) both carry TUN inside
/// the one generated config, while Linux still splits into a privileged TUN
/// process and an unprivileged SOCKS process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunTopology {
    /// One sing-box process owns both the TUN inbound and the remote outbounds.
    SingleProcess,
    /// A privileged TUN process forwards into an unprivileged SOCKS process.
    PreSocks,
}

impl TunTopology {
    /// Default topology for a platform when the caller injects nothing better.
    #[must_use]
    pub const fn for_platform(platform: CoreGenPlatform) -> Self {
        match platform {
            CoreGenPlatform::Windows | CoreGenPlatform::MacOS => Self::SingleProcess,
            CoreGenPlatform::Linux => Self::PreSocks,
        }
    }

    #[must_use]
    pub const fn is_pre_socks(self) -> bool {
        matches!(self, Self::PreSocks)
    }
}

/// How the generated config treats IPv6, derived from the TUN IPv6 switch and
/// what is known about the active node's IPv6 egress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ipv6Mode {
    /// IPv6 resolves and routes on every path.
    Full,
    /// The switch is on but the active node (or a member of the active group)
    /// is recorded as having no IPv6 egress: direct paths stay dual-stack,
    /// proxy-path DNS defaults to `ipv4_only`, and IPv6 destinations that no
    /// direct rule claims are rejected locally.
    DirectOnly,
    /// The switch is off: DNS is `ipv4_only` everywhere and IPv6 destinations
    /// that no direct rule claims are rejected locally.
    Off,
}

impl Ipv6Mode {
    /// Whether IPv6 destinations left to the proxy are refused locally.
    #[must_use]
    pub const fn rejects_proxied_ipv6(self) -> bool {
        !matches!(self, Self::Full)
    }
}

pub trait CoreGenEnv {
    fn platform(&self) -> CoreGenPlatform;

    /// TUN process topology for this host.
    ///
    /// Overriding this is how a caller tells `voya-core` that TUN runs inside
    /// the single generated config (macOS PacketTunnel, Windows service)
    /// instead of the Linux pre-socks split; see [`TunTopology`].
    fn tun_topology(&self) -> TunTopology {
        TunTopology::for_platform(self.platform())
    }

    /// Per-launch bearer token for the generated Clash API, if the caller has
    /// one. Injected so config generation stays deterministic.
    fn get_clash_api_secret(&self) -> Option<String> {
        None
    }

    fn get_profile_by_remarks(&self, remarks: &str) -> Option<ProfileItem>;
    /// A policy group a routing rule names, with its members resolved against
    /// the current node list. `None` when no such group exists.
    fn get_policy_group(&self, _id: &str) -> Option<ContextPolicyGroup> {
        None
    }
    fn get_default_routing(&self, config: &AppConfig) -> Option<RoutingItem>;
    fn get_local_port(&self, protocol: InboundProtocol) -> i32;
    fn get_singbox_ruleset_paths(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }
    /// Ids of the nodes recorded as having no IPv6 egress. Injected by the
    /// orchestration layer, which probes a node after connecting to it.
    fn ipv6_unsupported_nodes(&self) -> BTreeSet<String> {
        BTreeSet::new()
    }
}
