use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ConfigType, CoreType, GlobalHotkey, GridOrientation, RuleMode, SysProxyType};

pub const DEFAULT_LOCAL_PORT: i32 = 10808;
pub const DEFAULT_LOG_LEVEL: &str = "warning";
pub const DEFAULT_DOMAIN_STRATEGY: &str = "AsIs";
pub const DEFAULT_TUN_ICMP_ROUTING: &str = "rule";
pub const DEFAULT_LANGUAGE: &str = "en";
pub const DEFAULT_SPEED_TEST_URL: &str = "https://cachefly.cachefly.net/50mb.test";
pub const DEFAULT_SPEED_PING_TEST_URL: &str = "https://www.google.com/generate_204";
pub const DEFAULT_UDP_TEST_TARGET: &str = "ntp:pool.ntp.org";
pub const DEFAULT_SINGBOX_MUX: &str = "h2mux";
pub const DEFAULT_SYSTEM_PROXY_EXCEPTIONS: &str = "localhost,127.0.0.0/8,::1";
pub const DEFAULT_DIRECT_DNS: &str = "119.29.29.29";
pub const DEFAULT_REMOTE_DNS: &str = "https://cloudflare-dns.com/dns-query";
pub const DEFAULT_BOOTSTRAP_DNS: &str = "119.29.29.29";

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct AppConfig {
    pub index_id: String,
    pub sub_index_id: String,
    pub core_basic_item: CoreBasicItem,
    pub tun_mode_item: TunModeItem,
    pub kcp_item: KcpItem,
    pub grpc_item: GrpcItem,
    pub routing_basic_item: RoutingBasicItem,
    #[serde(rename = "GUIItem")]
    pub gui_item: GuiItem,
    #[serde(rename = "MsgUIItem")]
    pub msg_ui_item: MsgUiItem,
    #[serde(rename = "UIItem")]
    pub ui_item: UiItem,
    pub const_item: ConstItem,
    pub speed_test_item: SpeedTestItem,
    pub mux4_ray_item: Mux4RayItem,
    pub mux4_sbox_item: Mux4SboxItem,
    pub hysteria_item: HysteriaItem,
    #[serde(rename = "ClashUIItem")]
    pub clash_ui_item: ClashUiItem,
    pub system_proxy_item: SystemProxyItem,
    pub web_dav_item: WebDavItem,
    pub check_update_item: CheckUpdateItem,
    pub diagnostics_item: DiagnosticsItem,
    pub fragment4_ray_item: Fragment4RayItem,
    pub inbound: Vec<InItem>,
    pub global_hotkeys: Vec<KeyEventItem>,
    pub core_type_item: Vec<CoreTypeItem>,
    #[serde(rename = "SimpleDNSItem")]
    pub simple_dns_item: SimpleDnsItem,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            index_id: String::new(),
            sub_index_id: String::new(),
            core_basic_item: CoreBasicItem::default(),
            tun_mode_item: TunModeItem::default(),
            kcp_item: KcpItem::default(),
            grpc_item: GrpcItem::default(),
            routing_basic_item: RoutingBasicItem::default(),
            gui_item: GuiItem::default(),
            msg_ui_item: MsgUiItem::default(),
            ui_item: UiItem::default(),
            const_item: ConstItem::default(),
            speed_test_item: SpeedTestItem::default(),
            mux4_ray_item: Mux4RayItem::default(),
            mux4_sbox_item: Mux4SboxItem::default(),
            hysteria_item: HysteriaItem::default(),
            clash_ui_item: ClashUiItem::default(),
            system_proxy_item: SystemProxyItem::default(),
            web_dav_item: WebDavItem::default(),
            check_update_item: CheckUpdateItem::default(),
            diagnostics_item: DiagnosticsItem::default(),
            fragment4_ray_item: Fragment4RayItem::default(),
            inbound: vec![InItem::default()],
            global_hotkeys: Vec::new(),
            core_type_item: Vec::new(),
            simple_dns_item: SimpleDnsItem::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct CoreBasicItem {
    pub log_enabled: bool,
    pub loglevel: String,
    pub mux_enabled: bool,
    pub def_allow_insecure: bool,
    pub def_fingerprint: String,
    pub def_user_agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_through: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_interface: Option<String>,
    pub enable_fragment: bool,
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
            enable_fragment: false,
            enable_cache_file4_sbox: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct InItem {
    pub local_port: i32,
    pub protocol: String,
    pub udp_enabled: bool,
    pub sniffing_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_override: Option<Vec<String>>,
    pub route_only: bool,
    #[serde(rename = "AllowLANConn")]
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
            protocol: "socks".to_string(),
            udp_enabled: true,
            sniffing_enabled: true,
            dest_override: Some(vec!["http".to_string(), "tls".to_string()]),
            route_only: false,
            allow_lan_conn: false,
            new_port4_lan: false,
            user: String::new(),
            pass: String::new(),
            second_local_port_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct KcpItem {
    pub mtu: i32,
    pub tti: i32,
    pub uplink_capacity: i32,
    pub downlink_capacity: i32,
    pub cwnd_multiplier: i32,
    pub max_sending_window: i32,
}

impl Default for KcpItem {
    fn default() -> Self {
        Self {
            mtu: 1350,
            tti: 50,
            uplink_capacity: 12,
            downlink_capacity: 100,
            cwnd_multiplier: 1,
            max_sending_window: 2 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct GrpcItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check_timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permit_without_stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_windows_size: Option<i32>,
}

impl Default for GrpcItem {
    fn default() -> Self {
        Self {
            idle_timeout: Some(60),
            health_check_timeout: Some(20),
            permit_without_stream: Some(false),
            initial_windows_size: Some(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct GuiItem {
    pub auto_run: bool,
    pub enable_statistics: bool,
    pub display_real_time_speed: bool,
    pub keep_older_dedupl: bool,
    pub auto_update_interval: i32,
    pub tray_menu_servers_limit: i32,
    #[serde(rename = "EnableHWA")]
    pub enable_hwa: bool,
    pub enable_log: bool,
}

impl Default for GuiItem {
    fn default() -> Self {
        Self {
            auto_run: false,
            enable_statistics: false,
            display_real_time_speed: false,
            keep_older_dedupl: false,
            auto_update_interval: 0,
            tray_menu_servers_limit: 20,
            enable_hwa: false,
            enable_log: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct MsgUiItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_msg_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_refresh: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct UiItem {
    pub enable_auto_adjust_main_lv_col_width: bool,
    pub main_gird_height1: i32,
    pub main_gird_height2: i32,
    pub main_gird_orientation: GridOrientation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_primary_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_theme: Option<String>,
    pub current_language: String,
    pub current_font_family: String,
    pub current_font_size: i32,
    pub enable_drag_drop_sort: bool,
    pub double_click2_activate: bool,
    pub auto_hide_startup: bool,
    pub hide2_tray_when_close: bool,
    #[serde(rename = "MacOSShowInDock")]
    pub mac_os_show_in_dock: bool,
    pub main_column_item: Vec<ColumnItem>,
    pub window_size_item: Vec<WindowSizeItem>,
}

impl Default for UiItem {
    fn default() -> Self {
        Self {
            enable_auto_adjust_main_lv_col_width: false,
            main_gird_height1: 0,
            main_gird_height2: 0,
            main_gird_orientation: GridOrientation::Vertical,
            color_primary_name: None,
            current_theme: None,
            current_language: DEFAULT_LANGUAGE.to_string(),
            current_font_family: String::new(),
            current_font_size: 0,
            enable_drag_drop_sort: false,
            double_click2_activate: false,
            auto_hide_startup: false,
            hide2_tray_when_close: false,
            mac_os_show_in_dock: false,
            main_column_item: Vec::new(),
            window_size_item: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct ConstItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_convert_url: Option<String>,
    #[serde(rename = "CdnBaseUrl", skip_serializing_if = "Option::is_none")]
    pub cdn_base_url: Option<String>,
    #[serde(rename = "CdnReleaseIndexUrl", skip_serializing_if = "Option::is_none")]
    pub cdn_release_index_url: Option<String>,
    #[serde(rename = "CdnCoreManifestUrl", skip_serializing_if = "Option::is_none")]
    pub cdn_core_manifest_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo_source_url: Option<String>,
    #[serde(rename = "SrsSourceUrl", skip_serializing_if = "Option::is_none")]
    pub srs_source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_rules_template_source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct SpeedTestItem {
    pub speed_test_timeout: i32,
    pub speed_test_url: String,
    pub speed_ping_test_url: String,
    pub mixed_concurrency_count: i32,
    #[serde(rename = "IPAPIUrl")]
    pub ipapi_url: String,
    pub udp_test_target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_test_page_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_test_delay_interval: Option<i32>,
}

impl Default for SpeedTestItem {
    fn default() -> Self {
        Self {
            speed_test_timeout: 10,
            speed_test_url: DEFAULT_SPEED_TEST_URL.to_string(),
            speed_ping_test_url: DEFAULT_SPEED_PING_TEST_URL.to_string(),
            mixed_concurrency_count: 5,
            ipapi_url: String::new(),
            udp_test_target: DEFAULT_UDP_TEST_TARGET.to_string(),
            speed_test_page_size: None,
            speed_test_delay_interval: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct RoutingBasicItem {
    pub domain_strategy: String,
    pub domain_strategy4_singbox: String,
    pub routing_index_id: String,
}

impl Default for RoutingBasicItem {
    fn default() -> Self {
        Self {
            domain_strategy: DEFAULT_DOMAIN_STRATEGY.to_string(),
            domain_strategy4_singbox: String::new(),
            routing_index_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct ColumnItem {
    pub name: String,
    pub width: i32,
    pub index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct Mux4RayItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xudp_concurrency: Option<i32>,
    #[serde(rename = "XudpProxyUDP443", skip_serializing_if = "Option::is_none")]
    pub xudp_proxy_udp443: Option<String>,
}

impl Default for Mux4RayItem {
    fn default() -> Self {
        Self {
            concurrency: Some(8),
            xudp_concurrency: Some(16),
            xudp_proxy_udp443: Some("reject".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct Mux4SboxItem {
    pub protocol: String,
    pub max_connections: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct ClashUiItem {
    pub rule_mode: RuleMode,
    #[serde(rename = "EnableIPv6")]
    pub enable_ipv6: bool,
    pub enable_mixin_content: bool,
    pub proxies_sorting: i32,
    pub proxies_auto_refresh: bool,
    pub proxies_auto_delay_test_interval: i32,
    pub connections_auto_refresh: bool,
    pub connections_refresh_interval: i32,
    pub connections_column_item: Vec<ColumnItem>,
}

impl Default for ClashUiItem {
    fn default() -> Self {
        Self {
            rule_mode: RuleMode::Rule,
            enable_ipv6: false,
            enable_mixin_content: false,
            proxies_sorting: 0,
            proxies_auto_refresh: false,
            proxies_auto_delay_test_interval: 10,
            connections_auto_refresh: false,
            connections_refresh_interval: 2,
            connections_column_item: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct SystemProxyItem {
    pub sys_proxy_type: SysProxyType,
    pub system_proxy_exceptions: String,
    pub not_proxy_local_address: bool,
    pub system_proxy_advanced_protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_system_proxy_pac_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_system_proxy_script_path: Option<String>,
}

impl Default for SystemProxyItem {
    fn default() -> Self {
        Self {
            sys_proxy_type: SysProxyType::ForcedClear,
            system_proxy_exceptions: DEFAULT_SYSTEM_PROXY_EXCEPTIONS.to_string(),
            not_proxy_local_address: true,
            system_proxy_advanced_protocol: String::new(),
            custom_system_proxy_pac_path: None,
            custom_system_proxy_script_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct WebDavItem {
    #[serde(rename = "Url", skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct CheckUpdateItem {
    pub check_pre_release_update: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_core_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct DiagnosticsItem {
    pub enabled: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub anonymous_install_id: String,
    #[serde(rename = "EndpointUrl", skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<String>,
}

impl Default for DiagnosticsItem {
    fn default() -> Self {
        Self {
            enabled: true,
            anonymous_install_id: String::new(),
            endpoint_url: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct Fragment4RayItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packets: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
}

impl Default for Fragment4RayItem {
    fn default() -> Self {
        Self {
            packets: Some("tlshello".to_string()),
            length: Some("50-100".to_string()),
            interval: Some("10-20".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct KeyEventItem {
    #[serde(rename = "EGlobalHotkey")]
    pub global_hotkey: GlobalHotkey,
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_code: Option<i32>,
}

impl Default for KeyEventItem {
    fn default() -> Self {
        Self {
            global_hotkey: GlobalHotkey::ShowForm,
            alt: false,
            control: false,
            shift: false,
            key_code: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct CoreTypeItem {
    pub config_type: ConfigType,
    pub core_type: CoreType,
}

impl Default for CoreTypeItem {
    fn default() -> Self {
        Self {
            config_type: ConfigType::VMess,
            core_type: CoreType::sing_box,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct TunModeItem {
    pub enable_tun: bool,
    pub auto_route: bool,
    pub strict_route: bool,
    pub stack: String,
    pub mtu: i32,
    #[serde(rename = "EnableIPv6Address")]
    pub enable_ipv6_address: bool,
    pub icmp_routing: String,
    pub enable_legacy_protect: bool,
}

impl Default for TunModeItem {
    fn default() -> Self {
        Self {
            enable_tun: false,
            auto_route: true,
            strict_route: true,
            stack: String::new(),
            mtu: 9000,
            enable_ipv6_address: false,
            icmp_routing: DEFAULT_TUN_ICMP_ROUTING.to_string(),
            enable_legacy_protect: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct WindowSizeItem {
    pub type_name: String,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct SimpleDnsItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_system_hosts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_common_hosts: Option<bool>,
    #[serde(rename = "FakeIP", skip_serializing_if = "Option::is_none")]
    pub fake_ip: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_fake_ip: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_binding_query: Option<bool>,
    #[serde(rename = "DirectDNS", skip_serializing_if = "Option::is_none")]
    pub direct_dns: Option<String>,
    #[serde(rename = "RemoteDNS", skip_serializing_if = "Option::is_none")]
    pub remote_dns: Option<String>,
    #[serde(rename = "BootstrapDNS", skip_serializing_if = "Option::is_none")]
    pub bootstrap_dns: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy4_freedom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy4_proxy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serve_stale: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_query: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<String>,
    #[serde(rename = "DirectExpectedIPs", skip_serializing_if = "Option::is_none")]
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
            use_system_hosts: Some(false),
            add_common_hosts: Some(true),
            fake_ip: Some(false),
            global_fake_ip: Some(true),
            block_binding_query: Some(true),
            direct_dns: Some(DEFAULT_DIRECT_DNS.to_string()),
            remote_dns: Some(DEFAULT_REMOTE_DNS.to_string()),
            bootstrap_dns: Some(DEFAULT_BOOTSTRAP_DNS.to_string()),
            strategy4_freedom: None,
            strategy4_proxy: None,
            serve_stale: Some(false),
            parallel_query: Some(false),
            hosts: None,
            direct_expected_ips: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn app_config_defaults_match_foundation_source() {
        let config = AppConfig::default();

        assert_eq!(config.inbound.len(), 1);
        assert_eq!(config.inbound[0].protocol, "socks");
        assert_eq!(config.inbound[0].local_port, 10808);
        assert!(config.inbound[0].udp_enabled);
        assert_eq!(config.core_basic_item.loglevel, "warning");
        assert_eq!(config.routing_basic_item.domain_strategy, "AsIs");
        assert_eq!(config.tun_mode_item.mtu, 9000);
        assert_eq!(config.speed_test_item.speed_test_timeout, 10);
        assert_eq!(config.speed_test_item.mixed_concurrency_count, 5);
        assert_eq!(config.mux4_ray_item.concurrency, Some(8));
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
        assert!(config.diagnostics_item.enabled);
        assert!(config.diagnostics_item.anonymous_install_id.is_empty());
        assert_eq!(config.diagnostics_item.endpoint_url, None);
    }

    #[test]
    fn app_config_uses_v2rayn_acronym_property_names() {
        let json = serde_json::to_value(AppConfig::default())
            .expect("default app config should serialize to JSON");
        let object = json
            .as_object()
            .expect("default app config JSON should be an object");

        assert!(object.contains_key("GUIItem"));
        assert!(object.contains_key("MsgUIItem"));
        assert!(object.contains_key("UIItem"));
        assert!(object.contains_key("ClashUIItem"));
        assert!(object.contains_key("SimpleDNSItem"));
    }

    #[test]
    fn partial_config_json_is_backfilled_with_defaults() {
        let config: AppConfig = serde_json::from_value(json!({
            "CoreBasicItem": {
                "Loglevel": "debug"
            }
        }))
        .expect("partial app config JSON should deserialize with defaults");

        assert_eq!(config.core_basic_item.loglevel, "debug");
        assert_eq!(config.inbound[0].local_port, DEFAULT_LOCAL_PORT);
        assert_eq!(
            config.simple_dns_item.bootstrap_dns.as_deref(),
            Some(DEFAULT_BOOTSTRAP_DNS)
        );
        assert!(config.diagnostics_item.enabled);
    }

    #[test]
    fn diagnostics_config_is_default_on_and_backfilled() {
        let config: AppConfig =
            serde_json::from_value(json!({})).expect("empty app config JSON should deserialize");

        assert!(config.diagnostics_item.enabled);
        assert!(config.diagnostics_item.anonymous_install_id.is_empty());
        assert_eq!(config.diagnostics_item.endpoint_url, None);
    }

    #[test]
    fn diagnostics_config_persists_opt_out_install_id_and_endpoint() {
        let config: AppConfig = serde_json::from_value(json!({
            "DiagnosticsItem": {
                "Enabled": false,
                "AnonymousInstallId": "00000000-0000-4000-8000-000000000001",
                "EndpointUrl": "https://diagnostics.voyavpn.test/ingest"
            },
            "CheckUpdateItem": {
                "CheckPreReleaseUpdate": true
            }
        }))
        .expect("diagnostics app config JSON should deserialize");

        assert!(!config.diagnostics_item.enabled);
        assert_eq!(
            config.diagnostics_item.anonymous_install_id,
            "00000000-0000-4000-8000-000000000001"
        );
        assert_eq!(
            config.diagnostics_item.endpoint_url.as_deref(),
            Some("https://diagnostics.voyavpn.test/ingest")
        );
        assert!(config.check_update_item.check_pre_release_update);
    }
}
