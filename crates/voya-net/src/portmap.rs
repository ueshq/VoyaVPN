//! Asks the home router to forward the self-hosted node's ports (UPnP IGD).
//!
//! Only the router in front of this device can be asked: a carrier-grade NAT
//! further out never answers UPnP, which is what the environment check reports
//! when the router's own WAN address is not the public one.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::Duration,
};

use futures_util::future::BoxFuture;
use igd_next::{aio::tokio::search_gateway, PortMappingProtocol, SearchOptions};
use thiserror::Error;

const SEARCH_TIMEOUT: Duration = Duration::from_secs(3);
const SINGLE_SEARCH_TIMEOUT: Duration = Duration::from_secs(2);
pub const PORT_MAPPING_DESCRIPTION: &str = "VoyaVPN self-hosted node";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortMappingResult {
    /// The router's own WAN address, if it reported one.
    pub external_address: Option<IpAddr>,
    /// Ports whose TCP forward was accepted. UDP is added best-effort.
    pub mapped: Vec<u16>,
    /// Ports the router refused, with its reason.
    pub refused: Vec<(u16, String)>,
}

#[derive(Debug, Error)]
pub enum PortMappingError {
    #[error("no UPnP gateway answered: {0}")]
    NoGateway(String),
    #[error("could not find this device's address toward the gateway: {0}")]
    LocalAddress(io::Error),
    #[error("the gateway is not an IPv4 device: {0}")]
    UnsupportedGateway(IpAddr),
}

/// Router port forwarding, behind a trait so the orchestration can be tested
/// without a router.
pub trait PortMapper: Send + Sync {
    fn map(
        &self,
        ports: Vec<u16>,
        lease_seconds: u32,
    ) -> BoxFuture<'static, Result<PortMappingResult, PortMappingError>>;
    fn unmap(&self, ports: Vec<u16>) -> BoxFuture<'static, Result<(), PortMappingError>>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UpnpPortMapper;

impl PortMapper for UpnpPortMapper {
    fn map(
        &self,
        ports: Vec<u16>,
        lease_seconds: u32,
    ) -> BoxFuture<'static, Result<PortMappingResult, PortMappingError>> {
        Box::pin(async move {
            let gateway = search_gateway(search_options())
                .await
                .map_err(|error| PortMappingError::NoGateway(error.to_string()))?;
            let local = local_address_toward(gateway.addr)?;
            let external_address = gateway.get_external_ip().await.ok();
            let mut result = PortMappingResult {
                external_address,
                mapped: Vec::new(),
                refused: Vec::new(),
            };
            for port in ports {
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
                        result.mapped.push(port);
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
                    Err(error) => result.refused.push((port, error.to_string())),
                }
            }
            Ok(result)
        })
    }

    fn unmap(&self, ports: Vec<u16>) -> BoxFuture<'static, Result<(), PortMappingError>> {
        Box::pin(async move {
            let gateway = search_gateway(search_options())
                .await
                .map_err(|error| PortMappingError::NoGateway(error.to_string()))?;
            for port in ports {
                for protocol in [PortMappingProtocol::TCP, PortMappingProtocol::UDP] {
                    if let Err(error) = gateway.remove_port(protocol, port).await {
                        tracing::debug!(port, %error, "router kept a forward we asked to remove");
                    }
                }
            }
            Ok(())
        })
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
