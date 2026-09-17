use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::{
    context::SS_SECURITIES_IN_SINGBOX,
    protocol_common::{
        first_list_value, inbound_port, inbound_protocol_tag, parse_pem_chain,
        parse_wireguard_reserved, raw_http_user_agent, shadowsocks_plugin_for, split_csv,
        wireguard_allowed_ips, wireguard_public_key, DEFAULT_SECURITY, RAW_HEADER_HTTP,
        WIREGUARD_DEFAULT_ADDRESS, WIREGUARD_DEFAULT_MTU,
    },
    text::{nonempty_str, nonempty_string},
    AppConfig, ConfigType, CoreConfigContext, InItem, InboundProtocol, ProfileItem,
    ProfileProtocol, ProfileTransport, RuleType, RulesItem, SpeedtestConfigEntry, TlsMode,
    TlsSettings, BLOCK_TAG, DEFAULT_BOOTSTRAP_DNS, DEFAULT_DIRECT_DNS, DEFAULT_REMOTE_DNS,
    DIRECT_TAG, LOOPBACK, PROXY_TAG,
};

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
const IP_IF_NON_MATCH: &str = "IPIfNonMatch";
const IP_ON_DEMAND: &str = "IPOnDemand";
const VMESS_SECURITIES: &[&str] = &[
    "aes-128-gcm",
    "chacha20-poly1305",
    DEFAULT_SECURITY,
    "none",
    "zero",
];
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
pub(crate) mod support;

pub use dns::first_dns_address;
pub use entry::*;
pub use schema::*;

use dns::*;
use experimental::*;
use inbounds::*;
use outbounds::*;
use routing::*;
use support::*;

#[cfg(test)]
mod tests;
