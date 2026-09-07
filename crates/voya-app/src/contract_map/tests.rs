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
    ProfileExItem, ProfileItem, ProfileListItem, ProfileProtocol as CoreProfileProtocol,
    RoutingItem, RuleType, RulesItem, ServerEndpoint as CoreServerEndpoint, ServerStatItem,
    SimpleDnsItem, SubItem,
};

use super::*;
use crate::dns::DnsSettings;

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
            use_system_hosts: Some(true),
            add_common_hosts: Some(false),
            fake_ip: Some(true),
            global_fake_ip: Some(false),
            block_binding_query: Some(true),
            direct_dns: Some("direct-dns".to_string()),
            remote_dns: Some("remote-dns".to_string()),
            bootstrap_dns: Some("bootstrap-dns".to_string()),
            strategy4_freedom: Some("direct-strategy".to_string()),
            strategy4_proxy: Some("proxy-strategy".to_string()),
            serve_stale: Some(true),
            parallel_query: Some(false),
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
            message: Some("metrics-message".to_string()),
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
    assert_eq!(entry.metrics.message.as_deref(), Some("metrics-message"));
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
