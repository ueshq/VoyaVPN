use std::{collections::BTreeMap, net::IpAddr};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{
    AppConfig, ConfigType, CoreConfigContext, DnsItem, FullConfigTemplateItem, InItem,
    InboundProtocol, MultipleLoad, ProfileItem, ProtocolExtraItem, RuleType, RulesItem,
    TransportExtraItem, BLOCK_TAG, DEFAULT_BOOTSTRAP_DNS, DEFAULT_DIRECT_DNS, DEFAULT_LOCAL_PORT,
    DEFAULT_REMOTE_DNS, DIRECT_TAG, LOOPBACK, PROXY_TAG,
};

const BALANCER_TAG_SUFFIX: &str = "-balancer";
const DEFAULT_SECURITY: &str = "auto";
const DEFAULT_NETWORK: &str = "raw";
const DEFAULT_RAW_HTTP_PATH: &str = "/";
const DNS_OUTBOUND_TAG: &str = "dns";
const DNS_TAG: &str = "dns-module";
const GRPC_MULTI_MODE: &str = "multi";
const HYSTERIA_NETWORK: &str = "hysteria";
const API_TAG: &str = "api";
const API_PROTOCOL: &str = "dokodemo-door";
const RAW_HEADER_HTTP: &str = "http";
const ROUTING_RULE_COMMA: &str = "<COMMA>";
const STREAM_SECURITY_TLS: &str = "tls";
const STREAM_SECURITY_REALITY: &str = "reality";
const USER_EMAIL: &str = "t@t.tt";
const DIRECT_DNS_TAG: &str = "direct-dns";
const GEOIP_PREFIX: &str = "geoip:";
const GEOSITE_PREFIX: &str = "geosite:";
const IP_IF_NON_MATCH: &str = "IPIfNonMatch";
const AS_IS: &str = "AsIs";
const WIREGUARD_DEFAULT_ADDRESS: &str = "172.16.0.2/32";
const WIREGUARD_DEFAULT_MTU: i32 = 1280;
const XRAY_TUN_INBOUND_TAG: &str = "tun";
const XRAY_FAKE_DNS_POOL: &str = "198.18.0.0/15";
pub const DEFAULT_XRAY_DNS_NORMAL: &str = r#"{
  "hosts": {
    "dns.google": "8.8.8.8",
    "proxy.example.com": "127.0.0.1"
  },
  "servers": [
    {
      "address": "1.1.1.1",
      "skipFallback": true,
      "domains": [
        "geosite:google"
      ]
    },
    {
      "address": "223.5.5.5",
      "skipFallback": true,
      "domains": [
        "geosite:cn"
      ],
      "expectIPs": [
        "geoip:cn"
      ]
    },
    "1.1.1.1",
    "8.8.8.8",
    "https://dns.google/dns-query"
  ]
}"#;

const LIVE_XRAY_NETWORKS: &[&str] = &["raw", "xhttp", "kcp", "grpc", "ws", "httpupgrade"];
const VMESS_SECURITIES: &[&str] = &[
    "aes-128-gcm",
    "chacha20-poly1305",
    DEFAULT_SECURITY,
    "none",
    "zero",
];
const SS_SECURITIES_IN_XRAY: &[&str] = &[
    "aes-256-gcm",
    "aes-128-gcm",
    "chacha20-poly1305",
    "chacha20-ietf-poly1305",
    "xchacha20-poly1305",
    "xchacha20-ietf-poly1305",
    "none",
    "plain",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
];
const XHTTP_MODES: &[&str] = &["auto", "packet-up", "stream-up", "stream-one"];

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log: Option<XrayLog>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inbounds: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outbounds: Vec<XrayOutbound>,
    pub routing: XrayRouting,
    #[serde(rename = "fakedns", skip_serializing_if = "Option::is_none")]
    pub fake_dns: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observatory: Option<XrayObservatory>,
    #[serde(rename = "burstObservatory", skip_serializing_if = "Option::is_none")]
    pub burst_observatory: Option<XrayBurstObservatory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
}

impl XrayConfig {
    #[must_use]
    pub fn sample() -> Self {
        Self {
            log: Some(XrayLog {
                access: Some("Vaccess.log".to_string()),
                error: Some("Verror.log".to_string()),
                loglevel: Some("warning".to_string()),
            }),
            dns: None,
            inbounds: Vec::new(),
            outbounds: vec![
                XrayOutbound::builtin(DIRECT_TAG, "freedom"),
                XrayOutbound::builtin(BLOCK_TAG, "blackhole"),
            ],
            routing: XrayRouting {
                domain_strategy: "IPIfNonMatch".to_string(),
                rules: vec![XrayRule {
                    r#type: Some("field".to_string()),
                    inbound_tag: Some(vec![API_TAG.to_string()]),
                    outbound_tag: Some(API_TAG.to_string()),
                    ..XrayRule::default()
                }],
                balancers: None,
            },
            fake_dns: None,
            metrics: None,
            policy: None,
            stats: None,
            observatory: None,
            burst_observatory: None,
            remarks: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayLog {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loglevel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayOutbound {
    pub tag: String,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_through: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<XrayOutboundSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_settings: Option<XrayStreamSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mux: Option<XrayMux>,
}

impl XrayOutbound {
    fn builtin(tag: &str, protocol: &str) -> Self {
        Self {
            tag: tag.to_string(),
            protocol: protocol.to_string(),
            send_through: None,
            target_strategy: None,
            settings: None,
            stream_settings: None,
            mux: None,
        }
    }

    fn proxy_sample(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            protocol: "vmess".to_string(),
            send_through: None,
            target_strategy: None,
            settings: Some(XrayOutboundSettings {
                vnext: Some(vec![XrayVnext {
                    address: "v2ray.cool".to_string(),
                    port: 10086,
                    users: vec![XrayUser {
                        id: Some("a3482e88-686a-4a58-8126-99c9df64b7bf".to_string()),
                        security: Some(DEFAULT_SECURITY.to_string()),
                        ..XrayUser::default()
                    }],
                }]),
                servers: Some(vec![XrayServer {
                    address: "v2ray.cool".to_string(),
                    method: Some("chacha20".to_string()),
                    ota: Some(false),
                    password: Some("123456".to_string()),
                    port: 10086,
                    level: Some(1),
                    ..XrayServer::default()
                }]),
                ..XrayOutboundSettings::default()
            }),
            stream_settings: Some(XrayStreamSettings {
                network: "tcp".to_string(),
                ..XrayStreamSettings::default()
            }),
            mux: Some(XrayMux {
                enabled: false,
                concurrency: None,
                xudp_concurrency: None,
                xudp_proxy_udp443: None,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayOutboundSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vnext: Option<Vec<XrayVnext>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<XrayServer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_level: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peers: Option<Vec<XrayWireguardPeer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_kernel_tun: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayVnext {
    pub address: String,
    pub port: i32,
    pub users: Vec<XrayUser>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayServer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ota: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub port: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<XraySocksUser>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XraySocksUser {
    pub user: String,
    pub pass: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayWireguardPeer {
    pub endpoint: String,
    pub public_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_shared_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayMux {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xudp_concurrency: Option<i32>,
    #[serde(rename = "xudpProxyUDP443", skip_serializing_if = "Option::is_none")]
    pub xudp_proxy_udp443: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayStreamSettings {
    pub network: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_settings: Option<XrayTlsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_settings: Option<XrayRawSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcp_settings: Option<XrayKcpSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_settings: Option<XrayWsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub httpupgrade_settings: Option<XrayHttpUpgradeSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhttp_settings: Option<XrayXhttpSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_settings: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quic_settings: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality_settings: Option<XrayTlsSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_settings: Option<XrayGrpcSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hysteria_settings: Option<XrayHysteriaSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finalmask: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sockopt: Option<XraySockopt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayTlsSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_insecure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spider_x: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mldsa65_verify: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificates: Option<Vec<XrayCertificateSettings>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_peer_cert_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_system_root: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ech_config_list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ech_force_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ech_sockopt: Option<XraySockopt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayCertificateSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayRawSettings {
    pub header: XrayHeader,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayHeader {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayKcpSettings {
    pub mtu: i32,
    pub tti: i32,
    pub uplink_capacity: i32,
    pub downlink_capacity: i32,
    pub cwnd_multiplier: i32,
    pub max_sending_window: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayWsSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<XrayHeaders>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct XrayHeaders {
    #[serde(rename = "User-Agent")]
    pub user_agent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayHttpUpgradeSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<XrayHeaders>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayXhttpSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct XrayGrpcSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    #[serde(rename = "serviceName", skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(rename = "multiMode")]
    pub multi_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check_timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permit_without_stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_windows_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayHysteriaSettings {
    pub version: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct XraySockopt {
    #[serde(rename = "dialerProxy", skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    #[serde(rename = "interface", skip_serializing_if = "Option::is_none")]
    pub interface: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayRouting {
    pub domain_strategy: String,
    #[serde(default)]
    pub rules: Vec<XrayRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balancers: Option<Vec<XrayBalancer>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbound_tag: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outbound_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balancer_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayBalancer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<XrayBalancerStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayBalancerStrategy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<XrayBalancerStrategySettings>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayBalancerStrategySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<i32>,
    #[serde(rename = "maxRTT", skip_serializing_if = "Option::is_none")]
    pub max_rtt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baselines: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub costs: Option<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayObservatory {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_selector: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_concurrency: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayBurstObservatory {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_selector: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping_config: Option<XrayBurstPingConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XrayBurstPingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connectivity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

#[must_use]
pub fn generate_xray_config(context: &CoreConfigContext) -> XrayConfig {
    let mut config = XrayConfig::sample();
    gen_log(&mut config, context);
    gen_inbounds(&mut config, context);

    let proxy_outbounds = build_xray_proxy_outbounds(context, PROXY_TAG);
    let proxy_tag_count = proxy_outbounds
        .iter()
        .filter(|outbound| outbound.tag.starts_with(PROXY_TAG))
        .count();
    config.outbounds.splice(0..0, proxy_outbounds);

    if proxy_tag_count > 1 {
        let multiple_load = context
            .node
            .protocol_extra
            .multiple_load
            .unwrap_or(MultipleLoad::LeastPing);
        gen_observatory(
            &mut config,
            multiple_load,
            PROXY_TAG,
            &context.app_config.speed_test_item.speed_ping_test_url,
        );
        gen_balancer(&mut config, multiple_load, PROXY_TAG);
    }

    if context.is_tun_enabled {
        config
            .outbounds
            .push(XrayOutbound::builtin(DNS_OUTBOUND_TAG, "dns"));
    }

    gen_routing(&mut config, context);
    gen_dns(&mut config, context);
    gen_statistic(&mut config, context);

    if context.app_config.core_basic_item.enable_fragment {
        apply_xray_outbound_fragment(&mut config.outbounds, &context.app_config);
    }
    apply_outbound_bind_interface(&mut config, context);
    apply_outbound_send_through(&mut config, context);

    let final_rule = build_final_rule(&config);
    if final_rule
        .balancer_tag
        .as_ref()
        .is_some_and(|tag| !tag.is_empty())
    {
        config.routing.rules.push(final_rule);
    }

    config
}

#[must_use]
pub fn generate_xray_config_value(context: &CoreConfigContext) -> Value {
    let config = generate_xray_config(context);
    apply_full_config_template(context, &config)
}

#[must_use]
pub fn generate_xray_config_json(context: &CoreConfigContext) -> String {
    canonical_json_string(&generate_xray_config_value(context))
}

fn gen_log(config: &mut XrayConfig, context: &CoreConfigContext) {
    let mut log = config.log.clone().unwrap_or(XrayLog {
        access: None,
        error: None,
        loglevel: None,
    });
    log.loglevel = Some(context.app_config.core_basic_item.loglevel.clone());
    if context.app_config.core_basic_item.log_enabled {
        log.access = Some("Vaccess.log".to_string());
        log.error = Some("Verror.log".to_string());
    } else {
        log.access = None;
        log.error = None;
    }
    config.log = Some(log);
}

fn gen_inbounds(config: &mut XrayConfig, context: &CoreConfigContext) {
    let in_item = context
        .app_config
        .inbound
        .first()
        .cloned()
        .unwrap_or_default();
    let listen_port = inbound_port(&context.app_config, InboundProtocol::socks);
    let mut primary = build_inbound(&in_item, InboundProtocol::socks);
    let is_using_local_mixed_port =
        context.node.address == LOOPBACK && context.node.port == listen_port;

    config.inbounds.clear();
    if !context.is_tun_enabled || !is_using_local_mixed_port {
        if in_item.allow_lan_conn && !in_item.new_port4_lan {
            if let Some(object) = primary.as_object_mut() {
                object.insert("listen".to_string(), Value::String("0.0.0.0".to_string()));
            }
        }
        config.inbounds.push(primary.clone());

        if in_item.second_local_port_enabled {
            config
                .inbounds
                .push(build_inbound(&in_item, InboundProtocol::socks2));
        }

        if in_item.allow_lan_conn && in_item.new_port4_lan {
            let mut lan_inbound = build_inbound(&in_item, InboundProtocol::socks3);
            if let Some(object) = lan_inbound.as_object_mut() {
                object.insert("listen".to_string(), Value::String("0.0.0.0".to_string()));
            }
            if !trimmed(&in_item.user).is_empty() && !trimmed(&in_item.pass).is_empty() {
                if let Some(settings) = lan_inbound
                    .get_mut("settings")
                    .and_then(Value::as_object_mut)
                {
                    settings.insert("auth".to_string(), Value::String("password".to_string()));
                    settings.insert(
                        "accounts".to_string(),
                        json!([{ "user": in_item.user.clone(), "pass": in_item.pass.clone() }]),
                    );
                }
            }
            config.inbounds.push(lan_inbound);
        }
    }

    if context.is_tun_enabled {
        config.inbounds.push(build_tun_inbound(context, &primary));
    }
}

fn build_inbound(in_item: &InItem, protocol: InboundProtocol) -> Value {
    let dest_override = in_item
        .dest_override
        .clone()
        .unwrap_or_else(|| vec!["http".to_string(), "tls".to_string()]);
    json!({
        "tag": inbound_protocol_tag(protocol),
        "port": in_item.local_port + protocol.as_i32(),
        "protocol": "mixed",
        "listen": LOOPBACK,
        "settings": {
            "auth": "noauth",
            "udp": in_item.udp_enabled,
            "allowTransparent": false
        },
        "sniffing": {
            "enabled": in_item.sniffing_enabled,
            "destOverride": dest_override,
            "routeOnly": in_item.route_only
        }
    })
}

fn build_tun_inbound(context: &CoreConfigContext, primary_inbound: &Value) -> Value {
    let mtu = if context.app_config.tun_mode_item.mtu > 0 {
        context.app_config.tun_mode_item.mtu
    } else {
        WIREGUARD_DEFAULT_MTU
    };
    let mut gateway = vec![
        "172.18.0.1/30".to_string(),
        "fdfe:dcba:9876::1/126".to_string(),
    ];
    if !context.app_config.tun_mode_item.enable_ipv6_address {
        gateway = vec!["172.18.0.1/30".to_string()];
    }

    let mut inbound = json!({
        "tag": XRAY_TUN_INBOUND_TAG,
        "protocol": "tun",
        "settings": {
            "name": if context.is_macos() { "utun0" } else { "xray_tun" },
            "MTU": mtu,
            "gateway": gateway,
            "autoSystemRoutingTable": ["0.0.0.0/0", "::/0"],
            "autoOutboundsInterface": "auto"
        },
        "sniffing": primary_inbound
            .get("sniffing")
            .cloned()
            .unwrap_or_else(|| json!({
                "enabled": true,
                "destOverride": ["http", "tls"],
                "routeOnly": false
            }))
    });

    if let Some(bind_interface) =
        nonempty_str(context.app_config.core_basic_item.bind_interface.as_deref())
    {
        if let Some(settings) = inbound.get_mut("settings").and_then(Value::as_object_mut) {
            settings.insert(
                "autoOutboundsInterface".to_string(),
                Value::String(bind_interface.to_string()),
            );
        }
    }

    inbound
}

fn gen_routing(config: &mut XrayConfig, context: &CoreConfigContext) {
    if context.is_tun_enabled {
        config.routing.rules.extend([
            XrayRule {
                network: Some("udp".to_string()),
                port: Some("135,137-139,5353".to_string()),
                outbound_tag: Some(BLOCK_TAG.to_string()),
                ..XrayRule::default()
            },
            XrayRule {
                ip: Some(vec!["224.0.0.0/3".to_string(), "ff00::/8".to_string()]),
                outbound_tag: Some(BLOCK_TAG.to_string()),
                ..XrayRule::default()
            },
        ]);
        let (dns_exes, direct_exes) = build_routing_direct_exe();
        config.routing.rules.push(XrayRule {
            port: Some("53".to_string()),
            process: Some(dns_exes),
            outbound_tag: Some(DNS_OUTBOUND_TAG.to_string()),
            ..XrayRule::default()
        });
        config.routing.rules.push(XrayRule {
            process: Some(direct_exes),
            outbound_tag: Some(DIRECT_TAG.to_string()),
            ..XrayRule::default()
        });
        config.routing.rules.push(XrayRule {
            inbound_tag: Some(vec![XRAY_TUN_INBOUND_TAG.to_string()]),
            port: Some("53".to_string()),
            outbound_tag: Some(DNS_OUTBOUND_TAG.to_string()),
            ..XrayRule::default()
        });
    }

    config.routing.domain_strategy = context
        .app_config
        .routing_basic_item
        .domain_strategy
        .clone();
    if let Some(routing) = &context.routing_item {
        if !trimmed(&routing.domain_strategy).is_empty() {
            config.routing.domain_strategy = routing.domain_strategy.clone();
        }
        for item in &routing.rule_set {
            if item.enabled && item.rule_type != Some(RuleType::DNS) {
                gen_routing_user_rule(config, context, item);
            }
        }
    }

    let balancer_tags = config
        .routing
        .balancers
        .as_ref()
        .map(|balancers| {
            balancers
                .iter()
                .filter_map(|balancer| balancer.tag.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !balancer_tags.is_empty() {
        for rule in &mut config.routing.rules {
            let Some(outbound_tag) = rule.outbound_tag.clone() else {
                continue;
            };
            let candidate = format!("{outbound_tag}{BALANCER_TAG_SUFFIX}");
            if balancer_tags.contains(&candidate) {
                rule.balancer_tag = Some(candidate);
                rule.outbound_tag = None;
            }
        }
    }
}

fn gen_routing_user_rule(
    config: &mut XrayConfig,
    context: &CoreConfigContext,
    user_rule: &RulesItem,
) {
    let outbound_tag = gen_routing_user_rule_outbound(
        config,
        context,
        user_rule.outbound_tag.as_deref().unwrap_or(PROXY_TAG),
    );
    let base = XrayRule {
        port: nonempty_str(user_rule.port.as_deref()).map(str::to_string),
        network: nonempty_str(user_rule.network.as_deref()).map(str::to_string),
        inbound_tag: nonempty_vec(user_rule.inbound_tag.clone()),
        outbound_tag: Some(outbound_tag),
        ip: nonempty_vec(user_rule.ip.clone()),
        domain: nonempty_vec(user_rule.domain.clone()),
        protocol: nonempty_vec(user_rule.protocol.clone()),
        process: nonempty_vec(user_rule.process.clone()),
        ..XrayRule::default()
    };

    let mut has_domain_ip_process = false;
    if let Some(domains) = &base.domain {
        let domains = domains
            .iter()
            .filter(|domain| !domain.starts_with('#'))
            .map(|domain| domain.replace(ROUTING_RULE_COMMA, ","))
            .collect::<Vec<_>>();
        if !domains.is_empty() {
            let mut rule = base.clone();
            rule.r#type = Some("field".to_string());
            rule.domain = Some(domains);
            rule.ip = None;
            rule.process = None;
            config.routing.rules.push(rule);
            has_domain_ip_process = true;
        }
    }
    if base.ip.as_ref().is_some_and(|items| !items.is_empty()) {
        let mut rule = base.clone();
        rule.r#type = Some("field".to_string());
        rule.domain = None;
        rule.process = None;
        config.routing.rules.push(rule);
        has_domain_ip_process = true;
    }
    if base.process.as_ref().is_some_and(|items| !items.is_empty()) {
        let mut rule = base.clone();
        rule.r#type = Some("field".to_string());
        rule.domain = None;
        rule.ip = None;
        config.routing.rules.push(rule);
        has_domain_ip_process = true;
    }
    if !has_domain_ip_process
        && (base.port.is_some()
            || base
                .protocol
                .as_ref()
                .is_some_and(|items| !items.is_empty())
            || base
                .inbound_tag
                .as_ref()
                .is_some_and(|items| !items.is_empty())
            || base.network.is_some())
    {
        let mut rule = base;
        rule.r#type = Some("field".to_string());
        config.routing.rules.push(rule);
    }
}

fn gen_routing_user_rule_outbound(
    config: &mut XrayConfig,
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
    if !node.config_type.is_group_type() && !xray_supports_config_type(node.config_type) {
        return PROXY_TAG.to_string();
    }

    let tag = format!("{}-{PROXY_TAG}-{}", node.index_id, node.remarks);
    if config
        .outbounds
        .iter()
        .any(|outbound| outbound.tag.starts_with(&tag))
    {
        return tag;
    }

    let mut nested_context = context.clone();
    nested_context.node = node.clone();
    let proxy_outbounds = build_xray_proxy_outbounds(&nested_context, &tag);
    let proxy_tag_count = proxy_outbounds
        .iter()
        .filter(|outbound| outbound.tag.starts_with(&tag))
        .count();
    config.outbounds.extend(proxy_outbounds);
    if proxy_tag_count > 1 {
        let multiple_load = node
            .protocol_extra
            .multiple_load
            .unwrap_or(MultipleLoad::LeastPing);
        gen_observatory(
            config,
            multiple_load,
            &tag,
            &context.app_config.speed_test_item.speed_ping_test_url,
        );
        gen_balancer(config, multiple_load, &tag);
    }

    tag
}

fn gen_dns(config: &mut XrayConfig, context: &CoreConfigContext) {
    if context
        .raw_dns_item
        .as_ref()
        .is_some_and(|item| item.enabled)
    {
        gen_dns_custom(config, context);
        if config.routing.domain_strategy != IP_IF_NON_MATCH {
            return;
        }
        if let Some(dns) = config.dns.as_mut().and_then(Value::as_object_mut) {
            dns.insert("tag".to_string(), Value::String(DNS_TAG.to_string()));
        }
        config.routing.rules.push(XrayRule {
            r#type: Some("field".to_string()),
            inbound_tag: Some(vec![DNS_TAG.to_string()]),
            outbound_tag: Some(PROXY_TAG.to_string()),
            ..XrayRule::default()
        });
        return;
    }

    let simple_dns = &context.simple_dns_item;
    apply_dns_domain_strategy(config, context);

    let mut dns = Map::new();
    let servers = fill_dns_servers(context, config);
    dns.insert("servers".to_string(), Value::Array(servers));
    if let Some(hosts) = fill_dns_hosts(simple_dns) {
        dns.insert("hosts".to_string(), hosts);
    }
    if simple_dns.serve_stale == Some(true) {
        dns.insert("serveStale".to_string(), Value::Bool(true));
    }
    if simple_dns.parallel_query == Some(true) {
        dns.insert("enableParallelQuery".to_string(), Value::Bool(true));
    }
    dns.insert("tag".to_string(), Value::String(DNS_TAG.to_string()));

    if simple_dns.fake_ip == Some(true) {
        config.fake_dns = Some(vec![json!({
            "ipPool": XRAY_FAKE_DNS_POOL,
            "poolSize": 65535
        })]);
    }

    let final_rule = build_final_rule(config);
    config.routing.rules.push(XrayRule {
        r#type: Some("field".to_string()),
        inbound_tag: Some(vec![DNS_TAG.to_string()]),
        outbound_tag: final_rule.outbound_tag,
        balancer_tag: final_rule.balancer_tag,
        ..XrayRule::default()
    });
    config.dns = Some(Value::Object(dns));
}

fn apply_dns_domain_strategy(config: &mut XrayConfig, context: &CoreConfigContext) {
    let simple_dns = &context.simple_dns_item;
    let strategy4_freedom = simple_dns.strategy4_freedom.as_deref().unwrap_or(AS_IS);
    if !strategy4_freedom.is_empty() && strategy4_freedom != AS_IS {
        if let Some(outbound) = config
            .outbounds
            .iter_mut()
            .find(|outbound| outbound.protocol == "freedom" && outbound.tag == DIRECT_TAG)
        {
            outbound.settings = Some(XrayOutboundSettings {
                domain_strategy: Some(strategy4_freedom.to_string()),
                user_level: Some(0),
                ..XrayOutboundSettings::default()
            });
        }
    }

    let strategy4_proxy = simple_dns.strategy4_proxy.as_deref().unwrap_or(AS_IS);
    if !strategy4_proxy.is_empty() && strategy4_proxy != AS_IS {
        for outbound in &mut config.outbounds {
            if xray_supports_protocol(&outbound.protocol) {
                outbound.target_strategy = Some(strategy4_proxy.to_string());
            }
        }
    }
}

fn fill_dns_servers(context: &CoreConfigContext, config: &mut XrayConfig) -> Vec<Value> {
    let simple_dns = &context.simple_dns_item;
    let direct_dns_addresses =
        parse_dns_addresses(simple_dns.direct_dns.as_deref(), DEFAULT_DIRECT_DNS);
    let remote_dns_addresses =
        parse_dns_addresses(simple_dns.remote_dns.as_deref(), DEFAULT_REMOTE_DNS);
    let bootstrap_dns_addresses =
        parse_dns_addresses(simple_dns.bootstrap_dns.as_deref(), DEFAULT_BOOTSTRAP_DNS);

    let mut direct_domain_list = Vec::new();
    let mut direct_geosite_list = Vec::new();
    let mut proxy_domain_list = Vec::new();
    let mut proxy_geosite_list = Vec::new();
    let mut expected_domain_list = Vec::new();
    let expected_ips = split_dns_expected_ips(simple_dns.direct_expected_ips.as_deref());
    let region_name = expected_ips
        .iter()
        .filter_map(|ip| {
            ip.strip_prefix(GEOIP_PREFIX)
                .or_else(|| ip.strip_prefix("GEOIP:"))
                .filter(|region| !region.is_empty())
        })
        .next_back()
        .map(str::to_string)
        .unwrap_or_default();
    let dns_server_domains = dns_server_domains(&direct_dns_addresses, &remote_dns_addresses);

    if let Some(routing) = &context.routing_item {
        for item in &routing.rule_set {
            if !item.enabled
                || item.rule_type == Some(RuleType::Routing)
                || item.domain.as_ref().is_none_or(Vec::is_empty)
            {
                continue;
            }
            for domain in item.domain.as_ref().into_iter().flatten() {
                if domain.starts_with('#') {
                    continue;
                }
                let normalized = domain.replace(ROUTING_RULE_COMMA, ",");
                if item.outbound_tag.as_deref() == Some(DIRECT_TAG) {
                    if normalized.starts_with(GEOSITE_PREFIX) || normalized.starts_with("ext:") {
                        let is_expected = !region_name.is_empty()
                            && (normalized.ends_with(&format!("-{region_name}"))
                                || normalized.ends_with(&format!("@{region_name}"))
                                || normalized == format!("{GEOSITE_PREFIX}{region_name}"));
                        if is_expected {
                            expected_domain_list.push(normalized);
                        } else {
                            direct_geosite_list.push(normalized);
                        }
                    } else {
                        direct_domain_list.push(normalized);
                    }
                } else if item.outbound_tag.as_deref() != Some(BLOCK_TAG) {
                    if normalized.starts_with(GEOSITE_PREFIX) || normalized.starts_with("ext:") {
                        proxy_geosite_list.push(normalized);
                    } else {
                        proxy_domain_list.push(normalized);
                    }
                }
            }
        }
    }

    direct_domain_list.extend(context.protect_domain_list.clone());

    let mut servers = Vec::new();
    let mut direct_dns_tag_index = 1;
    add_dns_servers(
        &mut servers,
        &remote_dns_addresses,
        &proxy_domain_list,
        false,
        &[],
        &mut direct_dns_tag_index,
    );
    add_dns_servers(
        &mut servers,
        &direct_dns_addresses,
        &direct_domain_list,
        true,
        &[],
        &mut direct_dns_tag_index,
    );
    add_dns_servers(
        &mut servers,
        &remote_dns_addresses,
        &proxy_geosite_list,
        false,
        &[],
        &mut direct_dns_tag_index,
    );
    add_dns_servers(
        &mut servers,
        &direct_dns_addresses,
        &direct_geosite_list,
        true,
        &[],
        &mut direct_dns_tag_index,
    );
    add_dns_servers(
        &mut servers,
        &direct_dns_addresses,
        &expected_domain_list,
        true,
        &expected_ips,
        &mut direct_dns_tag_index,
    );
    if !dns_server_domains.is_empty() {
        add_dns_servers(
            &mut servers,
            &bootstrap_dns_addresses,
            &dns_server_domains,
            false,
            &[],
            &mut direct_dns_tag_index,
        );
    }

    if use_direct_dns(context.routing_item.as_ref()) {
        for dns in &direct_dns_addresses {
            let mut dns_server = create_dns_server(dns, &[], &[]);
            if let Some(object) = dns_server.as_object_mut() {
                object.insert(
                    "tag".to_string(),
                    Value::String(format!("{DIRECT_DNS_TAG}-{direct_dns_tag_index}")),
                );
                object.insert("skipFallback".to_string(), Value::Bool(false));
            }
            direct_dns_tag_index += 1;
            servers.push(dns_server);
        }
    } else {
        servers.extend(remote_dns_addresses.into_iter().map(Value::String));
    }

    let direct_dns_tags = servers
        .iter()
        .filter_map(|server| server.get("tag").and_then(Value::as_str))
        .filter(|tag| tag.starts_with(DIRECT_DNS_TAG))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !direct_dns_tags.is_empty() {
        config.routing.rules.push(XrayRule {
            r#type: Some("field".to_string()),
            inbound_tag: Some(direct_dns_tags),
            outbound_tag: Some(DIRECT_TAG.to_string()),
            ..XrayRule::default()
        });
    }

    servers
}

fn fill_dns_hosts(simple_dns: &crate::SimpleDnsItem) -> Option<Value> {
    if simple_dns.add_common_hosts == Some(false)
        && simple_dns.use_system_hosts == Some(false)
        && simple_dns
            .hosts
            .as_deref()
            .is_none_or(|hosts| hosts.trim().is_empty())
    {
        return None;
    }

    let mut hosts = BTreeMap::<String, Value>::new();
    if simple_dns.add_common_hosts == Some(true) {
        for (host, addresses) in predefined_hosts() {
            hosts.insert(host.to_string(), json!(addresses));
        }
    }
    for (host, addresses) in parse_hosts_to_dictionary(simple_dns.hosts.as_deref()) {
        hosts.insert(host, json!(addresses));
    }

    Some(json!(hosts))
}

fn gen_dns_custom(config: &mut XrayConfig, context: &CoreConfigContext) {
    let Some(item) = context.raw_dns_item.as_ref() else {
        return;
    };
    apply_custom_dns_freedom_strategy(config, item);

    let custom_dns = if context.is_tun_enabled {
        item.tun_dns.as_deref()
    } else {
        item.normal_dns.as_deref()
    }
    .and_then(|dns| nonempty_str(Some(dns)))
    .unwrap_or(DEFAULT_XRAY_DNS_NORMAL);
    let mut dns = serde_json::from_str::<Value>(custom_dns).unwrap_or_else(|_| {
        let servers = split_delimited(custom_dns, ',')
            .into_iter()
            .map(Value::String)
            .collect::<Vec<_>>();
        json!({ "servers": servers })
    });

    if let Some(hosts) = dns.get_mut("hosts").and_then(Value::as_object_mut) {
        for value in hosts.values_mut() {
            if let Some(ip) = value.as_str().map(str::to_string) {
                *value = json!([ip]);
            }
        }
    }
    fill_dns_domains_custom(&mut dns, context);
    config.dns = Some(dns);
}

fn apply_custom_dns_freedom_strategy(config: &mut XrayConfig, item: &DnsItem) {
    let Some(domain_strategy) = nonempty_str(item.domain_strategy4_freedom.as_deref()) else {
        return;
    };
    if let Some(outbound) = config
        .outbounds
        .iter_mut()
        .find(|outbound| outbound.protocol == "freedom" && outbound.tag == DIRECT_TAG)
    {
        outbound.settings = Some(XrayOutboundSettings {
            domain_strategy: Some(domain_strategy.to_string()),
            user_level: Some(0),
            ..XrayOutboundSettings::default()
        });
    }
}

fn fill_dns_domains_custom(dns: &mut Value, context: &CoreConfigContext) {
    if context.protect_domain_list.is_empty() {
        return;
    }
    let Some(servers) = dns.get_mut("servers").and_then(Value::as_array_mut) else {
        return;
    };
    let address = context
        .raw_dns_item
        .as_ref()
        .and_then(|item| nonempty_str(item.domain_dns_address.as_deref()))
        .unwrap_or(DEFAULT_BOOTSTRAP_DNS);
    servers.push(create_dns_server(
        address,
        &context.protect_domain_list,
        &[],
    ));
}

fn gen_statistic(config: &mut XrayConfig, context: &CoreConfigContext) {
    if !(context.app_config.gui_item.enable_statistics
        || context.app_config.gui_item.display_real_time_speed)
    {
        return;
    }

    config.stats = Some(json!({}));
    config.metrics = Some(json!({ "tag": API_TAG }));
    config.policy = Some(json!({
        "system": {
            "statsOutboundUplink": true,
            "statsOutboundDownlink": true
        }
    }));

    if !config
        .inbounds
        .iter()
        .any(|inbound| inbound.get("tag").and_then(Value::as_str) == Some(API_TAG))
    {
        config.inbounds.push(json!({
            "tag": API_TAG,
            "listen": LOOPBACK,
            "port": inbound_port(&context.app_config, InboundProtocol::api),
            "protocol": API_PROTOCOL,
            "settings": {
                "address": LOOPBACK
            }
        }));
    }

    if !config
        .routing
        .rules
        .iter()
        .any(|rule| rule.outbound_tag.as_deref() == Some(API_TAG))
    {
        config.routing.rules.push(XrayRule {
            inbound_tag: Some(vec![API_TAG.to_string()]),
            outbound_tag: Some(API_TAG.to_string()),
            r#type: Some("field".to_string()),
            ..XrayRule::default()
        });
    }
}

fn build_final_rule(config: &XrayConfig) -> XrayRule {
    let mut final_rule = XrayRule {
        r#type: Some("field".to_string()),
        network: Some("tcp,udp".to_string()),
        outbound_tag: Some(PROXY_TAG.to_string()),
        ..XrayRule::default()
    };
    let proxy_balancer_tag = format!("{PROXY_TAG}{BALANCER_TAG_SUFFIX}");
    if config.routing.balancers.as_ref().is_some_and(|balancers| {
        balancers
            .iter()
            .any(|balancer| balancer.tag.as_deref() == Some(proxy_balancer_tag.as_str()))
    }) {
        final_rule.outbound_tag = None;
        final_rule.balancer_tag = Some(proxy_balancer_tag);
    }
    if config.routing.domain_strategy == IP_IF_NON_MATCH {
        final_rule.network = None;
        final_rule.ip = Some(vec!["0.0.0.0/0".to_string(), "::/0".to_string()]);
    }
    final_rule
}

fn apply_outbound_bind_interface(config: &mut XrayConfig, context: &CoreConfigContext) {
    let Some(bind_interface) =
        nonempty_str(context.app_config.core_basic_item.bind_interface.as_deref())
    else {
        return;
    };
    if !(context.is_tun_enabled || context.is_windows()) {
        return;
    }

    for outbound in &mut config.outbounds {
        if !should_bind_net(outbound) {
            continue;
        }
        let stream_settings = outbound
            .stream_settings
            .get_or_insert_with(XrayStreamSettings::default);
        stream_settings
            .sockopt
            .get_or_insert_with(XraySockopt::default)
            .interface = Some(bind_interface.to_string());
        fill_xhttp_download_sockopt_string(outbound, "interface", bind_interface);
    }
}

fn apply_outbound_send_through(config: &mut XrayConfig, context: &CoreConfigContext) {
    let Some(send_through) =
        nonempty_str(context.app_config.core_basic_item.send_through.as_deref())
    else {
        return;
    };
    for outbound in &mut config.outbounds {
        outbound.send_through = should_bind_net(outbound).then(|| send_through.to_string());
    }
}

fn apply_full_config_template(context: &CoreConfigContext, config: &XrayConfig) -> Value {
    let Some(template) = context.full_config_template.as_ref() else {
        return value_from_config(config);
    };
    if !template.enabled {
        return value_from_config(config);
    }

    let template_json = template_json_for_context(template, context);
    let Some(template_json) = nonempty_str(template_json) else {
        return value_from_config(config);
    };
    let Ok(mut template_value) = serde_json::from_str::<Value>(template_json) else {
        return value_from_config(config);
    };
    let Some(template_object) = template_value.as_object_mut() else {
        return value_from_config(config);
    };

    merge_template_balancers(template_object, config);
    merge_template_observatory(template_object, "observatory", config.observatory.as_ref());
    merge_template_burst_observatory(
        template_object,
        "burstObservatory",
        config.burst_observatory.as_ref(),
    );

    let mut generated_outbounds = Vec::new();
    for outbound in &config.outbounds {
        let mut outbound = outbound.clone();
        let is_builtin = matches!(
            outbound.protocol.to_ascii_lowercase().as_str(),
            "blackhole" | "dns" | "freedom"
        );
        if is_builtin {
            if template.add_proxy_only == Some(true) {
                continue;
            }
        } else if let Some(proxy_detour) = nonempty_str(template.proxy_detour.as_deref()) {
            if outbound
                .stream_settings
                .as_ref()
                .and_then(|settings| settings.sockopt.as_ref())
                .and_then(|sockopt| sockopt.dialer_proxy.as_deref())
                .is_none_or(str::is_empty)
                && outbound_primary_address(&outbound)
                    .as_deref()
                    .is_none_or(|address| !is_private_network(address))
            {
                fill_dialer_proxy(&mut outbound, proxy_detour);
            }
        }
        generated_outbounds.push(value_from_outbound(&outbound));
    }

    if let Some(template_outbounds) = template_object
        .get("outbounds")
        .and_then(Value::as_array)
        .cloned()
    {
        generated_outbounds.extend(template_outbounds);
    }
    template_object.insert("outbounds".to_string(), Value::Array(generated_outbounds));

    template_value
}

fn merge_template_balancers(template_object: &mut Map<String, Value>, config: &XrayConfig) {
    let Some(balancers) = config.routing.balancers.as_ref() else {
        return;
    };
    if balancers.is_empty() {
        return;
    }

    let proxy_balancer_tag = format!("{PROXY_TAG}{BALANCER_TAG_SUFFIX}");
    if balancers
        .iter()
        .any(|balancer| balancer.tag.as_deref() == Some(proxy_balancer_tag.as_str()))
    {
        if let Some(rules) = template_object
            .get_mut("routing")
            .and_then(Value::as_object_mut)
            .and_then(|routing| routing.get_mut("rules"))
            .and_then(Value::as_array_mut)
        {
            for rule in rules {
                let Some(rule_object) = rule.as_object_mut() else {
                    continue;
                };
                if rule_object.get("outboundTag").and_then(Value::as_str) == Some(PROXY_TAG) {
                    rule_object.remove("outboundTag");
                    rule_object.insert(
                        "balancerTag".to_string(),
                        Value::String(proxy_balancer_tag.clone()),
                    );
                }
            }
        }
    }

    let routing = template_object
        .entry("routing".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(routing_object) = routing.as_object_mut() else {
        return;
    };
    let generated_balancers = serde_json::to_value(balancers).unwrap_or(Value::Null);
    if let Some(template_balancers) = routing_object
        .get_mut("balancers")
        .and_then(Value::as_array_mut)
    {
        if let Some(generated) = generated_balancers.as_array() {
            template_balancers.extend(generated.iter().cloned());
        }
    } else if !generated_balancers.is_null() {
        routing_object.insert("balancers".to_string(), generated_balancers);
    }
}

fn merge_template_observatory(
    template_object: &mut Map<String, Value>,
    key: &str,
    observatory: Option<&XrayObservatory>,
) {
    let Some(observatory) = observatory else {
        return;
    };
    let generated = serde_json::to_value(observatory).unwrap_or(Value::Null);
    merge_template_subject_selector(template_object, key, generated);
}

fn merge_template_burst_observatory(
    template_object: &mut Map<String, Value>,
    key: &str,
    observatory: Option<&XrayBurstObservatory>,
) {
    let Some(observatory) = observatory else {
        return;
    };
    let generated = serde_json::to_value(observatory).unwrap_or(Value::Null);
    merge_template_subject_selector(template_object, key, generated);
}

fn merge_template_subject_selector(
    template_object: &mut Map<String, Value>,
    key: &str,
    generated: Value,
) {
    if generated.is_null() {
        return;
    }
    if !template_object.contains_key(key) {
        template_object.insert(key.to_string(), generated);
        return;
    }

    let mut selectors = generated
        .get("subjectSelector")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(template_selectors) = template_object
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .and_then(|object| object.get_mut("subjectSelector"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    selectors.extend(template_selectors.iter().cloned());
    template_selectors.clear();
    let mut seen = Vec::new();
    for selector in selectors {
        let Some(selector) = selector.as_str() else {
            continue;
        };
        if !seen.iter().any(|item| item == selector) {
            seen.push(selector.to_string());
            template_selectors.push(Value::String(selector.to_string()));
        }
    }
}

fn template_json_for_context<'a>(
    template: &'a FullConfigTemplateItem,
    context: &CoreConfigContext,
) -> Option<&'a str> {
    if context.is_tun_enabled {
        template.tun_config.as_deref()
    } else {
        template.config.as_deref()
    }
}

fn build_routing_direct_exe() -> (Vec<String>, Vec<String>) {
    let dns_exes = [
        "v2ray",
        "wv2ray",
        "sing-box",
        "clash",
        "mihomo",
        "hysteria",
        "hysteria2",
        "naive",
        "tuic",
        "juicity",
        "brook",
        "overtls",
        "shadowquic",
        "mieru",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let mut direct_exes = dns_exes.clone();
    direct_exes.extend(["xray/".to_string(), "self/".to_string()]);
    (dns_exes, direct_exes)
}

fn inbound_protocol_tag(protocol: InboundProtocol) -> &'static str {
    match protocol {
        InboundProtocol::socks => "socks",
        InboundProtocol::socks2 => "socks2",
        InboundProtocol::socks3 => "socks3",
        InboundProtocol::pac => "pac",
        InboundProtocol::api => API_TAG,
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

fn nonempty_vec<T>(items: Option<Vec<T>>) -> Option<Vec<T>> {
    items.filter(|items| !items.is_empty())
}

fn parse_dns_addresses(input: Option<&str>, default_address: &str) -> Vec<String> {
    let source = input.unwrap_or(default_address);
    let separator = if source.contains(',') { ',' } else { ';' };
    let mut result = Vec::new();
    for address in source
        .split(separator)
        .map(str::trim)
        .filter(|address| !address.is_empty())
    {
        let normalized = if address
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("dhcp"))
        {
            LOOPBACK
        } else {
            address
        };
        push_unique(&mut result, normalized.to_string());
    }
    if result.is_empty() {
        result.push(default_address.to_string());
    }
    result
}

fn split_dns_expected_ips(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn dns_server_domains(
    direct_dns_addresses: &[String],
    remote_dns_addresses: &[String],
) -> Vec<String> {
    let mut domains = Vec::new();
    for dns in direct_dns_addresses.iter().chain(remote_dns_addresses) {
        let domain = parse_dns_domain(dns);
        if domain == "localhost" || domain.parse::<IpAddr>().is_ok() || domain.is_empty() {
            continue;
        }
        push_unique(&mut domains, format!("full:{domain}"));
    }
    domains
}

fn add_dns_servers(
    servers: &mut Vec<Value>,
    dns_addresses: &[String],
    domains: &[String],
    is_direct_dns: bool,
    expected_ips: &[String],
    direct_dns_tag_index: &mut i32,
) {
    if domains.is_empty() {
        return;
    }
    for dns_address in dns_addresses {
        let mut dns_server = create_dns_server(dns_address, domains, expected_ips);
        if is_direct_dns {
            if let Some(object) = dns_server.as_object_mut() {
                object.insert(
                    "tag".to_string(),
                    Value::String(format!("{DIRECT_DNS_TAG}-{direct_dns_tag_index}")),
                );
            }
            *direct_dns_tag_index += 1;
        }
        servers.push(dns_server);
    }
}

fn create_dns_server(dns_address: &str, domains: &[String], expected_ips: &[String]) -> Value {
    let parsed = parse_dns_address(dns_address);
    let mut object = Map::new();
    object.insert("address".to_string(), Value::String(parsed.address));
    if let Some(port) = parsed.port {
        object.insert("port".to_string(), json!(port));
    }
    object.insert("skipFallback".to_string(), Value::Bool(true));
    if !domains.is_empty() {
        object.insert("domains".to_string(), json!(domains));
    }
    if !expected_ips.is_empty() {
        object.insert("expectedIPs".to_string(), json!(expected_ips));
    }
    Value::Object(object)
}

#[derive(Debug, Clone)]
struct ParsedDnsAddress {
    address: String,
    port: Option<i32>,
}

fn parse_dns_address(dns_address: &str) -> ParsedDnsAddress {
    if let Ok(url) = url::Url::parse(dns_address) {
        let scheme = url.scheme();
        let host = url.host_str().unwrap_or_default();
        let port = url.port().map(i32::from);
        let address = if scheme.is_empty() || scheme.starts_with("udp") {
            host.to_string()
        } else if scheme.starts_with("tcp") {
            format!("{scheme}://{host}")
        } else {
            dns_address.to_string()
        };
        return ParsedDnsAddress { address, port };
    }

    if let Some((host, port)) = split_host_port(dns_address) {
        return ParsedDnsAddress {
            address: host.to_string(),
            port: port.parse::<i32>().ok(),
        };
    }

    ParsedDnsAddress {
        address: dns_address.to_string(),
        port: None,
    }
}

fn parse_dns_domain(dns_address: &str) -> String {
    if let Ok(url) = url::Url::parse(dns_address) {
        return url.host_str().unwrap_or_default().to_string();
    }
    split_host_port(dns_address)
        .map(|(host, _)| host.to_string())
        .unwrap_or_else(|| dns_address.to_string())
}

fn split_host_port(value: &str) -> Option<(&str, &str)> {
    let (host, port) = value.rsplit_once(':')?;
    if host.contains(':') || port.is_empty() || !port.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((host, port))
}

fn use_direct_dns(routing: Option<&crate::RoutingItem>) -> bool {
    let Some(last_rule) = routing.and_then(|routing| routing.rule_set.last()) else {
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

fn split_delimited(value: &str, delimiter: char) -> Vec<String> {
    value
        .split(delimiter)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.iter().any(|existing| existing == &item) {
        items.push(item);
    }
}

fn fill_xhttp_download_sockopt_string(outbound: &mut XrayOutbound, key: &str, value: &str) {
    let Some(extra) = outbound
        .stream_settings
        .as_mut()
        .and_then(|settings| settings.xhttp_settings.as_mut())
        .and_then(|settings| settings.extra.as_mut())
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let Some(download_settings) = extra
        .get_mut("downloadSettings")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let sockopt = download_settings
        .entry("sockopt".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(sockopt) = sockopt.as_object_mut() {
        sockopt.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn should_bind_net(outbound: &XrayOutbound) -> bool {
    if matches!(
        outbound.protocol.as_str(),
        "freedom" | "blackhole" | "dns" | "loopback"
    ) {
        return false;
    }
    if outbound
        .stream_settings
        .as_ref()
        .and_then(|settings| settings.sockopt.as_ref())
        .and_then(|sockopt| sockopt.dialer_proxy.as_deref())
        .is_some_and(|dialer_proxy| !dialer_proxy.is_empty())
    {
        return false;
    }

    outbound_primary_address(outbound)
        .as_deref()
        .is_none_or(|address| !is_loopback_address(address))
}

fn outbound_primary_address(outbound: &XrayOutbound) -> Option<String> {
    let settings = outbound.settings.as_ref()?;
    if let Some(server) = settings
        .servers
        .as_ref()
        .and_then(|servers| servers.first())
    {
        return Some(server.address.clone());
    }
    if let Some(vnext) = settings.vnext.as_ref().and_then(|items| items.first()) {
        return Some(vnext.address.clone());
    }
    match settings.address.as_ref() {
        Some(Value::String(address)) => Some(address.clone()),
        Some(Value::Array(addresses)) => addresses
            .first()
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
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

fn value_from_config(config: &XrayConfig) -> Value {
    serde_json::to_value(config).unwrap_or_else(|_| json!({}))
}

fn value_from_outbound(outbound: &XrayOutbound) -> Value {
    serde_json::to_value(outbound).unwrap_or_else(|_| json!({}))
}

fn canonical_json_string(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

fn xray_supports_config_type(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::VMess
            | ConfigType::Shadowsocks
            | ConfigType::SOCKS
            | ConfigType::HTTP
            | ConfigType::VLESS
            | ConfigType::Trojan
            | ConfigType::Hysteria2
            | ConfigType::WireGuard
    )
}

fn xray_supports_protocol(protocol: &str) -> bool {
    matches!(
        protocol,
        "vmess"
            | "shadowsocks"
            | "socks"
            | "http"
            | "vless"
            | "trojan"
            | "hysteria"
            | "hysteria2"
            | "wireguard"
    )
}

#[must_use]
pub fn build_xray_proxy_outbounds(
    context: &CoreConfigContext,
    base_tag_name: &str,
) -> Vec<XrayOutbound> {
    if context.node.config_type.is_group_type() {
        return build_group_proxy_outbounds(context, &context.node, base_tag_name);
    }
    vec![build_proxy_outbound(context, &context.node, base_tag_name)]
}

pub fn apply_xray_outbound_fragment(outbounds: &mut [XrayOutbound], app_config: &AppConfig) {
    let fragment_mask = json!({
        "type": "fragment",
        "settings": {
            "packets": app_config
                .fragment4_ray_item
                .packets
                .as_deref()
                .unwrap_or("tlshello"),
            "length": app_config
                .fragment4_ray_item
                .length
                .as_deref()
                .unwrap_or("50-100"),
            "delay": app_config
                .fragment4_ray_item
                .interval
                .as_deref()
                .unwrap_or("10-20"),
        }
    });
    let noise_mask = json!({
        "type": "noise",
        "settings": {
            "length": "10-20",
            "delay": "10-16",
        }
    });

    for outbound in outbounds {
        let Some(stream_settings) = outbound.stream_settings.as_mut() else {
            continue;
        };
        if stream_settings
            .security
            .as_deref()
            .is_none_or(str::is_empty)
            || stream_settings
                .sockopt
                .as_ref()
                .and_then(|sockopt| sockopt.dialer_proxy.as_deref())
                .is_some_and(|dialer_proxy| !dialer_proxy.is_empty())
        {
            continue;
        }

        let mut finalmask = stream_settings
            .finalmask
            .take()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        merge_mask_array_if_empty(&mut finalmask, "tcp", fragment_mask.clone());
        merge_mask_array_if_empty(&mut finalmask, "udp", noise_mask.clone());
        stream_settings.finalmask = Some(Value::Object(finalmask));
    }
}

fn build_group_proxy_outbounds(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<XrayOutbound> {
    match node.config_type {
        ConfigType::PolicyGroup => build_outbounds_list(context, node, base_tag_name),
        ConfigType::ProxyChain => build_chain_outbounds_list(context, node, base_tag_name),
        _ => Vec::new(),
    }
}

fn build_proxy_outbound(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> XrayOutbound {
    let mut outbound = XrayOutbound::proxy_sample(base_tag_name);
    fill_outbound(context, node, &mut outbound);
    outbound.tag = base_tag_name.to_string();
    outbound
}

fn fill_outbound(context: &CoreConfigContext, node: &ProfileItem, outbound: &mut XrayOutbound) {
    let protocol_extra = &node.protocol_extra;
    let mux_enabled = node
        .mux_enabled
        .unwrap_or(context.app_config.core_basic_item.mux_enabled);

    match node.config_type {
        ConfigType::VMess => {
            outbound.settings = Some(XrayOutboundSettings {
                vnext: Some(vec![XrayVnext {
                    address: node.address.clone(),
                    port: node.port,
                    users: vec![XrayUser {
                        id: Some(node.password.clone()),
                        alter_id: Some(parse_i32(protocol_extra.alter_id.as_deref()).unwrap_or(0)),
                        email: Some(USER_EMAIL.to_string()),
                        security: Some(vmess_security(protocol_extra)),
                        ..XrayUser::default()
                    }],
                }]),
                ..XrayOutboundSettings::default()
            });
            fill_outbound_mux(outbound, mux_enabled, mux_enabled, &context.app_config);
        }
        ConfigType::Shadowsocks => {
            outbound.settings = Some(XrayOutboundSettings {
                servers: Some(vec![XrayServer {
                    address: node.address.clone(),
                    port: node.port,
                    password: Some(node.password.clone()),
                    method: Some(shadowsocks_method(protocol_extra)),
                    uot: (protocol_extra.uot == Some(true)).then_some(true),
                    ota: Some(false),
                    level: Some(1),
                    ..XrayServer::default()
                }]),
                ..XrayOutboundSettings::default()
            });
            fill_outbound_mux(outbound, false, false, &context.app_config);
        }
        ConfigType::SOCKS | ConfigType::HTTP => {
            let users = (!trimmed(&node.username).is_empty()
                && !trimmed(&node.password).is_empty())
            .then(|| {
                vec![XraySocksUser {
                    user: node.username.clone(),
                    pass: node.password.clone(),
                    level: Some(1),
                }]
            });
            outbound.settings = Some(XrayOutboundSettings {
                servers: Some(vec![XrayServer {
                    address: node.address.clone(),
                    port: node.port,
                    users,
                    ..XrayServer::default()
                }]),
                ..XrayOutboundSettings::default()
            });
            fill_outbound_mux(outbound, false, false, &context.app_config);
        }
        ConfigType::VLESS => {
            let flow = trimmed(protocol_extra.flow.as_deref().unwrap_or_default());
            let mut user = XrayUser {
                id: Some(node.password.clone()),
                email: Some(USER_EMAIL.to_string()),
                encryption: protocol_extra.vless_encryption.clone(),
                ..XrayUser::default()
            };
            if flow.is_empty() {
                fill_outbound_mux(outbound, mux_enabled, mux_enabled, &context.app_config);
            } else {
                user.flow = Some(flow.to_string());
                fill_outbound_mux(outbound, false, mux_enabled, &context.app_config);
            }
            outbound.settings = Some(XrayOutboundSettings {
                vnext: Some(vec![XrayVnext {
                    address: node.address.clone(),
                    port: node.port,
                    users: vec![user],
                }]),
                ..XrayOutboundSettings::default()
            });
        }
        ConfigType::Trojan => {
            outbound.settings = Some(XrayOutboundSettings {
                servers: Some(vec![XrayServer {
                    address: node.address.clone(),
                    port: node.port,
                    password: Some(node.password.clone()),
                    ota: Some(false),
                    level: Some(1),
                    ..XrayServer::default()
                }]),
                ..XrayOutboundSettings::default()
            });
            fill_outbound_mux(outbound, false, false, &context.app_config);
        }
        ConfigType::Hysteria2 => {
            outbound.settings = Some(XrayOutboundSettings {
                version: Some(2),
                address: Some(Value::String(node.address.clone())),
                port: Some(node.port),
                ..XrayOutboundSettings::default()
            });
        }
        ConfigType::WireGuard => {
            outbound.settings = Some(wireguard_settings(node, protocol_extra));
        }
        _ => {}
    }

    outbound.protocol = protocol_name(node.config_type).to_string();
    if node.config_type == ConfigType::Hysteria2 {
        outbound.protocol = HYSTERIA_NETWORK.to_string();
    }
    fill_bound_stream_settings(context, node, outbound);
}

fn fill_outbound_mux(
    outbound: &mut XrayOutbound,
    enabled_tcp: bool,
    enabled_udp: bool,
    app_config: &AppConfig,
) {
    let mut mux = XrayMux {
        enabled: false,
        concurrency: Some(-1),
        xudp_concurrency: None,
        xudp_proxy_udp443: None,
    };
    if enabled_tcp {
        mux.enabled = true;
        mux.concurrency = app_config.mux4_ray_item.concurrency;
    } else if enabled_udp {
        mux.enabled = true;
        mux.xudp_concurrency = app_config.mux4_ray_item.xudp_concurrency;
        mux.xudp_proxy_udp443 = app_config.mux4_ray_item.xudp_proxy_udp443.clone();
    }
    outbound.mux = Some(mux);
}

fn fill_bound_stream_settings(
    context: &CoreConfigContext,
    node: &ProfileItem,
    outbound: &mut XrayOutbound,
) {
    let network = if node.config_type == ConfigType::Hysteria2 {
        HYSTERIA_NETWORK.to_string()
    } else {
        xray_network(node).to_string()
    };
    let transport = &node.transport_extra;
    let values = transport_values(&network, transport, context);
    let mut stream_settings = XrayStreamSettings {
        network: network.clone(),
        ..XrayStreamSettings::default()
    };

    if node.stream_security == STREAM_SECURITY_TLS {
        stream_settings.security = Some(node.stream_security.clone());
        stream_settings.tls_settings = Some(tls_settings(node, context, &values.host));
    }
    if node.stream_security == STREAM_SECURITY_REALITY {
        stream_settings.security = Some(node.stream_security.clone());
        stream_settings.reality_settings = Some(reality_settings(node, context));
    }

    match network.as_str() {
        "kcp" => fill_kcp_stream(context, &values, &mut stream_settings),
        "ws" => {
            stream_settings.ws_settings = Some(XrayWsSettings {
                host: nonempty_string(&values.host),
                path: nonempty_string(&values.path),
                headers: nonempty_string(&values.user_agent)
                    .map(|user_agent| XrayHeaders { user_agent }),
            });
        }
        "httpupgrade" => {
            stream_settings.httpupgrade_settings = Some(XrayHttpUpgradeSettings {
                host: nonempty_string(&values.host),
                path: nonempty_string(&values.path),
                headers: nonempty_string(&values.user_agent)
                    .map(|user_agent| XrayHeaders { user_agent }),
            });
        }
        "xhttp" => {
            stream_settings.network = "xhttp".to_string();
            stream_settings.xhttp_settings = Some(XrayXhttpSettings {
                path: nonempty_string(&values.path),
                host: nonempty_string(&values.host),
                mode: XHTTP_MODES
                    .contains(&values.header_type.as_str())
                    .then(|| values.header_type.clone()),
                extra: nonempty_string(&values.xhttp_extra)
                    .and_then(|extra| serde_json::from_str::<Value>(&extra).ok()),
            });
            fill_outbound_mux(outbound, false, false, &context.app_config);
        }
        "grpc" => {
            stream_settings.grpc_settings = Some(XrayGrpcSettings {
                authority: nonempty_string(&values.host),
                service_name: Some(values.path.clone()),
                multi_mode: values.header_type == GRPC_MULTI_MODE,
                idle_timeout: context.app_config.grpc_item.idle_timeout,
                health_check_timeout: context.app_config.grpc_item.health_check_timeout,
                permit_without_stream: context.app_config.grpc_item.permit_without_stream,
                initial_windows_size: context.app_config.grpc_item.initial_windows_size,
                user_agent: nonempty_string(&values.user_agent),
            });
        }
        HYSTERIA_NETWORK => fill_hysteria_stream(context, node, &mut stream_settings),
        _ => fill_raw_stream(&values, &mut stream_settings),
    }

    if !trimmed(&node.finalmask).is_empty() {
        if let Ok(finalmask) = serde_json::from_str::<Value>(&node.finalmask) {
            stream_settings.finalmask = Some(finalmask);
        }
    }

    outbound.stream_settings = Some(stream_settings);
}

fn fill_kcp_stream(
    context: &CoreConfigContext,
    values: &TransportValues,
    stream_settings: &mut XrayStreamSettings,
) {
    stream_settings.kcp_settings = Some(XrayKcpSettings {
        mtu: values.kcp_mtu,
        tti: context.app_config.kcp_item.tti,
        uplink_capacity: context.app_config.kcp_item.uplink_capacity,
        downlink_capacity: context.app_config.kcp_item.downlink_capacity,
        cwnd_multiplier: context.app_config.kcp_item.cwnd_multiplier,
        max_sending_window: context.app_config.kcp_item.max_sending_window,
    });

    let mut udp = Vec::new();
    if let Some(mask_type) = kcp_header_mask(&values.header_type) {
        udp.push(json!({ "type": mask_type }));
    }
    if values.kcp_seed.is_empty() {
        udp.push(json!({ "type": "mkcp-original" }));
    } else {
        udp.push(json!({
            "type": "mkcp-aes128gcm",
            "settings": { "password": values.kcp_seed }
        }));
    }
    stream_settings.finalmask = Some(json!({ "udp": udp }));
}

fn fill_hysteria_stream(
    context: &CoreConfigContext,
    node: &ProfileItem,
    stream_settings: &mut XrayStreamSettings,
) {
    let protocol_extra = &node.protocol_extra;
    let up_mbps = protocol_extra
        .up_mbps
        .filter(|value| *value >= 0)
        .unwrap_or(context.app_config.hysteria_item.up_mbps);
    let down_mbps = protocol_extra
        .down_mbps
        .filter(|value| *value >= 0)
        .unwrap_or(context.app_config.hysteria_item.down_mbps);
    let hop_interval = nonempty_str(protocol_extra.hop_interval.as_deref())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if context.app_config.hysteria_item.hop_interval >= 5 {
                context.app_config.hysteria_item.hop_interval.to_string()
            } else {
                "30".to_string()
            }
        });

    let mut quic_params = Map::new();
    if let Some(ports) = nonempty_str(protocol_extra.ports.as_deref()) {
        if ports.contains([':', '-', ',']) {
            quic_params.insert(
                "udpHop".to_string(),
                json!({
                    "ports": ports.replace(':', "-"),
                    "interval": hop_interval,
                }),
            );
        }
    }
    if up_mbps > 0 || down_mbps > 0 {
        quic_params.insert(
            "congestion".to_string(),
            Value::String("brutal".to_string()),
        );
        if up_mbps > 0 {
            quic_params.insert(
                "brutalUp".to_string(),
                Value::String(format!("{up_mbps}mbps")),
            );
        }
        if down_mbps > 0 {
            quic_params.insert(
                "brutalDown".to_string(),
                Value::String(format!("{down_mbps}mbps")),
            );
        }
    } else {
        quic_params.insert("congestion".to_string(), Value::String("bbr".to_string()));
    }

    let mut finalmask = Map::new();
    finalmask.insert("quicParams".to_string(), Value::Object(quic_params));
    if let Some(salamander_pass) = nonempty_str(protocol_extra.salamander_pass.as_deref()) {
        finalmask.insert(
            "udp".to_string(),
            json!([{
                "type": "salamander",
                "settings": { "password": salamander_pass.trim() }
            }]),
        );
    }

    stream_settings.hysteria_settings = Some(XrayHysteriaSettings {
        version: 2,
        auth: Some(node.password.clone()),
    });
    stream_settings.finalmask = Some(Value::Object(finalmask));
}

fn fill_raw_stream(values: &TransportValues, stream_settings: &mut XrayStreamSettings) {
    if values.header_type == RAW_HEADER_HTTP {
        stream_settings.raw_settings = Some(XrayRawSettings {
            header: XrayHeader {
                r#type: values.header_type.clone(),
                request: Some(raw_http_request(
                    &values.host,
                    &values.path,
                    &values.user_agent,
                )),
                response: None,
            },
        });
    }
}

fn tls_settings(
    node: &ProfileItem,
    context: &CoreConfigContext,
    transport_host: &str,
) -> XrayTlsSettings {
    let mut tls_settings = XrayTlsSettings {
        allow_insecure: Some(allow_insecure(node, context)),
        alpn: split_list(&node.alpn).filter(|values| !values.is_empty()),
        fingerprint: Some(effective_fingerprint(node, context)),
        ech_config_list: nonempty_string(&node.ech_config_list),
        ..XrayTlsSettings::default()
    };

    let sni = trimmed(&node.sni);
    if !sni.is_empty() {
        tls_settings.server_name = Some(sni.to_string());
    } else if let Some(first_host) =
        split_list(transport_host).and_then(|hosts| hosts.into_iter().next())
    {
        tls_settings.server_name = Some(first_host);
    }

    if tls_settings.ech_config_list.is_some() {
        tls_settings.ech_force_query = Some("full".to_string());
    }

    let certs = parse_pem_chain(&node.cert);
    if !certs.is_empty() {
        tls_settings.certificates = Some(
            certs
                .into_iter()
                .map(|cert| XrayCertificateSettings {
                    certificate: Some(cert.split('\n').map(str::to_string).collect()),
                    usage: Some("verify".to_string()),
                })
                .collect(),
        );
        tls_settings.disable_system_root = Some(true);
        tls_settings.allow_insecure = Some(false);
    } else if !trimmed(&node.cert_sha).is_empty() {
        tls_settings.pinned_peer_cert_sha256 = Some(node.cert_sha.clone());
        tls_settings.allow_insecure = Some(false);
    }

    tls_settings
}

fn reality_settings(node: &ProfileItem, context: &CoreConfigContext) -> XrayTlsSettings {
    XrayTlsSettings {
        fingerprint: Some(effective_fingerprint(node, context)),
        server_name: Some(trimmed(&node.sni).to_string()),
        public_key: Some(node.public_key.clone()),
        short_id: Some(node.short_id.clone()),
        spider_x: Some(node.spider_x.clone()),
        mldsa65_verify: Some(node.mldsa65_verify.clone()),
        show: Some(false),
        ..XrayTlsSettings::default()
    }
}

fn wireguard_settings(
    node: &ProfileItem,
    protocol_extra: &ProtocolExtraItem,
) -> XrayOutboundSettings {
    let endpoint_address = if node
        .address
        .parse::<IpAddr>()
        .is_ok_and(|addr| addr.is_ipv6())
    {
        format!("[{}]", node.address)
    } else {
        node.address.clone()
    };
    XrayOutboundSettings {
        address: Some(json!(split_list(
            protocol_extra
                .wg_interface_address
                .as_deref()
                .unwrap_or_default()
        )
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![WIREGUARD_DEFAULT_ADDRESS.to_string()]))),
        secret_key: Some(node.password.clone()),
        reserved: split_list(protocol_extra.wg_reserved.as_deref().unwrap_or_default()).map(
            |items| {
                items
                    .into_iter()
                    .filter_map(|item| item.trim().parse::<i32>().ok())
                    .collect()
            },
        ),
        mtu: Some(
            protocol_extra
                .wg_mtu
                .filter(|mtu| *mtu > 0)
                .unwrap_or(WIREGUARD_DEFAULT_MTU),
        ),
        peers: Some(vec![XrayWireguardPeer {
            public_key: protocol_extra.wg_public_key.clone().unwrap_or_default(),
            endpoint: format!("{endpoint_address}:{}", node.port),
            pre_shared_key: protocol_extra.wg_preshared_key.clone(),
        }]),
        ..XrayOutboundSettings::default()
    }
}

fn build_outbounds_list(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<XrayOutbound> {
    let nodes = child_nodes(context, node);
    let mut result_outbounds: Vec<XrayOutbound> = Vec::new();
    for (index, child_node) in nodes.iter().enumerate() {
        let mut current_tag = format!("{base_tag_name}-{}-{}", index + 1, child_node.remarks);
        if nodes.len() == 1 {
            current_tag = base_tag_name.to_string();
        }

        if child_node.config_type.is_group_type() {
            result_outbounds.extend(build_group_proxy_outbounds(
                context,
                child_node,
                &current_tag,
            ));
            continue;
        }

        let mut outbound = build_proxy_outbound(context, child_node, PROXY_TAG);
        outbound.tag = current_tag;
        result_outbounds.push(outbound);
    }
    result_outbounds
}

fn build_chain_outbounds_list(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<XrayOutbound> {
    let nodes = child_nodes(context, node);
    let nodes_reverse = nodes.into_iter().rev().collect::<Vec<_>>();
    let mut result_outbounds: Vec<XrayOutbound> = Vec::new();

    for (index, child_node) in nodes_reverse.iter().enumerate() {
        let current_tag = if index == 0 {
            base_tag_name.to_string()
        } else {
            format!("chain-{base_tag_name}-{index}-{}", child_node.remarks)
        };
        let dialer_proxy_tag = (index != nodes_reverse.len().saturating_sub(1)).then(|| {
            format!(
                "chain-{base_tag_name}-{}-{}",
                index + 1,
                nodes_reverse[index + 1].remarks
            )
        });

        if child_node.config_type.is_group_type() {
            let mut child_profiles = build_group_proxy_outbounds(context, child_node, &current_tag);
            if let Some(dialer_proxy_tag) = dialer_proxy_tag.as_deref() {
                for outbound in child_profiles.iter_mut().filter(|outbound| {
                    outbound
                        .stream_settings
                        .as_ref()
                        .and_then(|settings| settings.sockopt.as_ref())
                        .and_then(|sockopt| sockopt.dialer_proxy.as_deref())
                        .is_none_or(str::is_empty)
                }) {
                    fill_dialer_proxy(outbound, dialer_proxy_tag);
                }
            }

            if index != 0 {
                let chain_start_nodes = child_profiles
                    .iter()
                    .filter(|outbound| outbound.tag.starts_with(&current_tag))
                    .cloned()
                    .collect::<Vec<_>>();
                if chain_start_nodes.len() == 1 {
                    let first_chain_tag = chain_start_nodes[0].tag.clone();
                    for outbound in result_outbounds.iter_mut().filter(|outbound| {
                        outbound
                            .stream_settings
                            .as_ref()
                            .and_then(|settings| settings.sockopt.as_ref())
                            .and_then(|sockopt| sockopt.dialer_proxy.as_deref())
                            == Some(current_tag.as_str())
                    }) {
                        fill_dialer_proxy(outbound, &first_chain_tag);
                    }
                } else if chain_start_nodes.len() > 1 {
                    let existed_chain_nodes = result_outbounds.clone();
                    result_outbounds.clear();
                    for (branch_index, chain_start_node) in chain_start_nodes.iter().enumerate() {
                        let mut existed_chain_nodes_clone = existed_chain_nodes.clone();
                        for existed_chain_node in &mut existed_chain_nodes_clone {
                            existed_chain_node.tag =
                                format!("{}-clone-{}", existed_chain_node.tag, branch_index + 1);
                        }
                        for chain_index in 0..existed_chain_nodes_clone.len() {
                            let previous_dialer_proxy_tag = existed_chain_nodes_clone[chain_index]
                                .stream_settings
                                .as_ref()
                                .and_then(|settings| settings.sockopt.as_ref())
                                .and_then(|sockopt| sockopt.dialer_proxy.clone());
                            let next_tag = if chain_index + 1 < existed_chain_nodes_clone.len() {
                                existed_chain_nodes_clone[chain_index + 1].tag.clone()
                            } else {
                                chain_start_node.tag.clone()
                            };
                            let next_dialer = if previous_dialer_proxy_tag.as_deref()
                                == Some(current_tag.as_str())
                            {
                                chain_start_node.tag.as_str()
                            } else {
                                next_tag.as_str()
                            };
                            fill_dialer_proxy(
                                &mut existed_chain_nodes_clone[chain_index],
                                next_dialer,
                            );
                            result_outbounds.push(existed_chain_nodes_clone[chain_index].clone());
                        }
                    }
                }
            }

            result_outbounds.extend(child_profiles);
            continue;
        }

        let mut outbound = build_proxy_outbound(context, child_node, PROXY_TAG);
        outbound.tag = current_tag;
        if let Some(dialer_proxy_tag) = dialer_proxy_tag {
            fill_dialer_proxy(&mut outbound, &dialer_proxy_tag);
        }
        result_outbounds.push(outbound);
    }

    result_outbounds
}

fn fill_dialer_proxy(outbound: &mut XrayOutbound, dialer_proxy_tag: &str) {
    let stream_settings = outbound
        .stream_settings
        .get_or_insert_with(XrayStreamSettings::default);
    stream_settings
        .sockopt
        .get_or_insert_with(XraySockopt::default)
        .dialer_proxy = Some(dialer_proxy_tag.to_string());

    if let Some(extra) = stream_settings
        .xhttp_settings
        .as_mut()
        .and_then(|settings| settings.extra.as_mut())
        .and_then(Value::as_object_mut)
    {
        if let Some(download_settings) = extra
            .get_mut("downloadSettings")
            .and_then(Value::as_object_mut)
        {
            let sockopt = download_settings
                .entry("sockopt")
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(sockopt) = sockopt.as_object_mut() {
                sockopt.insert(
                    "dialerProxy".to_string(),
                    Value::String(dialer_proxy_tag.to_string()),
                );
            }
        }
    }
}

fn gen_observatory(
    config: &mut XrayConfig,
    multiple_load: MultipleLoad,
    base_tag_name: &str,
    probe_url: &str,
) {
    let mut subject_selectors = Vec::new();
    subject_selectors.extend(
        config
            .burst_observatory
            .as_ref()
            .and_then(|observatory| observatory.subject_selector.clone())
            .unwrap_or_default(),
    );
    subject_selectors.extend(
        config
            .observatory
            .as_ref()
            .and_then(|observatory| observatory.subject_selector.clone())
            .unwrap_or_default(),
    );

    if subject_selectors
        .iter()
        .any(|selector| base_tag_name.starts_with(selector))
    {
        return;
    }

    if let Some(matched) = subject_selectors
        .into_iter()
        .find(|selector| selector.starts_with(base_tag_name))
    {
        move_selector_to_front(
            config
                .burst_observatory
                .as_mut()
                .and_then(|observatory| observatory.subject_selector.as_mut()),
            &matched,
        );
        move_selector_to_front(
            config
                .observatory
                .as_mut()
                .and_then(|observatory| observatory.subject_selector.as_mut()),
            &matched,
        );
        return;
    }

    match multiple_load {
        MultipleLoad::LeastLoad | MultipleLoad::Fallback => {
            if let Some(observatory) = config.burst_observatory.as_mut() {
                observatory
                    .subject_selector
                    .get_or_insert_with(Vec::new)
                    .push(base_tag_name.to_string());
            } else {
                config.burst_observatory = Some(XrayBurstObservatory {
                    subject_selector: Some(vec![base_tag_name.to_string()]),
                    ping_config: Some(XrayBurstPingConfig {
                        destination: Some(probe_url.to_string()),
                        connectivity: None,
                        interval: Some("5m".to_string()),
                        sampling: Some(2),
                        timeout: Some("30s".to_string()),
                    }),
                });
            }
        }
        MultipleLoad::LeastPing => {
            if let Some(observatory) = config.observatory.as_mut() {
                observatory
                    .subject_selector
                    .get_or_insert_with(Vec::new)
                    .push(base_tag_name.to_string());
            } else {
                config.observatory = Some(XrayObservatory {
                    subject_selector: Some(vec![base_tag_name.to_string()]),
                    probe_url: Some(probe_url.to_string()),
                    probe_interval: Some("3m".to_string()),
                    enable_concurrency: Some(true),
                });
            }
        }
        MultipleLoad::Random | MultipleLoad::RoundRobin => {}
    }
}

fn gen_balancer(config: &mut XrayConfig, multiple_load: MultipleLoad, selector: &str) {
    let strategy_type = match multiple_load {
        MultipleLoad::Random => "random",
        MultipleLoad::RoundRobin => "roundRobin",
        MultipleLoad::LeastPing => "leastPing",
        MultipleLoad::LeastLoad => "leastLoad",
        MultipleLoad::Fallback => "roundRobin",
    };
    let balancer = XrayBalancer {
        selector: Some(vec![selector.to_string()]),
        strategy: Some(XrayBalancerStrategy {
            r#type: Some(strategy_type.to_string()),
            settings: Some(XrayBalancerStrategySettings {
                expected: Some(1),
                max_rtt: None,
                tolerance: None,
                baselines: None,
                costs: None,
            }),
        }),
        tag: Some(format!("{selector}{BALANCER_TAG_SUFFIX}")),
        fallback_tag: (multiple_load == MultipleLoad::Fallback).then(|| DIRECT_TAG.to_string()),
    };
    config
        .routing
        .balancers
        .get_or_insert_with(Vec::new)
        .push(balancer);
}

fn move_selector_to_front(selectors: Option<&mut Vec<String>>, target: &str) {
    let Some(selectors) = selectors else {
        return;
    };
    if let Some(index) = selectors.iter().position(|selector| selector == target) {
        let selector = selectors.remove(index);
        selectors.insert(0, selector);
    }
}

fn child_nodes(context: &CoreConfigContext, node: &ProfileItem) -> Vec<ProfileItem> {
    split_list(
        node.protocol_extra
            .child_items
            .as_deref()
            .unwrap_or_default(),
    )
    .unwrap_or_default()
    .into_iter()
    .filter_map(|node_id| context.all_proxies_map.get(&node_id).cloned())
    .collect()
}

fn merge_mask_array_if_empty(finalmask: &mut Map<String, Value>, key: &str, mask: Value) {
    let should_insert = finalmask
        .get(key)
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty);
    if should_insert {
        finalmask.insert(key.to_string(), Value::Array(vec![mask]));
    }
}

#[derive(Debug, Clone)]
struct TransportValues {
    host: String,
    path: String,
    kcp_seed: String,
    kcp_mtu: i32,
    header_type: String,
    xhttp_extra: String,
    user_agent: String,
}

fn transport_values(
    network: &str,
    transport: &TransportExtraItem,
    context: &CoreConfigContext,
) -> TransportValues {
    let mut values = TransportValues {
        host: String::new(),
        path: String::new(),
        kcp_seed: String::new(),
        kcp_mtu: context.app_config.kcp_item.mtu,
        header_type: String::new(),
        xhttp_extra: String::new(),
        user_agent: context.app_config.core_basic_item.def_user_agent.clone(),
    };

    match network {
        "raw" => {
            values.host = trimmed_opt(transport.host.as_deref());
            values.path = trimmed_opt(transport.path.as_deref());
            values.header_type = trimmed_opt(transport.raw_header_type.as_deref());
        }
        "kcp" => {
            values.kcp_seed = trimmed_opt(transport.kcp_seed.as_deref());
            values.header_type = trimmed_opt(transport.kcp_header_type.as_deref());
            values.kcp_mtu = transport
                .kcp_mtu
                .filter(|mtu| *mtu > 0)
                .unwrap_or(context.app_config.kcp_item.mtu);
        }
        "ws" | "httpupgrade" => {
            values.host = trimmed_opt(transport.host.as_deref());
            values.path = trimmed_opt(transport.path.as_deref());
        }
        "xhttp" => {
            values.host = trimmed_opt(transport.host.as_deref());
            values.path = trimmed_opt(transport.path.as_deref());
            values.header_type = trimmed_opt(transport.xhttp_mode.as_deref());
            values.xhttp_extra = trimmed_opt(transport.xhttp_extra.as_deref());
        }
        "grpc" => {
            values.host = trimmed_opt(transport.grpc_authority.as_deref());
            values.path = trimmed_opt(transport.grpc_service_name.as_deref());
            values.header_type = trimmed_opt(transport.grpc_mode.as_deref());
        }
        _ => {}
    }

    values
}

fn raw_http_request(host: &str, path: &str, user_agent: &str) -> Value {
    json!({
        "version": "1.1",
        "method": "GET",
        "path": split_csv_preserve_empty(path, DEFAULT_RAW_HTTP_PATH),
        "headers": {
            "Host": split_csv_preserve_empty(host, ""),
            "User-Agent": [raw_http_user_agent(user_agent)],
            "Accept-Encoding": ["gzip, deflate"],
            "Connection": ["keep-alive"],
            "Pragma": ["no-cache"],
        }
    })
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

fn split_csv_preserve_empty(value: &str, fallback: &str) -> Vec<String> {
    let value = if value.is_empty() { fallback } else { value };
    value.split(',').map(str::to_string).collect()
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
        ConfigType::WireGuard => "wireguard",
        ConfigType::TUIC => "tuic",
        ConfigType::Anytls => "anytls",
        ConfigType::Naive => "naive",
        ConfigType::Custom | ConfigType::PolicyGroup | ConfigType::ProxyChain => "vmess",
    }
}

fn xray_network(node: &ProfileItem) -> &str {
    let network = trimmed(&node.network);
    if network.is_empty() || !LIVE_XRAY_NETWORKS.contains(&network) {
        DEFAULT_NETWORK
    } else {
        network
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
    if SS_SECURITIES_IN_XRAY.contains(&method) {
        method.to_string()
    } else {
        "none".to_string()
    }
}

fn allow_insecure(node: &ProfileItem, context: &CoreConfigContext) -> bool {
    let value = if trimmed(&node.allow_insecure).is_empty() {
        context
            .app_config
            .core_basic_item
            .def_allow_insecure
            .to_string()
    } else {
        node.allow_insecure.clone()
    };
    value.eq_ignore_ascii_case("true")
}

fn effective_fingerprint(node: &ProfileItem, context: &CoreConfigContext) -> String {
    if trimmed(&node.fingerprint).is_empty() {
        context.app_config.core_basic_item.def_fingerprint.clone()
    } else {
        node.fingerprint.clone()
    }
}

fn kcp_header_mask(header_type: &str) -> Option<&'static str> {
    match header_type {
        "srtp" => Some("header-srtp"),
        "utp" => Some("header-utp"),
        "wechat-video" => Some("header-wechat"),
        "dtls" => Some("header-dtls"),
        "wireguard" => Some("header-wireguard"),
        "dns" => Some("header-dns"),
        _ => None,
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

fn parse_i32(value: Option<&str>) -> Option<i32> {
    value.and_then(|value| value.trim().parse::<i32>().ok())
}

fn nonempty_string(value: &str) -> Option<String> {
    nonempty_str(Some(value)).map(str::to_string)
}

fn nonempty_str(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn trimmed(value: &str) -> &str {
    value.trim()
}

fn trimmed_opt(value: Option<&str>) -> String {
    value.map(str::trim).unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::{
        golden, CoreGenPlatform, CoreType, FullConfigTemplateItem, KcpItem, ProtocolExtraItem,
        RoutingItem, RuleType, TransportExtraItem,
    };

    #[test]
    fn xray_outbound_vless_tls_xhttp_fragment_matches_golden() {
        let mut config = AppConfig::default();
        config.core_basic_item.enable_fragment = true;
        config.core_basic_item.mux_enabled = true;
        config.core_basic_item.def_fingerprint = "firefox".to_string();
        config.speed_test_item.speed_ping_test_url =
            "https://ping.example/generate_204".to_string();

        let node = ProfileItem {
            index_id: "n-vless".to_string(),
            config_type: ConfigType::VLESS,
            remarks: "vless-xhttp".to_string(),
            address: "server.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000001".to_string(),
            network: "xhttp".to_string(),
            stream_security: "tls".to_string(),
            sni: "tls.example".to_string(),
            alpn: "h2,http/1.1".to_string(),
            fingerprint: "chrome".to_string(),
            ech_config_list: "tls.example+https://ech.example/config".to_string(),
            mux_enabled: Some(true),
            protocol_extra: ProtocolExtraItem {
                vless_encryption: Some("none".to_string()),
                ..ProtocolExtraItem::default()
            },
            transport_extra: TransportExtraItem {
                host: Some("cdn.example".to_string()),
                path: Some("/xhttp".to_string()),
                xhttp_mode: Some("stream-up".to_string()),
                xhttp_extra: Some(
                    r#"{"downloadSettings":{"address":"download.example"}}"#.to_string(),
                ),
                ..TransportExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let context = test_context(config, node);

        let generated = generate_xray_config(&context);
        let first_outbound = generated.outbounds.first().expect("proxy outbound");
        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/xray/outbounds/vless_tls_xhttp_fragment.json"
        ))
        .expect("xray vless xhttp golden fixture should parse as JSON");

        golden::assert_json_eq(
            "xray-outbound-vless-tls-xhttp-fragment",
            &expected,
            &serde_json::to_value(first_outbound)
                .expect("xray first outbound should serialize to JSON"),
        );
    }

    #[test]
    fn xray_outbound_proxy_chain_rewrites_xhttp_download_dialer_proxy() {
        let xhttp_node = ProfileItem {
            index_id: "n-xhttp".to_string(),
            config_type: ConfigType::VLESS,
            remarks: "xhttp-hop".to_string(),
            address: "xhttp.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000002".to_string(),
            network: "xhttp".to_string(),
            stream_security: "tls".to_string(),
            sni: "xhttp.example".to_string(),
            protocol_extra: ProtocolExtraItem {
                vless_encryption: Some("none".to_string()),
                ..ProtocolExtraItem::default()
            },
            transport_extra: TransportExtraItem {
                host: Some("xhttp.example".to_string()),
                xhttp_extra: Some(
                    r#"{"downloadSettings":{"address":"assets.example"}}"#.to_string(),
                ),
                ..TransportExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let raw_node = socks_node("n-raw", "raw-hop");
        let chain = ProfileItem {
            index_id: "chain".to_string(),
            config_type: ConfigType::ProxyChain,
            remarks: "chain".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some("n-raw,n-xhttp".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let mut context = test_context(AppConfig::default(), chain);
        context
            .all_proxies_map
            .insert(raw_node.index_id.clone(), raw_node);
        context
            .all_proxies_map
            .insert(xhttp_node.index_id.clone(), xhttp_node);

        let generated = generate_xray_config(&context);
        let proxy = generated
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == PROXY_TAG)
            .expect("chain entrypoint");
        let stream = proxy.stream_settings.as_ref().expect("stream settings");

        assert_eq!(
            stream
                .sockopt
                .as_ref()
                .and_then(|sockopt| sockopt.dialer_proxy.as_deref()),
            Some("chain-proxy-1-raw-hop")
        );
        assert_eq!(
            stream
                .xhttp_settings
                .as_ref()
                .and_then(|settings| settings.extra.as_ref())
                .and_then(|extra| extra.pointer("/downloadSettings/sockopt/dialerProxy"))
                .and_then(Value::as_str),
            Some("chain-proxy-1-raw-hop")
        );
    }

    #[test]
    fn xray_outbound_node_finalmask_overrides_transport_then_fragment_fills_empty_arrays() {
        let mut config = AppConfig::default();
        config.core_basic_item.enable_fragment = true;
        config.kcp_item = KcpItem {
            mtu: 1408,
            ..KcpItem::default()
        };
        let node = ProfileItem {
            index_id: "n-kcp".to_string(),
            config_type: ConfigType::VMess,
            remarks: "kcp".to_string(),
            address: "kcp.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000003".to_string(),
            network: "kcp".to_string(),
            stream_security: "tls".to_string(),
            finalmask: r#"{"udp":[{"type":"custom-udp"}],"tcp":[]}"#.to_string(),
            protocol_extra: ProtocolExtraItem {
                alter_id: Some("0".to_string()),
                vmess_security: Some(DEFAULT_SECURITY.to_string()),
                ..ProtocolExtraItem::default()
            },
            transport_extra: TransportExtraItem {
                kcp_header_type: Some("srtp".to_string()),
                kcp_seed: Some("seed".to_string()),
                ..TransportExtraItem::default()
            },
            ..ProfileItem::default()
        };

        let generated = generate_xray_config(&test_context(config, node));
        let finalmask = generated.outbounds[0]
            .stream_settings
            .as_ref()
            .and_then(|stream| stream.finalmask.as_ref())
            .expect("finalmask");

        assert_eq!(
            finalmask.pointer("/udp/0/type").and_then(Value::as_str),
            Some("custom-udp")
        );
        assert_eq!(
            finalmask.pointer("/tcp/0/type").and_then(Value::as_str),
            Some("fragment")
        );
    }

    #[test]
    fn policy_group_xray_balancer_burst_observatory_matches_golden() {
        let mut config = AppConfig::default();
        config.speed_test_item.speed_ping_test_url =
            "https://ping.example/generate_204".to_string();
        let group = ProfileItem {
            index_id: "group".to_string(),
            config_type: ConfigType::PolicyGroup,
            remarks: "least-load".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some("n1,n2".to_string()),
                multiple_load: Some(MultipleLoad::LeastLoad),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let mut context = test_context(config, group);
        context
            .all_proxies_map
            .insert("n1".to_string(), socks_node("n1", "node-1"));
        context
            .all_proxies_map
            .insert("n2".to_string(), socks_node("n2", "node-2"));

        let generated = generate_xray_config(&context);
        let summary = json!({
            "balancers": generated.routing.balancers,
            "burstObservatory": generated.burst_observatory,
            "observatory": generated.observatory,
        });
        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/xray/outbounds/policy_group_least_load.json"
        ))
        .expect("xray policy group golden fixture should parse as JSON");

        golden::assert_json_eq("xray-policy-group-least-load", &expected, &summary);
    }

    #[test]
    fn xray_inbounds_stats_and_tun_inbound_match_golden() {
        let mut config = AppConfig::default();
        config.inbound[0].local_port = 12000;
        config.inbound[0].second_local_port_enabled = true;
        config.inbound[0].allow_lan_conn = true;
        config.inbound[0].new_port4_lan = true;
        config.inbound[0].user = "lan-user".to_string();
        config.inbound[0].pass = "lan-pass".to_string();
        config.gui_item.enable_statistics = true;
        config.tun_mode_item.mtu = 1408;
        config.tun_mode_item.enable_ipv6_address = false;
        config.core_basic_item.bind_interface = Some("eth0".to_string());
        config.simple_dns_item.add_common_hosts = Some(false);

        let node = ProfileItem {
            index_id: "n-vmess".to_string(),
            config_type: ConfigType::VMess,
            remarks: "vmess".to_string(),
            address: "remote.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000004".to_string(),
            protocol_extra: ProtocolExtraItem {
                vmess_security: Some(DEFAULT_SECURITY.to_string()),
                alter_id: Some("0".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let mut context = test_context(config, node);
        context.is_tun_enabled = true;

        let generated = generate_xray_config(&context);
        let summary = json!({
            "inbounds": generated.inbounds,
            "metrics": generated.metrics,
            "policy": generated.policy,
            "stats": generated.stats,
            "tunDnsOutbound": generated.outbounds.iter().any(|outbound| outbound.tag == DNS_OUTBOUND_TAG && outbound.protocol == "dns"),
        });
        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/xray/full/inbounds_stats_tun.json"
        ))
        .expect("xray inbounds stats golden fixture should parse as JSON");

        golden::assert_json_eq("xray-inbounds-stats-tun", &expected, &summary);
    }

    #[test]
    fn xray_advanced_dns_and_routing_match_golden() {
        let mut config = AppConfig::default();
        config.simple_dns_item.add_common_hosts = Some(false);
        config.simple_dns_item.direct_dns =
            Some("119.29.29.29,https://dns.alidns.com/dns-query".to_string());
        config.simple_dns_item.remote_dns =
            Some("https://cloudflare-dns.com/dns-query".to_string());
        config.simple_dns_item.bootstrap_dns = Some("223.5.5.5".to_string());
        config.simple_dns_item.strategy4_freedom = Some("UseIP".to_string());
        config.simple_dns_item.strategy4_proxy = Some("UseIPv4".to_string());
        config.simple_dns_item.serve_stale = Some(true);
        config.simple_dns_item.parallel_query = Some(true);
        config.simple_dns_item.fake_ip = Some(true);
        config.simple_dns_item.hosts = Some("example.test 1.2.3.4 5.6.7.8\n# ignored".to_string());
        config.simple_dns_item.direct_expected_ips = Some("geoip:cn,1.1.1.1".to_string());

        let node = ProfileItem {
            index_id: "n-vless".to_string(),
            config_type: ConfigType::VLESS,
            remarks: "main".to_string(),
            address: "main.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000005".to_string(),
            protocol_extra: ProtocolExtraItem {
                vless_encryption: Some("none".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let mut context = test_context(config, node);
        context.protect_domain_list = vec!["full:ech.example".to_string()];
        context.routing_item = Some(RoutingItem {
            id: "routing".to_string(),
            domain_strategy: IP_IF_NON_MATCH.to_string(),
            rule_set: vec![
                RulesItem {
                    id: "direct-domains".to_string(),
                    outbound_tag: Some(DIRECT_TAG.to_string()),
                    domain: Some(vec![
                        "geosite:cn".to_string(),
                        "domain:direct.example".to_string(),
                    ]),
                    rule_type: Some(RuleType::DNS),
                    ..RulesItem::default()
                },
                RulesItem {
                    id: "proxy-domains".to_string(),
                    outbound_tag: Some(PROXY_TAG.to_string()),
                    domain: Some(vec![
                        "geosite:google".to_string(),
                        "domain:proxy.example".to_string(),
                    ]),
                    ..RulesItem::default()
                },
                RulesItem {
                    id: "detour".to_string(),
                    outbound_tag: Some("detour".to_string()),
                    domain: Some(vec!["full:special<COMMA>domain".to_string()]),
                    ..RulesItem::default()
                },
                RulesItem {
                    id: "final-direct".to_string(),
                    outbound_tag: Some(DIRECT_TAG.to_string()),
                    ip: Some(vec!["0.0.0.0/0".to_string()]),
                    port: Some("0-65535".to_string()),
                    network: Some("tcp,udp".to_string()),
                    ..RulesItem::default()
                },
            ],
            ..RoutingItem::default()
        });
        context.all_proxies_map.insert(
            "remark:detour".to_string(),
            socks_node("detour-id", "detour-node"),
        );

        let generated = generate_xray_config(&context);
        let summary = json!({
            "dns": generated.dns,
            "fakedns": generated.fake_dns,
            "routing": generated.routing,
            "outbounds": generated.outbounds.iter().map(|outbound| {
                json!({
                    "tag": outbound.tag,
                    "protocol": outbound.protocol,
                    "targetStrategy": outbound.target_strategy,
                    "settings": outbound.settings,
                })
            }).collect::<Vec<_>>(),
        });
        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/xray/full/advanced_dns_routing.json"
        ))
        .expect("xray advanced dns golden fixture should parse as JSON");

        golden::assert_json_eq("xray-advanced-dns-routing", &expected, &summary);
    }

    #[test]
    fn xray_dns_raw_override_uses_custom_json_and_freedom_strategy() {
        let mut context = test_context(AppConfig::default(), socks_node("n-socks", "socks"));
        context.raw_dns_item = Some(DnsItem {
            enabled: true,
            core_type: CoreType::Xray,
            normal_dns: Some(r#"{"servers":["https://dns.example/dns-query"]}"#.to_string()),
            domain_strategy4_freedom: Some("UseIPv4".to_string()),
            ..DnsItem::default()
        });
        context.routing_item = Some(RoutingItem {
            domain_strategy: IP_IF_NON_MATCH.to_string(),
            ..RoutingItem::default()
        });

        let generated = generate_xray_config(&context);
        let dns = generated.dns.expect("raw dns");

        assert_eq!(
            dns.pointer("/servers/0").and_then(Value::as_str),
            Some("https://dns.example/dns-query")
        );
        assert!(generated.routing.rules.iter().any(|rule| {
            rule.inbound_tag.as_ref() == Some(&vec![DNS_TAG.to_string()])
                && rule.outbound_tag.as_deref() == Some(PROXY_TAG)
        }));
        assert_eq!(
            generated
                .outbounds
                .iter()
                .find(|outbound| outbound.tag == DIRECT_TAG)
                .and_then(|outbound| outbound.settings.as_ref())
                .and_then(|settings| settings.domain_strategy.as_deref()),
            Some("UseIPv4")
        );
    }

    #[test]
    fn xray_full_config_template_tun_proxy_detour_matches_golden() {
        let mut config = AppConfig::default();
        config.speed_test_item.speed_ping_test_url =
            "https://ping.example/generate_204".to_string();
        let group = ProfileItem {
            index_id: "group".to_string(),
            config_type: ConfigType::PolicyGroup,
            remarks: "template-group".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some("n1,n2".to_string()),
                multiple_load: Some(MultipleLoad::LeastPing),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let mut context = test_context(config, group);
        context.is_tun_enabled = true;
        context.full_config_template = Some(FullConfigTemplateItem {
            enabled: true,
            core_type: CoreType::Xray,
            add_proxy_only: Some(false),
            proxy_detour: Some("template-detour".to_string()),
            config: Some(r#"{"remarks":"unused"}"#.to_string()),
            tun_config: Some(
                r#"{
                    "remarks": "tun-template",
                    "routing": {
                        "rules": [
                            { "type": "field", "outboundTag": "proxy", "domain": ["geosite:private"] }
                        ]
                    },
                    "observatory": { "subjectSelector": ["template-observer"] },
                    "outbounds": [
                        { "tag": "template-detour", "protocol": "freedom" }
                    ]
                }"#
                .to_string(),
            ),
            ..FullConfigTemplateItem::default()
        });
        context.all_proxies_map.insert(
            "n1".to_string(),
            ProfileItem {
                index_id: "n1".to_string(),
                config_type: ConfigType::VMess,
                remarks: "remote-1".to_string(),
                address: "one.example".to_string(),
                port: 443,
                password: "00000000-0000-0000-0000-000000000006".to_string(),
                protocol_extra: ProtocolExtraItem {
                    alter_id: Some("0".to_string()),
                    vmess_security: Some(DEFAULT_SECURITY.to_string()),
                    ..ProtocolExtraItem::default()
                },
                ..ProfileItem::default()
            },
        );
        context.all_proxies_map.insert(
            "n2".to_string(),
            ProfileItem {
                index_id: "n2".to_string(),
                config_type: ConfigType::VMess,
                remarks: "remote-2".to_string(),
                address: "two.example".to_string(),
                port: 443,
                password: "00000000-0000-0000-0000-000000000007".to_string(),
                protocol_extra: ProtocolExtraItem {
                    alter_id: Some("0".to_string()),
                    vmess_security: Some(DEFAULT_SECURITY.to_string()),
                    ..ProtocolExtraItem::default()
                },
                ..ProfileItem::default()
            },
        );

        let generated = generate_xray_config_value(&context);
        let summary = json!({
            "remarks": generated.get("remarks"),
            "routing": generated.get("routing"),
            "observatory": generated.get("observatory"),
            "outbounds": generated.get("outbounds"),
        });
        let expected: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/xray/full/template_tun_proxy_detour.json"
        ))
        .expect("xray template tun golden fixture should parse as JSON");

        golden::assert_json_eq("xray-template-tun-proxy-detour", &expected, &summary);
    }

    #[test]
    fn xray_outbound_serde_skips_null_but_keeps_mux_disabled_state() {
        let outbound = XrayOutbound::proxy_sample(PROXY_TAG);
        let value = serde_json::to_value(outbound).expect("xray outbound should serialize to JSON");

        assert!(value.get("sendThrough").is_none());
        assert!(value.get("targetStrategy").is_none());
        assert_eq!(value.pointer("/mux/enabled"), Some(&Value::Bool(false)));
        assert!(value.pointer("/mux/concurrency").is_none());
    }

    fn test_context(app_config: AppConfig, node: ProfileItem) -> CoreConfigContext {
        let mut all_proxies_map = BTreeMap::new();
        all_proxies_map.insert(node.index_id.clone(), node.clone());
        let simple_dns_item = app_config.simple_dns_item.clone();
        CoreConfigContext {
            node,
            run_core_type: CoreType::Xray,
            app_config,
            simple_dns_item,
            all_proxies_map,
            platform: CoreGenPlatform::Linux,
            ..CoreConfigContext::default()
        }
    }

    fn socks_node(index_id: &str, remarks: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::SOCKS,
            remarks: remarks.to_string(),
            address: "127.0.0.1".to_string(),
            port: 1080,
            username: "user".to_string(),
            password: "pass".to_string(),
            network: DEFAULT_NETWORK.to_string(),
            mux_enabled: Some(false),
            protocol_extra: ProtocolExtraItem::default(),
            transport_extra: TransportExtraItem::default(),
            ..ProfileItem::default()
        }
    }
}
