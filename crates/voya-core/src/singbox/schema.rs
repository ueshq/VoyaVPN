use super::*;

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
    pub independent_cache: Option<bool>,
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
    pub ip_accept_any: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_port: Option<i32>,
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
    pub platform: Option<SingboxTunPlatform>,
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
            platform: None,
            users: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxTunPlatform {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_proxy: Option<SingboxTunHttpProxy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxTunHttpProxy {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_port: Option<i32>,
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
    pub fragment: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_fallback_delay: Option<String>,
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
            fragment: None,
            fragment_fallback_delay: None,
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
    /// Bearer token required by the Clash API server.
    ///
    /// sing-box only installs its auth middleware when this is non-empty, and
    /// it answers CORS preflights with `Access-Control-Allow-Origin: *`, so an
    /// external controller without a secret is readable and writable by any
    /// local process and by browser pages that do not enforce Private Network
    /// Access.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
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
