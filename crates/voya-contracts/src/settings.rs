use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{DnsSettings, SystemProxyType, TrafficMode, CURRENT_SCHEMA_VERSION};

// Settings use the same strict current shape for IPC and persistence. The
// database baseline rejects historical installations before reading these DTOs;
// there are no retired-field conversions. Keep the current settings fixture and
// generated bindings aligned when changing the contract.
// Plain comments avoid exporting persistence guidance into TypeScript bindings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettingsV1 {
    pub schema_version: u32,
    pub appearance: AppearanceSettings,
    pub behavior: BehaviorSettings,
    pub core: CoreSettings,
    pub network: NetworkSettings,
    pub routing: RoutingSettings,
    pub dns: DnsSettings,
    // Serialized as `speedTest`. The type follows the `Speedtest` spelling the
    // commands and events use, but the field name is a persisted JSON key and
    // stays as it is.
    pub speed_test: SpeedtestSettings,
    pub multiplexing: MultiplexingSettings,
    pub hysteria: HysteriaSettings,
    pub proxy: ProxySettings,
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
            dns: DnsSettings::default(),
            speed_test: SpeedtestSettings::default(),
            multiplexing: MultiplexingSettings::default(),
            hysteria: HysteriaSettings::default(),
            proxy: ProxySettings::default(),
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
    /// Look up the exit IP each time a connection is established.
    pub auto_check_ip: bool,
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
    pub tls_fragment: TlsFragmentMode,
    pub fragment_fallback_delay_ms: i32,
    pub cache_file_enabled: bool,
}

/// How TLS handshakes are split to get past filters that match on the
/// ClientHello.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TlsFragmentMode {
    #[default]
    Off,
    /// sing-box `tls.fragment`: the ClientHello is split into TCP segments.
    TlsHello,
    /// sing-box `tls.record_fragment`: the ClientHello is split into TLS records.
    Record,
}

const fn default_fragment_fallback_delay_ms() -> i32 {
    500
}

impl Default for CoreSettings {
    fn default() -> Self {
        Self {
            log_enabled: false,
            log_level: "warn".to_string(),
            mux_enabled: false,
            default_allow_insecure: false,
            default_fingerprint: String::new(),
            default_user_agent: String::new(),
            send_through: None,
            bind_interface: None,
            tls_fragment: TlsFragmentMode::Off,
            fragment_fallback_delay_ms: default_fragment_fallback_delay_ms(),
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
    /// The persisted OS-proxy mode. Typed rather than a `String`: the enum's
    /// `rename_all = "camelCase"` emits exactly the values this field stores
    /// (`forcedClear`, `forcedChange`, `unchanged`), and `voya-db` pins that
    /// with a value test. The retired `pac` value is normalized at startup.
    pub mode: SystemProxyType,
    pub exceptions: String,
    pub bypass_local: bool,
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
            mode: SystemProxyType::ForcedChange,
            exceptions: DEFAULT_SYSTEM_PROXY_EXCEPTIONS.to_string(),
            bypass_local: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingSettings {
    pub domain_strategy: String,
}

impl Default for RoutingSettings {
    fn default() -> Self {
        Self {
            domain_strategy: "AsIs".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeedtestSettings {
    pub timeout_seconds: i32,
    pub latency_url: String,
    pub ip_lookup_url: String,
    pub page_size: Option<i32>,
    pub delay_interval_seconds: Option<i32>,
}

impl Default for SpeedtestSettings {
    fn default() -> Self {
        Self {
            timeout_seconds: 10,
            latency_url: "https://www.google.com/generate_204".to_string(),
            ip_lookup_url: String::new(),
            page_size: None,
            delay_interval_seconds: None,
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
    /// The persisted Clash traffic mode. Typed for the same reason as
    /// [`SystemProxySettings::mode`]: `rename_all = "camelCase"` emits the very
    /// canonical strings this field stores (`rule`, `global`, `unchanged`).
    pub traffic_mode: TrafficMode,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            traffic_mode: TrafficMode::Rule,
        }
    }
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
            serde_json::Value::String("forcedChange".to_string())
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
