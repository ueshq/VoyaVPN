use crate::{AppConfig, ConfigType, InboundProtocol, ProtocolExtraItem, DEFAULT_LOCAL_PORT};

pub(crate) const DEFAULT_SECURITY: &str = "auto";
pub(crate) const DEFAULT_NETWORK: &str = "raw";
pub(crate) const WIREGUARD_DEFAULT_ADDRESS: &str = "172.16.0.2/32";
pub(crate) const WIREGUARD_DEFAULT_ALLOWED_IPS: &[&str] = &["0.0.0.0/0", "::/0"];
pub(crate) const WIREGUARD_DEFAULT_MTU: i32 = 1280;
pub(crate) const WIREGUARD_RESERVED_LEN: usize = 3;

pub(crate) fn protocol_name(config_type: ConfigType) -> &'static str {
    match config_type {
        ConfigType::VMess => "vmess",
        ConfigType::Shadowsocks => "shadowsocks",
        ConfigType::SOCKS => "socks",
        ConfigType::HTTP => "http",
        ConfigType::VLESS => "vless",
        ConfigType::Trojan => "trojan",
        ConfigType::Hysteria2 => "hysteria2",
        ConfigType::TUIC => "tuic",
        ConfigType::WireGuard => "wireguard",
        ConfigType::Anytls => "anytls",
        ConfigType::Naive => "naive",
        ConfigType::Custom | ConfigType::PolicyGroup | ConfigType::ProxyChain => "vmess",
    }
}

pub(crate) fn raw_http_user_agent(user_agent: &str) -> String {
    match user_agent {
        "chrome" => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36".to_string(),
        "firefox" => "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:90.0) Gecko/20100101 Firefox/90.0".to_string(),
        "safari" => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.1 Safari/605.1.15".to_string(),
        "edge" => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36 Edg/91.0.864.70".to_string(),
        "none" => String::new(),
        "golang" => "Go-http-client/1.1".to_string(),
        "curl" => "curl/7.68.0".to_string(),
        _ => user_agent.to_string(),
    }
}

pub(crate) fn parse_pem_chain(pem_chain: &str) -> Vec<String> {
    let pem_chain = pem_chain.replace("\r\n", "\n").replace('\r', "\n");
    let begin_marker = "-----BEGIN CERTIFICATE-----";
    let end_marker = "-----END CERTIFICATE-----";
    let mut certs = Vec::new();
    let mut index = 0;

    while index < pem_chain.len() {
        let Some(begin_offset) = pem_chain[index..].find(begin_marker) else {
            break;
        };
        let begin_index = index + begin_offset;
        let Some(end_offset) = pem_chain[begin_index..].find(end_marker) else {
            break;
        };
        let end_index = begin_index + end_offset;
        let base64_start = begin_index + begin_marker.len();
        let base64_content = pem_chain[base64_start..end_index]
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        certs.push(format!("{begin_marker}\n{base64_content}\n{end_marker}\n"));
        index = end_index + end_marker.len();
    }

    certs
}

pub(crate) fn wireguard_public_key(protocol_extra: &ProtocolExtraItem) -> Option<String> {
    nonempty_str(protocol_extra.wg_public_key.as_deref()).map(str::to_string)
}

pub(crate) fn wireguard_allowed_ips(protocol_extra: &ProtocolExtraItem) -> Vec<String> {
    split_list(protocol_extra.wg_allowed_ips.as_deref().unwrap_or_default())
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| {
            WIREGUARD_DEFAULT_ALLOWED_IPS
                .iter()
                .map(|item| (*item).to_string())
                .collect()
        })
}

pub(crate) fn parse_wireguard_reserved(value: Option<&str>) -> Option<Vec<i32>> {
    let value = nonempty_str(value)?;
    let mut reserved = Vec::new();
    for item in value.split(',') {
        let item = item.trim();
        let Ok(byte) = item.parse::<u8>() else {
            return None;
        };
        reserved.push(i32::from(byte));
    }
    (reserved.len() == WIREGUARD_RESERVED_LEN).then_some(reserved)
}

pub(crate) fn split_list(value: &str) -> Option<Vec<String>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        value
            .replace(['\n', '\r'], "")
            .split(',')
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

pub(crate) fn parse_i32(value: Option<&str>) -> Option<i32> {
    value.and_then(|value| value.trim().parse::<i32>().ok())
}

pub(crate) fn nonempty_str(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(crate) fn trimmed(value: &str) -> &str {
    value.trim()
}

pub(crate) fn first_list_value(value: Option<&str>) -> String {
    split_list(value.unwrap_or_default())
        .and_then(|items| {
            items
                .into_iter()
                .map(|item| item.trim().to_string())
                .find(|item| !item.is_empty())
        })
        .unwrap_or_default()
}

pub(crate) fn inbound_protocol_tag(protocol: InboundProtocol) -> &'static str {
    match protocol {
        InboundProtocol::socks => "socks",
        InboundProtocol::socks2 => "socks2",
        InboundProtocol::socks3 => "socks3",
        InboundProtocol::pac => "pac",
        InboundProtocol::api => "api",
        InboundProtocol::api2 => "api2",
        InboundProtocol::mixed => "mixed",
        InboundProtocol::speedtest => "speedtest",
    }
}

pub(crate) fn inbound_port(app_config: &AppConfig, protocol: InboundProtocol) -> i32 {
    app_config
        .inbound
        .iter()
        .find(|item| item.protocol == inbound_protocol_tag(InboundProtocol::socks))
        .map(|item| item.local_port)
        .or_else(|| app_config.inbound.first().map(|item| item.local_port))
        .unwrap_or(DEFAULT_LOCAL_PORT)
        + protocol.as_i32()
}
