//! The network check: where this device sits between its node and the
//! internet, and whether a peer out there can open a connection to it.
//!
//! Gathering the evidence is effectful (interfaces, the router, the probe
//! service, the firewall); turning evidence into a verdict is the pure
//! [`classify_family`], which is what the table tests pin.

use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{SystemTime, UNIX_EPOCH},
};

use voya_contracts::{
    SelfHostAddressFamily, SelfHostAddressScope, SelfHostEnvironmentReport, SelfHostFamilyReport,
    SelfHostFirewallStatus, SelfHostLocalAddress, SelfHostNatKind, SelfHostPortMappingReport,
    SelfHostPortMappingStatus, SelfHostReachability, SelfHostReasonCode, SelfHostRecordV1,
    SelfHostSelfTest,
};
use voya_net::{
    portmap::PortMappingError,
    probe::{PortProbeOutcome, PortProbeResult, ProbeFamily, ReachabilityProbeError},
};
use voya_platform::firewall::FirewallRuleStatus;

use super::{process::core_executable, selftest::SKIPPED, SelfHostDeps};

/// Routers drop a forward when its lease runs out; the watch loop renews it
/// well before that.
pub(super) const PORT_MAPPING_LEASE_SECONDS: u32 = 3600;

/// What the probe service could tell about one family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeEvidence {
    /// The node was not running, so there was nothing to connect to.
    NodeStopped,
    /// This device's own tunnel would have carried the probe.
    TunnelActive,
    /// The probe service did not answer.
    Unavailable,
    /// This family has no route out at all.
    NoConnectivity,
    Results(Vec<PortProbeResult>),
}

/// The router's answer, for IPv4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappingEvidence {
    pub status: SelfHostPortMappingStatus,
    pub gateway_external_address: Option<IpAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyEvidence {
    pub family: SelfHostAddressFamily,
    /// This family's addresses on the machine's interfaces.
    pub local_addresses: Vec<IpAddr>,
    pub public_address: Option<IpAddr>,
    pub probe: ProbeEvidence,
    pub port_mapping: Option<MappingEvidence>,
    pub firewall_rule_missing: bool,
}

/// Turns one family's evidence into the verdict the page explains.
#[must_use]
pub fn classify_family(evidence: &FamilyEvidence) -> SelfHostFamilyReport {
    let mut reasons = BTreeSet::new();
    let public_address = evidence.public_address.map(|address| address.to_string());

    if evidence.probe == ProbeEvidence::TunnelActive {
        // The tunnel answers for this device: neither the public address nor
        // the probe would describe the node's own path.
        reasons.insert(SelfHostReasonCode::TunActive);
        return report(
            evidence,
            None,
            SelfHostNatKind::Unknown,
            SelfHostReachability::Unknown,
            false,
            reasons,
        );
    }

    let nat = nat_kind(evidence, &mut reasons);
    push_mapping_reasons(evidence, nat, &mut reasons);
    let (reachability, verified) = match &evidence.probe {
        ProbeEvidence::Results(results) if !results.is_empty() => {
            (probed(results, &mut reasons), true)
        }
        probe => {
            match probe {
                ProbeEvidence::NodeStopped => {
                    reasons.insert(SelfHostReasonCode::NodeNotRunning);
                }
                ProbeEvidence::Unavailable => {
                    reasons.insert(SelfHostReasonCode::ProbeUnavailable);
                }
                _ => {}
            }
            (inferred(evidence, nat), false)
        }
    };
    let reachability = if nat == SelfHostNatKind::NoConnectivity {
        SelfHostReachability::NoConnectivity
    } else {
        reachability
    };
    if evidence.family == SelfHostAddressFamily::Ipv6
        && nat == SelfHostNatKind::None
        && reachability != SelfHostReachability::Reachable
    {
        reasons.insert(SelfHostReasonCode::Ipv6FirewallUnknown);
    }
    if evidence.firewall_rule_missing
        && !matches!(
            reachability,
            SelfHostReachability::Reachable | SelfHostReachability::NoConnectivity
        )
    {
        reasons.insert(SelfHostReasonCode::FirewallRuleMissing);
    }
    report(
        evidence,
        public_address,
        nat,
        reachability,
        verified,
        reasons,
    )
}

fn report(
    evidence: &FamilyEvidence,
    public_address: Option<String>,
    nat: SelfHostNatKind,
    reachability: SelfHostReachability,
    verified_by_probe: bool,
    reasons: BTreeSet<SelfHostReasonCode>,
) -> SelfHostFamilyReport {
    SelfHostFamilyReport {
        family: evidence.family,
        public_address,
        nat,
        reachability,
        verified_by_probe,
        reasons: reasons.into_iter().collect(),
    }
}

fn nat_kind(
    evidence: &FamilyEvidence,
    reasons: &mut BTreeSet<SelfHostReasonCode>,
) -> SelfHostNatKind {
    let Some(public) = evidence.public_address else {
        reasons.insert(SelfHostReasonCode::NoPublicAddress);
        return SelfHostNatKind::NoConnectivity;
    };
    if evidence.local_addresses.contains(&public) {
        reasons.insert(SelfHostReasonCode::PublicAddressOnDevice);
        return SelfHostNatKind::None;
    }
    if evidence.family == SelfHostAddressFamily::Ipv6 {
        // A public IPv6 address that is on no interface means prefix
        // translation somewhere; nothing more can be said.
        return SelfHostNatKind::Unknown;
    }
    let shared_local = evidence
        .local_addresses
        .iter()
        .any(|address| address_scope(*address) == SelfHostAddressScope::Shared);
    let gateway = evidence
        .port_mapping
        .and_then(|mapping| mapping.gateway_external_address);
    let kind = match gateway {
        Some(external) if address_scope(external) == SelfHostAddressScope::Shared => {
            SelfHostNatKind::CarrierGrade
        }
        Some(external) if address_scope(external) == SelfHostAddressScope::Private => {
            SelfHostNatKind::Double
        }
        Some(external) if external == public => SelfHostNatKind::Nat,
        Some(_) => SelfHostNatKind::Unknown,
        None if shared_local => SelfHostNatKind::CarrierGrade,
        None => SelfHostNatKind::Nat,
    };
    match kind {
        SelfHostNatKind::CarrierGrade => {
            reasons.insert(SelfHostReasonCode::CarrierGradeNat);
        }
        SelfHostNatKind::Double => {
            reasons.insert(SelfHostReasonCode::DoubleNat);
        }
        SelfHostNatKind::Nat => {
            reasons.insert(SelfHostReasonCode::BehindNat);
        }
        _ => {}
    }
    kind
}

fn push_mapping_reasons(
    evidence: &FamilyEvidence,
    nat: SelfHostNatKind,
    reasons: &mut BTreeSet<SelfHostReasonCode>,
) {
    let Some(mapping) = evidence.port_mapping else {
        return;
    };
    match mapping.status {
        SelfHostPortMappingStatus::Mapped => {
            reasons.insert(SelfHostReasonCode::PortMapped);
        }
        SelfHostPortMappingStatus::NoGateway
            if matches!(nat, SelfHostNatKind::Nat | SelfHostNatKind::Double) =>
        {
            reasons.insert(SelfHostReasonCode::UpnpUnavailable);
        }
        SelfHostPortMappingStatus::Failed => {
            reasons.insert(SelfHostReasonCode::UpnpFailed);
        }
        _ => {}
    }
}

fn probed(
    results: &[PortProbeResult],
    reasons: &mut BTreeSet<SelfHostReasonCode>,
) -> SelfHostReachability {
    if results.iter().all(|result| result.reachable) {
        reasons.insert(SelfHostReasonCode::ProbeReachable);
        return SelfHostReachability::Reachable;
    }
    for result in results.iter().filter(|result| !result.reachable) {
        reasons.insert(if result.reason == PortProbeOutcome::Refused {
            SelfHostReasonCode::ProbeRefused
        } else {
            SelfHostReasonCode::ProbeTimedOut
        });
    }
    SelfHostReachability::Unreachable
}

fn inferred(evidence: &FamilyEvidence, nat: SelfHostNatKind) -> SelfHostReachability {
    let mapped = evidence
        .port_mapping
        .is_some_and(|mapping| mapping.status == SelfHostPortMappingStatus::Mapped);
    match nat {
        SelfHostNatKind::NoConnectivity => SelfHostReachability::NoConnectivity,
        SelfHostNatKind::None => SelfHostReachability::LikelyReachable,
        SelfHostNatKind::Nat if mapped => SelfHostReachability::LikelyReachable,
        SelfHostNatKind::Nat | SelfHostNatKind::Double => SelfHostReachability::NeedsPortForward,
        SelfHostNatKind::CarrierGrade => SelfHostReachability::Unreachable,
        SelfHostNatKind::Unknown => SelfHostReachability::Unknown,
    }
}

/// How an address relates to the internet.
#[must_use]
pub(super) fn address_scope(address: IpAddr) -> SelfHostAddressScope {
    match address {
        IpAddr::V4(address) => {
            if address.is_link_local() {
                SelfHostAddressScope::LinkLocal
            } else if address.is_private() {
                SelfHostAddressScope::Private
            } else if is_shared_v4(address) {
                SelfHostAddressScope::Shared
            } else {
                SelfHostAddressScope::Public
            }
        }
        IpAddr::V6(address) => {
            if is_unicast_link_local_v6(address) {
                SelfHostAddressScope::LinkLocal
            } else if is_unique_local_v6(address) {
                SelfHostAddressScope::Private
            } else {
                SelfHostAddressScope::Public
            }
        }
    }
}

fn is_shared_v4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    first == 100 && (64..=127).contains(&second)
}

fn is_unicast_link_local_v6(address: Ipv6Addr) -> bool {
    address.segments()[0] & 0xffc0 == 0xfe80
}

fn is_unique_local_v6(address: Ipv6Addr) -> bool {
    address.segments()[0] & 0xfe00 == 0xfc00
}

/// What the check needs to know about the node.
#[derive(Debug, Clone, Default)]
pub(super) struct CheckInput {
    pub(super) ports: Vec<u16>,
    pub(super) node_running: bool,
    pub(super) upnp_enabled: bool,
    pub(super) previously_mapped: Vec<u16>,
    /// The running node's record, for the self-test; `None` skips it.
    pub(super) self_test: Option<SelfHostRecordV1>,
}

pub(super) struct CheckOutcome {
    pub(super) report: SelfHostEnvironmentReport,
    pub(super) mapped_ports: Vec<u16>,
    /// Ports the router accepted this time that it did not hold before.
    pub(super) newly_mapped: Vec<u16>,
    pub(super) mapping_failed: bool,
}

pub(super) async fn check_environment(deps: &SelfHostDeps, input: CheckInput) -> CheckOutcome {
    let network = std::sync::Arc::clone(&deps.network);
    let interfaces = tokio::task::spawn_blocking(move || network.interface_addresses())
        .await
        .unwrap_or_default();
    let tunnel_active = deps.host_tunnel.tunnel_active().await;

    // The forward comes first so the probe below tests the forwarded path.
    let mapping = port_mapping(deps, &input, tunnel_active).await;
    let (ipv4_probe, ipv6_probe, self_test) = tokio::join!(
        family_probe(deps, ProbeFamily::Ipv4, &input, tunnel_active),
        family_probe(deps, ProbeFamily::Ipv6, &input, tunnel_active),
        self_test(deps, input.self_test.as_ref()),
    );
    let firewall = firewall_status(deps).await;
    let firewall_rule_missing = matches!(
        firewall,
        SelfHostFirewallStatus::RuleMissing | SelfHostFirewallStatus::Unknown
    ) && deps.firewall.manages_firewall();

    let locals_of = |ipv6: bool| {
        interfaces
            .iter()
            .map(|entry| entry.address)
            .filter(|address| address.is_ipv6() == ipv6)
            .collect::<Vec<_>>()
    };
    let ipv4 = classify_family(&FamilyEvidence {
        family: SelfHostAddressFamily::Ipv4,
        local_addresses: locals_of(false),
        public_address: ipv4_probe.1,
        probe: ipv4_probe.0.clone(),
        port_mapping: Some(MappingEvidence {
            status: mapping.report.status,
            gateway_external_address: mapping.gateway_external_address,
        }),
        firewall_rule_missing,
    });
    let ipv6 = classify_family(&FamilyEvidence {
        family: SelfHostAddressFamily::Ipv6,
        local_addresses: locals_of(true),
        public_address: ipv6_probe.1,
        probe: ipv6_probe.0.clone(),
        port_mapping: None,
        firewall_rule_missing,
    });
    let probe_available = ![&ipv4_probe.0, &ipv6_probe.0]
        .into_iter()
        .any(|evidence| *evidence == ProbeEvidence::Unavailable);

    let newly_mapped = mapping
        .mapped_ports
        .iter()
        .copied()
        .filter(|port| !input.previously_mapped.contains(port))
        .collect();
    CheckOutcome {
        report: SelfHostEnvironmentReport {
            checked_at_ms: unix_now_ms(),
            local_addresses: interfaces
                .iter()
                .map(|entry| SelfHostLocalAddress {
                    interface: entry.interface.clone(),
                    address: entry.address.to_string(),
                    family: if entry.address.is_ipv6() {
                        SelfHostAddressFamily::Ipv6
                    } else {
                        SelfHostAddressFamily::Ipv4
                    },
                    scope: address_scope(entry.address),
                })
                .collect(),
            ipv4,
            ipv6,
            port_mapping: mapping.report.clone(),
            firewall,
            probe_available,
            self_test,
        },
        mapping_failed: mapping.report.status == SelfHostPortMappingStatus::Failed,
        mapped_ports: mapping.mapped_ports,
        newly_mapped,
    }
}

struct MappingOutcome {
    report: SelfHostPortMappingReport,
    gateway_external_address: Option<IpAddr>,
    mapped_ports: Vec<u16>,
}

async fn port_mapping(
    deps: &SelfHostDeps,
    input: &CheckInput,
    tunnel_active: bool,
) -> MappingOutcome {
    let wanted = input.upnp_enabled && input.node_running && !input.ports.is_empty();
    let stale = input
        .previously_mapped
        .iter()
        .copied()
        .filter(|port| !wanted || !input.ports.contains(port))
        .collect::<Vec<_>>();
    if !stale.is_empty() {
        if let Err(error) = deps.port_mapper.unmap(stale).await {
            tracing::debug!(%error, "could not remove stale router forwards");
        }
    }
    let disabled = |status| MappingOutcome {
        report: SelfHostPortMappingReport {
            status,
            gateway_external_address: None,
            mapped_ports: Vec::new(),
            detail: None,
        },
        gateway_external_address: None,
        mapped_ports: Vec::new(),
    };
    if !wanted || tunnel_active {
        return disabled(SelfHostPortMappingStatus::Disabled);
    }
    match deps
        .port_mapper
        .map(input.ports.clone(), PORT_MAPPING_LEASE_SECONDS)
        .await
    {
        Ok(result) => {
            let status = if result.mapped.is_empty() {
                SelfHostPortMappingStatus::Failed
            } else {
                SelfHostPortMappingStatus::Mapped
            };
            let detail = (!result.refused.is_empty()).then(|| {
                result
                    .refused
                    .iter()
                    .map(|(port, reason)| format!("{port}: {reason}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            });
            MappingOutcome {
                report: SelfHostPortMappingReport {
                    status,
                    gateway_external_address: result
                        .external_address
                        .map(|address| address.to_string()),
                    mapped_ports: result.mapped.clone(),
                    detail,
                },
                gateway_external_address: result.external_address,
                mapped_ports: result.mapped,
            }
        }
        Err(PortMappingError::NoGateway(_)) => disabled(SelfHostPortMappingStatus::NoGateway),
        Err(error) => MappingOutcome {
            report: SelfHostPortMappingReport {
                status: SelfHostPortMappingStatus::Failed,
                gateway_external_address: None,
                mapped_ports: Vec::new(),
                detail: Some(error.to_string()),
            },
            gateway_external_address: None,
            mapped_ports: Vec::new(),
        },
    }
}

async fn family_probe(
    deps: &SelfHostDeps,
    family: ProbeFamily,
    input: &CheckInput,
    tunnel_active: bool,
) -> (ProbeEvidence, Option<IpAddr>) {
    if tunnel_active {
        return (ProbeEvidence::TunnelActive, None);
    }
    if input.node_running && !input.ports.is_empty() {
        match deps.probe.probe(family, input.ports.clone()).await {
            Ok(response) => {
                return (
                    ProbeEvidence::Results(response.results),
                    response.ip.parse().ok(),
                );
            }
            Err(error) => {
                tracing::debug!(%error, ?family, "probe service unavailable");
            }
        }
        return public_address_only(deps, family, ProbeEvidence::Unavailable).await;
    }
    public_address_only(deps, family, ProbeEvidence::NodeStopped).await
}

async fn public_address_only(
    deps: &SelfHostDeps,
    family: ProbeFamily,
    evidence: ProbeEvidence,
) -> (ProbeEvidence, Option<IpAddr>) {
    match deps.probe.public_address(family).await {
        Ok(address) => (evidence, Some(address)),
        Err(ReachabilityProbeError::Unreachable { .. }) => (ProbeEvidence::NoConnectivity, None),
        Err(error) => {
            tracing::debug!(%error, ?family, "public address lookup failed");
            (evidence, None)
        }
    }
}

async fn self_test(deps: &SelfHostDeps, record: Option<&SelfHostRecordV1>) -> SelfHostSelfTest {
    match record {
        Some(record) => deps.self_tester.run(deps, record).await,
        None => SKIPPED,
    }
}

async fn firewall_status(deps: &SelfHostDeps) -> SelfHostFirewallStatus {
    if !deps.firewall.manages_firewall() {
        return SelfHostFirewallStatus::NotManaged;
    }
    let deps = deps.clone();
    let status = tokio::task::spawn_blocking(move || {
        let program = core_executable(&deps).ok()?;
        deps.firewall.status(&program).ok()
    })
    .await
    .ok()
    .flatten();
    match status {
        Some(FirewallRuleStatus::Present) => SelfHostFirewallStatus::RulePresent,
        Some(FirewallRuleStatus::Missing | FirewallRuleStatus::Stale) => {
            SelfHostFirewallStatus::RuleMissing
        }
        Some(FirewallRuleStatus::NotManaged) => SelfHostFirewallStatus::NotManaged,
        None => SelfHostFirewallStatus::Unknown,
    }
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests;
