use serde::{Deserialize, Serialize};

use crate::{ConfigType, RuleType};

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerEndpoint {
    pub address: String,
    pub port: i32,
}

/// Persisted as-is inside the profile blob column, so every field that is not
/// structurally required carries `#[serde(default)]`: a row written before that
/// field existed must still decode after an upgrade instead of being skipped as
/// undecodable. `#[serde(default)]` only affects deserialization, so the
/// serialized shape pinned by `voya-db`'s blob fixture is unchanged. New fields
/// must be added with `#[serde(default)]` for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProfileProtocol {
    Vmess {
        server: ServerEndpoint,
        uuid: String,
        #[serde(default)]
        cipher: Option<String>,
    },
    Shadowsocks {
        server: ServerEndpoint,
        password: String,
        method: String,
        #[serde(default)]
        udp_over_tcp: bool,
    },
    Socks {
        server: ServerEndpoint,
        #[serde(default)]
        username: String,
        #[serde(default)]
        password: String,
    },
    Vless {
        server: ServerEndpoint,
        uuid: String,
        #[serde(default)]
        flow: Option<String>,
        #[serde(default)]
        encryption: Option<String>,
    },
    Trojan {
        server: ServerEndpoint,
        password: String,
    },
    Hysteria2 {
        server: ServerEndpoint,
        password: String,
        #[serde(default)]
        port_hops: Option<String>,
        #[serde(default)]
        obfuscation_password: Option<String>,
    },
    Tuic {
        server: ServerEndpoint,
        uuid: String,
        password: String,
        #[serde(default)]
        congestion_control: Option<String>,
    },
    WireGuard {
        server: ServerEndpoint,
        private_key: String,
        #[serde(default)]
        peer_public_key: Option<String>,
        #[serde(default)]
        preshared_key: Option<String>,
        #[serde(default)]
        interface_address: Option<String>,
        #[serde(default)]
        allowed_ips: Option<String>,
        #[serde(default)]
        reserved: Option<String>,
        #[serde(default)]
        mtu: Option<i32>,
    },
    Http {
        server: ServerEndpoint,
        #[serde(default)]
        username: String,
        #[serde(default)]
        password: String,
    },
    Anytls {
        server: ServerEndpoint,
        password: String,
    },
    Naive {
        server: ServerEndpoint,
        username: String,
        password: String,
        #[serde(default)]
        quic: bool,
        #[serde(default)]
        congestion_control: Option<String>,
        #[serde(default)]
        insecure_concurrency: Option<i32>,
        #[serde(default)]
        udp_over_tcp: bool,
    },
}

impl Default for ProfileProtocol {
    fn default() -> Self {
        Self::Vmess {
            server: ServerEndpoint::default(),
            uuid: String::new(),
            cipher: None,
        }
    }
}

impl ProfileProtocol {
    #[must_use]
    pub fn empty(config_type: ConfigType, server: ServerEndpoint) -> Self {
        match config_type {
            ConfigType::VMess => Self::Vmess {
                server,
                uuid: String::new(),
                cipher: None,
            },
            ConfigType::Shadowsocks => Self::Shadowsocks {
                server,
                password: String::new(),
                method: String::new(),
                udp_over_tcp: false,
            },
            ConfigType::SOCKS => Self::Socks {
                server,
                username: String::new(),
                password: String::new(),
            },
            ConfigType::VLESS => Self::Vless {
                server,
                uuid: String::new(),
                flow: None,
                encryption: None,
            },
            ConfigType::Trojan => Self::Trojan {
                server,
                password: String::new(),
            },
            ConfigType::Hysteria2 => Self::Hysteria2 {
                server,
                password: String::new(),
                port_hops: None,
                obfuscation_password: None,
            },
            ConfigType::TUIC => Self::Tuic {
                server,
                uuid: String::new(),
                password: String::new(),
                congestion_control: None,
            },
            ConfigType::WireGuard => Self::WireGuard {
                server,
                private_key: String::new(),
                peer_public_key: None,
                preshared_key: None,
                interface_address: None,
                allowed_ips: None,
                reserved: None,
                mtu: None,
            },
            ConfigType::HTTP => Self::Http {
                server,
                username: String::new(),
                password: String::new(),
            },
            ConfigType::Anytls => Self::Anytls {
                server,
                password: String::new(),
            },
            ConfigType::Naive => Self::Naive {
                server,
                username: String::new(),
                password: String::new(),
                quic: false,
                congestion_control: None,
                insecure_concurrency: None,
                udp_over_tcp: false,
            },
        }
    }

    #[must_use]
    pub const fn config_type(&self) -> ConfigType {
        match self {
            Self::Vmess { .. } => ConfigType::VMess,
            Self::Shadowsocks { .. } => ConfigType::Shadowsocks,
            Self::Socks { .. } => ConfigType::SOCKS,
            Self::Vless { .. } => ConfigType::VLESS,
            Self::Trojan { .. } => ConfigType::Trojan,
            Self::Hysteria2 { .. } => ConfigType::Hysteria2,
            Self::Tuic { .. } => ConfigType::TUIC,
            Self::WireGuard { .. } => ConfigType::WireGuard,
            Self::Http { .. } => ConfigType::HTTP,
            Self::Anytls { .. } => ConfigType::Anytls,
            Self::Naive { .. } => ConfigType::Naive,
        }
    }

    #[must_use]
    pub const fn server(&self) -> Option<&ServerEndpoint> {
        match self {
            Self::Vmess { server, .. }
            | Self::Shadowsocks { server, .. }
            | Self::Socks { server, .. }
            | Self::Vless { server, .. }
            | Self::Trojan { server, .. }
            | Self::Hysteria2 { server, .. }
            | Self::Tuic { server, .. }
            | Self::WireGuard { server, .. }
            | Self::Http { server, .. }
            | Self::Anytls { server, .. }
            | Self::Naive { server, .. } => Some(server),
        }
    }

    #[must_use]
    pub fn searchable_address(&self) -> &str {
        self.server().map_or("", |server| server.address.as_str())
    }

    #[must_use]
    pub fn password(&self) -> &str {
        match self {
            Self::Vmess { uuid, .. } | Self::Vless { uuid, .. } => uuid,
            Self::Shadowsocks { password, .. }
            | Self::Socks { password, .. }
            | Self::Trojan { password, .. }
            | Self::Hysteria2 { password, .. }
            | Self::Tuic { password, .. }
            | Self::Http { password, .. }
            | Self::Anytls { password, .. }
            | Self::Naive { password, .. } => password,
            Self::WireGuard { private_key, .. } => private_key,
        }
    }

    #[must_use]
    pub fn username(&self) -> &str {
        match self {
            Self::Socks { username, .. }
            | Self::Http { username, .. }
            | Self::Naive { username, .. } => username,
            Self::Tuic { uuid, .. } => uuid,
            _ => "",
        }
    }
}

/// Every field is optional by nature and defaulted on deserialization for the
/// same forward-compatibility reason as [`ProfileProtocol`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProfileTransport {
    Tcp {
        #[serde(default)]
        header: Option<String>,
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },
    Kcp {
        #[serde(default)]
        header: Option<String>,
        #[serde(default)]
        seed: Option<String>,
        #[serde(default)]
        mtu: Option<i32>,
    },
    Websocket {
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },
    HttpUpgrade {
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },
    Xhttp {
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        extra: Option<String>,
    },
    Http2 {
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },
    Grpc {
        #[serde(default)]
        authority: Option<String>,
        #[serde(default)]
        service_name: Option<String>,
        #[serde(default)]
        mode: Option<String>,
    },
    Quic {
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },
}

impl ProfileTransport {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            // sing-box calls its plain TCP transport `raw`; keep that canonical
            // domain value even though the public tagged-union variant is `tcp`.
            Self::Tcp { .. } => "raw",
            Self::Kcp { .. } => "kcp",
            Self::Websocket { .. } => "ws",
            Self::HttpUpgrade { .. } => "httpupgrade",
            Self::Xhttp { .. } => "xhttp",
            Self::Http2 { .. } => "h2",
            Self::Grpc { .. } => "grpc",
            Self::Quic { .. } => "quic",
        }
    }

    #[must_use]
    pub fn host(&self) -> Option<&str> {
        match self {
            Self::Tcp { host, .. }
            | Self::Websocket { host, .. }
            | Self::HttpUpgrade { host, .. }
            | Self::Xhttp { host, .. }
            | Self::Http2 { host, .. }
            | Self::Quic { host, .. } => host.as_deref(),
            Self::Kcp { .. } => None,
            Self::Grpc { authority, .. } => authority.as_deref(),
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Tcp { path, .. } => path.as_deref(),
            Self::Kcp { seed, .. } => seed.as_deref(),
            Self::Websocket { path, .. }
            | Self::HttpUpgrade { path, .. }
            | Self::Xhttp { path, .. }
            | Self::Http2 { path, .. }
            | Self::Quic { path, .. } => path.as_deref(),
            Self::Grpc { service_name, .. } => service_name.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TlsMode {
    /// Plain TLS, and the default a TLS block decodes to when `mode` is absent.
    #[default]
    Tls,
    Reality,
}

/// Persisted as-is inside the profile blob column.
///
/// The container-level `#[serde(default)]` is what makes an older row survive
/// an upgrade: a TLS block written before a field existed decodes with that
/// field defaulted instead of failing with `missing field` and being skipped.
/// It changes deserialization only, so the shape pinned by `voya-db`'s blob
/// fixture is unchanged, and it covers fields added later automatically.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct TlsSettings {
    pub mode: TlsMode,
    pub server_name: Option<String>,
    pub alpn: Vec<String>,
    pub reality_public_key: Option<String>,
    pub reality_short_id: Option<String>,
    pub reality_spider_x: Option<String>,
    pub mldsa65_verify: Option<String>,
    pub certificate_pem: Option<String>,
    pub certificate_sha256: Vec<String>,
    pub ech_config: Vec<String>,
    pub final_mask: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileItem {
    pub index_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
    pub display_log: bool,
    pub remarks: String,
    pub protocol: ProfileProtocol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<ProfileTransport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsSettings>,
}

impl Default for ProfileItem {
    fn default() -> Self {
        Self {
            index_id: String::new(),
            subscription_id: None,
            display_log: true,
            remarks: String::new(),
            protocol: ProfileProtocol::default(),
            transport: None,
            tls: None,
        }
    }
}

impl ProfileItem {
    #[must_use]
    pub const fn config_type(&self) -> ConfigType {
        self.protocol.config_type()
    }

    #[must_use]
    pub fn address(&self) -> &str {
        self.protocol.searchable_address()
    }

    #[must_use]
    pub fn port(&self) -> i32 {
        self.protocol.server().map_or(0, |server| server.port)
    }

    #[must_use]
    pub fn network(&self) -> &str {
        self.transport.as_ref().map_or("", ProfileTransport::name)
    }

    #[must_use]
    pub fn stream_security(&self) -> &str {
        self.tls.as_ref().map_or("", |tls| match tls.mode {
            TlsMode::Tls => "tls",
            TlsMode::Reality => "reality",
        })
    }

    #[must_use]
    pub fn password(&self) -> &str {
        self.protocol.password()
    }

    #[must_use]
    pub fn username(&self) -> &str {
        self.protocol.username()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubItem {
    pub id: String,
    pub remarks: String,
    pub url: String,
    pub more_url: String,
    pub enabled: bool,
    pub user_agent: String,
    pub sort: i32,
    pub filter: Option<String>,
    pub convert_target: Option<String>,
    pub auto_update_interval_minutes: Option<i32>,
}

impl Default for SubItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            url: String::new(),
            more_url: String::new(),
            enabled: true,
            user_agent: String::new(),
            sort: 0,
            filter: None,
            convert_target: None,
            auto_update_interval_minutes: None,
        }
    }
}

/// Server-reported subscription usage captured from response headers
/// (`subscription-userinfo`, `profile-title`) on the latest successful fetch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubMetadataItem {
    pub subscription_id: String,
    pub upload_bytes: Option<i64>,
    pub download_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub expire_at: Option<i64>,
    pub last_update_at: Option<i64>,
    pub profile_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportProfilesResult {
    pub imported: u32,
    pub updated: u32,
    pub skipped: u32,
    pub parsed: u32,
    pub filtered: u32,
    pub deduped: u32,
    pub failed: u32,
    pub removed_existing: u32,
    pub removed_duplicates: u32,
    pub discarded_node_overrides: u32,
    pub subscription_id: Option<String>,
    pub imported_index_ids: Vec<String>,
    pub updated_index_ids: Vec<String>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SubscriptionUpdateResult {
    pub updated: u32,
    pub skipped: u32,
    pub imported: u32,
    pub removed_existing: u32,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingItem {
    pub id: String,
    pub remarks: String,
    pub url: String,
    pub rule_set: Vec<RulesItem>,
    pub enabled: bool,
    pub locked: bool,
    pub custom_icon: String,
    pub custom_ruleset_path4_singbox: String,
    pub domain_strategy: String,
    pub domain_strategy4_singbox: String,
    pub sort: i32,
    #[serde(default, skip_deserializing)]
    pub is_active: bool,
}

impl Default for RoutingItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            url: String::new(),
            rule_set: Vec::new(),
            enabled: true,
            locked: false,
            custom_icon: String::new(),
            custom_ruleset_path4_singbox: String::new(),
            domain_strategy: String::new(),
            domain_strategy4_singbox: String::new(),
            sort: 0,
            is_active: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesItem {
    pub id: String,
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
    pub ip: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Vec<String>>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_type: Option<RuleType>,
}

impl Default for RulesItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            r#type: None,
            port: None,
            network: None,
            inbound_tag: None,
            outbound_tag: None,
            ip: None,
            domain: None,
            protocol: None,
            process: None,
            enabled: true,
            remarks: None,
            rule_type: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProfileExItem {
    pub index_id: String,
    pub delay: i32,
    pub sort: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileListItem {
    pub profile: ProfileItem,
    pub profile_ex: ProfileExItem,
    pub server_stat: ServerStatItem,
    pub is_active: bool,
}

#[must_use]
pub fn profile_items_match(left: &ProfileItem, right: &ProfileItem, compare_remarks: bool) -> bool {
    left.protocol == right.protocol
        && left.transport == right.transport
        && left.tls == right.tls
        && (!compare_remarks || left.remarks == right.remarks)
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerStatItem {
    pub index_id: String,
    pub total_up: i64,
    pub total_down: i64,
    pub today_up: i64,
    pub today_down: i64,
    pub date_now: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_item_serializes_live_fields_without_obsolete_profile_columns() {
        let json = serde_json::to_value(ProfileItem::default())
            .expect("default profile item should serialize to JSON");
        let object = json
            .as_object()
            .expect("default profile item JSON should be an object");

        for obsolete in ["protocolExtra", "transportExtra", "AlterId", "HeaderType"] {
            assert!(
                !object.contains_key(obsolete),
                "{obsolete} should be absent"
            );
        }

        assert!(object.contains_key("protocol"));
        assert!(!object.contains_key("transport"));
        assert!(!object.contains_key("tls"));
    }

    #[test]
    fn profile_protocol_serializes_tagged_string_enums() {
        let protocol = ProfileProtocol::Vmess {
            server: ServerEndpoint {
                address: "node.example".to_string(),
                port: 443,
            },
            uuid: "uuid".to_string(),
            cipher: None,
        };
        let json = serde_json::to_value(&protocol).expect("protocol JSON");
        assert_eq!(json["kind"], "vmess");
        assert_eq!(json["server"]["port"], 443);
    }

    #[test]
    fn routing_rule_rejects_legacy_lowercase_outboundtag() {
        let result = serde_json::from_str::<RulesItem>(
            r#"{"outboundtag":"direct","enabled":true,"remarks":"regional"}"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn profile_items_match_uses_canonical_protocol_fields() {
        let base = ProfileItem {
            remarks: "one".to_string(),
            protocol: ProfileProtocol::Vless {
                server: ServerEndpoint {
                    address: "example.com".to_string(),
                    port: 443,
                },
                uuid: "uuid".to_string(),
                flow: Some("xtls-rprx-vision".to_string()),
                encryption: Some("none".to_string()),
            },
            transport: Some(ProfileTransport::Websocket {
                host: Some("example.com".to_string()),
                path: Some("/ws".to_string()),
            }),
            tls: Some(TlsSettings {
                mode: TlsMode::Tls,
                server_name: Some("example.com".to_string()),
                alpn: Vec::new(),
                reality_public_key: None,
                reality_short_id: None,
                reality_spider_x: None,
                mldsa65_verify: None,
                certificate_pem: None,
                certificate_sha256: Vec::new(),
                ech_config: Vec::new(),
                final_mask: None,
            }),
            ..ProfileItem::default()
        };
        let mut duplicate = base.clone();
        duplicate.index_id = "other".to_string();
        duplicate.remarks = "renamed".to_string();

        assert!(profile_items_match(&base, &duplicate, false));
        assert!(!profile_items_match(&base, &duplicate, true));

        duplicate.transport = Some(ProfileTransport::Websocket {
            host: Some("example.com".to_string()),
            path: Some("/other".to_string()),
        });
        assert!(!profile_items_match(&base, &duplicate, false));
    }
}
