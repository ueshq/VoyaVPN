use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use voya_core::{
    CoreType, DnsItem, SimpleDnsItem, SingboxDns, DEFAULT_BOOTSTRAP_DNS, DEFAULT_DIRECT_DNS,
    DEFAULT_REMOTE_DNS, DEFAULT_SINGBOX_DNS_NORMAL, DEFAULT_XRAY_DNS_NORMAL,
};
use voya_db::{Database, DbError};

static DNS_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub type Result<T> = std::result::Result<T, DnsManagerError>;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DnsSettings {
    pub simple_dns_item: SimpleDnsItem,
    pub xray_dns_item: DnsItem,
    pub singbox_dns_item: DnsItem,
    pub defaults: DnsSettingsDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DnsSettingsDefaults {
    pub xray_normal_dns: String,
    pub xray_tun_dns: String,
    pub singbox_normal_dns: String,
    pub singbox_tun_dns: String,
}

impl Default for DnsSettingsDefaults {
    fn default() -> Self {
        Self {
            xray_normal_dns: DEFAULT_XRAY_DNS_NORMAL.to_string(),
            xray_tun_dns: DEFAULT_XRAY_DNS_NORMAL.to_string(),
            singbox_normal_dns: DEFAULT_SINGBOX_DNS_NORMAL.to_string(),
            singbox_tun_dns: DEFAULT_SINGBOX_DNS_NORMAL.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DnsValidationIssue {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum DnsManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("DNS settings validation failed")]
    Validation(Vec<DnsValidationIssue>),
    #[error("DNS item for {0:?} is required")]
    MissingCoreDnsItem(CoreType),
}

#[derive(Debug, Clone, Copy)]
pub struct DnsManager<'db> {
    database: &'db Database,
}

impl<'db> DnsManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self { database }
    }

    pub async fn load_settings(&self, simple_dns_item: &SimpleDnsItem) -> Result<DnsSettings> {
        let xray_dns_item = self.ensure_core_dns_item(CoreType::Xray).await?;
        let singbox_dns_item = self.ensure_core_dns_item(CoreType::sing_box).await?;

        Ok(DnsSettings {
            simple_dns_item: normalize_simple_dns(simple_dns_item.clone()),
            xray_dns_item,
            singbox_dns_item,
            defaults: DnsSettingsDefaults::default(),
        })
    }

    pub async fn save_settings(&self, mut settings: DnsSettings) -> Result<DnsSettings> {
        settings.simple_dns_item = normalize_simple_dns(settings.simple_dns_item);
        normalize_dns_item(&mut settings.xray_dns_item, CoreType::Xray);
        normalize_dns_item(&mut settings.singbox_dns_item, CoreType::sing_box);

        validate_settings(&settings)?;

        self.database.dns().upsert(&settings.xray_dns_item).await?;
        self.database
            .dns()
            .upsert(&settings.singbox_dns_item)
            .await?;

        Ok(DnsSettings {
            defaults: DnsSettingsDefaults::default(),
            ..settings
        })
    }

    async fn ensure_core_dns_item(&self, core_type: CoreType) -> Result<DnsItem> {
        if let Some(mut item) = self.database.dns().get_by_core_type(core_type).await? {
            normalize_dns_item(&mut item, core_type);
            if item.enabled && item.normal_dns.as_deref().is_none_or(str::is_empty) {
                item.enabled = false;
                self.database.dns().upsert(&item).await?;
            }
            return Ok(item);
        }

        let item = default_dns_item(core_type);
        self.database.dns().upsert(&item).await?;

        Ok(item)
    }
}

#[must_use]
pub fn default_dns_item(core_type: CoreType) -> DnsItem {
    DnsItem {
        id: generate_dns_id(),
        remarks: match core_type {
            CoreType::Xray => "Xray".to_string(),
            CoreType::sing_box => "sing-box".to_string(),
            _ => format!("{core_type:?}"),
        },
        core_type,
        enabled: false,
        ..DnsItem::default()
    }
}

fn normalize_simple_dns(mut item: SimpleDnsItem) -> SimpleDnsItem {
    let defaults = SimpleDnsItem::default();
    item.use_system_hosts = item.use_system_hosts.or(defaults.use_system_hosts);
    item.add_common_hosts = item.add_common_hosts.or(defaults.add_common_hosts);
    item.fake_ip = item.fake_ip.or(defaults.fake_ip);
    item.global_fake_ip = item.global_fake_ip.or(defaults.global_fake_ip);
    item.block_binding_query = item.block_binding_query.or(defaults.block_binding_query);
    item.direct_dns =
        clean_optional_string(item.direct_dns).or_else(|| Some(DEFAULT_DIRECT_DNS.to_string()));
    item.remote_dns =
        clean_optional_string(item.remote_dns).or_else(|| Some(DEFAULT_REMOTE_DNS.to_string()));
    item.bootstrap_dns = clean_optional_string(item.bootstrap_dns)
        .or_else(|| Some(DEFAULT_BOOTSTRAP_DNS.to_string()));
    item.strategy4_freedom = clean_optional_string(item.strategy4_freedom);
    item.strategy4_proxy = clean_optional_string(item.strategy4_proxy);
    item.serve_stale = item.serve_stale.or(defaults.serve_stale);
    item.parallel_query = item.parallel_query.or(defaults.parallel_query);
    item.hosts = clean_optional_string(item.hosts);
    item.direct_expected_ips = clean_optional_string(item.direct_expected_ips);
    item
}

fn normalize_dns_item(item: &mut DnsItem, core_type: CoreType) {
    if item.id.trim().is_empty() {
        item.id = generate_dns_id();
    }
    if item.remarks.trim().is_empty() {
        item.remarks = match core_type {
            CoreType::Xray => "Xray".to_string(),
            CoreType::sing_box => "sing-box".to_string(),
            _ => format!("{core_type:?}"),
        };
    } else {
        item.remarks = item.remarks.trim().to_string();
    }
    item.core_type = core_type;
    item.normal_dns = clean_optional_string(item.normal_dns.take());
    item.tun_dns = clean_optional_string(item.tun_dns.take());
    item.domain_strategy4_freedom = clean_optional_string(item.domain_strategy4_freedom.take());
    item.domain_dns_address = clean_optional_string(item.domain_dns_address.take());
}

fn validate_settings(settings: &DnsSettings) -> Result<()> {
    let mut issues = Vec::new();

    validate_hosts(
        settings.simple_dns_item.hosts.as_deref(),
        "simpleDnsItem.hosts",
        &mut issues,
    );
    validate_expected_ips(
        settings.simple_dns_item.direct_expected_ips.as_deref(),
        "simpleDnsItem.directExpectedIPs",
        &mut issues,
    );
    validate_xray_dns_json(
        settings.xray_dns_item.normal_dns.as_deref(),
        "xrayDnsItem.normalDNS",
        &mut issues,
    );
    validate_xray_dns_json(
        settings.xray_dns_item.tun_dns.as_deref(),
        "xrayDnsItem.tunDNS",
        &mut issues,
    );
    validate_singbox_dns_json(
        settings.singbox_dns_item.normal_dns.as_deref(),
        "singboxDnsItem.normalDNS",
        &mut issues,
    );
    validate_singbox_dns_json(
        settings.singbox_dns_item.tun_dns.as_deref(),
        "singboxDnsItem.tunDNS",
        &mut issues,
    );

    if issues.is_empty() {
        Ok(())
    } else {
        Err(DnsManagerError::Validation(issues))
    }
}

fn validate_xray_dns_json(value: Option<&str>, field: &str, issues: &mut Vec<DnsValidationIssue>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(json) => {
            if json.get("servers").is_none() {
                issues.push(issue(field, "Xray DNS JSON must contain a servers field"));
            }
        }
        Err(error) => {
            if value.contains('{') || value.contains('}') || value.starts_with('[') {
                issues.push(issue(field, format!("Invalid Xray DNS JSON: {error}")));
            }
        }
    }
}

fn validate_singbox_dns_json(
    value: Option<&str>,
    field: &str,
    issues: &mut Vec<DnsValidationIssue>,
) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(raw) => {
            let Some(servers) = raw.get("servers").and_then(serde_json::Value::as_array) else {
                issues.push(issue(
                    field,
                    "sing-box DNS JSON must contain at least one server",
                ));
                return;
            };
            if servers.is_empty() {
                issues.push(issue(
                    field,
                    "sing-box DNS JSON must contain at least one server",
                ));
            }
            if servers.iter().any(|server| {
                server
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(|value| value.trim().is_empty())
            }) {
                issues.push(issue(
                    field,
                    "Every sing-box DNS server must include a non-empty type",
                ));
            }
            if serde_json::from_value::<SingboxDns>(raw).is_err() {
                issues.push(issue(field, "Invalid sing-box DNS typed server schema"));
            }
        }
        Err(error) => {
            issues.push(issue(field, format!("Invalid sing-box DNS JSON: {error}")));
        }
    }
}

fn validate_hosts(value: Option<&str>, field: &str, issues: &mut Vec<DnsValidationIssue>) {
    let Some(value) = value else {
        return;
    };
    for (index, line) in value.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.split_whitespace().count() < 2 {
            issues.push(issue(
                field,
                format!(
                    "Host line {} must contain a domain and at least one answer",
                    index + 1
                ),
            ));
        }
    }
}

fn validate_expected_ips(value: Option<&str>, field: &str, issues: &mut Vec<DnsValidationIssue>) {
    let Some(value) = value else {
        return;
    };
    if value
        .split(',')
        .map(str::trim)
        .any(|part| !part.is_empty() && part.chars().any(char::is_whitespace))
    {
        issues.push(issue(
            field,
            "Expected IPs must be comma-separated without embedded whitespace",
        ));
    }
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn issue(field: &str, message: impl Into<String>) -> DnsValidationIssue {
    DnsValidationIssue {
        field: field.to_string(),
        message: message.into(),
    }
}

fn generate_dns_id() -> String {
    let counter = DNS_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());

    format!("dns-{millis}-{counter}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dns_manager_persists_simple_and_raw_dns_settings() {
        let database = Database::connect_in_memory()
            .await
            .expect("DNS manager test operation should succeed");
        let manager = DnsManager::new(&database);
        let mut settings = manager
            .load_settings(&SimpleDnsItem::default())
            .await
            .expect("DNS manager test operation should succeed");

        settings.simple_dns_item.fake_ip = Some(true);
        settings.simple_dns_item.global_fake_ip = Some(false);
        settings.simple_dns_item.direct_expected_ips = Some("geoip:cn,192.0.2.0/24".to_string());
        settings.simple_dns_item.hosts = Some("example.test 192.0.2.1".to_string());
        settings.xray_dns_item.enabled = true;
        settings.xray_dns_item.normal_dns = Some(r#"{"servers":["1.1.1.1"]}"#.to_string());
        settings.singbox_dns_item.enabled = true;
        settings.singbox_dns_item.normal_dns =
            Some(r#"{"servers":[{"tag":"remote","type":"udp","server":"1.1.1.1"}]}"#.to_string());

        let saved = manager
            .save_settings(settings.clone())
            .await
            .expect("DNS manager test operation should succeed");
        assert_eq!(saved.simple_dns_item.fake_ip, Some(true));
        assert_eq!(
            database
                .dns()
                .get_by_core_type(CoreType::Xray)
                .await
                .expect("DNS manager test operation should succeed")
                .expect("DNS manager test operation should succeed")
                .normal_dns,
            settings.xray_dns_item.normal_dns
        );
        assert_eq!(
            database
                .dns()
                .get_by_core_type(CoreType::sing_box)
                .await
                .expect("DNS manager test operation should succeed")
                .expect("DNS manager test operation should succeed")
                .normal_dns,
            settings.singbox_dns_item.normal_dns
        );
    }

    #[tokio::test]
    async fn dns_manager_returns_typed_validation_errors_for_invalid_json() {
        let database = Database::connect_in_memory()
            .await
            .expect("DNS manager test operation should succeed");
        let manager = DnsManager::new(&database);
        let mut settings = manager
            .load_settings(&SimpleDnsItem::default())
            .await
            .expect("DNS manager test operation should succeed");
        settings.xray_dns_item.normal_dns = Some(r#"{"servers":"#.to_string());
        settings.singbox_dns_item.normal_dns =
            Some(r#"{"servers":[{"tag":"remote"}]}"#.to_string());

        let error = manager
            .save_settings(settings)
            .await
            .expect_err("invalid DNS JSON should fail validation");
        let DnsManagerError::Validation(issues) = error else {
            panic!("expected validation errors");
        };
        assert!(issues
            .iter()
            .any(|issue| issue.field == "xrayDnsItem.normalDNS"));
        assert!(issues
            .iter()
            .any(|issue| issue.field == "singboxDnsItem.normalDNS"));
    }
}
