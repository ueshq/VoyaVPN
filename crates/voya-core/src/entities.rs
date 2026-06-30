use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ConfigType, MultipleLoad, RuleType};

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct ProtocolExtraItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion_control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vmess_security: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vless_encryption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ss_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_preshared_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_interface_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_allowed_ips: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_reserved: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wg_mtu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salamander_pass: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up_mbps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down_mbps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insecure_concurrency: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naive_quic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_child_items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple_load: Option<MultipleLoad>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct TransportExtraItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_header_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhttp_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhttp_extra: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_authority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcp_header_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcp_seed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcp_mtu: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct ProfileItem {
    pub index_id: String,
    pub config_type: ConfigType,
    pub config_version: i32,
    pub subid: String,
    pub is_sub: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_socks_port: Option<i32>,
    pub display_log: bool,
    pub remarks: String,
    pub address: String,
    pub port: i32,
    pub password: String,
    pub username: String,
    pub network: String,
    pub stream_security: String,
    pub allow_insecure: String,
    pub sni: String,
    pub alpn: String,
    pub fingerprint: String,
    pub public_key: String,
    pub short_id: String,
    pub spider_x: String,
    pub mldsa65_verify: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mux_enabled: Option<bool>,
    pub cert: String,
    pub cert_sha: String,
    pub ech_config_list: String,
    pub finalmask: String,
    pub protocol_extra: ProtocolExtraItem,
    pub transport_extra: TransportExtraItem,
}

impl Default for ProfileItem {
    fn default() -> Self {
        Self {
            index_id: String::new(),
            config_type: ConfigType::VMess,
            config_version: 4,
            subid: String::new(),
            is_sub: true,
            pre_socks_port: None,
            display_log: true,
            remarks: String::new(),
            address: String::new(),
            port: 0,
            password: String::new(),
            username: String::new(),
            network: String::new(),
            stream_security: String::new(),
            allow_insecure: String::new(),
            sni: String::new(),
            alpn: String::new(),
            fingerprint: String::new(),
            public_key: String::new(),
            short_id: String::new(),
            spider_x: String::new(),
            mldsa65_verify: String::new(),
            mux_enabled: None,
            cert: String::new(),
            cert_sha: String::new(),
            ech_config_list: String::new(),
            finalmask: String::new(),
            protocol_extra: ProtocolExtraItem::default(),
            transport_extra: TransportExtraItem::default(),
        }
    }
}

impl ProfileItem {
    #[must_use]
    pub fn is_complex(&self) -> bool {
        self.config_type.is_complex_type()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct SubItem {
    pub id: String,
    pub remarks: String,
    pub url: String,
    pub more_url: String,
    pub enabled: bool,
    pub user_agent: String,
    pub sort: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    pub auto_update_interval: i32,
    #[specta(type = f64)]
    pub update_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub convert_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_socks_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
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
            auto_update_interval: 0,
            update_time: 0,
            convert_target: None,
            prev_profile: None,
            next_profile: None,
            pre_socks_port: None,
            memo: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
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
    pub subid: Option<String>,
    pub imported_index_ids: Vec<String>,
    pub updated_index_ids: Vec<String>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct SubscriptionUpdateResult {
    pub updated: u32,
    pub skipped: u32,
    pub imported: u32,
    pub removed_existing: u32,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct RoutingItem {
    #[serde(alias = "id")]
    pub id: String,
    #[serde(alias = "remarks")]
    pub remarks: String,
    #[serde(alias = "url")]
    pub url: String,
    #[serde(alias = "ruleSet")]
    pub rule_set: Vec<RulesItem>,
    #[serde(alias = "ruleNum")]
    pub rule_num: i32,
    #[serde(alias = "enabled")]
    pub enabled: bool,
    #[serde(alias = "locked")]
    pub locked: bool,
    #[serde(alias = "customIcon")]
    pub custom_icon: String,
    #[serde(alias = "customRulesetPath4Singbox")]
    pub custom_ruleset_path4_singbox: String,
    #[serde(alias = "domainStrategy")]
    pub domain_strategy: String,
    #[serde(alias = "domainStrategy4Singbox")]
    pub domain_strategy4_singbox: String,
    #[serde(alias = "sort")]
    pub sort: i32,
    #[serde(alias = "isActive")]
    pub is_active: bool,
}

impl Default for RoutingItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            url: String::new(),
            rule_set: Vec::new(),
            rule_num: 0,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct RulesItem {
    #[serde(alias = "id")]
    pub id: String,
    #[serde(alias = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(alias = "port", skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(alias = "network", skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(alias = "inboundTag", skip_serializing_if = "Option::is_none")]
    pub inbound_tag: Option<Vec<String>>,
    #[serde(alias = "outboundTag", skip_serializing_if = "Option::is_none")]
    pub outbound_tag: Option<String>,
    #[serde(alias = "ip", skip_serializing_if = "Option::is_none")]
    pub ip: Option<Vec<String>>,
    #[serde(alias = "domain", skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,
    #[serde(alias = "protocol", skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Vec<String>>,
    #[serde(alias = "process", skip_serializing_if = "Option::is_none")]
    pub process: Option<Vec<String>>,
    #[serde(alias = "enabled")]
    pub enabled: bool,
    #[serde(alias = "remarks", skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(alias = "ruleType", skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct DnsItem {
    pub id: String,
    pub remarks: String,
    pub enabled: bool,
    pub use_system_hosts: bool,
    #[serde(rename = "NormalDNS", skip_serializing_if = "Option::is_none")]
    pub normal_dns: Option<String>,
    #[serde(rename = "TunDNS", skip_serializing_if = "Option::is_none")]
    pub tun_dns: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_strategy4_freedom: Option<String>,
    #[serde(rename = "DomainDNSAddress", skip_serializing_if = "Option::is_none")]
    pub domain_dns_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct ProfileExItem {
    pub index_id: String,
    pub delay: i32,
    pub speed: f64,
    pub sort: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_info: Option<String>,
}

impl Default for ProfileExItem {
    fn default() -> Self {
        Self {
            index_id: String::new(),
            delay: 0,
            speed: 0.0,
            sort: 0,
            message: None,
            ip_info: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProfileSortKey {
    #[default]
    Sort,
    ConfigType,
    Remarks,
    Address,
    Port,
    Network,
    StreamSecurity,
    Delay,
    Speed,
    IpInfo,
    Subid,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileListItem {
    pub profile: ProfileItem,
    pub profile_ex: ProfileExItem,
    pub server_stat: ServerStatItem,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDedupeResult {
    pub total: u32,
    pub kept: u32,
    pub removed_index_ids: Vec<String>,
}

#[must_use]
pub fn profile_items_match(left: &ProfileItem, right: &ProfileItem, compare_remarks: bool) -> bool {
    left.config_type == right.config_type
        && text_equal(Some(&left.address), Some(&right.address))
        && left.port == right.port
        && text_equal(Some(&left.password), Some(&right.password))
        && text_equal(Some(&left.username), Some(&right.username))
        && text_equal(
            left.protocol_extra.vless_encryption.as_ref(),
            right.protocol_extra.vless_encryption.as_ref(),
        )
        && text_equal(
            left.protocol_extra.ss_method.as_ref(),
            right.protocol_extra.ss_method.as_ref(),
        )
        && text_equal(
            left.protocol_extra.vmess_security.as_ref(),
            right.protocol_extra.vmess_security.as_ref(),
        )
        && text_equal(Some(&left.network), Some(&right.network))
        && text_equal(
            left.transport_extra.raw_header_type.as_ref(),
            right.transport_extra.raw_header_type.as_ref(),
        )
        && text_equal(
            left.transport_extra.host.as_ref(),
            right.transport_extra.host.as_ref(),
        )
        && text_equal(
            left.transport_extra.path.as_ref(),
            right.transport_extra.path.as_ref(),
        )
        && text_equal(
            left.transport_extra.xhttp_mode.as_ref(),
            right.transport_extra.xhttp_mode.as_ref(),
        )
        && text_equal(
            left.transport_extra.xhttp_extra.as_ref(),
            right.transport_extra.xhttp_extra.as_ref(),
        )
        && text_equal(
            left.transport_extra.grpc_authority.as_ref(),
            right.transport_extra.grpc_authority.as_ref(),
        )
        && text_equal(
            left.transport_extra.grpc_service_name.as_ref(),
            right.transport_extra.grpc_service_name.as_ref(),
        )
        && text_equal(
            left.transport_extra.grpc_mode.as_ref(),
            right.transport_extra.grpc_mode.as_ref(),
        )
        && text_equal(
            left.transport_extra.kcp_header_type.as_ref(),
            right.transport_extra.kcp_header_type.as_ref(),
        )
        && text_equal(
            left.transport_extra.kcp_seed.as_ref(),
            right.transport_extra.kcp_seed.as_ref(),
        )
        && (left.config_type == ConfigType::Trojan
            || text_equal(Some(&left.stream_security), Some(&right.stream_security)))
        && text_equal(
            left.protocol_extra.flow.as_ref(),
            right.protocol_extra.flow.as_ref(),
        )
        && text_equal(
            left.protocol_extra.salamander_pass.as_ref(),
            right.protocol_extra.salamander_pass.as_ref(),
        )
        && text_equal(Some(&left.sni), Some(&right.sni))
        && text_equal(Some(&left.alpn), Some(&right.alpn))
        && text_equal(Some(&left.fingerprint), Some(&right.fingerprint))
        && text_equal(Some(&left.public_key), Some(&right.public_key))
        && text_equal(Some(&left.short_id), Some(&right.short_id))
        && text_equal(Some(&left.finalmask), Some(&right.finalmask))
        && (!compare_remarks || left.remarks == right.remarks)
}

fn text_equal(left: Option<&String>, right: Option<&String>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right || (left.is_empty() && right.is_empty()),
        (Some(left), None) => left.is_empty(),
        (None, Some(right)) => right.is_empty(),
        (None, None) => true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct FullConfigTemplateItem {
    pub id: String,
    pub remarks: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tun_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_proxy_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_detour: Option<String>,
}

impl Default for FullConfigTemplateItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            enabled: false,
            config: None,
            tun_config: None,
            add_proxy_only: Some(false),
            proxy_detour: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, Type)]
#[serde(default, rename_all = "PascalCase")]
pub struct ServerStatItem {
    pub index_id: String,
    #[specta(type = f64)]
    pub total_up: i64,
    #[specta(type = f64)]
    pub total_down: i64,
    #[specta(type = f64)]
    pub today_up: i64,
    #[specta(type = f64)]
    pub today_down: i64,
    #[specta(type = f64)]
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

        for obsolete in [
            "HeaderType",
            "RequestHost",
            "Path",
            "Extra",
            "Ports",
            "AlterId",
            "Flow",
            "Id",
            "Security",
        ] {
            assert!(
                !object.contains_key(obsolete),
                "{obsolete} should be absent"
            );
        }

        assert!(object.contains_key("ProtocolExtra"));
        assert!(object.contains_key("TransportExtra"));
        assert!(object.contains_key("Mldsa65Verify"));
        assert!(object.contains_key("Cert"));
        assert!(object.contains_key("CertSha"));
        assert!(object.contains_key("EchConfigList"));
        assert!(object.contains_key("Finalmask"));
        assert!(object.contains_key("SpiderX"));
    }

    #[test]
    fn protocol_extra_serializes_to_compact_pascal_case_blob() {
        let extra = ProtocolExtraItem {
            ss_method: Some("2022-blake3-aes-256-gcm".to_string()),
            multiple_load: Some(MultipleLoad::LeastLoad),
            ..ProtocolExtraItem::default()
        };

        assert_eq!(
            serde_json::to_string(&extra).expect("protocol extra should serialize to compact JSON"),
            r#"{"SsMethod":"2022-blake3-aes-256-gcm","MultipleLoad":4}"#
        );
    }

    #[test]
    fn profile_items_match_uses_v2rayn_dedupe_fields() {
        let base = ProfileItem {
            config_type: ConfigType::VLESS,
            remarks: "one".to_string(),
            address: "example.com".to_string(),
            port: 443,
            password: "uuid".to_string(),
            network: "ws".to_string(),
            stream_security: "tls".to_string(),
            sni: "example.com".to_string(),
            protocol_extra: ProtocolExtraItem {
                flow: Some("xtls-rprx-vision".to_string()),
                vless_encryption: Some("none".to_string()),
                ..ProtocolExtraItem::default()
            },
            transport_extra: TransportExtraItem {
                host: Some("example.com".to_string()),
                path: Some("/ws".to_string()),
                ..TransportExtraItem::default()
            },
            ..ProfileItem::default()
        };
        let mut duplicate = base.clone();
        duplicate.index_id = "other".to_string();
        duplicate.remarks = "renamed".to_string();

        assert!(profile_items_match(&base, &duplicate, false));
        assert!(!profile_items_match(&base, &duplicate, true));

        duplicate.transport_extra.path = Some("/other".to_string());
        assert!(!profile_items_match(&base, &duplicate, false));
    }
}
