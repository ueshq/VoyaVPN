use super::*;

const NONCE: Nonce = [7; NONCE_LENGTH];

fn client() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))
}

/// The server's answer to `request`: the request echoed with the response
/// bit, a result code and an assigned external address.
fn response(request: &[u8], result: u8, external: IpAddr) -> Vec<u8> {
    let mut bytes = request.to_vec();
    bytes[1] |= RESPONSE_BIT;
    bytes[3] = result;
    bytes[8..24].fill(0);
    bytes[44..60].copy_from_slice(&ipv6_bytes(external));
    bytes
}

#[test]
fn a_map_request_follows_rfc_6887() {
    let packet = pcp_map_request(client(), &NONCE, IP_PROTOCOL_TCP, 42_443, 3600);
    assert_eq!(packet[..4], [2, 1, 0, 0]);
    assert_eq!(packet[4..8], 3600_u32.to_be_bytes());
    // The client address travels IPv4-mapped.
    assert_eq!(packet[8..18], [0; 10]);
    assert_eq!(packet[18..24], [0xff, 0xff, 192, 168, 1, 20]);
    assert_eq!(packet[24..36], NONCE);
    assert_eq!(packet[36..40], [6, 0, 0, 0]);
    assert_eq!(packet[40..42], 42_443_u16.to_be_bytes());
    assert_eq!(packet[42..44], 42_443_u16.to_be_bytes());
    assert_eq!(
        packet[44..60],
        Ipv4Addr::UNSPECIFIED.to_ipv6_mapped().octets()
    );

    let delete = pcp_map_request(client(), &NONCE, IP_PROTOCOL_TCP, 42_443, 0);
    assert_eq!(delete[4..8], [0; 4]);
    assert_eq!(delete[42..44], [0; 2], "a deletion names no external port");

    let over_ipv6 = pcp_map_request(
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        &NONCE,
        IP_PROTOCOL_UDP,
        1,
        60,
    );
    assert_eq!(over_ipv6[8..24], Ipv6Addr::LOCALHOST.octets());
    assert_eq!(over_ipv6[44..60], [0; 16]);
}

#[test]
fn a_map_response_is_matched_to_its_request() {
    let request = pcp_map_request(client(), &NONCE, IP_PROTOCOL_TCP, 42_443, 3600);
    let external = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));
    let granted = response(&request, 0, external);
    assert_eq!(
        parse_pcp_map_response(&granted, &NONCE, IP_PROTOCOL_TCP, 42_443),
        Some(PcpReply::Mapped {
            external_address: external,
            external_port: 42_443
        })
    );
    // Another request's answer is not this one's.
    assert_eq!(
        parse_pcp_map_response(&granted, &[8; NONCE_LENGTH], IP_PROTOCOL_TCP, 42_443),
        None
    );
    assert_eq!(
        parse_pcp_map_response(&granted, &NONCE, IP_PROTOCOL_UDP, 42_443),
        None
    );
    assert_eq!(
        parse_pcp_map_response(&granted, &NONCE, IP_PROTOCOL_TCP, 42_444),
        None
    );
    // The request itself, looped back, is not an answer.
    assert_eq!(
        parse_pcp_map_response(&request, &NONCE, IP_PROTOCOL_TCP, 42_443),
        None
    );
    assert_eq!(
        parse_pcp_map_response(
            &response(&request, 2, external),
            &NONCE,
            IP_PROTOCOL_TCP,
            42_443
        ),
        Some(PcpReply::Refused(2))
    );
    assert_eq!(
        parse_pcp_map_response(&[], &NONCE, IP_PROTOCOL_TCP, 1),
        None
    );
    assert_eq!(
        parse_pcp_map_response(&[2, 0x81], &NONCE, IP_PROTOCOL_TCP, 1),
        None
    );
}

#[test]
fn an_ipv6_pinhole_answers_with_the_device_address() {
    let device = IpAddr::V6("2001:db8::20".parse().expect("address"));
    let request = pcp_map_request(device, &NONCE, IP_PROTOCOL_TCP, 42_443, 3600);
    assert_eq!(
        parse_pcp_map_response(
            &response(&request, 0, device),
            &NONCE,
            IP_PROTOCOL_TCP,
            42_443
        ),
        Some(PcpReply::Mapped {
            external_address: device,
            external_port: 42_443
        })
    );
}

#[test]
fn either_protocol_recognises_the_other_refusing_its_version() {
    // A NAT-PMP server's answer to a PCP request: version 0, result 1.
    assert_eq!(
        parse_pcp_map_response(&[0, 0x81, 0, 1, 0, 0, 0, 9], &NONCE, IP_PROTOCOL_TCP, 1),
        Some(PcpReply::UnsupportedVersion)
    );
    // A PCP server's answer to a NAT-PMP request: version 2, result 1.
    let mut pcp_refusal = [0_u8; 24];
    pcp_refusal[..4].copy_from_slice(&[2, 0x80, 0, 1]);
    assert_eq!(
        parse_nat_pmp_response(&pcp_refusal, NAT_PMP_OPCODE_EXTERNAL_ADDRESS),
        Some((RESULT_UNSUPPORTED_VERSION, &[][..]))
    );
}

#[test]
fn nat_pmp_packets_follow_rfc_6886() {
    let request = nat_pmp_map_request(NAT_PMP_OPCODE_TCP, 42_443, 3600);
    assert_eq!(request[..4], [0, 2, 0, 0]);
    assert_eq!(request[4..6], 42_443_u16.to_be_bytes());
    assert_eq!(request[6..8], 42_443_u16.to_be_bytes());
    assert_eq!(request[8..12], 3600_u32.to_be_bytes());

    let mut granted = vec![0, 0x82, 0, 0, 0, 0, 0, 9];
    granted.extend_from_slice(&42_443_u16.to_be_bytes());
    granted.extend_from_slice(&42_443_u16.to_be_bytes());
    granted.extend_from_slice(&3600_u32.to_be_bytes());
    let (result, body) = parse_nat_pmp_response(&granted, NAT_PMP_OPCODE_TCP).expect("response");
    assert_eq!(result, RESULT_SUCCESS);
    assert_eq!(body[2..4], 42_443_u16.to_be_bytes());
    // The UDP request's answer carries another opcode.
    assert_eq!(parse_nat_pmp_response(&granted, NAT_PMP_OPCODE_UDP), None);
}
