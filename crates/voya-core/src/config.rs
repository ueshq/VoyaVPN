use crate::{SysProxyType, TlsFragmentMode, TrafficMode};

pub const DEFAULT_LOCAL_PORT: i32 = 10808;
pub const DEFAULT_LOG_LEVEL: &str = "warn";
pub const DEFAULT_FRAGMENT_FALLBACK_DELAY_MS: i32 = 500;
pub const DEFAULT_TUN_ICMP_ROUTING: &str = "rule";
pub const DEFAULT_LANGUAGE: &str = "en";
pub const DEFAULT_SPEED_PING_TEST_URL: &str = "https://www.google.com/generate_204";
pub const DEFAULT_SINGBOX_MUX: &str = "h2mux";
pub const DEFAULT_SYSTEM_PROXY_EXCEPTIONS: &str = "localhost,127.0.0.0/8,::1";
pub const DEFAULT_DIRECT_DNS: &str = "119.29.29.29";
pub const DEFAULT_REMOTE_DNS: &str = "https://cloudflare-dns.com/dns-query";
pub const DEFAULT_BOOTSTRAP_DNS: &str = "119.29.29.29";

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub index_id: String,
    /// The active policy group; empty when a node, or nothing, is active.
    pub active_group_id: String,
    /// The routing profile generation uses; empty for the default one.
    pub active_routing_id: String,
    pub core: CoreConfig,
    pub tun: TunConfig,
    pub behavior: BehaviorConfig,
    pub appearance: AppearanceConfig,
    pub speed_test: SpeedtestConfig,
    pub multiplexing: MultiplexingConfig,
    pub hysteria: HysteriaConfig,
    pub proxy: ProxyConfig,
    pub system_proxy: SystemProxyConfig,
    pub inbounds: Vec<InboundConfig>,
    pub dns: DnsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            index_id: String::new(),
            active_group_id: String::new(),
            active_routing_id: String::new(),
            core: CoreConfig::default(),
            tun: TunConfig::default(),
            behavior: BehaviorConfig::default(),
            appearance: AppearanceConfig::default(),
            speed_test: SpeedtestConfig::default(),
            multiplexing: MultiplexingConfig::default(),
            hysteria: HysteriaConfig::default(),
            proxy: ProxyConfig::default(),
            system_proxy: SystemProxyConfig::default(),
            inbounds: vec![InboundConfig::default()],
            dns: DnsConfig::default(),
        }
    }
}

/// What connecting uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTarget<'config> {
    None,
    Node(&'config str),
    Group(&'config str),
}

impl AppConfig {
    /// The first inbound's local port, or [`DEFAULT_LOCAL_PORT`] when there is
    /// no inbound.
    #[must_use]
    pub fn local_port(&self) -> i32 {
        self.inbounds
            .first()
            .map_or(DEFAULT_LOCAL_PORT, |inbound| inbound.local_port)
    }

    #[must_use]
    pub fn active_target(&self) -> ActiveTarget<'_> {
        if !self.active_group_id.is_empty() {
            ActiveTarget::Group(&self.active_group_id)
        } else if !self.index_id.is_empty() {
            ActiveTarget::Node(&self.index_id)
        } else {
            ActiveTarget::None
        }
    }

    /// Makes a node the active target. A node and a group are never active
    /// together, which the `app_state` table enforces as well.
    pub fn set_active_node(&mut self, index_id: impl Into<String>) {
        self.index_id = index_id.into();
        self.active_group_id.clear();
    }

    /// Makes a policy group the active target, clearing the active node.
    pub fn set_active_group(&mut self, group_id: impl Into<String>) {
        self.active_group_id = group_id.into();
        self.index_id.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreConfig {
    pub log_enabled: bool,
    pub log_level: String,
    pub mux_enabled: bool,
    pub default_allow_insecure: bool,
    pub default_fingerprint: String,
    pub default_user_agent: String,
    pub send_through: Option<String>,
    pub bind_interface: Option<String>,
    pub tls_fragment: TlsFragmentMode,
    pub fragment_fallback_delay_ms: i32,
    pub cache_file_enabled: bool,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            log_enabled: false,
            log_level: DEFAULT_LOG_LEVEL.to_string(),
            mux_enabled: false,
            default_allow_insecure: false,
            default_fingerprint: String::new(),
            default_user_agent: String::new(),
            send_through: None,
            bind_interface: None,
            tls_fragment: TlsFragmentMode::Off,
            fragment_fallback_delay_ms: DEFAULT_FRAGMENT_FALLBACK_DELAY_MS,
            cache_file_enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundConfig {
    pub local_port: i32,
    pub sniffing_enabled: bool,
    pub lan_connections_allowed: bool,
    pub separate_lan_port: bool,
    pub username: String,
    pub password: String,
    pub secondary_port_enabled: bool,
}

impl Default for InboundConfig {
    fn default() -> Self {
        Self {
            local_port: DEFAULT_LOCAL_PORT,
            sniffing_enabled: true,
            lan_connections_allowed: false,
            separate_lan_port: false,
            username: String::new(),
            password: String::new(),
            secondary_port_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorConfig {
    pub autostart: bool,
    pub auto_check_ip: bool,
    pub close_action: crate::CloseAction,
    pub start_minimized: bool,
    /// Offer a lowest-latency policy group the first time a subscription
    /// imports nodes.
    pub auto_create_subscription_group: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            autostart: false,
            auto_check_ip: false,
            close_action: crate::CloseAction::default(),
            start_minimized: false,
            auto_create_subscription_group: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceConfig {
    pub theme: Option<String>,
    pub language: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: None,
            language: DEFAULT_LANGUAGE.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedtestConfig {
    pub timeout_seconds: i32,
    pub latency_url: String,
    pub ip_lookup_url: String,
    pub page_size: Option<i32>,
    pub delay_interval_seconds: Option<i32>,
}

impl Default for SpeedtestConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 10,
            latency_url: DEFAULT_SPEED_PING_TEST_URL.to_string(),
            ip_lookup_url: String::new(),
            page_size: None,
            delay_interval_seconds: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiplexingConfig {
    pub protocol: String,
    pub max_connections: i32,
    pub padding: Option<bool>,
}

impl Default for MultiplexingConfig {
    fn default() -> Self {
        Self {
            protocol: DEFAULT_SINGBOX_MUX.to_string(),
            max_connections: 8,
            padding: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HysteriaConfig {
    pub upload_mbps: i32,
    pub download_mbps: i32,
    pub hop_interval_seconds: i32,
}

impl Default for HysteriaConfig {
    fn default() -> Self {
        Self {
            upload_mbps: 100,
            download_mbps: 100,
            hop_interval_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyConfig {
    pub traffic_mode: TrafficMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProxyConfig {
    pub mode: SysProxyType,
    pub exceptions: String,
    pub bypass_local: bool,
}

impl Default for SystemProxyConfig {
    fn default() -> Self {
        Self {
            mode: SysProxyType::ForcedChange,
            exceptions: DEFAULT_SYSTEM_PROXY_EXCEPTIONS.to_string(),
            bypass_local: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunConfig {
    pub enabled: bool,
    pub auto_route: bool,
    pub strict_route: bool,
    pub stack: String,
    pub mtu: i32,
    /// Whether IPv6 traffic is allowed (default on). Off forces DNS to
    /// `ipv4_only` and refuses IPv6 destinations that no direct rule claims;
    /// see [`crate::Ipv6Mode`], which also narrows "on" for a node without
    /// IPv6 egress.
    pub ipv6_enabled: bool,
    pub icmp_routing: String,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_route: true,
            strict_route: false,
            stack: String::new(),
            mtu: 1500,
            ipv6_enabled: true,
            icmp_routing: DEFAULT_TUN_ICMP_ROUTING.to_string(),
        }
    }
}

/// Which address families a DNS answer carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsStrategy {
    PreferIpv4,
    PreferIpv6,
    Ipv4Only,
    Ipv6Only,
}

impl DnsStrategy {
    /// The value sing-box's `strategy` field takes.
    #[must_use]
    pub const fn singbox_name(self) -> &'static str {
        match self {
            Self::PreferIpv4 => "prefer_ipv4",
            Self::PreferIpv6 => "prefer_ipv6",
            Self::Ipv4Only => "ipv4_only",
            Self::Ipv6Only => "ipv6_only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsConfig {
    pub add_common_hosts: Option<bool>,
    pub fake_ip: Option<bool>,
    pub global_fake_ip: Option<bool>,
    pub block_binding_query: Option<bool>,
    pub direct: Option<String>,
    pub remote: Option<String>,
    pub bootstrap: Option<String>,
    pub direct_strategy: Option<DnsStrategy>,
    pub proxy_strategy: Option<DnsStrategy>,
    pub hosts: Option<String>,
    pub direct_expected_ips: Option<String>,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            add_common_hosts: Some(true),
            fake_ip: Some(false),
            global_fake_ip: Some(true),
            block_binding_query: Some(true),
            direct: Some(DEFAULT_DIRECT_DNS.to_string()),
            remote: Some(DEFAULT_REMOTE_DNS.to_string()),
            bootstrap: Some(DEFAULT_BOOTSTRAP_DNS.to_string()),
            direct_strategy: None,
            proxy_strategy: None,
            hosts: None,
            direct_expected_ips: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_defaults_match_foundation_source() {
        let config = AppConfig::default();

        assert_eq!(config.inbounds.len(), 1);
        assert_eq!(config.inbounds[0].local_port, 10808);
        assert!(config.inbounds[0].sniffing_enabled);
        assert_eq!(config.core.log_level, "warn");
        assert_eq!(config.tun.mtu, 1500);
        assert!(!config.tun.strict_route);
        assert_eq!(config.speed_test.timeout_seconds, 10);
        assert_eq!(config.multiplexing.protocol, "h2mux");
        assert_eq!(config.hysteria.upload_mbps, 100);
        assert_eq!(config.hysteria.download_mbps, 100);
        assert_eq!(
            config.system_proxy.exceptions,
            DEFAULT_SYSTEM_PROXY_EXCEPTIONS
        );
        assert_eq!(config.dns.direct.as_deref(), Some(DEFAULT_DIRECT_DNS));
        assert_eq!(config.dns.remote.as_deref(), Some(DEFAULT_REMOTE_DNS));
    }
}
