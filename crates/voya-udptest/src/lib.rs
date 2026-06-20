//! UDP tester support.
//!
//! This crate owns the SOCKS5 UDP-associate channel and the UDP probes used by
//! speed tests. The public API is intentionally small so app-level speed tests
//! can inject local fixtures and avoid external network dependencies.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};

use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
    time,
};

/// Names the crate boundary for workspace smoke tests.
pub const TESTER_FAMILY: &str = "udp";

const SOCKS5_VERSION: u8 = 0x05;
const SOCKS5_CMD_UDP_ASSOCIATE: u8 = 0x03;
const SOCKS5_ATYP_IPV4: u8 = 0x01;
const SOCKS5_ATYP_DOMAIN: u8 = 0x03;
const SOCKS5_ATYP_IPV6: u8 = 0x04;
const STUN_BINDING_SUCCESS_RESPONSE_TYPE: u16 = 0x0101;
const STUN_MAGIC_COOKIE: u32 = 0x2112_a442;
#[cfg(test)]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub type Result<T> = std::result::Result<T, UdpTestError>;

#[derive(Debug, Error)]
pub enum UdpTestError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("operation timed out after {0:?}")]
    Timeout(Duration),
    #[error("SOCKS5 server rejected no-auth method")]
    SocksNoAuthRejected,
    #[error("SOCKS5 UDP associate failed with reply {0}")]
    SocksUdpAssociateRejected(u8),
    #[error("SOCKS5 UDP relay address is invalid")]
    InvalidRelayAddress,
    #[error("SOCKS5 UDP channel has not been established")]
    ChannelNotEstablished,
    #[error("SOCKS5 UDP packet is too short")]
    PacketTooShort,
    #[error("SOCKS5 UDP fragmentation is not supported")]
    FragmentUnsupported,
    #[error("unsupported SOCKS5 address type {0}")]
    UnsupportedAddressType(u8),
    #[error("domain names in SOCKS5 UDP packets must be 255 bytes or shorter")]
    DomainTooLong,
    #[error("target port is out of range")]
    InvalidTargetPort,
    #[error("UDP response did not match the {0} probe")]
    ResponseVerificationFailed(UdpTestKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5RemoteEndpoint {
    pub host: String,
    pub port: u16,
    pub is_domain: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpTestKind {
    Ntp,
    Dns,
    Stun,
    Mcbe,
}

impl UdpTestKind {
    #[must_use]
    pub fn from_name(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "dns" => Self::Dns,
            "stun" => Self::Stun,
            "mcbe" => Self::Mcbe,
            _ => Self::Ntp,
        }
    }

    #[must_use]
    pub const fn default_target_host(self) -> &'static str {
        match self {
            Self::Ntp => "pool.ntp.org",
            Self::Dns => "8.8.8.8",
            Self::Stun => "stun.voztovoice.org",
            Self::Mcbe => "pms.mc-complex.com",
        }
    }

    #[must_use]
    pub const fn default_target_port(self) -> u16 {
        match self {
            Self::Ntp => 123,
            Self::Dns => 53,
            Self::Stun => 3478,
            Self::Mcbe => 19132,
        }
    }

    #[must_use]
    pub fn build_request_packet(self) -> Vec<u8> {
        match self {
            Self::Ntp => {
                let mut packet = vec![0; 48];
                packet[0] = 0x23;
                packet
            }
            Self::Dns => DNS_QUERY_PACKET.to_vec(),
            Self::Stun => STUN_BINDING_REQUEST_PACKET.to_vec(),
            Self::Mcbe => MCBE_QUERY_PACKET.to_vec(),
        }
    }

    #[must_use]
    pub fn verify_response(self, response: &[u8]) -> bool {
        match self {
            Self::Ntp => verify_ntp_response(response),
            Self::Dns => verify_dns_response(response),
            Self::Stun => verify_stun_response(response),
            Self::Mcbe => verify_mcbe_response(response),
        }
    }
}

impl fmt::Display for UdpTestKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ntp => formatter.write_str("ntp"),
            Self::Dns => formatter.write_str("dns"),
            Self::Stun => formatter.write_str("stun"),
            Self::Mcbe => formatter.write_str("mcbe"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpTestTarget {
    pub kind: UdpTestKind,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpTestService {
    kind: UdpTestKind,
}

impl UdpTestService {
    #[must_use]
    pub const fn new(kind: UdpTestKind) -> Self {
        Self { kind }
    }

    #[must_use]
    pub fn from_kind_name(kind: Option<&str>) -> Self {
        Self::new(kind.map_or(UdpTestKind::Ntp, UdpTestKind::from_name))
    }

    #[must_use]
    pub fn from_target(target: Option<&str>) -> (Self, UdpTestTarget) {
        let parsed = parse_udp_test_target(target);
        (Self::new(parsed.kind), parsed)
    }

    #[must_use]
    pub const fn kind(self) -> UdpTestKind {
        self.kind
    }

    #[must_use]
    pub fn build_request_packet(self) -> Vec<u8> {
        self.kind.build_request_packet()
    }

    #[must_use]
    pub fn verify_response(self, response: &[u8]) -> bool {
        self.kind.verify_response(response)
    }

    pub async fn send_via_socks5(
        self,
        socks5_host: &str,
        socks5_port: u16,
        target: &UdpTestTarget,
        operation_timeout: Duration,
    ) -> Result<Duration> {
        let deadline = async {
            let request = self.build_request_packet();
            let mut channel = Socks5UdpChannel::new(socks5_host, socks5_port);
            channel.establish_udp_association().await?;

            let mut best = Duration::MAX;
            let mut last_response = Vec::new();

            for attempt in 0..2 {
                let attempt_result: Result<(Duration, Vec<u8>)> = async {
                    let started = Instant::now();
                    channel
                        .send_to_host(&target.host, target.port, &request)
                        .await?;
                    let (_, response) = channel.receive().await?;
                    Ok((started.elapsed(), response))
                }
                .await;

                let (elapsed, response) = match attempt_result {
                    Ok(result) => result,
                    Err(_error) if attempt == 0 => continue,
                    Err(error) => return Err(error),
                };

                if elapsed < best {
                    best = elapsed;
                }
                last_response = response;

                if attempt == 0 && !self.verify_response(&last_response) {
                    continue;
                }
            }

            if self.verify_response(&last_response) {
                Ok(best)
            } else {
                Err(UdpTestError::ResponseVerificationFailed(self.kind))
            }
        };

        time::timeout(operation_timeout, deadline)
            .await
            .map_err(|_| UdpTestError::Timeout(operation_timeout))?
    }
}

#[derive(Debug)]
pub struct Socks5UdpChannel {
    socks5_host: String,
    socks5_tcp_port: u16,
    tcp_stream: Option<TcpStream>,
    udp_socket: Option<UdpSocket>,
    relay_endpoint: Option<SocketAddr>,
}

impl Socks5UdpChannel {
    #[must_use]
    pub fn new(socks5_host: impl Into<String>, socks5_tcp_port: u16) -> Self {
        Self {
            socks5_host: socks5_host.into(),
            socks5_tcp_port,
            tcp_stream: None,
            udp_socket: None,
            relay_endpoint: None,
        }
    }

    pub async fn establish_udp_association(&mut self) -> Result<()> {
        let mut tcp_stream =
            TcpStream::connect((self.socks5_host.as_str(), self.socks5_tcp_port)).await?;
        let udp_socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;

        tcp_stream.write_all(&[SOCKS5_VERSION, 0x01, 0x00]).await?;
        let mut handshake = [0; 2];
        tcp_stream.read_exact(&mut handshake).await?;
        if handshake != [SOCKS5_VERSION, 0x00] {
            return Err(UdpTestError::SocksNoAuthRejected);
        }

        let mut request = vec![SOCKS5_VERSION, SOCKS5_CMD_UDP_ASSOCIATE, 0x00];
        request.extend_from_slice(&encode_socks5_address("0.0.0.0", 0)?);
        tcp_stream.write_all(&request).await?;

        let mut header = [0; 3];
        tcp_stream.read_exact(&mut header).await?;
        if header[0] != SOCKS5_VERSION || header[1] != 0x00 {
            return Err(UdpTestError::SocksUdpAssociateRejected(header[1]));
        }

        let relay = read_socks5_address(&mut tcp_stream).await?;
        let relay_ip = if relay.host == "0.0.0.0" || relay.host == "::" {
            tcp_stream.peer_addr()?.ip()
        } else {
            relay
                .host
                .parse::<IpAddr>()
                .map_err(|_| UdpTestError::InvalidRelayAddress)?
        };

        self.relay_endpoint = Some(SocketAddr::new(relay_ip, relay.port));
        self.udp_socket = Some(udp_socket);
        self.tcp_stream = Some(tcp_stream);

        Ok(())
    }

    pub async fn send_to_host(&self, host: &str, port: u16, data: &[u8]) -> Result<usize> {
        let socket = self
            .udp_socket
            .as_ref()
            .ok_or(UdpTestError::ChannelNotEstablished)?;
        let relay = self
            .relay_endpoint
            .ok_or(UdpTestError::ChannelNotEstablished)?;
        let packet = build_socks5_udp_packet(host, port, data)?;

        Ok(socket.send_to(&packet, relay).await?)
    }

    pub async fn receive(&self) -> Result<(Socks5RemoteEndpoint, Vec<u8>)> {
        let socket = self
            .udp_socket
            .as_ref()
            .ok_or(UdpTestError::ChannelNotEstablished)?;
        let mut buffer = vec![0; 4096];
        let (length, _) = socket.recv_from(&mut buffer).await?;
        buffer.truncate(length);

        parse_socks5_udp_packet(&buffer)
    }
}

pub fn parse_udp_test_target(target: Option<&str>) -> UdpTestTarget {
    let target = target.map(str::trim).filter(|value| !value.is_empty());
    let (kind, host_port) = target
        .and_then(|value| value.split_once(':'))
        .map_or((UdpTestKind::Ntp, None), |(kind, host)| {
            (UdpTestKind::from_name(kind), Some(host))
        });

    let (host, port) = parse_host_and_port(host_port.unwrap_or_default(), kind);

    UdpTestTarget { kind, host, port }
}

pub fn build_socks5_udp_packet(host: &str, port: u16, data: &[u8]) -> Result<Vec<u8>> {
    let mut packet = vec![0x00, 0x00, 0x00];
    packet.extend_from_slice(&encode_socks5_address(host, port)?);
    packet.extend_from_slice(data);

    Ok(packet)
}

pub fn parse_socks5_udp_packet(packet: &[u8]) -> Result<(Socks5RemoteEndpoint, Vec<u8>)> {
    if packet.len() < 4 {
        return Err(UdpTestError::PacketTooShort);
    }
    if packet[2] != 0x00 {
        return Err(UdpTestError::FragmentUnsupported);
    }

    let (remote, offset) = parse_socks5_address_from_packet(packet, 3)?;
    Ok((remote, packet[offset..].to_vec()))
}

fn parse_host_and_port(value: &str, kind: UdpTestKind) -> (String, u16) {
    if value.is_empty() {
        return (
            kind.default_target_host().to_string(),
            kind.default_target_port(),
        );
    }

    if let Some(rest) = value.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            let host = &rest[..close];
            let port = rest
                .get(close + 1..)
                .and_then(|suffix| suffix.strip_prefix(':'))
                .and_then(|port| port.parse::<u16>().ok())
                .unwrap_or_else(|| kind.default_target_port());
            return (host.to_string(), port);
        }
    }

    if let Some((host, port)) = value.rsplit_once(':') {
        if !host.is_empty() {
            if let Ok(port) = port.parse::<u16>() {
                return (host.to_string(), port);
            }
        }
    }

    (value.to_string(), kind.default_target_port())
}

fn encode_socks5_address(host: &str, port: u16) -> Result<Vec<u8>> {
    let mut packet = Vec::new();
    if let Ok(ip) = host.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(ipv4) => {
                packet.push(SOCKS5_ATYP_IPV4);
                packet.extend_from_slice(&ipv4.octets());
            }
            IpAddr::V6(ipv6) => {
                packet.push(SOCKS5_ATYP_IPV6);
                packet.extend_from_slice(&ipv6.octets());
            }
        }
    } else {
        let bytes = host.as_bytes();
        let length = u8::try_from(bytes.len()).map_err(|_| UdpTestError::DomainTooLong)?;
        packet.push(SOCKS5_ATYP_DOMAIN);
        packet.push(length);
        packet.extend_from_slice(bytes);
    }

    packet.extend_from_slice(&port.to_be_bytes());
    Ok(packet)
}

async fn read_socks5_address<R>(reader: &mut R) -> Result<Socks5RemoteEndpoint>
where
    R: AsyncRead + Unpin,
{
    let mut atyp = [0; 1];
    reader.read_exact(&mut atyp).await?;
    match atyp[0] {
        SOCKS5_ATYP_IPV4 => {
            let mut address = [0; 4];
            reader.read_exact(&mut address).await?;
            let port = read_port(reader).await?;
            Ok(Socks5RemoteEndpoint {
                host: Ipv4Addr::from(address).to_string(),
                port,
                is_domain: false,
            })
        }
        SOCKS5_ATYP_IPV6 => {
            let mut address = [0; 16];
            reader.read_exact(&mut address).await?;
            let port = read_port(reader).await?;
            Ok(Socks5RemoteEndpoint {
                host: std::net::Ipv6Addr::from(address).to_string(),
                port,
                is_domain: false,
            })
        }
        SOCKS5_ATYP_DOMAIN => {
            let mut length = [0; 1];
            reader.read_exact(&mut length).await?;
            let mut domain = vec![0; usize::from(length[0])];
            reader.read_exact(&mut domain).await?;
            let port = read_port(reader).await?;
            Ok(Socks5RemoteEndpoint {
                host: String::from_utf8_lossy(&domain).into_owned(),
                port,
                is_domain: true,
            })
        }
        other => Err(UdpTestError::UnsupportedAddressType(other)),
    }
}

async fn read_port<R>(reader: &mut R) -> Result<u16>
where
    R: AsyncRead + Unpin,
{
    let mut port = [0; 2];
    reader.read_exact(&mut port).await?;
    Ok(u16::from_be_bytes(port))
}

fn parse_socks5_address_from_packet(
    packet: &[u8],
    mut offset: usize,
) -> Result<(Socks5RemoteEndpoint, usize)> {
    if packet.len() <= offset {
        return Err(UdpTestError::PacketTooShort);
    }

    let atyp = packet[offset];
    offset += 1;

    match atyp {
        SOCKS5_ATYP_IPV4 => {
            if packet.len() < offset + 4 + 2 {
                return Err(UdpTestError::PacketTooShort);
            }
            let host = Ipv4Addr::new(
                packet[offset],
                packet[offset + 1],
                packet[offset + 2],
                packet[offset + 3],
            )
            .to_string();
            offset += 4;
            let port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
            offset += 2;
            Ok((
                Socks5RemoteEndpoint {
                    host,
                    port,
                    is_domain: false,
                },
                offset,
            ))
        }
        SOCKS5_ATYP_IPV6 => {
            if packet.len() < offset + 16 + 2 {
                return Err(UdpTestError::PacketTooShort);
            }
            let mut address = [0; 16];
            address.copy_from_slice(&packet[offset..offset + 16]);
            let host = std::net::Ipv6Addr::from(address).to_string();
            offset += 16;
            let port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
            offset += 2;
            Ok((
                Socks5RemoteEndpoint {
                    host,
                    port,
                    is_domain: false,
                },
                offset,
            ))
        }
        SOCKS5_ATYP_DOMAIN => {
            if packet.len() <= offset {
                return Err(UdpTestError::PacketTooShort);
            }
            let length = usize::from(packet[offset]);
            offset += 1;
            if packet.len() < offset + length + 2 {
                return Err(UdpTestError::PacketTooShort);
            }
            let host = String::from_utf8_lossy(&packet[offset..offset + length]).into_owned();
            offset += length;
            let port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
            offset += 2;
            Ok((
                Socks5RemoteEndpoint {
                    host,
                    port,
                    is_domain: true,
                },
                offset,
            ))
        }
        other => Err(UdpTestError::UnsupportedAddressType(other)),
    }
}

fn verify_ntp_response(response: &[u8]) -> bool {
    response.len() >= 48 && (response[0] & 0x07) == 4
}

fn verify_dns_response(response: &[u8]) -> bool {
    if response.len() < 12 {
        return false;
    }
    let transaction_id = u16::from_be_bytes([response[0], response[1]]);
    let flags = u16::from_be_bytes([response[2], response[3]]);
    let answer_count = u16::from_be_bytes([response[6], response[7]]);

    transaction_id == 0x1234 && (flags & 0x8000) != 0 && (flags & 0x000f) == 0 && answer_count > 0
}

fn verify_stun_response(response: &[u8]) -> bool {
    if response.len() < 20 {
        return false;
    }
    let message_type = u16::from_be_bytes([response[0], response[1]]);
    let message_length = usize::from(u16::from_be_bytes([response[2], response[3]]));
    let magic_cookie = u32::from_be_bytes([response[4], response[5], response[6], response[7]]);

    message_type == STUN_BINDING_SUCCESS_RESPONSE_TYPE
        && message_length % 4 == 0
        && response.len() == 20 + message_length
        && magic_cookie == STUN_MAGIC_COOKIE
}

fn verify_mcbe_response(response: &[u8]) -> bool {
    if response.len() < 48 || response[0] != 0x1c {
        return false;
    }
    if response.get(17..33) != Some(MCBE_MAGIC_BYTES) {
        return false;
    }
    let Some(length_bytes) = response.get(33..35) else {
        return false;
    };
    let length = usize::from(u16::from_be_bytes([length_bytes[0], length_bytes[1]]));
    let Some(data) = response.get(35..35 + length) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(data) else {
        return false;
    };
    let game_mode = text.split(';').nth(8).unwrap_or_default();

    matches!(
        game_mode,
        "Survival" | "Creative" | "Adventure" | "Spectator"
    )
}

const DNS_QUERY_PACKET: &[u8] = &[
    0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x77, 0x77, 0x77,
    0x06, 0x67, 0x6f, 0x6f, 0x67, 0x6c, 0x65, 0x03, 0x63, 0x6f, 0x6d, 0x00, 0x00, 0x01, 0x00, 0x01,
];

const STUN_BINDING_REQUEST_PACKET: &[u8] = &[
    0x00, 0x01, 0x00, 0x00, 0x21, 0x12, 0xa4, 0x42, 0x66, 0x0e, 0xab, 0xbc, 0x61, 0x0d, 0xa4, 0x40,
    0x8c, 0x65, 0xc1, 0xbe,
];

const MCBE_QUERY_PACKET: &[u8] = &[
    0x01, 0x27, 0xc4, 0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe,
    0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78, 0x66, 0x0e, 0xab, 0xbc, 0x61, 0x0d, 0x1f,
    0x4e, 0xa4, 0x40, 0x8c, 0x65, 0xc1, 0xbe, 0xf5, 0x4b,
];

const MCBE_MAGIC_BYTES: &[u8] = &[
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

#[cfg(test)]
mod tests {
    use tokio::{net::TcpListener, sync::oneshot};

    use super::*;

    #[test]
    fn tester_family_is_udp() {
        assert_eq!(TESTER_FAMILY, "udp");
    }

    #[test]
    fn udp_test_target_defaults_and_host_port_parsing() {
        assert_eq!(
            parse_udp_test_target(None),
            UdpTestTarget {
                kind: UdpTestKind::Ntp,
                host: "pool.ntp.org".to_string(),
                port: 123,
            }
        );
        assert_eq!(
            parse_udp_test_target(Some("dns:1.1.1.1:5353")),
            UdpTestTarget {
                kind: UdpTestKind::Dns,
                host: "1.1.1.1".to_string(),
                port: 5353,
            }
        );
        assert_eq!(
            parse_udp_test_target(Some("stun:[2001:db8::1]:3479")),
            UdpTestTarget {
                kind: UdpTestKind::Stun,
                host: "2001:db8::1".to_string(),
                port: 3479,
            }
        );
        assert_eq!(
            parse_udp_test_target(Some("unknown:example.com")),
            UdpTestTarget {
                kind: UdpTestKind::Ntp,
                host: "example.com".to_string(),
                port: 123,
            }
        );
    }

    #[test]
    fn socks5_udp_packet_round_trips_domain_and_ip_addresses() {
        let packet = build_socks5_udp_packet("example.com", 53, b"hello")
            .expect("domain SOCKS5 UDP packet should build");
        let (remote, payload) =
            parse_socks5_udp_packet(&packet).expect("domain SOCKS5 UDP packet should parse");
        assert_eq!(remote.host, "example.com");
        assert_eq!(remote.port, 53);
        assert!(remote.is_domain);
        assert_eq!(payload, b"hello");

        let packet = build_socks5_udp_packet("127.0.0.1", 8080, b"v4")
            .expect("IPv4 SOCKS5 UDP packet should build");
        let (remote, payload) =
            parse_socks5_udp_packet(&packet).expect("IPv4 SOCKS5 UDP packet should parse");
        assert_eq!(remote.host, "127.0.0.1");
        assert_eq!(remote.port, 8080);
        assert!(!remote.is_domain);
        assert_eq!(payload, b"v4");

        let packet = build_socks5_udp_packet("2001:db8::1", 8081, b"v6")
            .expect("IPv6 SOCKS5 UDP packet should build");
        let (remote, payload) =
            parse_socks5_udp_packet(&packet).expect("IPv6 SOCKS5 UDP packet should parse");
        assert_eq!(remote.host, "2001:db8::1");
        assert_eq!(remote.port, 8081);
        assert!(!remote.is_domain);
        assert_eq!(payload, b"v6");
    }

    #[test]
    fn tester_packets_and_verifiers_match_reference_shapes() {
        let ntp = UdpTestKind::Ntp.build_request_packet();
        assert_eq!(ntp.len(), 48);
        assert_eq!(ntp[0], 0x23);
        let mut ntp_response = vec![0; 48];
        ntp_response[0] = 0x24;
        assert!(UdpTestKind::Ntp.verify_response(&ntp_response));

        let dns = UdpTestKind::Dns.build_request_packet();
        assert_eq!(&dns[..2], &[0x12, 0x34]);
        let mut dns_response = vec![0; 32];
        dns_response[..8].copy_from_slice(&[0x12, 0x34, 0x81, 0x80, 0, 1, 0, 1]);
        assert!(UdpTestKind::Dns.verify_response(&dns_response));

        let mut stun_response = vec![0; 20];
        stun_response[..2].copy_from_slice(&[0x01, 0x01]);
        stun_response[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        assert!(UdpTestKind::Stun.verify_response(&stun_response));

        let mcbe_text =
            b"MCPE;Dedicated Server;527;1.19.1;0;10;id;Bedrock level;Survival;1;19132;19133;";
        let mut mcbe_response = vec![0; 35 + mcbe_text.len()];
        mcbe_response[0] = 0x1c;
        mcbe_response[17..33].copy_from_slice(MCBE_MAGIC_BYTES);
        mcbe_response[33..35].copy_from_slice(&(mcbe_text.len() as u16).to_be_bytes());
        mcbe_response[35..].copy_from_slice(mcbe_text);
        assert!(UdpTestKind::Mcbe.verify_response(&mcbe_response));
    }

    #[test]
    fn stun_verifier_rejects_malformed_or_non_success_responses() {
        let mut valid = vec![0; 20];
        valid[..2].copy_from_slice(&STUN_BINDING_SUCCESS_RESPONSE_TYPE.to_be_bytes());
        valid[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        assert!(UdpTestKind::Stun.verify_response(&valid));

        let mut wrong_type = valid.clone();
        wrong_type[..2].copy_from_slice(&0x0111_u16.to_be_bytes());
        assert!(!UdpTestKind::Stun.verify_response(&wrong_type));

        let mut wrong_cookie = valid.clone();
        wrong_cookie[4..8].copy_from_slice(&0xfeed_beef_u32.to_be_bytes());
        assert!(!UdpTestKind::Stun.verify_response(&wrong_cookie));

        let mut truncated = valid.clone();
        truncated[2..4].copy_from_slice(&4_u16.to_be_bytes());
        assert!(!UdpTestKind::Stun.verify_response(&truncated));

        let mut extra_bytes = valid.clone();
        extra_bytes.push(0);
        assert!(!UdpTestKind::Stun.verify_response(&extra_bytes));

        assert!(!UdpTestKind::Stun.verify_response(&valid[..19]));
    }

    #[tokio::test]
    async fn socks5_udp_channel_uses_local_associate_relay() {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("test SOCKS5 TCP listener should bind");
        let tcp_port = listener
            .local_addr()
            .expect("test SOCKS5 TCP listener should expose local address")
            .port();
        let relay = UdpSocket::bind(("127.0.0.1", 0))
            .await
            .expect("test UDP relay should bind");
        let relay_addr = relay
            .local_addr()
            .expect("test UDP relay should expose local address");
        let (ready_tx, ready_rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("test SOCKS5 listener should accept client");
            let mut greeting = [0; 3];
            stream
                .read_exact(&mut greeting)
                .await
                .expect("test SOCKS5 server should read greeting");
            assert_eq!(greeting, [SOCKS5_VERSION, 1, 0]);
            stream
                .write_all(&[SOCKS5_VERSION, 0])
                .await
                .expect("test SOCKS5 server should write method response");

            let mut request = [0; 10];
            stream
                .read_exact(&mut request)
                .await
                .expect("test SOCKS5 server should read UDP associate request");
            assert_eq!(
                request[..4],
                [SOCKS5_VERSION, SOCKS5_CMD_UDP_ASSOCIATE, 0, 1]
            );

            let mut reply = vec![SOCKS5_VERSION, 0, 0];
            reply.extend_from_slice(
                &encode_socks5_address("127.0.0.1", relay_addr.port())
                    .expect("test UDP relay address should encode"),
            );
            stream
                .write_all(&reply)
                .await
                .expect("test SOCKS5 server should write UDP associate response");
            ready_tx
                .send(())
                .expect("test SOCKS5 server should notify readiness");

            let mut buffer = vec![0; 1024];
            let (length, peer) = relay
                .recv_from(&mut buffer)
                .await
                .expect("test UDP relay should receive packet");
            buffer.truncate(length);
            let (remote, payload) =
                parse_socks5_udp_packet(&buffer).expect("test UDP relay packet should parse");
            assert_eq!(remote.host, "example.com");
            assert_eq!(remote.port, 53);
            assert_eq!(payload, b"ping");

            let response = build_socks5_udp_packet("example.com", 53, b"pong")
                .expect("test UDP relay response should build");
            relay
                .send_to(&response, peer)
                .await
                .expect("test UDP relay should send response");
            let mut hold = [0; 1];
            let _ = stream.read(&mut hold).await;
        });

        let mut channel = Socks5UdpChannel::new("127.0.0.1", tcp_port);
        time::timeout(DEFAULT_TIMEOUT, channel.establish_udp_association())
            .await
            .expect("UDP association should finish before timeout")
            .expect("UDP association should succeed");
        ready_rx
            .await
            .expect("test SOCKS5 server should signal readiness");
        channel
            .send_to_host("example.com", 53, b"ping")
            .await
            .expect("UDP channel should send packet to host");
        let (remote, payload) = time::timeout(DEFAULT_TIMEOUT, channel.receive())
            .await
            .expect("UDP receive should finish before timeout")
            .expect("UDP channel should receive response");

        assert_eq!(remote.host, "example.com");
        assert_eq!(payload, b"pong");
    }
}
