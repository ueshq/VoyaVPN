//! Asks the home router to let the self-hosted node's ports through.
//!
//! IPv4 needs a forward through the router's NAT. Three protocols ask for
//! one, and routers differ in which they speak, so they are tried in turn:
//! PCP, its predecessor NAT-PMP (both one UDP datagram to the default
//! gateway), then UPnP IGD (multicast discovery and SOAP). IPv6 needs no
//! translation, but a router's firewall drops unsolicited inbound connections;
//! PCP over IPv6 asks it for a pinhole to this device's address.
//!
//! Only the router in front of this device can be asked: a carrier-grade NAT
//! further out answers none of them, which is what the environment check
//! reports when the router's own WAN address is not the public one.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6, UdpSocket},
    sync::{Arc, Mutex, PoisonError},
};

use futures_util::future::BoxFuture;
use thiserror::Error;

mod pcp;
mod upnp;

pub const PORT_MAPPING_DESCRIPTION: &str = "VoyaVPN self-hosted node";
/// Never dialled: connecting a UDP socket toward it only asks the OS which
/// source address it would use for the internet.
const PUBLIC_IPV6_DESTINATION: Ipv6Addr = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);

/// An IPv6 router. A link-local address needs the interface it is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv6Router {
    pub address: Ipv6Addr,
    pub scope_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortMappingRequest {
    pub ports: Vec<u16>,
    pub lease_seconds: u32,
    /// The default routes' gateways. NAT-PMP and PCP have no discovery step;
    /// without a gateway only UPnP is tried.
    pub ipv4_gateway: Option<Ipv4Addr>,
    pub ipv6_gateway: Option<Ipv6Router>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortMappingMethod {
    Pcp,
    NatPmp,
    Upnp,
}

/// The forwards a router granted through its IPv4 NAT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4PortMapping {
    pub method: PortMappingMethod,
    /// The router's own WAN address, if it reported one.
    pub external_address: Option<IpAddr>,
    /// Ports whose TCP forward was accepted. UDP is added best-effort.
    pub mapped: Vec<u16>,
    /// Ports the router refused, with its reason.
    pub refused: Vec<(u16, String)>,
}

#[derive(Debug)]
pub struct PortMappingResult {
    pub ipv4: Result<Ipv4PortMapping, PortMappingError>,
    /// Ports the router's IPv6 firewall now lets through to this device.
    /// Empty when the router speaks no PCP over IPv6, which also covers the
    /// routers that filter nothing.
    pub ipv6_opened: Vec<u16>,
}

#[derive(Debug, Error)]
pub enum PortMappingError {
    #[error("no router answered PCP, NAT-PMP or UPnP: {0}")]
    NoGateway(String),
    #[error("could not find this device's address toward the gateway: {0}")]
    LocalAddress(io::Error),
    #[error("the gateway is not an IPv4 device: {0}")]
    UnsupportedGateway(IpAddr),
}

/// Router port forwarding, behind a trait so the orchestration can be tested
/// without a router.
pub trait PortMapper: Send + Sync {
    /// Adds or renews the forwards for `request.ports`.
    fn map(&self, request: PortMappingRequest) -> BoxFuture<'static, PortMappingResult>;
    /// Gives back what `map` obtained for `ports`. Best effort: a forward the
    /// router keeps runs out with its lease.
    fn unmap(&self, ports: Vec<u16>) -> BoxFuture<'static, ()>;
}

/// One forward a router granted, with what giving it back takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lease {
    Pcp {
        server: SocketAddr,
        /// This device's address the forward leads to; an IPv6 one marks a
        /// firewall pinhole.
        client: IpAddr,
        port: u16,
        nonce: pcp::Nonce,
    },
    NatPmp {
        server: SocketAddr,
        port: u16,
    },
    Upnp {
        port: u16,
    },
}

impl Lease {
    fn port(&self) -> u16 {
        match *self {
            Self::Pcp { port, .. } | Self::NatPmp { port, .. } | Self::Upnp { port } => port,
        }
    }

    fn is_ipv6(&self) -> bool {
        matches!(self, Self::Pcp { client, .. } if client.is_ipv6())
    }
}

type Leases = Arc<Mutex<Vec<Lease>>>;

/// The production mapper. It remembers what each router granted, so a renewal
/// presents the nonce PCP expects and `unmap` asks the protocol that mapped.
#[derive(Debug, Clone, Default)]
pub struct RouterPortMapper {
    leases: Leases,
}

impl PortMapper for RouterPortMapper {
    fn map(&self, request: PortMappingRequest) -> BoxFuture<'static, PortMappingResult> {
        let leases = Arc::clone(&self.leases);
        Box::pin(async move {
            let (ipv4, ipv6_opened) = tokio::join!(
                map_ipv4(&leases, &request),
                open_ipv6(&leases, &request, pcp::SERVER_PORT),
            );
            PortMappingResult { ipv4, ipv6_opened }
        })
    }

    fn unmap(&self, ports: Vec<u16>) -> BoxFuture<'static, ()> {
        let leases = Arc::clone(&self.leases);
        Box::pin(async move {
            let released = {
                let mut leases = leases.lock().unwrap_or_else(PoisonError::into_inner);
                let (released, kept) = leases
                    .drain(..)
                    .partition::<Vec<_>, _>(|lease| ports.contains(&lease.port()));
                *leases = kept;
                released
            };
            let mut upnp_ports = Vec::new();
            for lease in released {
                match lease {
                    Lease::Upnp { port } => upnp_ports.push(port),
                    direct => pcp::release(&direct).await,
                }
            }
            if !upnp_ports.is_empty() {
                upnp::unmap(&upnp_ports).await;
            }
        })
    }
}

async fn map_ipv4(
    leases: &Leases,
    request: &PortMappingRequest,
) -> Result<Ipv4PortMapping, PortMappingError> {
    if let Some(gateway) = request.ipv4_gateway {
        let server = SocketAddr::from((gateway, pcp::SERVER_PORT));
        if let Some(mapping) = map_direct(leases, server, request).await {
            return Ok(mapping);
        }
    }
    let (mapping, granted) = upnp::map(&request.ports, request.lease_seconds).await?;
    store(leases, false, &request.ports, granted);
    Ok(mapping)
}

/// PCP, then NAT-PMP, at `server`. `None` when it speaks neither.
async fn map_direct(
    leases: &Leases,
    server: SocketAddr,
    request: &PortMappingRequest,
) -> Option<Ipv4PortMapping> {
    let local = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0));
    let outcome = pcp::pcp_map(
        server,
        local,
        &request.ports,
        request.lease_seconds,
        |client, port| nonce_for(leases, server, client, port),
    )
    .await;
    let (mapping, granted) = match outcome {
        pcp::PcpOutcome::Answered { mapping, leases } => (mapping, leases),
        // Some NAT-PMP routers drop a version they do not know instead of
        // rejecting it, so silence is no proof either.
        pcp::PcpOutcome::UnsupportedVersion | pcp::PcpOutcome::NoAnswer => {
            pcp::nat_pmp_map(server, &request.ports, request.lease_seconds).await?
        }
    };
    store(leases, false, &request.ports, granted);
    Some(mapping)
}

/// Asks the IPv6 router's firewall for pinholes to this device. `server_port`
/// is a parameter for the tests' sake.
async fn open_ipv6(leases: &Leases, request: &PortMappingRequest, server_port: u16) -> Vec<u16> {
    let Some(router) = request.ipv6_gateway else {
        return Vec::new();
    };
    let Some(client) = global_ipv6_source() else {
        return Vec::new();
    };
    let scope_id = if router.address.is_unicast_link_local() {
        router.scope_id
    } else {
        0
    };
    let server = SocketAddr::V6(SocketAddrV6::new(router.address, server_port, 0, scope_id));
    let local = SocketAddr::V6(SocketAddrV6::new(client, 0, 0, 0));
    let outcome = pcp::pcp_map(
        server,
        local,
        &request.ports,
        request.lease_seconds,
        |client, port| nonce_for(leases, server, client, port),
    )
    .await;
    match outcome {
        pcp::PcpOutcome::Answered {
            mapping,
            leases: granted,
        } => {
            store(leases, true, &request.ports, granted);
            mapping.mapped
        }
        pcp::PcpOutcome::UnsupportedVersion | pcp::PcpOutcome::NoAnswer => Vec::new(),
    }
}

/// The global IPv6 address this device reaches the internet from — the one a
/// peer's connection arrives at — or `None` without IPv6 connectivity.
fn global_ipv6_source() -> Option<Ipv6Addr> {
    let socket = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((PUBLIC_IPV6_DESTINATION, 53)).ok()?;
    match socket.local_addr().ok()?.ip() {
        // 2000::/3, the global unicast space.
        IpAddr::V6(address) if address.segments()[0] & 0xe000 == 0x2000 => Some(address),
        _ => None,
    }
}

/// The nonce of the lease already held for this forward, or a fresh one.
fn nonce_for(leases: &Leases, server: SocketAddr, client: IpAddr, port: u16) -> pcp::Nonce {
    let held = leases
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find_map(|lease| match *lease {
            Lease::Pcp {
                server: held_server,
                client: held_client,
                port: held_port,
                nonce,
            } if (held_server, held_client, held_port) == (server, client, port) => Some(nonce),
            _ => None,
        });
    held.unwrap_or_else(|| {
        let mut nonce = pcp::Nonce::default();
        if let Err(error) = getrandom::fill(&mut nonce) {
            tracing::warn!(%error, "no random bytes for a PCP nonce");
        }
        nonce
    })
}

/// Replaces what was held for `ports` in one address family with `granted`.
fn store(leases: &Leases, ipv6: bool, ports: &[u16], granted: Vec<Lease>) {
    let mut leases = leases.lock().unwrap_or_else(PoisonError::into_inner);
    leases.retain(|lease| lease.is_ipv6() != ipv6 || !ports.contains(&lease.port()));
    leases.extend(granted);
}

#[cfg(test)]
mod tests;
