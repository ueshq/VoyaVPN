//! The self-hosted exit node: what the user configures, what the page shows,
//! and the record the database keeps.
//!
//! The REALITY private key, the Shadowsocks key and the VLESS UUID live only in
//! [`SelfHostRecordV1`], which never crosses IPC. The renderer sees them only
//! inside the finished share links, which carry what a peer needs to connect
//! and nothing that would let it impersonate the node.

use std::fmt;

use serde::{Deserialize, Serialize};
use specta::Type;

/// The part of the node the user edits.
///
/// A port of `0` means "not chosen yet": the app picks a free one the first
/// time the node is enabled and stores it, so links stay stable afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostConfig {
    pub enabled: bool,
    /// First part of every link's remark. Empty uses the product name.
    pub device_label: String,
    pub vless_enabled: bool,
    pub vless_port: u16,
    pub shadowsocks_enabled: bool,
    pub shadowsocks_port: u16,
    /// The public site whose TLS handshake the REALITY inbound borrows.
    pub reality_server_name: String,
    pub reality_server_port: u16,
    /// A DDNS name or fixed address links should use instead of the detected
    /// public addresses.
    pub custom_address: Option<String>,
    /// Lets peers reach this device's local network. Loopback stays closed.
    pub allow_lan_access: bool,
    pub block_bittorrent: bool,
    /// Ask the router (UPnP IGD) to forward the node's ports.
    pub upnp_enabled: bool,
}

/// Verified to complete a REALITY handshake with sing-box 1.13. Some large
/// sites (www.microsoft.com among them) answer TLS 1.3 yet fail REALITY, so a
/// default must be one that has been tried; the node self-test catches a bad
/// choice made later.
pub const SELF_HOST_DEFAULT_REALITY_SERVER_NAME: &str = "www.apple.com";
pub const SELF_HOST_DEFAULT_REALITY_SERVER_PORT: u16 = 443;

impl Default for SelfHostConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device_label: String::new(),
            vless_enabled: true,
            vless_port: 0,
            shadowsocks_enabled: true,
            shadowsocks_port: 0,
            reality_server_name: SELF_HOST_DEFAULT_REALITY_SERVER_NAME.to_string(),
            reality_server_port: SELF_HOST_DEFAULT_REALITY_SERVER_PORT,
            custom_address: None,
            allow_lan_access: false,
            block_bittorrent: true,
            upnp_enabled: true,
        }
    }
}

/// The credentials minted for the node. Persistence only.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostCredentialsV1 {
    pub vless_uuid: String,
    /// base64url X25519 private key.
    pub reality_private_key: String,
    pub reality_short_id: String,
    /// base64 key for `2022-blake3-aes-128-gcm`.
    pub shadowsocks_password: String,
}

impl fmt::Debug for SelfHostCredentialsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SelfHostCredentialsV1(<redacted>)")
    }
}

/// The stored node: settings plus credentials, `None` until first enabled.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostRecordV1 {
    pub schema_version: u32,
    pub config: SelfHostConfig,
    pub credentials: Option<SelfHostCredentialsV1>,
}

impl Default for SelfHostRecordV1 {
    fn default() -> Self {
        Self {
            schema_version: crate::CURRENT_SCHEMA_VERSION,
            config: SelfHostConfig::default(),
            credentials: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostRuntimeStatus {
    Stopped,
    Starting,
    Running,
    /// The core exited and a restart is scheduled.
    Retrying,
    /// Starting failed, or the core kept exiting; the node stays down until the
    /// user changes something or switches it off and on.
    Failed,
}

/// Why the node is not running as asked. The frontend translates the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostProblem {
    /// Neither protocol is switched on.
    NoProtocol,
    /// Another program already listens on one of the node's ports.
    PortInUse,
    /// The sing-box core is not installed.
    CoreMissing,
    /// The core exited on its own.
    CoreExited,
    /// The config could not be written or the core could not be launched.
    StartFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostRuntime {
    pub status: SelfHostRuntimeStatus,
    pub problem: Option<SelfHostProblem>,
    /// Untranslated diagnostic behind `problem`.
    pub detail: Option<String>,
    /// The port behind `PortInUse`.
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostProtocol {
    Vless,
    Shadowsocks,
}

/// Where a share link's address came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostAddressKind {
    Custom,
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostShareLink {
    pub protocol: SelfHostProtocol,
    pub address_kind: SelfHostAddressKind,
    pub address: String,
    pub port: u16,
    pub remarks: String,
    pub link: String,
}

/// Everything the Self-hosted node page renders.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostState {
    pub config: SelfHostConfig,
    /// What every setting falls back to: the page shows these as placeholders
    /// and saves them to reset the node.
    pub defaults: SelfHostConfig,
    pub runtime: SelfHostRuntime,
    /// Links for every enabled protocol at every usable address.
    pub share_links: Vec<SelfHostShareLink>,
    /// The latest network check, if one ran since launch.
    pub environment: Option<SelfHostEnvironmentReport>,
    /// Whether this platform can add a firewall rule for the node.
    pub firewall_rule_supported: bool,
}

/// Live traffic through the node, read from its core.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostStats {
    pub active_connections: u32,
    pub upload_total_bytes: f64,
    pub download_total_bytes: f64,
}

// ---- network environment -------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostAddressFamily {
    Ipv4,
    Ipv6,
}

/// How an interface address relates to the internet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostAddressScope {
    /// Globally routable.
    Public,
    /// RFC 1918 or IPv6 unique-local.
    Private,
    /// 100.64.0.0/10, assigned by a carrier-grade NAT or an overlay network.
    Shared,
    LinkLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostLocalAddress {
    pub interface: String,
    pub address: String,
    pub family: SelfHostAddressFamily,
    pub scope: SelfHostAddressScope,
}

/// What sits between this device and the internet for one address family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostNatKind {
    /// The public address is on this device.
    None,
    /// One home router.
    Nat,
    /// The carrier shares one public address between customers.
    CarrierGrade,
    /// Two routers in a row.
    Double,
    /// No public address was learned for this family.
    NoConnectivity,
    Unknown,
}

/// Whether a device on the internet can open a connection to the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostReachability {
    /// The probe service connected to every node port.
    Reachable,
    /// The probe service could not connect.
    Unreachable,
    /// No probe, but nothing in the way was found.
    LikelyReachable,
    /// No probe, and a router is in the way without a forwarding rule.
    NeedsPortForward,
    NoConnectivity,
    Unknown,
}

/// One finding of the network check. Each code has its own explanation and
/// suggestion in the locale files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostReasonCode {
    PublicAddressOnDevice,
    BehindNat,
    CarrierGradeNat,
    DoubleNat,
    PortMapped,
    UpnpUnavailable,
    UpnpFailed,
    ProbeReachable,
    ProbeTimedOut,
    ProbeRefused,
    ProbeUnavailable,
    NoPublicAddress,
    Ipv6FirewallUnknown,
    TunActive,
    FirewallRuleMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostFamilyReport {
    pub family: SelfHostAddressFamily,
    pub public_address: Option<String>,
    pub nat: SelfHostNatKind,
    pub reachability: SelfHostReachability,
    /// Set when `reachability` comes from the probe service, not inference.
    pub verified_by_probe: bool,
    pub reasons: Vec<SelfHostReasonCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostPortMappingStatus {
    Disabled,
    /// No UPnP gateway answered on this network.
    NoGateway,
    Mapped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostPortMappingReport {
    pub status: SelfHostPortMappingStatus,
    /// The router's own WAN address, as it reported it.
    pub gateway_external_address: Option<String>,
    pub mapped_ports: Vec<u16>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostFirewallStatus {
    /// The platform's firewall is not managed by the app.
    NotManaged,
    RuleMissing,
    RulePresent,
    Unknown,
}

/// Whether a client on this device could use the node through one protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SelfHostSelfTestResult {
    Passed,
    Failed,
    /// The protocol is off, or the node was not running.
    Skipped,
}

/// A real client connecting to the node over loopback and fetching a page
/// through it: the only check that catches a disguise site REALITY cannot
/// use, which otherwise fails silently on every peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostSelfTest {
    pub vless: SelfHostSelfTestResult,
    pub shadowsocks: SelfHostSelfTestResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelfHostEnvironmentReport {
    /// Milliseconds since the Unix epoch.
    #[specta(type = f64)]
    pub checked_at_ms: u64,
    pub local_addresses: Vec<SelfHostLocalAddress>,
    pub ipv4: SelfHostFamilyReport,
    pub ipv6: SelfHostFamilyReport,
    pub port_mapping: SelfHostPortMappingReport,
    pub firewall: SelfHostFirewallStatus,
    /// Whether the probe service answered at all.
    pub probe_available: bool,
    pub self_test: SelfHostSelfTest,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_never_print() {
        let credentials = SelfHostCredentialsV1 {
            vless_uuid: "uuid-value".to_string(),
            reality_private_key: "private-key".to_string(),
            reality_short_id: "short-id".to_string(),
            shadowsocks_password: "ss-key".to_string(),
        };
        let record = SelfHostRecordV1 {
            credentials: Some(credentials),
            ..SelfHostRecordV1::default()
        };
        let printed = format!("{record:?}");
        for secret in ["uuid-value", "private-key", "short-id", "ss-key"] {
            assert!(!printed.contains(secret), "{printed}");
        }
    }

    #[test]
    fn record_is_strict_camel_case() {
        let value = serde_json::to_value(SelfHostRecordV1::default()).expect("serialize");
        assert_eq!(value["config"]["vlessPort"], 0);
        assert_eq!(
            value["config"]["realityServerName"],
            SELF_HOST_DEFAULT_REALITY_SERVER_NAME
        );
        let mut extra = value;
        extra["config"]["unknown"] = serde_json::json!(true);
        assert!(serde_json::from_value::<SelfHostRecordV1>(extra).is_err());
    }
}
