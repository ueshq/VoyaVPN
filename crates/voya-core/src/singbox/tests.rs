use super::*;
use crate::testutil::{
    base_remote_node, endpoint, linux_context as test_context, raw_transport, socks_node,
    tls_settings as full_tls_settings,
};
use crate::{golden, CoreGenPlatform, RoutingItem, TlsSettings};

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
        assert!(crate::validate_node(&node).success());

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
fn singbox_reality_client_always_carries_utls() {
    // sing-box rejects a REALITY client without uTLS, and the global
    // fingerprint setting defaults to empty.
    let reality_node = ProfileItem {
        tls: Some(TlsSettings {
            reality_public_key: Some("reality-public-key".to_string()),
            ..tls_settings(TlsMode::Reality, Some("server.example"))
        }),
        ..base_remote_node()
    };
    let context = test_context(AppConfig::default(), reality_node.clone());
    let utls = build_outbound(&context, &reality_node)
        .tls
        .and_then(|tls| tls.utls)
        .expect("REALITY needs uTLS");
    assert_eq!(utls.fingerprint, REALITY_FALLBACK_FINGERPRINT);

    let mut config = AppConfig::default();
    config.core_basic_item.def_fingerprint = "firefox".to_string();
    let context = test_context(config, reality_node.clone());
    let utls = build_outbound(&context, &reality_node)
        .tls
        .and_then(|tls| tls.utls)
        .expect("REALITY keeps the configured fingerprint");
    assert_eq!(utls.fingerprint, "firefox");

    let plain_tls_node = ProfileItem {
        tls: Some(tls_settings(TlsMode::Tls, Some("server.example"))),
        ..base_remote_node()
    };
    let context = test_context(AppConfig::default(), plain_tls_node.clone());
    assert!(build_outbound(&context, &plain_tls_node)
        .tls
        .expect("tls settings should be generated")
        .utls
        .is_none());
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

    for (strategy, name) in [
        (DnsStrategy::PreferIpv4, "prefer_ipv4"),
        (DnsStrategy::PreferIpv6, "prefer_ipv6"),
        (DnsStrategy::Ipv4Only, "ipv4_only"),
        (DnsStrategy::Ipv6Only, "ipv6_only"),
    ] {
        assert_eq!(strategy.singbox_name(), name);
    }

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
    app_config.simple_dns_item.direct_strategy = Some(DnsStrategy::PreferIpv4);
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
    // is constrained to the expected regions and CIDRs, strategy included:
    // IPv6 is on by default, so the explicit direct strategy reaches it.
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
    // TUN is always dual-stack so IPv6 destinations are captured (and can be
    // rejected) instead of leaking out of the physical NIC.
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
fn singbox_tun_address_is_always_dual_stack() {
    for ipv6 in [false, true] {
        let mut config = AppConfig::default();
        config.tun_mode_item.enable_tun = true;
        config.tun_mode_item.enable_ipv6_address = ipv6;
        let mut context = test_context(config, base_remote_node());
        context.is_tun_enabled = true;
        context.platform = CoreGenPlatform::MacOS;

        let generated = generate_singbox_config(&context).expect("sing-box config should generate");
        let tun = generated
            .inbounds
            .iter()
            .find(|inbound| inbound.tag == SINGBOX_TUN_INBOUND_TAG)
            .expect("TUN inbound should be generated");
        assert_eq!(
            tun.address.as_ref(),
            Some(&vec![
                "172.18.0.1/30".to_string(),
                "fdfe:dcba:9876::1/126".to_string()
            ]),
            "ipv6={ipv6}"
        );
    }
}

#[test]
fn singbox_dns_strategy_yields_to_the_ipv6_master_switch() {
    // Off: every rule-level strategy is suppressed; the top-level covers it.
    for explicit in [
        None,
        Some(DnsStrategy::PreferIpv4),
        Some(DnsStrategy::PreferIpv6),
        Some(DnsStrategy::Ipv4Only),
        Some(DnsStrategy::Ipv6Only),
    ] {
        let mut config = AppConfig::default();
        config.tun_mode_item.enable_ipv6_address = false;
        let context = test_context(config, base_remote_node());
        assert_eq!(context.ipv6_mode(), Ipv6Mode::Off);
        assert_eq!(
            direct_dns_strategy(&context, explicit),
            None,
            "IPv6 off suppresses direct {explicit:?}"
        );
        assert_eq!(
            proxy_dns_strategy(&context, explicit),
            None,
            "IPv6 off suppresses proxy {explicit:?}"
        );
    }

    // On: the explicit preference is mapped through on both paths.
    let mut config = AppConfig::default();
    config.tun_mode_item.enable_ipv6_address = true;
    let context = test_context(config, base_remote_node());
    assert_eq!(context.ipv6_mode(), Ipv6Mode::Full);
    for strategy in [direct_dns_strategy, proxy_dns_strategy] {
        assert_eq!(strategy(&context, None), None);
        assert_eq!(
            strategy(&context, Some(DnsStrategy::PreferIpv6)).as_deref(),
            Some("prefer_ipv6")
        );
        assert_eq!(
            strategy(&context, Some(DnsStrategy::Ipv4Only)).as_deref(),
            Some("ipv4_only")
        );
    }
}

#[test]
fn singbox_node_without_ipv6_egress_keeps_ipv6_on_the_direct_path_only() {
    let mut config = AppConfig::default();
    config.tun_mode_item.enable_ipv6_address = true;
    let mut context = test_context(config, base_remote_node());
    context.ipv6_egress_unsupported = true;
    assert_eq!(context.ipv6_mode(), Ipv6Mode::DirectOnly);

    for explicit in [
        None,
        Some(DnsStrategy::PreferIpv6),
        Some(DnsStrategy::Ipv6Only),
    ] {
        assert_eq!(
            proxy_dns_strategy(&context, explicit).as_deref(),
            Some("ipv4_only"),
            "the node cannot reach IPv6 whatever {explicit:?} asks for"
        );
    }
    assert_eq!(direct_dns_strategy(&context, None), None);
    assert_eq!(
        direct_dns_strategy(&context, Some(DnsStrategy::PreferIpv6)).as_deref(),
        Some("prefer_ipv6")
    );

    // The switch still wins: off is off whatever the node can do.
    context.app_config.tun_mode_item.enable_ipv6_address = false;
    assert_eq!(context.ipv6_mode(), Ipv6Mode::Off);
}

#[test]
fn singbox_ipv6_off_puts_ipv4_only_on_top_and_drops_rule_strategies() {
    let mut config = AppConfig::default();
    config.tun_mode_item.enable_ipv6_address = false;
    config.simple_dns_item.proxy_strategy = Some(DnsStrategy::PreferIpv6);
    config.simple_dns_item.direct_strategy = Some(DnsStrategy::PreferIpv4);
    config.simple_dns_item.add_common_hosts = Some(false);
    config.simple_dns_item.block_binding_query = Some(false);
    let mut context = test_context(config, base_remote_node());
    context.routing_item = Some(RoutingItem {
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

    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    let dns = generated.dns.expect("DNS config should be generated");
    assert_eq!(dns.strategy.as_deref(), Some("ipv4_only"));
    assert!(
        dns.rules.iter().all(|rule| rule.strategy.is_none()),
        "IPv6 off: no rule-level strategy (got {:?})",
        dns.rules
            .iter()
            .filter_map(|rule| rule.strategy.as_deref())
            .collect::<Vec<_>>()
    );
    // No trailing catch-all carrying a strategy.
    assert!(
        !dns.rules.iter().any(|rule| {
            rule.server.as_deref() == Some(SINGBOX_REMOTE_DNS_TAG)
                && rule.clash_mode.is_none()
                && rule.rule_set.is_none()
                && rule.domain.is_none()
                && rule.strategy.is_some()
        }),
        "IPv6 off: no trailing strategy catch-all"
    );
}

#[test]
fn singbox_ipv6_on_emits_explicit_rule_strategies_and_the_final_catch_all() {
    let mut config = AppConfig::default();
    config.tun_mode_item.enable_ipv6_address = true;
    config.simple_dns_item.proxy_strategy = Some(DnsStrategy::PreferIpv6);
    config.simple_dns_item.direct_strategy = Some(DnsStrategy::PreferIpv4);
    config.simple_dns_item.add_common_hosts = Some(false);
    config.simple_dns_item.block_binding_query = Some(false);
    let mut context = test_context(config, base_remote_node());
    context.routing_item = Some(RoutingItem {
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

    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    let dns = generated.dns.expect("DNS config should be generated");
    assert_eq!(dns.strategy, None, "IPv6 on: no top-level strategy");

    let global = dns
        .rules
        .iter()
        .find(|rule| rule.clash_mode.as_deref() == Some("Global"))
        .expect("Global DNS rule");
    assert_eq!(global.strategy.as_deref(), Some("prefer_ipv6"));

    let direct_rule = dns
        .rules
        .iter()
        .find(|rule| {
            rule.server.as_deref() == Some(SINGBOX_DIRECT_DNS_TAG)
                && rule
                    .rule_set
                    .as_ref()
                    .is_some_and(|items| items.iter().any(|item| item == "geosite-cn"))
        })
        .expect("direct DNS rule");
    assert_eq!(direct_rule.strategy.as_deref(), Some("prefer_ipv4"));

    let proxy_rule = dns
        .rules
        .iter()
        .find(|rule| {
            rule.server.as_deref() == Some(SINGBOX_REMOTE_DNS_TAG)
                && rule
                    .rule_set
                    .as_ref()
                    .is_some_and(|items| items.iter().any(|item| item == "geosite-google"))
        })
        .expect("proxy DNS rule");
    assert_eq!(proxy_rule.strategy.as_deref(), Some("prefer_ipv6"));

    let catch_all = dns
        .rules
        .iter()
        .rev()
        .find(|rule| {
            rule.server.as_deref() == Some(SINGBOX_REMOTE_DNS_TAG)
                && rule.clash_mode.is_none()
                && rule.rule_set.is_none()
                && rule.domain.is_none()
        })
        .expect("final remote catch-all");
    assert_eq!(catch_all.strategy.as_deref(), Some("prefer_ipv6"));
}

#[test]
fn singbox_ipv6_reject_lets_direct_rules_claim_ipv6_first() {
    let routing = RoutingItem {
        rule_set: vec![
            RulesItem {
                outbound_tag: Some(PROXY_TAG.to_string()),
                domain: Some(vec!["domain:proxied.example".to_string()]),
                enabled: true,
                ..RulesItem::default()
            },
            RulesItem {
                outbound_tag: Some(DIRECT_TAG.to_string()),
                ip: Some(vec!["geoip:cn".to_string()]),
                enabled: true,
                ..RulesItem::default()
            },
        ],
        ..RoutingItem::default()
    };
    let is_plain_reject = |rule: &SingboxRule| {
        rule.ip_version == Some(6)
            && rule.action.as_deref() == Some("reject")
            && rule.clash_mode.is_none()
    };
    let is_global_reject = |rule: &SingboxRule| {
        rule.ip_version == Some(6)
            && rule.action.as_deref() == Some("reject")
            && rule.clash_mode.as_deref() == Some("Global")
    };

    for (ipv6_enabled, unsupported) in [(false, false), (false, true), (true, true)] {
        for tun in [false, true] {
            {
                let mut app_config = AppConfig::default();
                app_config.tun_mode_item.enable_tun = tun;
                app_config.tun_mode_item.enable_ipv6_address = ipv6_enabled;
                let mut context = test_context(app_config, base_remote_node());
                context.is_tun_enabled = tun;
                context.ipv6_egress_unsupported = unsupported;
                context.routing_item = Some(routing.clone());
                let case = format!("ipv6={ipv6_enabled} unsupported={unsupported} tun={tun}");

                let rules = generate_singbox_config(&context)
                    .expect("sing-box config should generate")
                    .route
                    .rules;
                let position = |label: &str, matches: &dyn Fn(&SingboxRule) -> bool| {
                    rules
                        .iter()
                        .position(matches)
                        .unwrap_or_else(|| panic!("{case}: missing {label}"))
                };
                let global = position("Global mode", &|rule| {
                    rule.outbound.as_deref() == Some(PROXY_TAG)
                        && rule.clash_mode.as_deref() == Some("Global")
                });
                let global_reject = position("Global-mode reject", &is_global_reject);
                let reject = position("IPv6 reject", &is_plain_reject);
                let last_direct = rules
                    .iter()
                    .rposition(|rule| {
                        rule.rule_set.as_deref() == Some(&["geoip-cn".to_string()][..])
                    })
                    .unwrap_or_else(|| panic!("{case}: missing direct rule"));

                assert!(global_reject < global, "{case}: Global mode refuses IPv6");
                assert!(
                    last_direct < reject,
                    "{case}: direct rules claim IPv6 first"
                );
                assert_eq!(
                    reject,
                    rules.len() - 1,
                    "{case}: the reject sits just before final"
                );
                if tun {
                    let hijack = position("DNS hijack", &|rule| {
                        rule.action.as_deref() == Some("hijack-dns")
                    });
                    assert!(hijack < global_reject, "{case}: DNS is unaffected");
                }
            }
        }
    }

    // No routing profile: the reject still guards `final`.
    let mut app_config = AppConfig::default();
    app_config.tun_mode_item.enable_ipv6_address = false;
    let rules = generate_singbox_config(&test_context(app_config, base_remote_node()))
        .expect("sing-box config should generate")
        .route
        .rules;
    assert!(rules.last().is_some_and(is_plain_reject));

    let mut app_config = AppConfig::default();
    app_config.tun_mode_item.enable_ipv6_address = true;
    let mut context = test_context(app_config, base_remote_node());
    context.routing_item = Some(routing);
    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    assert!(
        !generated
            .route
            .rules
            .iter()
            .any(|rule| rule.ip_version == Some(6)),
        "IPv6 on with a capable node: no reject rule"
    );
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
    dns_config.simple_dns_item.direct_strategy = Some(DnsStrategy::PreferIpv4);
    dns_config.simple_dns_item.proxy_strategy = Some(DnsStrategy::PreferIpv6);
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

struct GroupTestEnv;

impl crate::CoreGenEnv for GroupTestEnv {
    fn platform(&self) -> CoreGenPlatform {
        CoreGenPlatform::Linux
    }

    fn get_profile_by_remarks(&self, _remarks: &str) -> Option<ProfileItem> {
        None
    }

    fn get_default_routing(&self, _config: &AppConfig) -> Option<RoutingItem> {
        None
    }

    fn get_local_port(&self, _protocol: InboundProtocol) -> i32 {
        crate::DEFAULT_LOCAL_PORT
    }
}

fn group_member(index_id: &str, remarks: &str, port: i32) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        remarks: remarks.to_string(),
        protocol: ProfileProtocol::Socks {
            server: endpoint("198.51.100.10", port),
            username: String::new(),
            password: String::new(),
        },
        transport: Some(raw_transport()),
        ..ProfileItem::default()
    }
}

#[test]
fn a_group_context_leaves_out_broken_members_and_keeps_the_rest_in_order() {
    let group = crate::PolicyGroupItem {
        id: "g".to_string(),
        name: "Asia".to_string(),
        strategy: crate::GroupStrategy::UrlTest,
        ..crate::PolicyGroupItem::default()
    };
    let members = vec![
        group_member("broken", "Broken", 0),
        group_member("tokyo", "Tokyo", 1080),
        group_member("osaka", "Osaka", 1081),
    ];
    let result = crate::CoreConfigContextBuilder::new(&GroupTestEnv).build_all_for_group(
        &AppConfig::default(),
        &group,
        &members,
    );

    assert!(result.success());
    let context = &result.main_result.context;
    assert_eq!(context.node.index_id, "tokyo");
    let ids: Vec<&str> = context
        .active_outbound_nodes()
        .iter()
        .map(|node| node.index_id.as_str())
        .collect();
    assert_eq!(ids, ["tokyo", "osaka"]);
    assert!(result
        .main_result
        .validator_result
        .warnings
        .iter()
        .any(|warning| warning.scope
            == [crate::ValidationScope::PolicyGroupMember {
                group: "Asia".to_string(),
                member: "Broken".to_string(),
            }]));
}

#[test]
fn a_group_with_no_usable_member_is_rejected_by_name() {
    let group = crate::PolicyGroupItem {
        name: "Empty".to_string(),
        ..crate::PolicyGroupItem::default()
    };
    let result = crate::CoreConfigContextBuilder::new(&GroupTestEnv).build_for_group(
        &AppConfig::default(),
        &group,
        &[group_member("broken", "Broken", 0)],
    );

    assert!(!result.success());
    assert_eq!(
        result
            .validator_result
            .errors
            .last()
            .map(|error| &error.code),
        Some(&crate::ValidationCode::PolicyGroupWithoutValidMembers {
            group: "Empty".to_string(),
        })
    );
    assert!(result.context.policy_group.is_none());
}

#[test]
fn group_outbounds_clamp_their_timing_and_start_on_the_first_member() {
    let members = vec![
        group_member("tokyo", "Tokyo", 1080),
        group_member("osaka", "Osaka", 1081),
    ];
    let mut group = crate::PolicyGroupItem {
        name: "Asia".to_string(),
        selected_profile_id: Some("gone".to_string()),
        ..crate::PolicyGroupItem::default()
    };
    let mut context = test_context(AppConfig::default(), members[0].clone());
    context.policy_group = Some(crate::ContextPolicyGroup {
        group: group.clone(),
        members: members.clone(),
    });
    let config = generate_singbox_config(&context).expect("selector config");
    let proxy = config
        .outbounds
        .iter()
        .find(|outbound| outbound.tag == PROXY_TAG)
        .expect("proxy outbound");
    assert_eq!(proxy.r#type, "selector");
    assert_eq!(proxy.default.as_deref(), Some("Tokyo [tokyo]"));
    assert_eq!(
        proxy.outbounds.as_deref(),
        Some(&["Tokyo [tokyo]".to_string(), "Osaka [osaka]".to_string()][..])
    );

    group.strategy = crate::GroupStrategy::UrlTest;
    group.interval_seconds = Some(1);
    group.tolerance_ms = Some(999_999);
    group.test_url = Some("  ".to_string());
    context.policy_group = Some(crate::ContextPolicyGroup { group, members });
    let config = generate_singbox_config(&context).expect("urltest config");
    let proxy = config
        .outbounds
        .iter()
        .find(|outbound| outbound.tag == PROXY_TAG)
        .expect("proxy outbound");
    assert_eq!(proxy.r#type, "urltest");
    assert_eq!(proxy.interval.as_deref(), Some("30s"));
    assert_eq!(proxy.tolerance, Some(5_000));
    assert_eq!(proxy.url.as_deref(), Some(crate::DEFAULT_GROUP_TEST_URL));
    assert!(config
        .outbounds
        .iter()
        .any(|outbound| outbound.tag == "Osaka [osaka]"));
}

fn tls_settings(mode: TlsMode, server_name: Option<&str>) -> TlsSettings {
    full_tls_settings(mode, server_name, &[], Vec::new())
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

struct RuleGroupEnv {
    group: Option<crate::ContextPolicyGroup>,
    rule_outbound: &'static str,
}

impl crate::CoreGenEnv for RuleGroupEnv {
    fn platform(&self) -> CoreGenPlatform {
        CoreGenPlatform::Linux
    }

    fn get_profile_by_remarks(&self, _remarks: &str) -> Option<ProfileItem> {
        None
    }

    fn get_default_routing(&self, _config: &AppConfig) -> Option<RoutingItem> {
        Some(RoutingItem {
            rule_set: vec![RulesItem {
                remarks: Some("Work sites".to_string()),
                outbound_tag: Some(self.rule_outbound.to_string()),
                domain: Some(vec!["domain:example.com".to_string()]),
                enabled: true,
                ..RulesItem::default()
            }],
            ..RoutingItem::default()
        })
    }

    fn get_local_port(&self, _protocol: InboundProtocol) -> i32 {
        crate::DEFAULT_LOCAL_PORT
    }

    fn get_policy_group(&self, id: &str) -> Option<crate::ContextPolicyGroup> {
        self.group.clone().filter(|group| group.group.id == id)
    }
}

fn work_group() -> crate::ContextPolicyGroup {
    crate::ContextPolicyGroup {
        group: crate::PolicyGroupItem {
            id: "work".to_string(),
            name: "Work".to_string(),
            strategy: crate::GroupStrategy::UrlTest,
            ..crate::PolicyGroupItem::default()
        },
        members: vec![
            group_member("tokyo", "Tokyo", 1080),
            group_member("osaka", "Osaka", 1081),
        ],
    }
}

#[test]
fn a_rule_sends_traffic_through_a_policy_group_other_than_the_active_one() {
    let env = RuleGroupEnv {
        group: Some(work_group()),
        rule_outbound: "group:work",
    };
    let result = crate::CoreConfigContextBuilder::new(&env)
        .build(&AppConfig::default(), &group_member("main", "Main", 1082));
    assert!(result.success(), "{:?}", result.validator_result);

    let value = generate_singbox_config_value(&result.context).expect("config");
    let outbounds = value["outbounds"].as_array().expect("outbounds");
    let group = outbounds
        .iter()
        .find(|outbound| outbound["tag"] == "Work [work]")
        .expect("the named group has its own outbound");
    assert_eq!(group["type"], "urltest");
    // Member tags live under the group, apart from the main proxy's.
    assert_eq!(
        group["outbounds"],
        serde_json::json!(["Work [work] / Tokyo [tokyo]", "Work [work] / Osaka [osaka]"])
    );
    for member in ["Work [work] / Tokyo [tokyo]", "Work [work] / Osaka [osaka]"] {
        assert!(
            outbounds.iter().any(|outbound| outbound["tag"] == member),
            "{member}"
        );
    }
    let rules = value["route"]["rules"].as_array().expect("rules");
    assert!(rules.iter().any(|rule| rule["outbound"] == "Work [work]"));
}

#[test]
fn a_rule_naming_the_active_group_uses_the_main_proxy() {
    let env = RuleGroupEnv {
        group: Some(work_group()),
        rule_outbound: "group:work",
    };
    let active = work_group();
    let result = crate::CoreConfigContextBuilder::new(&env).build_for_group(
        &AppConfig::default(),
        &active.group,
        &active.members,
    );
    assert!(result.success(), "{:?}", result.validator_result);

    let value = generate_singbox_config_value(&result.context).expect("config");
    let rules = value["route"]["rules"].as_array().expect("rules");
    assert!(rules
        .iter()
        .any(|rule| rule["outbound"] == PROXY_TAG && rule["domain_suffix"].is_array()));
    assert!(!value["outbounds"]
        .as_array()
        .expect("outbounds")
        .iter()
        .any(|outbound| outbound["tag"] == "Work [work]"));
}

#[test]
fn a_rule_naming_a_missing_policy_group_fails_the_build() {
    let env = RuleGroupEnv {
        group: None,
        rule_outbound: "group:gone",
    };
    let result = crate::CoreConfigContextBuilder::new(&env)
        .build(&AppConfig::default(), &group_member("main", "Main", 1082));

    assert!(!result.success());
    assert!(result.validator_result.errors.iter().any(|error| matches!(
        &error.code,
        crate::ValidationCode::RoutingRuleOutboundNotFound { outbound, .. } if outbound == "group:gone"
    )));
}

#[test]
fn singbox_macos_leaves_out_app_conditions_the_network_extension_cannot_match() {
    let routing = RoutingItem {
        rule_set: vec![RulesItem {
            remarks: Some("Apps".to_string()),
            outbound_tag: Some(DIRECT_TAG.to_string()),
            process: Some(vec!["curl".to_string()]),
            domain: Some(vec!["domain:example.com".to_string()]),
            enabled: true,
            ..RulesItem::default()
        }],
        ..RoutingItem::default()
    };
    let mut linux = test_context(AppConfig::default(), base_remote_node());
    linux.routing_item = Some(routing);
    let mut macos = linux.clone();
    macos.platform = CoreGenPlatform::MacOS;

    let has_app_rule = |context: &CoreConfigContext| {
        generate_singbox_config_value(context).expect("config")["route"]["rules"]
            .as_array()
            .expect("rules")
            .iter()
            .any(|rule| rule.get("process_name").is_some() || rule.get("process_path").is_some())
    };
    assert!(has_app_rule(&linux));
    assert!(!has_app_rule(&macos));
}

#[test]
fn singbox_macos_tun_leaves_out_the_core_process_rule() {
    let mut app_config = AppConfig::default();
    app_config.tun_mode_item.enable_tun = true;
    let mut linux = test_context(app_config, base_remote_node());
    linux.is_tun_enabled = true;
    let mut macos = linux.clone();
    macos.platform = CoreGenPlatform::MacOS;

    let core_rule = |context: &CoreConfigContext| {
        generate_singbox_config(context)
            .expect("config")
            .route
            .rules
            .iter()
            .any(|rule| rule.process_name.as_deref() == Some(&["sing-box".to_string()][..]))
    };
    assert!(
        core_rule(&linux),
        "a process TUN sends the core's own sockets direct"
    );
    assert!(
        !core_rule(&macos),
        "the NetworkExtension cannot match a process"
    );
}

#[test]
fn latency_probe_tag_follows_the_generated_outbound() {
    let context = test_context(AppConfig::default(), socks_node("active", "Active"));
    let node = socks_node("node", "Node");
    let tag = latency_probe_tag(&context, &node);

    assert!(tag.starts_with("probe:node:"), "{tag}");
    assert_eq!(tag, latency_probe_tag(&context, &node.clone()));
    // A renamed node generates the same outbound, so a running core can still
    // measure it; an identical node under another id is a different probe.
    let renamed = ProfileItem {
        remarks: "Renamed".to_string(),
        ..node.clone()
    };
    assert_eq!(tag, latency_probe_tag(&context, &renamed));
    assert_ne!(
        tag,
        latency_probe_tag(&context, &socks_node("twin", "Node"))
    );

    // An edited server or a changed outbound setting no longer matches what
    // the running core was started with.
    let moved = ProfileItem {
        protocol: ProfileProtocol::Socks {
            server: endpoint(LOOPBACK, 1081),
            username: "user".to_string(),
            password: "pass".to_string(),
        },
        ..node.clone()
    };
    assert_ne!(tag, latency_probe_tag(&context, &moved));
    let mut remote = base_remote_node();
    remote.index_id = "remote".to_string();
    let mut muxed = context.clone();
    muxed.app_config.core_basic_item.mux_enabled = true;
    assert_ne!(
        latency_probe_tag(&context, &remote),
        latency_probe_tag(&muxed, &remote)
    );
}

#[test]
fn latency_probe_candidates_exclude_nodes_that_could_break_the_connection() {
    assert!(is_latency_probe_candidate(&socks_node("node", "Node")));
    assert!(!is_latency_probe_candidate(&socks_node("", "No id")));
    let no_port = ProfileItem {
        protocol: ProfileProtocol::Socks {
            server: endpoint(LOOPBACK, 0),
            username: String::new(),
            password: String::new(),
        },
        ..socks_node("no-port", "No port")
    };
    assert!(!is_latency_probe_candidate(&no_port));
    let wireguard = ProfileItem {
        index_id: "wg".to_string(),
        protocol: ProfileProtocol::WireGuard {
            server: endpoint("198.51.100.7", 51820),
            private_key: "key".to_string(),
            peer_public_key: Some("peer".to_string()),
            preshared_key: None,
            interface_address: None,
            allowed_ips: None,
            reserved: None,
            mtu: None,
        },
        ..ProfileItem::default()
    };
    assert!(!is_latency_probe_candidate(&wireguard));
}

#[test]
fn latency_probes_are_absent_unless_requested() {
    let context = test_context(AppConfig::default(), socks_node("active", "Active"));
    let outbounds = generate_singbox_config_value(&context).expect("config")["outbounds"].clone();

    assert!(outbounds
        .as_array()
        .expect("outbounds")
        .iter()
        .all(|outbound| !outbound["tag"]
            .as_str()
            .unwrap_or_default()
            .starts_with("probe:")));
}
