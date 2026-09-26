use crate::protocol_common::DEFAULT_SECURITY;

const USER_AGENT_HEADER: &str = "Sec-WebSocket-Protocol";
const DEFAULT_HYSTERIA2_HOP_INTERVAL: i32 = 30;
const DEFAULT_TUN_STACK: &str = "gvisor";
/// gRPC keepalive values. They used to be settings nobody could reach from the
/// UI; the generated output is byte-identical to those settings' defaults.
const GRPC_IDLE_TIMEOUT: &str = "60s";
const GRPC_PING_TIMEOUT: &str = "20s";
const GRPC_PERMIT_WITHOUT_STREAM: bool = false;
const MACOS_TUN_SAFE_MTU: i32 = 1500;
const SINGBOX_TUN_INBOUND_TAG: &str = "tun";
const SINGBOX_DIRECT_DNS_TAG: &str = "direct_dns";
const SINGBOX_REMOTE_DNS_TAG: &str = "remote_dns";
const SINGBOX_LOCAL_DNS_TAG: &str = "local_local";
const SINGBOX_HOSTS_DNS_TAG: &str = "hosts_dns";
const SINGBOX_FAKE_DNS_TAG: &str = "fake_dns";
const SINGBOX_FAKEIP_INET4_RANGE: &str = "198.18.0.0/15";
const SINGBOX_FAKEIP_INET6_RANGE: &str = "fc00::/18";
/// Where a `geosite:`/`geoip:` rule set without a local file is downloaded
/// from: `{0}` is the kind (`geosite` or `geoip`) and `{1}` the rule-set tag.
pub const DEFAULT_SINGBOX_RULESET_URL: &str =
    "https://raw.githubusercontent.com/2dust/sing-box-rules/rule-set-{0}/{1}.srs";
const GEOIP_PREFIX: &str = "geoip:";
const GEOSITE_PREFIX: &str = "geosite:";
const VMESS_SECURITIES: &[&str] = &[
    "aes-128-gcm",
    "chacha20-poly1305",
    DEFAULT_SECURITY,
    "none",
    "zero",
];
/// The uTLS fingerprint a REALITY client uses when no global one is set, and
/// the one self-hosted REALITY share links advertise.
pub const REALITY_FALLBACK_FINGERPRINT: &str = "chrome";
const SINGBOX_UTLS_FINGERPRINTS: &[&str] = &[
    "chrome",
    "firefox",
    "safari",
    "ios",
    "android",
    "edge",
    "360",
    "qq",
    "random",
    "randomized",
];

mod dns;
mod entry;
mod experimental;
mod inbounds;
mod outbounds;
mod routing;
mod schema;
mod schema_inbound;
mod selfhost;
pub(crate) mod support;

pub use dns::first_dns_address;
pub use entry::*;
pub use outbounds::{is_latency_probe_candidate, latency_probe_tag};
pub use schema::*;
pub use schema_inbound::*;
pub use selfhost::*;

#[cfg(test)]
mod tests;
