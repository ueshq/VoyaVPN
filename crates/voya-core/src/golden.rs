use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::{
    generate_singbox_config, generate_singbox_config_value, generate_xray_config,
    generate_xray_config_value, AppConfig, ConfigType, CoreConfigContext, CoreGenPlatform,
    CoreType, FullConfigTemplateItem, MultipleLoad, ProfileItem, ProtocolExtraItem, RoutingItem,
    RuleType, RulesItem, TransportExtraItem, DIRECT_TAG, LOOPBACK, PROXY_TAG,
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
        "xray.outbound.vless_tls_xhttp_fragment" => xray_vless_tls_xhttp_fragment_outbound(),
        "xray.outbound.policy_group_least_load" => xray_policy_group_least_load(),
        "xray.full.inbounds_stats_tun" => xray_inbounds_stats_tun(),
        "xray.full.advanced_dns_routing" => xray_advanced_dns_routing(),
        "xray.full.template_tun_proxy_detour" => xray_template_tun_proxy_detour(),
        "singbox.outbound.vless_ws_tls_mux" => singbox_vless_ws_tls_mux_outbound(),
        "singbox.outbound.proxy_chain_detour" => singbox_proxy_chain_detour(),
        "singbox.outbound.policy_group_selector" => singbox_policy_group_selector(),
        "singbox.dns.fakeip_typed" => singbox_fakeip_typed_dns(),
        "singbox.route.rulesets_from_dns" => singbox_rulesets_from_dns(),
        "singbox.inbounds.tun" => singbox_tun_inbounds(),
        "singbox.route.tun" => singbox_tun_route(),
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

fn xray_vless_tls_xhttp_fragment_outbound() -> Value {
    let mut config = AppConfig::default();
    config.core_basic_item.enable_fragment = true;
    config.core_basic_item.mux_enabled = true;
    config.core_basic_item.def_fingerprint = "firefox".to_string();
    config.speed_test_item.speed_ping_test_url = "https://ping.example/generate_204".to_string();

    let node = ProfileItem {
        index_id: "n-vless".to_string(),
        config_type: ConfigType::VLESS,
        remarks: "vless-xhttp".to_string(),
        address: "server.example".to_string(),
        port: 443,
        password: "00000000-0000-0000-0000-000000000001".to_string(),
        network: "xhttp".to_string(),
        stream_security: "tls".to_string(),
        sni: "tls.example".to_string(),
        alpn: "h2,http/1.1".to_string(),
        fingerprint: "chrome".to_string(),
        ech_config_list: "tls.example+https://ech.example/config".to_string(),
        mux_enabled: Some(true),
        protocol_extra: ProtocolExtraItem {
            vless_encryption: Some("none".to_string()),
            ..ProtocolExtraItem::default()
        },
        transport_extra: TransportExtraItem {
            host: Some("cdn.example".to_string()),
            path: Some("/xhttp".to_string()),
            xhttp_mode: Some("stream-up".to_string()),
            xhttp_extra: Some(r#"{"downloadSettings":{"address":"download.example"}}"#.to_string()),
            ..TransportExtraItem::default()
        },
        ..ProfileItem::default()
    };
    let generated = generate_xray_config(&xray_context(config, node));
    serde_json::to_value(generated.outbounds.first().expect("proxy outbound"))
        .expect("xray outbound serializes")
}

fn xray_policy_group_least_load() -> Value {
    let mut config = AppConfig::default();
    config.speed_test_item.speed_ping_test_url = "https://ping.example/generate_204".to_string();
    let group = ProfileItem {
        index_id: "group".to_string(),
        config_type: ConfigType::PolicyGroup,
        remarks: "least-load".to_string(),
        protocol_extra: ProtocolExtraItem {
            child_items: Some("n1,n2".to_string()),
            multiple_load: Some(MultipleLoad::LeastLoad),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };
    let mut context = xray_context(config, group);
    context
        .all_proxies_map
        .insert("n1".to_string(), xray_socks_node("n1", "node-1"));
    context
        .all_proxies_map
        .insert("n2".to_string(), xray_socks_node("n2", "node-2"));

    let generated = generate_xray_config(&context);
    json!({
        "balancers": generated.routing.balancers,
        "burstObservatory": generated.burst_observatory,
        "observatory": generated.observatory,
    })
}

fn xray_inbounds_stats_tun() -> Value {
    let mut config = AppConfig::default();
    config.inbound[0].local_port = 12000;
    config.inbound[0].second_local_port_enabled = true;
    config.inbound[0].allow_lan_conn = true;
    config.inbound[0].new_port4_lan = true;
    config.inbound[0].user = "lan-user".to_string();
    config.inbound[0].pass = "lan-pass".to_string();
    config.gui_item.enable_statistics = true;
    config.tun_mode_item.mtu = 1408;
    config.tun_mode_item.enable_ipv6_address = false;
    config.core_basic_item.bind_interface = Some("eth0".to_string());
    config.simple_dns_item.add_common_hosts = Some(false);

    let node = ProfileItem {
        index_id: "n-vmess".to_string(),
        config_type: ConfigType::VMess,
        remarks: "vmess".to_string(),
        address: "remote.example".to_string(),
        port: 443,
        password: "00000000-0000-0000-0000-000000000004".to_string(),
        protocol_extra: ProtocolExtraItem {
            vmess_security: Some("auto".to_string()),
            alter_id: Some("0".to_string()),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };
    let mut context = xray_context(config, node);
    context.is_tun_enabled = true;

    let generated = generate_xray_config(&context);
    json!({
        "inbounds": generated.inbounds,
        "metrics": generated.metrics,
        "policy": generated.policy,
        "stats": generated.stats,
        "tunDnsOutbound": generated.outbounds.iter().any(|outbound| outbound.tag == "dns" && outbound.protocol == "dns"),
    })
}

fn xray_advanced_dns_routing() -> Value {
    let mut config = AppConfig::default();
    config.simple_dns_item.add_common_hosts = Some(false);
    config.simple_dns_item.direct_dns =
        Some("119.29.29.29,https://dns.alidns.com/dns-query".to_string());
    config.simple_dns_item.remote_dns = Some("https://cloudflare-dns.com/dns-query".to_string());
    config.simple_dns_item.bootstrap_dns = Some("223.5.5.5".to_string());
    config.simple_dns_item.strategy4_freedom = Some("UseIP".to_string());
    config.simple_dns_item.strategy4_proxy = Some("UseIPv4".to_string());
    config.simple_dns_item.serve_stale = Some(true);
    config.simple_dns_item.parallel_query = Some(true);
    config.simple_dns_item.fake_ip = Some(true);
    config.simple_dns_item.hosts = Some("example.test 1.2.3.4 5.6.7.8\n# ignored".to_string());
    config.simple_dns_item.direct_expected_ips = Some("geoip:cn,1.1.1.1".to_string());

    let node = ProfileItem {
        index_id: "n-vless".to_string(),
        config_type: ConfigType::VLESS,
        remarks: "main".to_string(),
        address: "main.example".to_string(),
        port: 443,
        password: "00000000-0000-0000-0000-000000000005".to_string(),
        protocol_extra: ProtocolExtraItem {
            vless_encryption: Some("none".to_string()),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };
    let mut context = xray_context(config, node);
    context.protect_domain_list = vec!["full:ech.example".to_string()];
    context.routing_item = Some(RoutingItem {
        id: "routing".to_string(),
        domain_strategy: "IPIfNonMatch".to_string(),
        rule_set: vec![
            RulesItem {
                id: "direct-domains".to_string(),
                outbound_tag: Some(DIRECT_TAG.to_string()),
                domain: Some(vec![
                    "geosite:cn".to_string(),
                    "domain:direct.example".to_string(),
                ]),
                rule_type: Some(RuleType::DNS),
                ..RulesItem::default()
            },
            RulesItem {
                id: "proxy-domains".to_string(),
                outbound_tag: Some(PROXY_TAG.to_string()),
                domain: Some(vec![
                    "geosite:google".to_string(),
                    "domain:proxy.example".to_string(),
                ]),
                ..RulesItem::default()
            },
            RulesItem {
                id: "detour".to_string(),
                outbound_tag: Some("detour".to_string()),
                domain: Some(vec!["full:special<COMMA>domain".to_string()]),
                ..RulesItem::default()
            },
            RulesItem {
                id: "final-direct".to_string(),
                outbound_tag: Some(DIRECT_TAG.to_string()),
                ip: Some(vec!["0.0.0.0/0".to_string()]),
                port: Some("0-65535".to_string()),
                network: Some("tcp,udp".to_string()),
                ..RulesItem::default()
            },
        ],
        ..RoutingItem::default()
    });
    context.all_proxies_map.insert(
        "remark:detour".to_string(),
        xray_socks_node("detour-id", "detour-node"),
    );

    let generated = generate_xray_config(&context);
    json!({
        "dns": generated.dns,
        "fakedns": generated.fake_dns,
        "routing": generated.routing,
        "outbounds": generated.outbounds.iter().map(|outbound| {
            json!({
                "tag": outbound.tag,
                "protocol": outbound.protocol,
                "targetStrategy": outbound.target_strategy,
                "settings": outbound.settings,
            })
        }).collect::<Vec<_>>(),
    })
}

fn xray_template_tun_proxy_detour() -> Value {
    let mut config = AppConfig::default();
    config.speed_test_item.speed_ping_test_url = "https://ping.example/generate_204".to_string();
    let group = ProfileItem {
        index_id: "group".to_string(),
        config_type: ConfigType::PolicyGroup,
        remarks: "template-group".to_string(),
        protocol_extra: ProtocolExtraItem {
            child_items: Some("n1,n2".to_string()),
            multiple_load: Some(MultipleLoad::LeastPing),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };
    let mut context = xray_context(config, group);
    context.is_tun_enabled = true;
    context.full_config_template = Some(FullConfigTemplateItem {
        enabled: true,
        core_type: CoreType::Xray,
        add_proxy_only: Some(false),
        proxy_detour: Some("template-detour".to_string()),
        config: Some(r#"{"remarks":"unused"}"#.to_string()),
        tun_config: Some(
            r#"{
                "remarks": "tun-template",
                "routing": {
                    "rules": [
                        { "type": "field", "outboundTag": "proxy", "domain": ["geosite:private"] }
                    ]
                },
                "observatory": { "subjectSelector": ["template-observer"] },
                "outbounds": [
                    { "tag": "template-detour", "protocol": "freedom" }
                ]
            }"#
            .to_string(),
        ),
        ..FullConfigTemplateItem::default()
    });
    context.all_proxies_map.insert(
        "n1".to_string(),
        ProfileItem {
            index_id: "n1".to_string(),
            config_type: ConfigType::VMess,
            remarks: "remote-1".to_string(),
            address: "one.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000006".to_string(),
            protocol_extra: ProtocolExtraItem {
                alter_id: Some("0".to_string()),
                vmess_security: Some("auto".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        },
    );
    context.all_proxies_map.insert(
        "n2".to_string(),
        ProfileItem {
            index_id: "n2".to_string(),
            config_type: ConfigType::VMess,
            remarks: "remote-2".to_string(),
            address: "two.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000007".to_string(),
            protocol_extra: ProtocolExtraItem {
                alter_id: Some("0".to_string()),
                vmess_security: Some("auto".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        },
    );

    let generated = generate_xray_config_value(&context);
    json!({
        "remarks": generated.get("remarks"),
        "routing": generated.get("routing"),
        "observatory": generated.get("observatory"),
        "outbounds": generated.get("outbounds"),
    })
}

fn singbox_vless_ws_tls_mux_outbound() -> Value {
    let mut config = AppConfig::default();
    config.core_basic_item.enable_fragment = true;
    config.core_basic_item.mux_enabled = true;
    config.core_basic_item.def_user_agent = "chrome".to_string();

    let node = ProfileItem {
        index_id: "n-vless".to_string(),
        config_type: ConfigType::VLESS,
        core_type: Some(CoreType::sing_box),
        remarks: "vless-ws".to_string(),
        address: "server.example".to_string(),
        port: 443,
        password: "00000000-0000-0000-0000-000000000011".to_string(),
        network: "ws".to_string(),
        stream_security: "tls".to_string(),
        sni: "tls.example".to_string(),
        alpn: "h2,http/1.1".to_string(),
        fingerprint: "firefox".to_string(),
        ech_config_list: "ech.example+https://dns.example/dns-query".to_string(),
        mux_enabled: Some(true),
        protocol_extra: ProtocolExtraItem {
            vless_encryption: Some("none".to_string()),
            ..ProtocolExtraItem::default()
        },
        transport_extra: TransportExtraItem {
            host: Some("cdn.example".to_string()),
            path: Some("/ws?ed=2048".to_string()),
            ..TransportExtraItem::default()
        },
        ..ProfileItem::default()
    };

    let generated = generate_singbox_config(&singbox_context(config, node));
    serde_json::to_value(
        generated
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == PROXY_TAG)
            .expect("proxy outbound"),
    )
    .expect("sing-box outbound serializes")
}

fn singbox_proxy_chain_detour() -> Value {
    let n1 = singbox_socks_node("n1", "node-1");
    let n2 = singbox_socks_node("n2", "node-2");
    let chain = ProfileItem {
        index_id: "chain".to_string(),
        config_type: ConfigType::ProxyChain,
        core_type: Some(CoreType::sing_box),
        remarks: "chain".to_string(),
        protocol_extra: ProtocolExtraItem {
            child_items: Some("n1,n2".to_string()),
            group_type: Some("ProxyChain".to_string()),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };
    let mut context = singbox_context(AppConfig::default(), chain);
    context.all_proxies_map.insert(n1.index_id.clone(), n1);
    context.all_proxies_map.insert(n2.index_id.clone(), n2);

    serde_json::to_value(generate_singbox_config(&context).outbounds)
        .expect("sing-box outbounds serialize")
}

fn singbox_policy_group_selector() -> Value {
    let n1 = singbox_socks_node("n1", "node-1");
    let n2 = singbox_socks_node("n2", "node-2");
    let group = ProfileItem {
        index_id: "group".to_string(),
        config_type: ConfigType::PolicyGroup,
        core_type: Some(CoreType::sing_box),
        remarks: "fallback".to_string(),
        protocol_extra: ProtocolExtraItem {
            child_items: Some("n1,n1,n2".to_string()),
            group_type: Some("PolicyGroup".to_string()),
            multiple_load: Some(MultipleLoad::Fallback),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };
    let mut context = singbox_context(AppConfig::default(), group);
    context.all_proxies_map.insert(n1.index_id.clone(), n1);
    context.all_proxies_map.insert(n2.index_id.clone(), n2);

    serde_json::to_value(generate_singbox_config(&context).outbounds)
        .expect("sing-box outbounds serialize")
}

fn singbox_fakeip_typed_dns() -> Value {
    let (dns_context, _) = singbox_routing_dns_contexts();
    serde_json::to_value(
        generate_singbox_config(&dns_context)
            .dns
            .expect("sing-box dns"),
    )
    .expect("sing-box dns serializes")
}

fn singbox_rulesets_from_dns() -> Value {
    let (dns_context, _) = singbox_routing_dns_contexts();
    serde_json::to_value(
        generate_singbox_config(&dns_context)
            .route
            .rule_set
            .expect("sing-box route rulesets"),
    )
    .expect("sing-box rulesets serialize")
}

fn singbox_tun_inbounds() -> Value {
    let (_, tun_context) = singbox_routing_dns_contexts();
    serde_json::to_value(generate_singbox_config(&tun_context).inbounds)
        .expect("sing-box inbounds serialize")
}

fn singbox_tun_route() -> Value {
    let (_, tun_context) = singbox_routing_dns_contexts();
    serde_json::to_value(generate_singbox_config(&tun_context).route)
        .expect("sing-box route serializes")
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
    dns_config.simple_dns_item.strategy4_freedom = Some("UseIPv4".to_string());
    dns_config.simple_dns_item.strategy4_proxy = Some("UseIPv6".to_string());
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

fn xray_context(app_config: AppConfig, node: ProfileItem) -> CoreConfigContext {
    let mut all_proxies_map = BTreeMap::new();
    all_proxies_map.insert(node.index_id.clone(), node.clone());
    let simple_dns_item = app_config.simple_dns_item.clone();
    CoreConfigContext {
        node,
        run_core_type: CoreType::Xray,
        app_config,
        simple_dns_item,
        all_proxies_map,
        platform: CoreGenPlatform::Linux,
        ..CoreConfigContext::default()
    }
}

fn singbox_context(app_config: AppConfig, node: ProfileItem) -> CoreConfigContext {
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

fn xray_socks_node(index_id: &str, remarks: &str) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        config_type: ConfigType::SOCKS,
        remarks: remarks.to_string(),
        address: "127.0.0.1".to_string(),
        port: 1080,
        username: "user".to_string(),
        password: "pass".to_string(),
        network: "raw".to_string(),
        mux_enabled: Some(false),
        protocol_extra: ProtocolExtraItem::default(),
        transport_extra: TransportExtraItem::default(),
        ..ProfileItem::default()
    }
}

fn singbox_base_remote_node() -> ProfileItem {
    ProfileItem {
        core_type: Some(CoreType::sing_box),
        remarks: "remote".to_string(),
        address: "server.example".to_string(),
        port: 443,
        network: "raw".to_string(),
        stream_security: "tls".to_string(),
        sni: "server.example".to_string(),
        ..ProfileItem::default()
    }
}

fn singbox_socks_node(index_id: &str, remarks: &str) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        config_type: ConfigType::SOCKS,
        core_type: Some(CoreType::sing_box),
        remarks: remarks.to_string(),
        address: LOOPBACK.to_string(),
        port: 1080,
        username: "user".to_string(),
        password: "pass".to_string(),
        network: "raw".to_string(),
        mux_enabled: Some(false),
        ..ProfileItem::default()
    }
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
            matches!(case.core.as_str(), "xray" | "sing-box"),
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
        let _core_acceptance = case.core_acceptance;
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
            "golden core acceptance skipped: set VOYA_GOLDEN_ACCEPTANCE=1 and install xray/sing-box"
        );
        return;
    }

    let xray_config = acceptance_xray_config();
    run_optional_core_check(
        "xray",
        "VOYA_XRAY_BIN",
        "xray",
        &["run", "-test", "-config"],
        &xray_config,
    );

    let singbox_config = acceptance_singbox_config();
    run_optional_core_check(
        "sing-box",
        "VOYA_SINGBOX_BIN",
        "sing-box",
        &["check", "-c"],
        &singbox_config,
    );
}

fn acceptance_xray_config() -> Value {
    let node = ProfileItem {
        index_id: "accept-xray".to_string(),
        config_type: ConfigType::VMess,
        remarks: "accept-xray".to_string(),
        address: "server.example".to_string(),
        port: 443,
        password: "00000000-0000-0000-0000-000000000031".to_string(),
        protocol_extra: ProtocolExtraItem {
            vmess_security: Some("auto".to_string()),
            alter_id: Some("0".to_string()),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };
    generate_xray_config_value(&xray_context(AppConfig::default(), node))
}

fn acceptance_singbox_config() -> Value {
    generate_singbox_config_value(&singbox_context(
        AppConfig::default(),
        singbox_socks_node("accept-singbox", "accept-singbox"),
    ))
}

fn run_optional_core_check(
    label: &str,
    env_var: &str,
    binary_name: &str,
    args_before_config: &[&str],
    config: &Value,
) {
    let Some(binary) = find_binary(env_var, binary_name) else {
        println!("golden core acceptance skipped for {label}: {binary_name} not found");
        return;
    };

    let config_path = write_temp_config(label, &canonical_json_string(config));
    let mut command = Command::new(&binary);
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

#[allow(dead_code)]
fn ensure_fixture_path(path: &Path) {
    assert!(path.is_file(), "missing golden fixture {}", path.display());
}
