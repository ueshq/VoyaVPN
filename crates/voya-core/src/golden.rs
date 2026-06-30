use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::PathBuf,
    process::Command,
};

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{
    generate_singbox_config, generate_singbox_config_value, AppConfig, ConfigType,
    CoreConfigContext, CoreGenPlatform, CoreType, MultipleLoad, ProfileItem, ProtocolExtraItem,
    RoutingItem, RuleType, RulesItem, TransportExtraItem, DIRECT_TAG, LOOPBACK, PROXY_TAG,
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
        "singbox.outbound.proxy_chain_detour" => singbox_proxy_chain_detour(),
        "singbox.outbound.policy_group_selector" => singbox_policy_group_selector(),
        "singbox.dns.fakeip_typed" => singbox_fakeip_typed_dns(),
        "singbox.route.rulesets_from_dns" => singbox_rulesets_from_dns(),
        "singbox.inbounds.tun" => singbox_tun_inbounds(),
        "singbox.route.tun" => singbox_tun_route(),
        "singbox.outbound.tuic_tls" => singbox_tuic_tls_outbound(),
        "singbox.outbound.anytls_tls" => singbox_anytls_tls_outbound(),
        "singbox.outbound.naive_quic_tls" => singbox_naive_quic_tls_outbound(),
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
    let mut config = AppConfig::default();
    config.core_basic_item.enable_fragment = true;
    config.core_basic_item.mux_enabled = true;
    config.core_basic_item.def_user_agent = "chrome".to_string();

    let node = ProfileItem {
        index_id: "n-vless".to_string(),
        config_type: ConfigType::VLESS,
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

    let generated = generate_singbox_config(&singbox_context(config, node))
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

fn singbox_tuic_tls_outbound() -> Value {
    let node = ProfileItem {
        index_id: "n-tuic".to_string(),
        config_type: ConfigType::TUIC,
        remarks: "tuic-tls".to_string(),
        address: "tuic.example".to_string(),
        port: 443,
        username: "00000000-0000-0000-0000-000000000021".to_string(),
        password: "tuic-pass".to_string(),
        stream_security: "tls".to_string(),
        sni: "tuic.example".to_string(),
        alpn: "h3".to_string(),
        fingerprint: "chrome".to_string(),
        protocol_extra: ProtocolExtraItem {
            congestion_control: Some("bbr".to_string()),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };

    singbox_proxy_outbound(AppConfig::default(), node)
}

fn singbox_anytls_tls_outbound() -> Value {
    let node = ProfileItem {
        index_id: "n-anytls".to_string(),
        config_type: ConfigType::Anytls,
        remarks: "anytls-tls".to_string(),
        address: "anytls.example".to_string(),
        port: 8443,
        password: "anytls-pass".to_string(),
        stream_security: "tls".to_string(),
        sni: "anytls.example".to_string(),
        alpn: "h2,http/1.1".to_string(),
        fingerprint: "safari".to_string(),
        ..ProfileItem::default()
    };

    singbox_proxy_outbound(AppConfig::default(), node)
}

fn singbox_naive_quic_tls_outbound() -> Value {
    let node = ProfileItem {
        index_id: "n-naive".to_string(),
        config_type: ConfigType::Naive,
        remarks: "naive-quic".to_string(),
        address: "naive.example".to_string(),
        port: 443,
        username: "naive-user".to_string(),
        password: "naive-pass".to_string(),
        stream_security: "tls".to_string(),
        sni: "naive.example".to_string(),
        alpn: "h3".to_string(),
        fingerprint: "edge".to_string(),
        protocol_extra: ProtocolExtraItem {
            naive_quic: Some(true),
            congestion_control: Some("bbr".to_string()),
            insecure_concurrency: Some(4),
            uot: Some(true),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    };

    singbox_proxy_outbound(AppConfig::default(), node)
}

fn singbox_proxy_outbound(config: AppConfig, node: ProfileItem) -> Value {
    let generated = generate_singbox_config(&singbox_context(config, node))
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

fn singbox_proxy_chain_detour() -> Value {
    let n1 = singbox_socks_node("n1", "node-1");
    let n2 = singbox_socks_node("n2", "node-2");
    let chain = ProfileItem {
        index_id: "chain".to_string(),
        config_type: ConfigType::ProxyChain,
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

    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    serde_json::to_value(generated.outbounds).expect("sing-box outbounds serialize")
}

fn singbox_policy_group_selector() -> Value {
    let n1 = singbox_socks_node("n1", "node-1");
    let n2 = singbox_socks_node("n2", "node-2");
    let group = ProfileItem {
        index_id: "group".to_string(),
        config_type: ConfigType::PolicyGroup,
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

    let generated = generate_singbox_config(&context).expect("sing-box config should generate");
    serde_json::to_value(generated.outbounds).expect("sing-box outbounds serialize")
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

fn singbox_tun_route() -> Value {
    let (_, tun_context) = singbox_routing_dns_contexts();
    let generated = generate_singbox_config(&tun_context).expect("sing-box config should generate");
    serde_json::to_value(generated.route).expect("sing-box route serializes")
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

fn singbox_base_remote_node() -> ProfileItem {
    ProfileItem {
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
            "golden core acceptance skipped: set VOYA_GOLDEN_ACCEPTANCE=1 and install sing-box"
        );
        return;
    }

    let singbox_config = acceptance_singbox_config();
    run_optional_core_check(
        "sing-box",
        "VOYA_SINGBOX_BIN",
        "sing-box",
        &["check", "-c"],
        &singbox_config,
    );
}

fn acceptance_singbox_config() -> Value {
    generate_singbox_config_value(&singbox_context(
        AppConfig::default(),
        singbox_socks_node("accept-singbox", "accept-singbox"),
    ))
    .expect("sing-box acceptance config should generate")
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
