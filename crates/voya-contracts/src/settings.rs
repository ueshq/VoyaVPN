use serde::{Deserialize, Serialize};
use specta::Type;

use crate::CURRENT_SCHEMA_VERSION;

// LOAD-BEARING FOR PERSISTENCE, not just for IPC.
//
// `voya_db::SettingsRepository` stores this exact type as the JSON payload of
// `app_settings`, so this struct and every nested settings struct below is also
// the on-disk schema of every installed database. They all carry
// `deny_unknown_fields`, which makes both directions breaking: removing or
// renaming a field rejects payloads written by earlier builds, and adding one
// without `#[serde(default)]` rejects them for the missing key. Either way
// `load()` fails for existing installs, `AppServices::load_config` propagates
// it out of `setup()`, and the app cannot launch.
//
// So: add fields with `#[serde(default)]`, never remove or rename one, and
// bump `CURRENT_SCHEMA_VERSION` with a migration when the layout really has to
// change. voya-db pins the current layout against a checked-in fixture
// (`crates/voya-db/fixtures/app_settings_v1.json`).
//
// Deliberately a plain comment, not a doc comment: specta copies doc comments
// into `bindings.ts`, and this note is for Rust authors only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettingsV1 {
    pub schema_version: u32,
    pub appearance: AppearanceSettings,
    pub behavior: BehaviorSettings,
    pub core: CoreSettings,
    pub network: NetworkSettings,
    pub routing: RoutingSettings,
    pub dns: AppDnsSettings,
    pub sources: SourceSettings,
    pub speed_test: SpeedTestSettings,
    pub multiplexing: MultiplexingSettings,
    pub grpc: GrpcSettings,
    pub hysteria: HysteriaSettings,
    pub proxy: ProxySettings,
    pub shortcuts: ShortcutSettings,
}

impl Default for AppSettingsV1 {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            appearance: AppearanceSettings::default(),
            behavior: BehaviorSettings::default(),
            core: CoreSettings::default(),
            network: NetworkSettings::default(),
            routing: RoutingSettings::default(),
            dns: AppDnsSettings::default(),
            sources: SourceSettings::default(),
            speed_test: SpeedTestSettings::default(),
            multiplexing: MultiplexingSettings::default(),
            grpc: GrpcSettings::default(),
            hysteria: HysteriaSettings::default(),
            proxy: ProxySettings::default(),
            shortcuts: ShortcutSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceSettings {
    pub language: String,
    pub theme: ThemeMode,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: ThemeMode::System,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BehaviorSettings {
    pub autostart: bool,
    pub statistics: bool,
    pub realtime_speed: bool,
    /// `None` means enabled (the default); optional so settings blobs saved by
    /// older builds keep deserializing under `deny_unknown_fields`.
    #[serde(default)]
    pub auto_create_subscription_group: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CoreSettings {
    pub log_enabled: bool,
    pub log_level: String,
    pub mux_enabled: bool,
    pub default_allow_insecure: bool,
    pub default_fingerprint: String,
    pub default_user_agent: String,
    pub send_through: Option<String>,
    pub bind_interface: Option<String>,
    pub fragment_enabled: bool,
    pub cache_file_enabled: bool,
}

impl Default for CoreSettings {
    fn default() -> Self {
        Self {
            log_enabled: false,
            log_level: "warning".to_string(),
            mux_enabled: false,
            default_allow_insecure: false,
            default_fingerprint: String::new(),
            default_user_agent: String::new(),
            send_through: None,
            bind_interface: None,
            fragment_enabled: false,
            cache_file_enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkSettings {
    pub tun: TunSettings,
    pub system_proxy: SystemProxySettings,
    pub inbounds: Vec<InboundSettings>,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            tun: TunSettings::default(),
            system_proxy: SystemProxySettings::default(),
            inbounds: vec![InboundSettings::default()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TunSettings {
    pub enabled: bool,
    pub auto_route: bool,
    pub strict_route: bool,
    pub stack: String,
    pub mtu: i32,
    pub ipv6_enabled: bool,
    pub icmp_routing: String,
}

impl Default for TunSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_route: true,
            strict_route: false,
            stack: String::new(),
            mtu: 1500,
            ipv6_enabled: false,
            icmp_routing: "rule".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InboundSettings {
    pub local_port: i32,
    pub protocol: String,
    pub sniffing_enabled: bool,
    pub lan_connections_allowed: bool,
    pub separate_lan_port: bool,
    pub username: String,
    pub password: String,
    pub secondary_port_enabled: bool,
}

impl Default for InboundSettings {
    fn default() -> Self {
        Self {
            local_port: 10_808,
            protocol: "socks".to_string(),
            sniffing_enabled: true,
            lan_connections_allowed: false,
            separate_lan_port: false,
            username: String::new(),
            password: String::new(),
            secondary_port_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemProxySettings {
    pub mode: String,
    pub exceptions: String,
    pub bypass_local: bool,
    pub advanced_protocol: String,
    pub custom_pac_path: Option<String>,
    pub custom_script_path: Option<String>,
}

/// Loopback and link-local destinations that never belong on a proxy. Mirrors
/// `voya_core::DEFAULT_SYSTEM_PROXY_EXCEPTIONS`; the literal is duplicated
/// because contracts must not depend on the domain crate.
const DEFAULT_SYSTEM_PROXY_EXCEPTIONS: &str = "localhost,127.0.0.0/8,::1";

impl Default for SystemProxySettings {
    fn default() -> Self {
        // A fresh install has no persisted settings row, so these defaults are
        // what the system proxy is actually configured with. They must stay in
        // step with `voya_core::SystemProxyItem::default()`; the equivalence is
        // guarded by a test in voya-app's settings mapping layer.
        Self {
            mode: "forcedClear".to_string(),
            exceptions: DEFAULT_SYSTEM_PROXY_EXCEPTIONS.to_string(),
            bypass_local: true,
            advanced_protocol: String::new(),
            custom_pac_path: None,
            custom_script_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingSettings {
    pub domain_strategy: String,
    pub singbox_domain_strategy: String,
}

impl Default for RoutingSettings {
    fn default() -> Self {
        Self {
            domain_strategy: "AsIs".to_string(),
            singbox_domain_strategy: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppDnsSettings {
    pub use_system_hosts: Option<bool>,
    pub add_common_hosts: Option<bool>,
    pub fake_ip: Option<bool>,
    pub global_fake_ip: Option<bool>,
    pub block_binding_query: Option<bool>,
    pub direct: Option<String>,
    pub remote: Option<String>,
    pub bootstrap: Option<String>,
    pub direct_strategy: Option<String>,
    pub proxy_strategy: Option<String>,
    pub serve_stale: Option<bool>,
    pub parallel_query: Option<bool>,
    pub hosts: Option<String>,
    pub direct_expected_ips: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSettings {
    pub subscription_converter: Option<String>,
    pub geo: Option<String>,
    pub singbox_ruleset: Option<String>,
    pub routing_template: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeedTestSettings {
    pub timeout_seconds: i32,
    pub download_url: String,
    pub latency_url: String,
    pub mixed_concurrency: i32,
    pub ip_lookup_url: String,
    pub udp_target: String,
    pub page_size: Option<i32>,
    pub delay_interval_ms: Option<i32>,
}

impl Default for SpeedTestSettings {
    fn default() -> Self {
        Self {
            timeout_seconds: 10,
            download_url: "https://cachefly.cachefly.net/50mb.test".to_string(),
            latency_url: "https://www.google.com/generate_204".to_string(),
            mixed_concurrency: 5,
            ip_lookup_url: String::new(),
            udp_target: "ntp:pool.ntp.org".to_string(),
            page_size: None,
            delay_interval_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MultiplexingSettings {
    pub protocol: String,
    pub max_connections: i32,
    pub padding: Option<bool>,
}

impl Default for MultiplexingSettings {
    fn default() -> Self {
        Self {
            protocol: "h2mux".to_string(),
            max_connections: 8,
            padding: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrpcSettings {
    pub idle_timeout_seconds: Option<i32>,
    pub health_check_timeout_seconds: Option<i32>,
    pub permit_without_stream: Option<bool>,
}

impl Default for GrpcSettings {
    fn default() -> Self {
        Self {
            idle_timeout_seconds: Some(60),
            health_check_timeout_seconds: Some(20),
            permit_without_stream: Some(false),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HysteriaSettings {
    pub upload_mbps: i32,
    pub download_mbps: i32,
    pub hop_interval_seconds: i32,
}

impl Default for HysteriaSettings {
    fn default() -> Self {
        Self {
            upload_mbps: 100,
            download_mbps: 100,
            hop_interval_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProxySettings {
    pub traffic_mode: String,
    pub node_sorting: i32,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            traffic_mode: "rule".to_string(),
            node_sorting: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutSettings {
    pub show_window_shortcut: Option<ShortcutChord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutChord {
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub key_code: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_strict_and_versioned() {
        let value = serde_json::to_value(AppSettingsV1::default()).expect("serialize settings");
        assert_eq!(value["schemaVersion"], CURRENT_SCHEMA_VERSION);
        assert_eq!(
            value["network"]["systemProxy"]["mode"],
            serde_json::Value::String("forcedClear".to_string())
        );
        assert!(value.get("core").is_some());
        assert!(value.get("CoreBasicItem").is_none());

        let mut invalid = value;
        invalid
            .as_object_mut()
            .expect("settings object")
            .insert("legacyField".to_string(), serde_json::Value::Bool(true));
        assert!(serde_json::from_value::<AppSettingsV1>(invalid).is_err());
    }
}
