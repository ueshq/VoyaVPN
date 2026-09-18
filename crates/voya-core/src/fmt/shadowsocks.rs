use super::*;

pub(super) fn parse(input: &str) -> Result<ProfileItem, ShareError> {
    let item = parse_shadowsocks_sip002(input)?;
    ensure_address_port("ss", &item)?;
    let ProfileProtocol::Shadowsocks {
        password, method, ..
    } = &item.protocol
    else {
        return Err(ShareError::WrongConfigType {
            protocol: "ss",
            actual: item.config_type(),
        });
    };
    if method.trim().is_empty() {
        return Err(ShareError::MissingField {
            protocol: "ss",
            field: "method",
        });
    }
    ensure_nonempty("ss", "password", password)?;
    Ok(item)
}

pub(super) fn export(item: &ProfileItem) -> Result<String, ShareError> {
    ensure_address_port("ss", item)?;
    let ProfileProtocol::Shadowsocks {
        password, method, ..
    } = &item.protocol
    else {
        return Err(ShareError::WrongConfigType {
            protocol: "ss",
            actual: item.config_type(),
        });
    };
    let method = nonempty_str(Some(method)).ok_or(ShareError::MissingField {
        protocol: "ss",
        field: "method",
    })?;
    ensure_nonempty("ss", "password", password)?;

    let mut query = Vec::new();
    if let Some(plugin) = shadowsocks_plugin_for(item) {
        query.push(("plugin".to_string(), url_encode(&plugin.render_share())));
    }

    // SIP022 forbids the base64 userinfo for the 2022 methods: their keys are
    // already base64 and clients expect `method:percent-encoded-key`.
    if is_shadowsocks_2022_method(method) {
        let user_info = format!("{}:{}", url_encode(method), url_encode(password));
        return Ok(format!(
            "{}{}",
            protocol_share(ConfigType::Shadowsocks),
            to_uri_without_scheme_preencoded_userinfo(
                item.address(),
                item.port(),
                &user_info,
                &query,
                &item.remarks,
            )
        ));
    }

    let user_info = base64_encode(&format!("{method}:{password}"), true);
    Ok(to_uri(
        ConfigType::Shadowsocks,
        item.address(),
        item.port(),
        &user_info,
        &query,
        &item.remarks,
    ))
}

fn is_shadowsocks_2022_method(method: &str) -> bool {
    method.trim().to_ascii_lowercase().starts_with("2022-")
}

pub fn parse_ss_sip008(input: &str) -> Result<Vec<ProfileItem>, ShareError> {
    let value: Value = serde_json::from_str(input).map_err(|error| ShareError::InvalidJson {
        protocol: "ss-sip008",
        reason: error.to_string(),
    })?;
    let servers = match value {
        Value::Array(items) => items,
        Value::Object(mut object) => match object.remove("servers") {
            Some(Value::Array(items)) => items,
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    if servers.is_empty() {
        return Err(ShareError::InvalidJson {
            protocol: "ss-sip008",
            reason: "missing servers".to_string(),
        });
    }

    let mut result = Vec::new();
    for server in servers {
        let object = server.as_object().ok_or_else(|| ShareError::InvalidJson {
            protocol: "ss-sip008",
            reason: "server entry must be an object".to_string(),
        })?;
        // SIP008 types `server_port` as a JSON number and allows numeric-looking
        // passwords, so both are read through the number-tolerant helpers.
        let item = ProfileItem {
            remarks: value_string(object, "remarks"),
            protocol: ProfileProtocol::Shadowsocks {
                server: ServerEndpoint {
                    address: value_string(object, "server"),
                    port: value_i32(object, "server_port").unwrap_or(0),
                },
                password: value_string(object, "password"),
                method: value_string(object, "method"),
                udp_over_tcp: false,
            },
            ..ProfileItem::default()
        };
        ensure_address_port("ss-sip008", &item)?;
        result.push(item);
    }
    Ok(result)
}

fn parse_shadowsocks_sip002(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri(input, "ss")?;
    let mut item = profile_from_uri(ConfigType::Shadowsocks, &parsed);
    let (method, password) = if parsed.user_info.contains(':') {
        let Some((method, password)) = parsed.user_info.split_once(':') else {
            return Err(ShareError::InvalidUri {
                protocol: "ss",
                reason: "invalid user info".to_string(),
            });
        };
        // `parsed.user_info` is already percent-decoded by `parse_uri`.
        (method.to_string(), password.to_string())
    } else {
        let decoded = base64_decode(&parsed.user_info, "ss")?;
        let Some((method, password)) = decoded.split_once(':') else {
            return Err(ShareError::InvalidUri {
                protocol: "ss",
                reason: "invalid encoded user info".to_string(),
            });
        };
        (method.to_string(), password.to_string())
    };
    if let ProfileProtocol::Shadowsocks {
        method: item_method,
        password: item_password,
        ..
    } = &mut item.protocol
    {
        *item_method = method;
        *item_password = password;
    }

    if let Some(plugin) = parsed.query.value("plugin") {
        parse_shadowsocks_plugin(plugin, &mut item)?;
    }

    Ok(item)
}

fn parse_shadowsocks_plugin(plugin: &str, item: &mut ProfileItem) -> Result<(), ShareError> {
    let plugin_parts = plugin
        .split(';')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if plugin_parts.is_empty() {
        return Err(ShareError::InvalidUri {
            protocol: "ss",
            reason: "empty plugin".to_string(),
        });
    }
    let plugin_name = if plugin_parts[0] == "simple-obfs" {
        "obfs-local"
    } else {
        plugin_parts[0]
    };

    if plugin_name == "obfs-local" {
        let obfs_mode = plugin_parts.iter().find(|part| part.starts_with("obfs="));
        let obfs_host = plugin_parts
            .iter()
            .find_map(|part| part.strip_prefix("obfs-host="));
        if obfs_mode.is_some_and(|part| part.contains("obfs=http"))
            && obfs_host.is_some_and(|host| !host.is_empty())
        {
            item.transport = Some(ProfileTransport::Tcp {
                header: Some(RAW_HEADER_HTTP.to_string()),
                host: obfs_host.map(str::to_string),
                path: None,
            });
        }
    } else if plugin_name == "v2ray-plugin" {
        let mode = plugin_parts
            .iter()
            .find_map(|part| part.strip_prefix("mode="))
            .unwrap_or("websocket");
        if mode == "websocket" {
            let mut host = None;
            let mut path = None;
            if let Some(parsed_host) = plugin_parts
                .iter()
                .find_map(|part| part.strip_prefix("host="))
            {
                let parsed_host = parsed_host.to_string();
                item.tls
                    .get_or_insert_with(TlsSettings::default)
                    .server_name = Some(parsed_host.clone());
                host = Some(parsed_host);
            }
            if let Some(parsed_path) = plugin_parts
                .iter()
                .find_map(|part| part.strip_prefix("path="))
            {
                path = Some(
                    parsed_path
                        .replace("\\=", "=")
                        .replace("\\,", ",")
                        .replace("\\\\", "\\"),
                );
            }
            item.transport = Some(ProfileTransport::Websocket { host, path });
        }
        if plugin_parts.contains(&"tls") {
            let tls = item.tls.get_or_insert_with(TlsSettings::default);
            tls.mode = TlsMode::Tls;
            if let Some(cert) = plugin_parts
                .iter()
                .find_map(|part| part.strip_prefix("certRaw="))
            {
                let cert = cert.replace("\\=", "=");
                tls.certificate_pem = Some(format!(
                    "-----BEGIN CERTIFICATE-----\n{cert}\n-----END CERTIFICATE-----"
                ));
            }
        }
        if let Some(mux) = plugin_parts
            .iter()
            .find_map(|part| part.strip_prefix("mux="))
            .and_then(|value| value.parse::<i32>().ok())
        {
            if mux > 0 {
                return Err(ShareError::InvalidUri {
                    protocol: "ss",
                    reason: "v2ray-plugin mux must be 0".to_string(),
                });
            }
        }
    }
    Ok(())
}
