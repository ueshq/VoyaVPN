//! NAT-PMP (RFC 6886) and its successor PCP (RFC 6887): one UDP request per
//! forward, sent straight to the default router on port 5351.
//!
//! PCP also opens the router's IPv6 firewall: the same MAP request, sent over
//! IPv6, asks for a pinhole to this device's own address instead of a
//! translation. NAT-PMP is IPv4 only.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use tokio::{net::UdpSocket, time::Instant};

use super::{Ipv4PortMapping, Lease, PortMappingMethod};

/// Both protocols listen here.
pub(super) const SERVER_PORT: u16 = 5351;
/// Each request is sent again after these waits (RFC 6887 section 8.1.1
/// starts at 250 ms and doubles). A router that speaks neither protocol
/// usually answers with an ICMP error long before the first one runs out.
const RETRY_WAITS: [Duration; 3] = [
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_millis(1000),
];

const PCP_VERSION: u8 = 2;
const NAT_PMP_VERSION: u8 = 0;
const PCP_OPCODE_MAP: u8 = 1;
const RESPONSE_BIT: u8 = 0x80;
const PCP_MAP_LENGTH: usize = 60;
pub(super) const NONCE_LENGTH: usize = 12;
const RESULT_SUCCESS: u16 = 0;
const RESULT_UNSUPPORTED_VERSION: u16 = 1;

const IP_PROTOCOL_TCP: u8 = 6;
const IP_PROTOCOL_UDP: u8 = 17;
const NAT_PMP_OPCODE_EXTERNAL_ADDRESS: u8 = 0;
const NAT_PMP_OPCODE_UDP: u8 = 1;
const NAT_PMP_OPCODE_TCP: u8 = 2;

pub(super) type Nonce = [u8; NONCE_LENGTH];

/// What one PCP MAP request came back with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PcpReply {
    Mapped {
        external_address: IpAddr,
        external_port: u16,
    },
    /// The server speaks PCP and said no.
    Refused(u8),
    /// The server only speaks NAT-PMP, or another PCP version.
    UnsupportedVersion,
}

/// What a PCP server made of a set of ports.
#[derive(Debug)]
pub(super) enum PcpOutcome {
    /// Nothing answered on the PCP port.
    NoAnswer,
    /// Something answered, in NAT-PMP.
    UnsupportedVersion,
    Answered {
        mapping: Ipv4PortMapping,
        leases: Vec<Lease>,
    },
}

fn pcp_map_request(
    client: IpAddr,
    nonce: &Nonce,
    protocol: u8,
    port: u16,
    lifetime_seconds: u32,
) -> [u8; PCP_MAP_LENGTH] {
    let mut packet = [0_u8; PCP_MAP_LENGTH];
    packet[0] = PCP_VERSION;
    packet[1] = PCP_OPCODE_MAP;
    packet[4..8].copy_from_slice(&lifetime_seconds.to_be_bytes());
    packet[8..24].copy_from_slice(&ipv6_bytes(client));
    packet[24..36].copy_from_slice(nonce);
    packet[36] = protocol;
    packet[40..42].copy_from_slice(&port.to_be_bytes());
    // The node's links name its own port, so only that external port is of
    // any use. A deletion (lifetime 0) names no external port.
    let external_port = if lifetime_seconds == 0 { 0 } else { port };
    packet[42..44].copy_from_slice(&external_port.to_be_bytes());
    // No preferred external address: all zeros in the client's family.
    let unspecified = match client {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    packet[44..60].copy_from_slice(&ipv6_bytes(unspecified));
    packet
}

/// IPv4 addresses travel IPv4-mapped (RFC 6887 section 5).
fn ipv6_bytes(address: IpAddr) -> [u8; 16] {
    match address {
        IpAddr::V4(address) => address.to_ipv6_mapped().octets(),
        IpAddr::V6(address) => address.octets(),
    }
}

/// The reply to the MAP request for `nonce`, `protocol` and `port`, or `None`
/// for a datagram that answers something else.
fn parse_pcp_map_response(
    bytes: &[u8],
    nonce: &Nonce,
    protocol: u8,
    port: u16,
) -> Option<PcpReply> {
    let (&version, rest) = bytes.split_first()?;
    if version == NAT_PMP_VERSION {
        // A NAT-PMP server answers a version it does not know in its own
        // format: result code 1 in bytes 2..4.
        let result = u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?);
        return (result == RESULT_UNSUPPORTED_VERSION).then_some(PcpReply::UnsupportedVersion);
    }
    if rest.first()? & RESPONSE_BIT == 0 {
        return None;
    }
    let result = *bytes.get(3)?;
    if u16::from(result) == RESULT_UNSUPPORTED_VERSION || version != PCP_VERSION {
        return Some(PcpReply::UnsupportedVersion);
    }
    if bytes[1] != PCP_OPCODE_MAP | RESPONSE_BIT || bytes.len() < PCP_MAP_LENGTH {
        // An error can come back with the request's payload cut short.
        return (u16::from(result) != RESULT_SUCCESS).then_some(PcpReply::Refused(result));
    }
    if bytes[24..36] != nonce[..]
        || bytes[36] != protocol
        || bytes[40..42] != port.to_be_bytes()[..]
    {
        return None;
    }
    if u16::from(result) != RESULT_SUCCESS {
        return Some(PcpReply::Refused(result));
    }
    let external: [u8; 16] = bytes[44..60].try_into().ok()?;
    let external = Ipv6Addr::from(external);
    Some(PcpReply::Mapped {
        external_address: external
            .to_ipv4_mapped()
            .map_or(IpAddr::V6(external), IpAddr::V4),
        external_port: u16::from_be_bytes([bytes[42], bytes[43]]),
    })
}

fn pcp_result_name(code: u8) -> &'static str {
    match code {
        2 => "not authorized",
        7 => "network failure",
        8 => "no resources",
        9 => "unsupported protocol",
        10 => "quota exceeded",
        11 => "cannot provide the external port",
        12 => "address mismatch",
        _ => "refused",
    }
}

fn nat_pmp_map_request(opcode: u8, port: u16, lifetime_seconds: u32) -> [u8; 12] {
    let mut packet = [0_u8; 12];
    packet[0] = NAT_PMP_VERSION;
    packet[1] = opcode;
    packet[4..6].copy_from_slice(&port.to_be_bytes());
    let external_port = if lifetime_seconds == 0 { 0 } else { port };
    packet[6..8].copy_from_slice(&external_port.to_be_bytes());
    packet[8..12].copy_from_slice(&lifetime_seconds.to_be_bytes());
    packet
}

/// The result code and the bytes after the eight-byte header of the NAT-PMP
/// response to `opcode`.
fn parse_nat_pmp_response(bytes: &[u8], opcode: u8) -> Option<(u16, &[u8])> {
    if *bytes.first()? != NAT_PMP_VERSION {
        // A PCP-only server answers version 0 in PCP's format.
        return (bytes.get(3).copied().map(u16::from) == Some(RESULT_UNSUPPORTED_VERSION))
            .then_some((RESULT_UNSUPPORTED_VERSION, &[][..]));
    }
    if *bytes.get(1)? != opcode | RESPONSE_BIT {
        return None;
    }
    let result = u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?);
    Some((result, bytes.get(8..).unwrap_or_default()))
}

/// Sends `request` until `parse` accepts an answer. `None` when nothing that
/// answers the request arrives, or the router says nothing listens there.
async fn exchange<T>(
    socket: &UdpSocket,
    request: &[u8],
    parse: impl Fn(&[u8]) -> Option<T>,
) -> Option<T> {
    let mut buffer = [0_u8; 1100];
    for wait in RETRY_WAITS {
        socket.send(request).await.ok()?;
        let deadline = Instant::now() + wait;
        loop {
            match tokio::time::timeout_at(deadline, socket.recv(&mut buffer)).await {
                Ok(Ok(length)) => {
                    if let Some(reply) = parse(&buffer[..length]) {
                        return Some(reply);
                    }
                }
                // An ICMP "port unreachable" surfaces here on a connected
                // socket: no server, and waiting longer changes nothing.
                Ok(Err(_)) => return None,
                Err(_) => break,
            }
        }
    }
    None
}

/// A socket bound to `local` that only talks to `server`.
async fn connected_socket(local: SocketAddr, server: SocketAddr) -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind(local).await?;
    socket.connect(server).await?;
    Ok(socket)
}

async fn pcp_request(
    socket: &UdpSocket,
    client: IpAddr,
    nonce: &Nonce,
    protocol: u8,
    port: u16,
    lifetime_seconds: u32,
) -> Option<PcpReply> {
    let request = pcp_map_request(client, nonce, protocol, port, lifetime_seconds);
    exchange(socket, &request, |bytes| {
        parse_pcp_map_response(bytes, nonce, protocol, port)
    })
    .await
}

/// Asks the PCP server at `server` to let `ports` through to this device.
/// `local` picks the source address, which PCP requires to be the address the
/// forward leads to; `nonce_for` returns the nonce an earlier request used for
/// a port, because a server only renews a mapping for the nonce that made it.
pub(super) async fn pcp_map(
    server: SocketAddr,
    local: SocketAddr,
    ports: &[u16],
    lifetime_seconds: u32,
    nonce_for: impl Fn(IpAddr, u16) -> Nonce,
) -> PcpOutcome {
    let Ok(socket) = connected_socket(local, server).await else {
        return PcpOutcome::NoAnswer;
    };
    let Ok(client) = socket.local_addr().map(|address| address.ip()) else {
        return PcpOutcome::NoAnswer;
    };
    let mut mapping = Ipv4PortMapping {
        method: PortMappingMethod::Pcp,
        external_address: None,
        mapped: Vec::new(),
        refused: Vec::new(),
    };
    let mut leases = Vec::new();
    for (index, &port) in ports.iter().enumerate() {
        let nonce = nonce_for(client, port);
        let reply = pcp_request(
            &socket,
            client,
            &nonce,
            IP_PROTOCOL_TCP,
            port,
            lifetime_seconds,
        )
        .await;
        match reply {
            // Only the first request decides whether a server is there.
            None if index == 0 => return PcpOutcome::NoAnswer,
            Some(PcpReply::UnsupportedVersion) if index == 0 => {
                return PcpOutcome::UnsupportedVersion
            }
            None | Some(PcpReply::UnsupportedVersion) => {
                mapping.refused.push((port, "no answer".to_string()));
            }
            Some(PcpReply::Refused(code)) => {
                mapping
                    .refused
                    .push((port, format!("PCP: {}", pcp_result_name(code))));
            }
            Some(PcpReply::Mapped {
                external_address,
                external_port,
            }) => {
                let lease = Lease::Pcp {
                    server,
                    client,
                    port,
                    nonce,
                };
                if external_port != port {
                    release(&lease).await;
                    mapping.refused.push((
                        port,
                        format!("PCP: the router offered port {external_port} instead"),
                    ));
                    continue;
                }
                mapping.external_address.get_or_insert(external_address);
                mapping.mapped.push(port);
                leases.push(lease);
                // Shadowsocks serves UDP on the same port; TCP decides.
                let udp = pcp_request(
                    &socket,
                    client,
                    &nonce,
                    IP_PROTOCOL_UDP,
                    port,
                    lifetime_seconds,
                )
                .await;
                if !matches!(udp, Some(PcpReply::Mapped { external_port, .. }) if external_port == port)
                {
                    tracing::debug!(port, ?udp, "router refused the UDP forward");
                }
            }
        }
    }
    PcpOutcome::Answered { mapping, leases }
}

/// Asks the NAT-PMP server at `server` to forward `ports`. `None` when
/// nothing there speaks NAT-PMP.
pub(super) async fn nat_pmp_map(
    server: SocketAddr,
    ports: &[u16],
    lifetime_seconds: u32,
) -> Option<(Ipv4PortMapping, Vec<Lease>)> {
    let local = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0));
    let socket = connected_socket(local, server).await.ok()?;
    let request = [NAT_PMP_VERSION, NAT_PMP_OPCODE_EXTERNAL_ADDRESS];
    let external = exchange(&socket, &request, |bytes| {
        let (result, body) = parse_nat_pmp_response(bytes, NAT_PMP_OPCODE_EXTERNAL_ADDRESS)?;
        // Any answer proves a server; the address is only there on success.
        let address: Option<[u8; 4]> = body.get(0..4).and_then(|bytes| bytes.try_into().ok());
        Some((result, address.map(Ipv4Addr::from)))
    })
    .await?;
    if external.0 == RESULT_UNSUPPORTED_VERSION {
        return None;
    }
    let mut mapping = Ipv4PortMapping {
        method: PortMappingMethod::NatPmp,
        external_address: external
            .1
            .filter(|_| external.0 == RESULT_SUCCESS)
            .map(IpAddr::V4),
        mapped: Vec::new(),
        refused: Vec::new(),
    };
    let mut leases = Vec::new();
    for &port in ports {
        match nat_pmp_request(&socket, NAT_PMP_OPCODE_TCP, port, lifetime_seconds).await {
            Some(Ok(external_port)) if external_port == port => {
                mapping.mapped.push(port);
                leases.push(Lease::NatPmp { server, port });
                let udp =
                    nat_pmp_request(&socket, NAT_PMP_OPCODE_UDP, port, lifetime_seconds).await;
                if udp != Some(Ok(port)) {
                    tracing::debug!(port, ?udp, "router refused the UDP forward");
                }
            }
            Some(Ok(external_port)) => {
                let _ = nat_pmp_request(&socket, NAT_PMP_OPCODE_TCP, port, 0).await;
                mapping.refused.push((
                    port,
                    format!("NAT-PMP: the router offered port {external_port} instead"),
                ));
            }
            Some(Err(code)) => mapping
                .refused
                .push((port, format!("NAT-PMP: result code {code}"))),
            None => mapping.refused.push((port, "no answer".to_string())),
        }
    }
    Some((mapping, leases))
}

/// The external port the server assigned, or its result code.
async fn nat_pmp_request(
    socket: &UdpSocket,
    opcode: u8,
    port: u16,
    lifetime_seconds: u32,
) -> Option<Result<u16, u16>> {
    let request = nat_pmp_map_request(opcode, port, lifetime_seconds);
    exchange(socket, &request, |bytes| {
        let (result, body) = parse_nat_pmp_response(bytes, opcode)?;
        if result != RESULT_SUCCESS {
            return Some(Err(result));
        }
        if body.get(0..2)? != port.to_be_bytes() {
            return None;
        }
        Some(Ok(u16::from_be_bytes(body.get(2..4)?.try_into().ok()?)))
    })
    .await
}

/// Gives a PCP or NAT-PMP forward back: the same request with a lifetime of
/// zero, for TCP and UDP.
pub(super) async fn release(lease: &Lease) {
    match *lease {
        Lease::Pcp {
            server,
            client,
            port,
            nonce,
        } => {
            let Ok(socket) = connected_socket(SocketAddr::new(client, 0), server).await else {
                return;
            };
            for protocol in [IP_PROTOCOL_TCP, IP_PROTOCOL_UDP] {
                let _ = pcp_request(&socket, client, &nonce, protocol, port, 0).await;
            }
        }
        Lease::NatPmp { server, port } => {
            let local = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0));
            let Ok(socket) = connected_socket(local, server).await else {
                return;
            };
            for opcode in [NAT_PMP_OPCODE_TCP, NAT_PMP_OPCODE_UDP] {
                let _ = nat_pmp_request(&socket, opcode, port, 0).await;
            }
        }
        Lease::Upnp { .. } => {}
    }
}

#[cfg(test)]
mod tests;
