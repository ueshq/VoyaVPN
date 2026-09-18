use std::{collections::BTreeMap, panic};

use proptest::prelude::*;

use super::{entry::export_share_link, *};
use crate::testutil::{endpoint, linux_context as fmt_test_context};
use crate::{generate_singbox_config_value, AppConfig, PROXY_TAG};

#[test]
fn fmt_share_round_trips_all_supported_protocols() {
    for source in sample_profiles() {
        let uri = export_share_link(&source).expect("export share link");
        let parsed = parse_share_link(&uri).expect("parse exported share link");

        assert_eq!(parsed.config_type(), source.config_type(), "{uri}");
        assert_eq!(parsed.address(), source.address(), "{uri}");
        assert_eq!(parsed.port(), source.port(), "{uri}");
        assert_eq!(parsed.remarks, source.remarks, "{uri}");
        assert_eq!(parsed.password(), source.password(), "{uri}");
        assert_eq!(parsed.username(), source.username(), "{uri}");
    }
}

#[test]
fn fmt_share_round_trip_preserves_protocol_transport_and_tls_payloads() {
    // The all-protocols round trip above only compares six scalars, which is
    // how a dropped transport, a TLS block derived from the wrong query key or
    // a rewritten plugin option string stayed invisible. These cases compare
    // the whole payload for links that are expected to be lossless.
    let cases = vec![
        profile(
            "vless ws tls",
            ProfileProtocol::Vless {
                server: endpoint("vless.example", 8443),
                uuid: "00000000-0000-0000-0000-000000000004".to_string(),
                flow: None,
                encryption: Some(NONE.to_string()),
            },
            Some(ProfileTransport::Websocket {
                host: Some("vless.example".to_string()),
                path: Some("/ws".to_string()),
            }),
            Some(tls(TlsMode::Tls, "vless.example")),
        ),
        profile(
            "trojan grpc tls",
            ProfileProtocol::Trojan {
                server: endpoint("trojan.example", 443),
                password: "trojan-pass".to_string(),
            },
            Some(ProfileTransport::Grpc {
                authority: Some("trojan.example".to_string()),
                service_name: Some("svc".to_string()),
                mode: Some(GRPC_MULTI_MODE.to_string()),
            }),
            Some(tls(TlsMode::Tls, "trojan.example")),
        ),
        profile(
            "trojan httpupgrade reality",
            ProfileProtocol::Trojan {
                server: endpoint("reality.example", 443),
                password: "trojan-pass".to_string(),
            },
            Some(ProfileTransport::HttpUpgrade {
                host: Some("reality.example".to_string()),
                path: Some("/upgrade".to_string()),
            }),
            Some(TlsSettings {
                reality_public_key: Some("public-key".to_string()),
                reality_short_id: Some("shortid".to_string()),
                ..tls(TlsMode::Reality, "reality.example")
            }),
        ),
        // The Shadowsocks plugin option string is the one payload both the
        // exporter and the sing-box generator build, so a round trip through it
        // is what keeps `crate::protocol_common::shadowsocks_plugin_for` honest.
        profile(
            "ss v2ray-plugin ws tls",
            ProfileProtocol::Shadowsocks {
                server: endpoint("ss.example", 8388),
                password: "pass123".to_string(),
                method: "aes-128-gcm".to_string(),
                udp_over_tcp: false,
            },
            Some(ProfileTransport::Websocket {
                host: Some("ws.example".to_string()),
                path: Some("/ws".to_string()),
            }),
            Some(tls(TlsMode::Tls, "ws.example")),
        ),
    ];

    for source in cases {
        let uri = export_share_link(&source).expect("export share link");
        let parsed = parse_share_link(&uri).expect("parse exported share link");

        assert_eq!(parsed.protocol, source.protocol, "protocol for {uri}");
        assert_eq!(parsed.transport, source.transport, "transport for {uri}");
        assert_eq!(parsed.tls, source.tls, "tls for {uri}");
    }
}

#[test]
fn fmt_shadowsocks_plugin_export_uses_the_shared_option_model() {
    let source = profile(
        "ss obfs",
        ProfileProtocol::Shadowsocks {
            server: endpoint("ss.example", 8388),
            password: "pass123".to_string(),
            method: "aes-128-gcm".to_string(),
            udp_over_tcp: false,
        },
        Some(ProfileTransport::Tcp {
            header: Some(RAW_HEADER_HTTP.to_string()),
            // Only the first authority reaches the plugin: `obfs-host` is a
            // single host, and the generator has always taken the first entry.
            host: Some("obfs.example,second.example".to_string()),
            path: None,
        }),
        None,
    );

    let uri = export_share_link(&source).expect("export obfs shadowsocks link");
    let plugin = Url::parse(&uri)
        .expect("obfs link should parse")
        .query_pairs()
        .find(|(key, _)| key == "plugin")
        .map(|(_, value)| value.into_owned())
        .expect("plugin query value");
    assert_eq!(plugin, "obfs-local;obfs=http;obfs-host=obfs.example");

    let parsed = parse_share_link(&uri).expect("parse obfs shadowsocks link");
    assert_eq!(
        parsed.transport,
        Some(ProfileTransport::Tcp {
            header: Some(RAW_HEADER_HTTP.to_string()),
            host: Some("obfs.example".to_string()),
            path: None,
        })
    );
}

#[test]
fn fmt_share_round_trip_documents_hysteria2_lossy_exports() {
    // A hysteria2 link is not a lossless carrier: `mport` ranges are rewritten
    // with `-` separators.
    let source = profile(
        "hy2 lossy",
        ProfileProtocol::Hysteria2 {
            server: endpoint("hy2.example", 443),
            password: "hy2-pass".to_string(),
            port_hops: Some("1000:2000".to_string()),
            obfuscation_password: Some("obfs-pass".to_string()),
        },
        None,
        Some(tls(TlsMode::Tls, "hy2.example")),
    );

    let uri = export_share_link(&source).expect("export hysteria2 link");
    let parsed = parse_share_link(&uri).expect("parse hysteria2 link");

    assert_eq!(
        parsed.protocol,
        ProfileProtocol::Hysteria2 {
            server: endpoint("hy2.example", 443),
            password: "hy2-pass".to_string(),
            port_hops: Some("1000-2000".to_string()),
            obfuscation_password: Some("obfs-pass".to_string()),
        }
    );
    // TLS-only protocols carry no `type=`, so the parser normalises the missing
    // transport into the raw default rather than leaving it unset.
    assert_eq!(
        parsed.transport,
        Some(ProfileTransport::Tcp {
            header: Some(NONE.to_string()),
            host: None,
            path: None,
        })
    );
}

#[test]
fn fmt_share_export_materializes_supported_global_runtime_options() {
    let options = ShareLinkOptions {
        allow_insecure: true,
        fingerprint: "firefox".to_string(),
        hysteria_up_mbps: 80,
        hysteria_down_mbps: 160,
        hysteria_hop_interval: 25,
    };
    let vless = ProfileItem {
        protocol: ProfileProtocol::Vless {
            server: endpoint("vless.example", 443),
            uuid: "00000000-0000-0000-0000-000000000001".to_string(),
            flow: None,
            encryption: Some(NONE.to_string()),
        },
        tls: Some(tls(TlsMode::Tls, "vless.example")),
        ..ProfileItem::default()
    };
    let vless_link = export_share_link_with_options(&vless, &options)
        .expect("global VLESS options should export");
    let vless_url = Url::parse(&vless_link).expect("VLESS link should parse");
    let vless_query = vless_url.query_pairs().collect::<BTreeMap<_, _>>();
    assert_eq!(
        vless_query.get("insecure").map(|value| value.as_ref()),
        Some("1")
    );
    assert_eq!(
        vless_query.get("fp").map(|value| value.as_ref()),
        Some("firefox")
    );

    let hysteria = ProfileItem {
        protocol: ProfileProtocol::Hysteria2 {
            server: endpoint("hy2.example", 443),
            password: "secret".to_string(),
            port_hops: None,
            obfuscation_password: None,
        },
        ..ProfileItem::default()
    };
    let hysteria_link = export_share_link_with_options(&hysteria, &options)
        .expect("global Hysteria options should export");
    let hysteria_url = Url::parse(&hysteria_link).expect("Hysteria link should parse");
    let hysteria_query = hysteria_url.query_pairs().collect::<BTreeMap<_, _>>();
    assert_eq!(
        hysteria_query.get("upmbps").map(|value| value.as_ref()),
        Some("80")
    );
    assert_eq!(
        hysteria_query.get("downmbps").map(|value| value.as_ref()),
        Some("160")
    );
    assert_eq!(
        hysteria_query
            .get("hopInterval")
            .map(|value| value.as_ref()),
        Some("25")
    );
}

#[test]
fn fmt_base_query_round_trips_transport_and_security() {
    let source = ProfileItem {
        remarks: "advanced vless".to_string(),
        protocol: ProfileProtocol::Vless {
            server: endpoint("vless.example", 443),
            uuid: "00000000-0000-0000-0000-000000000001".to_string(),
            flow: Some("xtls-rprx-vision".to_string()),
            encryption: Some(NONE.to_string()),
        },
        transport: Some(ProfileTransport::Grpc {
            authority: Some("cdn.example".to_string()),
            service_name: Some("svc".to_string()),
            mode: Some("multi".to_string()),
        }),
        tls: Some(TlsSettings {
            mode: TlsMode::Reality,
            server_name: Some("sni.example".to_string()),
            alpn: Vec::new(),
            reality_public_key: Some("public-key".to_string()),
            reality_short_id: Some("abcd".to_string()),
            certificate_pem: None,
            ech_config: vec!["https://ech.example/config".to_string()],
        }),
        ..ProfileItem::default()
    };

    let uri = export_share_link(&source).expect("export advanced vless");
    for retired in ["spx=", "pqv=", "pcs=", "fm="] {
        assert!(
            !uri.contains(retired),
            "{retired} must not be exported: {uri}"
        );
    }
    let parsed = parse_share_link(&uri).expect("parse advanced vless");

    let parsed_tls = parsed.tls.as_ref().expect("TLS settings");
    assert_eq!(parsed_tls.mode, TlsMode::Reality);
    assert_eq!(parsed_tls.reality_short_id.as_deref(), Some("abcd"));
    assert_eq!(
        parsed_tls.ech_config,
        vec!["https://ech.example/config".to_string()]
    );
    let Some(ProfileTransport::Grpc {
        service_name, mode, ..
    }) = parsed.transport.as_ref()
    else {
        panic!("expected grpc transport");
    };
    assert_eq!(service_name.as_deref(), Some("svc"));
    assert_eq!(mode.as_deref(), Some("multi"));
}

#[test]
fn fmt_query_parser_preserves_values_containing_equals() {
    let parsed = parse_share_link(
        "vless://00000000-0000-0000-0000-000000000001@example.com:443?encryption=none&type=ws&path=/a=b==#eq",
    )
    .expect("parse vless with equals in query value");

    let Some(ProfileTransport::Websocket { path, .. }) = parsed.transport else {
        panic!("expected websocket transport");
    };
    assert_eq!(path.as_deref(), Some("/a=b=="));
}

#[test]
fn fmt_rejects_retired_transports_in_uri_links() {
    for transport in ["xhttp", "splithttp", "kcp", "mKCP"] {
        for link in [
            format!("vless://00000000-0000-0000-0000-000000000001@example.com:443?encryption=none&type={transport}#retired"),
            format!("trojan://secret@example.com:443?security=tls&type={transport}#retired"),
            format!("vmess://00000000-0000-0000-0000-000000000001@example.com:443?type={transport}#retired"),
        ] {
            assert_eq!(
                parse_share_link(&link),
                Err(ShareError::UnsupportedTransport {
                    transport: transport.to_string()
                }),
                "{link}"
            );
        }
    }
}

#[test]
fn fmt_rejects_retired_transports_in_vmess_base64() {
    for transport in ["xhttp", "kcp"] {
        let payload = serde_json::json!({
            "v": "2", "ps": "retired", "add": "example.com", "port": "443",
            "id": "00000000-0000-0000-0000-000000000001", "net": transport,
        })
        .to_string();
        let link = format!("vmess://{}", STANDARD.encode(payload));
        assert_eq!(
            parse_share_link(&link),
            Err(ShareError::UnsupportedTransport {
                transport: transport.to_string()
            })
        );
    }
}

#[test]
fn fmt_hostile_subscription_tls_flags_are_not_trusted_by_generators() {
    let mut node = parse_share_link(
        "vless://00000000-0000-0000-0000-000000000099@hostile.example:443?encryption=none&security=tls&type=ws&host=cdn.example&path=/ws&insecure=1&fp=definitely-not-utls#hostile",
    )
    .expect("parse hostile vless share");
    node.index_id = "hostile-vless".to_string();

    let mut app_config = AppConfig::default();
    app_config.core_basic_item.def_allow_insecure = false;
    app_config.core_basic_item.def_fingerprint = "firefox".to_string();

    let singbox_value = generate_singbox_config_value(&fmt_test_context(app_config, node))
        .expect("sing-box config should generate");
    let singbox_proxy = proxy_outbound(&singbox_value);
    assert_eq!(
        singbox_proxy
            .pointer("/tls/insecure")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        singbox_proxy
            .pointer("/tls/utls/fingerprint")
            .and_then(Value::as_str),
        Some("firefox")
    );
}

#[test]
fn fmt_negative_inputs_return_typed_errors_without_panicking() {
    for bad in [
        "",
        "not-a-share-uri",
        "vmess://%%%%",
        "vless://uuid@example.com",
        "ss://not-base64",
        "wireguard://key@example.com:notaport",
        "tuic://onlyuser@example.com:443",
        "v2rayn://retired-private-format",
    ] {
        let result = panic::catch_unwind(|| parse_share_link(bad).map(|_| ()));
        assert!(result.is_ok(), "{bad} panicked");
        assert!(result.expect("panic checked").is_err(), "{bad} parsed");
    }
}

#[test]
fn fmt_negative_inputs_cover_port_host_and_large_base64_edges() {
    let bad_port_vmess = base64_encode(
        r#"{"v":"2","ps":"bad-port","add":"example.com","port":"70000","id":"00000000-0000-0000-0000-000000000012"}"#,
        false,
    );
    let bad_host_vmess = base64_encode(
        r#"{"v":"2","ps":"bad-host","add":"bad=host","port":"443","id":"00000000-0000-0000-0000-000000000013"}"#,
        false,
    );
    let oversized_vmess = format!("vmess://{}", "A".repeat(MAX_BASE64_DECODE_INPUT + 4));
    let bad_inputs = vec![
        "vless://00000000-0000-0000-0000-000000000014@example.com:0?encryption=none".to_string(),
        "vless://00000000-0000-0000-0000-000000000014@example.com:65536?encryption=none"
            .to_string(),
        "vless://00000000-0000-0000-0000-000000000014@bad=host:443?encryption=none".to_string(),
        format!("vmess://{bad_port_vmess}"),
        format!("vmess://{bad_host_vmess}"),
        oversized_vmess,
    ];

    for bad in bad_inputs {
        let result = panic::catch_unwind(|| parse_share_link(&bad));
        assert!(result.is_ok(), "{bad} panicked");
        assert!(result.expect("panic checked").is_err(), "{bad} parsed");
    }
}

#[test]
fn fmt_shadowsocks_rejects_full_link_base64_and_parses_plugins() {
    let old_payload = base64_encode("aes-128-gcm:pass@example.com:8388", false);
    assert!(parse_share_link(&format!("ss://{old_payload}#old")).is_err());

    let old_socks_payload = base64_encode("user:pass@example.com:1080", false);
    assert!(parse_share_link(&format!("socks://{old_socks_payload}")).is_err());

    let plugin =
        url_encode("v2ray-plugin;mode=websocket;host=ws.example;path=/a\\=b\\,c;tls;mux=0");
    let sip002 = format!(
        "ss://{}@example.com:8388?plugin={plugin}#plugin",
        base64_encode("aes-256-gcm:pass", true)
    );
    let parsed = parse_share_link(&sip002).expect("parse plugin ss");
    assert_eq!(parsed.stream_security(), STREAM_SECURITY_TLS);
    let Some(ProfileTransport::Websocket { path, .. }) = parsed.transport else {
        panic!("expected websocket transport");
    };
    assert_eq!(path.as_deref(), Some("/a=b,c"));
}

#[test]
fn fmt_parses_common_multiline_import_shapes() {
    let vmess_json = r#"{
        "v": "2",
        "ps": "JMS-TEST@example.test:17701",
        "add": "node-vmess.example.test",
        "port": "17701",
        "id": "00000000-0000-0000-0000-000000000001",
        "aid": "0",
        "scy": "auto",
        "net": "tcp",
        "type": "none",
        "host": "",
        "path": "",
        "tls": "",
        "sni": "",
        "alpn": "",
        "fp": "",
        "insecure": "0"
    }"#;
    let vmess = format!("vmess://{}", base64_encode(vmess_json, false));
    let parsed = parse_share_link(&vmess).expect("parse vmess base64 json");
    assert_eq!(parsed.config_type(), ConfigType::VMess);
    assert_eq!(parsed.remarks, "JMS-TEST@example.test:17701");
    assert_eq!(parsed.address(), "node-vmess.example.test");
    assert_eq!(parsed.port(), 17701);
    assert_eq!(parsed.network(), DEFAULT_NETWORK);
    let ProfileProtocol::Vmess { cipher, .. } = parsed.protocol else {
        panic!("expected VMess protocol");
    };
    assert_eq!(cipher.as_deref(), Some(DEFAULT_SECURITY));

    let paddingless_vmess = format!("vmess://{}", base64_encode(vmess_json, true));
    let parsed = parse_share_link(&paddingless_vmess).expect("parse paddingless vmess");
    assert_eq!(parsed.address(), "node-vmess.example.test");

    let vless = "vless://00000000-0000-0000-0000-000000000002@node-vless.example.test:443?encryption=none&security=tls&sni=node-vless.example.test&fp=randomized&insecure=0&allowInsecure=0&type=ws&host=node-vless.example.test&path=%2F%3Fed%3D2048#node-vless.example.test";
    let parsed = parse_share_link(vless).expect("parse vless ws tls");
    assert_eq!(parsed.config_type(), ConfigType::VLESS);
    assert_eq!(parsed.address(), "node-vless.example.test");
    assert_eq!(parsed.stream_security(), STREAM_SECURITY_TLS);
    let Some(ProfileTransport::Websocket { host, path }) = parsed.transport else {
        panic!("expected websocket transport");
    };
    assert_eq!(host.as_deref(), Some("node-vless.example.test"));
    assert_eq!(path.as_deref(), Some("/?ed=2048"));

    let ss_user_info = base64_encode("aes-256-gcm:test-password", true);
    let ss = format!(
        "ss://{ss_user_info}@node-ss.example.test:17701?#JMS-TEST%40node-ss.example.test%3A17701"
    );
    let parsed = parse_share_link(&ss).expect("parse sip002 ss with empty query");
    assert_eq!(parsed.config_type(), ConfigType::Shadowsocks);
    assert_eq!(parsed.address(), "node-ss.example.test");
    assert_eq!(parsed.port(), 17701);
    assert_eq!(parsed.remarks, "JMS-TEST@node-ss.example.test:17701");
    let ProfileProtocol::Shadowsocks { method, .. } = parsed.protocol else {
        panic!("expected Shadowsocks protocol");
    };
    assert_eq!(method, "aes-256-gcm");
}

#[test]
fn fmt_wireguard_config_parses_peers_and_inline_comments() {
    let config = r#"
        [Interface]
        PrivateKey = interface-private-key
        Address = 10.0.0.2/32, fd00::2/128 ; inline comment
        MTU = 1420

        [Peer]
        PublicKey = peer-public-key
        PresharedKey = peer-preshared-key
        AllowedIPs = 10.0.0.0/8, 192.168.0.0/16
        Reserved = 1, 2, 3 # inline comment
        Endpoint = [2001:db8::1]:51820 # inline comment

        [Peer]
        PublicKey = peer-public-key-2
        Endpoint = example.com:12345
    "#;

    let resolved = parse_wireguard_config(config).expect("wireguard config");
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].address(), "2001:db8::1");
    assert_eq!(resolved[0].port(), 51820);
    let ProfileProtocol::WireGuard {
        private_key,
        reserved,
        allowed_ips,
        interface_address,
        mtu,
        ..
    } = &resolved[0].protocol
    else {
        panic!("expected WireGuard protocol");
    };
    assert_eq!(private_key, "interface-private-key");
    assert_eq!(reserved.as_deref(), Some("1, 2, 3"));
    assert_eq!(allowed_ips.as_deref(), Some("10.0.0.0/8, 192.168.0.0/16"));
    assert_eq!(
        interface_address.as_deref(),
        Some("10.0.0.2/32, fd00::2/128")
    );
    assert_eq!(*mtu, Some(1420));
    assert_eq!(resolved[1].address(), "example.com");
    assert_eq!(resolved[1].port(), 12345);
}

#[test]
fn fmt_ss_sip008_accepts_numeric_ports_in_documents_and_bare_arrays() {
    let document = r#"{
        "version": 1,
        "servers": [
            {
                "id": "27b8a625-4f4b-4428-9f0f-8a2317db7c79",
                "remarks": "sip008 node",
                "server": "192.168.100.1",
                "server_port": 8388,
                "password": "example",
                "method": "aes-256-gcm"
            }
        ]
    }"#;
    let servers = parse_ss_sip008(document).expect("parse SIP008 document");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].remarks, "sip008 node");
    assert_eq!(servers[0].address(), "192.168.100.1");
    assert_eq!(servers[0].port(), 8388);
    assert_eq!(servers[0].password(), "example");

    let bare_array = r#"[
        {
            "server": "node.example",
            "server_port": "8389",
            "password": 12345,
            "method": "chacha20-ietf-poly1305"
        }
    ]"#;
    let servers = parse_ss_sip008(bare_array).expect("parse SIP008 bare array");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].address(), "node.example");
    assert_eq!(servers[0].port(), 8389);
    assert_eq!(servers[0].password(), "12345");
}

#[test]
fn fmt_tls_only_protocols_stay_tls_without_query_hints() {
    for link in [
        "hysteria2://pass@203.0.113.5:443/?insecure=1#node",
        "hy2://pass@hy2.example:443#node",
        "tuic://00000000-0000-0000-0000-000000000031:pass@tuic.example:443?congestion_control=bbr",
        "anytls://pass@anytls.example:443",
        "naive+https://user:pass@naive.example:443",
    ] {
        let parsed = parse_share_link(link).expect("parse TLS-only share link");
        let Some(tls) = parsed.tls.as_ref() else {
            panic!("{link} should keep TLS enabled");
        };
        assert_eq!(tls.mode, TlsMode::Tls, "{link}");
        assert_eq!(parsed.stream_security(), STREAM_SECURITY_TLS, "{link}");
    }
}

#[test]
fn fmt_plaintext_links_are_not_upgraded_to_tls_by_stray_sni() {
    let vless = parse_share_link(
        "vless://00000000-0000-0000-0000-000000000032@example.com:80?encryption=none&security=none&type=ws&host=cdn.example&path=/&sni=cdn.example#plain",
    )
    .expect("parse plaintext vless");
    assert!(vless.tls.is_none());
    assert_eq!(vless.stream_security(), "");

    let vmess_json = r#"{
        "v": "2",
        "ps": "plain vmess",
        "add": "example.com",
        "port": "80",
        "id": "00000000-0000-0000-0000-000000000033",
        "net": "ws",
        "host": "cdn.example",
        "path": "/",
        "tls": "",
        "sni": "cdn.example",
        "alpn": "h2"
    }"#;
    let vmess = parse_share_link(&format!("vmess://{}", base64_encode(vmess_json, false)))
        .expect("parse plaintext vmess");
    assert!(vmess.tls.is_none());
    assert_eq!(vmess.stream_security(), "");
}

#[test]
fn fmt_query_values_are_percent_decoded_exactly_once() {
    let hysteria2 = parse_share_link(
        "hysteria2://pass@hy2.example:443?obfs=salamander&obfs-password=100%25AB#obfs",
    )
    .expect("parse hysteria2 obfs link");
    let ProfileProtocol::Hysteria2 {
        obfuscation_password,
        ..
    } = &hysteria2.protocol
    else {
        panic!("expected Hysteria2 protocol");
    };
    assert_eq!(obfuscation_password.as_deref(), Some("100%AB"));

    let exported = export_share_link(&hysteria2).expect("export hysteria2 obfs link");
    let reparsed = parse_share_link(&exported).expect("reparse hysteria2 obfs link");
    assert_eq!(reparsed.protocol, hysteria2.protocol);

    let websocket = parse_share_link(
        "vless://00000000-0000-0000-0000-000000000034@example.com:443?encryption=none&security=tls&type=ws&path=%2Fp%2520q",
    )
    .expect("parse vless ws link");
    let Some(ProfileTransport::Websocket { path, .. }) = websocket.transport else {
        panic!("expected websocket transport");
    };
    assert_eq!(path.as_deref(), Some("/p%20q"));
}

#[test]
fn fmt_shadowsocks_plain_user_info_password_is_decoded_once() {
    let parsed = parse_share_link("ss://aes-256-gcm:p%2541ss@ss.example:8388#plain")
        .expect("parse plain ss");
    assert_eq!(parsed.password(), "p%41ss");
    let ProfileProtocol::Shadowsocks { method, .. } = &parsed.protocol else {
        panic!("expected Shadowsocks protocol");
    };
    assert_eq!(method, "aes-256-gcm");
}

#[test]
fn fmt_shadowsocks_2022_exports_plain_sip022_user_info() {
    // The key is base64 with `+`, `/` and `=`, the characters a double encoding
    // or a second base64 layer would corrupt.
    let key = "Qm+E/Zx0cWFyTr4w9q8a2A==";
    let source = ProfileItem {
        remarks: "Tokyo · SS".to_string(),
        protocol: ProfileProtocol::Shadowsocks {
            server: endpoint("2001:db8::1", 40_123),
            password: key.to_string(),
            method: "2022-blake3-aes-128-gcm".to_string(),
            udp_over_tcp: false,
        },
        ..ProfileItem::default()
    };

    let link = export_share_link(&source).expect("export SS-2022 link");
    assert_eq!(
        link,
        "ss://2022-blake3-aes-128-gcm:Qm%2BE%2FZx0cWFyTr4w9q8a2A%3D%3D@[2001:db8::1]:40123#Tokyo%20%C2%B7%20SS"
    );

    let parsed = parse_share_link(&link).expect("parse SS-2022 link");
    assert_eq!(parsed.protocol, source.protocol);
    assert_eq!(parsed.remarks, source.remarks);

    // Legacy methods keep the SIP002 base64 user info.
    let legacy = ProfileItem {
        protocol: ProfileProtocol::Shadowsocks {
            server: endpoint("ss.example", 8388),
            password: "legacy-pass".to_string(),
            method: "aes-256-gcm".to_string(),
            udp_over_tcp: false,
        },
        ..ProfileItem::default()
    };
    let legacy_link = export_share_link(&legacy).expect("export legacy SS link");
    assert!(legacy_link.starts_with("ss://YWVz"), "{legacy_link}");
}

#[test]
fn fmt_http2_and_quic_transports_survive_import_and_export() {
    for (link, host, path) in [
        (
            "vless://00000000-0000-0000-0000-000000000035@example.com:443?encryption=none&security=tls&type=http&host=h2.example&path=%2Fh2",
            "h2.example",
            "/h2",
        ),
        (
            "vless://00000000-0000-0000-0000-000000000035@example.com:443?encryption=none&security=tls&type=h2&host=h2.example&path=%2Fh2",
            "h2.example",
            "/h2",
        ),
    ] {
        let parsed = parse_share_link(link).expect("parse HTTP/2 share link");
        assert_eq!(parsed.network(), HTTP2_NETWORK, "{link}");
        assert_eq!(
            parsed.transport,
            Some(ProfileTransport::Http2 {
                host: Some(host.to_string()),
                path: Some(path.to_string()),
            }),
            "{link}"
        );

        let exported = export_share_link(&parsed).expect("export HTTP/2 share link");
        assert!(exported.contains("type=h2"), "{exported}");
        let reparsed = parse_share_link(&exported).expect("reparse HTTP/2 share link");
        assert_eq!(reparsed.transport, parsed.transport);
    }

    let quic = parse_share_link(
        "vless://00000000-0000-0000-0000-000000000036@example.com:443?encryption=none&security=tls&type=quic&host=none&path=quic-key",
    )
    .expect("parse QUIC share link");
    assert_eq!(quic.network(), QUIC_NETWORK);
    assert_eq!(
        quic.transport,
        Some(ProfileTransport::Quic {
            host: Some(NONE.to_string()),
            path: Some("quic-key".to_string()),
        })
    );
    let exported = export_share_link(&quic).expect("export QUIC share link");
    assert!(exported.contains("type=quic"), "{exported}");
    assert_eq!(
        parse_share_link(&exported)
            .expect("reparse QUIC share link")
            .transport,
        quic.transport
    );

    let vmess_json = r#"{
        "v": "2",
        "ps": "h2 vmess",
        "add": "example.com",
        "port": "443",
        "id": "00000000-0000-0000-0000-000000000037",
        "net": "h2",
        "host": "h2.example",
        "path": "/h2",
        "tls": "tls"
    }"#;
    let vmess = parse_share_link(&format!("vmess://{}", base64_encode(vmess_json, false)))
        .expect("parse HTTP/2 vmess");
    assert_eq!(
        vmess.transport,
        Some(ProfileTransport::Http2 {
            host: Some("h2.example".to_string()),
            path: Some("/h2".to_string()),
        })
    );
    let reparsed = parse_share_link(&export_share_link(&vmess).expect("export HTTP/2 vmess"))
        .expect("reparse HTTP/2 vmess");
    assert_eq!(reparsed.transport, vmess.transport);
}

#[test]
fn fmt_wireguard_config_skips_peers_with_unusable_endpoints() {
    let config = r#"
        [Interface]
        PrivateKey = interface-private-key

        [Peer]
        PublicKey = unbracketed-ipv6
        Endpoint = 2001:db8::1

        [Peer]
        PublicKey = invalid-host
        Endpoint = bad=host:51820

        [Peer]
        PublicKey = port-less
        Endpoint = warp.example

        [Peer]
        PublicKey = valid
        Endpoint = ok.example:51820
    "#;

    let resolved = parse_wireguard_config(config).expect("wireguard config");
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].address(), "warp.example");
    assert_eq!(resolved[0].port(), WIREGUARD_DEFAULT_ENDPOINT_PORT);
    assert_eq!(resolved[1].address(), "ok.example");
    assert_eq!(resolved[1].port(), 51820);

    let all_invalid = r#"
        [Interface]
        PrivateKey = interface-private-key

        [Peer]
        Endpoint = 2001:db8::1
    "#;
    assert!(parse_wireguard_config(all_invalid).is_err());
}

#[test]
fn fmt_socks_accepts_plain_user_names_and_anonymous_links() {
    let parsed =
        parse_share_link("socks://user@proxy.example:1080#plain").expect("parse plain socks");
    assert_eq!(parsed.username(), "user");
    assert_eq!(parsed.password(), "");

    let anonymous = ProfileItem {
        remarks: "anon".to_string(),
        protocol: ProfileProtocol::Socks {
            server: endpoint("127.0.0.1", 1080),
            username: String::new(),
            password: String::new(),
        },
        ..ProfileItem::default()
    };
    let link = export_share_link(&anonymous).expect("export anonymous socks");
    assert_eq!(link, "socks://127.0.0.1:1080#anon");
    let reparsed = parse_share_link(&link).expect("reparse anonymous socks");
    assert_eq!(reparsed.username(), "");
    assert_eq!(reparsed.password(), "");
}

proptest! {
    #[test]
    fn share_url_component_property_round_trips(value in "[A-Za-z0-9 _./:@,=+%\\-]{0,80}") {
        prop_assert_eq!(url_decode(&url_encode(&value)), value);
    }

    #[test]
    fn share_base64_property_round_trips(value in "[A-Za-z0-9 _./:@,=+\\-]{0,80}") {
        let encoded = base64_encode(&value, true);
        prop_assert_eq!(base64_decode(&encoded, "test").expect("decode"), value);
    }
}

fn sample_profiles() -> Vec<ProfileItem> {
    vec![
        profile(
            "vmess demo",
            ProfileProtocol::Vmess {
                server: endpoint("example.com", 443),
                uuid: "00000000-0000-0000-0000-000000000003".to_string(),
                cipher: Some(DEFAULT_SECURITY.to_string()),
            },
            Some(raw_transport()),
            None,
        ),
        profile(
            "vless demo",
            ProfileProtocol::Vless {
                server: endpoint("vless.example", 8443),
                uuid: "00000000-0000-0000-0000-000000000004".to_string(),
                flow: None,
                encryption: Some(NONE.to_string()),
            },
            Some(ProfileTransport::Websocket {
                host: Some("vless.example".to_string()),
                path: Some("/ws".to_string()),
            }),
            Some(tls(TlsMode::Tls, "vless.example")),
        ),
        profile(
            "trojan demo",
            ProfileProtocol::Trojan {
                server: endpoint("trojan.example", 443),
                password: "trojan-pass".to_string(),
            },
            Some(ProfileTransport::Grpc {
                authority: Some("trojan.example".to_string()),
                service_name: Some("svc".to_string()),
                mode: Some(GRPC_MULTI_MODE.to_string()),
            }),
            Some(tls(TlsMode::Tls, "trojan.example")),
        ),
        profile(
            "ss demo",
            ProfileProtocol::Shadowsocks {
                server: endpoint("1.2.3.4", 8388),
                password: "pass123".to_string(),
                method: "aes-128-gcm".to_string(),
                udp_over_tcp: false,
            },
            Some(raw_transport()),
            None,
        ),
        profile(
            "socks demo",
            ProfileProtocol::Socks {
                server: endpoint("127.0.0.1", 1080),
                username: "user".to_string(),
                password: "pass".to_string(),
            },
            None,
            None,
        ),
        profile(
            "hy2 demo",
            ProfileProtocol::Hysteria2 {
                server: endpoint("hy2.example", 443),
                password: "hy2-pass".to_string(),
                port_hops: Some("1000:2000".to_string()),
                obfuscation_password: Some("obfs-pass".to_string()),
            },
            None,
            Some(tls(TlsMode::Tls, "hy2.example")),
        ),
        profile(
            "tuic demo",
            ProfileProtocol::Tuic {
                server: endpoint("tuic.example", 443),
                uuid: "uuid".to_string(),
                password: "tuic-pass".to_string(),
                congestion_control: Some("bbr".to_string()),
            },
            None,
            Some(tls(TlsMode::Tls, "tuic.example")),
        ),
        profile(
            "wg demo",
            ProfileProtocol::WireGuard {
                server: endpoint("2001:db8::1", 51820),
                private_key: "private-key".to_string(),
                peer_public_key: Some("public-key".to_string()),
                preshared_key: Some("psk".to_string()),
                interface_address: Some("10.0.0.2/32".to_string()),
                allowed_ips: None,
                reserved: Some("1,2,3".to_string()),
                mtu: Some(1420),
            },
            None,
            None,
        ),
        profile(
            "anytls demo",
            ProfileProtocol::Anytls {
                server: endpoint("anytls.example", 443),
                password: "anytls-pass".to_string(),
            },
            None,
            Some(tls(TlsMode::Tls, "anytls.example")),
        ),
        profile(
            "naive demo",
            ProfileProtocol::Naive {
                server: endpoint("naive.example", 443),
                username: "user".to_string(),
                password: "pass".to_string(),
                quic: true,
                congestion_control: None,
                insecure_concurrency: Some(4),
                udp_over_tcp: false,
            },
            None,
            Some(tls(TlsMode::Tls, "naive.example")),
        ),
    ]
}

fn profile(
    remarks: &str,
    protocol: ProfileProtocol,
    transport: Option<ProfileTransport>,
    tls: Option<TlsSettings>,
) -> ProfileItem {
    ProfileItem {
        remarks: remarks.to_string(),
        protocol,
        transport,
        tls,
        ..ProfileItem::default()
    }
}

fn raw_transport() -> ProfileTransport {
    ProfileTransport::Tcp {
        header: Some(NONE.to_string()),
        host: None,
        path: None,
    }
}

fn tls(mode: TlsMode, server_name: &str) -> TlsSettings {
    crate::testutil::tls_settings(mode, Some(server_name), &[], Vec::new())
}

fn proxy_outbound(config: &Value) -> &Value {
    config
        .get("outbounds")
        .and_then(Value::as_array)
        .and_then(|outbounds| {
            outbounds
                .iter()
                .find(|outbound| outbound.get("tag").and_then(Value::as_str) == Some(PROXY_TAG))
        })
        .expect("proxy outbound should be generated")
}
