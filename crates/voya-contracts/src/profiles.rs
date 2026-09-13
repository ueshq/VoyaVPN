use serde::{Deserialize, Serialize};
use specta::Type;

use crate::SpeedtestOutcome;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProfileKind {
    #[default]
    Vmess,
    Shadowsocks,
    Socks,
    Vless,
    Trojan,
    Hysteria2,
    Tuic,
    WireGuard,
    Http,
    Anytls,
    Naive,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerEndpoint {
    pub address: String,
    pub port: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
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
        cipher: Option<String>,
    },
    Shadowsocks {
        server: ServerEndpoint,
        password: String,
        method: String,
        udp_over_tcp: bool,
    },
    Socks {
        server: ServerEndpoint,
        username: String,
        password: String,
    },
    Vless {
        server: ServerEndpoint,
        uuid: String,
        flow: Option<String>,
        encryption: Option<String>,
    },
    Trojan {
        server: ServerEndpoint,
        password: String,
    },
    Hysteria2 {
        server: ServerEndpoint,
        password: String,
        port_hops: Option<String>,
        obfuscation_password: Option<String>,
    },
    Tuic {
        server: ServerEndpoint,
        uuid: String,
        password: String,
        congestion_control: Option<String>,
    },
    WireGuard {
        server: ServerEndpoint,
        private_key: String,
        peer_public_key: Option<String>,
        preshared_key: Option<String>,
        interface_address: Option<String>,
        allowed_ips: Option<String>,
        reserved: Option<String>,
        mtu: Option<i32>,
    },
    Http {
        server: ServerEndpoint,
        username: String,
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
        quic: bool,
        congestion_control: Option<String>,
        insecure_concurrency: Option<i32>,
        udp_over_tcp: bool,
    },
}

impl ProfileProtocol {
    #[must_use]
    pub const fn kind(&self) -> ProfileKind {
        match self {
            Self::Vmess { .. } => ProfileKind::Vmess,
            Self::Shadowsocks { .. } => ProfileKind::Shadowsocks,
            Self::Socks { .. } => ProfileKind::Socks,
            Self::Vless { .. } => ProfileKind::Vless,
            Self::Trojan { .. } => ProfileKind::Trojan,
            Self::Hysteria2 { .. } => ProfileKind::Hysteria2,
            Self::Tuic { .. } => ProfileKind::Tuic,
            Self::WireGuard { .. } => ProfileKind::WireGuard,
            Self::Http { .. } => ProfileKind::Http,
            Self::Anytls { .. } => ProfileKind::Anytls,
            Self::Naive { .. } => ProfileKind::Naive,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProfileTransport {
    Tcp {
        header: Option<String>,
        host: Option<String>,
        path: Option<String>,
    },
    Websocket {
        host: Option<String>,
        path: Option<String>,
    },
    HttpUpgrade {
        host: Option<String>,
        path: Option<String>,
    },
    Http2 {
        host: Option<String>,
        path: Option<String>,
    },
    Grpc {
        authority: Option<String>,
        service_name: Option<String>,
        mode: Option<String>,
    },
    Quic {
        host: Option<String>,
        path: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TlsMode {
    Tls,
    Reality,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TlsSettings {
    pub mode: TlsMode,
    pub server_name: Option<String>,
    pub alpn: Vec<String>,
    pub reality_public_key: Option<String>,
    pub reality_short_id: Option<String>,
    pub certificate_pem: Option<String>,
    pub ech_config: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub subscription_id: Option<String>,
    pub display_log: bool,
    pub remarks: String,
    pub protocol: ProfileProtocol,
    pub transport: Option<ProfileTransport>,
    pub tls: Option<TlsSettings>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileMetrics {
    pub delay_ms: i32,
    pub sort: i32,
    /// The last probe's outcome, decoded from the persisted `profile_ex`
    /// column. `None` means the profile has never been tested.
    pub outcome: Option<SpeedtestOutcome>,
    pub ip_info: Option<String>,
    pub country_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileTraffic {
    #[specta(type = f64)]
    pub total_upload: i64,
    #[specta(type = f64)]
    pub total_download: i64,
    #[specta(type = f64)]
    pub today_upload: i64,
    #[specta(type = f64)]
    pub today_download: i64,
    #[specta(type = f64)]
    pub date: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileListEntry {
    pub profile: Profile,
    pub metrics: ProfileMetrics,
    pub traffic: ProfileTraffic,
    pub is_active: bool,
}

/// A profile listing plus what is missing from it.
///
/// `undecodableProfiles` counts stored profiles this build could not read —
/// most often ones written by a newer build. Persistence skips those rows
/// rather than failing the whole listing, so one unreadable server cannot take
/// away the user's ability to see, connect to or delete the others.
///
/// The count travels *with* the rows instead of arriving as a notice on the
/// side: the listing is re-fetched on every profile query, so an event would
/// repeat without end, while a field on the response lets the screen state the
/// shortfall once, quietly, beside the list it belongs to.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileListing {
    pub entries: Vec<ProfileListEntry>,
    pub undecodable_profiles: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_contract_is_tagged_and_rejects_flat_legacy_fields() {
        let profile = Profile {
            id: "p1".to_string(),
            subscription_id: None,
            display_log: true,
            remarks: "node".to_string(),
            protocol: ProfileProtocol::Vless {
                server: ServerEndpoint {
                    address: "example.test".to_string(),
                    port: 443,
                },
                uuid: "00000000-0000-4000-8000-000000000001".to_string(),
                flow: Some("xtls-rprx-vision".to_string()),
                encryption: Some("none".to_string()),
            },
            transport: Some(ProfileTransport::Websocket {
                host: Some("cdn.example.test".to_string()),
                path: Some("/ws".to_string()),
            }),
            tls: Some(TlsSettings {
                mode: TlsMode::Tls,
                server_name: Some("example.test".to_string()),
                alpn: vec!["h2".to_string()],
                reality_public_key: None,
                reality_short_id: None,
                certificate_pem: None,
                ech_config: Vec::new(),
            }),
        };

        let json = serde_json::to_value(profile).expect("profile contract should serialize");
        assert_eq!(json["protocol"]["kind"], "vless");
        assert_eq!(json["transport"]["kind"], "websocket");
        assert!(json.get("configType").is_none());
        assert!(json.get("protocolExtra").is_none());
        assert!(json.get("transportExtra").is_none());
        assert!(serde_json::from_str::<Profile>(
            r#"{"configType":"vless","address":"example.test"}"#
        )
        .is_err());
    }
}
