//! Profile shape conversions: protocol, transport, TLS and endpoint.
//!
//! Split out of `contract_map.rs` when it reached the architecture gate's
//! 800-line ceiling. These are the mechanical, exhaustive `match` tables
//! between `voya_core`'s tagged enums and the contract ones; the file they
//! came from keeps the entry points that call them.

use voya_contracts::{
    Profile as ProfileContract, ProfileProtocol, ProfileTransport,
    ServerEndpoint as ContractServerEndpoint, TlsMode, TlsSettings,
};
use voya_core::{
    ProfileItem, ProfileProtocol as CoreProfileProtocol, ProfileTransport as CoreProfileTransport,
    ServerEndpoint as CoreServerEndpoint, TlsMode as CoreTlsMode, TlsSettings as CoreTlsSettings,
};

#[must_use]
pub fn profile_to_contract(item: ProfileItem) -> ProfileContract {
    ProfileContract {
        id: item.index_id,
        subscription_id: item.subscription_id,
        display_log: item.display_log,
        remarks: item.remarks,
        protocol: protocol_to_contract(item.protocol),
        transport: item.transport.map(transport_to_contract),
        tls: item.tls.map(tls_to_contract),
    }
}

#[must_use]
pub fn profile_from_contract(profile: ProfileContract) -> ProfileItem {
    ProfileItem {
        index_id: profile.id,
        subscription_id: profile.subscription_id,
        display_log: profile.display_log,
        remarks: profile.remarks,
        protocol: protocol_from_contract(profile.protocol),
        transport: profile.transport.map(transport_from_contract),
        tls: profile.tls.map(tls_from_contract),
    }
}

fn protocol_to_contract(protocol: CoreProfileProtocol) -> ProfileProtocol {
    match protocol {
        CoreProfileProtocol::Vmess {
            server,
            uuid,
            cipher,
        } => ProfileProtocol::Vmess {
            server: server_to_contract(server),
            uuid,
            cipher,
        },
        CoreProfileProtocol::Shadowsocks {
            server,
            password,
            method,
            udp_over_tcp,
        } => ProfileProtocol::Shadowsocks {
            server: server_to_contract(server),
            password,
            method,
            udp_over_tcp,
        },
        CoreProfileProtocol::Socks {
            server,
            username,
            password,
        } => ProfileProtocol::Socks {
            server: server_to_contract(server),
            username,
            password,
        },
        CoreProfileProtocol::Vless {
            server,
            uuid,
            flow,
            encryption,
        } => ProfileProtocol::Vless {
            server: server_to_contract(server),
            uuid,
            flow,
            encryption,
        },
        CoreProfileProtocol::Trojan { server, password } => ProfileProtocol::Trojan {
            server: server_to_contract(server),
            password,
        },
        CoreProfileProtocol::Hysteria2 {
            server,
            password,
            port_hops,
            obfuscation_password,
        } => ProfileProtocol::Hysteria2 {
            server: server_to_contract(server),
            password,
            port_hops,
            obfuscation_password,
        },
        CoreProfileProtocol::Tuic {
            server,
            uuid,
            password,
            congestion_control,
        } => ProfileProtocol::Tuic {
            server: server_to_contract(server),
            uuid,
            password,
            congestion_control,
        },
        CoreProfileProtocol::WireGuard {
            server,
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
        } => ProfileProtocol::WireGuard {
            server: server_to_contract(server),
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
        },
        CoreProfileProtocol::Http {
            server,
            username,
            password,
        } => ProfileProtocol::Http {
            server: server_to_contract(server),
            username,
            password,
        },
        CoreProfileProtocol::Anytls { server, password } => ProfileProtocol::Anytls {
            server: server_to_contract(server),
            password,
        },
        CoreProfileProtocol::Naive {
            server,
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
        } => ProfileProtocol::Naive {
            server: server_to_contract(server),
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
        },
    }
}

fn protocol_from_contract(protocol: ProfileProtocol) -> CoreProfileProtocol {
    match protocol {
        ProfileProtocol::Vmess {
            server,
            uuid,
            cipher,
        } => CoreProfileProtocol::Vmess {
            server: server_from_contract(server),
            uuid,
            cipher,
        },
        ProfileProtocol::Shadowsocks {
            server,
            password,
            method,
            udp_over_tcp,
        } => CoreProfileProtocol::Shadowsocks {
            server: server_from_contract(server),
            password,
            method,
            udp_over_tcp,
        },
        ProfileProtocol::Socks {
            server,
            username,
            password,
        } => CoreProfileProtocol::Socks {
            server: server_from_contract(server),
            username,
            password,
        },
        ProfileProtocol::Vless {
            server,
            uuid,
            flow,
            encryption,
        } => CoreProfileProtocol::Vless {
            server: server_from_contract(server),
            uuid,
            flow,
            encryption,
        },
        ProfileProtocol::Trojan { server, password } => CoreProfileProtocol::Trojan {
            server: server_from_contract(server),
            password,
        },
        ProfileProtocol::Hysteria2 {
            server,
            password,
            port_hops,
            obfuscation_password,
        } => CoreProfileProtocol::Hysteria2 {
            server: server_from_contract(server),
            password,
            port_hops,
            obfuscation_password,
        },
        ProfileProtocol::Tuic {
            server,
            uuid,
            password,
            congestion_control,
        } => CoreProfileProtocol::Tuic {
            server: server_from_contract(server),
            uuid,
            password,
            congestion_control,
        },
        ProfileProtocol::WireGuard {
            server,
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
        } => CoreProfileProtocol::WireGuard {
            server: server_from_contract(server),
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
        },
        ProfileProtocol::Http {
            server,
            username,
            password,
        } => CoreProfileProtocol::Http {
            server: server_from_contract(server),
            username,
            password,
        },
        ProfileProtocol::Anytls { server, password } => CoreProfileProtocol::Anytls {
            server: server_from_contract(server),
            password,
        },
        ProfileProtocol::Naive {
            server,
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
        } => CoreProfileProtocol::Naive {
            server: server_from_contract(server),
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
        },
    }
}

fn transport_to_contract(transport: CoreProfileTransport) -> ProfileTransport {
    match transport {
        CoreProfileTransport::Tcp { header, host, path } => {
            ProfileTransport::Tcp { header, host, path }
        }
        CoreProfileTransport::Websocket { host, path } => {
            ProfileTransport::Websocket { host, path }
        }
        CoreProfileTransport::HttpUpgrade { host, path } => {
            ProfileTransport::HttpUpgrade { host, path }
        }
        CoreProfileTransport::Http2 { host, path } => ProfileTransport::Http2 { host, path },
        CoreProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        } => ProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        },
        CoreProfileTransport::Quic { host, path } => ProfileTransport::Quic { host, path },
    }
}

fn transport_from_contract(transport: ProfileTransport) -> CoreProfileTransport {
    match transport {
        ProfileTransport::Tcp { header, host, path } => {
            CoreProfileTransport::Tcp { header, host, path }
        }
        ProfileTransport::Websocket { host, path } => {
            CoreProfileTransport::Websocket { host, path }
        }
        ProfileTransport::HttpUpgrade { host, path } => {
            CoreProfileTransport::HttpUpgrade { host, path }
        }
        ProfileTransport::Http2 { host, path } => CoreProfileTransport::Http2 { host, path },
        ProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        } => CoreProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        },
        ProfileTransport::Quic { host, path } => CoreProfileTransport::Quic { host, path },
    }
}

fn tls_to_contract(tls: CoreTlsSettings) -> TlsSettings {
    TlsSettings {
        mode: match tls.mode {
            CoreTlsMode::Tls => TlsMode::Tls,
            CoreTlsMode::Reality => TlsMode::Reality,
        },
        server_name: tls.server_name,
        alpn: tls.alpn,
        reality_public_key: tls.reality_public_key,
        reality_short_id: tls.reality_short_id,
        certificate_pem: tls.certificate_pem,
        ech_config: tls.ech_config,
    }
}

fn tls_from_contract(tls: TlsSettings) -> CoreTlsSettings {
    CoreTlsSettings {
        mode: match tls.mode {
            TlsMode::Tls => CoreTlsMode::Tls,
            TlsMode::Reality => CoreTlsMode::Reality,
        },
        server_name: tls.server_name,
        alpn: tls.alpn,
        reality_public_key: tls.reality_public_key,
        reality_short_id: tls.reality_short_id,
        certificate_pem: tls.certificate_pem,
        ech_config: tls.ech_config,
    }
}

fn server_to_contract(server: CoreServerEndpoint) -> ContractServerEndpoint {
    ContractServerEndpoint {
        address: server.address,
        port: server.port,
    }
}

fn server_from_contract(server: ContractServerEndpoint) -> CoreServerEndpoint {
    CoreServerEndpoint {
        address: server.address,
        port: server.port,
    }
}
