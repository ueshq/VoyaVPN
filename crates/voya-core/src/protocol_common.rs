use crate::{
    AppConfig, ConfigType, InboundProtocol, ProfileItem, ProfileProtocol, ProfileTransport,
    DEFAULT_LOCAL_PORT, STREAM_SECURITY_TLS,
};

pub(crate) const DEFAULT_SECURITY: &str = "auto";
pub(crate) const RAW_HEADER_HTTP: &str = "http";
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

/// The SIP003 plugin a Shadowsocks node carries, derived once for both the
/// share-link exporter (`crate::fmt`) and the sing-box generator
/// (`crate::singbox`).
///
/// Both surfaces encode the same `obfs`/`v2ray-plugin` option list; building it
/// in one place is what keeps a new option from reaching a share link but not
/// the generated config, or the reverse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShadowsocksPlugin {
    pub name: &'static str,
    /// Ordered options. `None` renders as a bare flag (`tls`).
    pub opts: Vec<(String, Option<String>)>,
}

impl ShadowsocksPlugin {
    /// `k=v;flag;k=v` — sing-box's `plugin_opts`, and the tail of a SIP002
    /// `plugin=` value.
    pub(crate) fn render_opts(&self) -> String {
        self.opts
            .iter()
            .map(|(key, value)| match value {
                Some(value) => format!("{key}={value}"),
                None => key.clone(),
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    /// `name;k=v;flag` — the SIP002 `plugin=` query value.
    pub(crate) fn render_share(&self) -> String {
        let opts = self.render_opts();
        if opts.is_empty() {
            self.name.to_string()
        } else {
            format!("{};{opts}", self.name)
        }
    }
}

pub(crate) fn shadowsocks_plugin_for(item: &ProfileItem) -> Option<ShadowsocksPlugin> {
    if let Some(ProfileTransport::Tcp { header, host, .. }) = &item.transport {
        if header.as_deref() == Some(RAW_HEADER_HTTP) {
            return Some(ShadowsocksPlugin {
                name: "obfs-local",
                opts: vec![
                    ("obfs".to_string(), Some(RAW_HEADER_HTTP.to_string())),
                    (
                        "obfs-host".to_string(),
                        Some(first_list_value(host.as_deref())),
                    ),
                ],
            });
        }
    }

    let mut opts = Vec::new();
    if let Some(ProfileTransport::Websocket { host, path }) = &item.transport {
        opts.push(("mode".to_string(), Some("websocket".to_string())));
        opts.push(("host".to_string(), Some(first_list_value(host.as_deref()))));
        opts.push((
            "path".to_string(),
            Some(escape_plugin_value(path.as_deref().unwrap_or_default())),
        ));
    }
    if item.stream_security() == STREAM_SECURITY_TLS {
        opts.push(("tls".to_string(), None));
        if let Some(certificate) = item
            .tls
            .as_ref()
            .and_then(|tls| tls.certificate_pem.as_deref())
            .and_then(first_pem_body)
        {
            opts.push(("certRaw".to_string(), Some(certificate.replace('=', "\\="))));
        }
    }
    if opts.is_empty() {
        return None;
    }

    // v2ray-plugin multiplexing is incompatible with the way both cores dial
    // the plugin, so the option list always pins it off.
    opts.push(("mux".to_string(), Some("0".to_string())));
    Some(ShadowsocksPlugin {
        name: "v2ray-plugin",
        opts,
    })
}

fn escape_plugin_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('=', "\\=")
        .replace(',', "\\,")
}

fn first_pem_body(certificate_pem: &str) -> Option<String> {
    let certificate = parse_pem_chain(certificate_pem).into_iter().next()?;
    let body = certificate
        .trim()
        .trim_start_matches("-----BEGIN CERTIFICATE-----")
        .trim_end_matches("-----END CERTIFICATE-----")
        .trim()
        .to_string();
    (!body.is_empty()).then_some(body)
}

pub(crate) fn wireguard_public_key(protocol: &ProfileProtocol) -> Option<String> {
    let ProfileProtocol::WireGuard {
        peer_public_key, ..
    } = protocol
    else {
        return None;
    };
    nonempty_str(peer_public_key.as_deref()).map(str::to_string)
}

pub(crate) fn wireguard_allowed_ips(protocol: &ProfileProtocol) -> Vec<String> {
    let allowed_ips = match protocol {
        ProfileProtocol::WireGuard { allowed_ips, .. } => allowed_ips.as_deref(),
        _ => None,
    };
    split_list(allowed_ips.unwrap_or_default())
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

pub(crate) fn nonempty_str(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
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
        + protocol.port_offset()
}
