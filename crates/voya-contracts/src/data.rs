use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Subscription {
    pub id: String,
    pub remarks: String,
    pub url: String,
    pub additional_url: String,
    pub enabled: bool,
    pub user_agent: String,
    pub sort: i32,
    pub filter: Option<String>,
    pub converter_target: Option<String>,
    pub auto_update_interval_minutes: Option<i32>,
}

/// Usage metadata reported by the subscription server on the latest successful
/// fetch (`subscription-userinfo` and `profile-title` response headers).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubscriptionMetadata {
    pub subscription_id: String,
    #[specta(type = Option<f64>)]
    pub upload_bytes: Option<i64>,
    #[specta(type = Option<f64>)]
    pub download_bytes: Option<i64>,
    #[specta(type = Option<f64>)]
    pub total_bytes: Option<i64>,
    #[specta(type = Option<f64>)]
    pub expire_at: Option<i64>,
    #[specta(type = Option<f64>)]
    pub last_update_at: Option<i64>,
    pub profile_title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    pub imported_profile_ids: Vec<String>,
    pub updated_profile_ids: Vec<String>,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubscriptionUpdateResult {
    pub updated: u32,
    pub skipped: u32,
    pub imported: u32,
    pub removed_existing: u32,
    pub messages: Vec<String>,
}

impl Default for Subscription {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            url: String::new(),
            additional_url: String::new(),
            enabled: true,
            user_agent: String::new(),
            sort: 0,
            filter: None,
            converter_target: None,
            auto_update_interval_minutes: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProcessCandidateSource {
    RunningProcess,
    InstalledApplication,
}

/// A running process or installed application offered by the per-app proxy
/// picker; `process_name` is the executable basename sing-box matches on.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessCandidate {
    pub display_name: String,
    pub process_name: String,
    pub executable_path: Option<String>,
    pub source: ProcessCandidateSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RoutingRuleScope {
    All,
    #[default]
    Routing,
    Dns,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingRule {
    pub id: String,
    pub kind: Option<String>,
    pub port: Option<String>,
    pub network: Option<String>,
    pub inbound_tags: Option<Vec<String>>,
    pub outbound: Option<String>,
    pub ip: Option<Vec<String>>,
    pub domain: Option<Vec<String>>,
    pub protocol: Option<Vec<String>>,
    pub process: Option<Vec<String>>,
    pub enabled: bool,
    pub remarks: Option<String>,
    pub scope: Option<RoutingRuleScope>,
}

impl Default for RoutingRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: None,
            port: None,
            network: None,
            inbound_tags: None,
            outbound: None,
            ip: None,
            domain: None,
            protocol: None,
            process: None,
            enabled: true,
            remarks: None,
            scope: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Routing {
    pub id: String,
    pub remarks: String,
    pub rules: Vec<RoutingRule>,
    pub enabled: bool,
    pub locked: bool,
    pub icon: String,
    pub singbox_ruleset_path: String,
    pub singbox_domain_strategy: String,
    pub sort: i32,
    #[serde(default, skip_deserializing)]
    pub is_active: bool,
}

impl Default for Routing {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            rules: Vec::new(),
            enabled: true,
            locked: false,
            icon: String::new(),
            singbox_ruleset_path: String::new(),
            singbox_domain_strategy: String::new(),
            sort: 0,
            is_active: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MoveAction {
    Top,
    Up,
    Down,
    Bottom,
    Position,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DnsSettings {
    pub add_common_hosts: Option<bool>,
    pub fake_ip: Option<bool>,
    pub global_fake_ip: Option<bool>,
    pub block_binding_query: Option<bool>,
    pub direct: Option<String>,
    pub remote: Option<String>,
    pub bootstrap: Option<String>,
    pub direct_strategy: Option<String>,
    pub proxy_strategy: Option<String>,
    pub hosts: Option<String>,
    pub direct_expected_ips: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_data_contracts_reject_pascal_case() {
        assert!(serde_json::from_str::<Subscription>(r#"{"Id":"legacy"}"#).is_err());
        assert!(serde_json::from_str::<Routing>(r#"{"Id":"legacy"}"#).is_err());
        assert!(serde_json::from_str::<DnsSettings>(r#"{"RemoteDNS":"1.1.1.1"}"#).is_err());
    }
}
