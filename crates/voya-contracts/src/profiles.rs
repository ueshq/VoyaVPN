use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{SpeedtestOutcome, ValidationIssue};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProfileKind {
    #[default]
    Vmess,
    Custom,
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
    PolicyGroup,
    ProxyChain,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerEndpoint {
    pub address: String,
    pub port: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LoadStrategy {
    #[default]
    LeastPing,
    Fallback,
    Random,
    RoundRobin,
    LeastLoad,
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
    Custom {
        source: String,
        filter: Option<String>,
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
    PolicyGroup {
        child_profile_ids: Vec<String>,
        source_subscription_id: Option<String>,
        filter: Option<String>,
        strategy: LoadStrategy,
    },
    ProxyChain {
        child_profile_ids: Vec<String>,
    },
}

impl ProfileProtocol {
    #[must_use]
    pub const fn kind(&self) -> ProfileKind {
        match self {
            Self::Vmess { .. } => ProfileKind::Vmess,
            Self::Custom { .. } => ProfileKind::Custom,
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
            Self::PolicyGroup { .. } => ProfileKind::PolicyGroup,
            Self::ProxyChain { .. } => ProfileKind::ProxyChain,
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
    Kcp {
        header: Option<String>,
        seed: Option<String>,
        mtu: Option<i32>,
    },
    Websocket {
        host: Option<String>,
        path: Option<String>,
    },
    HttpUpgrade {
        host: Option<String>,
        path: Option<String>,
    },
    Xhttp {
        host: Option<String>,
        path: Option<String>,
        mode: Option<String>,
        extra: Option<String>,
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
    pub reality_spider_x: Option<String>,
    pub mldsa65_verify: Option<String>,
    pub certificate_pem: Option<String>,
    pub certificate_sha256: Vec<String>,
    pub ech_config: Vec<String>,
    pub final_mask: Option<String>,
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
    pub speed_bytes_per_second: f64,
    pub sort: i32,
    /// The last probe's outcome, decoded from the persisted `profile_ex`
    /// column. `None` means the profile has never been tested.
    pub outcome: Option<SpeedtestOutcome>,
    pub ip_info: Option<String>,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProfileSortKey {
    #[default]
    Sort,
    Protocol,
    Remarks,
    Address,
    Port,
    Transport,
    Tls,
    Delay,
    Speed,
    IpInfo,
    SubscriptionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileDedupeResult {
    pub total: u32,
    pub kept: u32,
    pub removed_profile_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupChildCandidate {
    pub profile_id: String,
    pub remarks: String,
    pub address: String,
    pub protocol: ProfileKind,
    pub subscription_id: Option<String>,
    pub is_group: bool,
    pub selectable: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupValidation {
    pub valid: bool,
    pub child_profile_ids: Vec<String>,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupPreviewRoute {
    pub tag: String,
    pub kind: String,
    pub dialer_proxy: Option<String>,
    pub download_dialer_proxy: Option<String>,
    pub detour: Option<String>,
    pub outbounds: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupPreview {
    pub validation: GroupValidation,
    pub singbox_routes: Vec<GroupPreviewRoute>,
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
                reality_spider_x: None,
                mldsa65_verify: None,
                certificate_pem: None,
                certificate_sha256: Vec::new(),
                ech_config: Vec::new(),
                final_mask: None,
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
