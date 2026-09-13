use std::collections::BTreeMap;

use super::*;
use crate::{golden, CoreGenPlatform, CoreType, RoutingItem, ServerEndpoint, TlsSettings};

#[test]
fn singbox_outbound_vless_ws_tls_mux_matches_golden() {
    let mut config = AppConfig::default();
    config.core_basic_item.tls_fragment = crate::TlsFragmentMode::Record;
    config.core_basic_item.mux_enabled = true;
    config.core_basic_item.def_fingerprint = "firefox".to_string();
    config.core_basic_item.def_user_agent = "chrome".to_string();

    let node = ProfileItem {
        index_id: "n-vless".to_string(),
        remarks: "vless-ws".to_string(),
        protocol: ProfileProtocol::Vless {
            server: endpoint("server.example", 443),
            uuid: "00000000-0000-0000-0000-000000000011".to_string(),
            flow: None,
            encryption: Some("none".to_string()),
        },
        transport: Some(ProfileTransport::Websocket {
            host: Some("cdn.example".to_string()),
            path: Some("/ws?ed=2048".to_string()),
        }),
        tls: Some(TlsSettings {
            alpn: vec!["h2".to_string(), "http/1.1".to_string()],
            ech_config: vec![
                "ech.example".to_string(),
                "https://dns.example/dns-query".to_string(),
            ],
            ..tls_settings(TlsMode::Tls, Some("tls.example"))
        }),
        ..ProfileItem::default()
    };

    let generated = generate_singbox_config(&test_context(config, node))
        .expect("sing-box config should generate");
    let proxy = generated
        .outbounds
        .iter()
        .find(|outbound| outbound.tag == PROXY_TAG)
        .expect("proxy outbound");
    let value =
        serde_json::to_value(proxy).expect("sing-box VLESS outbound should serialize to JSON");
    assert_no_nulls(&value);

    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/singbox/outbounds/vless_ws_tls_mux.json"
    ))
    .expect("sing-box VLESS outbound golden fixture should parse as JSON");
    golden::assert_json_eq("singbox-outbound-vless-ws-tls-mux", &expected, &value);

    let full_value =
        serde_json::to_value(generated).expect("sing-box config should serialize to JSON");
    assert_no_nulls(&full_value);
    assert_eq!(
        full_value.pointer("/experimental/clash_api/external_controller"),
        Some(&Value::String("127.0.0.1:10813".to_string()))
    );
    assert_eq!(
        full_value.pointer("/experimental/cache_file/enabled"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn singbox_outbound_vmess_h2_transport_matches_golden() {
    let node = ProfileItem {
        index_id: "n-vmess-h2".to_string(),
        remarks: "vmess-h2".to_string(),
        protocol: ProfileProtocol::Vmess {
            server: endpoint("h2.example", 443),
            uuid: "00000000-0000-0000-0000-000000000031".to_string(),
            cipher: Some(DEFAULT_SECURITY.to_string()),
        },
        transport: Some(ProfileTransport::Http2 {
            host: Some("h2-one.example,h2-two.example".to_string()),
            path: Some("/h2".to_string()),
        }),
        tls: Some(tls_settings(TlsMode::Tls, Some("h2.example"))),
        ..ProfileItem::default()
    };

    let value = generated_proxy_outbound_value(AppConfig::default(), node);
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/singbox/outbounds/vmess_h2_tls.json"
    ))
    .expect("sing-box VMess h2 golden fixture should parse as JSON");
    golden::assert_json_eq("singbox-outbound-vmess-h2-tls", &expected, &value);
}

#[test]
fn singbox_outbound_vless_quic_transport_matches_golden() {
    let node = ProfileItem {
        index_id: "n-vless-quic".to_string(),
        remarks: "vless-quic".to_string(),
        protocol: ProfileProtocol::Vless {
            server: endpoint("quic.example", 443),
            uuid: "00000000-0000-0000-0000-000000000032".to_string(),
            flow: None,
            encryption: Some("none".to_string()),
        },
        transport: Some(ProfileTransport::Quic {
            host: None,
            path: None,
        }),
        tls: Some(tls_settings(TlsMode::Tls, Some("quic.example"))),
        ..ProfileItem::default()
    };

    let value = generated_proxy_outbound_value(AppConfig::default(), node);
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/singbox/outbounds/vless_quic_tls.json"
    ))
    .expect("sing-box VLESS quic golden fixture should parse as JSON");
    golden::assert_json_eq("singbox-outbound-vless-quic-tls", &expected, &value);
}

#[test]
fn singbox_h2_and_quic_transports_are_not_emitted_as_raw_tcp() {
    for (transport, expected_type) in [
        (
            ProfileTransport::Http2 {
                host: Some("h2.example".to_string()),
                path: Some("/h2".to_string()),
            },
            "http",
        ),
        (
            ProfileTransport::Quic {
                host: None,
                path: None,
            },
            "quic",
        ),
    ] {
        let node = ProfileItem {
            protocol: ProfileProtocol::Vmess {
                server: endpoint("server.example", 443),
                uuid: "00000000-0000-0000-0000-000000000034".to_string(),
                cipher: Some(DEFAULT_SECURITY.to_string()),
            },
            transport: Some(transport),
            tls: Some(tls_settings(TlsMode::Tls, Some("server.example"))),
            ..ProfileItem::default()
        };
        // Validation lets these through, so the generator has to emit them.
        assert!(crate::validate_node(&node, CoreType::sing_box).success());

        let context = test_context(AppConfig::default(), node.clone());
        let outbound = build_outbound(&context, &node);
        assert_eq!(
            outbound
                .transport
                .as_ref()
                .and_then(|transport| transport.r#type.as_deref()),
            Some(expected_type)
        );
    }
}

#[test]
fn singbox_outbound_hysteria2_without_tls_settings_matches_golden() {
    let node = ProfileItem {
        index_id: "n-hy2-min".to_string(),
        remarks: "hysteria2-minimal".to_string(),
        protocol: ProfileProtocol::Hysteria2 {
            server: endpoint("203.0.113.5", 443),
            password: "hy2-pass".to_string(),
            port_hops: None,
            obfuscation_password: None,
        },
        transport: None,
        tls: None,
        ..ProfileItem::default()
    };

    let value = generated_proxy_outbound_value(AppConfig::default(), node);
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/singbox/outbounds/hysteria2_minimal.json"
    ))
    .expect("sing-box Hysteria2 golden fixture should parse as JSON");
    golden::assert_json_eq("singbox-outbound-hysteria2-minimal", &expected, &value);
}

#[test]
fn singbox_tls_only_protocols_always_emit_a_tls_block() {
    let cases = [
        (
            ConfigType::Hysteria2,
            ProfileProtocol::Hysteria2 {
                server: endpoint("server.example", 443),
                password: "secret".to_string(),
                port_hops: None,
                obfuscation_password: None,
            },
        ),
        (
            ConfigType::TUIC,
            ProfileProtocol::Tuic {
                server: endpoint("server.example", 443),
                uuid: "00000000-0000-0000-0000-000000000033".to_string(),
                password: "secret".to_string(),
                congestion_control: None,
            },
        ),
        (
            ConfigType::Anytls,
            ProfileProtocol::Anytls {
                server: endpoint("server.example", 443),
                password: "secret".to_string(),
            },
        ),
        (
            ConfigType::Naive,
            ProfileProtocol::Naive {
                server: endpoint("server.example", 443),
                username: "user".to_string(),
                password: "secret".to_string(),
                quic: false,
                congestion_control: None,
                insecure_concurrency: None,
                udp_over_tcp: false,
            },
        ),
    ];

    for (config_type, protocol) in cases {
        let node = ProfileItem {
            remarks: format!("{config_type:?}"),
            protocol,
            transport: None,
            tls: None,
            ..ProfileItem::default()
        };
        assert_eq!(node.config_type(), config_type);

        let context = test_context(AppConfig::default(), node.clone());
        let tls = build_outbound(&context, &node).tls;
        assert!(
            tls.as_ref().is_some_and(|tls| tls.enabled),
            "{config_type:?} outbound must carry an enabled TLS block"
        );
        assert_eq!(
            tls.and_then(|tls| tls.server_name),
            None,
            "{config_type:?} outbound must leave SNI to the server address"
        );
    }

    // Plaintext-capable protocols keep their existing "no TLS settings, no TLS
    // block" behavior.
    let socks = socks_node("socks", "socks");
    let context = test_context(AppConfig::default(), socks.clone());
    assert!(build_outbound(&context, &socks).tls.is_none());
}

#[test]
fn singbox_protocol_support_table_agrees_with_node_validation() {
    for config_type in [
        ConfigType::VMess,
        ConfigType::Shadowsocks,
        ConfigType::SOCKS,
        ConfigType::VLESS,
        ConfigType::Trojan,
        ConfigType::Hysteria2,
        ConfigType::TUIC,
        ConfigType::WireGuard,
        ConfigType::HTTP,
        ConfigType::Anytls,
        ConfigType::Naive,
    ] {
        let node = ProfileItem {
            remarks: format!("{config_type:?}"),
            protocol: sample_protocol(config_type),
            transport: Some(raw_transport()),
            tls: None,
            ..ProfileItem::default()
        };
        assert_eq!(node.config_type(), config_type);

        let rejects_protocol = crate::validate_node(&node, CoreType::sing_box)
            .errors
            .iter()
            .any(|error| {
                matches!(
                    error.code,
                    crate::validation::ValidationCode::UnsupportedProtocol { .. }
                        | crate::validation::ValidationCode::UnsupportedProtocolNetwork { .. }
                )
            });
        let expected_rejection = !singbox_supports_config_type(config_type);
        assert_eq!(
            rejects_protocol, expected_rejection,
            "validation and generation disagree about {config_type:?}"
        );
    }
}

#[test]
fn singbox_tls_insecure_requires_application_gate() {
    let node = base_remote_node();
    let context = test_context(AppConfig::default(), node.clone());
    assert_eq!(
        build_outbound(&context, &node)
            .tls
            .expect("tls settings should be generated")
            .insecure,
        Some(false)
    );

    let mut config = AppConfig::default();
    config.core_basic_item.def_allow_insecure = true;
    let context = test_context(config.clone(), node.clone());
    assert_eq!(
        build_outbound(&context, &node)
            .tls
            .expect("tls settings should be generated")
            .insecure,
        Some(true)
    );

    let context = test_context(config, node.clone());
    assert_eq!(
        build_outbound(&context, &node)
            .tls
            .expect("tls settings should be generated")
            .insecure,
        Some(true)
    );
}

#[test]
fn singbox_pinned_cert_and_reality_force_insecure_false() {
    let mut config = AppConfig::default();
    config.core_basic_item.def_allow_insecure = true;

    let mut pinned_node = base_remote_node();
    pinned_node.tls = Some(TlsSettings {
        certificate_pem: Some(
            "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----".to_string(),
        ),
        ..tls_settings(TlsMode::Tls, Some("server.example"))
    });
    let context = test_context(config.clone(), pinned_node.clone());
    let tls = build_outbound(&context, &pinned_node)
        .tls
        .expect("tls settings should be generated");
    assert_eq!(tls.insecure, Some(false));
    assert!(tls.certificate.is_some());

    let reality_node = ProfileItem {
        tls: Some(TlsSettings {
            reality_public_key: Some("reality-public-key".to_string()),
            reality_short_id: Some("reality-short-id".to_string()),
            ..tls_settings(TlsMode::Reality, Some("server.example"))
        }),
        ..base_remote_node()
    };
    let context = test_context(config, reality_node.clone());
    let tls = build_outbound(&context, &reality_node)
        .tls
        .expect("tls settings should be generated");
    assert_eq!(tls.insecure, Some(false));
    assert!(tls.reality.is_some());
}

#[test]
fn singbox_transport_hosts_use_first_authority() {
    let node = ProfileItem {
        transport: Some(ProfileTransport::Websocket {
            host: Some("one.example, two.example".to_string()),
            path: Some("/ws".to_string()),
        }),
        tls: Some(tls_settings(TlsMode::Tls, None)),
        ..base_remote_node()
    };
    let context = test_context(AppConfig::default(), node.clone());
    let outbound = build_outbound(&context, &node);
    assert_eq!(
        outbound
            .transport
            .as_ref()
            .and_then(|transport| transport.headers.as_ref())
            .and_then(|headers| headers.host.as_deref()),
        Some("one.example")
    );
    assert_eq!(
        outbound
            .tls
            .as_ref()
            .and_then(|tls| tls.server_name.as_deref()),
        Some("one.example")
    );

    let node = ProfileItem {
        transport: Some(ProfileTransport::HttpUpgrade {
            host: Some("upgrade.example, backup.example".to_string()),
            path: Some("/up".to_string()),
        }),
        tls: Some(tls_settings(TlsMode::Tls, None)),
        ..base_remote_node()
    };
    let context = test_context(AppConfig::default(), node.clone());
    let outbound = build_outbound(&context, &node);
    assert_eq!(
        outbound
            .transport
            .as_ref()
            .and_then(|transport| transport.host.as_ref()),
        Some(&Value::String("upgrade.example".to_string()))
    );
    assert_eq!(
        outbound
            .tls
            .as_ref()
            .and_then(|tls| tls.server_name.as_deref()),
        Some("upgrade.example")
    );

    let node = ProfileItem {
        protocol: ProfileProtocol::Trojan {
            server: endpoint("server.example", 443),
            password: "secret".to_string(),
        },
        transport: Some(ProfileTransport::Grpc {
            authority: Some("grpc-one.example, grpc-two.example".to_string()),
            service_name: Some("svc".to_string()),
            mode: None,
        }),
        tls: Some(tls_settings(TlsMode::Tls, None)),
        ..base_remote_node()
    };
    let context = test_context(AppConfig::default(), node.clone());
    let outbound = build_outbound(&context, &node);
    assert_eq!(
        outbound
            .tls
            .as_ref()
            .and_then(|tls| tls.server_name.as_deref()),
        Some("grpc-one.example")
    );
    // The keepalive values are fixed constants now; pin their generated form.
    let grpc = outbound.transport.as_ref().expect("gRPC transport");
    assert_eq!(grpc.idle_timeout.as_deref(), Some("60s"));
    assert_eq!(grpc.ping_timeout.as_deref(), Some("20s"));
    assert_eq!(grpc.permit_without_stream, Some(false));

    let node = ProfileItem {
        protocol: ProfileProtocol::Shadowsocks {
            server: endpoint("server.example", 443),
            password: "secret".to_string(),
            method: "aes-128-gcm".to_string(),
            udp_over_tcp: false,
        },
        transport: Some(ProfileTransport::Websocket {
            host: Some("plugin-one.example, plugin-two.example".to_string()),
            path: Some("/plugin".to_string()),
        }),
        tls: None,
        ..base_remote_node()
    };
    let context = test_context(AppConfig::default(), node.clone());
    let outbound = build_outbound(&context, &node);
    assert_eq!(
        outbound.plugin_opts.as_deref(),
        Some("mode=websocket;host=plugin-one.example;path=/plugin;mux=0")
    );
}

#[test]
fn singbox_invalid_ports_are_rejected_or_skipped() {
    assert!(parse_dns_address("1.1.1.1:70000").is_none());
    assert!(parse_dns_address("https://dns.example:70000/dns-query").is_none());
    assert!(parse_dns_address("8.8.8.8:0").is_none());

    let fallback = parse_dns_address_or_default("1.1.1.1:70000", DEFAULT_DIRECT_DNS);
    assert_ne!(fallback.server_port, Some(70000));

    let node = ProfileItem {
        protocol: ProfileProtocol::Vmess {
            server: endpoint("server.example", 70_000),
            uuid: String::new(),
            cipher: None,
        },
        ..base_remote_node()
    };
    let error = generate_singbox_config(&test_context(AppConfig::default(), node))
        .expect_err("invalid node port should be rejected");
    assert!(matches!(
        error,
        SingboxConfigError::InvalidNodePort { port: 70000, .. }
    ));
}

#[test]
fn singbox_dns_final_tracks_enabled_route_rules_only() {
    let catch_all_direct = RulesItem {
        outbound_tag: Some(DIRECT_TAG.to_string()),
        port: Some("0-65535".to_string()),
        rule_type: Some(RuleType::Routing),
        ..RulesItem::default()
    };
    let final_dns_tag = |rule: RulesItem| {
        let mut context = test_context(AppConfig::default(), base_remote_node());
        context.routing_item = Some(RoutingItem {
            rule_set: vec![rule],
            ..RoutingItem::default()
        });
        let generated = generate_singbox_config(&context).expect("sing-box config should generate");
        assert_eq!(generated.route.final_outbound.as_deref(), Some(PROXY_TAG));
        generated
            .dns
            .and_then(|dns| dns.final_server)
            .expect("sing-box DNS final server")
    };

    assert_eq!(
        final_dns_tag(catch_all_direct.clone()),
        SINGBOX_DIRECT_DNS_TAG
    );
    assert_eq!(
        final_dns_tag(RulesItem {
            enabled: false,
            ..catch_all_direct.clone()
        }),
        SINGBOX_REMOTE_DNS_TAG
    );
    assert_eq!(
        final_dns_tag(RulesItem {
            rule_type: Some(RuleType::DNS),
            ..catch_all_direct
        }),
        SINGBOX_REMOTE_DNS_TAG
    );
}

#[test]
fn singbox_dns_address_uses_dhcp_interface_and_unbracketed_ipv6() {
    let dhcp = parse_dns_address("dhcp://en0").expect("dhcp DNS server");
    assert_eq!(dhcp.r#type, "dhcp");
    assert_eq!(dhcp.interface_name.as_deref(), Some("en0"));
    assert_eq!(dhcp.server, None);
    assert_eq!(
        serde_json::to_value(&dhcp)
            .expect("dhcp DNS server should serialize to JSON")
            .get("interface")
            .and_then(Value::as_str),
        Some("en0")
    );

    let dhcp_auto = parse_dns_address("dhcp://auto").expect("dhcp auto DNS server");
    assert_eq!(dhcp_auto.interface_name, None);
    assert_eq!(dhcp_auto.server, None);

    let dot = parse_dns_address("tls://[2606:4700:4700::1111]:853").expect("DoT DNS server");
    assert_eq!(dot.server.as_deref(), Some("2606:4700:4700::1111"));
    assert_eq!(dot.server_port, Some(853));

    let doh =
        parse_dns_address("https://[2001:4860:4860::8888]/dns-query").expect("DoH DNS server");
    assert_eq!(doh.server.as_deref(), Some("2001:4860:4860::8888"));
    assert_eq!(doh.path.as_deref(), Some("/dns-query"));

    let plain = parse_dns_address("[2606:4700:4700::1111]:53").expect("plain IPv6 DNS server");
    assert_eq!(plain.server.as_deref(), Some("2606:4700:4700::1111"));
    assert_eq!(plain.server_port, Some(53));

    let bare = parse_dns_address("2606:4700:4700::1111").expect("bare IPv6 DNS server");
    assert_eq!(bare.server.as_deref(), Some("2606:4700:4700::1111"));
    assert_eq!(bare.server_port, None);
}

#[test]
fn singbox_clash_api_port_follows_the_context_not_the_tun_setting() {
    // Linux + TUN splits into two processes: the main config keeps the base
    // api2 port and only the pre-socks (TUN) config takes api2 + 1. Deriving
    // the port from `tun_mode_item.enable_tun` instead would point every
    // client at the TUN process, which has no selector or urltest outbounds.
    let mut app_config = AppConfig::default();
    app_config.tun_mode_item.enable_tun = true;
    let api2_port = inbound_port(&app_config, InboundProtocol::api2);

    let mut main_context = test_context(app_config.clone(), base_remote_node());
    main_context.is_tun_enabled = false;
    let mut pre_context = test_context(app_config, socks_node("pre", "pre-socks"));
    pre_context.is_tun_enabled = true;

    assert_eq!(main_context.clash_api_port(), api2_port);
    assert_eq!(pre_context.clash_api_port(), api2_port + 1);
    for context in [&main_context, &pre_context] {
        let generated = generate_singbox_config(context).expect("sing-box config should generate");
        assert_eq!(
            generated
                .experimental
                .and_then(|experimental| experimental.clash_api)
                .and_then(|clash_api| clash_api.external_controller),
            Some(format!("{LOOPBACK}:{}", context.clash_api_port()))
        );
    }
}

#[test]
fn singbox_clash_api_secret_is_emitted_only_when_injected() {
    let clash_api = |secret: Option<&str>| {
        let mut context = test_context(AppConfig::default(), base_remote_node());
        context.clash_api_secret = secret.map(str::to_string);
        generate_singbox_config(&context)
            .expect("sing-box config should generate")
            .experimental
            .and_then(|experimental| experimental.clash_api)
            .expect("clash api block")
    };

    assert_eq!(clash_api(None).secret, None);
    assert_eq!(clash_api(Some("   ")).secret, None);
    let guarded = clash_api(Some("token-1234"));
    assert_eq!(guarded.secret.as_deref(), Some("token-1234"));
    assert_eq!(
        serde_json::to_value(&guarded)
            .expect("clash api should serialize to JSON")
            .get("secret")
            .and_then(Value::as_str),
        Some("token-1234")
    );
}

/// One `parse_dns_address` expectation: input, then the expected
/// `(kind, server, port, path)` the parser should produce.
type DnsAddressCase = (
    &'static str,
    &'static str,
    Option<&'static str>,
    Option<i32>,
    Option<&'static str>,
);

#[test]
fn singbox_dns_address_table_covers_local_scheme_and_path_branches() {
    let cases: &[DnsAddressCase] = &[
        ("local", "local", None, None, None),
        ("localhost", "local", None, None, None),
        ("8.8.8.8", "udp", Some("8.8.8.8"), None, None),
        ("8.8.8.8:5353", "udp", Some("8.8.8.8"), Some(5353), None),
        ("tls://1.1.1.1", "tls", Some("1.1.1.1"), None, None),
        ("udp+local://1.1.1.1", "udp", Some("1.1.1.1"), None, None),
        (
            "https://dns.example/dns-query?x=1",
            "https",
            Some("dns.example"),
            None,
            Some("/dns-query?x=1"),
        ),
        (
            "h3://dns.example/q",
            "h3",
            Some("dns.example"),
            None,
            Some("/q"),
        ),
        // Only the HTTP-family transports carry a path; a DoT authority with a
        // trailing slash must not leak one into the config.
        ("tls://dns.example/", "tls", Some("dns.example"), None, None),
        (
            "quic://dns.example/ignored",
            "quic",
            Some("dns.example"),
            None,
            None,
        ),
        (
            "dns.example/dns-query",
            "udp",
            Some("dns.example"),
            None,
            None,
        ),
    ];

    for (input, kind, server, port, path) in cases {
        let parsed = parse_dns_address(input)
            .unwrap_or_else(|| panic!("`{input}` should parse as a DNS server"));
        assert_eq!(&parsed.r#type, kind, "type for `{input}`");
        assert_eq!(parsed.server.as_deref(), *server, "server for `{input}`");
        assert_eq!(parsed.server_port, *port, "port for `{input}`");
        assert_eq!(parsed.path.as_deref(), *path, "path for `{input}`");
    }

    for input in ["", "   ", "1.1.1.1:70000", "8.8.8.8:0", "://nope"] {
        assert!(
            parse_dns_address(input).is_none(),
            "`{input}` must not parse as a DNS server"
        );
    }

    // A comma or semicolon list keeps only the first non-empty entry.
    let first = parse_dns_address(" , 9.9.9.9, 1.1.1.1").expect("first DNS address");
    assert_eq!(first.server.as_deref(), Some("9.9.9.9"));

    // The fallback default is used when the primary address is unusable.
    let fallback = parse_dns_address_or_default("1.1.1.1:0", "8.8.4.4");
    assert_eq!(fallback.server.as_deref(), Some("8.8.4.4"));
}

#[test]
fn singbox_dns_helper_tables_parse_hosts_strategies_and_rcodes() {
    let hosts = parse_hosts_to_dictionary(Some(concat!(
        "# comment line\n",
        "\n",
        "bare-line-without-value\n",
        "example.test 1.1.1.1 2.2.2.2\n",
        "example.test 3.3.3.3\n",
        "block.test #3\n"
    )));
    assert_eq!(
        hosts.get("example.test").map(Vec::as_slice),
        Some(
            ["1.1.1.1", "2.2.2.2", "3.3.3.3"]
                .map(str::to_string)
                .as_slice()
        )
    );
    assert_eq!(
        hosts.get("block.test").map(Vec::as_slice),
        Some(["#3".to_string()].as_slice())
    );
    assert!(!hosts.contains_key("bare-line-without-value"));
    assert!(!hosts.contains_key("# comment line"));

    assert_eq!(domain_strategy4_sbox(None), None);
    assert_eq!(domain_strategy4_sbox(Some("AsIs")), None);
    assert_eq!(
        domain_strategy4_sbox(Some("UseIPv4")).as_deref(),
        Some("prefer_ipv4")
    );
    assert_eq!(
        domain_strategy4_sbox(Some("UseIPv6")).as_deref(),
        Some("prefer_ipv6")
    );
    assert_eq!(
        domain_strategy4_sbox(Some("ForceIPv4")).as_deref(),
        Some("ipv4_only")
    );
    assert_eq!(
        domain_strategy4_sbox(Some("ForceIPv6v4")).as_deref(),
        Some("ipv6_only")
    );

    assert_eq!(dns_rcode(0), "NOERROR");
    assert_eq!(dns_rcode(1), "FORMERR");
    assert_eq!(dns_rcode(2), "SERVFAIL");
    assert_eq!(dns_rcode(3), "NXDOMAIN");
    assert_eq!(dns_rcode(4), "NOTIMP");
    assert_eq!(dns_rcode(5), "REFUSED");
    assert_eq!(dns_rcode(99), "NOERROR");

    assert!(is_domain_name("target.example"));
    assert!(!is_domain_name("1.2.3.4"));
    assert!(!is_domain_name("2606:4700::1111"));
    assert!(!is_domain_name("localhost"));
    assert!(!is_domain_name(""));

    let (ip_cidr, regions, region_name) =
        parse_direct_expected_ips(Some("geoip:cn; 192.0.2.0/24 ,geoip:hk"));
    assert_eq!(ip_cidr, vec!["192.0.2.0/24".to_string()]);
    assert_eq!(regions, vec!["cn".to_string(), "hk".to_string()]);
    assert_eq!(region_name, "hk");
    assert_eq!(
        parse_direct_expected_ips(None),
        (Vec::new(), Vec::new(), String::new())
    );
}

#[test]
fn singbox_shadowsocks_plugin_options_come_from_the_shared_model() {
    let obfs = ProfileItem {
        index_id: "ss-obfs".to_string(),
        remarks: "ss-obfs".to_string(),
        protocol: ProfileProtocol::Shadowsocks {
            server: endpoint("ss.example", 8388),
            password: "secret".to_string(),
            method: "aes-256-gcm".to_string(),
            udp_over_tcp: false,
        },
        transport: Some(ProfileTransport::Tcp {
            header: Some("http".to_string()),
            host: Some("obfs.example,second.example".to_string()),
            path: None,
        }),
        tls: None,
        ..ProfileItem::default()
    };
    let value = generated_proxy_outbound_value(AppConfig::default(), obfs);
    assert_eq!(value["plugin"].as_str(), Some("obfs-local"));
    assert_eq!(
        value["plugin_opts"].as_str(),
        Some("obfs=http;obfs-host=obfs.example")
    );

    let websocket = ProfileItem {
        index_id: "ss-ws".to_string(),
        remarks: "ss-ws".to_string(),
        protocol: ProfileProtocol::Shadowsocks {
            server: endpoint("ss.example", 8388),
            password: "secret".to_string(),
            method: "aes-256-gcm".to_string(),
            udp_over_tcp: false,
        },
        transport: Some(ProfileTransport::Websocket {
            host: Some("ws.example".to_string()),
            path: Some("/path?a=1,2".to_string()),
        }),
        tls: Some(tls_settings(TlsMode::Tls, Some("ws.example"))),
        ..ProfileItem::default()
    };
    let value = generated_proxy_outbound_value(AppConfig::default(), websocket);
    assert_eq!(value["plugin"].as_str(), Some("v2ray-plugin"));
    assert_eq!(
        value["plugin_opts"].as_str(),
        Some("mode=websocket;host=ws.example;path=/path?a\\=1\\,2;tls;mux=0")
    );
}

#[test]
fn singbox_dns_bootstrap_and_expected_ips_reach_servers_and_rules() {
    let mut app_config = AppConfig::default();
    app_config.simple_dns_item.bootstrap_dns = Some("223.5.5.5:5353".to_string());
    app_config.simple_dns_item.direct_dns = Some("tls://dot.example".to_string());
    app_config.simple_dns_item.remote_dns = Some("https://remote.example/dns-query".to_string());
    app_config.simple_dns_item.direct_expected_ips = Some("geoip:cn,192.0.2.0/24".to_string());
    app_config.simple_dns_item.add_common_hosts = Some(false);
    app_config.simple_dns_item.strategy4_freedom = Some("UseIPv4".to_string());
    let mut context = test_context(app_config, base_remote_node());
    context.routing_item = Some(RoutingItem {
        rule_set: vec![RulesItem {
            outbound_tag: Some(DIRECT_TAG.to_string()),
            domain: Some(vec!["geosite:cn".to_string()]),
            rule_type: Some(RuleType::DNS),
            ..RulesItem::default()
        }],
        ..RoutingItem::default()
    });

    let dns = generate_singbox_config(&context)
        .expect("sing-box config should generate")
        .dns
        .expect("DNS config should be generated");
    let bootstrap = dns
        .servers
        .iter()
        .find(|server| server.tag == SINGBOX_LOCAL_DNS_TAG)
        .expect("bootstrap DNS server");
    assert_eq!(bootstrap.r#type, "udp");
    assert_eq!(bootstrap.server.as_deref(), Some("223.5.5.5"));
    assert_eq!(bootstrap.server_port, Some(5353));

    let direct = dns
        .servers
        .iter()
        .find(|server| server.tag == SINGBOX_DIRECT_DNS_TAG)
        .expect("direct DNS server");
    assert_eq!(direct.r#type, "tls");
    assert_eq!(direct.server.as_deref(), Some("dot.example"));
    assert_eq!(direct.server_port, None);
    assert_eq!(
        direct.domain_resolver.as_deref(),
        Some(SINGBOX_LOCAL_DNS_TAG)
    );

    let remote = dns
        .servers
        .iter()
        .find(|server| server.tag == SINGBOX_REMOTE_DNS_TAG)
        .expect("remote DNS server");
    assert_eq!(remote.r#type, "https");
    assert_eq!(remote.path.as_deref(), Some("/dns-query"));
    assert_eq!(remote.detour.as_deref(), Some(PROXY_TAG));

    // The expected-IP split clones the direct rule so the matching geosite half
    // is constrained to the expected regions and CIDRs.
    let expected = dns
        .rules
        .iter()
        .find(|rule| {
            rule.rule_set
                .as_ref()
                .is_some_and(|items| items.iter().any(|item| item == "geoip-cn"))
        })
        .expect("expected-ip DNS rule");
    assert_eq!(expected.server.as_deref(), Some(SINGBOX_DIRECT_DNS_TAG));
    assert_eq!(
        expected.ip_cidr.as_ref(),
        Some(&vec!["192.0.2.0/24".to_string()])
    );
    assert_eq!(expected.strategy.as_deref(), Some("prefer_ipv4"));
}

#[test]
fn singbox_wireguard_reserved_requires_exactly_three_bytes() {
    assert_eq!(
        parse_wireguard_reserved(Some("1, 2, 255")),
        Some(vec![1, 2, 255])
    );

    for value in ["1,2", "1,2,3,4", "1,x,3", "1,256,3", "1,,2,3"] {
        assert_eq!(parse_wireguard_reserved(Some(value)), None);
    }
}

#[test]
fn singbox_outbound_live_protocol_matrix_serializes_without_nulls() {
    let cases = vec![
        (
            "vmess",
            ProfileItem {
                index_id: "vmess".to_string(),
                protocol: ProfileProtocol::Vmess {
                    server: endpoint("server.example", 443),
                    uuid: "00000000-0000-0000-0000-000000000021".to_string(),
                    cipher: Some(DEFAULT_SECURITY.to_string()),
                },
                ..base_remote_node()
            },
        ),
        (
            "shadowsocks",
            ProfileItem {
                index_id: "ss".to_string(),
                protocol: ProfileProtocol::Shadowsocks {
                    server: endpoint("server.example", 443),
                    password: "secret".to_string(),
                    method: "2022-blake3-aes-128-gcm".to_string(),
                    udp_over_tcp: false,
                },
                ..base_remote_node()
            },
        ),
        ("socks", socks_node("socks", "socks")),
        (
            "http",
            ProfileItem {
                index_id: "http".to_string(),
                protocol: ProfileProtocol::Http {
                    server: endpoint("server.example", 443),
                    username: "user".to_string(),
                    password: "pass".to_string(),
                },
                ..base_remote_node()
            },
        ),
        (
            "vless",
            ProfileItem {
                index_id: "vless".to_string(),
                protocol: ProfileProtocol::Vless {
                    server: endpoint("server.example", 443),
                    uuid: "00000000-0000-0000-0000-000000000022".to_string(),
                    flow: None,
                    encryption: Some("none".to_string()),
                },
                ..base_remote_node()
            },
        ),
        (
            "trojan",
            ProfileItem {
                index_id: "trojan".to_string(),
                protocol: ProfileProtocol::Trojan {
                    server: endpoint("server.example", 443),
                    password: "secret".to_string(),
                },
                ..base_remote_node()
            },
        ),
        (
            "hysteria2",
            ProfileItem {
                index_id: "hy2".to_string(),
                protocol: ProfileProtocol::Hysteria2 {
                    server: endpoint("server.example", 443),
                    password: "secret".to_string(),
                    port_hops: Some("443,8443-8445".to_string()),
                    obfuscation_password: Some("obfs".to_string()),
                },
                ..base_remote_node()
            },
        ),
        (
            "tuic",
            ProfileItem {
                index_id: "tuic".to_string(),
                protocol: ProfileProtocol::Tuic {
                    server: endpoint("server.example", 443),
                    uuid: "00000000-0000-0000-0000-000000000023".to_string(),
                    password: "secret".to_string(),
                    congestion_control: Some("bbr".to_string()),
                },
                ..base_remote_node()
            },
        ),
        (
            "anytls",
            ProfileItem {
                index_id: "anytls".to_string(),
                protocol: ProfileProtocol::Anytls {
                    server: endpoint("server.example", 443),
                    password: "secret".to_string(),
                },
                ..base_remote_node()
            },
        ),
        (
            "naive",
            ProfileItem {
                index_id: "naive".to_string(),
                protocol: ProfileProtocol::Naive {
                    server: endpoint("server.example", 443),
                    username: "user".to_string(),
                    password: "pass".to_string(),
                    quic: true,
                    congestion_control: Some("bbr".to_string()),
                    insecure_concurrency: Some(2),
                    udp_over_tcp: false,
                },
                ..base_remote_node()
            },
        ),
    ];

    for (expected_type, node) in cases {
        let generated = generate_singbox_config(&test_context(AppConfig::default(), node))
            .expect("sing-box config should generate");
        let proxy = generated
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == PROXY_TAG)
            .expect("proxy outbound");
        assert_eq!(proxy.r#type, expected_type);
        assert_no_nulls(
            &serde_json::to_value(proxy)
                .expect("sing-box protocol matrix outbound should serialize to JSON"),
        );
    }

    let wireguard = ProfileItem {
        index_id: "wg".to_string(),
        protocol: ProfileProtocol::WireGuard {
            server: endpoint("server.example", 443),
            private_key: "private-key".to_string(),
            peer_public_key: Some("public-key".to_string()),
            preshared_key: None,
            interface_address: Some("172.16.0.2/32,fd00::2/128".to_string()),
            allowed_ips: None,
            reserved: None,
            mtu: None,
        },
        transport: None,
        tls: None,
        ..base_remote_node()
    };
    let generated = generate_singbox_config(&test_context(AppConfig::default(), wireguard))
        .expect("sing-box config should generate");
    assert_eq!(generated.endpoints.len(), 1);
    assert_eq!(generated.endpoints[0].r#type, "wireguard");
    assert_no_nulls(
        &serde_json::to_value(&generated.endpoints[0])
            .expect("sing-box wireguard endpoint should serialize to JSON"),
    );
}

#[test]
fn singbox_dns_fakeip_typed_schema_and_rulesets_match_golden() {
    let (dns_context, _) = singbox_routing_dns_snapshot_contexts();
    let dns_generated =
        generate_singbox_config(&dns_context).expect("sing-box config should generate");
    let dns_value = serde_json::to_value(
        dns_generated
            .dns
            .as_ref()
            .expect("sing-box DNS config should be generated"),
    )
    .expect("sing-box DNS config should serialize to JSON");
    assert_no_nulls(&dns_value);
    let expected_dns: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/singbox/dns/fakeip_typed.json"
    ))
    .expect("sing-box fakeip DNS golden fixture should parse as JSON");
    golden::assert_json_eq("singbox-dns-fakeip-typed", &expected_dns, &dns_value);

    let ruleset_value = serde_json::to_value(
        dns_generated
            .route
            .rule_set
            .as_ref()
            .expect("sing-box DNS rulesets should be generated"),
    )
    .expect("sing-box DNS rulesets should serialize to JSON");
    let expected_ruleset: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/singbox/route/rulesets_from_dns.json"
    ))
    .expect("sing-box DNS ruleset golden fixture should parse as JSON");
    golden::assert_json_eq(
        "singbox-rulesets-from-dns",
        &expected_ruleset,
        &ruleset_value,
    );
}

#[test]
fn singbox_ruleset_generation_prefers_resolved_local_asset_paths() {
    let (mut dns_context, _) = singbox_routing_dns_snapshot_contexts();
    dns_context.singbox_ruleset_paths.insert(
        "geosite-cn".to_string(),
        "/tmp/VoyaVPN/bin/srss/geosite-cn.srs".to_string(),
    );

    let generated = generate_singbox_config(&dns_context).expect("sing-box config should generate");
    let rule_set = generated.route.rule_set.expect("rulesets");
    let local = rule_set
        .iter()
        .find(|ruleset| ruleset.tag.as_deref() == Some("geosite-cn"))
        .expect("geosite-cn");
    let remote = rule_set
        .iter()
        .find(|ruleset| ruleset.tag.as_deref() == Some("geosite-google"))
        .expect("geosite-google");

    assert_eq!(local.r#type.as_deref(), Some("local"));
    assert_eq!(
        local.path.as_deref(),
        Some("/tmp/VoyaVPN/bin/srss/geosite-cn.srs")
    );
    assert_eq!(local.url, None);
    assert_eq!(remote.r#type.as_deref(), Some("remote"));
    assert_eq!(remote.url.as_deref(), Some("https://raw.githubusercontent.com/2dust/sing-box-rules/rule-set-geosite/geosite-google.srs"));
    assert_eq!(remote.download_detour.as_deref(), Some(PROXY_TAG));
}

#[test]
fn singbox_invalid_inline_custom_rulesets_are_reported() {
    let (mut dns_context, _) = singbox_routing_dns_snapshot_contexts();
    dns_context
        .routing_item
        .as_mut()
        .expect("routing item")
        .custom_ruleset_path4_singbox = "[{\"tag\":\"geosite-cn\"}]".to_string();

    let error = generate_singbox_config(&dns_context)
        .expect_err("missing custom ruleset fields should fail generation");
    assert!(matches!(
        error,
        SingboxConfigError::CustomRulesetMissingRequiredFields { index: 0 }
    ));

    dns_context
        .routing_item
        .as_mut()
        .expect("routing item")
        .custom_ruleset_path4_singbox = "[{\"tag\":\"geosite-cn\"}".to_string();
    let error = generate_singbox_config(&dns_context)
        .expect_err("invalid custom ruleset JSON should fail generation");
    assert!(matches!(error, SingboxConfigError::CustomRulesetJson(_)));
}

#[test]
fn singbox_negative_ip_rules_use_and_and_skip_negative_only_rules() {
    let mut context = test_context(AppConfig::default(), base_remote_node());
    context.routing_item = Some(RoutingItem {
        rule_set: vec![
            RulesItem {
                outbound_tag: Some(DIRECT_TAG.to_string()),
                ip: Some(vec!["10.0.0.0/8".to_string(), "!10.1.0.0/16".to_string()]),
                ..RulesItem::default()
            },
            RulesItem {
                outbound_tag: Some(BLOCK_TAG.to_string()),
                ip: Some(vec!["!geoip:private".to_string()]),
                port: Some("443".to_string()),
                ..RulesItem::default()
            },
        ],
        ..RoutingItem::default()
    });

    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    let logical_rule = generated
        .route
        .rules
        .iter()
        .find(|rule| {
            rule.r#type.as_deref() == Some("logical")
                && rule.outbound.as_deref() == Some(DIRECT_TAG)
        })
        .expect("logical negative IP rule");
    assert_eq!(logical_rule.mode.as_deref(), Some("and"));
    let nested = logical_rule.rules.as_ref().expect("nested rules");
    assert_eq!(nested.len(), 2);
    assert_eq!(
        nested[0].ip_cidr.as_ref(),
        Some(&vec!["10.0.0.0/8".to_string()])
    );
    assert_eq!(nested[1].invert, Some(true));
    assert_eq!(
        nested[1].ip_cidr.as_ref(),
        Some(&vec!["10.1.0.0/16".to_string()])
    );
    assert!(!generated.route.rules.iter().any(|rule| {
        rule.action.as_deref() == Some("reject") && rule.port.as_ref() == Some(&vec![443])
    }));
}

#[test]
fn singbox_wireguard_uses_allowed_ips_and_rejects_empty_public_key() {
    let wireguard = ProfileItem {
        index_id: "wg".to_string(),
        protocol: ProfileProtocol::WireGuard {
            server: endpoint("server.example", 443),
            private_key: "private-key".to_string(),
            peer_public_key: Some("public-key".to_string()),
            preshared_key: None,
            interface_address: None,
            allowed_ips: Some("10.0.0.0/8,192.168.0.0/16".to_string()),
            reserved: None,
            mtu: None,
        },
        transport: None,
        tls: None,
        ..base_remote_node()
    };
    let generated = generate_singbox_config(&test_context(AppConfig::default(), wireguard))
        .expect("sing-box config should generate");
    assert_eq!(
        generated.endpoints[0].peers[0].allowed_ips,
        vec!["10.0.0.0/8".to_string(), "192.168.0.0/16".to_string()]
    );

    let missing_public_key = ProfileItem {
        index_id: "wg-missing-key".to_string(),
        protocol: ProfileProtocol::WireGuard {
            server: endpoint("server.example", 443),
            private_key: "private-key".to_string(),
            peer_public_key: None,
            preshared_key: None,
            interface_address: None,
            allowed_ips: None,
            reserved: None,
            mtu: None,
        },
        transport: None,
        tls: None,
        ..base_remote_node()
    };
    let error = generate_singbox_config(&test_context(AppConfig::default(), missing_public_key))
        .expect_err("missing WireGuard public key should fail");
    assert!(matches!(
        error,
        SingboxConfigError::MissingWireGuardPublicKey { .. }
    ));
}

#[test]
fn singbox_tun_inbound_and_route_match_golden() {
    let (_, tun_context) = singbox_routing_dns_snapshot_contexts();
    let generated = generate_singbox_config(&tun_context).expect("sing-box config should generate");
    let inbounds_value = serde_json::to_value(&generated.inbounds)
        .expect("sing-box tun inbounds should serialize to JSON");
    assert_no_nulls(&inbounds_value);
    let expected_inbounds: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/singbox/inbounds/tun.json"
    ))
    .expect("sing-box tun inbounds golden fixture should parse as JSON");
    golden::assert_json_eq("singbox-tun-inbounds", &expected_inbounds, &inbounds_value);

    let route_value =
        serde_json::to_value(&generated.route).expect("sing-box route should serialize to JSON");
    assert_no_nulls(&route_value);
    let expected_route: Value = serde_json::from_str(include_str!(
        "../../../../tests/golden/singbox/route/tun.json"
    ))
    .expect("sing-box tun route golden fixture should parse as JSON");
    golden::assert_json_eq("singbox-tun-route", &expected_route, &route_value);
}

#[test]
fn singbox_macos_tun_inbound_lets_singbox_allocate_utun() {
    let mut config = AppConfig::default();
    config.tun_mode_item.enable_tun = true;
    config.tun_mode_item.mtu = 9000;
    config.tun_mode_item.strict_route = true;
    let mut context = test_context(config, base_remote_node());
    context.is_tun_enabled = true;
    context.platform = CoreGenPlatform::MacOS;

    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    let tun = generated
        .inbounds
        .iter()
        .find(|inbound| inbound.tag == SINGBOX_TUN_INBOUND_TAG)
        .expect("TUN inbound should be generated");

    assert_eq!(tun.interface_name, None);
    assert_eq!(
        tun.address.as_ref(),
        Some(&vec![
            "172.18.0.1/30".to_string(),
            "fdfe:dcba:9876::1/126".to_string()
        ])
    );
    assert_eq!(tun.mtu, Some(1500));
    assert_eq!(tun.strict_route, Some(false));
    let http_proxy = tun
        .platform
        .as_ref()
        .and_then(|platform| platform.http_proxy.as_ref())
        .expect("macOS TUN platform HTTP proxy");
    assert!(http_proxy.enabled);
    assert_eq!(http_proxy.server.as_deref(), Some(LOOPBACK));
    assert_eq!(http_proxy.server_port, Some(crate::DEFAULT_LOCAL_PORT));
}

#[test]
fn singbox_global_mode_precedes_user_rules_and_no_domain_list_is_injected() {
    let mut app_config = AppConfig::default();
    app_config.tun_mode_item.enable_tun = true;
    app_config.tun_mode_item.enable_ipv6_address = false;
    let mut context = test_context(app_config, base_remote_node());
    context.is_tun_enabled = true;
    context.routing_item = Some(RoutingItem {
        rule_set: vec![RulesItem {
            outbound_tag: Some(DIRECT_TAG.to_string()),
            port: Some("0-65535".to_string()),
            rule_type: Some(RuleType::Routing),
            ..RulesItem::default()
        }],
        ..RoutingItem::default()
    });

    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    let route_index = |label: &str, matches: fn(&SingboxRule) -> bool| {
        generated
            .route
            .rules
            .iter()
            .position(matches)
            .unwrap_or_else(|| panic!("missing {label} route rule"))
    };
    let sniff_index = route_index("sniff", |rule| rule.action.as_deref() == Some("sniff"));
    let dns_hijack_index = route_index("DNS hijack", |rule| {
        rule.action.as_deref() == Some("hijack-dns")
    });
    let global_mode_index = route_index("Global mode", |rule| {
        rule.outbound.as_deref() == Some(PROXY_TAG) && rule.clash_mode.as_deref() == Some("Global")
    });
    let direct_final_index = route_index("direct final", |rule| {
        rule.outbound.as_deref() == Some(DIRECT_TAG)
            && rule.port_range.as_ref() == Some(&vec!["0:65535".to_string()])
    });
    assert!(sniff_index < global_mode_index);
    assert!(dns_hijack_index < global_mode_index);
    assert!(global_mode_index < direct_final_index);
    assert!(generated
        .route
        .rules
        .iter()
        .all(|rule| rule.clash_mode.as_deref() != Some("Direct") && !names_ai_service(rule)));

    let dns = generated.dns.expect("DNS config should be generated");
    assert_eq!(dns.reverse_mapping, Some(true));
    assert!(dns.rules.iter().any(|rule| {
        rule.server.as_deref() == Some(SINGBOX_REMOTE_DNS_TAG)
            && rule.clash_mode.as_deref() == Some("Global")
    }));
    assert!(dns
        .rules
        .iter()
        .all(|rule| rule.clash_mode.as_deref() != Some("Direct") && !names_ai_service(rule)));
}

#[test]
fn singbox_speedtest_config_adds_mixed_inbound_proxy_and_route_per_entry() {
    let entries = vec![
        SpeedtestConfigEntry {
            index_id: "a".to_string(),
            port: 12000,
            context: test_context(AppConfig::default(), socks_node("a", "node-a")),
        },
        SpeedtestConfigEntry {
            index_id: "b".to_string(),
            port: 12001,
            context: test_context(AppConfig::default(), socks_node("b", "node-b")),
        },
    ];

    let generated: Value = serde_json::from_str(
        &generate_singbox_speedtest_config_json(&entries)
            .expect("sing-box speedtest config should serialize"),
    )
    .expect("sing-box speedtest config should parse as JSON");
    for port in [12000, 12001] {
        let inbound_tag = format!("mixed{port}");
        let proxy_tag = format!("proxy{port}");
        assert!(generated["inbounds"].as_array().is_some_and(|inbounds| {
            inbounds.iter().any(|inbound| {
                inbound["tag"] == inbound_tag
                    && inbound["listen"] == LOOPBACK
                    && inbound["listen_port"] == port
                    && inbound["type"] == "mixed"
            })
        }));
        assert!(generated["outbounds"].as_array().is_some_and(|outbounds| {
            outbounds
                .iter()
                .any(|outbound| outbound["tag"] == proxy_tag)
        }));
        assert!(generated["route"]["rules"].as_array().is_some_and(|rules| {
            rules.iter().any(|rule| {
                rule["inbound"].as_array().is_some_and(|tags| {
                    tags.iter()
                        .any(|tag| tag == &Value::String(inbound_tag.clone()))
                }) && rule["outbound"] == proxy_tag
            })
        }));
    }
    assert_eq!(
        generated.pointer("/dns/final").and_then(Value::as_str),
        Some(SINGBOX_DIRECT_DNS_TAG)
    );
}

#[test]
fn singbox_log_level_accepts_current_levels_and_defaults_invalid_values() {
    // (stored app level, generated `log.level`, generated `log.disabled`)
    let cases = [
        (crate::DEFAULT_LOG_LEVEL, "warn", None),
        ("warn", "warn", None),
        ("trace", "trace", None),
        ("debug", "debug", None),
        ("info", "info", None),
        ("error", "error", None),
        ("  Info  ", "info", None),
        ("none", "warn", Some(true)),
        ("verbose", "warn", None),
        ("", "warn", None),
    ];

    for (configured, expected_level, expected_disabled) in cases {
        let mut app_config = AppConfig::default();
        app_config.core_basic_item.loglevel = configured.to_string();
        let generated =
            generate_singbox_config(&test_context(app_config, socks_node("log", "Log")))
                .expect("sing-box config should generate");
        let log = generated.log.as_ref().expect("sing-box log section");

        assert_eq!(log.level, expected_level, "log level for `{configured}`");
        assert_eq!(
            log.disabled, expected_disabled,
            "log disabled for `{configured}`"
        );
    }
}

#[test]
fn singbox_default_log_level_is_generated_as_warn() {
    let value = generate_singbox_config_value(&test_context(
        AppConfig::default(),
        socks_node("log", "Log"),
    ))
    .expect("sing-box config should generate");

    assert_eq!(
        value.pointer("/log/level"),
        Some(&Value::String("warn".to_string()))
    );
    // `none` is the only level that disables logging, so the default must not.
    assert_eq!(value.pointer("/log/disabled"), None);
}
/// The AI service list is ordinary routing data now; nothing injects it.
fn names_ai_service(rule: &SingboxRule) -> bool {
    rule.domain_suffix
        .as_ref()
        .is_some_and(|suffixes| suffixes.iter().any(|suffix| suffix == "anthropic.com"))
}

fn singbox_routing_dns_snapshot_contexts() -> (CoreConfigContext, CoreConfigContext) {
    let mut dns_config = AppConfig::default();
    dns_config.simple_dns_item.fake_ip = Some(true);
    dns_config.simple_dns_item.global_fake_ip = Some(true);
    dns_config.simple_dns_item.direct_dns = Some("https://resolver.example/dns-query".to_string());
    dns_config.simple_dns_item.remote_dns =
        Some("https://cloudflare-dns.com/dns-query".to_string());
    dns_config.simple_dns_item.hosts =
        Some("resolver.example 1.1.1.1\nblock.test #3\ncname.test target.example".to_string());
    dns_config.simple_dns_item.strategy4_freedom = Some("UseIPv4".to_string());
    dns_config.simple_dns_item.strategy4_proxy = Some("UseIPv6".to_string());
    dns_config.simple_dns_item.direct_expected_ips = Some("geoip:cn,192.0.2.0/24".to_string());
    let mut dns_context = test_context(dns_config, base_remote_node());
    dns_context.routing_item = Some(RoutingItem {
        rule_set: vec![
            RulesItem {
                outbound_tag: Some(DIRECT_TAG.to_string()),
                domain: Some(vec!["geosite:cn".to_string()]),
                rule_type: Some(RuleType::DNS),
                ..RulesItem::default()
            },
            RulesItem {
                outbound_tag: Some(PROXY_TAG.to_string()),
                domain: Some(vec!["geosite:google".to_string()]),
                rule_type: Some(RuleType::DNS),
                ..RulesItem::default()
            },
        ],
        ..RoutingItem::default()
    });

    let mut tun_config = AppConfig::default();
    tun_config.tun_mode_item.enable_tun = true;
    tun_config.tun_mode_item.mtu = 1500;
    tun_config.tun_mode_item.stack = "system".to_string();
    tun_config.tun_mode_item.strict_route = false;
    tun_config.tun_mode_item.enable_ipv6_address = false;
    tun_config.simple_dns_item.add_common_hosts = Some(false);
    tun_config.simple_dns_item.block_binding_query = Some(false);
    let mut tun_context = test_context(tun_config, base_remote_node());
    tun_context.is_tun_enabled = true;

    (dns_context, tun_context)
}

fn test_context(app_config: AppConfig, node: ProfileItem) -> CoreConfigContext {
    let mut all_proxies_map = BTreeMap::new();
    all_proxies_map.insert(node.index_id.clone(), node.clone());
    let simple_dns_item = app_config.simple_dns_item.clone();
    CoreConfigContext {
        node,
        run_core_type: CoreType::sing_box,
        app_config,
        simple_dns_item,
        all_proxies_map,
        platform: CoreGenPlatform::Linux,
        ..CoreConfigContext::default()
    }
}

fn base_remote_node() -> ProfileItem {
    ProfileItem {
        remarks: "remote".to_string(),
        protocol: ProfileProtocol::Vmess {
            server: endpoint("server.example", 443),
            uuid: String::new(),
            cipher: None,
        },
        transport: Some(raw_transport()),
        tls: Some(tls_settings(TlsMode::Tls, Some("server.example"))),
        ..ProfileItem::default()
    }
}

fn socks_node(index_id: &str, remarks: &str) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        remarks: remarks.to_string(),
        protocol: ProfileProtocol::Socks {
            server: endpoint(LOOPBACK, 1080),
            username: "user".to_string(),
            password: "pass".to_string(),
        },
        transport: Some(raw_transport()),
        ..ProfileItem::default()
    }
}

fn generated_proxy_outbound_value(app_config: AppConfig, node: ProfileItem) -> Value {
    let generated = generate_singbox_config(&test_context(app_config, node))
        .expect("sing-box config should generate");
    let proxy = generated
        .outbounds
        .iter()
        .find(|outbound| outbound.tag == PROXY_TAG)
        .expect("proxy outbound");
    let value = serde_json::to_value(proxy).expect("sing-box outbound should serialize to JSON");
    assert_no_nulls(&value);
    value
}

fn sample_protocol(config_type: ConfigType) -> ProfileProtocol {
    let server = endpoint("server.example", 443);
    match config_type {
        ConfigType::VMess => ProfileProtocol::Vmess {
            server,
            uuid: "00000000-0000-0000-0000-000000000041".to_string(),
            cipher: Some(DEFAULT_SECURITY.to_string()),
        },
        ConfigType::Shadowsocks => ProfileProtocol::Shadowsocks {
            server,
            password: "secret".to_string(),
            method: "aes-128-gcm".to_string(),
            udp_over_tcp: false,
        },
        ConfigType::SOCKS => ProfileProtocol::Socks {
            server,
            username: "user".to_string(),
            password: "pass".to_string(),
        },
        ConfigType::VLESS => ProfileProtocol::Vless {
            server,
            uuid: "00000000-0000-0000-0000-000000000042".to_string(),
            flow: None,
            encryption: Some("none".to_string()),
        },
        ConfigType::Trojan => ProfileProtocol::Trojan {
            server,
            password: "secret".to_string(),
        },
        ConfigType::Hysteria2 => ProfileProtocol::Hysteria2 {
            server,
            password: "secret".to_string(),
            port_hops: None,
            obfuscation_password: None,
        },
        ConfigType::TUIC => ProfileProtocol::Tuic {
            server,
            uuid: "00000000-0000-0000-0000-000000000043".to_string(),
            password: "secret".to_string(),
            congestion_control: None,
        },
        ConfigType::WireGuard => ProfileProtocol::WireGuard {
            server,
            private_key: "private-key".to_string(),
            peer_public_key: Some("public-key".to_string()),
            preshared_key: None,
            interface_address: None,
            allowed_ips: None,
            reserved: None,
            mtu: None,
        },
        ConfigType::HTTP => ProfileProtocol::Http {
            server,
            username: "user".to_string(),
            password: "pass".to_string(),
        },
        ConfigType::Anytls => ProfileProtocol::Anytls {
            server,
            password: "secret".to_string(),
        },
        ConfigType::Naive => ProfileProtocol::Naive {
            server,
            username: "user".to_string(),
            password: "pass".to_string(),
            quic: false,
            congestion_control: None,
            insecure_concurrency: None,
            udp_over_tcp: false,
        },
    }
}

fn endpoint(address: &str, port: i32) -> ServerEndpoint {
    ServerEndpoint {
        address: address.to_string(),
        port,
    }
}

fn raw_transport() -> ProfileTransport {
    ProfileTransport::Tcp {
        header: None,
        host: None,
        path: None,
    }
}

fn tls_settings(mode: TlsMode, server_name: Option<&str>) -> TlsSettings {
    TlsSettings {
        mode,
        server_name: server_name.map(str::to_string),
        alpn: Vec::new(),
        reality_public_key: None,
        reality_short_id: None,
        certificate_pem: None,
        ech_config: Vec::new(),
    }
}

fn assert_no_nulls(value: &Value) {
    match value {
        Value::Null => panic!("sing-box JSON must not contain null"),
        Value::Array(items) => {
            for item in items {
                assert_no_nulls(item);
            }
        }
        Value::Object(object) => {
            for item in object.values() {
                assert_no_nulls(item);
            }
        }
        Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
