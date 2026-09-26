use crate::{
    export_share_link_with_options, parse_share_link,
    singbox::{generate_singbox_config, REALITY_FALLBACK_FINGERPRINT},
    testutil::linux_context,
    AppConfig, ShareLinkOptions, DIRECT_TAG, PROXY_TAG,
};

use super::*;

const TEST_REALITY_PRIVATE_KEY: &str = "sJ2_PK3Bd1use05cc9jK6gcEarznMKgXeVNz9Dt4VF0";
const TEST_REALITY_PUBLIC_KEY: &str = "Q5mEoK_fSpzT4d13YC4_HI_2Crte_pRkSElgYb5wCD8";

fn test_spec() -> SelfHostSpec {
    SelfHostSpec {
        vless: Some(SelfHostVlessSpec {
            port: 42_443,
            uuid: "bd3a7c33-98cb-4faf-b0b5-853e2707be3f".to_string(),
            reality_private_key: TEST_REALITY_PRIVATE_KEY.to_string(),
            reality_short_id: "751998bfb8ed69a6".to_string(),
            reality_server_name: "www.apple.com".to_string(),
            reality_server_port: 443,
        }),
        shadowsocks: Some(SelfHostShadowsocksSpec {
            port: 42_444,
            password: "2oYz+Tnxj/q1Y/fi4+DkkQ==".to_string(),
        }),
        allow_lan_access: false,
        block_bittorrent: true,
        clash_api: Some(SelfHostClashApi {
            port: 42_445,
            secret: "selfhost-secret".to_string(),
        }),
        log_level: "warning".to_string(),
    }
}

fn rule_actions(config: &SingboxConfig) -> Vec<String> {
    config
        .route
        .rules
        .iter()
        .map(|rule| rule.action.clone().unwrap_or_default())
        .collect()
}

#[test]
fn selfhost_config_listens_on_every_family_and_exits_direct() {
    let config = generate_singbox_selfhost_config(&test_spec());
    let tags = config
        .inbounds
        .iter()
        .map(|inbound| (inbound.tag.as_str(), inbound.listen.as_deref()))
        .collect::<Vec<_>>();
    assert_eq!(
        tags,
        [
            (SELFHOST_VLESS_INBOUND_TAG, Some("::")),
            (SELFHOST_SHADOWSOCKS_INBOUND_TAG, Some("::")),
        ]
    );
    assert_eq!(config.outbounds.len(), 1);
    assert_eq!(config.outbounds[0].tag, DIRECT_TAG);
    assert_eq!(config.route.final_outbound.as_deref(), Some(DIRECT_TAG));
    assert_eq!(config.route.auto_detect_interface, Some(true));
    // `warning` is not a sing-box level; the generator must not forward it.
    assert_eq!(
        config.log.as_ref().map(|log| log.level.as_str()),
        Some("warn")
    );
    let clash_api = config
        .experimental
        .and_then(|experimental| experimental.clash_api)
        .expect("clash api");
    assert_eq!(
        clash_api.external_controller.as_deref(),
        Some("127.0.0.1:42445")
    );
    assert_eq!(clash_api.secret.as_deref(), Some("selfhost-secret"));
}

#[test]
fn selfhost_config_resolves_before_it_rejects() {
    let config = generate_singbox_selfhost_config(&test_spec());
    assert_eq!(
        rule_actions(&config),
        ["sniff", "resolve", "reject", "reject", "reject"]
    );
    let bittorrent = config.route.rules.last().expect("bittorrent rule");
    assert_eq!(bittorrent.protocol, Some(vec!["bittorrent".to_string()]));
}

#[test]
fn selfhost_config_never_opens_loopback_or_metadata() {
    for allow_lan_access in [false, true] {
        let spec = SelfHostSpec {
            allow_lan_access,
            block_bittorrent: false,
            ..test_spec()
        };
        let config = generate_singbox_selfhost_config(&spec);
        let denied = config.route.rules[2]
            .ip_cidr
            .clone()
            .expect("always-denied rule");
        for cidr in ["127.0.0.0/8", "::1/128", "169.254.0.0/16", "fe80::/10"] {
            assert!(denied.iter().any(|value| value == cidr), "{cidr}");
        }
        let private_rule = config
            .route
            .rules
            .iter()
            .find(|rule| rule.ip_is_private == Some(true));
        assert_eq!(private_rule.is_some(), !allow_lan_access);
        if let Some(rule) = private_rule {
            assert_eq!(rule.ip_cidr, Some(vec!["100.64.0.0/10".to_string()]));
        }
    }
}

#[test]
fn selfhost_config_without_protocols_has_no_inbounds() {
    let spec = SelfHostSpec {
        vless: None,
        shadowsocks: None,
        clash_api: None,
        ..test_spec()
    };
    assert!(!spec.has_inbound());
    let config = generate_singbox_selfhost_config(&spec);
    assert!(config.inbounds.is_empty());
    assert!(config.experimental.is_none());
}

#[test]
fn selfhost_config_log_level_none_disables_logging() {
    let spec = SelfHostSpec {
        log_level: "none".to_string(),
        ..test_spec()
    };
    let log = generate_singbox_selfhost_config(&spec)
        .log
        .expect("log block");
    assert_eq!(log.disabled, Some(true));
}

#[test]
fn selfhost_share_profiles_cover_every_protocol_and_endpoint() {
    let endpoints = [
        SelfHostEndpoint {
            address: "203.0.113.7".to_string(),
            label: "IPv4".to_string(),
        },
        SelfHostEndpoint {
            address: "2001:db8::7".to_string(),
            label: "IPv6".to_string(),
        },
    ];
    let profiles =
        selfhost_share_profiles(&test_spec(), TEST_REALITY_PUBLIC_KEY, &endpoints, "Tokyo");
    let remarks = profiles
        .iter()
        .map(|profile| profile.remarks.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        remarks,
        [
            "Tokyo · VLESS · IPv4",
            "Tokyo · SS · IPv4",
            "Tokyo · VLESS · IPv6",
            "Tokyo · SS · IPv6",
        ]
    );
    let unlabeled = selfhost_share_profiles(&test_spec(), TEST_REALITY_PUBLIC_KEY, &endpoints, " ");
    assert_eq!(unlabeled[0].remarks, "VLESS · IPv4");
}

#[test]
fn selfhost_share_links_round_trip_into_a_matching_client() {
    let spec = test_spec();
    let server = generate_singbox_selfhost_config(&spec);
    let endpoints = [SelfHostEndpoint {
        address: "2001:db8::7".to_string(),
        label: "IPv6".to_string(),
    }];
    let options = ShareLinkOptions {
        fingerprint: REALITY_FALLBACK_FINGERPRINT.to_string(),
        ..ShareLinkOptions::default()
    };

    for profile in selfhost_share_profiles(&spec, TEST_REALITY_PUBLIC_KEY, &endpoints, "Tokyo") {
        let link = export_share_link_with_options(&profile, &options).expect("export link");
        let parsed = parse_share_link(&link).expect("parse link");
        assert_eq!(parsed.protocol, profile.protocol, "{link}");
        assert_eq!(parsed.tls, profile.tls, "{link}");

        let client = generate_singbox_config(&linux_context(AppConfig::default(), parsed))
            .expect("client config");
        let outbound = client
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == PROXY_TAG)
            .expect("proxy outbound");
        let inbound = server
            .inbounds
            .iter()
            .find(|inbound| inbound.r#type == outbound.r#type)
            .expect("matching server inbound");
        assert_eq!(outbound.server.as_deref(), Some("2001:db8::7"));
        assert_eq!(outbound.server_port, inbound.listen_port);

        match outbound.r#type.as_str() {
            "vless" => {
                let user = &inbound.users.as_ref().expect("vless users")[0];
                assert_eq!(outbound.uuid, user.uuid);
                assert_eq!(outbound.flow, user.flow);
                let client_tls = outbound.tls.as_ref().expect("client tls");
                let server_tls = inbound.tls.as_ref().expect("server tls");
                assert_eq!(client_tls.server_name, server_tls.server_name);
                let client_reality = client_tls.reality.as_ref().expect("client reality");
                let server_reality = server_tls.reality.as_ref().expect("server reality");
                assert_eq!(client_reality.public_key, TEST_REALITY_PUBLIC_KEY);
                assert!(server_reality.short_id.contains(&client_reality.short_id));
                assert_eq!(
                    client_tls
                        .utls
                        .as_ref()
                        .map(|utls| utls.fingerprint.as_str()),
                    Some(REALITY_FALLBACK_FINGERPRINT)
                );
            }
            "shadowsocks" => {
                assert_eq!(outbound.method, inbound.method);
                assert_eq!(outbound.password, inbound.password);
            }
            other => panic!("unexpected outbound type {other}"),
        }
    }
}
