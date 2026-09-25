//! From the stored record to what the core runs and what peers import.
//!
//! Pure functions: the core's [`SelfHostSpec`] and the share links are built
//! from the same record, so a link can never describe an inbound the running
//! config does not have.

use std::net::IpAddr;

use voya_contracts::{
    SelfHostAddressKind, SelfHostConfig, SelfHostEnvironmentReport, SelfHostProtocol,
    SelfHostRecord, SelfHostShareLink, ValidationCode, ValidationIssue,
};
use voya_core::{
    export_share_link_with_options, host::is_valid_host, selfhost_share_profiles, ConfigType,
    SelfHostClashApi, SelfHostEndpoint, SelfHostShadowsocksSpec, SelfHostSpec, SelfHostVlessSpec,
    ShareLinkOptions, REALITY_FALLBACK_FINGERPRINT,
};

use super::identity::reality_public_key;

/// Ports below 1024 need root on macOS and Linux; the node never runs as root.
pub const SELF_HOST_MIN_PORT: u16 = 1024;
const DEVICE_LABEL_MAX_CHARS: usize = 64;
const DEFAULT_DEVICE_LABEL: &str = "VoyaVPN";

/// Every rejection in `config`, addressed by field name. Port `0` passes: it
/// means "pick one when the node is first enabled".
pub fn validate_self_host_config(config: &SelfHostConfig) -> Result<(), Vec<ValidationIssue>> {
    let mut issues = Vec::new();
    let mut push = |field: &str, code: ValidationCode| {
        issues.push(ValidationIssue {
            field: field.to_string(),
            code,
            scope: Vec::new(),
        });
    };

    if !config.vless_enabled && !config.shadowsocks_enabled {
        push("protocols", ValidationCode::SelfHostNoProtocol);
    }
    for (field, port) in [
        ("vlessPort", config.vless_port),
        ("shadowsocksPort", config.shadowsocks_port),
    ] {
        if port != 0 && port < SELF_HOST_MIN_PORT {
            push(
                field,
                ValidationCode::SelfHostPortOutOfRange {
                    min: u32::from(SELF_HOST_MIN_PORT),
                    max: u32::from(u16::MAX),
                },
            );
        }
    }
    if config.vless_enabled
        && config.shadowsocks_enabled
        && config.vless_port != 0
        && config.vless_port == config.shadowsocks_port
    {
        push("shadowsocksPort", ValidationCode::SelfHostPortsCollide);
    }
    let server_name = config.reality_server_name.trim();
    if server_name.is_empty()
        || server_name.parse::<IpAddr>().is_ok()
        || !is_valid_host(server_name)
    {
        push("realityServerName", ValidationCode::InvalidAddress);
    }
    if config.reality_server_port == 0 {
        push("realityServerPort", ValidationCode::InvalidPort);
    }
    if let Some(address) = config.custom_address.as_deref() {
        if !address.trim().is_empty() && !is_valid_host(address) {
            push("customAddress", ValidationCode::InvalidAddress);
        }
    }
    let label = config.device_label.trim();
    if label.chars().count() > DEVICE_LABEL_MAX_CHARS {
        push("deviceLabel", ValidationCode::TextTooLong);
    } else if label.chars().any(char::is_control) {
        push("deviceLabel", ValidationCode::TextControlCharacters);
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// Trims what the user typed into the stored shape.
pub(super) fn normalized_config(mut config: SelfHostConfig) -> SelfHostConfig {
    config.device_label = config.device_label.trim().to_string();
    config.reality_server_name = config.reality_server_name.trim().to_ascii_lowercase();
    config.custom_address = config
        .custom_address
        .map(|address| address.trim().trim_matches(['[', ']']).to_string())
        .filter(|address| !address.is_empty());
    config
}

/// Whether `after` changes anything the running core was generated from.
/// The label, the custom address and UPnP only change links or the router.
pub(super) fn core_settings_changed(before: &SelfHostConfig, after: &SelfHostConfig) -> bool {
    (
        before.vless_enabled,
        before.vless_port,
        before.shadowsocks_enabled,
        before.shadowsocks_port,
        &before.reality_server_name,
        before.reality_server_port,
        before.allow_lan_access,
        before.block_bittorrent,
    ) != (
        after.vless_enabled,
        after.vless_port,
        after.shadowsocks_enabled,
        after.shadowsocks_port,
        &after.reality_server_name,
        after.reality_server_port,
        after.allow_lan_access,
        after.block_bittorrent,
    )
}

/// The ports the node listens on, in inbound order.
pub(super) fn enabled_ports(config: &SelfHostConfig) -> Vec<u16> {
    let mut ports = Vec::new();
    if config.vless_enabled && config.vless_port != 0 {
        ports.push(config.vless_port);
    }
    if config.shadowsocks_enabled && config.shadowsocks_port != 0 {
        ports.push(config.shadowsocks_port);
    }
    ports
}

/// The core config for `record`, or `None` before credentials and ports exist.
pub(super) fn selfhost_spec(
    record: &SelfHostRecord,
    clash_api: Option<SelfHostClashApi>,
    log_level: &str,
) -> Option<SelfHostSpec> {
    let credentials = record.credentials.as_ref()?;
    let config = &record.config;
    let vless = (config.vless_enabled && config.vless_port != 0).then(|| SelfHostVlessSpec {
        port: i32::from(config.vless_port),
        uuid: credentials.vless_uuid.clone(),
        reality_private_key: credentials.reality_private_key.clone(),
        reality_short_id: credentials.reality_short_id.clone(),
        reality_server_name: config.reality_server_name.clone(),
        reality_server_port: i32::from(config.reality_server_port),
    });
    let shadowsocks = (config.shadowsocks_enabled && config.shadowsocks_port != 0).then(|| {
        SelfHostShadowsocksSpec {
            port: i32::from(config.shadowsocks_port),
            password: credentials.shadowsocks_password.clone(),
        }
    });
    Some(SelfHostSpec {
        vless,
        shadowsocks,
        allow_lan_access: config.allow_lan_access,
        block_bittorrent: config.block_bittorrent,
        clash_api,
        log_level: log_level.to_string(),
    })
}

/// The addresses links point at: the custom address alone when one is set,
/// otherwise every public address the last network check learned.
pub(super) fn link_endpoints(
    config: &SelfHostConfig,
    environment: Option<&SelfHostEnvironmentReport>,
) -> Vec<(SelfHostAddressKind, SelfHostEndpoint)> {
    if let Some(address) = config.custom_address.as_deref() {
        return vec![(
            SelfHostAddressKind::Custom,
            SelfHostEndpoint {
                address: address.to_string(),
                label: address.to_string(),
            },
        )];
    }
    let Some(environment) = environment else {
        return Vec::new();
    };
    [
        (
            SelfHostAddressKind::Ipv4,
            "IPv4",
            &environment.ipv4.public_address,
        ),
        (
            SelfHostAddressKind::Ipv6,
            "IPv6",
            &environment.ipv6.public_address,
        ),
    ]
    .into_iter()
    .filter_map(|(kind, label, address)| {
        address.as_ref().map(|address| {
            (
                kind,
                SelfHostEndpoint {
                    address: address.clone(),
                    label: label.to_string(),
                },
            )
        })
    })
    .collect()
}

/// One link per enabled protocol per usable address.
pub(super) fn share_links(
    record: &SelfHostRecord,
    environment: Option<&SelfHostEnvironmentReport>,
) -> Vec<SelfHostShareLink> {
    let Some(spec) = selfhost_spec(record, None, "") else {
        return Vec::new();
    };
    let public_key = record
        .credentials
        .as_ref()
        .and_then(|credentials| reality_public_key(&credentials.reality_private_key))
        .unwrap_or_default();
    let label = device_label(&record.config);
    let options = ShareLinkOptions {
        fingerprint: REALITY_FALLBACK_FINGERPRINT.to_string(),
        ..ShareLinkOptions::default()
    };

    let mut links = Vec::new();
    for (kind, endpoint) in link_endpoints(&record.config, environment) {
        let profiles =
            selfhost_share_profiles(&spec, &public_key, std::slice::from_ref(&endpoint), &label);
        for profile in profiles {
            let protocol = match profile.config_type() {
                ConfigType::VLESS => SelfHostProtocol::Vless,
                _ => SelfHostProtocol::Shadowsocks,
            };
            match export_share_link_with_options(&profile, &options) {
                Ok(link) => links.push(SelfHostShareLink {
                    protocol,
                    address_kind: kind,
                    address: endpoint.address.clone(),
                    port: u16::try_from(profile.port()).unwrap_or_default(),
                    remarks: profile.remarks.clone(),
                    link,
                }),
                Err(error) => {
                    tracing::warn!(%error, ?kind, "failed to build a self-hosted share link");
                }
            }
        }
    }
    links
}

fn device_label(config: &SelfHostConfig) -> String {
    let label = config.device_label.trim();
    if label.is_empty() {
        DEFAULT_DEVICE_LABEL.to_string()
    } else {
        label.to_string()
    }
}

#[cfg(test)]
mod tests {
    use voya_contracts::{
        SelfHostAddressFamily, SelfHostCredentials, SelfHostFamilyReport, SelfHostFirewallStatus,
        SelfHostNatKind, SelfHostPortMappingReport, SelfHostPortMappingStatus,
        SelfHostReachability,
    };
    use voya_core::parse_share_link;

    use super::*;

    fn record() -> SelfHostRecord {
        SelfHostRecord {
            config: SelfHostConfig {
                enabled: true,
                vless_port: 42_443,
                shadowsocks_port: 42_444,
                device_label: "Tokyo".to_string(),
                ..SelfHostConfig::default()
            },
            credentials: Some(SelfHostCredentials {
                vless_uuid: "bd3a7c33-98cb-4faf-b0b5-853e2707be3f".to_string(),
                reality_private_key: "sJ2_PK3Bd1use05cc9jK6gcEarznMKgXeVNz9Dt4VF0".to_string(),
                reality_short_id: "751998bfb8ed69a6".to_string(),
                shadowsocks_password: "2oYz+Tnxj/q1Y/fi4+DkkQ==".to_string(),
            }),
        }
    }

    fn family(family: SelfHostAddressFamily, address: Option<&str>) -> SelfHostFamilyReport {
        SelfHostFamilyReport {
            family,
            public_address: address.map(str::to_string),
            nat: SelfHostNatKind::None,
            reachability: SelfHostReachability::Reachable,
            verified_by_probe: true,
            reasons: Vec::new(),
        }
    }

    fn environment(ipv4: Option<&str>, ipv6: Option<&str>) -> SelfHostEnvironmentReport {
        SelfHostEnvironmentReport {
            checked_at_ms: 0,
            local_addresses: Vec::new(),
            ipv4: family(SelfHostAddressFamily::Ipv4, ipv4),
            ipv6: family(SelfHostAddressFamily::Ipv6, ipv6),
            port_mapping: SelfHostPortMappingReport {
                status: SelfHostPortMappingStatus::Disabled,
                gateway_external_address: None,
                mapped_ports: Vec::new(),
                detail: None,
            },
            firewall: SelfHostFirewallStatus::NotManaged,
            probe_available: true,
            self_test: voya_contracts::SelfHostSelfTest {
                vless: voya_contracts::SelfHostSelfTestResult::Passed,
                shadowsocks: voya_contracts::SelfHostSelfTestResult::Passed,
            },
        }
    }

    fn codes(config: &SelfHostConfig) -> Vec<(String, ValidationCode)> {
        validate_self_host_config(config)
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|issue| (issue.field, issue.code))
            .collect()
    }

    #[test]
    fn the_default_config_is_valid() {
        assert!(validate_self_host_config(&SelfHostConfig::default()).is_ok());
    }

    #[test]
    fn validation_names_every_bad_field() {
        let config = SelfHostConfig {
            vless_port: 80,
            shadowsocks_port: 80,
            reality_server_name: "1.2.3.4".to_string(),
            reality_server_port: 0,
            custom_address: Some("bad host/".to_string()),
            device_label: "x".repeat(65),
            ..SelfHostConfig::default()
        };
        let fields = codes(&config)
            .into_iter()
            .map(|(field, _)| field)
            .collect::<Vec<_>>();
        assert_eq!(
            fields,
            [
                "vlessPort",
                "shadowsocksPort",
                "shadowsocksPort",
                "realityServerName",
                "realityServerPort",
                "customAddress",
                "deviceLabel",
            ]
        );

        let none = SelfHostConfig {
            vless_enabled: false,
            shadowsocks_enabled: false,
            ..SelfHostConfig::default()
        };
        assert_eq!(
            codes(&none),
            [("protocols".to_string(), ValidationCode::SelfHostNoProtocol)]
        );
    }

    #[test]
    fn normalization_trims_what_was_typed() {
        let config = normalized_config(SelfHostConfig {
            device_label: "  Tokyo  ".to_string(),
            reality_server_name: " WWW.Example.COM ".to_string(),
            custom_address: Some(" [2001:db8::1] ".to_string()),
            ..SelfHostConfig::default()
        });
        assert_eq!(config.device_label, "Tokyo");
        assert_eq!(config.reality_server_name, "www.example.com");
        assert_eq!(config.custom_address.as_deref(), Some("2001:db8::1"));
        let blank = normalized_config(SelfHostConfig {
            custom_address: Some("  ".to_string()),
            ..SelfHostConfig::default()
        });
        assert_eq!(blank.custom_address, None);
    }

    #[test]
    fn only_core_settings_restart_the_core() {
        let base = record().config;
        let cosmetic = SelfHostConfig {
            device_label: "Osaka".to_string(),
            custom_address: Some("node.example".to_string()),
            upnp_enabled: false,
            ..base.clone()
        };
        assert!(!core_settings_changed(&base, &cosmetic));
        let lan = SelfHostConfig {
            allow_lan_access: true,
            ..base.clone()
        };
        assert!(core_settings_changed(&base, &lan));
    }

    #[test]
    fn nothing_is_generated_before_credentials_exist() {
        let fresh = SelfHostRecord::default();
        assert!(selfhost_spec(&fresh, None, "warn").is_none());
        assert!(share_links(&fresh, Some(&environment(Some("203.0.113.7"), None))).is_empty());
    }

    #[test]
    fn links_cover_every_public_address_and_protocol() {
        let links = share_links(
            &record(),
            Some(&environment(Some("203.0.113.7"), Some("2001:db8::7"))),
        );
        let summary = links
            .iter()
            .map(|link| (link.protocol, link.address_kind, link.port))
            .collect::<Vec<_>>();
        assert_eq!(
            summary,
            [
                (SelfHostProtocol::Vless, SelfHostAddressKind::Ipv4, 42_443),
                (
                    SelfHostProtocol::Shadowsocks,
                    SelfHostAddressKind::Ipv4,
                    42_444
                ),
                (SelfHostProtocol::Vless, SelfHostAddressKind::Ipv6, 42_443),
                (
                    SelfHostProtocol::Shadowsocks,
                    SelfHostAddressKind::Ipv6,
                    42_444
                ),
            ]
        );
        let vless = parse_share_link(&links[2].link).expect("VLESS link parses");
        assert_eq!(vless.address(), "2001:db8::7");
        assert_eq!(
            vless.tls.and_then(|tls| tls.reality_public_key).as_deref(),
            Some("Q5mEoK_fSpzT4d13YC4_HI_2Crte_pRkSElgYb5wCD8")
        );
        assert!(links[0].link.contains("fp=chrome"), "{}", links[0].link);
        assert_eq!(links[0].remarks, "Tokyo · VLESS · IPv4");
        assert!(!links.iter().any(|link| link
            .link
            .contains("sJ2_PK3Bd1use05cc9jK6gcEarznMKgXeVNz9Dt4VF0")));
    }

    #[test]
    fn a_custom_address_replaces_the_detected_ones() {
        let mut record = record();
        record.config.custom_address = Some("node.example.org".to_string());
        record.config.shadowsocks_enabled = false;
        let links = share_links(&record, Some(&environment(Some("203.0.113.7"), None)));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].address_kind, SelfHostAddressKind::Custom);
        assert_eq!(links[0].address, "node.example.org");
        assert_eq!(enabled_ports(&record.config), [42_443]);
    }

    #[test]
    fn the_spec_carries_the_stored_credentials() {
        let spec = selfhost_spec(
            &record(),
            Some(SelfHostClashApi {
                port: 1,
                secret: "s".to_string(),
            }),
            "info",
        )
        .expect("spec");
        let vless = spec.vless.expect("vless");
        assert_eq!(vless.port, 42_443);
        assert_eq!(vless.reality_server_name, "www.apple.com");
        assert_eq!(
            spec.shadowsocks.expect("ss").password,
            "2oYz+Tnxj/q1Y/fi4+DkkQ=="
        );
        assert!(spec.block_bittorrent);
        assert!(!spec.allow_lan_access);
    }
}
