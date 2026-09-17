use super::*;

pub(super) fn json_object<const N: usize>(items: [(&str, Value); N]) -> Value {
    Value::Object(
        items
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Map<_, _>>(),
    )
}

pub(super) fn value_string(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

pub(super) fn value_i32(object: &Map<String, Value>, key: &str) -> Option<i32> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_i64().and_then(|value| i32::try_from(value).ok()),
        Value::String(text) => text.parse().ok(),
        _ => None,
    })
}

/// Cloudflare WARP `.conf` exports omit the port on some peers; 2408 is the
/// WARP endpoint port and keeps those imports working.
pub(super) const WIREGUARD_DEFAULT_ENDPOINT_PORT: i32 = 2408;

pub(super) fn parse_wireguard_endpoint(endpoint: &str) -> Option<(String, i32)> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return None;
    }
    if let Some(rest) = endpoint.strip_prefix('[') {
        let close_index = rest.find(']')?;
        let address = rest[..close_index].trim().to_string();
        if address.is_empty() {
            return None;
        }
        let after = rest[(close_index + 1)..].trim();
        let port = after
            .strip_prefix(':')
            .and_then(|text| text.trim().parse::<i32>().ok())
            .filter(|port| (1..=65535).contains(port))
            .unwrap_or(WIREGUARD_DEFAULT_ENDPOINT_PORT);
        return Some((address, port));
    }

    if let Some((address, port_text)) = endpoint.rsplit_once(':') {
        // An unbracketed endpoint carries exactly one `host:port` colon. More
        // colons mean a bare IPv6 literal, which must not be split into a host
        // and a port.
        let address = address.trim();
        if address.is_empty() || address.contains(':') {
            return None;
        }
        let port = port_text
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|port| (1..=65535).contains(port))?;
        return Some((address.to_string(), port));
    }
    Some((endpoint.to_string(), WIREGUARD_DEFAULT_ENDPOINT_PORT))
}

pub(super) fn parse_positive_i32(value: &str) -> Option<i32> {
    value.parse::<i32>().ok().filter(|value| *value > 0)
}

pub(super) fn ensure_address_port(
    protocol: &'static str,
    item: &ProfileItem,
) -> Result<(), ShareError> {
    ensure_nonempty(protocol, "address", item.address())?;
    if !valid_host(item.address()) {
        return Err(ShareError::InvalidUri {
            protocol,
            reason: format!("invalid host {}", item.address()),
        });
    }
    if !(1..=65535).contains(&item.port()) {
        return Err(ShareError::InvalidPort {
            protocol,
            port: item.port().to_string(),
        });
    }
    Ok(())
}

pub(super) fn ensure_nonempty(
    protocol: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), ShareError> {
    if value.is_empty() {
        Err(ShareError::MissingField { protocol, field })
    } else {
        Ok(())
    }
}

pub(super) fn valid_host(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty()
        || host.len() > 253
        || host.chars().any(|ch| {
            ch.is_control()
                || ch.is_whitespace()
                || matches!(ch, '=' | '/' | '?' | '#' | '@' | '\\')
        })
    {
        return false;
    }

    let host_for_ip = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    if host_for_ip.parse::<IpAddr>().is_ok() {
        return true;
    }

    if host.contains(':') {
        return false;
    }

    let domain = host.trim_end_matches('.');
    !domain.is_empty() && domain.split('.').all(is_dns_label)
}

pub(super) fn protocol_share(config_type: ConfigType) -> &'static str {
    match config_type {
        ConfigType::VMess => "vmess://",
        ConfigType::Shadowsocks => "ss://",
        ConfigType::SOCKS => "socks://",
        ConfigType::VLESS => "vless://",
        ConfigType::Trojan => "trojan://",
        ConfigType::Hysteria2 => HYSTERIA2_DEFAULT_SCHEME,
        ConfigType::TUIC => "tuic://",
        ConfigType::WireGuard => "wireguard://",
        ConfigType::Anytls => "anytls://",
        ConfigType::Naive => "naive://",
        // Not shareable: export rejects HTTP nodes with WrongConfigType.
        ConfigType::HTTP => "",
    }
}

pub(super) fn item_network(item: &ProfileItem) -> &str {
    if item.network().is_empty() || !NETWORKS.contains(&item.network()) {
        DEFAULT_NETWORK
    } else {
        item.network().trim()
    }
}

pub(super) fn option_or(value: &Option<String>, default_value: &str) -> String {
    nonempty_str(value.as_deref())
        .unwrap_or(default_value)
        .to_string()
}

/// An is-empty check on an owned value — unlike the trimming
/// [`crate::text`] helpers, wire-format fields keep their whitespace.
pub(super) fn nonempty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub(super) fn push_encoded_opt(query: &mut QueryPairs, key: &str, value: &Option<String>) {
    if let Some(value) = nonempty_str(value.as_deref()) {
        query.push((key.to_string(), url_encode(value)));
    }
}

pub(super) fn push_encoded_str(query: &mut QueryPairs, key: &str, value: &str) {
    if !value.is_empty() {
        query.push((key.to_string(), url_encode(value)));
    }
}

pub(super) fn format_query(query: &[(String, String)]) -> String {
    if query.is_empty() {
        String::new()
    } else {
        format!(
            "?{}",
            query
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&")
        )
    }
}

pub(super) fn ipv6_host(address: &str) -> String {
    if address.starts_with('[') && address.ends_with(']') {
        return address.to_string();
    }
    if address
        .parse::<IpAddr>()
        .is_ok_and(|address| address.is_ipv6())
    {
        format!("[{address}]")
    } else {
        address.to_string()
    }
}

pub(super) fn base64_encode(input: &str, remove_padding: bool) -> String {
    let mut encoded = STANDARD.encode(input.as_bytes());
    if remove_padding {
        encoded = encoded.trim_end_matches('=').to_string();
    }
    encoded
}

pub(super) fn base64_decode(input: &str, protocol: &'static str) -> Result<String, ShareError> {
    if input.trim().len() > MAX_BASE64_DECODE_INPUT {
        return Err(ShareError::InvalidBase64 { protocol });
    }
    decode_base64_text(input).ok_or(ShareError::InvalidBase64 { protocol })
}

pub(super) fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(*byte))
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

pub(super) fn url_decode(input: &str) -> String {
    percent_decode_str(input).decode_utf8_lossy().into_owned()
}

pub(super) fn starts_with_ci(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}
pub(super) trait StripPrefixCi {
    fn strip_prefix_ci<'a>(&'a self, prefix: &str) -> Option<&'a str>;
}

impl StripPrefixCi for str {
    fn strip_prefix_ci<'a>(&'a self, prefix: &str) -> Option<&'a str> {
        if starts_with_ci(self, prefix) {
            self.get(prefix.len()..)
        } else {
            None
        }
    }
}
