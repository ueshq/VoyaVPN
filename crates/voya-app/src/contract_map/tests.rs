//! Mapping-layer round trips.
//!
//! Struct literals guarantee that every field is *assigned*; nothing but
//! distinct values proves it is assigned to the *right* field. Every
//! same-typed neighbour here (`url`/`more_url`, `domain_strategy`/
//! `domain_strategy4_singbox`, `ip`/`domain`/`protocol`/`process`,
//! `direct`/`remote`/`bootstrap`, `total_up`/`total_down`/`today_up`/
//! `today_down`, `delay`/`speed`) would swap silently otherwise: the compiler
//! cannot tell two `String`s or two `i64`s apart.

use voya_core::{
    ConfigType, MultipleLoad, ProfileExItem, ProfileItem, ProfileListItem,
    ProfileProtocol as CoreProfileProtocol, ProfileSortKey as CoreProfileSortKey,
    ProfileTransport as CoreProfileTransport, RoutingItem, RuleType, RulesItem,
    ServerEndpoint as CoreServerEndpoint, ServerStatItem, SimpleDnsItem, SubItem,
    TlsMode as CoreTlsMode, TlsSettings as CoreTlsSettings,
};

use voya_contracts::{ProfileKind, SpeedtestOutcome};

use super::*;
use crate::{
    dns::DnsSettings,
    supervisor::{SupervisorConnectionState, SupervisorSnapshot},
};

fn distinct_rule() -> RulesItem {
    RulesItem {
        id: "rule-id".to_string(),
        r#type: Some("rule-type".to_string()),
        port: Some("1000-2000".to_string()),
        network: Some("tcp".to_string()),
        inbound_tag: Some(vec!["inbound-tag".to_string()]),
        outbound_tag: Some("outbound-tag".to_string()),
        ip: Some(vec!["ip-value".to_string()]),
        domain: Some(vec!["domain-value".to_string()]),
        protocol: Some(vec!["protocol-value".to_string()]),
        process: Some(vec!["process-value".to_string()]),
        enabled: true,
        remarks: Some("rule-remarks".to_string()),
        rule_type: Some(RuleType::DNS),
    }
}

fn distinct_routing() -> RoutingItem {
    RoutingItem {
        id: "routing-id".to_string(),
        remarks: "routing-remarks".to_string(),
        url: "https://routing.test/source".to_string(),
        rule_set: vec![distinct_rule()],
        enabled: true,
        locked: false,
        custom_icon: "custom-icon".to_string(),
        custom_ruleset_path4_singbox: "/tmp/custom-ruleset".to_string(),
        domain_strategy: "domain-strategy".to_string(),
        domain_strategy4_singbox: "singbox-domain-strategy".to_string(),
        sort: 7,
        is_active: true,
    }
}

#[test]
fn rule_mapping_round_trips_every_distinct_field() {
    let rule = distinct_rule();

    assert_eq!(rule_from_contract(rule_to_contract(rule.clone())), rule);
}

#[test]
fn routing_mapping_round_trips_every_distinct_field() {
    let routing = distinct_routing();

    assert_eq!(
        routing_from_contract(routing_to_contract(routing.clone())),
        routing
    );
}

#[test]
fn rule_scope_round_trips_every_variant() {
    for scope in [RuleType::ALL, RuleType::Routing, RuleType::DNS] {
        let rule = RulesItem {
            rule_type: Some(scope),
            ..distinct_rule()
        };
        assert_eq!(
            rule_from_contract(rule_to_contract(rule.clone())).rule_type,
            Some(scope),
            "{scope:?}"
        );
    }
}

#[test]
fn subscription_mapping_round_trips_every_distinct_field() {
    let subscription = SubItem {
        id: "subscription-id".to_string(),
        remarks: "subscription-remarks".to_string(),
        url: "https://subscription.test/primary".to_string(),
        more_url: "https://subscription.test/additional".to_string(),
        enabled: true,
        user_agent: "user-agent".to_string(),
        sort: 11,
        filter: Some("filter-value".to_string()),
        convert_target: Some("convert-target".to_string()),
        pre_socks_port: Some(10809),
        auto_update_interval_minutes: Some(120),
    };

    assert_eq!(
        subscription_from_contract(subscription_to_contract(subscription.clone())),
        subscription
    );
}

#[test]
fn dns_mapping_round_trips_every_distinct_field() {
    let settings = DnsSettings {
        simple_dns_item: SimpleDnsItem {
            add_common_hosts: Some(false),
            fake_ip: Some(true),
            global_fake_ip: Some(false),
            block_binding_query: Some(true),
            direct_dns: Some("direct-dns".to_string()),
            remote_dns: Some("remote-dns".to_string()),
            bootstrap_dns: Some("bootstrap-dns".to_string()),
            strategy4_freedom: Some("direct-strategy".to_string()),
            strategy4_proxy: Some("proxy-strategy".to_string()),
            hosts: Some("hosts-value".to_string()),
            direct_expected_ips: Some("direct-expected-ips".to_string()),
        },
    };

    assert_eq!(
        dns_from_contract(dns_to_contract(settings.clone())),
        settings
    );
}

#[test]
fn profile_mapping_round_trips_every_distinct_field() {
    let profile = ProfileItem {
        index_id: "profile-index-id".to_string(),
        subscription_id: Some("profile-subscription-id".to_string()),
        display_log: true,
        remarks: "profile-remarks".to_string(),
        protocol: CoreProfileProtocol::Socks {
            server: CoreServerEndpoint {
                address: "socks.test".to_string(),
                port: 1080,
            },
            username: "socks-username".to_string(),
            password: "socks-password".to_string(),
        },
        transport: None,
        tls: None,
    };

    assert_eq!(
        profile_from_contract(profile_to_contract(profile.clone())),
        profile
    );
}

fn endpoint(address: &str, port: i32) -> CoreServerEndpoint {
    CoreServerEndpoint {
        address: address.to_string(),
        port,
    }
}

/// Every protocol, with every optional field populated by a value distinct from
/// its neighbours.
///
/// Same-typed neighbours are the whole risk here: `uuid`/`password`,
/// `username`/`password`, `flow`/`encryption`, `privateKey`/`peerPublicKey`.
/// A transposition between any pair compiles, passes clippy and passes the
/// bindings-drift gate — only distinct sentinels catch it.
fn every_protocol() -> Vec<(&'static str, CoreProfileProtocol)> {
    vec![
        (
            "vmess",
            CoreProfileProtocol::Vmess {
                server: endpoint("vmess.test", 10_001),
                uuid: "vmess-uuid".to_string(),
                cipher: Some("vmess-cipher".to_string()),
            },
        ),
        (
            "custom",
            CoreProfileProtocol::Custom {
                source: "custom-source".to_string(),
                filter: Some("custom-filter".to_string()),
            },
        ),
        (
            "shadowsocks",
            CoreProfileProtocol::Shadowsocks {
                server: endpoint("shadowsocks.test", 10_002),
                password: "shadowsocks-password".to_string(),
                method: "shadowsocks-method".to_string(),
                udp_over_tcp: true,
            },
        ),
        (
            "socks",
            CoreProfileProtocol::Socks {
                server: endpoint("socks.test", 10_003),
                username: "socks-username".to_string(),
                password: "socks-password".to_string(),
            },
        ),
        (
            "vless",
            CoreProfileProtocol::Vless {
                server: endpoint("vless.test", 10_004),
                uuid: "vless-uuid".to_string(),
                flow: Some("vless-flow".to_string()),
                encryption: Some("vless-encryption".to_string()),
            },
        ),
        (
            "trojan",
            CoreProfileProtocol::Trojan {
                server: endpoint("trojan.test", 10_005),
                password: "trojan-password".to_string(),
            },
        ),
        (
            "hysteria2",
            CoreProfileProtocol::Hysteria2 {
                server: endpoint("hysteria2.test", 10_006),
                password: "hysteria2-password".to_string(),
                port_hops: Some("hysteria2-port-hops".to_string()),
                obfuscation_password: Some("hysteria2-obfuscation".to_string()),
            },
        ),
        (
            "tuic",
            CoreProfileProtocol::Tuic {
                server: endpoint("tuic.test", 10_007),
                uuid: "tuic-uuid".to_string(),
                password: "tuic-password".to_string(),
                congestion_control: Some("tuic-congestion".to_string()),
            },
        ),
        (
            "wireGuard",
            CoreProfileProtocol::WireGuard {
                server: endpoint("wireguard.test", 10_008),
                private_key: "wireguard-private-key".to_string(),
                peer_public_key: Some("wireguard-peer-public-key".to_string()),
                preshared_key: Some("wireguard-preshared-key".to_string()),
                interface_address: Some("wireguard-interface-address".to_string()),
                allowed_ips: Some("wireguard-allowed-ips".to_string()),
                reserved: Some("wireguard-reserved".to_string()),
                mtu: Some(1_408),
            },
        ),
        (
            "http",
            CoreProfileProtocol::Http {
                server: endpoint("http.test", 10_009),
                username: "http-username".to_string(),
                password: "http-password".to_string(),
            },
        ),
        (
            "anytls",
            CoreProfileProtocol::Anytls {
                server: endpoint("anytls.test", 10_010),
                password: "anytls-password".to_string(),
            },
        ),
        (
            "naive",
            CoreProfileProtocol::Naive {
                server: endpoint("naive.test", 10_011),
                username: "naive-username".to_string(),
                password: "naive-password".to_string(),
                quic: true,
                congestion_control: Some("naive-congestion".to_string()),
                insecure_concurrency: Some(7),
                udp_over_tcp: true,
            },
        ),
        (
            "policyGroup",
            CoreProfileProtocol::PolicyGroup {
                child_profile_ids: vec!["policy-child".to_string()],
                source_subscription_id: Some("policy-subscription".to_string()),
                filter: Some("policy-filter".to_string()),
                strategy: MultipleLoad::RoundRobin,
            },
        ),
        (
            "proxyChain",
            CoreProfileProtocol::ProxyChain {
                child_profile_ids: vec!["chain-child".to_string()],
            },
        ),
    ]
}

fn every_transport() -> Vec<(&'static str, CoreProfileTransport)> {
    vec![
        (
            "tcp",
            CoreProfileTransport::Tcp {
                header: Some("tcp-header".to_string()),
                host: Some("tcp-host".to_string()),
                path: Some("tcp-path".to_string()),
            },
        ),
        (
            "kcp",
            CoreProfileTransport::Kcp {
                header: Some("kcp-header".to_string()),
                seed: Some("kcp-seed".to_string()),
                mtu: Some(1_350),
            },
        ),
        (
            "websocket",
            CoreProfileTransport::Websocket {
                host: Some("websocket-host".to_string()),
                path: Some("websocket-path".to_string()),
            },
        ),
        (
            "httpUpgrade",
            CoreProfileTransport::HttpUpgrade {
                host: Some("http-upgrade-host".to_string()),
                path: Some("http-upgrade-path".to_string()),
            },
        ),
        (
            "xhttp",
            CoreProfileTransport::Xhttp {
                host: Some("xhttp-host".to_string()),
                path: Some("xhttp-path".to_string()),
                mode: Some("xhttp-mode".to_string()),
                extra: Some("xhttp-extra".to_string()),
            },
        ),
        (
            "http2",
            CoreProfileTransport::Http2 {
                host: Some("http2-host".to_string()),
                path: Some("http2-path".to_string()),
            },
        ),
        (
            "grpc",
            CoreProfileTransport::Grpc {
                authority: Some("grpc-authority".to_string()),
                service_name: Some("grpc-service-name".to_string()),
                mode: Some("grpc-mode".to_string()),
            },
        ),
        (
            "quic",
            CoreProfileTransport::Quic {
                host: Some("quic-host".to_string()),
                path: Some("quic-path".to_string()),
            },
        ),
    ]
}

fn every_tls() -> Vec<(&'static str, CoreTlsSettings)> {
    [CoreTlsMode::Tls, CoreTlsMode::Reality]
        .into_iter()
        .map(|mode| {
            let label = match mode {
                CoreTlsMode::Tls => "tls",
                CoreTlsMode::Reality => "reality",
            };
            (
                label,
                CoreTlsSettings {
                    mode,
                    server_name: Some("tls-server-name".to_string()),
                    alpn: vec!["tls-alpn".to_string()],
                    reality_public_key: Some("tls-reality-public-key".to_string()),
                    reality_short_id: Some("tls-reality-short-id".to_string()),
                    reality_spider_x: Some("tls-reality-spider-x".to_string()),
                    mldsa65_verify: Some("tls-mldsa65-verify".to_string()),
                    certificate_pem: Some("tls-certificate-pem".to_string()),
                    certificate_sha256: vec!["tls-certificate-sha256".to_string()],
                    ech_config: vec!["tls-ech-config".to_string()],
                    final_mask: Some("tls-final-mask".to_string()),
                },
            )
        })
        .collect()
}

#[test]
fn every_protocol_round_trips_through_the_contract() {
    for (label, protocol) in every_protocol() {
        let profile = ProfileItem {
            index_id: format!("{label}-id"),
            subscription_id: Some(format!("{label}-subscription")),
            display_log: true,
            remarks: format!("{label}-remarks"),
            protocol,
            transport: None,
            tls: None,
        };

        assert_eq!(
            profile_from_contract(profile_to_contract(profile.clone())),
            profile,
            "{label}"
        );
    }
}

#[test]
fn every_transport_round_trips_through_the_contract() {
    for (label, transport) in every_transport() {
        let profile = ProfileItem {
            index_id: format!("{label}-id"),
            subscription_id: None,
            display_log: false,
            remarks: format!("{label}-remarks"),
            protocol: CoreProfileProtocol::Trojan {
                server: endpoint("transport.test", 443),
                password: "transport-password".to_string(),
            },
            transport: Some(transport),
            tls: None,
        };

        assert_eq!(
            profile_from_contract(profile_to_contract(profile.clone())),
            profile,
            "{label}"
        );
    }
}

#[test]
fn every_tls_mode_round_trips_with_all_eleven_fields_populated() {
    for (label, tls) in every_tls() {
        let profile = ProfileItem {
            index_id: format!("{label}-id"),
            subscription_id: None,
            display_log: false,
            remarks: format!("{label}-remarks"),
            protocol: CoreProfileProtocol::Vless {
                server: endpoint("tls.test", 443),
                uuid: "tls-uuid".to_string(),
                flow: Some("tls-flow".to_string()),
                encryption: Some("tls-encryption".to_string()),
            },
            transport: None,
            tls: Some(tls),
        };

        assert_eq!(
            profile_from_contract(profile_to_contract(profile.clone())),
            profile,
            "{label}"
        );
    }
}

/// The full cross product, so a protocol that only breaks when a transport or a
/// TLS block is attached cannot hide behind the single-axis tests above.
#[test]
fn protocol_transport_and_tls_round_trip_together() {
    for (protocol_label, protocol) in every_protocol() {
        for (transport_label, transport) in every_transport() {
            for (tls_label, tls) in every_tls() {
                let profile = ProfileItem {
                    index_id: "combined-id".to_string(),
                    subscription_id: Some("combined-subscription".to_string()),
                    display_log: true,
                    remarks: "combined-remarks".to_string(),
                    protocol: protocol.clone(),
                    transport: Some(transport.clone()),
                    tls: Some(tls),
                };

                assert_eq!(
                    profile_from_contract(profile_to_contract(profile.clone())),
                    profile,
                    "{protocol_label} + {transport_label} + {tls_label}"
                );
            }
        }
    }
}

/// `profile_sort_key_from_contract` renames three keys (`Protocol` ->
/// `ConfigType`, `Transport` -> `Network`, `Tls` -> `StreamSecurity`), so a
/// mismatch sorts the table by the wrong column and nothing fails.
#[test]
fn every_sort_key_maps_to_its_renamed_domain_key() {
    let cases = [
        (ProfileSortKey::Sort, CoreProfileSortKey::Sort),
        (ProfileSortKey::Protocol, CoreProfileSortKey::ConfigType),
        (ProfileSortKey::Remarks, CoreProfileSortKey::Remarks),
        (ProfileSortKey::Address, CoreProfileSortKey::Address),
        (ProfileSortKey::Port, CoreProfileSortKey::Port),
        (ProfileSortKey::Transport, CoreProfileSortKey::Network),
        (ProfileSortKey::Tls, CoreProfileSortKey::StreamSecurity),
        (ProfileSortKey::Delay, CoreProfileSortKey::Delay),
        (ProfileSortKey::Speed, CoreProfileSortKey::Speed),
        (ProfileSortKey::IpInfo, CoreProfileSortKey::IpInfo),
        (
            ProfileSortKey::SubscriptionId,
            CoreProfileSortKey::SubscriptionId,
        ),
    ];

    for (contract, domain) in cases {
        assert_eq!(
            profile_sort_key_from_contract(contract),
            domain,
            "{contract:?}"
        );
    }

    // A new key without a row above fails to compile here.
    const fn exhaustive(key: ProfileSortKey) {
        match key {
            ProfileSortKey::Sort
            | ProfileSortKey::Protocol
            | ProfileSortKey::Remarks
            | ProfileSortKey::Address
            | ProfileSortKey::Port
            | ProfileSortKey::Transport
            | ProfileSortKey::Tls
            | ProfileSortKey::Delay
            | ProfileSortKey::Speed
            | ProfileSortKey::IpInfo
            | ProfileSortKey::SubscriptionId => (),
        }
    }
    exhaustive(ProfileSortKey::Sort);
}

#[test]
fn every_load_strategy_round_trips() {
    for strategy in [
        MultipleLoad::LeastPing,
        MultipleLoad::Fallback,
        MultipleLoad::Random,
        MultipleLoad::RoundRobin,
        MultipleLoad::LeastLoad,
    ] {
        let protocol = CoreProfileProtocol::PolicyGroup {
            child_profile_ids: vec!["child".to_string()],
            source_subscription_id: None,
            filter: None,
            strategy,
        };
        let profile = ProfileItem {
            protocol,
            ..ProfileItem::default()
        };

        assert_eq!(
            profile_from_contract(profile_to_contract(profile.clone())),
            profile,
            "{strategy:?}"
        );
    }
}

/// `profile_kind` is the only mapping with no inverse, so every `ConfigType`
/// gets an explicit row instead of a round trip.
#[test]
fn every_config_type_maps_to_its_profile_kind() {
    let cases = [
        (ConfigType::VMess, ProfileKind::Vmess),
        (ConfigType::Custom, ProfileKind::Custom),
        (ConfigType::Shadowsocks, ProfileKind::Shadowsocks),
        (ConfigType::SOCKS, ProfileKind::Socks),
        (ConfigType::VLESS, ProfileKind::Vless),
        (ConfigType::Trojan, ProfileKind::Trojan),
        (ConfigType::Hysteria2, ProfileKind::Hysteria2),
        (ConfigType::TUIC, ProfileKind::Tuic),
        (ConfigType::WireGuard, ProfileKind::WireGuard),
        (ConfigType::HTTP, ProfileKind::Http),
        (ConfigType::Anytls, ProfileKind::Anytls),
        (ConfigType::Naive, ProfileKind::Naive),
        (ConfigType::PolicyGroup, ProfileKind::PolicyGroup),
        (ConfigType::ProxyChain, ProfileKind::ProxyChain),
    ];

    for (config_type, kind) in cases {
        let candidate = group_child_to_contract(GroupChildCandidate {
            index_id: "child-id".to_string(),
            remarks: "child-remarks".to_string(),
            address: "child-address".to_string(),
            config_type,
            subscription_id: None,
            is_group: false,
            selectable: true,
            reason: None,
        });
        assert_eq!(candidate.protocol, kind, "{config_type:?}");
    }
}

#[test]
fn every_system_proxy_mode_round_trips() {
    for mode in [
        voya_core::SysProxyType::ForcedClear,
        voya_core::SysProxyType::ForcedChange,
        voya_core::SysProxyType::Unchanged,
        voya_core::SysProxyType::Pac,
    ] {
        assert_eq!(
            sysproxy_type_from_contract(sysproxy_type_to_contract(mode)),
            mode,
            "{mode:?}"
        );
    }
}

#[test]
fn every_traffic_mode_round_trips() {
    for mode in [
        voya_core::TrafficMode::Rule,
        voya_core::TrafficMode::Global,
        voya_core::TrafficMode::Direct,
        voya_core::TrafficMode::Unchanged,
    ] {
        assert_eq!(
            traffic_mode_from_contract(traffic_mode_to_contract(mode)),
            mode,
            "{mode:?}"
        );
    }
}

/// The runtime status is now one type for both the command answer and the
/// `coreState` event, so the two builders have to agree on every field.
///
/// A settled supervisor can only be connected or disconnected;
/// `runtime_status_event` adds the two transitions and, when it has a snapshot,
/// must fill the other four fields exactly as the response does. The event also
/// has to prefer the snapshot's active profile over the id the flow was called
/// with, and fall back to that id when there is no snapshot yet — which is the
/// whole reason it takes both.
#[test]
fn the_status_response_and_the_status_event_agree_on_every_field() {
    let snapshot = SupervisorSnapshot {
        state: SupervisorConnectionState::Connected,
        active_profile_id: Some("running-node".to_string()),
        main_pid: Some(4242),
        pre_pid: Some(4243),
        running_core_type: Some(voya_core::CoreType::sing_box),
        ..SupervisorSnapshot::disconnected()
    };

    let response = runtime_status_response(snapshot.clone());
    let event = runtime_status_event(
        voya_contracts::CoreState::Connected,
        Some("requested-node".to_string()),
        Some(&snapshot),
    );

    assert!(matches!(
        response.state,
        voya_contracts::CoreState::Connected
    ));
    assert!(matches!(event.state, voya_contracts::CoreState::Connected));
    assert_eq!(event.active_profile_id, response.active_profile_id);
    assert_eq!(event.main_pid, response.main_pid);
    assert_eq!(event.pre_pid, response.pre_pid);
    assert_eq!(event.running_core_type, response.running_core_type);
    // The snapshot wins over the id the flow was invoked with.
    assert_eq!(event.active_profile_id.as_deref(), Some("running-node"));

    // No snapshot: the transition still names the profile it is connecting to,
    // and reports no pids because there is no process to report yet.
    let connecting = runtime_status_event(
        voya_contracts::CoreState::Connecting,
        Some("requested-node".to_string()),
        None,
    );
    assert!(matches!(
        connecting.state,
        voya_contracts::CoreState::Connecting
    ));
    assert_eq!(
        connecting.active_profile_id.as_deref(),
        Some("requested-node")
    );
    assert_eq!(connecting.main_pid, None);
    assert_eq!(connecting.pre_pid, None);
    assert_eq!(connecting.running_core_type, None);

    let disconnected = runtime_status_response(SupervisorSnapshot::disconnected());
    assert!(matches!(
        disconnected.state,
        voya_contracts::CoreState::Disconnected
    ));
}

#[test]
fn every_move_action_maps_to_its_domain_action() {
    let cases = [
        (MoveAction::Top, CoreMoveAction::Top),
        (MoveAction::Up, CoreMoveAction::Up),
        (MoveAction::Down, CoreMoveAction::Down),
        (MoveAction::Bottom, CoreMoveAction::Bottom),
        (MoveAction::Position, CoreMoveAction::Position),
    ];

    for (contract, domain) in cases {
        assert_eq!(move_action_from_contract(contract), domain, "{contract:?}");
    }
}

/// `profile_list_to_contract` has no inverse, so the pairing is asserted
/// directly: four `i64` traffic counters and an `i32`/`f64` metrics pair are
/// exactly the shape a swap hides in.
#[test]
fn profile_list_entry_keeps_metrics_and_traffic_in_their_own_fields() {
    let entry = profile_list_to_contract(ProfileListItem {
        profile: ProfileItem {
            index_id: "profile-index-id".to_string(),
            subscription_id: None,
            display_log: false,
            remarks: "profile-remarks".to_string(),
            protocol: CoreProfileProtocol::Custom {
                source: "custom-source".to_string(),
                filter: None,
            },
            transport: None,
            tls: None,
        },
        profile_ex: ProfileExItem {
            index_id: "profile-index-id".to_string(),
            delay: 111,
            speed: 222.0,
            sort: 333,
            message: Some("timedOut".to_string()),
            ip_info: Some("metrics-ip-info".to_string()),
        },
        server_stat: ServerStatItem {
            index_id: "profile-index-id".to_string(),
            total_up: 41,
            total_down: 42,
            today_up: 43,
            today_down: 44,
            date_now: 45,
        },
        is_active: true,
    });

    assert_eq!(entry.metrics.delay_ms, 111);
    assert!((entry.metrics.speed_bytes_per_second - 222.0).abs() < f64::EPSILON);
    assert_eq!(entry.metrics.sort, 333);
    assert_eq!(entry.metrics.outcome, Some(SpeedtestOutcome::TimedOut));
    assert_eq!(entry.metrics.ip_info.as_deref(), Some("metrics-ip-info"));
    assert_eq!(entry.traffic.total_upload, 41);
    assert_eq!(entry.traffic.total_download, 42);
    assert_eq!(entry.traffic.today_upload, 43);
    assert_eq!(entry.traffic.today_download, 44);
    assert_eq!(entry.traffic.date, 45);
    assert!(entry.is_active);
}

#[test]
fn server_stat_mapping_keeps_each_counter_in_its_own_field() {
    let contract = server_stat_to_contract(ServerStatItem {
        index_id: "profile-index-id".to_string(),
        total_up: 51,
        total_down: 52,
        today_up: 53,
        today_down: 54,
        date_now: 55,
    });

    assert_eq!(contract.total_up, 51);
    assert_eq!(contract.total_down, 52);
    assert_eq!(contract.today_up, 53);
    assert_eq!(contract.today_down, 54);
    assert_eq!(contract.date_now, 55);
}
