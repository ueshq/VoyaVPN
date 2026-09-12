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
        first_list_value, inbound_port, inbound_protocol_tag, nonempty_str, parse_pem_chain,
        parse_wireguard_reserved, protocol_name, raw_http_user_agent, shadowsocks_plugin_for,
        split_list, wireguard_allowed_ips, wireguard_public_key, DEFAULT_SECURITY, RAW_HEADER_HTTP,
        WIREGUARD_DEFAULT_ADDRESS, WIREGUARD_DEFAULT_MTU,
    },
    AppConfig, ConfigType, CoreConfigContext, InItem, InboundProtocol, ProfileItem,
    ProfileProtocol, ProfileTransport, RuleType, RulesItem, SpeedtestConfigEntry, TlsMode,
    TlsSettings, BLOCK_TAG, DEFAULT_BOOTSTRAP_DNS, DEFAULT_DIRECT_DNS, DEFAULT_REMOTE_DNS,
    DIRECT_TAG, LOOPBACK, PROXY_TAG,
};

const USER_AGENT_HEADER: &str = "Sec-WebSocket-Protocol";
const DEFAULT_HYSTERIA2_HOP_INTERVAL: i32 = 30;
const DEFAULT_TUN_STACK: &str = "gvisor";
const MACOS_TUN_SAFE_MTU: i32 = 1500;
const SINGBOX_TUN_INBOUND_TAG: &str = "tun";
const SINGBOX_DIRECT_DNS_TAG: &str = "direct_dns";
const SINGBOX_REMOTE_DNS_TAG: &str = "remote_dns";
const SINGBOX_LOCAL_DNS_TAG: &str = "local_local";
const SINGBOX_HOSTS_DNS_TAG: &str = "hosts_dns";
const SINGBOX_FAKE_DNS_TAG: &str = "fake_dns";
const SINGBOX_FAKEIP_INET4_RANGE: &str = "198.18.0.0/15";
const SINGBOX_FAKEIP_INET6_RANGE: &str = "fc00::/18";
/// Vendors whose endpoints VoyaVPN forces through the proxy in Rule mode.
///
/// These hosts geo-block or hard-fail on a direct connection from the regions
/// this app targets, so a "Rule" routing table that would otherwise send them
/// direct produces a broken tool rather than a faster one. The rules are
/// appended *after* the `clash_mode` rules (see
/// `crate::singbox::routing::gen_routing` and
/// `crate::singbox::dns::gen_dns_rules`) so an explicit Global mode
/// still wins; within Rule mode they precede the user's own rules.
///
/// **This overrides user routing intent.** Inside Rule mode a user rule that
/// sends one of these suffixes direct is generated after the priority rule and
/// therefore never matches, and the names are always resolved through the
/// remote DNS server. There is no per-domain opt-out today.
/// Editing the list changes generated JSON; see `docs/adr/0006-priority-proxy-domain-list.md`.
const PRIORITY_PROXY_DOMAIN_SUFFIXES: &[&str] = &[
    "anthropic.com",
    "claude.ai",
    "claudeusercontent.com",
    "openai.com",
    "chatgpt.com",
    "oaistatic.com",
    "oaiusercontent.com",
    "openai.azure.com",
    "githubcopilot.com",
    "copilot-proxy.githubusercontent.com",
    "copilot-telemetry.githubusercontent.com",
    "cursor.com",
    "cursor.sh",
    "codeium.com",
    "windsurf.com",
    "sourcegraph.com",
    "perplexity.ai",
    "generativelanguage.googleapis.com",
    "aistudio.google.com",
    "gemini.google.com",
    "ai.google.dev",
    "poe.com",
    "x.ai",
    "grok.com",
    "cohere.ai",
    "mistral.ai",
    "huggingface.co",
];
const SINGBOX_RULESET_URL: &str =
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
