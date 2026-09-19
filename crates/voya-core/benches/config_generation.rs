//! sing-box config generation as the node count grows: the macOS connect
//! config (one latency probe outbound per node), a policy group spanning a
//! whole subscription, and a speedtest page. Run with `pnpm bench:rust`.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use voya_core::{
    generate_singbox_config_json, generate_singbox_speedtest_config_json, AppConfig,
    CoreConfigContextBuilder, CoreGenEnv, CoreGenPlatform, GroupStrategy, InboundProtocol,
    PolicyGroupItem, ProfileItem, ProfileProtocol, ProfileTransport, RoutingItem, ServerEndpoint,
    SpeedtestConfigEntry, TlsMode, TlsSettings,
};

/// `check:rust:test` runs every bench once in the unoptimized test profile to
/// keep them compiling; the smallest size is enough there.
const NODE_COUNTS: &[usize] = if cfg!(debug_assertions) {
    &[100]
} else {
    &[100, 1_000, 3_000]
};

struct BenchEnv;

impl CoreGenEnv for BenchEnv {
    fn platform(&self) -> CoreGenPlatform {
        CoreGenPlatform::MacOS
    }

    fn get_profile_by_remarks(&self, _remarks: &str) -> Option<ProfileItem> {
        None
    }

    fn get_default_routing(&self, _config: &AppConfig) -> Option<RoutingItem> {
        None
    }

    fn get_local_port(&self, protocol: InboundProtocol) -> i32 {
        10_808 + protocol.port_offset()
    }
}

/// A WebSocket + TLS VLESS node, the common subscription shape; the `ed`
/// query exercises the early-data path every WebSocket node goes through.
fn node(index: usize) -> ProfileItem {
    ProfileItem {
        index_id: format!("node-{index:05}"),
        remarks: format!("Node {index}"),
        protocol: ProfileProtocol::Vless {
            server: ServerEndpoint {
                address: format!("n{index}.example.com"),
                port: 443,
            },
            uuid: "d7b8b5a5-3c5e-4f7a-9d3b-2f0e8c1a6b4d".to_string(),
            flow: None,
            encryption: Some("none".to_string()),
        },
        transport: Some(ProfileTransport::Websocket {
            host: Some("cdn.example.com".to_string()),
            path: Some("/ws?ed=2048".to_string()),
        }),
        tls: Some(TlsSettings {
            mode: TlsMode::Tls,
            server_name: Some("cdn.example.com".to_string()),
            alpn: Vec::new(),
            reality_public_key: None,
            reality_short_id: None,
            certificate_pem: None,
            ech_config: Vec::new(),
        }),
        ..ProfileItem::default()
    }
}

fn nodes(count: usize) -> Vec<ProfileItem> {
    (0..count).map(node).collect()
}

fn macos_connect_config(criterion: &mut Criterion) {
    let config = AppConfig::default();
    let env = BenchEnv;
    let builder = CoreConfigContextBuilder::new(&env);
    let mut group = criterion.benchmark_group("macos_connect_with_latency_probes");
    group.sample_size(20);
    for &count in NODE_COUNTS {
        let all = nodes(count);
        group.bench_with_input(BenchmarkId::from_parameter(count), &all, |bench, all| {
            bench.iter(|| {
                let mut context = builder.build(&config, &all[0]).context;
                context.latency_probe_nodes = all.clone();
                black_box(generate_singbox_config_json(&context))
            });
        });
    }
    group.finish();
}

fn policy_group_config(criterion: &mut Criterion) {
    let config = AppConfig::default();
    let env = BenchEnv;
    let builder = CoreConfigContextBuilder::new(&env);
    let mut group = criterion.benchmark_group("policy_group_of_whole_subscription");
    group.sample_size(20);
    for &count in NODE_COUNTS {
        let members = nodes(count);
        let policy_group = PolicyGroupItem {
            id: "group".to_string(),
            name: "Everything".to_string(),
            strategy: GroupStrategy::UrlTest,
            member_ids: members.iter().map(|node| node.index_id.clone()).collect(),
            ..PolicyGroupItem::default()
        };
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &members,
            |bench, members| {
                bench.iter(|| {
                    let result = builder.build_for_group(&config, &policy_group, members);
                    black_box(generate_singbox_config_json(&result.context))
                });
            },
        );
    }
    group.finish();
}

fn speedtest_page_config(criterion: &mut Criterion) {
    let config = AppConfig::default();
    let env = BenchEnv;
    let builder = CoreConfigContextBuilder::new(&env);
    let mut group = criterion.benchmark_group("speedtest_page");
    group.sample_size(20);
    for &count in NODE_COUNTS {
        let all = nodes(count);
        group.bench_with_input(BenchmarkId::from_parameter(count), &all, |bench, all| {
            bench.iter(|| {
                // What a speedtest run does: resolve once, then build each
                // node's outbound-only context from that.
                let contexts = builder.prepare(&config);
                let entries = all
                    .iter()
                    .zip(20_000..)
                    .map(|(node, port)| SpeedtestConfigEntry {
                        index_id: node.index_id.clone(),
                        port,
                        context: contexts.build_node_outbound(node).context,
                    })
                    .collect::<Vec<_>>();
                black_box(generate_singbox_speedtest_config_json(&entries))
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    macos_connect_config,
    policy_group_config,
    speedtest_page_config
);
criterion_main!(benches);
