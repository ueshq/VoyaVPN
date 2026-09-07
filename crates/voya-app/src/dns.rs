use thiserror::Error;
use voya_contracts::ValidationCode;
pub use voya_contracts::ValidationIssue;
use voya_core::{SimpleDnsItem, DEFAULT_BOOTSTRAP_DNS, DEFAULT_DIRECT_DNS, DEFAULT_REMOTE_DNS};
use voya_db::{Database, DatabaseSession, UnitOfWork};

pub type Result<T> = std::result::Result<T, DnsManagerError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsSettings {
    pub simple_dns_item: SimpleDnsItem,
}

#[derive(Debug, Error)]
pub enum DnsManagerError {
    #[error("DNS settings validation failed")]
    Validation(Vec<ValidationIssue>),
}

#[derive(Debug, Clone, Copy)]
pub struct DnsManager<'db> {
    _database: DatabaseSession<'db>,
}

impl<'db> DnsManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self {
            _database: DatabaseSession::from_database(database),
        }
    }

    #[must_use]
    pub fn new_in(unit_of_work: &'db UnitOfWork) -> Self {
        Self {
            _database: DatabaseSession::from_unit_of_work(unit_of_work),
        }
    }

    pub async fn load_settings(&self, simple_dns_item: &SimpleDnsItem) -> Result<DnsSettings> {
        Ok(DnsSettings {
            simple_dns_item: normalize_simple_dns(simple_dns_item.clone()),
        })
    }

    pub async fn save_settings(&self, mut settings: DnsSettings) -> Result<DnsSettings> {
        settings.simple_dns_item = normalize_simple_dns(settings.simple_dns_item);
        validate_settings(&settings)?;
        Ok(settings)
    }
}

#[must_use]
pub fn normalize_simple_dns(mut item: SimpleDnsItem) -> SimpleDnsItem {
    let defaults = SimpleDnsItem::default();
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
    item.hosts = clean_optional_string(item.hosts);
    item.direct_expected_ips = clean_optional_string(item.direct_expected_ips);
    item
}

pub fn validate_settings(settings: &DnsSettings) -> Result<()> {
    let mut issues = Vec::new();
    validate_hosts(
        settings.simple_dns_item.hosts.as_deref(),
        "hosts",
        &mut issues,
    );
    validate_expected_ips(
        settings.simple_dns_item.direct_expected_ips.as_deref(),
        "directExpectedIps",
        &mut issues,
    );
    for (value, field) in [
        (&settings.simple_dns_item.direct_dns, "direct"),
        (&settings.simple_dns_item.remote_dns, "remote"),
        (&settings.simple_dns_item.bootstrap_dns, "bootstrap"),
    ] {
        validate_dns_address(value.as_deref(), field, &mut issues);
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(DnsManagerError::Validation(issues))
    }
}

/// Rejects a resolver address that config generation would silently discard.
///
/// `voya_core` resolves each address with
/// `parse_dns_address(value).or_else(|| parse_dns_address(default))`, so an
/// address it cannot parse is replaced by the built-in resolver without any
/// signal: the form reports success, the settings screen keeps showing the
/// typed value, and the running core queries a different server. The check is
/// deliberately conservative — it only reports what the generator is certain to
/// reject — so it can never refuse an address that would have worked.
fn validate_dns_address(value: Option<&str>, field: &str, issues: &mut Vec<ValidationIssue>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    let Some(address) = first_dns_address(value) else {
        issues.push(ValidationIssue::new(field, ValidationCode::DnsAddressEmpty));
        return;
    };
    if matches!(address, "local" | "localhost") {
        return;
    }
    if let Some(port) = dns_address_port(address) {
        if port.parse::<u16>().ok().filter(|port| *port > 0).is_none() {
            issues.push(ValidationIssue::new(
                field,
                ValidationCode::DnsAddressPort {
                    port: port.to_string(),
                },
            ));
        }
    }
}

/// The entry config generation actually uses: `voya_core` keeps only the first
/// item of a comma- or semicolon-separated address list, so validating the rest
/// would reject values the core never looks at.
fn first_dns_address(address: &str) -> Option<&str> {
    let delimiter = if address.contains(',') { ',' } else { ';' };
    address
        .split(delimiter)
        .map(str::trim)
        .find(|item| !item.is_empty())
}

/// The explicit port of a DNS address, when it carries one. Mirrors the
/// authority parsing in `voya_core::singbox`: the scheme and path are dropped,
/// userinfo is ignored, a bracketed IPv6 host keeps its own colons, and a bare
/// IPv6 literal has no port at all.
fn dns_address_port(address: &str) -> Option<&str> {
    let after_scheme = address.split_once("://").map_or(address, |(_, rest)| rest);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, authority)| authority);
    if let Some(closing_bracket) = authority.rfind(']') {
        return authority.get(closing_bracket + 1..)?.strip_prefix(':');
    }
    let (host, port) = authority.rsplit_once(':')?;
    (!host.contains(':')).then_some(port)
}

fn validate_hosts(value: Option<&str>, field: &str, issues: &mut Vec<ValidationIssue>) {
    let Some(value) = value else {
        return;
    };
    for (index, line) in value.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.split_whitespace().count() < 2 {
            issues.push(ValidationIssue::new(
                field,
                ValidationCode::DnsHostsLine {
                    line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                },
            ));
        }
    }
}

fn validate_expected_ips(value: Option<&str>, field: &str, issues: &mut Vec<ValidationIssue>) {
    let Some(value) = value else {
        return;
    };
    if value
        .split(',')
        .map(str::trim)
        .any(|part| !part.is_empty() && part.chars().any(char::is_whitespace))
    {
        issues.push(ValidationIssue::new(field, ValidationCode::DnsExpectedIps));
    }
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dns_manager_normalizes_and_validates_simple_dns_only() {
        let database = Database::connect_in_memory()
            .await
            .expect("DNS manager test operation should succeed");
        let manager = DnsManager::new(&database);
        let settings = manager
            .save_settings(DnsSettings {
                simple_dns_item: SimpleDnsItem {
                    hosts: Some("example.test 192.0.2.1".to_string()),
                    direct_dns: Some(" 1.1.1.1 ".to_string()),
                    ..SimpleDnsItem::default()
                },
            })
            .await
            .expect("DNS manager test operation should succeed");

        assert_eq!(
            settings.simple_dns_item.direct_dns.as_deref(),
            Some("1.1.1.1")
        );
    }

    fn validation_fields(settings: DnsSettings) -> Vec<String> {
        match validate_settings(&settings) {
            Ok(()) => Vec::new(),
            Err(DnsManagerError::Validation(issues)) => {
                issues.into_iter().map(|issue| issue.field).collect()
            }
        }
    }

    #[test]
    fn resolver_addresses_the_core_would_discard_are_rejected() {
        assert_eq!(
            validation_fields(DnsSettings {
                simple_dns_item: normalize_simple_dns(SimpleDnsItem {
                    direct_dns: Some("1.1.1.1:70000".to_string()),
                    remote_dns: Some("https://dns.example.test:0/dns-query".to_string()),
                    bootstrap_dns: Some("8.8.8.8:dns".to_string()),
                    ..SimpleDnsItem::default()
                }),
            }),
            vec![
                "direct".to_string(),
                "remote".to_string(),
                "bootstrap".to_string(),
            ]
        );
    }

    #[test]
    fn resolver_addresses_the_core_accepts_stay_valid() {
        for address in [
            "119.29.29.29",
            "1.1.1.1:5353",
            "https://cloudflare-dns.com/dns-query",
            "tls://8.8.8.8",
            "local",
            "2001:4860:4860::8888",
            "[2001:4860:4860::8888]:53",
            "dhcp://auto",
            // Only the first entry reaches the generated config.
            "1.1.1.1, 8.8.8.8",
        ] {
            assert_eq!(
                validation_fields(DnsSettings {
                    simple_dns_item: normalize_simple_dns(SimpleDnsItem {
                        direct_dns: Some(address.to_string()),
                        ..SimpleDnsItem::default()
                    }),
                }),
                Vec::<String>::new(),
                "{address} should be accepted"
            );
        }
    }
}
