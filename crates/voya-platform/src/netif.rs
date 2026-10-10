//! This machine's network interfaces and listening ports, as the self-hosted
//! node's environment check sees them.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, UdpSocket},
};

/// One address assigned to a network interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceAddress {
    pub interface: String,
    pub address: IpAddr,
}

/// Every non-loopback address on the machine's interfaces, IPv4 first.
pub fn interface_addresses() -> io::Result<Vec<InterfaceAddress>> {
    let mut addresses = if_addrs::get_if_addrs()?
        .into_iter()
        .filter(|interface| !interface.is_loopback())
        .map(|interface| InterfaceAddress {
            address: interface.ip(),
            interface: interface.name,
        })
        .collect::<Vec<_>>();
    addresses.sort_by_key(|entry| (entry.address.is_ipv6(), entry.interface.clone()));
    addresses.dedup();
    Ok(addresses)
}

/// The routers on this machine's default routes: where a NAT-PMP or PCP
/// request has to be sent, since neither protocol has a discovery step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DefaultGateways {
    pub ipv4: Option<Ipv4Addr>,
    pub ipv6: Option<Ipv6Gateway>,
}

/// An IPv6 router. It is normally a link-local address, which only means
/// something together with the interface it was learned on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv6Gateway {
    pub address: Ipv6Addr,
    pub scope_id: u32,
}

/// The default routes' gateways, skipping tunnels: a VPN's virtual router
/// forwards nothing to this device.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
#[must_use]
pub fn default_gateways() -> DefaultGateways {
    let mut interfaces = netdev::get_interfaces();
    // The default interface first; another physical one only fills a family
    // the default route does not have.
    interfaces.sort_by_key(|interface| !interface.default);
    let mut gateways = DefaultGateways::default();
    for interface in interfaces {
        if interface.is_loopback() || interface.is_tun() || !interface.is_up() {
            continue;
        }
        let Some(gateway) = interface.gateway else {
            continue;
        };
        if gateways.ipv4.is_none() {
            gateways.ipv4 = gateway.ipv4.first().copied();
        }
        if gateways.ipv6.is_none() {
            gateways.ipv6 = gateway.ipv6.first().map(|address| Ipv6Gateway {
                address: *address,
                scope_id: interface.index,
            });
        }
    }
    gateways
}

/// A phone hosts no node, so it never asks a router for anything.
#[cfg(any(target_os = "ios", target_os = "android"))]
#[must_use]
pub fn default_gateways() -> DefaultGateways {
    DefaultGateways::default()
}

/// Whether a server could listen on `port` on every address, for both TCP and
/// UDP (Shadowsocks serves both on one port).
///
/// Probed on the IPv4 and the IPv6 wildcard separately: a port can be free on
/// loopback and taken on the wildcard, and Windows binds `[::]` IPv6-only. A
/// machine without an IPv6 stack only has IPv4 to check.
#[must_use]
pub fn wildcard_port_available(port: u16) -> bool {
    let wildcards = [
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    ];
    wildcards.into_iter().all(|address| {
        let socket = SocketAddr::new(address, port);
        bindable(TcpListener::bind(socket).map(drop)) && bindable(UdpSocket::bind(socket).map(drop))
    })
}

fn bindable(result: io::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        // No IPv6 stack (or IPv6 disabled): nothing can collide there.
        Err(error) => {
            error.kind() == io::ErrorKind::AddrNotAvailable || is_family_unsupported(&error)
        }
    }
}

fn is_family_unsupported(error: &io::Error) -> bool {
    // EAFNOSUPPORT is 97 on Linux, 47 on macOS; WSAEAFNOSUPPORT is 10047.
    matches!(error.raw_os_error(), Some(97 | 47 | 10047))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_port_held_on_the_wildcard_is_unavailable() {
        let listener =
            TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).expect("bind an ephemeral port");
        let port = listener.local_addr().expect("local address").port();
        assert!(!wildcard_port_available(port));
        drop(listener);
    }

    #[test]
    fn a_released_port_is_available_again() {
        // Anything else on the machine — including the tests sharing this
        // binary's thread pool — can take the port between the release and the
        // probe, and it keeps holding it. Retrying the same port is therefore
        // no retry at all: every attempt picks a fresh one, so this fails only
        // when the probe is wrong about every freshly released port.
        let available = (0..8).any(|_| wildcard_port_available(ephemeral_port()));
        assert!(available);
    }

    fn ephemeral_port() -> u16 {
        TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
            .and_then(|listener| listener.local_addr())
            .map(|address| address.port())
            .expect("ephemeral port")
    }

    #[test]
    fn default_gateways_are_never_loopback() {
        let gateways = default_gateways();
        println!("default gateways: {gateways:?}");
        assert!(gateways.ipv4.is_none_or(|address| !address.is_loopback()));
        assert!(gateways
            .ipv6
            .is_none_or(|gateway| !gateway.address.is_loopback()));
    }

    #[test]
    fn interface_addresses_never_include_loopback() {
        let addresses = interface_addresses().expect("interfaces enumerate");
        assert!(addresses.iter().all(|entry| !entry.address.is_loopback()));
    }
}
