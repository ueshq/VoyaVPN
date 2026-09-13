use crate::{SysProxyType, TlsFragmentMode, TrafficMode};

pub const DEFAULT_LOCAL_PORT: i32 = 10808;
pub const DEFAULT_LOG_LEVEL: &str = "warn";
pub const DEFAULT_FRAGMENT_FALLBACK_DELAY_MS: i32 = 500;
pub const DEFAULT_DOMAIN_STRATEGY: &str = "AsIs";
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
    pub core_basic_item: CoreBasicItem,
    pub tun_mode_item: TunModeItem,
    pub routing_basic_item: RoutingBasicItem,
    pub gui_item: GuiItem,
    pub ui_item: UiItem,
    pub speed_test_item: SpeedTestItem,
    pub mux4_sbox_item: Mux4SboxItem,
    pub hysteria_item: HysteriaItem,
    pub proxy_ui_item: ProxyUiItem,
    pub system_proxy_item: SystemProxyItem,
    pub inbound: Vec<InItem>,
    pub simple_dns_item: SimpleDnsItem,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            index_id: String::new(),
            core_basic_item: CoreBasicItem::default(),
            tun_mode_item: TunModeItem::default(),
            routing_basic_item: RoutingBasicItem::default(),
            gui_item: GuiItem::default(),
            ui_item: UiItem::default(),
            speed_test_item: SpeedTestItem::default(),
            mux4_sbox_item: Mux4SboxItem::default(),
            hysteria_item: HysteriaItem::default(),
            proxy_ui_item: ProxyUiItem::default(),
            system_proxy_item: SystemProxyItem::default(),
            inbound: vec![InItem::default()],
            simple_dns_item: SimpleDnsItem::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreBasicItem {
    pub log_enabled: bool,
    pub loglevel: String,
    pub mux_enabled: bool,
    pub def_allow_insecure: bool,
    pub def_fingerprint: String,
    pub def_user_agent: String,
    pub send_through: Option<String>,
    pub bind_interface: Option<String>,
    pub tls_fragment: TlsFragmentMode,
    pub fragment_fallback_delay_ms: i32,
    pub enable_cache_file4_sbox: bool,
}

impl Default for CoreBasicItem {
    fn default() -> Self {
        Self {
            log_enabled: false,
            loglevel: DEFAULT_LOG_LEVEL.to_string(),
            mux_enabled: false,
            def_allow_insecure: false,
            def_fingerprint: String::new(),
            def_user_agent: String::new(),
            send_through: None,
            bind_interface: None,
            tls_fragment: TlsFragmentMode::Off,
            fragment_fallback_delay_ms: DEFAULT_FRAGMENT_FALLBACK_DELAY_MS,
            enable_cache_file4_sbox: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InItem {
    pub local_port: i32,
    pub sniffing_enabled: bool,
    pub allow_lan_conn: bool,
    pub new_port4_lan: bool,
    pub user: String,
    pub pass: String,
    pub second_local_port_enabled: bool,
}

impl Default for InItem {
    fn default() -> Self {
        Self {
            local_port: DEFAULT_LOCAL_PORT,
            sniffing_enabled: true,
            allow_lan_conn: false,
            new_port4_lan: false,
            user: String::new(),
            pass: String::new(),
            second_local_port_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuiItem {
    pub auto_run: bool,
    pub auto_check_ip: bool,
    pub close_action: crate::CloseAction,
    pub start_minimized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiItem {
    pub current_theme: Option<String>,
    pub current_language: String,
}

impl Default for UiItem {
    fn default() -> Self {
        Self {
            current_theme: None,
            current_language: DEFAULT_LANGUAGE.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedTestItem {
    pub speed_test_timeout: i32,
    pub speed_ping_test_url: String,
    pub ipapi_url: String,
    pub speed_test_page_size: Option<i32>,
    pub speed_test_delay_interval_seconds: Option<i32>,
}

impl Default for SpeedTestItem {
    fn default() -> Self {
        Self {
            speed_test_timeout: 10,
            speed_ping_test_url: DEFAULT_SPEED_PING_TEST_URL.to_string(),
            ipapi_url: String::new(),
            speed_test_page_size: None,
            speed_test_delay_interval_seconds: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingBasicItem {
    pub domain_strategy: String,
    pub routing_index_id: String,
}

impl Default for RoutingBasicItem {
    fn default() -> Self {
        Self {
            domain_strategy: DEFAULT_DOMAIN_STRATEGY.to_string(),
            routing_index_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mux4SboxItem {
    pub protocol: String,
    pub max_connections: i32,
    pub padding: Option<bool>,
}

impl Default for Mux4SboxItem {
    fn default() -> Self {
        Self {
            protocol: DEFAULT_SINGBOX_MUX.to_string(),
            max_connections: 8,
            padding: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HysteriaItem {
    pub up_mbps: i32,
    pub down_mbps: i32,
    pub hop_interval: i32,
}

impl Default for HysteriaItem {
    fn default() -> Self {
        Self {
            up_mbps: 100,
            down_mbps: 100,
            hop_interval: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyUiItem {
    pub traffic_mode: TrafficMode,
}

impl Default for ProxyUiItem {
    fn default() -> Self {
        Self {
            traffic_mode: TrafficMode::Rule,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProxyItem {
    pub sys_proxy_type: SysProxyType,
    pub system_proxy_exceptions: String,
    pub not_proxy_local_address: bool,
}

impl Default for SystemProxyItem {
    fn default() -> Self {
        Self {
            sys_proxy_type: SysProxyType::ForcedChange,
            system_proxy_exceptions: DEFAULT_SYSTEM_PROXY_EXCEPTIONS.to_string(),
            not_proxy_local_address: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunModeItem {
    pub enable_tun: bool,
    pub auto_route: bool,
    pub strict_route: bool,
    pub stack: String,
    pub mtu: i32,
    pub enable_ipv6_address: bool,
    pub icmp_routing: String,
}

impl Default for TunModeItem {
    fn default() -> Self {
        Self {
            enable_tun: false,
            auto_route: true,
            strict_route: false,
            stack: String::new(),
            mtu: 1500,
            enable_ipv6_address: false,
            icmp_routing: DEFAULT_TUN_ICMP_ROUTING.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleDnsItem {
    pub add_common_hosts: Option<bool>,
    pub fake_ip: Option<bool>,
    pub global_fake_ip: Option<bool>,
    pub block_binding_query: Option<bool>,
    pub direct_dns: Option<String>,
    pub remote_dns: Option<String>,
    pub bootstrap_dns: Option<String>,
    pub strategy4_freedom: Option<String>,
    pub strategy4_proxy: Option<String>,
    pub hosts: Option<String>,
    pub direct_expected_ips: Option<String>,
}

impl Default for SimpleDnsItem {
    fn default() -> Self {
        SimpleDnsDefaults::builtin()
    }
}

pub struct SimpleDnsDefaults;

impl SimpleDnsDefaults {
    #[must_use]
    pub fn builtin() -> SimpleDnsItem {
        SimpleDnsItem {
            add_common_hosts: Some(true),
            fake_ip: Some(false),
            global_fake_ip: Some(true),
            block_binding_query: Some(true),
            direct_dns: Some(DEFAULT_DIRECT_DNS.to_string()),
            remote_dns: Some(DEFAULT_REMOTE_DNS.to_string()),
            bootstrap_dns: Some(DEFAULT_BOOTSTRAP_DNS.to_string()),
            strategy4_freedom: None,
            strategy4_proxy: None,
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

        assert_eq!(config.inbound.len(), 1);
        assert_eq!(config.inbound[0].local_port, 10808);
        assert!(config.inbound[0].sniffing_enabled);
        assert_eq!(config.core_basic_item.loglevel, "warn");
        assert_eq!(config.routing_basic_item.domain_strategy, "AsIs");
        assert_eq!(config.tun_mode_item.mtu, 1500);
        assert!(!config.tun_mode_item.strict_route);
        assert_eq!(config.speed_test_item.speed_test_timeout, 10);
        assert_eq!(config.mux4_sbox_item.protocol, "h2mux");
        assert_eq!(config.hysteria_item.up_mbps, 100);
        assert_eq!(config.hysteria_item.down_mbps, 100);
        assert_eq!(
            config.system_proxy_item.system_proxy_exceptions,
            DEFAULT_SYSTEM_PROXY_EXCEPTIONS
        );
        assert_eq!(
            config.simple_dns_item.direct_dns.as_deref(),
            Some(DEFAULT_DIRECT_DNS)
        );
        assert_eq!(
            config.simple_dns_item.remote_dns.as_deref(),
            Some(DEFAULT_REMOTE_DNS)
        );
    }
}
