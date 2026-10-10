use std::time::{Duration, Instant};

use tokio::net::UdpSocket as TokioUdpSocket;

use super::*;

const EXTERNAL: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 7);
const TCP: u8 = 6;

type Requests = Arc<Mutex<Vec<Vec<u8>>>>;

/// A router on loopback: answers each datagram with what `answer` returns and
/// records what it was sent.
async fn router(
    answer: impl Fn(&[u8]) -> Option<Vec<u8>> + Send + 'static,
) -> (SocketAddr, Requests) {
    let socket = TokioUdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind the fake router");
    let address = socket.local_addr().expect("router address");
    let requests = Requests::default();
    let seen = Arc::clone(&requests);
    tokio::spawn(async move {
        let mut buffer = [0_u8; 1100];
        while let Ok((length, peer)) = socket.recv_from(&mut buffer).await {
            let request = buffer[..length].to_vec();
            let reply = answer(&request);
            seen.lock().expect("requests").push(request);
            if let Some(reply) = reply {
                let _ = socket.send_to(&reply, peer).await;
            }
        }
    });
    (address, requests)
}

/// A PCP server that grants every MAP, on the external port `offer` picks.
fn pcp_server(offer: fn(u16) -> u16) -> impl Fn(&[u8]) -> Option<Vec<u8>> {
    move |request| {
        if request.first() != Some(&2) {
            // NAT-PMP is not spoken here: PCP's "unsupported version".
            let mut refusal = vec![0_u8; 24];
            refusal[..4].copy_from_slice(&[2, request.get(1)? | 0x80, 0, 1]);
            return Some(refusal);
        }
        let mut reply = request.to_vec();
        reply[1] |= 0x80;
        reply[8..24].fill(0);
        let internal = u16::from_be_bytes([request[40], request[41]]);
        let deleting = request[4..8] == [0; 4];
        let external = if deleting { 0 } else { offer(internal) };
        reply[42..44].copy_from_slice(&external.to_be_bytes());
        reply[44..60].copy_from_slice(&EXTERNAL.to_ipv6_mapped().octets());
        Some(reply)
    }
}

/// A NAT-PMP server, as an older Apple router is.
fn nat_pmp_server(request: &[u8]) -> Option<Vec<u8>> {
    let epoch = [0, 0, 0, 9];
    match request {
        [0, 0] => Some([&[0, 0x80, 0, 0][..], &epoch, &EXTERNAL.octets()].concat()),
        [0, opcode @ (1 | 2), _, _, rest @ ..] => {
            Some([&[0, opcode | 0x80, 0, 0][..], &epoch, rest].concat())
        }
        [_, opcode, ..] => Some([&[0, opcode | 0x80, 0, 1][..], &epoch].concat()),
        _ => None,
    }
}

fn request(ports: &[u16]) -> PortMappingRequest {
    PortMappingRequest {
        ports: ports.to_vec(),
        lease_seconds: 3600,
        ..PortMappingRequest::default()
    }
}

fn tcp_maps(requests: &Requests) -> Vec<Vec<u8>> {
    requests
        .lock()
        .expect("requests")
        .iter()
        .filter(|request| request.len() == 60 && request[36] == TCP)
        .cloned()
        .collect()
}

#[tokio::test]
async fn pcp_maps_tcp_and_udp_and_renews_under_the_same_nonce() {
    let (server, requests) = router(pcp_server(|port| port)).await;
    let leases = Leases::default();

    let mapping = map_direct(&leases, server, &request(&[42_443, 42_444]))
        .await
        .expect("a PCP server answers");
    assert_eq!(mapping.method, PortMappingMethod::Pcp);
    assert_eq!(mapping.external_address, Some(IpAddr::V4(EXTERNAL)));
    assert_eq!(mapping.mapped, [42_443, 42_444]);
    assert!(mapping.refused.is_empty());
    // TCP and UDP for each port.
    assert_eq!(requests.lock().expect("requests").len(), 4);

    map_direct(&leases, server, &request(&[42_443, 42_444]))
        .await
        .expect("renewal");
    let maps = tcp_maps(&requests);
    assert_eq!(maps.len(), 4);
    assert_eq!(
        maps[0][24..36],
        maps[2][24..36],
        "the renewal reuses the nonce"
    );
    assert_ne!(maps[0][24..36], [0; 12]);
    assert_eq!(leases.lock().expect("leases").len(), 2);
}

#[tokio::test]
async fn unmapping_gives_every_forward_back() {
    let (server, requests) = router(pcp_server(|port| port)).await;
    let mapper = RouterPortMapper::default();
    map_direct(&mapper.leases, server, &request(&[42_443, 42_444]))
        .await
        .expect("mapped");
    requests.lock().expect("requests").clear();

    mapper.unmap(vec![42_443]).await;

    let sent = requests.lock().expect("requests").clone();
    assert_eq!(sent.len(), 2, "TCP and UDP");
    for deletion in &sent {
        assert_eq!(deletion[4..8], [0; 4], "lifetime zero");
        assert_eq!(deletion[40..42], 42_443_u16.to_be_bytes());
    }
    let kept = mapper.leases.lock().expect("leases").clone();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].port(), 42_444);
}

#[tokio::test]
async fn a_forward_on_another_external_port_is_refused_and_returned() {
    let (server, requests) = router(pcp_server(|port| port + 1)).await;
    let leases = Leases::default();

    let mapping = map_direct(&leases, server, &request(&[42_443]))
        .await
        .expect("a PCP server answers");
    assert!(mapping.mapped.is_empty());
    assert_eq!(mapping.refused.len(), 1);
    assert!(mapping.refused[0].1.contains("42444"), "{mapping:?}");
    assert!(leases.lock().expect("leases").is_empty());
    // The links name the node's own port, so the forward is of no use.
    let sent = requests.lock().expect("requests").clone();
    assert!(sent.iter().any(|request| request[4..8] == [0; 4]));
}

#[tokio::test]
async fn a_nat_pmp_router_is_asked_in_nat_pmp() {
    let (server, requests) = router(nat_pmp_server).await;
    let mapper = RouterPortMapper::default();

    let mapping = map_direct(&mapper.leases, server, &request(&[42_443]))
        .await
        .expect("a NAT-PMP server answers");
    assert_eq!(mapping.method, PortMappingMethod::NatPmp);
    assert_eq!(mapping.external_address, Some(IpAddr::V4(EXTERNAL)));
    assert_eq!(mapping.mapped, [42_443]);

    requests.lock().expect("requests").clear();
    mapper.unmap(vec![42_443]).await;
    let sent = requests.lock().expect("requests").clone();
    assert_eq!(sent.len(), 2);
    assert!(sent.iter().all(|deletion| deletion[8..12] == [0; 4]));
}

#[tokio::test]
async fn a_router_that_speaks_neither_is_left_to_upnp_quickly() {
    // A port nothing listens on: the OS answers with an ICMP error.
    let closed = {
        let socket = TokioUdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind");
        socket.local_addr().expect("address")
    };
    let started = Instant::now();
    let mapping = map_direct(&Leases::default(), closed, &request(&[42_443])).await;
    assert!(mapping.is_none());
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn nothing_is_asked_of_ipv6_without_a_router() {
    let leases = Leases::default();
    assert!(open_ipv6(&leases, &request(&[42_443]), pcp::SERVER_PORT)
        .await
        .is_empty());
}
