use super::*;

pub(super) fn parse(input: &str) -> Result<ProfileItem, ShareError> {
    if input.contains('@') {
        // A standard URI that names a retired transport is a real answer, not
        // a reason to retry the input as a base64 payload.
        parse_vmess_standard(input).or_else(|error| match error {
            ShareError::UnsupportedTransport { .. } => Err(error),
            _ => parse_vmess_base64(input),
        })
    } else {
        parse_vmess_base64(input)
    }
}

pub(super) fn export(item: &ProfileItem) -> Result<String, ShareError> {
    ensure_address_port("vmess", item)?;
    ensure_nonempty("vmess", "password", item.password())?;
    let ProfileProtocol::Vmess { uuid, cipher, .. } = &item.protocol else {
        return Err(ShareError::WrongConfigType {
            protocol: "vmess",
            actual: item.config_type(),
        });
    };
    let network = item_network(item);
    let transport_type = match item.transport.as_ref() {
        Some(ProfileTransport::Tcp { header, .. }) => option_or(header, NONE),
        Some(ProfileTransport::Grpc { mode, .. }) => option_or(mode, NONE),
        _ => NONE.to_string(),
    };
    let transport_host = item
        .transport
        .as_ref()
        .and_then(ProfileTransport::host)
        .unwrap_or_default()
        .to_string();
    let transport_path = item
        .transport
        .as_ref()
        .and_then(ProfileTransport::path)
        .unwrap_or_default()
        .to_string();
    let tls = item.tls.as_ref();
    let vmess = json_object([
        ("v", Value::String("2".to_string())),
        ("ps", Value::String(item.remarks.trim().to_string())),
        ("add", Value::String(item.address().to_string())),
        ("port", Value::String(item.port().to_string())),
        ("id", Value::String(uuid.clone())),
        ("aid", Value::String("0".to_string())),
        (
            "scy",
            Value::String(
                nonempty_str(cipher.as_deref())
                    .unwrap_or(DEFAULT_SECURITY)
                    .to_string(),
            ),
        ),
        (
            "net",
            Value::String(if network == DEFAULT_NETWORK {
                RAW_NETWORK_ALIAS.to_string()
            } else {
                network.to_string()
            }),
        ),
        ("type", Value::String(transport_type)),
        ("host", Value::String(transport_host)),
        ("path", Value::String(transport_path)),
        ("tls", Value::String(item.stream_security().to_string())),
        (
            "sni",
            Value::String(
                tls.and_then(|tls| tls.server_name.clone())
                    .unwrap_or_default(),
            ),
        ),
        (
            "alpn",
            Value::String(tls.map(|tls| tls.alpn.join(",")).unwrap_or_default()),
        ),
    ]);

    let payload = serde_json::to_string(&vmess).map_err(|error| ShareError::InvalidJson {
        protocol: "vmess",
        reason: error.to_string(),
    })?;
    Ok(format!("vmess://{}", base64_encode(&payload, false)))
}

fn parse_vmess_standard(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri(input, "vmess")?;
    let mut item = profile_from_uri(ConfigType::VMess, &parsed);
    if let ProfileProtocol::Vmess { uuid, cipher, .. } = &mut item.protocol {
        *uuid = parsed.user_info;
        *cipher = Some(DEFAULT_SECURITY.to_string());
    }
    resolve_uri_query(&parsed.query, &mut item)?;
    ensure_address_port("vmess", &item)?;
    ensure_nonempty("vmess", "password", item.password())?;
    Ok(item)
}

fn parse_vmess_base64(input: &str) -> Result<ProfileItem, ShareError> {
    let payload = input
        .trim()
        .strip_prefix_ci("vmess://")
        .ok_or(ShareError::UnsupportedProtocol)?;
    let decoded = base64_decode(payload, "vmess")?;
    let value: Value = serde_json::from_str(&decoded).map_err(|error| ShareError::InvalidJson {
        protocol: "vmess",
        reason: error.to_string(),
    })?;
    let object = value.as_object().ok_or_else(|| ShareError::InvalidJson {
        protocol: "vmess",
        reason: "expected object".to_string(),
    })?;

    let address = value_string(object, "add");
    let port = value_i32(object, "port").unwrap_or(0);
    let security = value_string(object, "scy");
    let mut item = ProfileItem {
        remarks: value_string(object, "ps"),
        protocol: ProfileProtocol::Vmess {
            server: ServerEndpoint { address, port },
            uuid: value_string(object, "id"),
            cipher: Some(if security.is_empty() {
                DEFAULT_SECURITY.to_string()
            } else {
                security
            }),
        },
        ..ProfileItem::default()
    };

    let network = match value_string(object, "net").as_str() {
        "" | RAW_NETWORK_ALIAS => DEFAULT_NETWORK.to_string(),
        HTTP2_NETWORK_ALIAS => HTTP2_NETWORK.to_string(),
        network => network.to_string(),
    };
    if RETIRED_NETWORKS
        .iter()
        .any(|retired| retired.eq_ignore_ascii_case(&network))
    {
        return Err(ShareError::UnsupportedTransport { transport: network });
    }
    let vmess_type = value_string(object, "type");
    let host = value_string(object, "host");
    let path = value_string(object, "path");
    item.transport = Some(match network.as_str() {
        "ws" => ProfileTransport::Websocket {
            host: nonempty(host),
            path: nonempty(path),
        },
        "httpupgrade" => ProfileTransport::HttpUpgrade {
            host: nonempty(host),
            path: nonempty(path),
        },
        "grpc" => ProfileTransport::Grpc {
            authority: nonempty(host),
            service_name: nonempty(path),
            mode: nonempty(vmess_type),
        },
        HTTP2_NETWORK => ProfileTransport::Http2 {
            host: nonempty(host),
            path: nonempty(path),
        },
        QUIC_NETWORK => ProfileTransport::Quic {
            host: nonempty(host),
            path: nonempty(path),
        },
        _ => ProfileTransport::Tcp {
            header: nonempty(vmess_type),
            host: nonempty(host),
            path: nonempty(path),
        },
    });
    let tls_mode = value_string(object, "tls");
    let sni = nonempty(value_string(object, "sni"));
    let alpn = split_csv(&value_string(object, "alpn"));
    // `tls` alone decides whether the node is a TLS node: a stray `sni`/`alpn`
    // on a plaintext node must not silently enable TLS.
    item.tls = match tls_mode.as_str() {
        STREAM_SECURITY_TLS => Some(TlsMode::Tls),
        STREAM_SECURITY_REALITY => Some(TlsMode::Reality),
        _ => None,
    }
    .map(|mode| TlsSettings {
        mode,
        server_name: sni,
        alpn,
        ..TlsSettings::default()
    });

    ensure_address_port("vmess", &item)?;
    ensure_nonempty("vmess", "password", item.password())?;
    Ok(item)
}
