//! UPnP IGD: the router is found by multicast and asked over SOAP.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::Duration,
};

use igd_next::{aio::tokio::search_gateway, PortMappingProtocol, SearchOptions};

use super::{
    Ipv4PortMapping, Lease, PortMappingError, PortMappingMethod, PORT_MAPPING_DESCRIPTION,
};

const SEARCH_TIMEOUT: Duration = Duration::from_secs(3);
const SINGLE_SEARCH_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) async fn map(
    ports: &[u16],
    lease_seconds: u32,
) -> Result<(Ipv4PortMapping, Vec<Lease>), PortMappingError> {
    let gateway = search_gateway(search_options())
        .await
        .map_err(|error| PortMappingError::NoGateway(error.to_string()))?;
    let local = local_address_toward(gateway.addr)?;
    let mut mapping = Ipv4PortMapping {
        method: PortMappingMethod::Upnp,
        external_address: gateway.get_external_ip().await.ok(),
        mapped: Vec::new(),
        refused: Vec::new(),
    };
    let mut leases = Vec::new();
    for &port in ports {
        let target = SocketAddr::new(IpAddr::V4(local), port);
        match gateway
            .add_port(
                PortMappingProtocol::TCP,
                port,
                target,
                lease_seconds,
                PORT_MAPPING_DESCRIPTION,
            )
            .await
        {
            Ok(()) => {
                mapping.mapped.push(port);
                leases.push(Lease::Upnp { port });
                // Shadowsocks serves UDP on the same port; TCP decides.
                if let Err(error) = gateway
                    .add_port(
                        PortMappingProtocol::UDP,
                        port,
                        target,
                        lease_seconds,
                        PORT_MAPPING_DESCRIPTION,
                    )
                    .await
                {
                    tracing::debug!(port, %error, "router refused the UDP forward");
                }
            }
            Err(error) => mapping.refused.push((port, error.to_string())),
        }
    }
    Ok((mapping, leases))
}

pub(super) async fn unmap(ports: &[u16]) {
    let gateway = match search_gateway(search_options()).await {
        Ok(gateway) => gateway,
        Err(error) => {
            tracing::debug!(%error, "no UPnP gateway to remove forwards from");
            return;
        }
    };
    for &port in ports {
        for protocol in [PortMappingProtocol::TCP, PortMappingProtocol::UDP] {
            if let Err(error) = gateway.remove_port(protocol, port).await {
                tracing::debug!(port, %error, "router kept a forward we asked to remove");
            }
        }
    }
}

fn search_options() -> SearchOptions {
    SearchOptions {
        timeout: Some(SEARCH_TIMEOUT),
        single_search_timeout: Some(SINGLE_SEARCH_TIMEOUT),
        ..SearchOptions::default()
    }
}

/// The local IPv4 address the OS routes toward `gateway`: the address the
/// router has to forward to. Connecting a UDP socket sends nothing.
fn local_address_toward(gateway: SocketAddr) -> Result<Ipv4Addr, PortMappingError> {
    if !gateway.is_ipv4() {
        return Err(PortMappingError::UnsupportedGateway(gateway.ip()));
    }
    let socket =
        UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(PortMappingError::LocalAddress)?;
    socket
        .connect(gateway)
        .map_err(PortMappingError::LocalAddress)?;
    match socket
        .local_addr()
        .map_err(PortMappingError::LocalAddress)?
        .ip()
    {
        IpAddr::V4(address) => Ok(address),
        other => Err(PortMappingError::UnsupportedGateway(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_address_toward_a_gateway_is_ipv4() {
        let local = local_address_toward(SocketAddr::from((Ipv4Addr::LOCALHOST, 1900)))
            .expect("loopback route");
        assert_eq!(local, Ipv4Addr::LOCALHOST);
    }

    /// Opt-in and read-only: `VOYA_LIVE_NETWORK=1` looks for the router and
    /// asks its WAN address, without adding any forward.
    #[tokio::test]
    async fn live_gateway_discovery_is_read_only() {
        if std::env::var_os("VOYA_LIVE_NETWORK").is_none() {
            println!("live UPnP discovery skipped: set VOYA_LIVE_NETWORK=1");
            return;
        }
        match search_gateway(search_options()).await {
            Ok(gateway) => {
                let local = local_address_toward(gateway.addr).expect("local address");
                println!(
                    "gateway {} answers for {local}; WAN address {:?}",
                    gateway.addr,
                    gateway.get_external_ip().await
                );
            }
            Err(error) => println!("no UPnP gateway on this network: {error}"),
        }
    }

    #[test]
    fn an_ipv6_gateway_is_rejected() {
        let error = local_address_toward("[::1]:1900".parse().expect("address"))
            .expect_err("IPv6 gateways are not mapped");
        assert!(matches!(error, PortMappingError::UnsupportedGateway(_)));
    }
}
