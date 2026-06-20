use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::{
    AppConfig, ConfigType, CoreConfigContext, InItem, InboundProtocol, MultipleLoad, ProfileItem,
    ProtocolExtraItem, RuleType, RulesItem, TransportExtraItem, BLOCK_TAG, DEFAULT_BOOTSTRAP_DNS,
    DEFAULT_DIRECT_DNS, DEFAULT_LOCAL_PORT, DEFAULT_REMOTE_DNS, DIRECT_TAG, LOOPBACK, PROXY_TAG,
};

const DEFAULT_SECURITY: &str = "auto";
const DEFAULT_NETWORK: &str = "raw";
const RAW_HEADER_HTTP: &str = "http";
const STREAM_SECURITY_TLS: &str = "tls";
const STREAM_SECURITY_REALITY: &str = "reality";
const USER_AGENT_HEADER: &str = "Sec-WebSocket-Protocol";
const WIREGUARD_DEFAULT_ADDRESS: &str = "172.16.0.2/32";
const WIREGUARD_DEFAULT_ALLOWED_IPS: &[&str] = &["0.0.0.0/0", "::/0"];
const WIREGUARD_DEFAULT_MTU: i32 = 1280;
const DEFAULT_HYSTERIA2_HOP_INTERVAL: i32 = 30;
const DEFAULT_TUN_STACK: &str = "gvisor";
const SINGBOX_TUN_INBOUND_TAG: &str = "tun";
const SINGBOX_DIRECT_DNS_TAG: &str = "direct_dns";
const SINGBOX_REMOTE_DNS_TAG: &str = "remote_dns";
const SINGBOX_LOCAL_DNS_TAG: &str = "local_local";
const SINGBOX_HOSTS_DNS_TAG: &str = "hosts_dns";
const SINGBOX_FAKE_DNS_TAG: &str = "fake_dns";
const SINGBOX_FAKEIP_INET4_RANGE: &str = "198.18.0.0/15";
const SINGBOX_FAKEIP_INET6_RANGE: &str = "fc00::/18";
const SINGBOX_RULESET_URL: &str =
    "https://raw.githubusercontent.com/2dust/sing-box-rules/rule-set-{0}/{1}.srs";
const GEOIP_PREFIX: &str = "geoip:";
const GEOSITE_PREFIX: &str = "geosite:";
const IP_IF_NON_MATCH: &str = "IPIfNonMatch";
const IP_ON_DEMAND: &str = "IPOnDemand";
pub const DEFAULT_SINGBOX_DNS_NORMAL: &str = r#"{
  "servers": [
    {
      "tag": "remote",
      "type": "tcp",
      "server": "8.8.8.8",
      "detour": "proxy"
    },
    {
      "tag": "local",
      "type": "udp",
      "server": "223.5.5.5"
    }
  ],
  "rules": [
    {
      "rule_set": [
        "geosite-google"
      ],
      "server": "remote",
      "strategy": "prefer_ipv4"
    },
    {
      "rule_set": [
        "geosite-cn"
      ],
      "server": "local",
      "strategy": "prefer_ipv4"
    }
  ],
  "final": "remote",
  "strategy": "prefer_ipv4"
}"#;

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

const SS_SECURITIES_IN_SINGBOX: &[&str] = &[
    "aes-256-gcm",
    "aes-192-gcm",
    "aes-128-gcm",
    "chacha20-ietf-poly1305",
    "xchacha20-ietf-poly1305",
    "none",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
    "aes-128-ctr",
    "aes-192-ctr",
    "aes-256-ctr",
    "aes-128-cfb",
    "aes-192-cfb",
    "aes-256-cfb",
    "rc4-md5",
    "chacha20-ietf",
    "xchacha20",
];

#[derive(Debug, Error)]
pub enum SingboxConfigError {
    #[error("invalid sing-box custom ruleset JSON: {0}")]
    CustomRulesetJson(#[source] serde_json::Error),
    #[error("sing-box custom ruleset at index {index} is missing tag, type, or format")]
    CustomRulesetMissingRequiredFields { index: usize },
    #[error("sing-box node {remarks} has invalid port {port}")]
    InvalidNodePort { remarks: String, port: i32 },
    #[error("sing-box WireGuard node {remarks} is missing peer public key")]
    MissingWireGuardPublicKey { remarks: String },
    #[error("failed to serialize sing-box config: {0}")]
    Serialize(#[source] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log: Option<SingboxLog>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<SingboxDns>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inbounds: Vec<SingboxInbound>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outbounds: Vec<SingboxOutbound>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<SingboxEndpoint>,
    pub route: SingboxRoute,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<SingboxExperimental>,
}

impl Default for SingboxConfig {
    fn default() -> Self {
        Self::sample()
    }
}

impl SingboxConfig {
    #[must_use]
    pub fn sample() -> Self {
        Self {
            log: Some(SingboxLog {
                disabled: None,
                level: "debug".to_string(),
                output: None,
                timestamp: Some(true),
            }),
            dns: None,
            inbounds: Vec::new(),
            outbounds: vec![SingboxOutbound::direct()],
            endpoints: Vec::new(),
            route: SingboxRoute {
                default_domain_resolver: None,
                auto_detect_interface: None,
                rules: Vec::new(),
                rule_set: None,
                final_outbound: None,
            },
            experimental: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxLog {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    pub level: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<bool>,
}

impl Default for SingboxLog {
    fn default() -> Self {
        Self {
            disabled: None,
            level: "debug".to_string(),
            output: None,
            timestamp: Some(true),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxRoute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_domain_resolver: Option<SingboxRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_detect_interface: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<SingboxRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<Vec<SingboxRuleset>>,
    #[serde(rename = "final", skip_serializing_if = "Option::is_none")]
    pub final_outbound: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxDns {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<SingboxDnsServer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<SingboxRule>,
    #[serde(rename = "final", skip_serializing_if = "Option::is_none")]
    pub final_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_cache: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_expire: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub independent_cache: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_capacity: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse_mapping: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_subnet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxDnsServer {
    pub r#type: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inet4_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inet6_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_subnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_resolver: Option<String>,
    #[serde(rename = "interface", skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<SingboxHeaders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predefined: Option<BTreeMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detour: Option<String>,
}

impl Default for SingboxDnsServer {
    fn default() -> Self {
        Self {
            r#type: "udp".to_string(),
            tag: String::new(),
            inet4_range: None,
            inet6_range: None,
            client_subnet: None,
            server: None,
            domain_resolver: None,
            interface_name: None,
            server_port: None,
            path: None,
            headers: None,
            predefined: None,
            detour: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxRuleset {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_interval: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outbound: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_cache: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_subnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite_ttl: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clash_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbound: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geosite: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_suffix: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_keyword: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_regex: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geoip: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_cidr: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip_cidr: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_is_private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_name: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_path: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<SingboxRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniffer: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rcode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_type: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_drop: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip_is_private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_accept_any: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_port_range: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_type: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_is_expensive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_is_constrained: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wifi_ssid: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wifi_bssid: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_set_ip_cidr_match_source: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_set_ip_cidr_accept_empty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxInbound {
    pub r#type: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_route: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_route: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_independent_nat: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<SingboxUser>>,
}

impl Default for SingboxInbound {
    fn default() -> Self {
        Self {
            r#type: "mixed".to_string(),
            tag: "socks".to_string(),
            listen: Some(LOOPBACK.to_string()),
            listen_port: None,
            interface_name: None,
            address: None,
            mtu: None,
            auto_route: None,
            strict_route: None,
            endpoint_independent_nat: None,
            stack: None,
            users: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxUser {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxOutbound {
    pub r#type: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_mbps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down_mbps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion_control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quic_congestion_control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insecure_concurrency: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_over_tcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet_encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_opts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outbounds: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt_exist_connections: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_interface: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inet4_bind_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<SingboxTls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplex: Option<SingboxMultiplex>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<SingboxTransport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfs: Option<SingboxHyObfs>,
}

impl Default for SingboxOutbound {
    fn default() -> Self {
        Self {
            r#type: "vless".to_string(),
            tag: PROXY_TAG.to_string(),
            server: None,
            server_port: None,
            server_ports: None,
            uuid: None,
            security: None,
            alter_id: None,
            flow: None,
            hop_interval: None,
            up_mbps: None,
            down_mbps: None,
            password: None,
            method: None,
            username: None,
            version: None,
            congestion_control: None,
            quic: None,
            quic_congestion_control: None,
            insecure_concurrency: None,
            udp_over_tcp: None,
            packet_encoding: None,
            plugin: None,
            plugin_opts: None,
            outbounds: None,
            interrupt_exist_connections: None,
            tolerance: None,
            detour: None,
            bind_interface: None,
            inet4_bind_address: None,
            tls: None,
            multiplex: None,
            transport: None,
            obfs: None,
        }
    }
}

impl SingboxOutbound {
    fn direct() -> Self {
        Self {
            r#type: DIRECT_TAG.to_string(),
            tag: DIRECT_TAG.to_string(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxEndpoint {
    pub r#type: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<String>,
    pub private_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub peers: Vec<SingboxPeer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_interface: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inet4_bind_address: Option<String>,
}

impl Default for SingboxEndpoint {
    fn default() -> Self {
        Self {
            r#type: "wireguard".to_string(),
            tag: PROXY_TAG.to_string(),
            system: None,
            name: None,
            mtu: None,
            address: Vec::new(),
            private_key: String::new(),
            listen_port: None,
            udp_timeout: None,
            workers: None,
            peers: Vec::new(),
            detour: None,
            bind_interface: None,
            inet4_bind_address: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxPeer {
    pub address: String,
    pub port: i32,
    pub public_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_shared_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_ips: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent_keepalive_interval: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved: Option<Vec<i32>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxTls {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insecure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utls: Option<SingboxUtls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality: Option<SingboxReality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_fragment: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ech: Option<SingboxEch>,
}

impl Default for SingboxTls {
    fn default() -> Self {
        Self {
            enabled: true,
            server_name: None,
            insecure: None,
            alpn: None,
            utls: None,
            reality: None,
            record_fragment: None,
            certificate: None,
            ech: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxEch {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_server_name: Option<String>,
}

impl Default for SingboxEch {
    fn default() -> Self {
        Self {
            enabled: true,
            config: None,
            query_server_name: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxMultiplex {
    pub enabled: bool,
    pub protocol: String,
    pub max_connections: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<bool>,
}

impl Default for SingboxMultiplex {
    fn default() -> Self {
        Self {
            enabled: true,
            protocol: "h2mux".to_string(),
            max_connections: 8,
            padding: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxUtls {
    pub enabled: bool,
    pub fingerprint: String,
}

impl Default for SingboxUtls {
    fn default() -> Self {
        Self {
            enabled: true,
            fingerprint: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxReality {
    pub enabled: bool,
    pub public_key: String,
    pub short_id: String,
}

impl Default for SingboxReality {
    fn default() -> Self {
        Self {
            enabled: true,
            public_key: String::new(),
            short_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxTransport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<SingboxHeaders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permit_without_stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_early_data: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub early_data_header_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SingboxHeaders {
    #[serde(rename = "Host", skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(rename = "User-Agent", skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxHyObfs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxExperimental {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_file: Option<SingboxCacheFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clash_api: Option<SingboxClashApi>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxClashApi {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_controller: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_selected: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxCacheFile {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_fakeip: Option<bool>,
}

impl Default for SingboxCacheFile {
    fn default() -> Self {
        Self {
            enabled: true,
            path: None,
            cache_id: None,
            store_fakeip: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum SingboxServer {
    Outbound(Box<SingboxOutbound>),
    Endpoint(Box<SingboxEndpoint>),
}

impl SingboxServer {
    fn tag(&self) -> &str {
        match self {
            Self::Outbound(outbound) => &outbound.tag,
            Self::Endpoint(endpoint) => &endpoint.tag,
        }
    }

    fn set_tag(&mut self, tag: String) {
        match self {
            Self::Outbound(outbound) => outbound.tag = tag,
            Self::Endpoint(endpoint) => endpoint.tag = tag,
        }
    }

    fn detour(&self) -> Option<&str> {
        match self {
            Self::Outbound(outbound) => outbound.detour.as_deref(),
            Self::Endpoint(endpoint) => endpoint.detour.as_deref(),
        }
    }

    fn set_detour(&mut self, detour: &str) {
        match self {
            Self::Outbound(outbound) => outbound.detour = Some(detour.to_string()),
            Self::Endpoint(endpoint) => endpoint.detour = Some(detour.to_string()),
        }
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.tag().starts_with(prefix)
    }
}

pub fn generate_singbox_config(
    context: &CoreConfigContext,
) -> Result<SingboxConfig, SingboxConfigError> {
    validate_proxy_ports(context)?;
    validate_active_wireguard(context)?;
    let mut config = SingboxConfig::sample();
    gen_log(&mut config, context);
    gen_inbounds(&mut config, context);
    gen_outbounds(&mut config, context);
    gen_routing(&mut config, context);
    gen_dns(&mut config, context);
    gen_experimental(&mut config, context);
    convert_geo_to_ruleset(&mut config, context)?;
    apply_outbound_bind_interface(&mut config, context);
    apply_outbound_send_through(&mut config, context);
    Ok(config)
}

pub fn generate_singbox_config_value(
    context: &CoreConfigContext,
) -> Result<Value, SingboxConfigError> {
    let config = generate_singbox_config(context)?;
    Ok(apply_full_config_template(context, &config))
}

pub fn generate_singbox_config_json(
    context: &CoreConfigContext,
) -> Result<String, SingboxConfigError> {
    let value = generate_singbox_config_value(context)?;
    serde_json::to_string_pretty(&value).map_err(SingboxConfigError::Serialize)
}

fn validate_active_wireguard(context: &CoreConfigContext) -> Result<(), SingboxConfigError> {
    if context.node.config_type == ConfigType::WireGuard
        && wireguard_public_key(&context.node.protocol_extra).is_none()
    {
        return Err(SingboxConfigError::MissingWireGuardPublicKey {
            remarks: context.node.remarks.clone(),
        });
    }
    Ok(())
}

fn validate_proxy_ports(context: &CoreConfigContext) -> Result<(), SingboxConfigError> {
    let mut pending = vec![context.node.clone()];
    let mut seen = BTreeSet::new();

    while let Some(node) = pending.pop() {
        if !node.index_id.is_empty() && !seen.insert(node.index_id.clone()) {
            continue;
        }
        if node.config_type == ConfigType::Custom {
            continue;
        }
        if node.config_type.is_group_type() {
            if let Some(child_ids) = split_list(
                node.protocol_extra
                    .child_items
                    .as_deref()
                    .unwrap_or_default(),
            ) {
                pending.extend(
                    child_ids
                        .into_iter()
                        .filter_map(|node_id| context.all_proxies_map.get(&node_id).cloned()),
                );
            }
            continue;
        }
        if !(1..=65535).contains(&node.port) {
            return Err(SingboxConfigError::InvalidNodePort {
                remarks: node.remarks,
                port: node.port,
            });
        }
    }

    Ok(())
}

fn gen_log(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let mut log = config.log.clone().unwrap_or_default();
    log.level = match context.app_config.core_basic_item.loglevel.as_str() {
        "debug" | "info" | "error" => context.app_config.core_basic_item.loglevel.clone(),
        "warning" => "warn".to_string(),
        _ => log.level,
    };
    if context.app_config.core_basic_item.loglevel == "none" {
        log.disabled = Some(true);
    }
    if context.app_config.core_basic_item.log_enabled {
        log.output = Some("sbox.log".to_string());
    }
    config.log = Some(log);
}

fn gen_inbounds(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let in_item = context
        .app_config
        .inbound
        .first()
        .cloned()
        .unwrap_or_default();
    let listen_port = inbound_port(&context.app_config, InboundProtocol::socks);
    let is_using_local_mixed_port =
        context.node.address == LOOPBACK && context.node.port == listen_port;

    config.inbounds.clear();
    if !context.is_tun_enabled || !is_using_local_mixed_port {
        let mut primary = build_mixed_inbound(&in_item, InboundProtocol::socks);
        if in_item.allow_lan_conn && !in_item.new_port4_lan {
            primary.listen = Some("0.0.0.0".to_string());
        }
        config.inbounds.push(primary.clone());

        if in_item.second_local_port_enabled {
            config
                .inbounds
                .push(build_mixed_inbound(&in_item, InboundProtocol::socks2));
        }

        if in_item.allow_lan_conn && in_item.new_port4_lan {
            let mut lan = build_mixed_inbound(&in_item, InboundProtocol::socks3);
            lan.listen = Some("0.0.0.0".to_string());
            if !trimmed(&in_item.user).is_empty() && !trimmed(&in_item.pass).is_empty() {
                lan.users = Some(vec![SingboxUser {
                    username: in_item.user.clone(),
                    password: in_item.pass.clone(),
                }]);
            }
            config.inbounds.push(lan);
        }
    }

    if context.is_tun_enabled {
        config.inbounds.push(build_tun_inbound(context));
    }
}

fn build_mixed_inbound(in_item: &InItem, protocol: InboundProtocol) -> SingboxInbound {
    SingboxInbound {
        r#type: "mixed".to_string(),
        tag: inbound_protocol_tag(protocol).to_string(),
        listen: Some(LOOPBACK.to_string()),
        listen_port: Some(in_item.local_port + protocol.as_i32()),
        ..SingboxInbound::default()
    }
}

fn build_tun_inbound(context: &CoreConfigContext) -> SingboxInbound {
    let mtu = if context.app_config.tun_mode_item.mtu > 0 {
        context.app_config.tun_mode_item.mtu
    } else {
        WIREGUARD_DEFAULT_MTU
    };
    let address = if context.app_config.tun_mode_item.enable_ipv6_address {
        vec![
            "172.18.0.1/30".to_string(),
            "fdfe:dcba:9876::1/126".to_string(),
        ]
    } else {
        vec!["172.18.0.1/30".to_string()]
    };
    let stack = nonempty_str(Some(&context.app_config.tun_mode_item.stack))
        .unwrap_or(DEFAULT_TUN_STACK)
        .to_string();

    SingboxInbound {
        r#type: "tun".to_string(),
        tag: SINGBOX_TUN_INBOUND_TAG.to_string(),
        listen: None,
        listen_port: None,
        interface_name: Some(if context.is_macos() {
            "utun0".to_string()
        } else {
            "singbox_tun".to_string()
        }),
        address: Some(address),
        mtu: Some(mtu),
        auto_route: Some(context.app_config.tun_mode_item.auto_route),
        strict_route: Some(context.app_config.tun_mode_item.strict_route),
        stack: Some(stack),
        ..SingboxInbound::default()
    }
}

fn gen_outbounds(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let servers = build_all_proxy_servers(context, &context.node, PROXY_TAG, true);
    prepend_servers(config, servers);
}

fn build_all_proxy_servers(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
    with_selector: bool,
) -> Vec<SingboxServer> {
    let mut proxy_servers = if node.config_type.is_group_type() {
        build_group_proxy_servers(context, node, base_tag_name)
    } else {
        build_proxy_server(context, node, base_tag_name)
            .into_iter()
            .collect()
    };

    if with_selector {
        let proxy_tags = ordered_proxy_tags(&proxy_servers, base_tag_name);
        if proxy_tags.len() > 1 {
            let mut selectors = build_selector_servers(node, &proxy_tags, base_tag_name);
            selectors.extend(proxy_servers);
            proxy_servers = selectors;
        }
    }

    proxy_servers
}

fn build_proxy_server(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Option<SingboxServer> {
    if node.config_type == ConfigType::WireGuard {
        let mut endpoint = build_wireguard_endpoint(node)?;
        endpoint.tag = base_tag_name.to_string();
        return Some(SingboxServer::Endpoint(Box::new(endpoint)));
    }

    let mut outbound = build_outbound(context, node);
    outbound.tag = base_tag_name.to_string();
    Some(SingboxServer::Outbound(Box::new(outbound)))
}

fn build_group_proxy_servers(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<SingboxServer> {
    match node.config_type {
        ConfigType::PolicyGroup => build_outbounds_list(context, node, base_tag_name),
        ConfigType::ProxyChain => build_chain_outbounds_list(context, node, base_tag_name),
        _ => Vec::new(),
    }
}

fn build_outbounds_list(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<SingboxServer> {
    let nodes = buildable_child_nodes(context, node);
    let mut result: Vec<SingboxServer> = Vec::new();

    for (index, child_node) in nodes.iter().enumerate() {
        let current_tag = if nodes.len() == 1 {
            base_tag_name.to_string()
        } else {
            format!("{base_tag_name}-{}-{}", index + 1, child_node.remarks)
        };

        if child_node.config_type.is_group_type() {
            result.extend(build_group_proxy_servers(context, child_node, &current_tag));
            continue;
        }

        if let Some(server) = build_proxy_server(context, child_node, &current_tag) {
            result.push(server);
        }
    }

    result
}

fn build_chain_outbounds_list(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<SingboxServer> {
    let nodes = buildable_child_nodes(context, node);
    let nodes_reverse = nodes.into_iter().rev().collect::<Vec<_>>();
    let mut result: Vec<SingboxServer> = Vec::new();

    for (index, child_node) in nodes_reverse.iter().enumerate() {
        let current_tag = if index == 0 {
            base_tag_name.to_string()
        } else {
            format!("chain-{base_tag_name}-{index}-{}", child_node.remarks)
        };
        let detour_tag = (index != nodes_reverse.len().saturating_sub(1)).then(|| {
            format!(
                "chain-{base_tag_name}-{}-{}",
                index + 1,
                nodes_reverse[index + 1].remarks
            )
        });

        if child_node.config_type.is_group_type() {
            let mut child_profiles = build_group_proxy_servers(context, child_node, &current_tag);
            if let Some(detour_tag) = detour_tag.as_deref() {
                for server in child_profiles
                    .iter_mut()
                    .filter(|server| server.detour().is_none_or(str::is_empty))
                {
                    server.set_detour(detour_tag);
                }
            }

            if index != 0 {
                let chain_start_nodes = child_profiles
                    .iter()
                    .filter(|server| server.starts_with(&current_tag))
                    .cloned()
                    .collect::<Vec<_>>();
                if chain_start_nodes.len() == 1 {
                    let first_chain_tag = chain_start_nodes[0].tag().to_string();
                    for server in &mut result {
                        if server.detour() == Some(current_tag.as_str()) {
                            server.set_detour(&first_chain_tag);
                        }
                    }
                } else if chain_start_nodes.len() > 1 {
                    let existed_chain_nodes = result.clone();
                    result.clear();
                    for (branch_index, chain_start_node) in chain_start_nodes.iter().enumerate() {
                        let mut existed_chain_nodes_clone = existed_chain_nodes.clone();
                        for existed_chain_node in &mut existed_chain_nodes_clone {
                            existed_chain_node.set_tag(format!(
                                "{}-clone-{}",
                                existed_chain_node.tag(),
                                branch_index + 1
                            ));
                        }
                        for chain_index in 0..existed_chain_nodes_clone.len() {
                            let previous_detour = existed_chain_nodes_clone[chain_index]
                                .detour()
                                .map(str::to_string);
                            let next_tag = if chain_index + 1 < existed_chain_nodes_clone.len() {
                                existed_chain_nodes_clone[chain_index + 1].tag().to_string()
                            } else {
                                chain_start_node.tag().to_string()
                            };
                            let next_detour =
                                if previous_detour.as_deref() == Some(current_tag.as_str()) {
                                    chain_start_node.tag()
                                } else {
                                    &next_tag
                                };
                            existed_chain_nodes_clone[chain_index].set_detour(next_detour);
                            result.push(existed_chain_nodes_clone[chain_index].clone());
                        }
                    }
                }
            }

            result.extend(child_profiles);
            continue;
        }

        let Some(mut outbound) = build_proxy_server(context, child_node, &current_tag) else {
            continue;
        };
        if let Some(detour_tag) = detour_tag {
            outbound.set_detour(&detour_tag);
        }
        result.push(outbound);
    }

    result
}

fn build_outbound(context: &CoreConfigContext, node: &ProfileItem) -> SingboxOutbound {
    let protocol_extra = &node.protocol_extra;
    let transport_extra = &node.transport_extra;
    let network = singbox_network(node);
    let mut outbound = SingboxOutbound {
        r#type: protocol_name(node.config_type).to_string(),
        tag: PROXY_TAG.to_string(),
        server: Some(node.address.clone()),
        server_port: Some(node.port),
        ..SingboxOutbound::default()
    };

    match node.config_type {
        ConfigType::VMess => {
            outbound.uuid = Some(node.password.clone());
            outbound.alter_id = Some(parse_i32(protocol_extra.alter_id.as_deref()).unwrap_or(0));
            outbound.security = Some(vmess_security(protocol_extra));
            fill_outbound_mux(&mut outbound, context, node);
            fill_outbound_transport(&mut outbound, context, node, &network, transport_extra);
        }
        ConfigType::Shadowsocks => {
            outbound.method = Some(shadowsocks_method(protocol_extra));
            outbound.password = Some(node.password.clone());
            outbound.udp_over_tcp = (protocol_extra.uot == Some(true)).then_some(true);
            fill_shadowsocks_plugin(&mut outbound, node, &network, transport_extra);
            fill_outbound_mux(&mut outbound, context, node);
        }
        ConfigType::SOCKS => {
            outbound.version = Some("5".to_string());
            if !trimmed(&node.username).is_empty() && !trimmed(&node.password).is_empty() {
                outbound.username = Some(node.username.clone());
                outbound.password = Some(node.password.clone());
            }
        }
        ConfigType::HTTP => {
            if !trimmed(&node.username).is_empty() && !trimmed(&node.password).is_empty() {
                outbound.username = Some(node.username.clone());
                outbound.password = Some(node.password.clone());
            }
        }
        ConfigType::VLESS => {
            outbound.uuid = Some(node.password.clone());
            outbound.packet_encoding = Some("xudp".to_string());
            if let Some(flow) = nonempty_string(protocol_extra.flow.as_deref()) {
                outbound.flow = Some(flow);
            } else {
                fill_outbound_mux(&mut outbound, context, node);
            }
            fill_outbound_transport(&mut outbound, context, node, &network, transport_extra);
        }
        ConfigType::Trojan => {
            outbound.password = Some(node.password.clone());
            fill_outbound_mux(&mut outbound, context, node);
            fill_outbound_transport(&mut outbound, context, node, &network, transport_extra);
        }
        ConfigType::Hysteria2 => {
            outbound.password = Some(node.password.clone());
            fill_hysteria2_fields(&mut outbound, context, protocol_extra);
        }
        ConfigType::TUIC => {
            outbound.uuid = nonempty_string(Some(&node.username));
            outbound.password = Some(node.password.clone());
            outbound.congestion_control =
                nonempty_string(protocol_extra.congestion_control.as_deref());
        }
        ConfigType::Anytls => {
            outbound.password = Some(node.password.clone());
        }
        ConfigType::Naive => {
            outbound.username = nonempty_string(Some(&node.username));
            outbound.password = Some(node.password.clone());
            if protocol_extra.naive_quic == Some(true) {
                outbound.quic = Some(true);
                outbound.quic_congestion_control =
                    nonempty_string(protocol_extra.congestion_control.as_deref());
            }
            outbound.insecure_concurrency = protocol_extra
                .insecure_concurrency
                .filter(|value| *value > 0);
            outbound.udp_over_tcp = (protocol_extra.uot == Some(true)).then_some(true);
        }
        ConfigType::WireGuard
        | ConfigType::Custom
        | ConfigType::PolicyGroup
        | ConfigType::ProxyChain => {}
    }

    fill_outbound_tls(&mut outbound, context, node);
    outbound
}

fn build_wireguard_endpoint(node: &ProfileItem) -> Option<SingboxEndpoint> {
    let protocol_extra = &node.protocol_extra;
    let public_key = wireguard_public_key(protocol_extra)?;
    Some(SingboxEndpoint {
        r#type: protocol_name(node.config_type).to_string(),
        tag: PROXY_TAG.to_string(),
        address: split_list(
            protocol_extra
                .wg_interface_address
                .as_deref()
                .unwrap_or_default(),
        )
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![WIREGUARD_DEFAULT_ADDRESS.to_string()]),
        private_key: node.password.clone(),
        mtu: Some(
            protocol_extra
                .wg_mtu
                .filter(|mtu| *mtu > 0)
                .unwrap_or(WIREGUARD_DEFAULT_MTU),
        ),
        peers: vec![SingboxPeer {
            address: node.address.clone(),
            port: node.port,
            public_key,
            pre_shared_key: protocol_extra.wg_preshared_key.clone(),
            allowed_ips: wireguard_allowed_ips(protocol_extra),
            reserved: parse_i32_list(protocol_extra.wg_reserved.as_deref()),
            persistent_keepalive_interval: None,
        }],
        ..SingboxEndpoint::default()
    })
}

fn fill_shadowsocks_plugin(
    outbound: &mut SingboxOutbound,
    node: &ProfileItem,
    network: &str,
    transport_extra: &TransportExtraItem,
) {
    if network == DEFAULT_NETWORK
        && transport_extra.raw_header_type.as_deref() == Some(RAW_HEADER_HTTP)
    {
        outbound.plugin = Some("obfs-local".to_string());
        outbound.plugin_opts = Some(format!(
            "obfs=http;obfs-host={};",
            transport_extra.host.as_deref().unwrap_or_default()
        ));
        return;
    }

    let mut plugin_args = String::new();
    if network == "ws" {
        plugin_args.push_str("mode=websocket;");
        plugin_args.push_str(&format!(
            "host={};",
            first_list_value(transport_extra.host.as_deref())
        ));
        let path = transport_extra
            .path
            .as_deref()
            .unwrap_or_default()
            .replace('\\', "\\\\")
            .replace('=', "\\=")
            .replace(',', "\\,");
        plugin_args.push_str(&format!("path={path};"));
    }
    if node.stream_security == STREAM_SECURITY_TLS {
        plugin_args.push_str("tls;");
        let certs = parse_pem_chain(&node.cert);
        if let Some(cert) = certs.first() {
            let base64_content = cert
                .replace("-----BEGIN CERTIFICATE-----\n", "")
                .replace("\n-----END CERTIFICATE-----\n", "")
                .trim()
                .replace('=', "\\=");
            plugin_args.push_str(&format!("certRaw={base64_content};"));
        }
    }
    if !plugin_args.is_empty() {
        plugin_args.push_str("mux=0;");
        plugin_args.pop();
        outbound.plugin = Some("v2ray-plugin".to_string());
        outbound.plugin_opts = Some(plugin_args);
    }
}

fn fill_hysteria2_fields(
    outbound: &mut SingboxOutbound,
    context: &CoreConfigContext,
    protocol_extra: &ProtocolExtraItem,
) {
    if let Some(salamander_pass) = nonempty_str(protocol_extra.salamander_pass.as_deref()) {
        outbound.obfs = Some(SingboxHyObfs {
            r#type: Some("salamander".to_string()),
            password: Some(salamander_pass.to_string()),
        });
    }

    let up_mbps = protocol_extra
        .up_mbps
        .filter(|value| *value >= 0)
        .unwrap_or(context.app_config.hysteria_item.up_mbps);
    let down_mbps = protocol_extra
        .down_mbps
        .filter(|value| *value >= 0)
        .unwrap_or(context.app_config.hysteria_item.down_mbps);
    outbound.up_mbps = (up_mbps > 0).then_some(up_mbps);
    outbound.down_mbps = (down_mbps > 0).then_some(down_mbps);

    let Some(ports) = nonempty_str(protocol_extra.ports.as_deref()) else {
        return;
    };
    if !ports.contains([':', '-', ',']) {
        return;
    }

    let server_ports = ports
        .split(',')
        .map(str::trim)
        .filter(|port| !port.is_empty())
        .map(|port| {
            let port = port.replace('-', ":");
            if port.contains(':') {
                port
            } else {
                format!("{port}:{port}")
            }
        })
        .collect::<Vec<_>>();
    if !server_ports.is_empty() {
        outbound.server_port = None;
        outbound.server_ports = Some(server_ports);
    }

    let default_interval = if context.app_config.hysteria_item.hop_interval >= 5 {
        context.app_config.hysteria_item.hop_interval
    } else {
        DEFAULT_HYSTERIA2_HOP_INTERVAL
    };
    let interval = protocol_extra
        .hop_interval
        .as_deref()
        .and_then(parse_hysteria_hop_interval)
        .filter(|value| *value >= 5)
        .unwrap_or(default_interval);
    outbound.hop_interval = Some(format!("{interval}s"));
}

fn parse_hysteria_hop_interval(value: &str) -> Option<i32> {
    let value = value.trim();
    if let Ok(value) = value.parse::<i32>() {
        return Some(value);
    }
    let (left, right) = value.split_once('-')?;
    let left = left.trim().parse::<i32>().ok()?;
    let right = right.trim().parse::<i32>().ok()?;
    Some((left + right) / 2)
}

fn fill_outbound_mux(
    outbound: &mut SingboxOutbound,
    context: &CoreConfigContext,
    node: &ProfileItem,
) {
    let mux_enabled = node
        .mux_enabled
        .unwrap_or(context.app_config.core_basic_item.mux_enabled);
    if !mux_enabled {
        return;
    }
    let protocol = trimmed(&context.app_config.mux4_sbox_item.protocol);
    if protocol.is_empty() {
        return;
    }
    outbound.multiplex = Some(SingboxMultiplex {
        enabled: true,
        protocol: protocol.to_string(),
        max_connections: context.app_config.mux4_sbox_item.max_connections,
        padding: context.app_config.mux4_sbox_item.padding,
    });
}

fn fill_outbound_transport(
    outbound: &mut SingboxOutbound,
    context: &CoreConfigContext,
    node: &ProfileItem,
    network: &str,
    transport_extra: &TransportExtraItem,
) {
    let user_agent = raw_http_user_agent(&context.app_config.core_basic_item.def_user_agent);
    let mut transport = SingboxTransport::default();

    match network {
        DEFAULT_NETWORK => {
            if transport_extra.raw_header_type.as_deref() == Some(RAW_HEADER_HTTP) {
                transport.r#type = Some("http".to_string());
                transport.host = split_list(transport_extra.host.as_deref().unwrap_or_default())
                    .filter(|items| !items.is_empty())
                    .map(|items| json!(items));
                transport.path = nonempty_string(transport_extra.path.as_deref());
                if !user_agent.is_empty() {
                    transport.headers = Some(SingboxHeaders {
                        host: None,
                        user_agent: Some(user_agent),
                    });
                }
            }
        }
        "ws" => {
            transport.r#type = Some("ws".to_string());
            let mut ws_path = transport_extra.path.clone().unwrap_or_default();
            let (path, early_data, early_header) = parse_ws_early_data(&ws_path);
            ws_path = path;
            transport.path = nonempty_string(Some(&ws_path));
            transport.max_early_data = early_data;
            transport.early_data_header_name = early_header;
            let host = first_list_value(transport_extra.host.as_deref());
            if !host.is_empty() || !user_agent.is_empty() {
                transport.headers = Some(SingboxHeaders {
                    host: nonempty_string(Some(&host)),
                    user_agent: nonempty_string(Some(&user_agent)),
                });
            }
        }
        "httpupgrade" => {
            transport.r#type = Some("httpupgrade".to_string());
            transport.path = nonempty_string(transport_extra.path.as_deref());
            let host = first_list_value(transport_extra.host.as_deref());
            transport.host = nonempty_string(Some(&host)).map(Value::String);
            if !user_agent.is_empty() {
                transport.headers = Some(SingboxHeaders {
                    host: None,
                    user_agent: Some(user_agent),
                });
            }
        }
        "grpc" => {
            transport.r#type = Some("grpc".to_string());
            transport.service_name = Some(
                transport_extra
                    .grpc_service_name
                    .clone()
                    .unwrap_or_default(),
            );
            transport.idle_timeout = context
                .app_config
                .grpc_item
                .idle_timeout
                .map(|value| format!("{value}s"));
            transport.ping_timeout = context
                .app_config
                .grpc_item
                .health_check_timeout
                .map(|value| format!("{value}s"));
            transport.permit_without_stream = context.app_config.grpc_item.permit_without_stream;
        }
        _ => {}
    }

    if transport.r#type.is_some() {
        outbound.transport = Some(transport);
    }

    if node.config_type == ConfigType::Shadowsocks {
        outbound.transport = None;
    }
}

fn parse_ws_early_data(path: &str) -> (String, Option<i32>, Option<String>) {
    let mut result_path = path.to_string();
    let mut early_data = None;
    let mut early_header = None;

    if let Ok(ed_regex) = Regex::new(r"[?&]ed=(\d+)") {
        if let Some(captures) = ed_regex.captures(&result_path) {
            early_data = captures
                .get(1)
                .and_then(|value| value.as_str().parse::<i32>().ok());
            if early_data.is_some() {
                early_header = Some(USER_AGENT_HEADER.to_string());
                result_path = ed_regex.replace(&result_path, "").to_string();
                result_path = result_path.replace("?&", "?");
                if result_path.ends_with('?') {
                    result_path.pop();
                }
            }
        }
    }

    if let Ok(eh_regex) = Regex::new(r"[?&]eh=([^&]+)") {
        if let Some(captures) = eh_regex.captures(&result_path) {
            if let Some(value) = captures.get(1) {
                early_header = percent_encoding::percent_decode_str(value.as_str())
                    .decode_utf8()
                    .ok()
                    .map(|value| value.to_string());
            }
        }
    }

    (result_path, early_data, early_header)
}

fn fill_outbound_tls(
    outbound: &mut SingboxOutbound,
    context: &CoreConfigContext,
    node: &ProfileItem,
) {
    if !matches!(
        node.stream_security.as_str(),
        STREAM_SECURITY_TLS | STREAM_SECURITY_REALITY
    ) || matches!(
        node.config_type,
        ConfigType::Shadowsocks | ConfigType::SOCKS | ConfigType::WireGuard
    ) {
        return;
    }

    let transport_host = transport_host_for_tls(node);
    let server_name = nonempty_string(Some(&node.sni)).or_else(|| {
        split_list(transport_host.as_deref().unwrap_or_default()).and_then(|items| {
            items
                .into_iter()
                .map(|item| item.trim().to_string())
                .find(|item| !item.is_empty())
        })
    });
    let mut tls = SingboxTls {
        enabled: true,
        server_name,
        insecure: Some(allow_insecure(node, context)),
        alpn: split_list(&node.alpn).filter(|items| !items.is_empty()),
        record_fragment: context
            .app_config
            .core_basic_item
            .enable_fragment
            .then_some(true),
        ech: parse_ech(&node.ech_config_list),
        ..SingboxTls::default()
    };

    if let Some(fingerprint) = effective_fingerprint(node, context) {
        tls.utls = Some(SingboxUtls {
            enabled: true,
            fingerprint,
        });
    }

    if node.stream_security == STREAM_SECURITY_TLS {
        let certs = parse_pem_chain(&node.cert);
        if !certs.is_empty() {
            tls.certificate = Some(certs);
            tls.insecure = Some(false);
        }
    } else if node.stream_security == STREAM_SECURITY_REALITY {
        tls.reality = Some(SingboxReality {
            enabled: true,
            public_key: node.public_key.clone(),
            short_id: node.short_id.clone(),
        });
        tls.insecure = Some(false);
    }

    outbound.tls = Some(tls);
}

fn parse_ech(ech_config: &str) -> Option<SingboxEch> {
    let ech_config = ech_config.trim();
    if ech_config.is_empty() {
        return None;
    }
    if !ech_config.contains("://") {
        return Some(SingboxEch {
            enabled: true,
            config: Some(vec![format!(
                "-----BEGIN ECH CONFIGS-----\n{ech_config}\n-----END ECH CONFIGS-----"
            )]),
            query_server_name: None,
        });
    }

    let query_server_name = ech_config
        .split_once('+')
        .map(|(query_server_name, _)| query_server_name)
        .and_then(|value| nonempty_string(Some(value)));

    Some(SingboxEch {
        enabled: true,
        config: None,
        query_server_name,
    })
}

fn build_selector_servers(
    node: &ProfileItem,
    proxy_tags: &[String],
    base_tag_name: &str,
) -> Vec<SingboxServer> {
    let multiple_load = node
        .protocol_extra
        .multiple_load
        .unwrap_or(MultipleLoad::LeastPing);
    let auto_tag = format!("{base_tag_name}-auto");
    let out_urltest = SingboxOutbound {
        r#type: "urltest".to_string(),
        tag: auto_tag.clone(),
        outbounds: Some(proxy_tags.to_vec()),
        interrupt_exist_connections: Some(false),
        tolerance: (multiple_load == MultipleLoad::Fallback).then_some(5000),
        ..SingboxOutbound::default()
    };
    let mut selector_outbounds = proxy_tags.to_vec();
    selector_outbounds.insert(0, auto_tag);
    let out_selector = SingboxOutbound {
        r#type: "selector".to_string(),
        tag: base_tag_name.to_string(),
        outbounds: Some(selector_outbounds),
        interrupt_exist_connections: Some(false),
        ..SingboxOutbound::default()
    };

    vec![
        SingboxServer::Outbound(Box::new(out_selector)),
        SingboxServer::Outbound(Box::new(out_urltest)),
    ]
}

fn ordered_proxy_tags(servers: &[SingboxServer], base_tag_name: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut tags = Vec::new();
    for server in servers {
        let tag = server.tag();
        if tag.starts_with(base_tag_name) && seen.insert(tag.to_string()) {
            tags.push(tag.to_string());
        }
    }
    tags
}

fn prepend_servers(config: &mut SingboxConfig, servers: Vec<SingboxServer>) {
    let mut outbounds = Vec::new();
    let mut endpoints = Vec::new();
    for server in servers {
        match server {
            SingboxServer::Outbound(outbound) => outbounds.push(*outbound),
            SingboxServer::Endpoint(endpoint) => endpoints.push(*endpoint),
        }
    }
    config.outbounds.splice(0..0, outbounds);
    config.endpoints.splice(0..0, endpoints);
}

fn append_servers(config: &mut SingboxConfig, servers: Vec<SingboxServer>) {
    for server in servers {
        match server {
            SingboxServer::Outbound(outbound) => config.outbounds.push(*outbound),
            SingboxServer::Endpoint(endpoint) => config.endpoints.push(*endpoint),
        }
    }
}

fn gen_routing(config: &mut SingboxConfig, context: &CoreConfigContext) {
    config.route.final_outbound = Some(PROXY_TAG.to_string());
    let simple_dns = &context.simple_dns_item;
    let raw_dns_enabled = context
        .raw_dns_item
        .as_ref()
        .is_some_and(|item| item.enabled);
    let default_domain_resolver_tag = if raw_dns_enabled {
        SINGBOX_LOCAL_DNS_TAG
    } else {
        SINGBOX_DIRECT_DNS_TAG
    };
    let direct_dns_strategy = if raw_dns_enabled {
        context
            .raw_dns_item
            .as_ref()
            .and_then(|item| nonempty_string(item.domain_strategy4_freedom.as_deref()))
    } else {
        domain_strategy4_sbox(simple_dns.strategy4_freedom.as_deref())
    };
    config.route.default_domain_resolver = Some(SingboxRule {
        server: Some(default_domain_resolver_tag.to_string()),
        strategy: direct_dns_strategy,
        ..SingboxRule::default()
    });

    if context.is_tun_enabled {
        config.route.auto_detect_interface = Some(true);
        config.route.rules.extend(tun_route_rules());

        config.route.rules.push(SingboxRule {
            port: Some(vec![53]),
            action: Some("hijack-dns".to_string()),
            process_name: Some(tun_dns_process_names()),
            ..SingboxRule::default()
        });
        config.route.rules.push(SingboxRule {
            outbound: Some(DIRECT_TAG.to_string()),
            process_name: Some(tun_direct_process_names()),
            ..SingboxRule::default()
        });
        match tun_icmp_routing(&context.app_config.tun_mode_item.icmp_routing) {
            "direct" => config.route.rules.push(SingboxRule {
                network: Some(vec!["icmp".to_string()]),
                outbound: Some(DIRECT_TAG.to_string()),
                ..SingboxRule::default()
            }),
            "unreachable" | "drop" | "reply" => {
                let method = match tun_icmp_routing(&context.app_config.tun_mode_item.icmp_routing)
                {
                    "unreachable" => "default",
                    "drop" => "drop",
                    _ => "reply",
                };
                config.route.rules.push(SingboxRule {
                    network: Some(vec!["icmp".to_string()]),
                    action: Some("reject".to_string()),
                    method: Some(method.to_string()),
                    ..SingboxRule::default()
                });
            }
            _ => {}
        }
    }

    if context
        .app_config
        .inbound
        .first()
        .is_none_or(|inbound| inbound.sniffing_enabled)
    {
        config.route.rules.push(SingboxRule {
            action: Some("sniff".to_string()),
            ..SingboxRule::default()
        });
        config.route.rules.push(SingboxRule {
            r#type: Some("logical".to_string()),
            mode: Some("or".to_string()),
            action: Some("hijack-dns".to_string()),
            rules: Some(vec![
                SingboxRule {
                    port: Some(vec![53]),
                    ..SingboxRule::default()
                },
                SingboxRule {
                    protocol: Some(vec!["dns".to_string()]),
                    ..SingboxRule::default()
                },
            ]),
            ..SingboxRule::default()
        });
    } else {
        config.route.rules.push(SingboxRule {
            port: Some(vec![53]),
            action: Some("hijack-dns".to_string()),
            ..SingboxRule::default()
        });
    }

    if !raw_dns_enabled {
        if let Some(hosts_resolve_rule) = hosts_resolve_rule(simple_dns) {
            config.route.rules.push(hosts_resolve_rule);
        }
    }

    config.route.rules.push(SingboxRule {
        outbound: Some(DIRECT_TAG.to_string()),
        clash_mode: Some("Direct".to_string()),
        ..SingboxRule::default()
    });
    config.route.rules.push(SingboxRule {
        outbound: Some(PROXY_TAG.to_string()),
        clash_mode: Some("Global".to_string()),
        ..SingboxRule::default()
    });

    let routing = context.routing_item.as_ref();
    let domain_strategy = routing
        .and_then(|routing| nonempty_string(Some(routing.domain_strategy4_singbox.as_str())))
        .or_else(|| {
            nonempty_string(Some(
                context
                    .app_config
                    .routing_basic_item
                    .domain_strategy4_singbox
                    .as_str(),
            ))
        });
    let resolve_rule = SingboxRule {
        action: Some("resolve".to_string()),
        strategy: domain_strategy,
        ..SingboxRule::default()
    };
    if context.app_config.routing_basic_item.domain_strategy == IP_ON_DEMAND {
        config.route.rules.push(resolve_rule.clone());
    }

    let Some(routing) = context.routing_item.clone() else {
        return;
    };
    let mut ip_rules = Vec::new();
    for item in routing
        .rule_set
        .iter()
        .filter(|item| item.enabled && item.rule_type != Some(RuleType::DNS))
    {
        gen_routing_user_rule(config, context, item);
        if item.ip.as_ref().is_some_and(|ips| !ips.is_empty()) {
            ip_rules.push(item.clone());
        }
    }
    if context.app_config.routing_basic_item.domain_strategy == IP_IF_NON_MATCH {
        config.route.rules.push(resolve_rule);
        for item in &ip_rules {
            gen_routing_user_rule(config, context, item);
        }
    }
}

fn tun_route_rules() -> Vec<SingboxRule> {
    vec![
        SingboxRule {
            network: Some(vec!["udp".to_string()]),
            port: Some(vec![135, 137, 138, 139, 5353]),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        },
        SingboxRule {
            ip_cidr: Some(vec!["224.0.0.0/3".to_string(), "ff00::/8".to_string()]),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        },
    ]
}

fn tun_dns_process_names() -> Vec<String> {
    [
        "xray",
        "v2ray",
        "wv2ray",
        "hysteria",
        "hysteria-windows-amd64",
        "hysteria-linux-amd64",
        "hysteria-darwin-amd64",
        "hysteria-darwin-arm64",
        "naive",
        "naiveproxy",
        "tuic-client",
        "juicity-client",
        "mieru",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn tun_direct_process_names() -> Vec<String> {
    let mut names = tun_dns_process_names();
    names.push("sing-box".to_string());
    names.push("mihomo".to_string());
    names
}

fn tun_icmp_routing(value: &str) -> &str {
    match value {
        "direct" | "unreachable" | "drop" | "reply" => value,
        _ => "rule",
    }
}

fn hosts_resolve_rule(simple_dns: &crate::SimpleDnsItem) -> Option<SingboxRule> {
    let host_keys = parse_hosts_to_dictionary(simple_dns.hosts.as_deref())
        .into_keys()
        .collect::<Vec<_>>();
    if host_keys.is_empty() {
        return None;
    }

    let mut rule = SingboxRule {
        action: Some("resolve".to_string()),
        ..SingboxRule::default()
    };
    let mut count = 0;
    for host in host_keys {
        let mut domain_rule = SingboxRule::default();
        if !parse_v2_domain(&host, &mut domain_rule) {
            continue;
        }
        normalize_bare_host_domain(&host, &mut domain_rule);
        if let Some(items) = domain_rule.domain {
            rule.domain.get_or_insert_with(Vec::new).extend(items);
            count += 1;
        } else if let Some(items) = domain_rule.domain_keyword {
            rule.domain_keyword
                .get_or_insert_with(Vec::new)
                .extend(items);
            count += 1;
        } else if let Some(items) = domain_rule.domain_suffix {
            rule.domain_suffix
                .get_or_insert_with(Vec::new)
                .extend(items);
            count += 1;
        } else if let Some(items) = domain_rule.domain_regex {
            rule.domain_regex.get_or_insert_with(Vec::new).extend(items);
            count += 1;
        } else if let Some(items) = domain_rule.geosite {
            rule.geosite.get_or_insert_with(Vec::new).extend(items);
            count += 1;
        }
    }

    (count > 0).then_some(rule)
}

fn gen_routing_user_rule(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
    user_rule: &RulesItem,
) {
    let outbound_tag = gen_routing_user_rule_outbound(
        config,
        context,
        user_rule.outbound_tag.as_deref().unwrap_or(PROXY_TAG),
    );
    let mut rule = SingboxRule {
        outbound: Some(outbound_tag.clone()),
        ..SingboxRule::default()
    };
    if outbound_tag == BLOCK_TAG {
        rule.outbound = None;
        rule.action = Some("reject".to_string());
    }
    fill_rule_common(&mut rule, user_rule);

    let mut has_domain_ip_process = false;
    if let Some(domains) = &user_rule.domain {
        let mut domain_rule = rule.clone();
        let count = domains
            .iter()
            .filter(|domain| parse_v2_domain(domain, &mut domain_rule))
            .count();
        if count > 0 {
            config.route.rules.push(domain_rule);
            has_domain_ip_process = true;
        }
    }
    if let Some(ips) = &user_rule.ip {
        let mut ip_rule = rule.clone();
        let negative_ips = ips
            .iter()
            .filter_map(|ip| ip.strip_prefix('!').map(str::trim))
            .collect::<Vec<_>>();
        let count = if negative_ips.is_empty() {
            ips.iter()
                .filter(|ip| parse_v2_address(ip, &mut ip_rule))
                .count()
        } else {
            let mut positive_rule = rule.clone();
            positive_rule.outbound = None;
            positive_rule.action = None;
            let mut negative_rule = SingboxRule::default();
            let positive_count = ips
                .iter()
                .filter(|ip| !ip.starts_with('!'))
                .filter(|ip| parse_v2_address(ip, &mut positive_rule))
                .count();
            let negative_count = negative_ips
                .iter()
                .filter(|ip| parse_v2_address(ip, &mut negative_rule))
                .count();
            if positive_count > 0 && negative_count > 0 && route_ip_rule_has_matcher(&negative_rule)
            {
                negative_rule.invert = Some(true);
                ip_rule = SingboxRule {
                    outbound: rule.outbound.clone(),
                    action: rule.action.clone(),
                    r#type: Some("logical".to_string()),
                    mode: Some("and".to_string()),
                    rules: Some(vec![positive_rule, negative_rule]),
                    ..SingboxRule::default()
                };
                positive_count + negative_count
            } else if positive_count > 0 {
                ip_rule = positive_rule;
                positive_count
            } else {
                has_domain_ip_process = true;
                0
            }
        };
        if count > 0 {
            config.route.rules.push(ip_rule);
            has_domain_ip_process = true;
        }
    }
    if let Some(processes) = &user_rule.process {
        let mut process_name_rule = rule.clone();
        let mut process_path_rule = rule.clone();
        for process in processes {
            if process == "self/" || process == "xray/" {
                process_name_rule
                    .process_name
                    .get_or_insert_with(Vec::new)
                    .push("sing-box".to_string());
                continue;
            }
            if process.contains('/') || process.contains('\\') {
                process_path_rule
                    .process_path
                    .get_or_insert_with(Vec::new)
                    .push(process.clone());
            } else {
                process_name_rule
                    .process_name
                    .get_or_insert_with(Vec::new)
                    .push(exe_name(process));
            }
        }
        if process_name_rule
            .process_name
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        {
            config.route.rules.push(process_name_rule);
            has_domain_ip_process = true;
        }
        if process_path_rule
            .process_path
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        {
            config.route.rules.push(process_path_rule);
            has_domain_ip_process = true;
        }
    }

    if !has_domain_ip_process
        && (rule.port.is_some()
            || rule.port_range.is_some()
            || rule.protocol.is_some()
            || rule.inbound.is_some()
            || rule.network.is_some())
    {
        config.route.rules.push(rule);
    }
}

fn fill_rule_common(rule: &mut SingboxRule, user_rule: &RulesItem) {
    if let Some(port) = nonempty_str(user_rule.port.as_deref()) {
        let mut ports = Vec::new();
        let mut port_ranges = Vec::new();
        for item in port
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            if item.contains('-') {
                port_ranges.push(item.replace('-', ":"));
            } else if let Ok(port) = item.parse::<i32>() {
                ports.push(port);
            }
        }
        if !ports.is_empty() {
            rule.port = Some(ports);
        }
        if !port_ranges.is_empty() {
            rule.port_range = Some(port_ranges);
        }
    }
    if let Some(network) = nonempty_str(user_rule.network.as_deref()) {
        rule.network = Some(split_csv(network));
    }
    rule.protocol = user_rule.protocol.clone().filter(|items| !items.is_empty());
    rule.inbound = user_rule
        .inbound_tag
        .clone()
        .filter(|items| !items.is_empty());
}

fn route_ip_rule_has_matcher(rule: &SingboxRule) -> bool {
    rule.geoip.as_ref().is_some_and(|items| !items.is_empty())
        || rule.ip_cidr.as_ref().is_some_and(|items| !items.is_empty())
        || rule.ip_is_private == Some(true)
}

fn gen_routing_user_rule_outbound(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
    outbound_tag: &str,
) -> String {
    if [PROXY_TAG, DIRECT_TAG, BLOCK_TAG].contains(&outbound_tag) {
        return outbound_tag.to_string();
    }

    let Some(node) = context
        .all_proxies_map
        .get(&format!("remark:{outbound_tag}"))
        .cloned()
    else {
        return PROXY_TAG.to_string();
    };
    if !node.config_type.is_group_type() && !singbox_supports_config_type(node.config_type) {
        return PROXY_TAG.to_string();
    }

    let tag = format!("{}-{PROXY_TAG}-{}", node.index_id, node.remarks);
    if config
        .outbounds
        .iter()
        .any(|outbound| outbound.tag.starts_with(&tag))
        || config
            .endpoints
            .iter()
            .any(|endpoint| endpoint.tag.starts_with(&tag))
    {
        return tag;
    }

    let servers = build_all_proxy_servers(context, &node, &tag, true);
    if servers.is_empty() {
        return PROXY_TAG.to_string();
    }
    append_servers(config, servers);
    tag
}

fn parse_v2_domain(domain: &str, rule: &mut SingboxRule) -> bool {
    if domain.starts_with('#') || domain.starts_with("ext:") || domain.starts_with("ext-domain:") {
        return false;
    }
    if let Some(value) = domain.strip_prefix(GEOSITE_PREFIX) {
        rule.geosite
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else if let Some(value) = domain.strip_prefix("regexp:") {
        rule.domain_regex
            .get_or_insert_with(Vec::new)
            .push(value.replace("<COMMA>", ","));
    } else if let Some(value) = domain.strip_prefix("domain:") {
        rule.domain_suffix
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else if let Some(value) = domain.strip_prefix("full:") {
        rule.domain
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else if let Some(value) = domain.strip_prefix("keyword:") {
        rule.domain_keyword
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else if let Some(value) = domain.strip_prefix("dotless:") {
        rule.domain_keyword
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else {
        rule.domain_keyword
            .get_or_insert_with(Vec::new)
            .push(domain.to_string());
    }
    true
}

fn parse_v2_address(address: &str, rule: &mut SingboxRule) -> bool {
    if address.starts_with("ext:") || address.starts_with("ext-ip:") {
        return false;
    }
    if address == "geoip:private" {
        rule.ip_is_private = Some(true);
    } else if let Some(value) = address.strip_prefix("geoip:") {
        rule.geoip
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else {
        rule.ip_cidr
            .get_or_insert_with(Vec::new)
            .push(address.to_string());
    }
    true
}

fn gen_dns(config: &mut SingboxConfig, context: &CoreConfigContext) {
    if context
        .raw_dns_item
        .as_ref()
        .is_some_and(|item| item.enabled)
    {
        gen_dns_custom(config, context);
        return;
    }

    gen_dns_servers(config, context);
    gen_dns_rules(config, context);

    let use_direct_dns = final_dns_uses_direct(context);
    let dns = config.dns.get_or_insert_with(SingboxDns::default);
    dns.independent_cache = Some(true);
    dns.final_server = Some(
        if use_direct_dns {
            SINGBOX_DIRECT_DNS_TAG
        } else {
            SINGBOX_REMOTE_DNS_TAG
        }
        .to_string(),
    );

    let simple_dns = &context.simple_dns_item;
    if !use_direct_dns
        && simple_dns.fake_ip == Some(true)
        && simple_dns.global_fake_ip == Some(false)
    {
        dns.rules.push(SingboxRule {
            server: Some(SINGBOX_FAKE_DNS_TAG.to_string()),
            query_type: Some(vec![1, 28]),
            rewrite_ttl: Some(1),
            ..SingboxRule::default()
        });
    }
}

fn gen_dns_servers(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let simple_dns = &context.simple_dns_item;
    let mut bootstrap_dns = parse_dns_address_or_default(
        simple_dns
            .bootstrap_dns
            .as_deref()
            .unwrap_or(DEFAULT_BOOTSTRAP_DNS),
        DEFAULT_BOOTSTRAP_DNS,
    );
    bootstrap_dns.tag = SINGBOX_LOCAL_DNS_TAG.to_string();

    let mut direct_dns = parse_dns_address_or_default(
        simple_dns
            .direct_dns
            .as_deref()
            .unwrap_or(DEFAULT_DIRECT_DNS),
        DEFAULT_DIRECT_DNS,
    );
    direct_dns.tag = SINGBOX_DIRECT_DNS_TAG.to_string();
    direct_dns.domain_resolver = Some(SINGBOX_LOCAL_DNS_TAG.to_string());

    let mut remote_dns = parse_dns_address_or_default(
        simple_dns
            .remote_dns
            .as_deref()
            .unwrap_or(DEFAULT_REMOTE_DNS),
        DEFAULT_REMOTE_DNS,
    );
    remote_dns.tag = SINGBOX_REMOTE_DNS_TAG.to_string();
    remote_dns.detour = Some(PROXY_TAG.to_string());
    remote_dns.domain_resolver = Some(SINGBOX_LOCAL_DNS_TAG.to_string());

    let mut predefined = BTreeMap::new();
    if simple_dns.add_common_hosts == Some(true) {
        for (host, addresses) in predefined_hosts() {
            predefined.insert(
                host.to_string(),
                addresses
                    .iter()
                    .map(|address| (*address).to_string())
                    .collect(),
            );
        }
    }
    for (host, addresses) in parse_hosts_to_dictionary(simple_dns.hosts.as_deref()) {
        let mut test_rule = SingboxRule::default();
        if !parse_v2_domain(&host, &mut test_rule) {
            continue;
        }
        normalize_bare_host_domain(&host, &mut test_rule);
        if let Some(domain) = test_rule.domain.as_ref().and_then(|items| items.first()) {
            let ips = addresses
                .into_iter()
                .filter(|address| is_ip_address(address))
                .collect::<Vec<_>>();
            predefined.insert(domain.clone(), ips);
        }
    }

    for host in predefined.keys() {
        if bootstrap_dns.server.as_deref() == Some(host.as_str()) {
            bootstrap_dns.domain_resolver = Some(SINGBOX_HOSTS_DNS_TAG.to_string());
        }
        if remote_dns.server.as_deref() == Some(host.as_str()) {
            remote_dns.domain_resolver = Some(SINGBOX_HOSTS_DNS_TAG.to_string());
        }
        if direct_dns.server.as_deref() == Some(host.as_str()) {
            direct_dns.domain_resolver = Some(SINGBOX_HOSTS_DNS_TAG.to_string());
        }
    }

    let mut servers = vec![
        bootstrap_dns,
        remote_dns,
        direct_dns,
        SingboxDnsServer {
            tag: SINGBOX_HOSTS_DNS_TAG.to_string(),
            r#type: "hosts".to_string(),
            predefined: Some(predefined),
            ..SingboxDnsServer::default()
        },
    ];
    if simple_dns.fake_ip == Some(true) {
        servers.push(SingboxDnsServer {
            tag: SINGBOX_FAKE_DNS_TAG.to_string(),
            r#type: "fakeip".to_string(),
            inet4_range: Some(SINGBOX_FAKEIP_INET4_RANGE.to_string()),
            inet6_range: Some(SINGBOX_FAKEIP_INET6_RANGE.to_string()),
            ..SingboxDnsServer::default()
        });
    }

    config.dns.get_or_insert_with(SingboxDns::default).servers = servers;
}

fn gen_dns_rules(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let simple_dns = &context.simple_dns_item;
    let mut rules = vec![SingboxRule {
        ip_accept_any: Some(true),
        server: Some(SINGBOX_HOSTS_DNS_TAG.to_string()),
        ..SingboxRule::default()
    }];

    if !context.protect_domain_list.is_empty() {
        rules.push(SingboxRule {
            server: Some(SINGBOX_DIRECT_DNS_TAG.to_string()),
            strategy: domain_strategy4_sbox(simple_dns.strategy4_freedom.as_deref()),
            domain: Some(context.protect_domain_list.clone()),
            ..SingboxRule::default()
        });
    }

    rules.push(SingboxRule {
        server: Some(SINGBOX_REMOTE_DNS_TAG.to_string()),
        strategy: domain_strategy4_sbox(simple_dns.strategy4_proxy.as_deref()),
        clash_mode: Some("Global".to_string()),
        ..SingboxRule::default()
    });
    rules.push(SingboxRule {
        server: Some(SINGBOX_DIRECT_DNS_TAG.to_string()),
        strategy: domain_strategy4_sbox(simple_dns.strategy4_freedom.as_deref()),
        clash_mode: Some("Direct".to_string()),
        ..SingboxRule::default()
    });

    for (host, addresses) in parse_hosts_to_dictionary(simple_dns.hosts.as_deref()) {
        let Some(predefined) = addresses.first() else {
            continue;
        };
        if predefined.is_empty() {
            continue;
        }
        let mut rule = SingboxRule {
            query_type: Some(vec![1, 5, 28]),
            action: Some("predefined".to_string()),
            rcode: Some("NOERROR".to_string()),
            ..SingboxRule::default()
        };
        if !parse_v2_domain(&host, &mut rule) {
            continue;
        }
        normalize_bare_host_domain(&host, &mut rule);
        if let Some(rcode) = predefined
            .strip_prefix('#')
            .and_then(|value| value.parse::<i32>().ok())
        {
            rule.rcode = Some(dns_rcode(rcode).to_string());
        } else if is_domain_name(predefined) {
            rule.answer = Some(vec![format!("*. IN CNAME {predefined}.")]);
        } else if is_ip_address(predefined) && rule.domain.as_ref().is_none_or(Vec::is_empty) {
            if predefined.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
                rule.answer = Some(vec![format!("*. IN AAAA {predefined}")]);
            } else {
                rule.answer = Some(vec![format!("*. IN A {predefined}")]);
            }
        } else {
            continue;
        }
        rules.push(rule);
    }

    if simple_dns.block_binding_query == Some(true) {
        rules.push(SingboxRule {
            query_type: Some(vec![64, 65]),
            action: Some("predefined".to_string()),
            rcode: Some("NOERROR".to_string()),
            ..SingboxRule::default()
        });
    }

    if simple_dns.fake_ip == Some(true) && simple_dns.global_fake_ip == Some(true) {
        let mut fakeip_filter_rule = fakeip_filter_rule();
        fakeip_filter_rule.invert = Some(true);
        rules.push(SingboxRule {
            server: Some(SINGBOX_FAKE_DNS_TAG.to_string()),
            r#type: Some("logical".to_string()),
            mode: Some("and".to_string()),
            rewrite_ttl: Some(1),
            rules: Some(vec![
                SingboxRule {
                    query_type: Some(vec![1, 28]),
                    ..SingboxRule::default()
                },
                fakeip_filter_rule,
            ]),
            ..SingboxRule::default()
        });
    }

    append_dns_routing_rules(&mut rules, context);
    config.dns.get_or_insert_with(SingboxDns::default).rules = rules;
}

fn append_dns_routing_rules(rules: &mut Vec<SingboxRule>, context: &CoreConfigContext) {
    let Some(routing) = context.routing_item.as_ref() else {
        return;
    };
    let simple_dns = &context.simple_dns_item;
    let (expected_ip_cidr, expected_ip_regions, region_name) =
        parse_direct_expected_ips(simple_dns.direct_expected_ips.as_deref());

    for item in routing
        .rule_set
        .iter()
        .filter(|item| item.enabled && item.rule_type != Some(RuleType::Routing))
    {
        let Some(domains) = item.domain.as_ref().filter(|domains| !domains.is_empty()) else {
            continue;
        };
        let mut rule = SingboxRule::default();
        let valid_domains = domains
            .iter()
            .filter(|domain| parse_v2_domain(domain, &mut rule))
            .count();
        if valid_domains == 0 {
            continue;
        }

        match item.outbound_tag.as_deref() {
            Some(DIRECT_TAG) => {
                rule.server = Some(SINGBOX_DIRECT_DNS_TAG.to_string());
                rule.strategy = domain_strategy4_sbox(simple_dns.strategy4_freedom.as_deref());
                if !expected_ip_regions.is_empty() && !region_name.is_empty() {
                    if let Some(geosite) = &mut rule.geosite {
                        let matched_geosite = geosite
                            .iter()
                            .filter(|item| {
                                item.ends_with(&format!("-{region_name}"))
                                    || item.ends_with(&format!("@{region_name}"))
                                    || *item == &region_name
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        if !matched_geosite.is_empty() {
                            geosite.retain(|item| !matched_geosite.contains(item));
                            let mut expected_rule = rule.clone();
                            expected_rule.geosite = Some(matched_geosite);
                            expected_rule.geoip = Some(expected_ip_regions.clone());
                            if !expected_ip_cidr.is_empty() {
                                expected_rule.ip_cidr = Some(expected_ip_cidr.clone());
                            }
                            rules.push(expected_rule);
                        }
                    }
                }
            }
            Some(BLOCK_TAG) => {
                rule.action = Some("predefined".to_string());
                rule.rcode = Some("NXDOMAIN".to_string());
            }
            _ => {
                if simple_dns.fake_ip == Some(true) && simple_dns.global_fake_ip == Some(false) {
                    let mut fake_rule = rule.clone();
                    fake_rule.server = Some(SINGBOX_FAKE_DNS_TAG.to_string());
                    fake_rule.query_type = Some(vec![1, 28]);
                    fake_rule.rewrite_ttl = Some(1);
                    rules.push(fake_rule);
                }
                rule.server = Some(SINGBOX_REMOTE_DNS_TAG.to_string());
                rule.strategy = domain_strategy4_sbox(simple_dns.strategy4_proxy.as_deref());
            }
        }

        if dns_rule_has_matcher(&rule) {
            rules.push(rule);
        }
    }
}

fn dns_rule_has_matcher(rule: &SingboxRule) -> bool {
    rule.domain.as_ref().is_some_and(|items| !items.is_empty())
        || rule
            .domain_suffix
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || rule
            .domain_keyword
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || rule
            .domain_regex
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || rule.geosite.as_ref().is_some_and(|items| !items.is_empty())
        || rule.geoip.as_ref().is_some_and(|items| !items.is_empty())
        || rule.ip_cidr.as_ref().is_some_and(|items| !items.is_empty())
        || rule
            .rule_set
            .as_ref()
            .is_some_and(|items| !items.is_empty())
}

fn gen_dns_custom(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let Some(item) = context.raw_dns_item.as_ref() else {
        return;
    };
    let custom_dns = if context.is_tun_enabled {
        item.tun_dns.as_deref()
    } else {
        item.normal_dns.as_deref()
    }
    .filter(|value| !value.trim().is_empty())
    .unwrap_or(DEFAULT_SINGBOX_DNS_NORMAL);
    let Ok(mut dns) = serde_json::from_str::<SingboxDns>(custom_dns) else {
        return;
    };
    gen_dns_protect_custom(&mut dns, context);
    config.dns = Some(dns);
}

fn gen_dns_protect_custom(dns: &mut SingboxDns, context: &CoreConfigContext) {
    let final_dns_address = context
        .raw_dns_item
        .as_ref()
        .and_then(|item| nonempty_string(item.domain_dns_address.as_deref()))
        .unwrap_or_else(|| DEFAULT_BOOTSTRAP_DNS.to_string());
    if !dns_server_tag_exists(dns, SINGBOX_LOCAL_DNS_TAG) {
        if let Some(mut local_dns_server) = parse_dns_address(&final_dns_address) {
            local_dns_server.tag = SINGBOX_LOCAL_DNS_TAG.to_string();
            dns.servers.push(local_dns_server);
        }
    }

    if let Some(global_server_tag) = custom_dns_global_server_tag(dns) {
        dns.rules.insert(
            0,
            SingboxRule {
                server: Some(global_server_tag),
                clash_mode: Some("Global".to_string()),
                ..SingboxRule::default()
            },
        );
    }

    if dns_server_tag_exists(dns, SINGBOX_LOCAL_DNS_TAG) {
        dns.rules.insert(
            0,
            SingboxRule {
                server: Some(SINGBOX_LOCAL_DNS_TAG.to_string()),
                clash_mode: Some("Direct".to_string()),
                ..SingboxRule::default()
            },
        );
    }

    if !context.protect_domain_list.is_empty() && dns_server_tag_exists(dns, SINGBOX_LOCAL_DNS_TAG)
    {
        dns.rules.insert(
            0,
            SingboxRule {
                server: Some(SINGBOX_LOCAL_DNS_TAG.to_string()),
                domain: Some(context.protect_domain_list.clone()),
                ..SingboxRule::default()
            },
        );
    }
}

fn custom_dns_global_server_tag(dns: &SingboxDns) -> Option<String> {
    dns.servers
        .iter()
        .find(|server| server.detour.as_deref() == Some(PROXY_TAG))
        .or_else(|| dns.servers.first())
        .map(|server| server.tag.clone())
}

fn dns_server_tag_exists(dns: &SingboxDns, tag: &str) -> bool {
    dns.servers.iter().any(|server| server.tag == tag)
}

fn final_dns_uses_direct(context: &CoreConfigContext) -> bool {
    let Some(last_rule) = context
        .routing_item
        .as_ref()
        .and_then(|routing| routing.rule_set.last())
    else {
        return false;
    };
    if last_rule.outbound_tag.as_deref() != Some(DIRECT_TAG) {
        return false;
    }

    let no_domain = last_rule.domain.as_ref().is_none_or(Vec::is_empty);
    let no_process = last_rule.process.as_ref().is_none_or(Vec::is_empty);
    let is_any_ip = last_rule
        .ip
        .as_ref()
        .is_none_or(|ips| ips.is_empty() || ips.iter().any(|ip| ip == "0.0.0.0/0"));
    let is_any_port = last_rule
        .port
        .as_deref()
        .is_none_or(|port| port.is_empty() || port == "0-65535");
    let is_any_network = last_rule
        .network
        .as_deref()
        .is_none_or(|network| network.is_empty() || network == "tcp,udp");

    no_domain && no_process && is_any_ip && is_any_port && is_any_network
}

fn parse_direct_expected_ips(value: Option<&str>) -> (Vec<String>, Vec<String>, String) {
    let mut ip_cidr = Vec::new();
    let mut regions = Vec::new();
    let mut region_name = String::new();
    for item in value
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        if let Some(region) = item.strip_prefix(GEOIP_PREFIX) {
            if !region.is_empty() {
                regions.push(region.to_string());
                region_name = region.to_string();
            }
        } else {
            ip_cidr.push(item.to_string());
        }
    }
    (ip_cidr, regions, region_name)
}

fn parse_dns_address_or_default(address: &str, default_address: &str) -> SingboxDnsServer {
    parse_dns_address(address)
        .or_else(|| parse_dns_address(default_address))
        .unwrap_or_default()
}

fn parse_dns_address(address: &str) -> Option<SingboxDnsServer> {
    let address_first = first_dns_address(address)?;
    if matches!(address_first.as_str(), "local" | "localhost") {
        return Some(SingboxDnsServer {
            r#type: "local".to_string(),
            ..SingboxDnsServer::default()
        });
    }

    let (domain, scheme, port, path) = parse_url_parts(&address_first)?;
    if scheme.eq_ignore_ascii_case("dhcp") {
        return Some(SingboxDnsServer {
            r#type: "dhcp".to_string(),
            server: (!domain.is_empty() && domain != "auto").then_some(domain),
            ..SingboxDnsServer::default()
        });
    }

    let server_type = if scheme.is_empty() {
        "udp".to_string()
    } else {
        scheme.replace("+local", "").to_lowercase()
    };
    Some(SingboxDnsServer {
        r#type: server_type.clone(),
        server: (!domain.is_empty()).then_some(domain),
        server_port: port.map(i32::from),
        path: matches!(server_type.as_str(), "https" | "h3")
            .then(|| path)
            .filter(|path| !path.is_empty() && path != "/"),
        ..SingboxDnsServer::default()
    })
}

fn first_dns_address(address: &str) -> Option<String> {
    let delimiter = if address.contains(',') { ',' } else { ';' };
    address
        .split(delimiter)
        .map(str::trim)
        .find(|item| !item.is_empty())
        .map(str::to_string)
}

fn parse_url_parts(input: &str) -> Option<(String, String, Option<u16>, String)> {
    if let Ok(url) = url::Url::parse(input) {
        if let Some(host) = url.host_str() {
            let mut path = url.path().to_string();
            if let Some(query) = url.query() {
                path.push('?');
                path.push_str(query);
            }
            let port = match url.port() {
                Some(0) => return None,
                Some(port) => Some(port),
                None => None,
            };
            return Some((host.to_string(), url.scheme().to_string(), port, path));
        }
    }
    if input.contains("://") {
        return None;
    }

    let (scheme, rest) = input
        .split_once("://")
        .map_or(("", input), |(scheme, rest)| (scheme, rest));
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let path = if authority_end < rest.len() && rest[authority_end..].starts_with('/') {
        rest[authority_end..]
            .split('#')
            .next()
            .unwrap_or_default()
            .to_string()
    } else {
        String::new()
    };
    let (domain, port) = parse_authority(authority)?;
    if domain.is_empty() {
        Some((input.to_string(), String::new(), None, String::new()))
    } else {
        Some((domain, scheme.to_string(), port, path))
    }
}

fn parse_authority(authority: &str) -> Option<(String, Option<u16>)> {
    if authority.is_empty() {
        return Some((String::new(), None));
    }
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, authority)| authority);
    if authority.starts_with('[') {
        if let Some(closing_bracket_index) = authority.rfind(']') {
            let domain = authority[..=closing_bracket_index].to_string();
            let rest = authority
                .get(closing_bracket_index + 1..)
                .unwrap_or_default();
            if rest.is_empty() {
                return Some((domain, None));
            }
            let port = parse_authority_port(rest.strip_prefix(':')?)?;
            return Some((domain, Some(port)));
        }
    }
    if let Some((domain, port)) = authority.rsplit_once(':') {
        if !domain.contains(':') {
            let port = parse_authority_port(port)?;
            return Some((domain.to_string(), Some(port)));
        }
    }
    Some((authority.to_string(), None))
}

fn parse_authority_port(port: &str) -> Option<u16> {
    port.parse::<u16>().ok().filter(|port| *port > 0)
}

fn domain_strategy4_sbox(strategy: Option<&str>) -> Option<String> {
    let strategy = strategy?;
    if strategy.starts_with("UseIPv4") {
        Some("prefer_ipv4".to_string())
    } else if strategy.starts_with("UseIPv6") {
        Some("prefer_ipv6".to_string())
    } else if strategy.starts_with("ForceIPv4") {
        Some("ipv4_only".to_string())
    } else if strategy.starts_with("ForceIPv6") {
        Some("ipv6_only".to_string())
    } else {
        None
    }
}

fn dns_rcode(value: i32) -> &'static str {
    match value {
        1 => "FORMERR",
        2 => "SERVFAIL",
        3 => "NXDOMAIN",
        4 => "NOTIMP",
        5 => "REFUSED",
        _ => "NOERROR",
    }
}

fn is_domain_name(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !is_ip_address(value)
        && value.contains('.')
        && value
            .rsplit('.')
            .next()
            .is_some_and(|tld| tld.chars().all(|ch| ch.is_ascii_alphanumeric()))
}

fn is_ip_address(value: &str) -> bool {
    value.trim().parse::<IpAddr>().is_ok()
}

fn normalize_bare_host_domain(host: &str, rule: &mut SingboxRule) {
    if host.contains(':') {
        return;
    }
    if let Some(domain_keyword) = rule.domain_keyword.take() {
        if !domain_keyword.is_empty() {
            rule.domain = Some(domain_keyword);
        }
    }
}

fn parse_hosts_to_dictionary(hosts_content: Option<&str>) -> BTreeMap<String, Vec<String>> {
    let mut result = BTreeMap::new();
    for line in hosts_content
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty() && !line.starts_with('#') && line.contains(char::is_whitespace)
        })
    {
        let parts = line
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        result
            .entry(parts[0].clone())
            .or_insert_with(Vec::new)
            .extend(parts.into_iter().skip(1));
    }
    result
}

fn predefined_hosts() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        (
            "dns.google",
            &[
                "8.8.8.8",
                "8.8.4.4",
                "2001:4860:4860::8888",
                "2001:4860:4860::8844",
            ],
        ),
        (
            "dns.alidns.com",
            &[
                "223.5.5.5",
                "223.6.6.6",
                "2400:3200::1",
                "2400:3200:baba::1",
            ],
        ),
        (
            "one.one.one.one",
            &[
                "1.1.1.1",
                "1.0.0.1",
                "2606:4700:4700::1111",
                "2606:4700:4700::1001",
            ],
        ),
        (
            "1dot1dot1dot1.cloudflare-dns.com",
            &[
                "1.1.1.1",
                "1.0.0.1",
                "2606:4700:4700::1111",
                "2606:4700:4700::1001",
            ],
        ),
        (
            "cloudflare-dns.com",
            &[
                "104.16.249.249",
                "104.16.248.249",
                "2606:4700::6810:f8f9",
                "2606:4700::6810:f9f9",
            ],
        ),
        (
            "dns.cloudflare.com",
            &[
                "104.16.132.229",
                "104.16.133.229",
                "2606:4700::6810:84e5",
                "2606:4700::6810:85e5",
            ],
        ),
        ("dot.pub", &["1.12.12.12", "120.53.53.53"]),
        ("doh.pub", &["1.12.12.12", "120.53.53.53"]),
        (
            "dns.quad9.net",
            &["9.9.9.9", "149.112.112.112", "2620:fe::fe", "2620:fe::9"],
        ),
        (
            "dns.yandex.net",
            &[
                "77.88.8.8",
                "77.88.8.1",
                "2a02:6b8::feed:0ff",
                "2a02:6b8:0:1::feed:0ff",
            ],
        ),
        ("dns.sb", &["185.222.222.222", "2a09::"]),
        (
            "dns.umbrella.com",
            &[
                "208.67.220.220",
                "208.67.222.222",
                "2620:119:35::35",
                "2620:119:53::53",
            ],
        ),
        (
            "dns.sse.cisco.com",
            &[
                "208.67.220.220",
                "208.67.222.222",
                "2620:119:35::35",
                "2620:119:53::53",
            ],
        ),
        ("engage.cloudflareclient.com", &["162.159.192.1"]),
    ]
}

fn fakeip_filter_rule() -> SingboxRule {
    SingboxRule {
        domain: Some(vec![
            "amobile.music.tc.qq.com".to_string(),
            "api-jooxtt.sanook.com".to_string(),
            "api.joox.com".to_string(),
            "aqqmusic.tc.qq.com".to_string(),
            "dl.stream.qqmusic.qq.com".to_string(),
            "ff.dorado.sdo.com".to_string(),
            "heartbeat.belkin.com".to_string(),
            "isure.stream.qqmusic.qq.com".to_string(),
            "joox.com".to_string(),
            "lens.l.google.com".to_string(),
            "localhost.ptlogin2.qq.com".to_string(),
            "localhost.sec.qq.com".to_string(),
            "mesu.apple.com".to_string(),
            "mobileoc.music.tc.qq.com".to_string(),
            "music.taihe.com".to_string(),
            "musicapi.taihe.com".to_string(),
            "na.b.g-tun.com".to_string(),
            "proxy.golang.org".to_string(),
            "ps.res.netease.com".to_string(),
            "shark007.net".to_string(),
            "songsearch.kugou.com".to_string(),
            "static.adtidy.org".to_string(),
            "streamoc.music.tc.qq.com".to_string(),
            "swcdn.apple.com".to_string(),
            "swdist.apple.com".to_string(),
            "swdownload.apple.com".to_string(),
            "swquery.apple.com".to_string(),
            "swscan.apple.com".to_string(),
            "turn.cloudflare.com".to_string(),
            "trackercdn.kugou.com".to_string(),
            "xnotify.xboxlive.com".to_string(),
        ]),
        domain_keyword: Some(vec![
            "ntp".to_string(),
            "stun".to_string(),
            "time".to_string(),
        ]),
        domain_regex: Some(vec![
            "^[^.]+$".to_string(),
            r"^[^.]+\.[^.]+\.xboxlive\.com$".to_string(),
            r"^localhost\.[^.]+\.weixin\.qq\.com$".to_string(),
            r"^mijia\scloud$".to_string(),
            r"^xbox\.[^.]+\.microsoft\.com$".to_string(),
            r"^xbox\.[^.]+\.[^.]+\.microsoft\.com$".to_string(),
        ]),
        domain_suffix: Some(vec![
            "126.net".to_string(),
            "3gppnetwork.org".to_string(),
            "battle.net".to_string(),
            "battlenet.com.cn".to_string(),
            "cdn.nintendo.net".to_string(),
            "cmbchina.com".to_string(),
            "cmbimg.com".to_string(),
            "ff14.sdo.com".to_string(),
            "ffxiv.com".to_string(),
            "finalfantasyxiv.com".to_string(),
            "gcloudcs.com".to_string(),
            "home.arpa".to_string(),
            "invalid".to_string(),
            "kuwo.cn".to_string(),
            "lan".to_string(),
            "linksys.com".to_string(),
            "linksyssmartwifi.com".to_string(),
            "local".to_string(),
            "localdomain".to_string(),
            "localhost".to_string(),
            "market.xiaomi.com".to_string(),
            "mcdn.bilivideo.cn".to_string(),
            "media.dssott.com".to_string(),
            "msftconnecttest.com".to_string(),
            "msftncsi.com".to_string(),
            "music.163.com".to_string(),
            "music.migu.cn".to_string(),
            "n0808.com".to_string(),
            "nflxvideo.net".to_string(),
            "oray.com".to_string(),
            "orayimg.com".to_string(),
            "router.asus.com".to_string(),
            "sandai.net".to_string(),
            "square-enix.com".to_string(),
            "srv.nintendo.net".to_string(),
            "steamcontent.com".to_string(),
            "uu.163.com".to_string(),
            "wargaming.net".to_string(),
            "wggames.cn".to_string(),
            "wotgame.cn".to_string(),
            "wowsgame.cn".to_string(),
            "xiami.com".to_string(),
            "y.qq.com".to_string(),
        ]),
        ..SingboxRule::default()
    }
}

fn gen_experimental(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let mut experimental = config.experimental.clone().unwrap_or_default();
    experimental.clash_api = Some(SingboxClashApi {
        external_controller: Some(format!(
            "{LOOPBACK}:{}",
            state_port2(&context.app_config, context.is_tun_enabled)
        )),
        store_selected: None,
    });

    if context.app_config.core_basic_item.enable_cache_file4_sbox {
        experimental.cache_file = Some(SingboxCacheFile {
            enabled: true,
            path: Some("cache.db".to_string()),
            cache_id: None,
            store_fakeip: (context.simple_dns_item.fake_ip == Some(true)).then_some(true),
        });
    }

    config.experimental = Some(experimental);
}

fn convert_geo_to_ruleset(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
) -> Result<(), SingboxConfigError> {
    let mut rule_sets = Vec::new();
    for rule in &mut config.route.rules {
        convert_rule_geo_to_ruleset(rule, &mut rule_sets);
    }
    if let Some(dns) = &mut config.dns {
        for rule in &mut dns.rules {
            convert_rule_geo_to_ruleset(rule, &mut rule_sets);
        }
    }

    let unique_rule_sets = rule_sets
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<BTreeSet<_>>();
    if unique_rule_sets.is_empty() {
        return Ok(());
    }

    let custom_rulesets = parse_inline_custom_rulesets(
        context
            .routing_item
            .as_ref()
            .map(|routing| routing.custom_ruleset_path4_singbox.as_str()),
    )?;
    let source_url = nonempty_str(context.app_config.const_item.srs_source_url.as_deref())
        .unwrap_or(SINGBOX_RULESET_URL);
    config.route.rule_set = Some(
        unique_rule_sets
            .into_iter()
            .map(|tag| {
                custom_rulesets
                    .iter()
                    .find(|ruleset| ruleset.tag.as_deref() == Some(tag.as_str()))
                    .cloned()
                    .unwrap_or_else(|| ruleset_for_tag(&tag, source_url, context))
            })
            .collect(),
    );
    Ok(())
}

fn convert_rule_geo_to_ruleset(rule: &mut SingboxRule, rule_sets: &mut Vec<String>) {
    let mut converted = Vec::new();
    if rule.geosite.as_ref().is_some_and(|items| !items.is_empty()) {
        if let Some(geosite) = rule.geosite.take() {
            converted.extend(geosite.into_iter().map(|item| format!("geosite-{item}")));
        }
    }
    if rule.geoip.as_ref().is_some_and(|items| !items.is_empty()) {
        if let Some(geoip) = rule.geoip.take() {
            converted.extend(geoip.into_iter().map(|item| format!("geoip-{item}")));
        }
    }
    if !converted.is_empty() {
        rule.rule_set.get_or_insert_with(Vec::new).extend(converted);
    }
    if let Some(rule_set) = &rule.rule_set {
        rule_sets.extend(rule_set.clone());
    }
    if let Some(nested_rules) = &mut rule.rules {
        for nested_rule in nested_rules {
            convert_rule_geo_to_ruleset(nested_rule, rule_sets);
        }
    }
}

fn parse_inline_custom_rulesets(
    value: Option<&str>,
) -> Result<Vec<SingboxRuleset>, SingboxConfigError> {
    let Some(value) = value.map(str::trim).filter(|value| value.starts_with('[')) else {
        return Ok(Vec::new());
    };
    let rulesets = serde_json::from_str::<Vec<SingboxRuleset>>(value)
        .map_err(SingboxConfigError::CustomRulesetJson)?;
    for (index, ruleset) in rulesets.iter().enumerate() {
        if ruleset
            .tag
            .as_deref()
            .and_then(|value| nonempty_str(Some(value)))
            .is_none()
            || ruleset
                .r#type
                .as_deref()
                .and_then(|value| nonempty_str(Some(value)))
                .is_none()
            || ruleset
                .format
                .as_deref()
                .and_then(|value| nonempty_str(Some(value)))
                .is_none()
        {
            return Err(SingboxConfigError::CustomRulesetMissingRequiredFields { index });
        }
    }
    Ok(rulesets)
}

fn ruleset_for_tag(tag: &str, source_url: &str, context: &CoreConfigContext) -> SingboxRuleset {
    if let Some(path) = context.singbox_ruleset_paths.get(tag) {
        return SingboxRuleset {
            tag: Some(tag.to_string()),
            r#type: Some("local".to_string()),
            format: Some("binary".to_string()),
            path: Some(path.clone()),
            ..SingboxRuleset::default()
        };
    }

    remote_ruleset(tag, source_url)
}

fn remote_ruleset(tag: &str, source_url: &str) -> SingboxRuleset {
    let kind = if tag.starts_with("geosite") {
        "geosite"
    } else {
        "geoip"
    };
    SingboxRuleset {
        tag: Some(tag.to_string()),
        r#type: Some("remote".to_string()),
        format: Some("binary".to_string()),
        url: Some(
            source_url
                .replace("{0}", kind)
                .replace("{1}", tag)
                .to_string(),
        ),
        download_detour: Some(PROXY_TAG.to_string()),
        ..SingboxRuleset::default()
    }
}

fn apply_outbound_bind_interface(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let Some(bind_interface) =
        nonempty_string(context.app_config.core_basic_item.bind_interface.as_deref())
    else {
        return;
    };
    if !(context.is_tun_enabled || context.is_windows()) {
        return;
    }
    for outbound in &mut config.outbounds {
        if should_bind_outbound(outbound) {
            outbound.bind_interface = Some(bind_interface.clone());
        }
    }
}

fn apply_outbound_send_through(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let Some(send_through) =
        nonempty_string(context.app_config.core_basic_item.send_through.as_deref())
    else {
        return;
    };
    for outbound in &mut config.outbounds {
        if should_bind_outbound(outbound) {
            outbound.inet4_bind_address = Some(send_through.clone());
        }
    }
}

fn should_bind_outbound(outbound: &SingboxOutbound) -> bool {
    if matches!(
        outbound.r#type.as_str(),
        "direct" | "block" | "dns" | "selector" | "urltest"
    ) || outbound
        .detour
        .as_deref()
        .is_some_and(|detour| !detour.is_empty())
    {
        return false;
    }
    outbound
        .server
        .as_deref()
        .is_none_or(|server| !is_loopback_address(server))
}

fn apply_full_config_template(context: &CoreConfigContext, config: &SingboxConfig) -> Value {
    let Some(template) = &context.full_config_template else {
        return value_from_config(config);
    };
    if !template.enabled {
        return value_from_config(config);
    }
    let Some(template_json) = template_json_for_context(context) else {
        return value_from_config(config);
    };
    let Ok(mut template_value) = serde_json::from_str::<Value>(template_json) else {
        return value_from_config(config);
    };
    let Some(template_object) = template_value.as_object_mut() else {
        return value_from_config(config);
    };

    let mut generated_outbounds = template_object
        .get("outbounds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for outbound in &config.outbounds {
        if template.add_proxy_only == Some(true)
            && matches!(outbound.r#type.as_str(), "direct" | "block")
        {
            continue;
        }
        let mut outbound = outbound.clone();
        if outbound.detour.as_deref().is_none_or(str::is_empty) {
            if let Some(proxy_detour) = nonempty_str(template.proxy_detour.as_deref()) {
                if outbound
                    .server
                    .as_deref()
                    .is_none_or(|server| !is_private_network(server))
                    && !matches!(outbound.r#type.as_str(), "direct" | "block")
                {
                    outbound.detour = Some(proxy_detour.to_string());
                }
            }
        }
        generated_outbounds.push(serde_json::to_value(outbound).unwrap_or_else(|_| json!({})));
    }
    template_object.insert("outbounds".to_string(), Value::Array(generated_outbounds));

    if !config.endpoints.is_empty() {
        let mut generated_endpoints = template_object
            .get("endpoints")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for endpoint in &config.endpoints {
            let mut endpoint = endpoint.clone();
            if endpoint.detour.as_deref().is_none_or(str::is_empty) {
                if let Some(proxy_detour) = nonempty_str(template.proxy_detour.as_deref()) {
                    endpoint.detour = Some(proxy_detour.to_string());
                }
            }
            generated_endpoints.push(serde_json::to_value(endpoint).unwrap_or_else(|_| json!({})));
        }
        template_object.insert("endpoints".to_string(), Value::Array(generated_endpoints));
    }

    template_value
}

fn template_json_for_context(context: &CoreConfigContext) -> Option<&str> {
    let template = context.full_config_template.as_ref()?;
    if context.is_tun_enabled {
        template.tun_config.as_deref()
    } else {
        template.config.as_deref()
    }
}

fn child_nodes(context: &CoreConfigContext, node: &ProfileItem) -> Vec<ProfileItem> {
    let mut seen = BTreeSet::new();
    split_list(
        node.protocol_extra
            .child_items
            .as_deref()
            .unwrap_or_default(),
    )
    .unwrap_or_default()
    .into_iter()
    .filter(|node_id| seen.insert(node_id.clone()))
    .filter_map(|node_id| context.all_proxies_map.get(&node_id).cloned())
    .collect()
}

fn buildable_child_nodes(context: &CoreConfigContext, node: &ProfileItem) -> Vec<ProfileItem> {
    child_nodes(context, node)
        .into_iter()
        .filter(|child| child.config_type.is_group_type() || singbox_can_build_leaf(child))
        .collect()
}

fn singbox_can_build_leaf(node: &ProfileItem) -> bool {
    singbox_supports_config_type(node.config_type)
        && (node.config_type != ConfigType::WireGuard
            || wireguard_public_key(&node.protocol_extra).is_some())
}

fn protocol_name(config_type: ConfigType) -> &'static str {
    match config_type {
        ConfigType::VMess => "vmess",
        ConfigType::Shadowsocks => "shadowsocks",
        ConfigType::SOCKS => "socks",
        ConfigType::HTTP => "http",
        ConfigType::VLESS => "vless",
        ConfigType::Trojan => "trojan",
        ConfigType::Hysteria2 => "hysteria2",
        ConfigType::TUIC => "tuic",
        ConfigType::WireGuard => "wireguard",
        ConfigType::Anytls => "anytls",
        ConfigType::Naive => "naive",
        ConfigType::Custom | ConfigType::PolicyGroup | ConfigType::ProxyChain => "vmess",
    }
}

fn singbox_supports_config_type(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::VMess
            | ConfigType::VLESS
            | ConfigType::Shadowsocks
            | ConfigType::Trojan
            | ConfigType::Hysteria2
            | ConfigType::TUIC
            | ConfigType::Anytls
            | ConfigType::Naive
            | ConfigType::WireGuard
            | ConfigType::SOCKS
            | ConfigType::HTTP
    )
}

fn singbox_network(node: &ProfileItem) -> String {
    let network = trimmed(&node.network);
    if network.is_empty() {
        DEFAULT_NETWORK.to_string()
    } else {
        network.to_string()
    }
}

fn vmess_security(protocol_extra: &ProtocolExtraItem) -> String {
    let security = protocol_extra.vmess_security.as_deref().unwrap_or_default();
    if VMESS_SECURITIES.contains(&security) {
        security.to_string()
    } else {
        DEFAULT_SECURITY.to_string()
    }
}

fn shadowsocks_method(protocol_extra: &ProtocolExtraItem) -> String {
    let method = protocol_extra.ss_method.as_deref().unwrap_or_default();
    if SS_SECURITIES_IN_SINGBOX.contains(&method) {
        method.to_string()
    } else {
        "none".to_string()
    }
}

fn allow_insecure(node: &ProfileItem, context: &CoreConfigContext) -> bool {
    if !context.app_config.core_basic_item.def_allow_insecure {
        return false;
    }

    let node_value = trimmed(&node.allow_insecure);
    node_value.is_empty() || node_value.eq_ignore_ascii_case("true")
}

fn effective_fingerprint(node: &ProfileItem, context: &CoreConfigContext) -> Option<String> {
    singbox_utls_fingerprint(&node.fingerprint)
        .or_else(|| singbox_utls_fingerprint(&context.app_config.core_basic_item.def_fingerprint))
}

fn singbox_utls_fingerprint(value: &str) -> Option<String> {
    let fingerprint = trimmed(value).to_ascii_lowercase();
    if SINGBOX_UTLS_FINGERPRINTS.contains(&fingerprint.as_str()) {
        Some(fingerprint)
    } else {
        None
    }
}

fn transport_host_for_tls(node: &ProfileItem) -> Option<String> {
    let host = match singbox_network(node).as_str() {
        DEFAULT_NETWORK | "ws" | "httpupgrade" | "xhttp" => node.transport_extra.host.clone(),
        "grpc" => node.transport_extra.grpc_authority.clone(),
        _ => None,
    };
    let first_host = first_list_value(host.as_deref());
    nonempty_string(Some(&first_host))
}

fn raw_http_user_agent(user_agent: &str) -> String {
    match user_agent {
        "chrome" => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36".to_string(),
        "firefox" => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:90.0) Gecko/20100101 Firefox/90.0".to_string(),
        "safari" => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.1 Safari/605.1.15".to_string(),
        "edge" => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36 Edg/91.0.864.70".to_string(),
        "none" => String::new(),
        "golang" => "Go-http-client/1.1".to_string(),
        "curl" => "curl/7.68.0".to_string(),
        _ => user_agent.to_string(),
    }
}

fn parse_pem_chain(pem_chain: &str) -> Vec<String> {
    let pem_chain = pem_chain.replace("\r\n", "\n").replace('\r', "\n");
    let begin_marker = "-----BEGIN CERTIFICATE-----";
    let end_marker = "-----END CERTIFICATE-----";
    let mut certs = Vec::new();
    let mut index = 0;

    while index < pem_chain.len() {
        let Some(begin_offset) = pem_chain[index..].find(begin_marker) else {
            break;
        };
        let begin_index = index + begin_offset;
        let Some(end_offset) = pem_chain[begin_index..].find(end_marker) else {
            break;
        };
        let end_index = begin_index + end_offset;
        let base64_start = begin_index + begin_marker.len();
        let base64_content = pem_chain[base64_start..end_index]
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        certs.push(format!("{begin_marker}\n{base64_content}\n{end_marker}\n"));
        index = end_index + end_marker.len();
    }

    certs
}

fn wireguard_public_key(protocol_extra: &ProtocolExtraItem) -> Option<String> {
    nonempty_string(protocol_extra.wg_public_key.as_deref())
}

fn wireguard_allowed_ips(protocol_extra: &ProtocolExtraItem) -> Vec<String> {
    split_list(protocol_extra.wg_allowed_ips.as_deref().unwrap_or_default())
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| {
            WIREGUARD_DEFAULT_ALLOWED_IPS
                .iter()
                .map(|item| (*item).to_string())
                .collect()
        })
}

fn parse_i32(value: Option<&str>) -> Option<i32> {
    value.and_then(|value| value.trim().parse::<i32>().ok())
}

fn parse_i32_list(value: Option<&str>) -> Option<Vec<i32>> {
    let values = split_list(value.unwrap_or_default())?
        .into_iter()
        .filter_map(|item| item.trim().parse::<i32>().ok())
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn split_list(value: &str) -> Option<Vec<String>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        value
            .replace(['\n', '\r'], "")
            .split(',')
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn first_list_value(value: Option<&str>) -> String {
    split_list(value.unwrap_or_default())
        .and_then(|items| {
            items
                .into_iter()
                .map(|item| item.trim().to_string())
                .find(|item| !item.is_empty())
        })
        .unwrap_or_default()
}

fn nonempty_string(value: Option<&str>) -> Option<String> {
    nonempty_str(value).map(str::to_string)
}

fn nonempty_str(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn trimmed(value: &str) -> &str {
    value.trim()
}

fn inbound_protocol_tag(protocol: InboundProtocol) -> &'static str {
    match protocol {
        InboundProtocol::socks => "socks",
        InboundProtocol::socks2 => "socks2",
        InboundProtocol::socks3 => "socks3",
        InboundProtocol::pac => "pac",
        InboundProtocol::api => "api",
        InboundProtocol::api2 => "api2",
        InboundProtocol::mixed => "mixed",
        InboundProtocol::speedtest => "speedtest",
    }
}

fn inbound_port(app_config: &AppConfig, protocol: InboundProtocol) -> i32 {
    app_config
        .inbound
        .iter()
        .find(|item| item.protocol == inbound_protocol_tag(InboundProtocol::socks))
        .map(|item| item.local_port)
        .or_else(|| app_config.inbound.first().map(|item| item.local_port))
        .unwrap_or(DEFAULT_LOCAL_PORT)
        + protocol.as_i32()
}

fn state_port2(app_config: &AppConfig, is_tun_enabled: bool) -> i32 {
    inbound_port(app_config, InboundProtocol::api2) + i32::from(is_tun_enabled)
}

fn exe_name(process: &str) -> String {
    process
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(process)
        .trim_end_matches(".exe")
        .to_string()
}

fn is_loopback_address(address: &str) -> bool {
    let address = address.trim_matches(['[', ']']);
    address.eq_ignore_ascii_case("localhost")
        || address
            .parse::<IpAddr>()
            .is_ok_and(|ip_address| ip_address.is_loopback())
}

fn is_private_network(address: &str) -> bool {
    let address = address.trim_matches(['[', ']']);
    if address.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match address.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => address.is_private() || address.is_loopback(),
        Ok(IpAddr::V6(address)) => {
            address.is_loopback() || (address.segments()[0] & 0xfe00) == 0xfc00
        }
        Err(_) => false,
    }
}

fn value_from_config(config: &SingboxConfig) -> Value {
    serde_json::to_value(config).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{golden, CoreGenPlatform, CoreType, DnsItem, RoutingItem};

    #[test]
    fn singbox_outbound_vless_ws_tls_mux_matches_golden() {
        let mut config = AppConfig::default();
        config.core_basic_item.enable_fragment = true;
        config.core_basic_item.mux_enabled = true;
        config.core_basic_item.def_user_agent = "chrome".to_string();

        let node = ProfileItem {
            index_id: "n-vless".to_string(),
            config_type: ConfigType::VLESS,
            core_type: Some(CoreType::sing_box),
            remarks: "vless-ws".to_string(),
            address: "server.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000011".to_string(),
            network: "ws".to_string(),
            stream_security: "tls".to_string(),
            sni: "tls.example".to_string(),
            alpn: "h2,http/1.1".to_string(),
            fingerprint: "firefox".to_string(),
            ech_config_list: "ech.example+https://dns.example/dns-query".to_string(),
            mux_enabled: Some(true),
            protocol_extra: ProtocolExtraItem {
                vless_encryption: Some("none".to_string()),
                ..ProtocolExtraItem::default()
            },
            transport_extra: TransportExtraItem {
                host: Some("cdn.example".to_string()),
                path: Some("/ws?ed=2048".to_string()),
                ..TransportExtraItem::default()
            },
            ..ProfileItem::default()
        };

        let generated = generate_singbox_config(&test_context(config, node))
            .expect("sing-box config should generate");
        let proxy = generated
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == PROXY_TAG)
            .expect("proxy outbound");
        let value =
            serde_json::to_value(proxy).expect("sing-box VLESS outbound should serialize to JSON");
        assert_no_nulls(&value);

        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/singbox/outbounds/vless_ws_tls_mux.json"
        ))
        .expect("sing-box VLESS outbound golden fixture should parse as JSON");
        golden::assert_json_eq("singbox-outbound-vless-ws-tls-mux", &expected, &value);

        let full_value =
            serde_json::to_value(generated).expect("sing-box config should serialize to JSON");
        assert_no_nulls(&full_value);
        assert_eq!(
            full_value.pointer("/experimental/clash_api/external_controller"),
            Some(&Value::String("127.0.0.1:10813".to_string()))
        );
        assert_eq!(
            full_value.pointer("/experimental/cache_file/enabled"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn singbox_tls_insecure_requires_application_gate() {
        let node = ProfileItem {
            allow_insecure: "true".to_string(),
            ..base_remote_node()
        };
        let context = test_context(AppConfig::default(), node.clone());
        assert_eq!(
            build_outbound(&context, &node)
                .tls
                .expect("tls settings should be generated")
                .insecure,
            Some(false)
        );

        let mut config = AppConfig::default();
        config.core_basic_item.def_allow_insecure = true;
        let context = test_context(config.clone(), node.clone());
        assert_eq!(
            build_outbound(&context, &node)
                .tls
                .expect("tls settings should be generated")
                .insecure,
            Some(true)
        );

        let node = ProfileItem {
            allow_insecure: "false".to_string(),
            ..node
        };
        let context = test_context(config, node.clone());
        assert_eq!(
            build_outbound(&context, &node)
                .tls
                .expect("tls settings should be generated")
                .insecure,
            Some(false)
        );
    }

    #[test]
    fn singbox_pinned_cert_and_reality_force_insecure_false() {
        let mut config = AppConfig::default();
        config.core_basic_item.def_allow_insecure = true;

        let pinned_node = ProfileItem {
            allow_insecure: "true".to_string(),
            cert: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----".to_string(),
            ..base_remote_node()
        };
        let context = test_context(config.clone(), pinned_node.clone());
        let tls = build_outbound(&context, &pinned_node)
            .tls
            .expect("tls settings should be generated");
        assert_eq!(tls.insecure, Some(false));
        assert!(tls.certificate.is_some());

        let reality_node = ProfileItem {
            allow_insecure: "true".to_string(),
            stream_security: STREAM_SECURITY_REALITY.to_string(),
            public_key: "reality-public-key".to_string(),
            short_id: "reality-short-id".to_string(),
            ..base_remote_node()
        };
        let context = test_context(config, reality_node.clone());
        let tls = build_outbound(&context, &reality_node)
            .tls
            .expect("tls settings should be generated");
        assert_eq!(tls.insecure, Some(false));
        assert!(tls.reality.is_some());
    }

    #[test]
    fn singbox_transport_hosts_use_first_authority() {
        let node = ProfileItem {
            network: "ws".to_string(),
            sni: String::new(),
            transport_extra: TransportExtraItem {
                host: Some("one.example, two.example".to_string()),
                path: Some("/ws".to_string()),
                ..TransportExtraItem::default()
            },
            ..base_remote_node()
        };
        let context = test_context(AppConfig::default(), node.clone());
        let outbound = build_outbound(&context, &node);
        assert_eq!(
            outbound
                .transport
                .as_ref()
                .and_then(|transport| transport.headers.as_ref())
                .and_then(|headers| headers.host.as_deref()),
            Some("one.example")
        );
        assert_eq!(
            outbound
                .tls
                .as_ref()
                .and_then(|tls| tls.server_name.as_deref()),
            Some("one.example")
        );

        let node = ProfileItem {
            network: "httpupgrade".to_string(),
            sni: String::new(),
            transport_extra: TransportExtraItem {
                host: Some("upgrade.example, backup.example".to_string()),
                path: Some("/up".to_string()),
                ..TransportExtraItem::default()
            },
            ..base_remote_node()
        };
        let context = test_context(AppConfig::default(), node.clone());
        let outbound = build_outbound(&context, &node);
        assert_eq!(
            outbound
                .transport
                .as_ref()
                .and_then(|transport| transport.host.as_ref()),
            Some(&Value::String("upgrade.example".to_string()))
        );
        assert_eq!(
            outbound
                .tls
                .as_ref()
                .and_then(|tls| tls.server_name.as_deref()),
            Some("upgrade.example")
        );

        let node = ProfileItem {
            config_type: ConfigType::Trojan,
            password: "secret".to_string(),
            network: "grpc".to_string(),
            sni: String::new(),
            transport_extra: TransportExtraItem {
                grpc_authority: Some("grpc-one.example, grpc-two.example".to_string()),
                grpc_service_name: Some("svc".to_string()),
                ..TransportExtraItem::default()
            },
            ..base_remote_node()
        };
        let context = test_context(AppConfig::default(), node.clone());
        let outbound = build_outbound(&context, &node);
        assert_eq!(
            outbound
                .tls
                .as_ref()
                .and_then(|tls| tls.server_name.as_deref()),
            Some("grpc-one.example")
        );

        let node = ProfileItem {
            config_type: ConfigType::Shadowsocks,
            password: "secret".to_string(),
            network: "ws".to_string(),
            stream_security: String::new(),
            protocol_extra: ProtocolExtraItem {
                ss_method: Some("aes-128-gcm".to_string()),
                ..ProtocolExtraItem::default()
            },
            transport_extra: TransportExtraItem {
                host: Some("plugin-one.example, plugin-two.example".to_string()),
                path: Some("/plugin".to_string()),
                ..TransportExtraItem::default()
            },
            ..base_remote_node()
        };
        let context = test_context(AppConfig::default(), node.clone());
        let outbound = build_outbound(&context, &node);
        assert_eq!(
            outbound.plugin_opts.as_deref(),
            Some("mode=websocket;host=plugin-one.example;path=/plugin;mux=0")
        );
    }

    #[test]
    fn singbox_invalid_ports_are_rejected_or_skipped() {
        assert!(parse_dns_address("1.1.1.1:70000").is_none());
        assert!(parse_dns_address("https://dns.example:70000/dns-query").is_none());
        assert!(parse_dns_address("8.8.8.8:0").is_none());

        let fallback = parse_dns_address_or_default("1.1.1.1:70000", DEFAULT_DIRECT_DNS);
        assert_ne!(fallback.server_port, Some(70000));

        let node = ProfileItem {
            port: 70000,
            ..base_remote_node()
        };
        let error = generate_singbox_config(&test_context(AppConfig::default(), node))
            .expect_err("invalid node port should be rejected");
        assert!(matches!(
            error,
            SingboxConfigError::InvalidNodePort { port: 70000, .. }
        ));
    }

    #[test]
    fn singbox_outbound_proxy_chain_detour_matches_golden() {
        let n1 = socks_node("n1", "node-1");
        let n2 = socks_node("n2", "node-2");
        let chain = ProfileItem {
            index_id: "chain".to_string(),
            config_type: ConfigType::ProxyChain,
            core_type: Some(CoreType::sing_box),
            remarks: "chain".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some("n1,n2".to_string()),
                group_type: Some("ProxyChain".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let mut context = test_context(AppConfig::default(), chain);
        context.all_proxies_map.insert(n1.index_id.clone(), n1);
        context.all_proxies_map.insert(n2.index_id.clone(), n2);

        let generated = generate_singbox_config(&context).expect("sing-box config should generate");
        let value = serde_json::to_value(&generated.outbounds)
            .expect("sing-box proxy chain outbounds should serialize to JSON");
        assert_no_nulls(&value);

        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/singbox/outbounds/proxy_chain_detour.json"
        ))
        .expect("sing-box proxy chain golden fixture should parse as JSON");
        golden::assert_json_eq("singbox-proxy-chain-detour", &expected, &value);
    }

    #[test]
    fn singbox_outbound_live_protocol_matrix_serializes_without_nulls() {
        let cases = vec![
            (
                "vmess",
                ProfileItem {
                    index_id: "vmess".to_string(),
                    config_type: ConfigType::VMess,
                    password: "00000000-0000-0000-0000-000000000021".to_string(),
                    protocol_extra: ProtocolExtraItem {
                        alter_id: Some("0".to_string()),
                        vmess_security: Some(DEFAULT_SECURITY.to_string()),
                        ..ProtocolExtraItem::default()
                    },
                    ..base_remote_node()
                },
            ),
            (
                "shadowsocks",
                ProfileItem {
                    index_id: "ss".to_string(),
                    config_type: ConfigType::Shadowsocks,
                    password: "secret".to_string(),
                    protocol_extra: ProtocolExtraItem {
                        ss_method: Some("2022-blake3-aes-128-gcm".to_string()),
                        ..ProtocolExtraItem::default()
                    },
                    ..base_remote_node()
                },
            ),
            ("socks", socks_node("socks", "socks")),
            (
                "http",
                ProfileItem {
                    index_id: "http".to_string(),
                    config_type: ConfigType::HTTP,
                    username: "user".to_string(),
                    password: "pass".to_string(),
                    ..base_remote_node()
                },
            ),
            (
                "vless",
                ProfileItem {
                    index_id: "vless".to_string(),
                    config_type: ConfigType::VLESS,
                    password: "00000000-0000-0000-0000-000000000022".to_string(),
                    protocol_extra: ProtocolExtraItem {
                        vless_encryption: Some("none".to_string()),
                        ..ProtocolExtraItem::default()
                    },
                    ..base_remote_node()
                },
            ),
            (
                "trojan",
                ProfileItem {
                    index_id: "trojan".to_string(),
                    config_type: ConfigType::Trojan,
                    password: "secret".to_string(),
                    ..base_remote_node()
                },
            ),
            (
                "hysteria2",
                ProfileItem {
                    index_id: "hy2".to_string(),
                    config_type: ConfigType::Hysteria2,
                    password: "secret".to_string(),
                    protocol_extra: ProtocolExtraItem {
                        salamander_pass: Some("obfs".to_string()),
                        ports: Some("443,8443-8445".to_string()),
                        ..ProtocolExtraItem::default()
                    },
                    ..base_remote_node()
                },
            ),
            (
                "tuic",
                ProfileItem {
                    index_id: "tuic".to_string(),
                    config_type: ConfigType::TUIC,
                    username: "00000000-0000-0000-0000-000000000023".to_string(),
                    password: "secret".to_string(),
                    protocol_extra: ProtocolExtraItem {
                        congestion_control: Some("bbr".to_string()),
                        ..ProtocolExtraItem::default()
                    },
                    ..base_remote_node()
                },
            ),
            (
                "anytls",
                ProfileItem {
                    index_id: "anytls".to_string(),
                    config_type: ConfigType::Anytls,
                    password: "secret".to_string(),
                    ..base_remote_node()
                },
            ),
            (
                "naive",
                ProfileItem {
                    index_id: "naive".to_string(),
                    config_type: ConfigType::Naive,
                    username: "user".to_string(),
                    password: "pass".to_string(),
                    protocol_extra: ProtocolExtraItem {
                        naive_quic: Some(true),
                        congestion_control: Some("bbr".to_string()),
                        insecure_concurrency: Some(2),
                        ..ProtocolExtraItem::default()
                    },
                    ..base_remote_node()
                },
            ),
        ];

        for (expected_type, node) in cases {
            let generated = generate_singbox_config(&test_context(AppConfig::default(), node))
                .expect("sing-box config should generate");
            let proxy = generated
                .outbounds
                .iter()
                .find(|outbound| outbound.tag == PROXY_TAG)
                .expect("proxy outbound");
            assert_eq!(proxy.r#type, expected_type);
            assert_no_nulls(
                &serde_json::to_value(proxy)
                    .expect("sing-box protocol matrix outbound should serialize to JSON"),
            );
        }

        let wireguard = ProfileItem {
            index_id: "wg".to_string(),
            config_type: ConfigType::WireGuard,
            password: "private-key".to_string(),
            protocol_extra: ProtocolExtraItem {
                wg_public_key: Some("public-key".to_string()),
                wg_interface_address: Some("172.16.0.2/32,fd00::2/128".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..base_remote_node()
        };
        let generated = generate_singbox_config(&test_context(AppConfig::default(), wireguard))
            .expect("sing-box config should generate");
        assert_eq!(generated.endpoints.len(), 1);
        assert_eq!(generated.endpoints[0].r#type, "wireguard");
        assert_no_nulls(
            &serde_json::to_value(&generated.endpoints[0])
                .expect("sing-box wireguard endpoint should serialize to JSON"),
        );
    }

    #[test]
    fn singbox_selector_policy_group_order_dedupe_and_urltest_match_golden() {
        let n1 = socks_node("n1", "node-1");
        let n2 = socks_node("n2", "node-2");
        let group = ProfileItem {
            index_id: "group".to_string(),
            config_type: ConfigType::PolicyGroup,
            core_type: Some(CoreType::sing_box),
            remarks: "fallback".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some("n1,n1,n2".to_string()),
                group_type: Some("PolicyGroup".to_string()),
                multiple_load: Some(MultipleLoad::Fallback),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let mut context = test_context(AppConfig::default(), group);
        context.all_proxies_map.insert(n1.index_id.clone(), n1);
        context.all_proxies_map.insert(n2.index_id.clone(), n2);

        let generated = generate_singbox_config(&context).expect("sing-box config should generate");
        let value = serde_json::to_value(&generated.outbounds)
            .expect("sing-box policy group outbounds should serialize to JSON");
        assert_no_nulls(&value);

        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/singbox/outbounds/policy_group_selector.json"
        ))
        .expect("sing-box policy group golden fixture should parse as JSON");
        golden::assert_json_eq("singbox-policy-group-selector", &expected, &value);
    }

    #[test]
    fn singbox_dns_fakeip_typed_schema_and_rulesets_match_golden() {
        let (dns_context, _) = singbox_routing_dns_snapshot_contexts();
        let dns_generated =
            generate_singbox_config(&dns_context).expect("sing-box config should generate");
        let dns_value = serde_json::to_value(
            dns_generated
                .dns
                .as_ref()
                .expect("sing-box DNS config should be generated"),
        )
        .expect("sing-box DNS config should serialize to JSON");
        assert_no_nulls(&dns_value);
        let expected_dns: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/singbox/dns/fakeip_typed.json"
        ))
        .expect("sing-box fakeip DNS golden fixture should parse as JSON");
        golden::assert_json_eq("singbox-dns-fakeip-typed", &expected_dns, &dns_value);

        let ruleset_value = serde_json::to_value(
            dns_generated
                .route
                .rule_set
                .as_ref()
                .expect("sing-box DNS rulesets should be generated"),
        )
        .expect("sing-box DNS rulesets should serialize to JSON");
        let expected_ruleset: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/singbox/route/rulesets_from_dns.json"
        ))
        .expect("sing-box DNS ruleset golden fixture should parse as JSON");
        golden::assert_json_eq(
            "singbox-rulesets-from-dns",
            &expected_ruleset,
            &ruleset_value,
        );
    }

    #[test]
    fn singbox_ruleset_generation_prefers_resolved_local_asset_paths() {
        let (mut dns_context, _) = singbox_routing_dns_snapshot_contexts();
        dns_context.singbox_ruleset_paths.insert(
            "geosite-cn".to_string(),
            "/tmp/VoyaVPN/bin/srss/geosite-cn.srs".to_string(),
        );

        let generated =
            generate_singbox_config(&dns_context).expect("sing-box config should generate");
        let rule_set = generated.route.rule_set.expect("rulesets");
        let local = rule_set
            .iter()
            .find(|ruleset| ruleset.tag.as_deref() == Some("geosite-cn"))
            .expect("geosite-cn");
        let remote = rule_set
            .iter()
            .find(|ruleset| ruleset.tag.as_deref() == Some("geosite-google"))
            .expect("geosite-google");

        assert_eq!(local.r#type.as_deref(), Some("local"));
        assert_eq!(
            local.path.as_deref(),
            Some("/tmp/VoyaVPN/bin/srss/geosite-cn.srs")
        );
        assert_eq!(local.url, None);
        assert_eq!(remote.r#type.as_deref(), Some("remote"));
        assert_eq!(remote.download_detour.as_deref(), Some(PROXY_TAG));
    }

    #[test]
    fn singbox_invalid_inline_custom_rulesets_are_reported() {
        let (mut dns_context, _) = singbox_routing_dns_snapshot_contexts();
        dns_context
            .routing_item
            .as_mut()
            .expect("routing item")
            .custom_ruleset_path4_singbox = "[{\"tag\":\"geosite-cn\"}]".to_string();

        let error = generate_singbox_config(&dns_context)
            .expect_err("missing custom ruleset fields should fail generation");
        assert!(matches!(
            error,
            SingboxConfigError::CustomRulesetMissingRequiredFields { index: 0 }
        ));

        dns_context
            .routing_item
            .as_mut()
            .expect("routing item")
            .custom_ruleset_path4_singbox = "[{\"tag\":\"geosite-cn\"}".to_string();
        let error = generate_singbox_config(&dns_context)
            .expect_err("invalid custom ruleset JSON should fail generation");
        assert!(matches!(error, SingboxConfigError::CustomRulesetJson(_)));
    }

    #[test]
    fn singbox_dns_raw_override_uses_typed_custom_schema_and_protect_rules() {
        let mut context = test_context(AppConfig::default(), base_remote_node());
        context.protect_domain_list = vec!["ech.example".to_string()];
        context.raw_dns_item = Some(DnsItem {
            enabled: true,
            core_type: CoreType::sing_box,
            normal_dns: Some(
                r#"{"servers":[{"tag":"remote","type":"udp","server":"1.1.1.1","detour":"proxy"}],"rules":[],"final":"remote"}"#.to_string(),
            ),
            domain_dns_address: Some("9.9.9.9".to_string()),
            ..DnsItem::default()
        });

        let generated = generate_singbox_config(&context).expect("sing-box config should generate");
        let dns = generated.dns.expect("raw dns");

        assert!(dns.servers.iter().any(|server| {
            server.tag == SINGBOX_LOCAL_DNS_TAG && server.server.as_deref() == Some("9.9.9.9")
        }));
        assert!(dns.rules.iter().any(|rule| {
            rule.domain.as_ref() == Some(&vec!["ech.example".to_string()])
                && rule.server.as_deref() == Some(SINGBOX_LOCAL_DNS_TAG)
        }));
        assert_no_nulls(
            &serde_json::to_value(&dns).expect("sing-box raw DNS should serialize to JSON"),
        );
    }

    #[test]
    fn singbox_negative_ip_rules_use_and_and_skip_negative_only_rules() {
        let mut context = test_context(AppConfig::default(), base_remote_node());
        context.routing_item = Some(RoutingItem {
            rule_set: vec![
                RulesItem {
                    outbound_tag: Some(DIRECT_TAG.to_string()),
                    ip: Some(vec!["10.0.0.0/8".to_string(), "!10.1.0.0/16".to_string()]),
                    ..RulesItem::default()
                },
                RulesItem {
                    outbound_tag: Some(BLOCK_TAG.to_string()),
                    ip: Some(vec!["!geoip:private".to_string()]),
                    port: Some("443".to_string()),
                    ..RulesItem::default()
                },
            ],
            ..RoutingItem::default()
        });

        let generated = generate_singbox_config(&context).expect("sing-box config should generate");
        let logical_rule = generated
            .route
            .rules
            .iter()
            .find(|rule| {
                rule.r#type.as_deref() == Some("logical")
                    && rule.outbound.as_deref() == Some(DIRECT_TAG)
            })
            .expect("logical negative IP rule");
        assert_eq!(logical_rule.mode.as_deref(), Some("and"));
        let nested = logical_rule.rules.as_ref().expect("nested rules");
        assert_eq!(nested.len(), 2);
        assert_eq!(
            nested[0].ip_cidr.as_ref(),
            Some(&vec!["10.0.0.0/8".to_string()])
        );
        assert_eq!(nested[1].invert, Some(true));
        assert_eq!(
            nested[1].ip_cidr.as_ref(),
            Some(&vec!["10.1.0.0/16".to_string()])
        );
        assert!(!generated.route.rules.iter().any(|rule| {
            rule.action.as_deref() == Some("reject") && rule.port.as_ref() == Some(&vec![443])
        }));
    }

    #[test]
    fn singbox_custom_dns_protect_rules_reference_existing_server_tags() {
        let mut context = test_context(AppConfig::default(), base_remote_node());
        context.protect_domain_list = vec!["ech.example".to_string()];
        context.raw_dns_item = Some(DnsItem {
            enabled: true,
            core_type: CoreType::sing_box,
            normal_dns: Some(
                r#"{"servers":[{"tag":"only-local","type":"udp","server":"1.1.1.1"}],"rules":[],"final":"only-local"}"#.to_string(),
            ),
            domain_dns_address: Some("9.9.9.9".to_string()),
            ..DnsItem::default()
        });

        let generated = generate_singbox_config(&context).expect("sing-box config should generate");
        let dns = generated.dns.expect("raw dns");
        let server_tags = dns
            .servers
            .iter()
            .map(|server| server.tag.as_str())
            .collect::<BTreeSet<_>>();
        for rule in dns.rules.iter().filter_map(|rule| rule.server.as_deref()) {
            assert!(server_tags.contains(rule));
        }
        assert!(dns.rules.iter().any(|rule| {
            rule.clash_mode.as_deref() == Some("Global")
                && rule.server.as_deref() == Some("only-local")
        }));
    }

    #[test]
    fn singbox_wireguard_uses_allowed_ips_and_rejects_empty_public_key() {
        let wireguard = ProfileItem {
            index_id: "wg".to_string(),
            config_type: ConfigType::WireGuard,
            password: "private-key".to_string(),
            protocol_extra: ProtocolExtraItem {
                wg_public_key: Some("public-key".to_string()),
                wg_allowed_ips: Some("10.0.0.0/8,192.168.0.0/16".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..base_remote_node()
        };
        let generated = generate_singbox_config(&test_context(AppConfig::default(), wireguard))
            .expect("sing-box config should generate");
        assert_eq!(
            generated.endpoints[0].peers[0].allowed_ips,
            vec!["10.0.0.0/8".to_string(), "192.168.0.0/16".to_string()]
        );

        let missing_public_key = ProfileItem {
            index_id: "wg-missing-key".to_string(),
            config_type: ConfigType::WireGuard,
            password: "private-key".to_string(),
            ..base_remote_node()
        };
        let error =
            generate_singbox_config(&test_context(AppConfig::default(), missing_public_key))
                .expect_err("missing WireGuard public key should fail");
        assert!(matches!(
            error,
            SingboxConfigError::MissingWireGuardPublicKey { .. }
        ));
    }

    #[test]
    fn singbox_tun_inbound_and_route_match_golden() {
        let (_, tun_context) = singbox_routing_dns_snapshot_contexts();
        let generated =
            generate_singbox_config(&tun_context).expect("sing-box config should generate");
        let inbounds_value = serde_json::to_value(&generated.inbounds)
            .expect("sing-box tun inbounds should serialize to JSON");
        assert_no_nulls(&inbounds_value);
        let expected_inbounds: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/singbox/inbounds/tun.json"
        ))
        .expect("sing-box tun inbounds golden fixture should parse as JSON");
        golden::assert_json_eq("singbox-tun-inbounds", &expected_inbounds, &inbounds_value);

        let route_value = serde_json::to_value(&generated.route)
            .expect("sing-box route should serialize to JSON");
        assert_no_nulls(&route_value);
        let expected_route: Value =
            serde_json::from_str(include_str!("../../../tests/golden/singbox/route/tun.json"))
                .expect("sing-box tun route golden fixture should parse as JSON");
        golden::assert_json_eq("singbox-tun-route", &expected_route, &route_value);
    }

    fn singbox_routing_dns_snapshot_contexts() -> (CoreConfigContext, CoreConfigContext) {
        let mut dns_config = AppConfig::default();
        dns_config.simple_dns_item.fake_ip = Some(true);
        dns_config.simple_dns_item.global_fake_ip = Some(true);
        dns_config.simple_dns_item.direct_dns =
            Some("https://resolver.example/dns-query".to_string());
        dns_config.simple_dns_item.remote_dns =
            Some("https://cloudflare-dns.com/dns-query".to_string());
        dns_config.simple_dns_item.hosts =
            Some("resolver.example 1.1.1.1\nblock.test #3\ncname.test target.example".to_string());
        dns_config.simple_dns_item.strategy4_freedom = Some("UseIPv4".to_string());
        dns_config.simple_dns_item.strategy4_proxy = Some("UseIPv6".to_string());
        dns_config.simple_dns_item.direct_expected_ips = Some("geoip:cn,192.0.2.0/24".to_string());
        let mut dns_context = test_context(dns_config, base_remote_node());
        dns_context.routing_item = Some(RoutingItem {
            rule_set: vec![
                RulesItem {
                    outbound_tag: Some(DIRECT_TAG.to_string()),
                    domain: Some(vec!["geosite:cn".to_string()]),
                    rule_type: Some(RuleType::DNS),
                    ..RulesItem::default()
                },
                RulesItem {
                    outbound_tag: Some(PROXY_TAG.to_string()),
                    domain: Some(vec!["geosite:google".to_string()]),
                    rule_type: Some(RuleType::DNS),
                    ..RulesItem::default()
                },
            ],
            ..RoutingItem::default()
        });

        let mut tun_config = AppConfig::default();
        tun_config.tun_mode_item.enable_tun = true;
        tun_config.tun_mode_item.mtu = 1500;
        tun_config.tun_mode_item.stack = "system".to_string();
        tun_config.tun_mode_item.strict_route = false;
        tun_config.tun_mode_item.enable_ipv6_address = false;
        tun_config.simple_dns_item.add_common_hosts = Some(false);
        tun_config.simple_dns_item.block_binding_query = Some(false);
        let mut tun_context = test_context(tun_config, base_remote_node());
        tun_context.is_tun_enabled = true;

        (dns_context, tun_context)
    }

    fn test_context(app_config: AppConfig, node: ProfileItem) -> CoreConfigContext {
        let mut all_proxies_map = BTreeMap::new();
        all_proxies_map.insert(node.index_id.clone(), node.clone());
        let simple_dns_item = app_config.simple_dns_item.clone();
        CoreConfigContext {
            node,
            run_core_type: CoreType::sing_box,
            app_config,
            simple_dns_item,
            all_proxies_map,
            platform: CoreGenPlatform::Linux,
            ..CoreConfigContext::default()
        }
    }

    fn base_remote_node() -> ProfileItem {
        ProfileItem {
            core_type: Some(CoreType::sing_box),
            remarks: "remote".to_string(),
            address: "server.example".to_string(),
            port: 443,
            network: DEFAULT_NETWORK.to_string(),
            stream_security: "tls".to_string(),
            sni: "server.example".to_string(),
            ..ProfileItem::default()
        }
    }

    fn socks_node(index_id: &str, remarks: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::SOCKS,
            core_type: Some(CoreType::sing_box),
            remarks: remarks.to_string(),
            address: LOOPBACK.to_string(),
            port: 1080,
            username: "user".to_string(),
            password: "pass".to_string(),
            network: DEFAULT_NETWORK.to_string(),
            mux_enabled: Some(false),
            ..ProfileItem::default()
        }
    }

    fn assert_no_nulls(value: &Value) {
        match value {
            Value::Null => panic!("sing-box JSON must not contain null"),
            Value::Array(items) => {
                for item in items {
                    assert_no_nulls(item);
                }
            }
            Value::Object(object) => {
                for item in object.values() {
                    assert_no_nulls(item);
                }
            }
            Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
}
