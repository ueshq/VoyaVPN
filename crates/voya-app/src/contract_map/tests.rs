//! Mapping-layer round trips.
//!
//! Struct literals guarantee that every field is *assigned*; nothing but
//! distinct values proves it is assigned to the *right* field. Every
//! same-typed neighbour here (`url`/`more_url`, `custom_icon`/
//! `custom_ruleset_path4_singbox`, `ip`/`domain`/`protocol`/`process`,
//! `direct`/`remote`/`bootstrap`, `total_up`/`total_down`/`today_up`/
//! `today_down`, `delay`/`sort`) would swap silently otherwise: the compiler
//! cannot tell two `String`s or two `i64`s apart.

use voya_core::{
    ProfileExItem, ProfileItem, ProfileListItem, ProfileProtocol as CoreProfileProtocol,
    ProfileTransport as CoreProfileTransport, RoutingItem, RuleType, RulesItem,
    ServerEndpoint as CoreServerEndpoint, ServerStatItem, SimpleDnsItem, SubItem,
    TlsMode as CoreTlsMode, TlsSettings as CoreTlsSettings,
};

use voya_contracts::SpeedtestOutcome;

use super::*;
use crate::supervisor::{SupervisorConnectionState, SupervisorSnapshot};

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
        rule_set: vec![distinct_rule()],
        enabled: true,
        locked: false,
        custom_icon: "custom-icon".to_string(),
        custom_ruleset_path4_singbox: "/tmp/custom-ruleset".to_string(),
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
        auto_update_interval_minutes: Some(120),
    };

    assert_eq!(
        subscription_from_contract(subscription_to_contract(subscription.clone())),
        subscription
    );
}

#[test]
fn dns_mapping_round_trips_every_distinct_field() {
    let item = SimpleDnsItem {
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
    };

    assert_eq!(
        simple_dns_from_contract(simple_dns_to_contract(item.clone())),
        item
    );

    let config = voya_core::AppConfig {
        simple_dns_item: item.clone(),
        ..voya_core::AppConfig::default()
    };
    let bundle = crate::settings::save::settings_from_app_config(&config);
    assert_eq!(bundle.dns, simple_dns_to_contract(item.clone()));
    let restored = crate::settings::save::config_from_settings(&bundle, &config);
    assert_eq!(restored.simple_dns_item, item);
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
                    certificate_pem: Some("tls-certificate-pem".to_string()),
                    ech_config: vec!["tls-ech-config".to_string()],
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

#[test]
fn every_system_proxy_mode_round_trips() {
    for mode in [
        voya_core::SysProxyType::ForcedClear,
        voya_core::SysProxyType::ForcedChange,
        voya_core::SysProxyType::Unchanged,
    ] {
        assert_eq!(
            sysproxy_type_from_contract(sysproxy_type_to_contract(mode)),
            mode,
            "{mode:?}"
        );
    }
}

#[test]
fn every_tls_fragment_mode_round_trips() {
    for mode in [
        voya_core::TlsFragmentMode::Off,
        voya_core::TlsFragmentMode::TlsHello,
        voya_core::TlsFragmentMode::Record,
    ] {
        assert_eq!(
            tls_fragment_mode_from_contract(tls_fragment_mode_to_contract(mode)),
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
        connected_duration_ms: Some(1458000),
        state: SupervisorConnectionState::Connected,
        active_profile_id: Some("running-node".to_string()),
        active_tun_backend: Some(voya_platform::tun::TunBackend::MacosPacketTunnel),
        main_pid: Some(4242),
        pre_pid: Some(4243),
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
    assert_eq!(event.connected_duration_ms, Some(1458000));
    assert_eq!(event.connected_duration_ms, response.connected_duration_ms);
    assert_eq!(event.active_profile_id, response.active_profile_id);
    assert_eq!(event.main_pid, response.main_pid);
    assert_eq!(event.pre_pid, response.pre_pid);
    assert_eq!(event.active_tun_backend, response.active_tun_backend);
    assert_eq!(
        event.active_tun_backend,
        Some(voya_contracts::TunBackend::MacosPacketTunnel)
    );
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
    assert_eq!(connecting.connected_duration_ms, None);
    assert_eq!(connecting.main_pid, None);
    assert_eq!(connecting.pre_pid, None);
    assert_eq!(connecting.active_tun_backend, None);

    let disconnected = runtime_status_response(SupervisorSnapshot::disconnected());
    assert!(matches!(
        disconnected.state,
        voya_contracts::CoreState::Disconnected
    ));

    // Pending cleanup is settled, so the response must keep it rather than
    // collapsing it into either neighbour.
    let cleanup_pending = runtime_status_response(SupervisorSnapshot {
        state: SupervisorConnectionState::CleanupPending,
        active_profile_id: Some("stuck-node".to_string()),
        main_pid: Some(4242),
        ..SupervisorSnapshot::disconnected()
    });
    assert_eq!(
        cleanup_pending.state,
        voya_contracts::CoreState::CleanupPending
    );
    assert_eq!(
        cleanup_pending.active_profile_id.as_deref(),
        Some("stuck-node")
    );
    assert_eq!(cleanup_pending.main_pid, Some(4242));
    assert_eq!(cleanup_pending.connected_duration_ms, None);
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

/// `profile_details_to_contract` has no inverse, so the pairing is asserted
/// directly: four `i64` traffic counters and an `i32`/`f64` metrics pair are
/// exactly the shape a swap hides in.
#[test]
fn profile_details_keep_metrics_and_traffic_in_their_own_fields() {
    let entry = profile_details_to_contract(ProfileListItem {
        profile: ProfileItem {
            index_id: "profile-index-id".to_string(),
            subscription_id: None,
            display_log: false,
            remarks: "profile-remarks".to_string(),
            protocol: CoreProfileProtocol::Socks {
                server: endpoint("node.example", 1080),
                username: String::new(),
                password: String::new(),
            },
            transport: None,
            tls: None,
        },
        profile_ex: ProfileExItem {
            index_id: "profile-index-id".to_string(),
            delay: 111,
            sort: 333,
            message: Some("timedOut".to_string()),
            ip_info: Some("metrics-ip-info".to_string()),
            country_code: None,
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

/// A summary row pairs a node's kind, address and port from its protocol and
/// keeps the metrics of the full shape; the kind table is exhaustive.
#[test]
fn profile_summaries_carry_kind_address_and_port() {
    let entry = profile_summary_to_contract(ProfileSummaryItem {
        profile: ProfileItem {
            index_id: "summary-id".to_string(),
            subscription_id: Some("sub-1".to_string()),
            display_log: false,
            remarks: "Tokyo".to_string(),
            protocol: CoreProfileProtocol::Socks {
                server: endpoint("node.example", 1080),
                username: String::new(),
                password: String::new(),
            },
            transport: None,
            tls: None,
        },
        profile_ex: ProfileExItem {
            index_id: "summary-id".to_string(),
            delay: 77,
            sort: 20,
            message: Some("completed".to_string()),
            ip_info: None,
            country_code: Some("JP".to_string()),
        },
        is_active: true,
    });

    assert_eq!(entry.profile.id, "summary-id");
    assert_eq!(entry.profile.subscription_id.as_deref(), Some("sub-1"));
    assert_eq!(entry.profile.remarks, "Tokyo");
    assert_eq!(entry.profile.kind, ProfileKind::Socks);
    assert_eq!(
        (entry.profile.address.as_str(), entry.profile.port),
        ("node.example", 1080)
    );
    assert_eq!(entry.metrics.delay_ms, 77);
    assert_eq!(entry.metrics.sort, 20);
    assert_eq!(entry.metrics.outcome, Some(SpeedtestOutcome::Completed));
    assert_eq!(entry.metrics.country_code.as_deref(), Some("JP"));
    assert!(entry.is_active);

    // The same kind the full profile's protocol reports, for every kind.
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
        let kind = serde_json::to_value(profile_kind_to_contract(config_type))
            .expect("the kind serializes");
        assert_eq!(kind.as_str(), Some(config_type.as_str()), "{config_type:?}");
    }
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

#[test]
fn statistics_snapshot_mapping_keeps_each_rate_in_its_own_field() {
    let contract = statistics_snapshot_to_contract(crate::statistics::StatisticsSnapshot {
        active_profile_id: Some("profile".to_string()),
        proxy_upload_bytes_per_second: 1.0,
        proxy_download_bytes_per_second: 2.0,
        direct_upload_bytes_per_second: 3.0,
        direct_download_bytes_per_second: 4.0,
        upload_bytes_per_second: 5.0,
        download_bytes_per_second: 6.0,
        server_stat: None,
    });

    assert_eq!(contract.active_profile_id.as_deref(), Some("profile"));
    assert_eq!(contract.proxy_upload_bytes_per_second, 1.0);
    assert_eq!(contract.proxy_download_bytes_per_second, 2.0);
    assert_eq!(contract.direct_upload_bytes_per_second, 3.0);
    assert_eq!(contract.direct_download_bytes_per_second, 4.0);
    assert_eq!(contract.upload_bytes_per_second, 5.0);
    assert_eq!(contract.download_bytes_per_second, 6.0);
    assert!(contract.server_stat.is_none());
}

#[test]
fn system_proxy_status_mapping_keeps_requested_and_effective_modes_apart() {
    for (management, expected) in [
        (
            voya_platform::sysproxy::SystemProxyManagement::Automatic,
            voya_contracts::SystemProxyManagement::Automatic,
        ),
        (
            voya_platform::sysproxy::SystemProxyManagement::Unsupported,
            voya_contracts::SystemProxyManagement::Unsupported,
        ),
    ] {
        let contract =
            system_proxy_status_to_contract(voya_platform::sysproxy::SystemProxyStatus {
                management,
                requested_type: voya_core::SysProxyType::ForcedChange,
                effective_type: voya_core::SysProxyType::ForcedClear,
                target_os: voya_platform::coreinfo::TargetOs::Linux,
                proxy: Some("127.0.0.1:10808".to_string()),
                exceptions: "localhost".to_string(),
            });

        assert_eq!(contract.management, expected);
        assert_eq!(
            contract.requested_mode,
            voya_contracts::SystemProxyType::ForcedChange
        );
        assert_eq!(
            contract.effective_mode,
            voya_contracts::SystemProxyType::ForcedClear
        );
        assert_eq!(contract.proxy.as_deref(), Some("127.0.0.1:10808"));
        assert_eq!(contract.exceptions, "localhost");
    }
}
