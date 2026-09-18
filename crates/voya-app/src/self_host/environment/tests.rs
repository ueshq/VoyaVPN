use voya_contracts::{
    SelfHostAddressFamily, SelfHostAddressScope, SelfHostNatKind, SelfHostPortMappingStatus,
    SelfHostReachability, SelfHostReasonCode as Reason,
};
use voya_net::probe::{PortProbeOutcome, PortProbeResult};

use super::*;

fn ip(value: &str) -> IpAddr {
    value.parse().expect("test address")
}

fn result(port: u16, reason: PortProbeOutcome) -> PortProbeResult {
    PortProbeResult {
        port,
        reachable: reason == PortProbeOutcome::Connected,
        reason,
        elapsed_ms: 10,
    }
}

fn ipv4(local: &[&str], public: Option<&str>, probe: ProbeEvidence) -> FamilyEvidence {
    FamilyEvidence {
        family: SelfHostAddressFamily::Ipv4,
        local_addresses: local.iter().map(|address| ip(address)).collect(),
        public_address: public.map(ip),
        probe,
        port_mapping: Some(MappingEvidence {
            status: SelfHostPortMappingStatus::Disabled,
            gateway_external_address: None,
        }),
        firewall_rule_missing: false,
    }
}

fn mapped(
    mut evidence: FamilyEvidence,
    status: SelfHostPortMappingStatus,
    gateway: Option<&str>,
) -> FamilyEvidence {
    evidence.port_mapping = Some(MappingEvidence {
        status,
        gateway_external_address: gateway.map(ip),
    });
    evidence
}

#[test]
fn a_vps_with_its_public_address_on_the_interface_is_reachable() {
    let report = classify_family(&ipv4(
        &["203.0.113.7"],
        Some("203.0.113.7"),
        ProbeEvidence::Results(vec![
            result(42_443, PortProbeOutcome::Connected),
            result(42_444, PortProbeOutcome::Connected),
        ]),
    ));
    assert_eq!(report.nat, SelfHostNatKind::None);
    assert_eq!(report.reachability, SelfHostReachability::Reachable);
    assert!(report.verified_by_probe);
    assert_eq!(report.public_address.as_deref(), Some("203.0.113.7"));
    assert_eq!(
        report.reasons,
        [Reason::PublicAddressOnDevice, Reason::ProbeReachable]
    );
}

#[test]
fn a_home_router_with_a_upnp_forward_is_reachable() {
    let evidence = mapped(
        ipv4(
            &["192.168.1.20"],
            Some("198.51.100.4"),
            ProbeEvidence::Results(vec![result(42_443, PortProbeOutcome::Connected)]),
        ),
        SelfHostPortMappingStatus::Mapped,
        Some("198.51.100.4"),
    );
    let report = classify_family(&evidence);
    assert_eq!(report.nat, SelfHostNatKind::Nat);
    assert_eq!(report.reachability, SelfHostReachability::Reachable);
    assert_eq!(
        report.reasons,
        [
            Reason::BehindNat,
            Reason::PortMapped,
            Reason::ProbeReachable
        ]
    );
}

#[test]
fn a_router_whose_wan_address_is_shared_space_is_carrier_grade_nat() {
    let evidence = mapped(
        ipv4(
            &["192.168.1.20"],
            Some("198.51.100.4"),
            ProbeEvidence::Results(vec![result(42_443, PortProbeOutcome::Timeout)]),
        ),
        SelfHostPortMappingStatus::Mapped,
        Some("100.72.3.9"),
    );
    let report = classify_family(&evidence);
    assert_eq!(report.nat, SelfHostNatKind::CarrierGrade);
    assert_eq!(report.reachability, SelfHostReachability::Unreachable);
    assert!(report.reasons.contains(&Reason::CarrierGradeNat));
    assert!(report.reasons.contains(&Reason::ProbeTimedOut));
}

#[test]
fn a_router_behind_another_router_is_double_nat() {
    let evidence = mapped(
        ipv4(
            &["192.168.1.20"],
            Some("198.51.100.4"),
            ProbeEvidence::Unavailable,
        ),
        SelfHostPortMappingStatus::Mapped,
        Some("10.0.0.2"),
    );
    let report = classify_family(&evidence);
    assert_eq!(report.nat, SelfHostNatKind::Double);
    assert_eq!(report.reachability, SelfHostReachability::NeedsPortForward);
    assert!(!report.verified_by_probe);
    assert!(report.reasons.contains(&Reason::DoubleNat));
    assert!(report.reasons.contains(&Reason::ProbeUnavailable));
}

#[test]
fn a_shared_interface_address_without_upnp_is_carrier_grade_nat() {
    let report = classify_family(&ipv4(
        &["100.80.1.2"],
        Some("198.51.100.4"),
        ProbeEvidence::Unavailable,
    ));
    assert_eq!(report.nat, SelfHostNatKind::CarrierGrade);
    assert_eq!(report.reachability, SelfHostReachability::Unreachable);
}

#[test]
fn nat_without_a_forward_needs_one_when_unprobed() {
    let evidence = mapped(
        ipv4(
            &["192.168.1.20"],
            Some("198.51.100.4"),
            ProbeEvidence::NodeStopped,
        ),
        SelfHostPortMappingStatus::NoGateway,
        None,
    );
    let report = classify_family(&evidence);
    assert_eq!(report.reachability, SelfHostReachability::NeedsPortForward);
    assert_eq!(
        report.reasons,
        [
            Reason::BehindNat,
            Reason::UpnpUnavailable,
            Reason::NodeNotRunning
        ]
    );
}

#[test]
fn a_refused_port_means_nothing_listens_behind_the_forward() {
    let report = classify_family(&ipv4(
        &["203.0.113.7"],
        Some("203.0.113.7"),
        ProbeEvidence::Results(vec![
            result(42_443, PortProbeOutcome::Connected),
            result(42_444, PortProbeOutcome::Refused),
        ]),
    ));
    assert_eq!(report.reachability, SelfHostReachability::Unreachable);
    assert!(report.reasons.contains(&Reason::ProbeRefused));
}

#[test]
fn no_public_address_means_no_connectivity() {
    let report = classify_family(&ipv4(
        &["192.168.1.20"],
        None,
        ProbeEvidence::NoConnectivity,
    ));
    assert_eq!(report.nat, SelfHostNatKind::NoConnectivity);
    assert_eq!(report.reachability, SelfHostReachability::NoConnectivity);
    assert_eq!(report.reasons, [Reason::NoPublicAddress]);
}

#[test]
fn an_active_tunnel_makes_every_answer_unknown() {
    let report = classify_family(&ipv4(
        &["192.168.1.20"],
        Some("198.51.100.4"),
        ProbeEvidence::TunnelActive,
    ));
    assert_eq!(report.nat, SelfHostNatKind::Unknown);
    assert_eq!(report.reachability, SelfHostReachability::Unknown);
    assert_eq!(report.public_address, None);
    assert_eq!(report.reasons, [Reason::TunActive]);
}

#[test]
fn ipv6_on_the_interface_may_still_be_blocked_by_the_router() {
    let evidence = FamilyEvidence {
        family: SelfHostAddressFamily::Ipv6,
        local_addresses: vec![ip("2001:db8::7"), ip("fe80::1")],
        public_address: Some(ip("2001:db8::7")),
        probe: ProbeEvidence::Results(vec![result(42_443, PortProbeOutcome::Timeout)]),
        port_mapping: None,
        firewall_rule_missing: false,
    };
    let report = classify_family(&evidence);
    assert_eq!(report.nat, SelfHostNatKind::None);
    assert_eq!(report.reachability, SelfHostReachability::Unreachable);
    assert!(report.reasons.contains(&Reason::Ipv6FirewallUnknown));

    let unprobed = classify_family(&FamilyEvidence {
        probe: ProbeEvidence::Unavailable,
        ..evidence
    });
    assert_eq!(unprobed.reachability, SelfHostReachability::LikelyReachable);
    assert!(unprobed.reasons.contains(&Reason::Ipv6FirewallUnknown));
}

#[test]
fn a_missing_firewall_rule_is_named_only_while_unreachable() {
    let mut evidence = ipv4(
        &["203.0.113.7"],
        Some("203.0.113.7"),
        ProbeEvidence::Results(vec![result(42_443, PortProbeOutcome::Timeout)]),
    );
    evidence.firewall_rule_missing = true;
    assert!(classify_family(&evidence)
        .reasons
        .contains(&Reason::FirewallRuleMissing));
    evidence.probe = ProbeEvidence::Results(vec![result(42_443, PortProbeOutcome::Connected)]);
    assert!(!classify_family(&evidence)
        .reasons
        .contains(&Reason::FirewallRuleMissing));
}

#[test]
fn address_scopes_follow_the_special_purpose_registries() {
    for (address, scope) in [
        ("10.1.2.3", SelfHostAddressScope::Private),
        ("172.16.0.1", SelfHostAddressScope::Private),
        ("192.168.0.1", SelfHostAddressScope::Private),
        ("100.64.0.1", SelfHostAddressScope::Shared),
        ("100.127.255.254", SelfHostAddressScope::Shared),
        ("100.128.0.1", SelfHostAddressScope::Public),
        ("169.254.1.1", SelfHostAddressScope::LinkLocal),
        ("8.8.8.8", SelfHostAddressScope::Public),
        ("fe80::1", SelfHostAddressScope::LinkLocal),
        ("fd12::1", SelfHostAddressScope::Private),
        ("2001:db8::1", SelfHostAddressScope::Public),
    ] {
        assert_eq!(address_scope(ip(address)), scope, "{address}");
    }
}
