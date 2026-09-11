use super::*;

pub fn validate_node(item: &ProfileItem, core_type: CoreType) -> NodeValidatorResult {
    let mut result = NodeValidatorResult::empty();

    if item.address().trim().is_empty() {
        result.push_error(ValidationCode::InvalidAddress);
    }
    if !(1..=65535).contains(&item.port()) {
        result.push_error(ValidationCode::InvalidPort);
    }

    let network = get_network(item);
    if core_type == CoreType::sing_box {
        if SINGBOX_UNSUPPORTED_TRANSPORTS.contains(&network.as_str()) {
            result.push_error(ValidationCode::UnsupportedNetwork {
                network: network.clone(),
            });
        }
        if !singbox_supports_config_type(item.config_type()) {
            result.push_error(ValidationCode::UnsupportedProtocol {
                protocol: protocol_label(item.config_type()),
            });
        }
        if !singbox_transport_supported_protocol(item.config_type()) && network != DEFAULT_NETWORK {
            result.push_error(ValidationCode::UnsupportedProtocolNetwork {
                protocol: protocol_label(item.config_type()),
                network: network.clone(),
            });
        }
        if item.config_type() == ConfigType::Shadowsocks
            && !SINGBOX_SHADOWSOCKS_ALLOWED_TRANSPORTS.contains(&network.as_str())
        {
            result.push_error(ValidationCode::UnsupportedShadowsocksNetwork { network });
        }
    }

    match &item.protocol {
        ProfileProtocol::Vmess { uuid, .. } => {
            if uuid.trim().is_empty() || !is_guid_like(uuid) {
                result.push_error(ValidationCode::InvalidPassword);
            }
        }
        ProfileProtocol::Vless { uuid, flow, .. } => {
            if uuid.trim().is_empty() || (!is_guid_like(uuid) && uuid.chars().count() > 30) {
                result.push_error(ValidationCode::InvalidPassword);
            }
            if !FLOWS.contains(&flow.as_deref().unwrap_or_default().trim()) {
                result.push_error(ValidationCode::InvalidFlow);
            }
        }
        ProfileProtocol::Shadowsocks {
            password, method, ..
        } => {
            if password.trim().is_empty() {
                result.push_error(ValidationCode::InvalidPassword);
            }
            if !SS_SECURITIES_IN_SINGBOX.contains(&method.trim()) {
                result.push_error(ValidationCode::InvalidShadowsocksMethod);
            }
        }
        _ => {}
    }

    if item.tls.as_ref().is_some_and(|tls| {
        tls.mode == TlsMode::Reality
            && tls
                .reality_public_key
                .as_deref()
                .is_none_or(|key| key.trim().is_empty())
    }) {
        result.push_error(ValidationCode::InvalidRealityPublicKey);
    }

    if let Some(final_mask) = item
        .tls
        .as_ref()
        .and_then(|tls| tls.final_mask.as_deref())
        .filter(|value| !value.trim().is_empty())
    {
        if serde_json::from_str::<Value>(final_mask).map_or(true, |value| !value.is_object()) {
            result.push_error(ValidationCode::InvalidFinalMask);
        }
    }

    result
}

/// The protocol name a rejection names, in the spelling the share links and
/// the UI already use. It is a proper noun, not prose, so it is interpolated
/// into the translated sentence rather than translated itself.
fn protocol_label(config_type: ConfigType) -> String {
    format!("{config_type:?}")
}

fn get_network(item: &ProfileItem) -> String {
    let network = item.network().trim();
    if network.is_empty() {
        DEFAULT_NETWORK.to_string()
    } else {
        network.to_string()
    }
}

fn singbox_transport_supported_protocol(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::VMess | ConfigType::VLESS | ConfigType::Trojan | ConfigType::Shadowsocks
    )
}

fn is_guid_like(value: &str) -> bool {
    let value = value
        .trim()
        .trim_start_matches(['{', '('])
        .trim_end_matches(['}', ')']);
    if value.len() == 32 {
        return value.chars().all(|ch| ch.is_ascii_hexdigit());
    }
    let expected = [8, 4, 4, 4, 12];
    let chunks = value.split('-').collect::<Vec<_>>();
    chunks.len() == expected.len()
        && chunks.iter().zip(expected).all(|(chunk, len)| {
            chunk.len() == len && chunk.chars().all(|ch| ch.is_ascii_hexdigit())
        })
}

pub(super) fn is_builtin_outbound(outbound_tag: Option<&str>) -> bool {
    outbound_tag.is_some_and(|tag| matches!(tag, PROXY_TAG | DIRECT_TAG | BLOCK_TAG))
}

pub(super) fn xhttp_download_settings_address(node: &ProfileItem) -> Option<String> {
    let ProfileTransport::Xhttp { extra, .. } = node.transport.as_ref()? else {
        return None;
    };
    let extra = extra.as_deref()?.trim();
    if extra.is_empty() {
        return None;
    }
    let value = serde_json::from_str::<Value>(extra).ok()?;
    value
        .get("downloadSettings")
        .and_then(|settings| settings.get("address"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|address| !address.is_empty())
        .map(str::to_string)
}

pub(super) fn push_domain_if_needed(protect_domain_list: &mut Vec<String>, candidate: &str) {
    let candidate = candidate.trim();
    if is_domain(candidate) && !protect_domain_list.iter().any(|domain| domain == candidate) {
        protect_domain_list.push(candidate.to_string());
    }
}

pub(super) fn merge_protect_domains(target: &mut Vec<String>, source: &[String]) {
    for domain in source {
        push_domain_if_needed(target, domain);
    }
}

#[must_use]
pub fn is_domain(candidate: &str) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty()
        || candidate.contains("://")
        || candidate.contains('/')
        || candidate.contains('\\')
        || candidate.parse::<IpAddr>().is_ok()
    {
        return false;
    }

    let blocked_ext = [
        "json", "txt", "xml", "cfg", "ini", "log", "yaml", "yml", "toml",
    ];
    if candidate
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .is_some_and(|extension| blocked_ext.contains(&extension.to_ascii_lowercase().as_str()))
    {
        return false;
    }

    candidate.split('.').all(|label| {
        !label.is_empty()
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    }) && candidate.chars().any(|ch| ch.is_ascii_alphabetic())
}
pub(super) fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TlsSettings;

    #[test]
    fn validate_node_rejection_table_covers_every_branch() {
        // Every rejection must identify the invalid node field precisely.
        let cases: &[(&str, ProfileItem, &[ValidationCode])] = &[
            ("valid vless", vless_node(), &[]),
            (
                "empty address",
                with_server(vless_node(), "", 443),
                &[ValidationCode::InvalidAddress],
            ),
            (
                "zero port",
                with_server(vless_node(), "node.example", 0),
                &[ValidationCode::InvalidPort],
            ),
            (
                "port above range",
                with_server(vless_node(), "node.example", 65536),
                &[ValidationCode::InvalidPort],
            ),
            (
                "kcp transport",
                with_transport(
                    vless_node(),
                    ProfileTransport::Kcp {
                        header: None,
                        seed: None,
                        mtu: None,
                    },
                ),
                &[ValidationCode::UnsupportedNetwork {
                    network: "kcp".to_string(),
                }],
            ),
            (
                "xhttp transport",
                with_transport(
                    vless_node(),
                    ProfileTransport::Xhttp {
                        host: None,
                        path: None,
                        mode: None,
                        extra: None,
                    },
                ),
                &[ValidationCode::UnsupportedNetwork {
                    network: "xhttp".to_string(),
                }],
            ),
            (
                "grpc on a protocol without transports",
                with_transport(
                    socks_node(),
                    ProfileTransport::Grpc {
                        authority: None,
                        service_name: None,
                        mode: None,
                    },
                ),
                &[ValidationCode::UnsupportedProtocolNetwork {
                    protocol: "SOCKS".to_string(),
                    network: "grpc".to_string(),
                }],
            ),
            (
                "shadowsocks over grpc",
                with_transport(
                    shadowsocks_node("aes-256-gcm"),
                    ProfileTransport::Grpc {
                        authority: None,
                        service_name: None,
                        mode: None,
                    },
                ),
                // Shadowsocks is in the transport-capable table, so only the
                // narrower Shadowsocks allow-list rejects this pair.
                &[ValidationCode::UnsupportedShadowsocksNetwork {
                    network: "grpc".to_string(),
                }],
            ),
            (
                "vmess with a non-uuid id",
                vmess_node("not-a-uuid"),
                &[ValidationCode::InvalidPassword],
            ),
            (
                "vless with an unknown flow",
                with_flow(vless_node(), "bogus-flow"),
                &[ValidationCode::InvalidFlow],
            ),
            (
                "shadowsocks with an unsupported cipher",
                shadowsocks_node("aes-256-eax"),
                &[ValidationCode::InvalidShadowsocksMethod],
            ),
            (
                "reality without a public key",
                with_tls(
                    vless_node(),
                    TlsSettings {
                        mode: TlsMode::Reality,
                        ..tls()
                    },
                ),
                &[ValidationCode::InvalidRealityPublicKey],
            ),
            (
                "final mask that is not a JSON object",
                with_tls(
                    vless_node(),
                    TlsSettings {
                        final_mask: Some("[1,2]".to_string()),
                        ..tls()
                    },
                ),
                &[ValidationCode::InvalidFinalMask],
            ),
        ];

        for (label, node, expected) in cases {
            let result = validate_node(node, CoreType::sing_box);
            assert_eq!(
                result.errors,
                expected
                    .iter()
                    .map(|code| ValidationMessage::new(code.clone()))
                    .collect::<Vec<_>>(),
                "unexpected validation errors for `{label}`"
            );
        }
    }

    #[test]
    fn validate_node_accepts_every_singbox_supported_protocol_over_raw() {
        for node in [
            vless_node(),
            vmess_node("00000000-0000-0000-0000-000000000000"),
            shadowsocks_node("chacha20-ietf-poly1305"),
            socks_node(),
        ] {
            let result = validate_node(&node, CoreType::sing_box);
            assert!(
                result.success(),
                "{:?} rejected: {:?}",
                node.protocol,
                result
            );
        }
    }

    #[test]
    fn is_guid_like_accepts_braced_and_bare_forms() {
        assert!(is_guid_like("00000000-0000-0000-0000-000000000000"));
        assert!(is_guid_like("{00000000-0000-0000-0000-000000000000}"));
        assert!(is_guid_like("(00000000-0000-0000-0000-000000000000)"));
        assert!(is_guid_like("00000000000000000000000000000000"));
        assert!(!is_guid_like("0000000000000000000000000000000g"));
        assert!(!is_guid_like("0000-0000-0000-0000-000000000000"));
        assert!(!is_guid_like(""));
    }

    #[test]
    fn is_domain_rejects_asset_file_names_and_addresses() {
        assert!(is_domain("a-b.example"));
        assert!(is_domain("node.example.com"));
        assert!(!is_domain("rules.json"));
        assert!(!is_domain("Rules.YAML"));
        assert!(!is_domain("1.2.3.4"));
        assert!(!is_domain("2606:4700::1111"));
        assert!(!is_domain("https://example.com"));
        assert!(!is_domain("example.com/path"));
        assert!(!is_domain(""));
        assert!(!is_domain("1234"));
    }

    fn tls() -> TlsSettings {
        TlsSettings {
            mode: TlsMode::Tls,
            server_name: None,
            alpn: Vec::new(),
            reality_public_key: None,
            reality_short_id: None,
            reality_spider_x: None,
            mldsa65_verify: None,
            certificate_pem: None,
            certificate_sha256: Vec::new(),
            ech_config: Vec::new(),
            final_mask: None,
        }
    }

    fn raw_transport() -> ProfileTransport {
        ProfileTransport::Tcp {
            header: None,
            host: None,
            path: None,
        }
    }

    fn endpoint(address: &str, port: i32) -> ServerEndpoint {
        ServerEndpoint {
            address: address.to_string(),
            port,
        }
    }

    fn vless_node() -> ProfileItem {
        ProfileItem {
            index_id: "vless".to_string(),
            remarks: "VLESS".to_string(),
            protocol: ProfileProtocol::Vless {
                server: endpoint("node.example", 443),
                uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                flow: Some(String::new()),
                encryption: Some("none".to_string()),
            },
            transport: Some(raw_transport()),
            ..ProfileItem::default()
        }
    }

    fn vmess_node(uuid: &str) -> ProfileItem {
        ProfileItem {
            index_id: "vmess".to_string(),
            remarks: "VMess".to_string(),
            protocol: ProfileProtocol::Vmess {
                server: endpoint("node.example", 443),
                uuid: uuid.to_string(),
                cipher: None,
            },
            transport: Some(raw_transport()),
            ..ProfileItem::default()
        }
    }

    fn shadowsocks_node(method: &str) -> ProfileItem {
        ProfileItem {
            index_id: "ss".to_string(),
            remarks: "Shadowsocks".to_string(),
            protocol: ProfileProtocol::Shadowsocks {
                server: endpoint("node.example", 443),
                password: "secret".to_string(),
                method: method.to_string(),
                udp_over_tcp: false,
            },
            transport: Some(raw_transport()),
            ..ProfileItem::default()
        }
    }

    fn socks_node() -> ProfileItem {
        ProfileItem {
            index_id: "socks".to_string(),
            remarks: "SOCKS".to_string(),
            protocol: ProfileProtocol::Socks {
                server: endpoint("node.example", 1080),
                username: String::new(),
                password: String::new(),
            },
            transport: Some(raw_transport()),
            ..ProfileItem::default()
        }
    }

    fn with_server(mut node: ProfileItem, address: &str, port: i32) -> ProfileItem {
        if let ProfileProtocol::Vless { server, .. } = &mut node.protocol {
            *server = endpoint(address, port);
        }
        node
    }

    fn with_transport(mut node: ProfileItem, transport: ProfileTransport) -> ProfileItem {
        node.transport = Some(transport);
        node
    }

    fn with_flow(mut node: ProfileItem, flow: &str) -> ProfileItem {
        if let ProfileProtocol::Vless { flow: value, .. } = &mut node.protocol {
            *value = Some(flow.to_string());
        }
        node
    }

    fn with_tls(mut node: ProfileItem, tls: TlsSettings) -> ProfileItem {
        node.tls = Some(tls);
        node
    }
}
