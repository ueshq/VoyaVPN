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
/// Byte range of the STUN transaction id inside a binding request or response.
const STUN_TRANSACTION_ID_RANGE: std::ops::Range<usize> = 8..20;
/// Probes sent per UDP test. The second one exists to survive a single lost datagram, so each
/// attempt is bounded separately — the whole budget must never be spent waiting for the first.
const UDP_TEST_ATTEMPTS: u32 = 2;

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
    #[error("UDP test target {0} is not a valid [kind:]host[:port]")]
    InvalidTargetHost(String),
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
    /// Parses a probe-kind prefix, returning `None` for anything that is not one.
    ///
    /// A bare `host:port` target such as `1.1.1.1:53` must not be reinterpreted as a kind — that
    /// silently turned the host into `53` and probed it on the NTP port.
    #[must_use]
    pub fn parse_name(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ntp" => Some(Self::Ntp),
            "dns" => Some(Self::Dns),
            "stun" => Some(Self::Stun),
            "mcbe" => Some(Self::Mcbe),
            _ => None,
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

    /// Probes `target` through the local SOCKS5 UDP associate and returns the best verified
    /// round-trip time.
    ///
    /// Every attempt is bounded separately inside `operation_timeout`, so a lost datagram — the
    /// exact case the retry exists for — reaches the retry instead of consuming the whole budget.
    /// Only a reply that passes `verify_response` can produce a measurement, and once one has,
    /// nothing a later attempt does can take it away.
    pub async fn send_via_socks5(
        self,
        socks5_host: &str,
        socks5_port: u16,
        target: &UdpTestTarget,
        operation_timeout: Duration,
    ) -> Result<Duration> {
        let request = self.build_request_packet();
        let started = Instant::now();
        let mut channel = Socks5UdpChannel::new(socks5_host, socks5_port);
        time::timeout(operation_timeout, channel.establish_udp_association())
            .await
            .map_err(|_| UdpTestError::Timeout(operation_timeout))??;

        let mut best_verified: Option<Duration> = None;
        let mut last_error = None;

        for attempt in 0..UDP_TEST_ATTEMPTS {
            let remaining = operation_timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            // Split what is left evenly across the attempts that have not run yet.
            let budget = remaining / (UDP_TEST_ATTEMPTS - attempt);
            let outcome = time::timeout(budget, async {
                let sent_at = Instant::now();
                channel
                    .send_to_host(&target.host, target.port, &request)
                    .await?;
                let (_, response) = channel.receive().await?;
                Ok::<_, UdpTestError>((sent_at.elapsed(), response))
            })
            .await;

            match outcome {
                Ok(Ok((elapsed, response))) if self.verify_response(&response) => {
                    best_verified = Some(best_verified.map_or(elapsed, |best| best.min(elapsed)));
                }
                Ok(Ok(_)) => {
                    last_error = Some(UdpTestError::ResponseVerificationFailed(self.kind));
                }
                Ok(Err(error)) => last_error = Some(error),
                Err(_) => last_error = Some(UdpTestError::Timeout(budget)),
            }
        }

        match best_verified {
            Some(best) => Ok(best),
            // No attempt ran at all only when the association already consumed the budget.
            None => Err(last_error.unwrap_or(UdpTestError::Timeout(operation_timeout))),
        }
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

    /// Opens the SOCKS5 control connection and the UDP socket that carries the probe.
    ///
    /// The UDP socket binds to the control connection's own local address rather than to every
    /// interface: the relay is always the proxy this app just launched, so a socket on all
    /// interfaces would only add an inbound attack surface (and, on Windows, a firewall prompt)
    /// for the duration of the test. After the association is known the socket is `connect`ed to
    /// the relay, which makes the kernel drop datagrams from anybody else — a LAN host can no
    /// longer race a forged reply into a STUN or NTP measurement.
    pub async fn establish_udp_association(&mut self) -> Result<()> {
        let mut tcp_stream =
            TcpStream::connect((self.socks5_host.as_str(), self.socks5_tcp_port)).await?;
        let udp_socket = UdpSocket::bind(SocketAddr::new(tcp_stream.local_addr()?.ip(), 0)).await?;

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
        let relay_endpoint = SocketAddr::new(relay_ip, relay.port);
        udp_socket.connect(relay_endpoint).await?;

        self.relay_endpoint = Some(relay_endpoint);
        self.udp_socket = Some(udp_socket);
        self.tcp_stream = Some(tcp_stream);

        Ok(())
    }

    pub async fn send_to_host(&self, host: &str, port: u16, data: &[u8]) -> Result<usize> {
        let socket = self
            .udp_socket
            .as_ref()
            .ok_or(UdpTestError::ChannelNotEstablished)?;
        if self.relay_endpoint.is_none() {
            return Err(UdpTestError::ChannelNotEstablished);
        }
        let packet = build_socks5_udp_packet(host, port, data)?;

        Ok(socket.send(&packet).await?)
    }

    /// Reads the next datagram from the relay.
    ///
    /// The socket is connected, so the kernel has already discarded anything that did not come
    /// from the relay endpoint and the sender address needs no further checking here.
    pub async fn receive(&self) -> Result<(Socks5RemoteEndpoint, Vec<u8>)> {
        let socket = self
            .udp_socket
            .as_ref()
            .ok_or(UdpTestError::ChannelNotEstablished)?;
        let mut buffer = vec![0; 4096];
        let length = socket.recv(&mut buffer).await?;
        buffer.truncate(length);

        parse_socks5_udp_packet(&buffer)
    }
}

/// Parses a UDP test target of the form `[<kind>:]host[:port]`.
///
/// The leading segment is only treated as a probe kind when it actually names one, so a plain
/// `1.1.1.1:53` stays the host and port the user wrote instead of becoming host `53`. Everything
/// the target leaves out falls back to the kind's own defaults.
#[must_use]
pub fn parse_udp_test_target(target: Option<&str>) -> UdpTestTarget {
    let target = target
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    let (kind, host_port) = target
        .split_once(':')
        .and_then(|(prefix, rest)| Some((UdpTestKind::parse_name(prefix)?, rest)))
        .unwrap_or((UdpTestKind::Ntp, target));

    let (host, port) = parse_host_and_port(host_port.trim(), kind);

    UdpTestTarget { kind, host, port }
}

/// Rejects a UDP test target the parser would otherwise have to guess at, so a malformed setting
/// is refused at save time rather than surfacing later as an unexplained probe failure.
pub fn validate_udp_test_target(target: &str) -> Result<UdpTestTarget> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Ok(parse_udp_test_target(None));
    }

    // A recognised kind prefix with nothing after it ("dns:") would otherwise fall
    // through to that kind's built-in default, so the user's half-typed target
    // would silently probe a server they never named.
    let names_a_kind_without_a_host = trimmed.split_once(':').is_some_and(|(prefix, rest)| {
        UdpTestKind::parse_name(prefix).is_some() && rest.trim().is_empty()
    });

    let parsed = parse_udp_test_target(Some(trimmed));
    // `parse_host_and_port` keeps an unparseable `:port` suffix as part of the host rather than
    // guessing, so a host that still carries a colon without being an IP literal is malformed.
    if names_a_kind_without_a_host
        || parsed.host.trim().is_empty()
        || (parsed.host.contains(':') && parsed.host.parse::<IpAddr>().is_err())
    {
        return Err(UdpTestError::InvalidTargetHost(trimmed.to_string()));
    }

    Ok(parsed)
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
    // The transaction id is what ties a reply to the request this crate sent, so an unrelated
    // binding response that happens to reach the socket cannot be timed as ours.
    let transaction_id_matches = response.get(STUN_TRANSACTION_ID_RANGE)
        == STUN_BINDING_REQUEST_PACKET.get(STUN_TRANSACTION_ID_RANGE);

    message_type == STUN_BINDING_SUCCESS_RESPONSE_TYPE
        && message_length % 4 == 0
        && response.len() == 20 + message_length
        && magic_cookie == STUN_MAGIC_COOKIE
        && transaction_id_matches
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

    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

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
    }

    /// `1.1.1.1:53` used to be read as kind `1.1.1.1` (an unknown name, so NTP) with host `53`,
    /// which probed a host literally called "53" on port 123.
    #[test]
    fn plain_host_port_targets_are_not_mistaken_for_a_probe_kind() {
        assert_eq!(
            parse_udp_test_target(Some("1.1.1.1:53")),
            UdpTestTarget {
                kind: UdpTestKind::Ntp,
                host: "1.1.1.1".to_string(),
                port: 53,
            }
        );
        assert_eq!(
            parse_udp_test_target(Some("example.com")),
            UdpTestTarget {
                kind: UdpTestKind::Ntp,
                host: "example.com".to_string(),
                port: 123,
            }
        );
        assert_eq!(
            parse_udp_test_target(Some("[::1]:53")),
            UdpTestTarget {
                kind: UdpTestKind::Ntp,
                host: "::1".to_string(),
                port: 53,
            }
        );
        assert_eq!(
            parse_udp_test_target(Some("ntp:time.example.com")),
            UdpTestTarget {
                kind: UdpTestKind::Ntp,
                host: "time.example.com".to_string(),
                port: 123,
            }
        );
        assert_eq!(UdpTestKind::parse_name("nope"), None);
    }

    #[test]
    fn target_validation_rejects_hosts_the_parser_would_have_to_guess_at() {
        assert_eq!(
            validate_udp_test_target("dns:1.1.1.1:5353").expect("well formed target"),
            UdpTestTarget {
                kind: UdpTestKind::Dns,
                host: "1.1.1.1".to_string(),
                port: 5353,
            }
        );
        assert_eq!(
            validate_udp_test_target("   ").expect("an empty target falls back to the default"),
            parse_udp_test_target(None)
        );

        for target in ["unknown:example.com", "2001:db8::1", "dns:"] {
            let error = validate_udp_test_target(target)
                .expect_err("a malformed target should be rejected");
            assert!(
                matches!(error, UdpTestError::InvalidTargetHost(_)),
                "{target}: {error:?}"
            );
        }
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

        let stun_response = stun_binding_success_response();
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
        let valid = stun_binding_success_response();
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

        // A well-formed binding response from an unrelated exchange must not be timed as ours.
        let mut foreign_transaction = valid.clone();
        foreign_transaction[STUN_TRANSACTION_ID_RANGE].fill(0x5a);
        assert!(!UdpTestKind::Stun.verify_response(&foreign_transaction));
    }

    /// Builds the success response that matches this crate's fixed binding request.
    fn stun_binding_success_response() -> Vec<u8> {
        let mut response = vec![0; 20];
        response[..2].copy_from_slice(&STUN_BINDING_SUCCESS_RESPONSE_TYPE.to_be_bytes());
        response[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        response[STUN_TRANSACTION_ID_RANGE]
            .copy_from_slice(&STUN_BINDING_REQUEST_PACKET[STUN_TRANSACTION_ID_RANGE]);
        response
    }

    /// Runs a local SOCKS5 UDP associate: it answers `valid_replies` probes with a well-formed
    /// NTP response, the next `junk_replies` with an unverifiable payload, and silently drops
    /// everything after that — the lost-datagram case the retry exists for.
    async fn spawn_socks5_udp_relay(valid_replies: usize, junk_replies: usize) -> u16 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("test SOCKS5 TCP listener should bind");
        let tcp_port = listener
            .local_addr()
            .expect("test SOCKS5 TCP listener address")
            .port();
        let relay = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("test UDP relay should bind");
        let relay_port = relay.local_addr().expect("test UDP relay address").port();

        tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut greeting = [0; 3];
            if stream.read_exact(&mut greeting).await.is_err() {
                return;
            }
            if stream.write_all(&[SOCKS5_VERSION, 0]).await.is_err() {
                return;
            }
            let mut request = [0; 10];
            if stream.read_exact(&mut request).await.is_err() {
                return;
            }
            let mut reply = vec![SOCKS5_VERSION, 0, 0];
            reply.extend_from_slice(
                &encode_socks5_address("127.0.0.1", relay_port)
                    .expect("test UDP relay address should encode"),
            );
            if stream.write_all(&reply).await.is_err() {
                return;
            }

            let mut buffer = vec![0; 4096];
            let mut answered = 0;
            loop {
                let Ok((length, peer)) = relay.recv_from(&mut buffer).await else {
                    return;
                };
                let Ok((remote, _)) = parse_socks5_udp_packet(&buffer[..length]) else {
                    return;
                };
                let payload = if answered < valid_replies {
                    let mut ntp = vec![0; 48];
                    ntp[0] = 0x24;
                    Some(ntp)
                } else if answered < valid_replies + junk_replies {
                    Some(b"junk".to_vec())
                } else {
                    None
                };
                answered += 1;
                let Some(payload) = payload else {
                    continue;
                };
                let Ok(response) = build_socks5_udp_packet(&remote.host, remote.port, &payload)
                else {
                    return;
                };
                if relay.send_to(&response, peer).await.is_err() {
                    return;
                }
            }
        });

        tcp_port
    }

    fn loopback_ntp_target() -> UdpTestTarget {
        UdpTestTarget {
            kind: UdpTestKind::Ntp,
            host: "127.0.0.1".to_string(),
            port: 123,
        }
    }

    /// The retry used to overwrite the verified first reply, so an unusable second answer failed
    /// a test that had already succeeded.
    #[tokio::test]
    async fn udp_test_keeps_a_verified_measurement_when_the_retry_answers_with_junk() {
        let socks_port = spawn_socks5_udp_relay(1, 1).await;

        let elapsed = UdpTestService::new(UdpTestKind::Ntp)
            .send_via_socks5(
                "127.0.0.1",
                socks_port,
                &loopback_ntp_target(),
                Duration::from_millis(800),
            )
            .await
            .expect("a verified first probe must survive an unusable retry");

        assert!(elapsed < Duration::from_millis(800), "{elapsed:?}");
    }

    /// A dropped second datagram must land in the retry's own budget instead of consuming the
    /// whole operation timeout and reporting the verified first probe as a timeout.
    #[tokio::test]
    async fn udp_test_keeps_a_verified_measurement_when_the_retry_is_dropped() {
        let socks_port = spawn_socks5_udp_relay(1, 0).await;

        let elapsed = UdpTestService::new(UdpTestKind::Ntp)
            .send_via_socks5(
                "127.0.0.1",
                socks_port,
                &loopback_ntp_target(),
                Duration::from_millis(500),
            )
            .await
            .expect("a verified first probe must survive a lost retry");

        assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");
    }

    #[tokio::test]
    async fn udp_test_times_out_when_no_probe_is_answered() {
        let socks_port = spawn_socks5_udp_relay(0, 0).await;

        let error = UdpTestService::new(UdpTestKind::Ntp)
            .send_via_socks5(
                "127.0.0.1",
                socks_port,
                &loopback_ntp_target(),
                Duration::from_millis(300),
            )
            .await
            .expect_err("an unanswered probe should time out");

        assert!(matches!(error, UdpTestError::Timeout(_)), "{error:?}");
    }

    /// The probe socket used to listen on every interface and accept datagrams from anybody, so
    /// a LAN host could race a forged reply into the measurement.
    #[tokio::test]
    async fn udp_channel_binds_to_the_control_address_and_ignores_foreign_senders() {
        let socks_port = spawn_socks5_udp_relay(0, 0).await;
        let mut channel = Socks5UdpChannel::new("127.0.0.1", socks_port);
        time::timeout(DEFAULT_TIMEOUT, channel.establish_udp_association())
            .await
            .expect("UDP association should finish before timeout")
            .expect("UDP association should succeed");

        let local = channel
            .udp_socket
            .as_ref()
            .expect("an established channel owns its UDP socket")
            .local_addr()
            .expect("probe socket address");
        assert!(!local.ip().is_unspecified(), "{local}");
        assert_eq!(local.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));

        let outsider = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("foreign sender should bind");
        let forged = build_socks5_udp_packet("example.com", 53, &[0x24; 48])
            .expect("forged reply should build");
        outsider
            .send_to(&forged, local)
            .await
            .expect("foreign sender should send");

        assert!(
            time::timeout(Duration::from_millis(200), channel.receive())
                .await
                .is_err(),
            "a datagram from anyone but the relay must never reach the probe"
        );
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
