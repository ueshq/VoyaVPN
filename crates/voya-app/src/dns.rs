use thiserror::Error;
use voya_contracts::ValidationCode;
use voya_contracts::ValidationIssue;
use voya_core::{
    first_dns_address, text::nonempty_string, SimpleDnsItem, DEFAULT_BOOTSTRAP_DNS,
    DEFAULT_DIRECT_DNS, DEFAULT_REMOTE_DNS,
};

pub type Result<T> = std::result::Result<T, DnsSettingsError>;

#[derive(Debug, Error)]
pub enum DnsSettingsError {
    #[error("DNS settings validation failed")]
    Validation(Vec<ValidationIssue>),
}

/// A submitted DNS form, normalized and checked the way it will be stored.
pub fn validated_settings(item: SimpleDnsItem) -> Result<SimpleDnsItem> {
    let item = normalize_simple_dns(item);
    validate_settings(&item)?;
    Ok(item)
}

#[must_use]
pub fn normalize_simple_dns(mut item: SimpleDnsItem) -> SimpleDnsItem {
    let defaults = SimpleDnsItem::default();
    item.add_common_hosts = item.add_common_hosts.or(defaults.add_common_hosts);
    item.fake_ip = item.fake_ip.or(defaults.fake_ip);
    item.global_fake_ip = item.global_fake_ip.or(defaults.global_fake_ip);
    item.block_binding_query = item.block_binding_query.or(defaults.block_binding_query);
    item.direct_dns = nonempty_string(item.direct_dns.as_deref())
        .or_else(|| Some(DEFAULT_DIRECT_DNS.to_string()));
    item.remote_dns = nonempty_string(item.remote_dns.as_deref())
        .or_else(|| Some(DEFAULT_REMOTE_DNS.to_string()));
    item.bootstrap_dns = nonempty_string(item.bootstrap_dns.as_deref())
        .or_else(|| Some(DEFAULT_BOOTSTRAP_DNS.to_string()));
    item.strategy4_freedom = nonempty_string(item.strategy4_freedom.as_deref());
    item.strategy4_proxy = nonempty_string(item.strategy4_proxy.as_deref());
    item.hosts = nonempty_string(item.hosts.as_deref());
    item.direct_expected_ips = nonempty_string(item.direct_expected_ips.as_deref());
    item
}

fn validate_settings(item: &SimpleDnsItem) -> Result<()> {
    let mut issues = Vec::new();
    validate_hosts(item.hosts.as_deref(), "hosts", &mut issues);
    validate_expected_ips(
        item.direct_expected_ips.as_deref(),
        "directExpectedIps",
        &mut issues,
    );
    for (value, field) in [
        (&item.direct_dns, "direct"),
        (&item.remote_dns, "remote"),
        (&item.bootstrap_dns, "bootstrap"),
    ] {
        validate_dns_address(value.as_deref(), field, &mut issues);
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(DnsSettingsError::Validation(issues))
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
    // Only the entry generation uses: validating the rest of a list would
    // reject values the core never looks at.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_normalizes_and_validates_simple_dns_only() {
        let settings = validated_settings(SimpleDnsItem {
            hosts: Some("example.test 192.0.2.1".to_string()),
            direct_dns: Some(" 1.1.1.1 ".to_string()),
            ..SimpleDnsItem::default()
        })
        .expect("valid DNS settings should save");

        assert_eq!(settings.direct_dns.as_deref(), Some("1.1.1.1"));
    }

    fn validation_fields(item: SimpleDnsItem) -> Vec<String> {
        match validate_settings(&normalize_simple_dns(item)) {
            Ok(()) => Vec::new(),
            Err(DnsSettingsError::Validation(issues)) => {
                issues.into_iter().map(|issue| issue.field).collect()
            }
        }
    }

    #[test]
    fn resolver_addresses_the_core_would_discard_are_rejected() {
        assert_eq!(
            validation_fields(SimpleDnsItem {
                direct_dns: Some("1.1.1.1:70000".to_string()),
                remote_dns: Some("https://dns.example.test:0/dns-query".to_string()),
                bootstrap_dns: Some("8.8.8.8:dns".to_string()),
                ..SimpleDnsItem::default()
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
                validation_fields(SimpleDnsItem {
                    direct_dns: Some(address.to_string()),
                    ..SimpleDnsItem::default()
                }),
                Vec::<String>::new(),
                "{address} should be accepted"
            );
        }
    }
}
