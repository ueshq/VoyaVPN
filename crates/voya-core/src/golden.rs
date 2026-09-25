use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::testutil::{
    base_remote_node as singbox_base_remote_node, endpoint, linux_context as singbox_context,
    raw_transport, socks_node as singbox_socks_node, tls_settings as full_tls_settings,
};
use crate::{
    generate_singbox_config, generate_singbox_config_value, AppConfig, ContextPolicyGroup,
    CoreConfigContext, CoreGenPlatform, DnsStrategy, GroupStrategy, PolicyGroupItem, ProfileItem,
    ProfileProtocol, ProfileTransport, RoutingItem, RuleType, RulesItem, TlsMode, TlsSettings,
    BLOCK_TAG, DIRECT_TAG, LOOPBACK, PROXY_TAG,
};

#[derive(Debug, Deserialize)]
pub(crate) struct GoldenMatrix {
    pub version: u32,
    pub cases: Vec<GoldenCase>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GoldenCase {
    pub id: String,
    pub core: String,
    pub fixture: String,
    pub generated: String,
    pub summary: String,
    #[serde(default)]
    pub hotspots: Vec<String>,
    #[serde(default)]
    pub reference_paths: Vec<String>,
    #[serde(default)]
    pub core_acceptance: bool,
    #[serde(default)]
    pub volatile_fields: Vec<GoldenVolatileField>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GoldenVolatileField {
    pub pointer: String,
    pub reason: String,
}

pub(crate) fn load_matrix() -> GoldenMatrix {
    let path = golden_root().join("matrix.json");
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read golden matrix {}: {err}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| panic!("failed to parse golden matrix {}: {err}", path.display()))
}

pub(crate) fn golden_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

pub(crate) fn load_fixture(case: &GoldenCase) -> Value {
    let path = golden_root().join(&case.fixture);
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read golden fixture {}: {err}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| panic!("failed to parse golden fixture {}: {err}", path.display()))
}

pub(crate) fn assert_fixture_matches(case: &GoldenCase, actual: Value) {
    let mut expected = load_fixture(case);
    let mut actual = actual;
    for volatile in &case.volatile_fields {
        remove_json_pointer(&mut expected, &volatile.pointer);
        remove_json_pointer(&mut actual, &volatile.pointer);
    }
    assert_json_eq(&case.id, &expected, &actual);
}

pub(crate) fn assert_json_eq(case_id: &str, expected: &Value, actual: &Value) {
    let expected_json = canonical_json_string(expected);
    let actual_json = canonical_json_string(actual);
    assert!(
        expected_json == actual_json,
        "golden case `{case_id}` did not match\n{}",
        unified_diff(&expected_json, &actual_json)
    );
}

pub(crate) fn canonical_json_string(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(&canonicalize(value))
        .unwrap_or_else(|err| panic!("failed to serialize canonical JSON: {err}"));
    text.push('\n');
    text
}

pub(crate) fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            let mut sorted = Map::new();
            for key in keys {
                if let Some(value) = object.get(key) {
                    sorted.insert(key.clone(), canonicalize(value));
                }
            }
            Value::Object(sorted)
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
    }
}

pub(crate) fn generated_value_for_case(case: &GoldenCase) -> Value {
    match case.generated.as_str() {
        "singbox.outbound.vless_ws_tls_mux" => singbox_vless_ws_tls_mux_outbound(),
        "singbox.outbound.vless_tls_fragment_hello" => singbox_vless_tls_fragment_hello_outbound(),
        "singbox.dns.fakeip_typed" => singbox_fakeip_typed_dns(),
        "singbox.route.rulesets_from_dns" => singbox_rulesets_from_dns(),
        "singbox.inbounds.tun" => singbox_tun_inbounds(),
        "singbox.inbounds.tun_macos" => singbox_tun_inbounds_macos(),
        "singbox.route.tun" => singbox_tun_route(),
        "singbox.route.default_seed" => singbox_default_seed_snapshot(),
        "singbox.route.default_seed_ipv6" => singbox_default_seed_ipv6_snapshot(),
        "singbox.outbound.policy_groups" => singbox_policy_groups_snapshot(),
        "singbox.outbound.latency_probes" => singbox_latency_probes_snapshot(),
        "singbox.outbound.tuic_tls" => singbox_tuic_tls_outbound(),
        "singbox.outbound.anytls_tls" => singbox_anytls_tls_outbound(),
        "singbox.outbound.naive_quic_tls" => singbox_naive_quic_tls_outbound(),
        "singbox.runtime.pre_socks" => singbox_pre_socks_snapshot(),
        "singbox.routing.per_rule_outbound" => singbox_per_rule_outbound_snapshot(),
        "singbox.runtime.logs_and_api" => singbox_logs_and_api_snapshot(),
        "singbox.outbound.hysteria2_minimal" => singbox_hysteria2_minimal_outbound(),
        "singbox.outbound.vmess_h2_tls" => singbox_vmess_h2_tls_outbound(),
        "singbox.outbound.vless_quic_tls" => singbox_vless_quic_tls_outbound(),
        "singbox.outbound.shadowsocks_plugins" => singbox_shadowsocks_plugins_outbounds(),
        "singbox.route.bind_interface_windows" => singbox_bind_interface_windows_outbounds(),
        "singbox.selfhost.vless_reality" => selfhost::selfhost_vless_reality(),
        "singbox.selfhost.shadowsocks_2022" => selfhost::selfhost_shadowsocks_2022(),
        "singbox.selfhost.dual_allow_lan" => selfhost::selfhost_dual_allow_lan(),
        "singbox.outbound.vless_reality_vision" => {
            selfhost::proxy_outbound(&selfhost::vless_reality_vision_context())
        }
        "singbox.outbound.shadowsocks_2022" => {
            selfhost::proxy_outbound(&selfhost::shadowsocks_2022_context())
        }
        generated => panic!(
            "golden case `{}` references unknown generated selector `{generated}`",
            case.id
        ),
    }
}

fn remove_json_pointer(value: &mut Value, pointer: &str) {
    if pointer.is_empty() {
        *value = Value::Null;
        return;
    }

    let Some(pointer) = pointer.strip_prefix('/') else {
        return;
    };
    let tokens = pointer
        .split('/')
        .map(decode_json_pointer)
        .collect::<Vec<_>>();
    remove_pointer_at(value, &tokens);
}

fn decode_json_pointer(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

fn remove_pointer_at(value: &mut Value, tokens: &[String]) -> bool {
    if tokens.is_empty() {
        *value = Value::Null;
        return true;
    }

    if tokens.len() == 1 {
        match value {
            Value::Object(object) => object.remove(&tokens[0]).is_some(),
            Value::Array(items) => tokens[0]
                .parse::<usize>()
                .ok()
                .filter(|index| *index < items.len())
                .map(|index| {
                    items.remove(index);
                    true
                })
                .unwrap_or(false),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
        }
    } else {
        match value {
            Value::Object(object) => object
                .get_mut(&tokens[0])
                .map(|child| remove_pointer_at(child, &tokens[1..]))
                .unwrap_or(false),
            Value::Array(items) => tokens[0]
                .parse::<usize>()
                .ok()
                .and_then(|index| items.get_mut(index))
                .map(|child| remove_pointer_at(child, &tokens[1..]))
                .unwrap_or(false),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
        }
    }
}

fn unified_diff(expected: &str, actual: &str) -> String {
    let expected_lines = expected.lines().collect::<Vec<_>>();
    let actual_lines = actual.lines().collect::<Vec<_>>();
    let rows = expected_lines.len();
    let cols = actual_lines.len();
    let mut lcs = vec![vec![0usize; cols + 1]; rows + 1];

    for row in (0..rows).rev() {
        for col in (0..cols).rev() {
            lcs[row][col] = if expected_lines[row] == actual_lines[col] {
                lcs[row + 1][col + 1] + 1
            } else {
                lcs[row + 1][col].max(lcs[row][col + 1])
            };
        }
    }

    let mut rendered = String::from("--- expected\n+++ actual\n");
    let mut row = 0;
    let mut col = 0;
    let mut emitted = 0usize;
    while row < rows || col < cols {
        if row < rows && col < cols && expected_lines[row] == actual_lines[col] {
            push_diff_line(&mut rendered, ' ', expected_lines[row], &mut emitted);
            row += 1;
            col += 1;
        } else if col < cols && (row == rows || lcs[row][col + 1] >= lcs[row + 1][col]) {
            push_diff_line(&mut rendered, '+', actual_lines[col], &mut emitted);
            col += 1;
        } else if row < rows {
            push_diff_line(&mut rendered, '-', expected_lines[row], &mut emitted);
            row += 1;
        }

        if emitted >= 240 {
            rendered.push_str("... diff truncated after 240 lines\n");
            break;
        }
    }

    rendered
}

fn push_diff_line(rendered: &mut String, prefix: char, line: &str, emitted: &mut usize) {
    if prefix != ' ' || *emitted > 0 {
        rendered.push(prefix);
        rendered.push_str(line);
        rendered.push('\n');
        *emitted += 1;
    }
}

fn singbox_vless_ws_tls_mux_outbound() -> Value {
    let generated = generate_singbox_config(&singbox_vless_ws_tls_mux_context())
        .expect("sing-box config should generate");
    serde_json::to_value(
        generated
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == PROXY_TAG)
            .expect("proxy outbound"),
    )
    .expect("sing-box outbound serializes")
}

fn singbox_vless_ws_tls_mux_context() -> CoreConfigContext {
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
        tls: Some(tls_settings(
            "tls.example",
            &["h2", "http/1.1"],
            vec![
                "ech.example".to_string(),
                "https://dns.example/dns-query".to_string(),
            ],
        )),
        ..ProfileItem::default()
    };

    singbox_context(config, node)
}

fn singbox_vless_tls_fragment_hello_outbound() -> Value {
    let generated = generate_singbox_config(&singbox_vless_tls_fragment_hello_context())
        .expect("sing-box config should generate");
    serde_json::to_value(
        generated
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == PROXY_TAG)
            .expect("proxy outbound"),
    )
    .expect("sing-box outbound serializes")
}

fn singbox_vless_tls_fragment_hello_context() -> CoreConfigContext {
    let mut config = AppConfig::default();
    config.core_basic_item.tls_fragment = crate::TlsFragmentMode::TlsHello;
    config.core_basic_item.fragment_fallback_delay_ms = 800;

    let node = ProfileItem {
        index_id: "n-vless-fragment".to_string(),
        remarks: "vless-fragment".to_string(),
        protocol: ProfileProtocol::Vless {
            server: endpoint("server.example", 443),
            uuid: "00000000-0000-0000-0000-000000000012".to_string(),
            flow: None,
            encryption: Some("none".to_string()),
        },
        transport: Some(raw_transport()),
        tls: Some(tls_settings("tls.example", &[], Vec::new())),
        ..ProfileItem::default()
    };

    singbox_context(config, node)
}

fn singbox_tuic_tls_outbound() -> Value {
    let mut config = AppConfig::default();
    config.core_basic_item.def_fingerprint = "chrome".to_string();
    let node = ProfileItem {
        index_id: "n-tuic".to_string(),
        remarks: "tuic-tls".to_string(),
        protocol: ProfileProtocol::Tuic {
            server: endpoint("tuic.example", 443),
            uuid: "00000000-0000-0000-0000-000000000021".to_string(),
            password: "tuic-pass".to_string(),
            congestion_control: Some("bbr".to_string()),
        },
        tls: Some(tls_settings("tuic.example", &["h3"], Vec::new())),
        ..ProfileItem::default()
    };

    singbox_proxy_outbound(config, node)
}

fn singbox_anytls_tls_outbound() -> Value {
    let mut config = AppConfig::default();
    config.core_basic_item.def_fingerprint = "safari".to_string();
    let node = ProfileItem {
        index_id: "n-anytls".to_string(),
        remarks: "anytls-tls".to_string(),
        protocol: ProfileProtocol::Anytls {
            server: endpoint("anytls.example", 8443),
            password: "anytls-pass".to_string(),
        },
        tls: Some(tls_settings(
            "anytls.example",
            &["h2", "http/1.1"],
            Vec::new(),
        )),
        ..ProfileItem::default()
    };

    singbox_proxy_outbound(config, node)
}

fn singbox_naive_quic_tls_outbound() -> Value {
    let mut config = AppConfig::default();
    config.core_basic_item.def_fingerprint = "edge".to_string();
    let node = ProfileItem {
        index_id: "n-naive".to_string(),
        remarks: "naive-quic".to_string(),
        protocol: ProfileProtocol::Naive {
            server: endpoint("naive.example", 443),
            username: "naive-user".to_string(),
            password: "naive-pass".to_string(),
            quic: true,
            congestion_control: Some("bbr".to_string()),
            insecure_concurrency: Some(4),
            udp_over_tcp: true,
        },
        tls: Some(tls_settings("naive.example", &["h3"], Vec::new())),
        ..ProfileItem::default()
    };

    singbox_proxy_outbound(config, node)
}

fn singbox_hysteria2_minimal_outbound() -> Value {
    proxy_outbound_of(singbox_hysteria2_minimal_context())
}

fn singbox_hysteria2_minimal_context() -> CoreConfigContext {
    // A very common real-world link shape: IP host, self-signed certificate and
    // no `sni`, so the parsed profile carries no TLS hints at all. sing-box
    // rejects a hysteria2 outbound without a TLS block, so the generator has to
    // supply one for the TLS-only protocols.
    let node = ProfileItem {
        index_id: "n-hy2".to_string(),
        remarks: "hysteria2-minimal".to_string(),
        protocol: ProfileProtocol::Hysteria2 {
            server: endpoint("203.0.113.5", 443),
            password: "hy2-pass".to_string(),
            port_hops: None,
            obfuscation_password: None,
        },
        tls: None,
        ..ProfileItem::default()
    };

    singbox_context(AppConfig::default(), node)
}

fn singbox_vmess_h2_tls_outbound() -> Value {
    proxy_outbound_of(singbox_vmess_h2_tls_context())
}

fn singbox_vmess_h2_tls_context() -> CoreConfigContext {
    let node = ProfileItem {
        index_id: "n-h2".to_string(),
        remarks: "vmess-h2".to_string(),
        protocol: ProfileProtocol::Vmess {
            server: endpoint("h2.example", 443),
            uuid: "00000000-0000-0000-0000-000000000031".to_string(),
            cipher: Some("auto".to_string()),
        },
        transport: Some(ProfileTransport::Http2 {
            host: Some("h2-one.example,h2-two.example".to_string()),
            path: Some("/h2".to_string()),
        }),
        tls: Some(tls_settings("h2.example", &[], Vec::new())),
        ..ProfileItem::default()
    };

    singbox_context(AppConfig::default(), node)
}

fn singbox_vless_quic_tls_outbound() -> Value {
    proxy_outbound_of(singbox_vless_quic_tls_context())
}

fn singbox_vless_quic_tls_context() -> CoreConfigContext {
    let node = ProfileItem {
        index_id: "n-quic".to_string(),
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
        tls: Some(tls_settings("quic.example", &[], Vec::new())),
        ..ProfileItem::default()
    };

    singbox_context(AppConfig::default(), node)
}

fn singbox_shadowsocks_plugin_contexts() -> [CoreConfigContext; 2] {
    // The SIP003 option string is built once for both the share link and the
    // generated config (`crate::protocol_common::shadowsocks_plugin_for`), so
    // this fixture is what pins the generated half of that contract.
    let obfs = ProfileItem {
        index_id: "n-ss-obfs".to_string(),
        remarks: "ss-obfs".to_string(),
        protocol: ProfileProtocol::Shadowsocks {
            server: endpoint("ss.example", 8388),
            password: "ss-pass".to_string(),
            method: "aes-256-gcm".to_string(),
            udp_over_tcp: false,
        },
        transport: Some(ProfileTransport::Tcp {
            header: Some("http".to_string()),
            // `obfs-host` is a single authority: only the first entry is used.
            host: Some("obfs.example,second.example".to_string()),
            path: None,
        }),
        tls: None,
        ..ProfileItem::default()
    };
    let websocket = ProfileItem {
        index_id: "n-ss-ws".to_string(),
        remarks: "ss-ws".to_string(),
        protocol: ProfileProtocol::Shadowsocks {
            server: endpoint("ss.example", 8389),
            password: "ss-pass".to_string(),
            method: "aes-256-gcm".to_string(),
            udp_over_tcp: true,
        },
        transport: Some(ProfileTransport::Websocket {
            host: Some("ws.example".to_string()),
            path: Some("/ws".to_string()),
        }),
        tls: Some(tls_settings("ws.example", &[], Vec::new())),
        ..ProfileItem::default()
    };

    [
        singbox_context(AppConfig::default(), obfs),
        singbox_context(AppConfig::default(), websocket),
    ]
}

fn singbox_shadowsocks_plugins_outbounds() -> Value {
    let [obfs, websocket] = singbox_shadowsocks_plugin_contexts();
    serde_json::json!({
        "obfs": proxy_outbound_of(obfs),
        "v2rayPlugin": proxy_outbound_of(websocket),
    })
}
fn singbox_bind_interface_windows_context() -> CoreConfigContext {
    // Windows applies `bind_interface` with TUN off; every other platform needs
    // TUN for it to take effect (`singbox::support::apply_outbound_bind_interface`).
    let mut config = AppConfig::default();
    config.core_basic_item.bind_interface = Some("eth0".to_string());
    let node = ProfileItem {
        index_id: "n-win".to_string(),
        remarks: "win-bind".to_string(),
        protocol: ProfileProtocol::Vmess {
            server: endpoint("bind.example", 443),
            uuid: "00000000-0000-0000-0000-000000000041".to_string(),
            cipher: Some("auto".to_string()),
        },
        transport: Some(raw_transport()),
        tls: Some(tls_settings("bind.example", &[], Vec::new())),
        ..ProfileItem::default()
    };
    let mut context = singbox_context(config, node);
    context.platform = CoreGenPlatform::Windows;
    context
}

fn singbox_bind_interface_windows_outbounds() -> Value {
    let generated = generate_singbox_config(&singbox_bind_interface_windows_context())
        .expect("Windows bind_interface config should generate");
    serde_json::to_value(generated.outbounds).expect("Windows outbounds serialize")
}

fn singbox_proxy_outbound(config: AppConfig, node: ProfileItem) -> Value {
    proxy_outbound_of(singbox_context(config, node))
}

fn proxy_outbound_of(context: CoreConfigContext) -> Value {
    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    serde_json::to_value(
        generated
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == PROXY_TAG)
            .expect("proxy outbound"),
    )
    .expect("sing-box outbound serializes")
}
fn singbox_fakeip_typed_dns() -> Value {
    let (dns_context, _) = singbox_routing_dns_contexts();
    serde_json::to_value(
        generate_singbox_config(&dns_context)
            .expect("sing-box config should generate")
            .dns
            .expect("sing-box dns"),
    )
    .expect("sing-box dns serializes")
}

fn singbox_rulesets_from_dns() -> Value {
    let (dns_context, _) = singbox_routing_dns_contexts();
    serde_json::to_value(
        generate_singbox_config(&dns_context)
            .expect("sing-box config should generate")
            .route
            .rule_set
            .expect("sing-box route rulesets"),
    )
    .expect("sing-box rulesets serialize")
}

fn singbox_tun_inbounds() -> Value {
    let (_, tun_context) = singbox_routing_dns_contexts();
    let generated = generate_singbox_config(&tun_context).expect("sing-box config should generate");
    serde_json::to_value(generated.inbounds).expect("sing-box inbounds serialize")
}

fn singbox_tun_inbounds_macos() -> Value {
    let mut tun_config = AppConfig::default();
    tun_config.tun_mode_item.enable_tun = true;
    tun_config.tun_mode_item.mtu = 9000;
    tun_config.tun_mode_item.stack = String::new();
    tun_config.tun_mode_item.strict_route = true;
    tun_config.simple_dns_item.add_common_hosts = Some(false);
    tun_config.simple_dns_item.block_binding_query = Some(false);
    let mut tun_context = singbox_context(tun_config, singbox_base_remote_node());
    tun_context.is_tun_enabled = true;
    tun_context.platform = CoreGenPlatform::MacOS;

    let generated = generate_singbox_config(&tun_context).expect("sing-box config should generate");
    serde_json::to_value(generated.inbounds).expect("sing-box macOS inbounds serialize")
}

fn singbox_tun_route() -> Value {
    let (_, tun_context) = singbox_routing_dns_contexts();
    let generated = generate_singbox_config(&tun_context).expect("sing-box config should generate");
    serde_json::to_value(generated.route).expect("sing-box route serializes")
}

fn singbox_default_seed_context() -> CoreConfigContext {
    let mut context = singbox_context(AppConfig::default(), singbox_socks_node("active", "Active"));
    context.routing_item = Some(crate::default_routing_item("Smart routing"));
    context
}

fn singbox_default_seed_snapshot() -> Value {
    seed_route_and_dns_snapshot(&singbox_default_seed_context())
}

/// The default seed where IPv6 is limited: switched off, and switched on with
/// a node recorded as having no IPv6 egress.
fn singbox_default_seed_ipv6_contexts() -> [(&'static str, CoreConfigContext); 2] {
    let mut off = singbox_default_seed_context();
    off.app_config.tun_mode_item.enable_ipv6_address = false;
    let mut direct_only = singbox_default_seed_context();
    direct_only.ipv6_egress_unsupported = true;
    [("off", off), ("directOnly", direct_only)]
}

fn singbox_default_seed_ipv6_snapshot() -> Value {
    Value::Object(
        singbox_default_seed_ipv6_contexts()
            .iter()
            .map(|(label, context)| ((*label).to_string(), seed_route_and_dns_snapshot(context)))
            .collect(),
    )
}

fn seed_route_and_dns_snapshot(context: &CoreConfigContext) -> Value {
    let config =
        generate_singbox_config_value(context).expect("default seed config should generate");
    serde_json::json!({
        "route": {
            "rules": config["route"]["rules"],
            "rule_set": config["route"]["rule_set"],
            "final": config["route"]["final"],
        },
        "dns": {
            "rules": config["dns"]["rules"],
            "strategy": config["dns"]["strategy"],
        },
    })
}

const POLICY_GROUP_STRATEGIES: [GroupStrategy; 3] = [
    GroupStrategy::Selector,
    GroupStrategy::UrlTest,
    GroupStrategy::Fallback,
];

fn singbox_policy_group_context(strategy: GroupStrategy) -> CoreConfigContext {
    // Two members share a name and their first eight id characters, so their
    // tags have to fall back to the full id to stay distinct.
    let members = vec![
        singbox_socks_node("a1b2c3d4e5", "Tokyo"),
        singbox_socks_node("f6a7b8c9d0", "Osaka"),
        singbox_socks_node("a1b2c3d4ff", "Tokyo"),
    ];
    let group = PolicyGroupItem {
        id: format!("group-{}", strategy.as_db_str()),
        name: "Asia".to_string(),
        strategy,
        selected_profile_id: Some("f6a7b8c9d0".to_string()),
        interval_seconds: Some(300),
        tolerance_ms: Some(80),
        member_ids: members
            .iter()
            .map(|member| member.index_id.clone())
            .collect(),
        ..PolicyGroupItem::default()
    };
    let mut context = singbox_context(AppConfig::default(), members[0].clone());
    for member in &members {
        context
            .all_proxies_map
            .insert(member.index_id.clone(), member.clone());
    }
    context.policy_group = Some(ContextPolicyGroup { group, members });
    context
}

fn singbox_policy_group_configs() -> Vec<Value> {
    POLICY_GROUP_STRATEGIES
        .into_iter()
        .map(|strategy| {
            generate_singbox_config_value(&singbox_policy_group_context(strategy))
                .expect("policy group config should generate")
        })
        .collect()
}

fn singbox_policy_groups_snapshot() -> Value {
    let [selector, urltest, fallback] = POLICY_GROUP_STRATEGIES.map(|strategy| {
        generate_singbox_config_value(&singbox_policy_group_context(strategy))
            .expect("policy group config should generate")["outbounds"]
            .clone()
    });
    serde_json::json!({ "selector": selector, "urltest": urltest, "fallback": fallback })
}

/// The macOS runtime config with latency probes: the active node stays the
/// `proxy` outbound and every probe-worthy node, the active one included, gets
/// an unrouted `probe:` outbound. A WireGuard node and a node without a usable
/// port get none.
fn singbox_latency_probes_context() -> CoreConfigContext {
    let active = singbox_socks_node("active", "Active");
    let mut context = singbox_context(AppConfig::default(), active.clone());
    context.platform = CoreGenPlatform::MacOS;
    context.latency_probe_nodes = vec![
        active,
        singbox_hysteria2_minimal_context().node,
        ProfileItem {
            index_id: "wg".to_string(),
            remarks: "WireGuard".to_string(),
            protocol: ProfileProtocol::WireGuard {
                server: endpoint("198.51.100.7", 51820),
                private_key: "cHJpdmF0ZS1rZXktcHJpdmF0ZS1rZXktcHJpdmF0ZTA=".to_string(),
                peer_public_key: Some("cHVibGljLWtleS1wdWJsaWMta2V5LXB1YmxpYy1rZXk=".to_string()),
                preshared_key: None,
                interface_address: Some("10.0.0.2/32".to_string()),
                allowed_ips: None,
                reserved: None,
                mtu: None,
            },
            ..ProfileItem::default()
        },
        ProfileItem {
            protocol: ProfileProtocol::Socks {
                server: endpoint(LOOPBACK, 0),
                username: String::new(),
                password: String::new(),
            },
            ..singbox_socks_node("no-port", "No port")
        },
    ];
    context
}

fn singbox_latency_probes_config() -> Value {
    generate_singbox_config_value(&singbox_latency_probes_context())
        .expect("latency probe config should generate")
}

fn singbox_latency_probes_snapshot() -> Value {
    let config = singbox_latency_probes_config();
    serde_json::json!({
        "outbounds": config["outbounds"],
        "final": config["route"]["final"],
    })
}

fn singbox_pre_socks_configs() -> Vec<Value> {
    let mut config = AppConfig::default();
    config.tun_mode_item.enable_tun = true;
    config.tun_mode_item.enable_ipv6_address = false;
    config.simple_dns_item.add_common_hosts = Some(false);
    config.simple_dns_item.block_binding_query = Some(false);

    let mut main_context = singbox_vless_ws_tls_mux_context();
    main_context.app_config = config.clone();
    main_context.simple_dns_item = config.simple_dns_item.clone();
    main_context.is_tun_enabled = false;

    let mut pre_context = singbox_context(
        config,
        ProfileItem {
            index_id: "pre-socks".to_string(),
            remarks: "pre-socks".to_string(),
            protocol: ProfileProtocol::Socks {
                server: endpoint(LOOPBACK, 20_808),
                username: String::new(),
                password: String::new(),
            },
            transport: Some(raw_transport()),
            ..ProfileItem::default()
        },
    );
    pre_context.is_tun_enabled = true;

    [main_context, pre_context]
        .into_iter()
        .map(|context| {
            generate_singbox_config_value(&context).expect("pre-socks config should generate")
        })
        .collect()
}

fn singbox_pre_socks_snapshot() -> Value {
    let configs = singbox_pre_socks_configs();
    // `clashApi` is part of the snapshot because the split assigns the base
    // api2 port to the main process and api2 + 1 to the TUN process; a client
    // that re-derives the port from `tun_mode_item.enable_tun` talks to the
    // wrong core.
    serde_json::json!({
        "main": {
            "inbounds": configs[0]["inbounds"],
            "outbounds": configs[0]["outbounds"],
            "routeFinal": configs[0]["route"]["final"],
            "clashApi": configs[0]["experimental"]["clash_api"],
        },
        "preSocks": {
            "inbounds": configs[1]["inbounds"],
            "outbounds": configs[1]["outbounds"],
            "routeFinal": configs[1]["route"]["final"],
            "clashApi": configs[1]["experimental"]["clash_api"],
        },
    })
}

fn singbox_per_rule_outbound_config() -> Value {
    let rule_node = singbox_socks_node("rule-node", "RuleNode");
    let mut context = singbox_context(AppConfig::default(), singbox_socks_node("active", "Active"));
    context
        .all_proxies_map
        .insert("remark:RuleNode".to_string(), rule_node);
    context.routing_item = Some(RoutingItem {
        rule_set: vec![
            RulesItem {
                id: "direct".to_string(),
                outbound_tag: Some(DIRECT_TAG.to_string()),
                domain: Some(vec!["full:direct.example".to_string()]),
                rule_type: Some(RuleType::Routing),
                ..RulesItem::default()
            },
            RulesItem {
                id: "block".to_string(),
                outbound_tag: Some(BLOCK_TAG.to_string()),
                domain: Some(vec!["full:block.example".to_string()]),
                rule_type: Some(RuleType::Routing),
                ..RulesItem::default()
            },
            RulesItem {
                id: "node".to_string(),
                outbound_tag: Some("RuleNode".to_string()),
                domain: Some(vec!["domain:node.example".to_string()]),
                rule_type: Some(RuleType::Routing),
                ..RulesItem::default()
            },
        ],
        ..RoutingItem::default()
    });
    generate_singbox_config_value(&context).expect("per-rule config should generate")
}

fn singbox_per_rule_outbound_snapshot() -> Value {
    let config = singbox_per_rule_outbound_config();
    let rules = config["route"]["rules"]
        .as_array()
        .expect("route rules")
        .iter()
        .filter(|rule| {
            rule.pointer("/domain/0")
                .or_else(|| rule.pointer("/domain_suffix/0"))
                .and_then(Value::as_str)
                .is_some_and(|domain| domain.ends_with(".example"))
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "outbounds": config["outbounds"],
        "route": {
            "final": config["route"]["final"],
            "rules": rules,
        },
    })
}

fn singbox_logs_and_api_config() -> Value {
    let mut config = AppConfig::default();
    config.core_basic_item.log_enabled = true;
    config.core_basic_item.loglevel = "info".to_string();
    generate_singbox_config_value(&singbox_context(
        config,
        singbox_socks_node("runtime", "Runtime"),
    ))
    .expect("runtime config should generate")
}

fn singbox_logs_and_api_snapshot() -> Value {
    let config = singbox_logs_and_api_config();
    serde_json::json!({
        "experimental": config["experimental"],
        "log": config["log"],
    })
}

fn singbox_routing_dns_contexts() -> (CoreConfigContext, CoreConfigContext) {
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
    let mut dns_context = singbox_context(dns_config, singbox_base_remote_node());
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
    let mut tun_context = singbox_context(tun_config, singbox_base_remote_node());
    tun_context.is_tun_enabled = true;

    (dns_context, tun_context)
}

fn tls_settings(server_name: &str, alpn: &[&str], ech_config: Vec<String>) -> TlsSettings {
    full_tls_settings(TlsMode::Tls, Some(server_name), alpn, ech_config)
}

#[test]
fn golden_matrix_manifest_loads_fixture_files() {
    let matrix = load_matrix();
    assert_eq!(matrix.version, 1, "unexpected golden matrix version");
    assert!(!matrix.cases.is_empty(), "golden matrix is empty");

    let mut ids = BTreeSet::new();
    for case in &matrix.cases {
        assert!(
            ids.insert(case.id.clone()),
            "duplicate golden case id {}",
            case.id
        );
        assert!(
            case.core == "sing-box",
            "golden case {} has unsupported core {}",
            case.id,
            case.core
        );
        assert!(
            !case.summary.trim().is_empty(),
            "golden case {} lacks summary",
            case.id
        );
        assert!(
            !case.reference_paths.is_empty(),
            "golden case {} lacks reference paths",
            case.id
        );
        assert!(
            !case.hotspots.is_empty(),
            "golden case {} lacks hotspot tags",
            case.id
        );
        if case.core_acceptance {
            assert!(
                !acceptance_configs_for_case(case).is_empty(),
                "golden case {} lacks an acceptance config",
                case.id
            );
        }
        for volatile in &case.volatile_fields {
            assert!(
                !volatile.reason.trim().is_empty(),
                "golden case {} volatile field {} lacks a reason",
                case.id,
                volatile.pointer
            );
        }
        let fixture = load_fixture(case);
        let canonical = canonical_json_string(&fixture);
        assert!(
            canonical.ends_with('\n'),
            "golden case {} canonical fixture lacks trailing newline",
            case.id
        );
    }
}

#[test]
fn golden_matrix_generated_sections_match_reference_fixtures() {
    let matrix = load_matrix();
    for case in &matrix.cases {
        assert_fixture_matches(case, generated_value_for_case(case));
    }
}

#[test]
fn golden_core_acceptance_checks_are_opt_in() {
    if env::var_os("VOYA_GOLDEN_ACCEPTANCE").is_none() {
        println!(
            "golden core acceptance skipped: set VOYA_GOLDEN_ACCEPTANCE=1 and install sing-box"
        );
        return;
    }

    // Opted in, nothing may be skipped: `pnpm check:sing-box` only proves the
    // configs are accepted if every one of them reached the core.
    let binary = find_binary("VOYA_SINGBOX_BIN", "sing-box").unwrap_or_else(|| {
        panic!("VOYA_GOLDEN_ACCEPTANCE is set but sing-box was not found; set VOYA_SINGBOX_BIN")
    });
    let matrix = load_matrix();
    let mut checked = 0_usize;
    for case in matrix.cases.iter().filter(|case| case.core_acceptance) {
        for (index, config) in acceptance_configs_for_case(case).iter().enumerate() {
            run_core_check(
                &format!("{}-{index}", case.id),
                &binary,
                &["check", "-c"],
                config,
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no golden case is marked core_acceptance");
    println!("golden core acceptance: sing-box accepted {checked} configs");
}

fn acceptance_configs_for_case(case: &GoldenCase) -> Vec<Value> {
    match case.generated.as_str() {
        "singbox.outbound.vless_ws_tls_mux" => {
            vec![
                generate_singbox_config_value(&singbox_vless_ws_tls_mux_context())
                    .expect("VLESS acceptance config should generate"),
            ]
        }
        "singbox.outbound.vless_tls_fragment_hello" => {
            vec![
                generate_singbox_config_value(&singbox_vless_tls_fragment_hello_context())
                    .expect("VLESS fragment acceptance config should generate"),
            ]
        }
        "singbox.runtime.pre_socks" => singbox_pre_socks_configs(),
        "singbox.route.default_seed" => {
            vec![
                generate_singbox_config_value(&singbox_default_seed_context())
                    .expect("default seed acceptance config should generate"),
            ]
        }
        "singbox.route.default_seed_ipv6" => singbox_default_seed_ipv6_contexts()
            .iter()
            .map(|(_, context)| {
                generate_singbox_config_value(context)
                    .expect("default seed IPv6 acceptance config should generate")
            })
            .collect(),
        "singbox.outbound.policy_groups" => singbox_policy_group_configs(),
        "singbox.outbound.latency_probes" => vec![singbox_latency_probes_config()],
        "singbox.routing.per_rule_outbound" => vec![singbox_per_rule_outbound_config()],
        "singbox.runtime.logs_and_api" => vec![singbox_logs_and_api_config()],
        "singbox.outbound.hysteria2_minimal" => {
            vec![
                generate_singbox_config_value(&singbox_hysteria2_minimal_context())
                    .expect("Hysteria2 acceptance config should generate"),
            ]
        }
        "singbox.outbound.vmess_h2_tls" => {
            vec![
                generate_singbox_config_value(&singbox_vmess_h2_tls_context())
                    .expect("VMess h2 acceptance config should generate"),
            ]
        }
        "singbox.outbound.vless_quic_tls" => {
            vec![
                generate_singbox_config_value(&singbox_vless_quic_tls_context())
                    .expect("VLESS quic acceptance config should generate"),
            ]
        }
        "singbox.outbound.shadowsocks_plugins" => singbox_shadowsocks_plugin_contexts()
            .into_iter()
            .map(|context| {
                generate_singbox_config_value(&context)
                    .expect("Shadowsocks plugin acceptance config should generate")
            })
            .collect(),
        "singbox.route.bind_interface_windows" => {
            vec![
                generate_singbox_config_value(&singbox_bind_interface_windows_context())
                    .expect("Windows bind_interface acceptance config should generate"),
            ]
        }
        generated => selfhost::acceptance_configs(generated).unwrap_or_default(),
    }
}

fn run_core_check(label: &str, binary: &Path, args_before_config: &[&str], config: &Value) {
    let config_path = write_temp_config(label, &canonical_json_string(config));
    let mut command = Command::new(binary);
    for arg in args_before_config {
        command.arg(arg);
    }
    command.arg(&config_path);

    let output = command.output().unwrap_or_else(|err| {
        panic!(
            "failed to run {label} acceptance command {}: {err}",
            binary.display()
        )
    });
    let _ = fs::remove_file(&config_path);

    assert!(
        output.status.success(),
        "{label} acceptance failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn find_binary(env_var: &str, binary_name: &str) -> Option<PathBuf> {
    if let Some(path) = env::var_os(env_var).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }

    let path = env::var_os("PATH")?;
    for directory in env::split_paths(&path) {
        for candidate in binary_candidates(binary_name) {
            let candidate = directory.join(candidate);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn binary_candidates(binary_name: &str) -> Vec<String> {
    let mut candidates = vec![binary_name.to_string()];
    if !binary_name.ends_with(".exe") {
        candidates.push(format!("{binary_name}.exe"));
    }
    candidates
}

fn write_temp_config(label: &str, contents: &str) -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!(
        "voyavpn-{label}-golden-{}-{}.json",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(&path, contents)
        .unwrap_or_else(|err| panic!("failed to write temp config {}: {err}", path.display()));
    path
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

#[test]
fn generated_configs_only_have_ordinary_nodes_and_platform_forwarding() {
    let mut checked = 0;
    for case in load_matrix().cases {
        for config in acceptance_configs_for_case(&case) {
            // Only the policy group case carries a group, and only as the single
            // `proxy` outbound; no case ever chains outbounds.
            let group_case = case.generated == "singbox.outbound.policy_groups";
            for outbound in config["outbounds"].as_array().expect("outbounds") {
                assert!(
                    !matches!(outbound["type"].as_str(), Some("selector" | "urltest"))
                        || (group_case && outbound["tag"] == PROXY_TAG),
                    "{}: {outbound}",
                    case.id
                );
                assert!(
                    outbound.get("detour").is_none(),
                    "{}: user chain {outbound}",
                    case.id
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[cfg(test)]
mod selfhost;
