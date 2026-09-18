use std::sync::LazyLock;

use super::*;

pub(crate) fn fill_outbound_transport(
    outbound: &mut SingboxOutbound,
    context: &CoreConfigContext,
    node: &ProfileItem,
) {
    let user_agent = raw_http_user_agent(&context.app_config.core_basic_item.def_user_agent);
    let mut transport = SingboxTransport::default();

    match &node.transport {
        Some(ProfileTransport::Tcp { header, host, path }) => {
            if header.as_deref() == Some(RAW_HEADER_HTTP) {
                transport = http_transport(host.as_deref(), path.as_deref(), user_agent);
            }
        }
        Some(ProfileTransport::Websocket { host, path }) => {
            transport.r#type = Some("ws".to_string());
            let mut ws_path = path.clone().unwrap_or_default();
            let (path, early_data, early_header) = parse_ws_early_data(&ws_path);
            ws_path = path;
            transport.path = nonempty_string(Some(&ws_path));
            transport.max_early_data = early_data;
            transport.early_data_header_name = early_header;
            let host = first_list_value(host.as_deref());
            if !host.is_empty() || !user_agent.is_empty() {
                transport.headers = Some(SingboxHeaders {
                    host: nonempty_string(Some(&host)),
                    user_agent: nonempty_string(Some(&user_agent)),
                });
            }
        }
        Some(ProfileTransport::HttpUpgrade { host, path }) => {
            transport.r#type = Some("httpupgrade".to_string());
            transport.path = nonempty_string(path.as_deref());
            let host = first_list_value(host.as_deref());
            transport.host = nonempty_string(Some(&host)).map(Value::String);
            transport.headers = ua_only_headers(user_agent);
        }
        Some(ProfileTransport::Grpc { service_name, .. }) => {
            transport.r#type = Some("grpc".to_string());
            transport.service_name = Some(service_name.clone().unwrap_or_default());
            transport.idle_timeout = Some(GRPC_IDLE_TIMEOUT.to_string());
            transport.ping_timeout = Some(GRPC_PING_TIMEOUT.to_string());
            transport.permit_without_stream = Some(GRPC_PERMIT_WITHOUT_STREAM);
        }
        Some(ProfileTransport::Http2 { host, path }) => {
            transport = http_transport(host.as_deref(), path.as_deref(), user_agent);
        }
        Some(ProfileTransport::Quic { .. }) => {
            // sing-box's QUIC transport carries no host, path, or header options.
            transport.r#type = Some("quic".to_string());
        }
        _ => {}
    }

    if transport.r#type.is_some() {
        outbound.transport = Some(transport);
    }

    if node.config_type() == ConfigType::Shadowsocks {
        outbound.transport = None;
    }
}

/// sing-box's `http` transport, shared by HTTP/2 and the raw TCP HTTP header.
fn http_transport(host: Option<&str>, path: Option<&str>, user_agent: String) -> SingboxTransport {
    SingboxTransport {
        r#type: Some("http".to_string()),
        host: {
            let hosts = split_csv(host.unwrap_or_default());
            (!hosts.is_empty()).then(|| json!(hosts))
        },
        path: nonempty_string(path),
        headers: ua_only_headers(user_agent),
        ..SingboxTransport::default()
    }
}

/// A headers block carrying only the user agent, or none when it is empty.
fn ua_only_headers(user_agent: String) -> Option<SingboxHeaders> {
    (!user_agent.is_empty()).then_some(SingboxHeaders {
        host: None,
        user_agent: Some(user_agent),
    })
}

// Compiled once: every WebSocket node passes through here, and a macOS connect
// builds each node's outbound twice (main and latency probe).
static EARLY_DATA_REGEX: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"[?&]ed=(\d+)").ok());
static EARLY_HEADER_REGEX: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"[?&]eh=([^&]+)").ok());

fn parse_ws_early_data(path: &str) -> (String, Option<i32>, Option<String>) {
    let mut result_path = path.to_string();
    let mut early_data = None;
    let mut early_header = None;

    if let Some(ed_regex) = EARLY_DATA_REGEX.as_ref() {
        if let Some(captures) = ed_regex.captures(&result_path) {
            early_data = captures
                .get(1)
                .and_then(|value| value.as_str().parse::<i32>().ok());
            if early_data.is_some() {
                early_header = Some(USER_AGENT_HEADER.to_string());
                result_path = ed_regex.replace(&result_path, "").to_string();
                result_path = result_path.replace("?&", "?");
                if result_path.ends_with('?') {
                    result_path.pop();
                }
            }
        }
    }

    if let Some(eh_regex) = EARLY_HEADER_REGEX.as_ref() {
        if let Some(captures) = eh_regex.captures(&result_path) {
            if let Some(value) = captures.get(1) {
                early_header = percent_encoding::percent_decode_str(value.as_str())
                    .decode_utf8()
                    .ok()
                    .map(|value| value.to_string());
            }
        }
    }

    (result_path, early_data, early_header)
}

pub(crate) fn fill_outbound_tls(
    outbound: &mut SingboxOutbound,
    context: &CoreConfigContext,
    node: &ProfileItem,
) {
    if matches!(
        node.config_type(),
        ConfigType::Shadowsocks | ConfigType::SOCKS | ConfigType::WireGuard
    ) {
        return;
    }
    match node.tls.as_ref() {
        Some(domain_tls) => apply_outbound_tls(outbound, context, node, domain_tls),
        None if requires_implicit_tls(node.config_type()) => {
            apply_outbound_tls(outbound, context, node, &TlsSettings::default());
        }
        None => {}
    }
}

fn apply_outbound_tls(
    outbound: &mut SingboxOutbound,
    context: &CoreConfigContext,
    node: &ProfileItem,
    domain_tls: &TlsSettings,
) {
    let server_name =
        nonempty_string(domain_tls.server_name.as_deref()).or_else(|| transport_host_for_tls(node));
    let core = &context.app_config.core_basic_item;
    let split_hello = core.tls_fragment == crate::TlsFragmentMode::TlsHello;
    let mut tls = SingboxTls {
        enabled: true,
        server_name,
        insecure: Some(allow_insecure(context)),
        alpn: (!domain_tls.alpn.is_empty()).then(|| domain_tls.alpn.clone()),
        fragment: split_hello.then_some(true),
        fragment_fallback_delay: split_hello
            .then(|| format!("{}ms", core.fragment_fallback_delay_ms.max(1))),
        record_fragment: (core.tls_fragment == crate::TlsFragmentMode::Record).then_some(true),
        ech: parse_ech(&domain_tls.ech_config),
        ..SingboxTls::default()
    };

    // sing-box refuses a REALITY client without uTLS ("uTLS is required by
    // reality client"), so REALITY falls back to a fingerprint when the global
    // setting leaves it empty.
    let fingerprint = effective_fingerprint(context).or_else(|| {
        (domain_tls.mode == TlsMode::Reality).then(|| REALITY_FALLBACK_FINGERPRINT.to_string())
    });
    if let Some(fingerprint) = fingerprint {
        tls.utls = Some(SingboxUtls {
            enabled: true,
            fingerprint,
        });
    }

    if domain_tls.mode == TlsMode::Tls {
        let certs = domain_tls
            .certificate_pem
            .as_deref()
            .map(parse_pem_chain)
            .unwrap_or_default();
        if !certs.is_empty() {
            tls.certificate = Some(certs);
            tls.insecure = Some(false);
        }
    } else if domain_tls.mode == TlsMode::Reality {
        tls.reality = Some(SingboxReality {
            enabled: true,
            public_key: domain_tls.reality_public_key.clone().unwrap_or_default(),
            short_id: domain_tls.reality_short_id.clone().unwrap_or_default(),
        });
        tls.insecure = Some(false);
    }

    outbound.tls = Some(tls);
}

/// sing-box refuses these outbound types without a TLS block, so a profile that
/// reached the generator without TLS settings still needs an enabled section.
fn requires_implicit_tls(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::Hysteria2 | ConfigType::TUIC | ConfigType::Anytls | ConfigType::Naive
    )
}

fn parse_ech(ech_configs: &[String]) -> Option<SingboxEch> {
    let ech_configs = ech_configs
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if ech_configs.is_empty() {
        return None;
    }
    if !ech_configs.iter().any(|value| value.contains("://")) {
        let ech_config = ech_configs.join(",");
        return Some(SingboxEch {
            enabled: true,
            config: Some(vec![format!(
                "-----BEGIN ECH CONFIGS-----\n{ech_config}\n-----END ECH CONFIGS-----"
            )]),
            query_server_name: None,
        });
    }

    let query_server_name = ech_configs
        .iter()
        .find(|value| !value.contains("://"))
        .map(|value| value.split_once('+').map_or(*value, |(name, _)| name))
        .and_then(|value| nonempty_string(Some(value)));

    Some(SingboxEch {
        enabled: true,
        config: None,
        query_server_name,
    })
}
