use super::*;

#[derive(Debug, Clone)]
pub(super) struct ParsedUri {
    pub(super) scheme: String,
    pub(super) address: String,
    pub(super) port: i32,
    pub(super) remarks: String,
    pub(super) user_info: String,
    pub(super) query: Query,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Query(Vec<(String, String)>);

impl Query {
    pub(super) fn parse(raw_query: Option<&str>) -> Self {
        let Some(raw_query) = raw_query else {
            return Self::default();
        };
        let mut query = Self::default();
        for part in raw_query.split('&').filter(|part| !part.is_empty()) {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            let key = url_decode(key);
            if query.contains_key(&key) {
                continue;
            }
            query.0.push((key, url_decode(value)));
        }
        query
    }

    pub(super) fn contains_key(&self, wanted: &str) -> bool {
        self.0
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(wanted))
    }

    pub(super) fn value(&self, wanted: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(wanted))
            .map(|(_, value)| value.as_str())
    }

    /// Returns the stored value for `wanted`, which [`Query::parse`] already
    /// percent-decoded once. Decoding again here would corrupt every value that
    /// legitimately contains a `%XX` sequence.
    pub(super) fn value_or(&self, wanted: &str, default_value: &str) -> String {
        self.value(wanted).unwrap_or(default_value).to_string()
    }
}

pub(super) type QueryPairs = Vec<(String, String)>;

pub(super) fn parse_uri(input: &str, protocol: &'static str) -> Result<ParsedUri, ShareError> {
    parse_uri_with_schemes(input, protocol, &[protocol])
}

pub(super) fn parse_uri_with_schemes(
    input: &str,
    protocol: &'static str,
    schemes: &[&str],
) -> Result<ParsedUri, ShareError> {
    let url = Url::parse(input).map_err(|error| ShareError::InvalidUri {
        protocol,
        reason: error.to_string(),
    })?;
    if !schemes
        .iter()
        .any(|scheme| url.scheme().eq_ignore_ascii_case(scheme))
    {
        return Err(ShareError::UnsupportedProtocol);
    }
    let address = strip_host_brackets(url.host_str().unwrap_or(""));
    if address.is_empty() {
        return Err(ShareError::MissingField {
            protocol,
            field: "host",
        });
    }
    let port = url.port().ok_or(ShareError::MissingField {
        protocol,
        field: "port",
    })?;
    if port == 0 {
        return Err(ShareError::InvalidPort {
            protocol,
            port: port.to_string(),
        });
    }
    let username = url.username();
    let user_info = if let Some(password) = url.password() {
        format!("{}:{}", url_decode(username), url_decode(password))
    } else {
        url_decode(username)
    };

    Ok(ParsedUri {
        scheme: url.scheme().to_string(),
        address,
        port: i32::from(port),
        remarks: url.fragment().map(url_decode).unwrap_or_default(),
        user_info,
        query: Query::parse(url.query()),
    })
}

pub(super) fn profile_from_uri(config_type: ConfigType, parsed: &ParsedUri) -> ProfileItem {
    ProfileItem {
        remarks: parsed.remarks.clone(),
        protocol: ProfileProtocol::empty(
            config_type,
            ServerEndpoint {
                address: parsed.address.clone(),
                port: parsed.port,
            },
        ),
        ..ProfileItem::default()
    }
}

pub(super) fn to_uri(
    config_type: ConfigType,
    address: &str,
    port: i32,
    user_info: &str,
    query: &[(String, String)],
    remark: &str,
) -> String {
    format!(
        "{}{}",
        protocol_share(config_type),
        to_uri_without_scheme_preencoded_userinfo(
            address,
            port,
            &url_encode(user_info),
            query,
            remark
        )
    )
}

pub(super) fn to_uri_without_scheme_preencoded_userinfo(
    address: &str,
    port: i32,
    user_info: &str,
    query: &[(String, String)],
    remark: &str,
) -> String {
    let query = format_query(query);
    let remark = if remark.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(remark))
    };
    // Only anonymous SOCKS links reach here without credentials; they must not
    // carry a dangling `@`.
    let user_info = if user_info.is_empty() {
        String::new()
    } else {
        format!("{user_info}@")
    };
    format!("{user_info}{}:{port}{query}{remark}", ipv6_host(address))
}

pub(super) fn to_uri_query(
    item: &ProfileItem,
    security_default: Option<&str>,
    query: &mut QueryPairs,
) {
    if !item.stream_security().is_empty() {
        query.push(("security".to_string(), item.stream_security().to_string()));
    } else if let Some(default_value) = security_default {
        query.push(("security".to_string(), default_value.to_string()));
    }
    if let Some(tls) = &item.tls {
        push_encoded_str(query, "sni", tls.server_name.as_deref().unwrap_or_default());
        push_encoded_str(
            query,
            "pbk",
            tls.reality_public_key.as_deref().unwrap_or_default(),
        );
        push_encoded_str(
            query,
            "sid",
            tls.reality_short_id.as_deref().unwrap_or_default(),
        );
        if tls.mode == TlsMode::Tls {
            push_encoded_str(query, "alpn", &tls.alpn.join(","));
        }
        push_encoded_str(query, "ech", &tls.ech_config.join(","));
    }

    let network = item_network(item);
    query.push((
        "type".to_string(),
        if network == DEFAULT_NETWORK {
            RAW_NETWORK_ALIAS.to_string()
        } else {
            network.to_string()
        },
    ));

    match network {
        "raw" => {
            let (header, host, path) = match item.transport.as_ref() {
                Some(ProfileTransport::Tcp { header, host, path }) => (header, host, path),
                _ => return,
            };
            query.push((
                "headerType".to_string(),
                nonempty_str(header.as_deref()).unwrap_or(NONE).to_string(),
            ));
            push_encoded_opt(query, "host", host);
            push_encoded_opt(query, "path", path);
        }
        "ws" | "httpupgrade" | HTTP2_NETWORK | QUIC_NETWORK => {
            let (host, path) = match item.transport.as_ref() {
                Some(ProfileTransport::Websocket { host, path })
                | Some(ProfileTransport::HttpUpgrade { host, path })
                | Some(ProfileTransport::Http2 { host, path })
                | Some(ProfileTransport::Quic { host, path }) => (host, path),
                _ => return,
            };
            push_encoded_opt(query, "host", host);
            push_encoded_opt(query, "path", path);
        }
        "grpc" => {
            let (authority, service_name, mode) = match item.transport.as_ref() {
                Some(ProfileTransport::Grpc {
                    authority,
                    service_name,
                    mode,
                }) => (authority, service_name, mode),
                _ => return,
            };
            if nonempty_str(service_name.as_deref()).is_none() {
                return;
            }
            query.push((
                "authority".to_string(),
                url_encode(authority.as_deref().unwrap_or("")),
            ));
            query.push((
                "serviceName".to_string(),
                url_encode(service_name.as_deref().unwrap_or("")),
            ));
            if let Some(mode) = nonempty_str(mode.as_deref()) {
                if mode == GRPC_GUN_MODE || mode == GRPC_MULTI_MODE {
                    query.push(("mode".to_string(), url_encode(mode)));
                }
            }
        }
        _ => {}
    }
}

pub(super) fn to_uri_query_lite(item: &ProfileItem, query: &mut QueryPairs) {
    if let Some(tls) = &item.tls {
        push_encoded_str(query, "sni", tls.server_name.as_deref().unwrap_or_default());
        push_encoded_str(query, "alpn", &tls.alpn.join(","));
    }
}

/// Resolves the shared TLS and transport query parameters of a share link.
///
/// TLS is derived from `security=` alone: a stray `sni`/`alpn` on a plaintext
/// link must not silently enable TLS.
pub(super) fn resolve_uri_query(query: &Query, item: &mut ProfileItem) -> Result<(), ShareError> {
    resolve_query_into(query, item, false)
}

/// Same as [`resolve_uri_query`], for protocols that are TLS-only by
/// definition (Hysteria2/TUIC/AnyTLS/Naive).
///
/// Links for those protocols routinely omit `security=` and every TLS-adjacent
/// parameter (`hysteria2://pass@203.0.113.5:443/?insecure=1`), so TLS stays
/// enabled unless the link explicitly asks for REALITY.
pub(super) fn resolve_uri_query_tls_only(
    query: &Query,
    item: &mut ProfileItem,
) -> Result<(), ShareError> {
    resolve_query_into(query, item, true)
}

fn resolve_query_into(
    query: &Query,
    item: &mut ProfileItem,
    tls_only_protocol: bool,
) -> Result<(), ShareError> {
    item.transport = Some(resolve_query_transport(query)?);
    item.tls = resolve_query_tls(query, tls_only_protocol);
    Ok(())
}

fn resolve_query_tls(query: &Query, tls_only_protocol: bool) -> Option<TlsSettings> {
    let mode = match query.value_or("security", "").as_str() {
        STREAM_SECURITY_TLS => TlsMode::Tls,
        STREAM_SECURITY_REALITY => TlsMode::Reality,
        _ if tls_only_protocol => TlsMode::Tls,
        _ => return None,
    };
    Some(TlsSettings {
        mode,
        server_name: nonempty(query.value_or("sni", "")),
        alpn: split_csv(&query.value_or("alpn", "")),
        reality_public_key: nonempty(query.value_or("pbk", "")),
        reality_short_id: nonempty(query.value_or("sid", "")),
        certificate_pem: None,
        ech_config: split_csv(&query.value_or("ech", "")),
    })
}

fn resolve_query_transport(query: &Query) -> Result<ProfileTransport, ShareError> {
    let mut network = query.value_or("type", DEFAULT_NETWORK);
    if RETIRED_NETWORKS
        .iter()
        .any(|retired| retired.eq_ignore_ascii_case(&network))
    {
        return Err(ShareError::UnsupportedTransport { transport: network });
    }
    if network == RAW_NETWORK_ALIAS {
        network = DEFAULT_NETWORK.to_string();
    }
    if network == HTTP2_NETWORK_ALIAS {
        network = HTTP2_NETWORK.to_string();
    }
    if !NETWORKS.contains(&network.as_str()) {
        network = DEFAULT_NETWORK.to_string();
    }
    Ok(match network.as_str() {
        "ws" => ProfileTransport::Websocket {
            host: nonempty(query.value_or("host", "")),
            path: nonempty(query.value_or("path", "/")),
        },
        "httpupgrade" => ProfileTransport::HttpUpgrade {
            host: nonempty(query.value_or("host", "")),
            path: nonempty(query.value_or("path", "/")),
        },
        HTTP2_NETWORK => ProfileTransport::Http2 {
            host: nonempty(query.value_or("host", "")),
            path: nonempty(query.value_or("path", "/")),
        },
        QUIC_NETWORK => ProfileTransport::Quic {
            host: nonempty(query.value_or("host", "")),
            path: nonempty(query.value_or("path", "")),
        },
        "grpc" => ProfileTransport::Grpc {
            authority: nonempty(query.value_or("authority", "")),
            service_name: nonempty(query.value_or("serviceName", "")),
            mode: nonempty(query.value_or("mode", GRPC_GUN_MODE)),
        },
        _ => ProfileTransport::Tcp {
            header: nonempty(query.value_or("headerType", NONE)),
            host: nonempty(query.value_or("host", "")),
            path: nonempty(query.value_or("path", "")),
        },
    })
}
