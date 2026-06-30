//! Share-link parsers and exporters.
//!
//! The behavior is ported from `ServiceLib/Handler/Fmt` while keeping this
//! crate pure: helpers that recognize full custom configs return the content
//! and suggested extension instead of writing temp files.

use std::{collections::BTreeMap, net::IpAddr};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use percent_encoding::percent_decode_str;
use serde_json::{Map, Value};
use thiserror::Error;
use url::Url;

use crate::{ConfigType, MultipleLoad, ProfileItem, ProtocolExtraItem, TransportExtraItem};

const DEFAULT_SECURITY: &str = "auto";
const DEFAULT_NETWORK: &str = "raw";
const RAW_NETWORK_ALIAS: &str = "tcp";
const RAW_HEADER_HTTP: &str = "http";
const NONE: &str = "none";
const STREAM_SECURITY_TLS: &str = "tls";
const ALLOW_INSECURE_TRUE: &str = "true";
const ALLOW_INSECURE_FALSE: &str = "false";
const GRPC_GUN_MODE: &str = "gun";
const GRPC_MULTI_MODE: &str = "multi";
const HYSTERIA2_DEFAULT_SCHEME: &str = "hysteria2://";
const HYSTERIA2_ALT_SCHEME: &str = "hy2://";
const NAIVE_HTTPS_SCHEME: &str = "naive+https://";
const NAIVE_QUIC_SCHEME: &str = "naive+quic://";
const INNER_URI_PROTOCOL: &str = "v2rayn://";
const MAX_BASE64_DECODE_INPUT: usize = 1024 * 1024;

const NETWORKS: &[&str] = &["raw", "xhttp", "kcp", "grpc", "ws", "httpupgrade"];
const XHTTP_MODES: &[&str] = &["auto", "packet-up", "stream-up", "stream-one"];

/// Protocol share-link parser/exporter interface.
pub trait ShareFmt {
    fn config_type(&self) -> ConfigType;
    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError>;
    fn export(&self, item: &ProfileItem) -> Result<String, ShareError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShareError {
    #[error("share link is empty")]
    EmptyInput,
    #[error("unsupported share protocol")]
    UnsupportedProtocol,
    #[error("invalid {protocol} URI: {reason}")]
    InvalidUri {
        protocol: &'static str,
        reason: String,
    },
    #[error("invalid {protocol} base64 payload")]
    InvalidBase64 { protocol: &'static str },
    #[error("invalid {protocol} JSON payload: {reason}")]
    InvalidJson {
        protocol: &'static str,
        reason: String,
    },
    #[error("{protocol} is missing required field {field}")]
    MissingField {
        protocol: &'static str,
        field: &'static str,
    },
    #[error("{protocol} has invalid port {port}")]
    InvalidPort {
        protocol: &'static str,
        port: String,
    },
    #[error("{protocol} cannot export config type {actual:?}")]
    WrongConfigType {
        protocol: &'static str,
        actual: ConfigType,
    },
    #[error("invalid full custom config")]
    InvalidFullConfig,
    #[error("invalid inner v2rayn profile: {reason}")]
    InvalidInner { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomConfigKind {
    SingBox,
    Hysteria2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomConfigImport {
    pub kind: CustomConfigKind,
    pub extension: String,
    pub contents: String,
    pub profile: ProfileItem,
}

#[derive(Debug, Clone)]
struct ParsedUri {
    scheme: String,
    address: String,
    port: i32,
    remarks: String,
    user_info: String,
    query: Query,
}

#[derive(Debug, Clone, Default)]
struct Query(Vec<(String, String)>);

impl Query {
    fn parse(raw_query: Option<&str>) -> Self {
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

    fn contains_key(&self, wanted: &str) -> bool {
        self.0
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(wanted))
    }

    fn value(&self, wanted: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(wanted))
            .map(|(_, value)| value.as_str())
    }

    fn value_or(&self, wanted: &str, default_value: &str) -> String {
        self.value(wanted).unwrap_or(default_value).to_string()
    }

    fn decoded_or(&self, wanted: &str, default_value: &str) -> String {
        url_decode(self.value(wanted).unwrap_or(default_value))
    }
}

type QueryPairs = Vec<(String, String)>;

#[derive(Debug, Clone, Copy)]
pub struct VmessFmt;
#[derive(Debug, Clone, Copy)]
pub struct VlessFmt;
#[derive(Debug, Clone, Copy)]
pub struct TrojanFmt;
#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksFmt;
#[derive(Debug, Clone, Copy)]
pub struct SocksFmt;
#[derive(Debug, Clone, Copy)]
pub struct Hysteria2Fmt;
#[derive(Debug, Clone, Copy)]
pub struct TuicFmt;
#[derive(Debug, Clone, Copy)]
pub struct WireguardFmt;
#[derive(Debug, Clone, Copy)]
pub struct AnytlsFmt;
#[derive(Debug, Clone, Copy)]
pub struct NaiveFmt;

impl ShareFmt for VmessFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::VMess
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        if input.contains('@') {
            parse_vmess_standard(input).or_else(|_| parse_vmess_base64(input))
        } else {
            parse_vmess_base64(input)
        }
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("vmess", item, ConfigType::VMess)?;
        ensure_address_port("vmess", item)?;
        ensure_nonempty("vmess", "password", &item.password)?;

        let aid = item
            .protocol_extra
            .alter_id
            .as_deref()
            .unwrap_or("0")
            .parse::<i32>()
            .unwrap_or(0);
        let network = item_network(item);
        let transport = &item.transport_extra;
        let vmess = json_object([
            ("v", Value::String("2".to_string())),
            ("ps", Value::String(item.remarks.trim().to_string())),
            ("add", Value::String(item.address.clone())),
            ("port", Value::String(item.port.to_string())),
            ("id", Value::String(item.password.clone())),
            ("aid", Value::String(aid.to_string())),
            (
                "scy",
                Value::String(
                    nonempty_option(&item.protocol_extra.vmess_security)
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
            (
                "type",
                Value::String(match network {
                    "raw" => option_or(&transport.raw_header_type, NONE),
                    "kcp" => option_or(&transport.kcp_header_type, NONE),
                    "xhttp" => option_or(&transport.xhttp_mode, NONE),
                    "grpc" => option_or(&transport.grpc_mode, NONE),
                    _ => NONE.to_string(),
                }),
            ),
            (
                "host",
                Value::String(match network {
                    "raw" | "ws" | "httpupgrade" | "xhttp" => option_or(&transport.host, ""),
                    "grpc" => option_or(&transport.grpc_authority, ""),
                    _ => String::new(),
                }),
            ),
            (
                "path",
                Value::String(match network {
                    "raw" | "ws" | "httpupgrade" | "xhttp" => option_or(&transport.path, ""),
                    "kcp" => option_or(&transport.kcp_seed, ""),
                    "grpc" => option_or(&transport.grpc_service_name, ""),
                    _ => String::new(),
                }),
            ),
            ("tls", Value::String(item.stream_security.clone())),
            ("sni", Value::String(item.sni.clone())),
            ("alpn", Value::String(item.alpn.clone())),
            ("fp", Value::String(item.fingerprint.clone())),
            (
                "insecure",
                Value::String(if item.allow_insecure == ALLOW_INSECURE_TRUE {
                    "1".to_string()
                } else {
                    "0".to_string()
                }),
            ),
        ]);

        let payload = serde_json::to_string(&vmess).map_err(|error| ShareError::InvalidJson {
            protocol: "vmess",
            reason: error.to_string(),
        })?;
        Ok(format!("vmess://{}", base64_encode(&payload, false)))
    }
}

impl ShareFmt for VlessFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::VLESS
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed = parse_uri(input, "vless")?;
        let mut item = profile_from_uri(ConfigType::VLESS, &parsed);
        item.password = parsed.user_info;
        item.protocol_extra.vless_encryption = Some(parsed.query.value_or("encryption", NONE));
        item.protocol_extra.flow = nonempty(parsed.query.value_or("flow", ""));
        resolve_uri_query(&parsed.query, &mut item);
        ensure_address_port("vless", &item)?;
        ensure_nonempty("vless", "password", &item.password)?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("vless", item, ConfigType::VLESS)?;
        ensure_address_port("vless", item)?;
        ensure_nonempty("vless", "password", &item.password)?;
        let mut query = Vec::new();
        query.push((
            "encryption".to_string(),
            nonempty_option(&item.protocol_extra.vless_encryption)
                .unwrap_or(NONE)
                .to_string(),
        ));
        if let Some(flow) = nonempty_option(&item.protocol_extra.flow) {
            query.push(("flow".to_string(), flow.to_string()));
        }
        to_uri_query(item, Some(NONE), &mut query);
        Ok(to_uri(
            ConfigType::VLESS,
            &item.address,
            item.port,
            &item.password,
            &query,
            &item.remarks,
        ))
    }
}

impl ShareFmt for TrojanFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::Trojan
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed = parse_uri(input, "trojan")?;
        let mut item = profile_from_uri(ConfigType::Trojan, &parsed);
        item.password = parsed.user_info;
        item.protocol_extra.flow = nonempty(parsed.query.value_or("flow", ""));
        resolve_uri_query(&parsed.query, &mut item);
        ensure_address_port("trojan", &item)?;
        ensure_nonempty("trojan", "password", &item.password)?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("trojan", item, ConfigType::Trojan)?;
        ensure_address_port("trojan", item)?;
        ensure_nonempty("trojan", "password", &item.password)?;
        let mut query = Vec::new();
        if let Some(flow) = nonempty_option(&item.protocol_extra.flow) {
            query.push(("flow".to_string(), flow.to_string()));
        }
        to_uri_query(item, None, &mut query);
        Ok(to_uri(
            ConfigType::Trojan,
            &item.address,
            item.port,
            &item.password,
            &query,
            &item.remarks,
        ))
    }
}

impl ShareFmt for ShadowsocksFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::Shadowsocks
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let mut item =
            parse_shadowsocks_legacy(input).or_else(|_| parse_shadowsocks_sip002(input))?;
        ensure_address_port("ss", &item)?;
        if nonempty_option(&item.protocol_extra.ss_method).is_none() {
            return Err(ShareError::MissingField {
                protocol: "ss",
                field: "method",
            });
        }
        ensure_nonempty("ss", "password", &item.password)?;
        item.config_type = ConfigType::Shadowsocks;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("ss", item, ConfigType::Shadowsocks)?;
        ensure_address_port("ss", item)?;
        let method =
            nonempty_option(&item.protocol_extra.ss_method).ok_or(ShareError::MissingField {
                protocol: "ss",
                field: "method",
            })?;
        ensure_nonempty("ss", "password", &item.password)?;

        let user_info = base64_encode(&format!("{method}:{}", item.password), true);
        let mut query = Vec::new();
        if let Some(plugin) = shadowsocks_plugin(item) {
            query.push(("plugin".to_string(), url_encode(&plugin)));
        }

        Ok(to_uri(
            ConfigType::Shadowsocks,
            &item.address,
            item.port,
            &user_info,
            &query,
            &item.remarks,
        ))
    }
}

impl ShareFmt for SocksFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::SOCKS
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let mut item = parse_socks_new(input).or_else(|_| parse_socks_legacy(input))?;
        ensure_address_port("socks", &item)?;
        item.config_type = ConfigType::SOCKS;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("socks", item, ConfigType::SOCKS)?;
        ensure_address_port("socks", item)?;
        let user_info = base64_encode(&format!("{}:{}", item.username, item.password), true);
        Ok(to_uri(
            ConfigType::SOCKS,
            &item.address,
            item.port,
            &user_info,
            &[],
            &item.remarks,
        ))
    }
}

impl ShareFmt for Hysteria2Fmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::Hysteria2
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed = parse_uri_with_schemes(input, "hysteria2", &["hysteria2", "hy2"])?;
        let mut item = profile_from_uri(ConfigType::Hysteria2, &parsed);
        item.password = parsed.user_info;
        resolve_uri_query(&parsed.query, &mut item);
        if item.cert_sha.is_empty() {
            item.cert_sha = parsed.query.decoded_or("pinSHA256", "");
        }
        item.protocol_extra.ports = nonempty(parsed.query.decoded_or("mport", ""));
        item.protocol_extra.salamander_pass =
            nonempty(parsed.query.decoded_or("obfs-password", ""));
        ensure_address_port("hysteria2", &item)?;
        ensure_nonempty("hysteria2", "password", &item.password)?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("hysteria2", item, ConfigType::Hysteria2)?;
        ensure_address_port("hysteria2", item)?;
        ensure_nonempty("hysteria2", "password", &item.password)?;
        let mut query = Vec::new();
        to_uri_query_lite(item, &mut query);
        if let Some(pass) = nonempty_option(&item.protocol_extra.salamander_pass) {
            query.push(("obfs".to_string(), "salamander".to_string()));
            query.push(("obfs-password".to_string(), url_encode(pass)));
        }
        if let Some(ports) = nonempty_option(&item.protocol_extra.ports) {
            query.push(("mport".to_string(), url_encode(&ports.replace(':', "-"))));
        }
        if !item.cert_sha.is_empty() {
            let sha = item.cert_sha.split(',').next().unwrap_or("");
            query.push(("pinSHA256".to_string(), url_encode(sha)));
        }
        Ok(format!(
            "{}{}",
            HYSTERIA2_DEFAULT_SCHEME,
            to_uri_without_scheme(
                &item.address,
                item.port,
                &item.password,
                &query,
                &item.remarks
            )
        ))
    }
}

impl ShareFmt for TuicFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::TUIC
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed = parse_uri(input, "tuic")?;
        let mut item = profile_from_uri(ConfigType::TUIC, &parsed);
        if let Some((username, password)) = parsed.user_info.split_once(':') {
            item.username = username.to_string();
            item.password = password.to_string();
        }
        resolve_uri_query(&parsed.query, &mut item);
        item.protocol_extra.congestion_control =
            nonempty(parsed.query.value_or("congestion_control", ""));
        ensure_address_port("tuic", &item)?;
        ensure_nonempty("tuic", "username", &item.username)?;
        ensure_nonempty("tuic", "password", &item.password)?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("tuic", item, ConfigType::TUIC)?;
        ensure_address_port("tuic", item)?;
        ensure_nonempty("tuic", "username", &item.username)?;
        ensure_nonempty("tuic", "password", &item.password)?;
        let mut query = Vec::new();
        to_uri_query_lite(item, &mut query);
        if let Some(congestion) = nonempty_option(&item.protocol_extra.congestion_control) {
            query.push(("congestion_control".to_string(), congestion.to_string()));
        }
        Ok(to_uri(
            ConfigType::TUIC,
            &item.address,
            item.port,
            &format!("{}:{}", item.username, item.password),
            &query,
            &item.remarks,
        ))
    }
}

impl ShareFmt for WireguardFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::WireGuard
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed = parse_uri(input, "wireguard")?;
        let mut item = profile_from_uri(ConfigType::WireGuard, &parsed);
        item.password = parsed.user_info;
        item.protocol_extra.wg_public_key = nonempty(parsed.query.decoded_or("publickey", ""));
        item.protocol_extra.wg_preshared_key =
            nonempty(parsed.query.decoded_or("presharedkey", ""));
        item.protocol_extra.wg_reserved = nonempty(parsed.query.decoded_or("reserved", ""));
        item.protocol_extra.wg_interface_address = nonempty(parsed.query.decoded_or("address", ""));
        let allowed_ips = parsed.query.decoded_or("allowedips", "");
        item.protocol_extra.wg_allowed_ips = nonempty(if allowed_ips.is_empty() {
            parsed.query.decoded_or("allowed_ips", "")
        } else {
            allowed_ips
        });
        item.protocol_extra.wg_mtu = parse_positive_i32(&parsed.query.decoded_or("mtu", ""));
        ensure_address_port("wireguard", &item)?;
        ensure_nonempty("wireguard", "private key", &item.password)?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("wireguard", item, ConfigType::WireGuard)?;
        ensure_address_port("wireguard", item)?;
        ensure_nonempty("wireguard", "private key", &item.password)?;
        let mut query = Vec::new();
        push_encoded_opt(&mut query, "publickey", &item.protocol_extra.wg_public_key);
        push_encoded_opt(
            &mut query,
            "presharedkey",
            &item.protocol_extra.wg_preshared_key,
        );
        push_encoded_opt(&mut query, "reserved", &item.protocol_extra.wg_reserved);
        push_encoded_opt(
            &mut query,
            "address",
            &item.protocol_extra.wg_interface_address,
        );
        push_encoded_opt(
            &mut query,
            "allowedips",
            &item.protocol_extra.wg_allowed_ips,
        );
        if let Some(mtu) = item.protocol_extra.wg_mtu.filter(|value| *value > 0) {
            query.push(("mtu".to_string(), mtu.to_string()));
        }
        Ok(to_uri(
            ConfigType::WireGuard,
            &item.address,
            item.port,
            &item.password,
            &query,
            &item.remarks,
        ))
    }
}

impl ShareFmt for AnytlsFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::Anytls
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed = parse_uri(input, "anytls")?;
        let mut item = profile_from_uri(ConfigType::Anytls, &parsed);
        item.password = parsed.user_info;
        resolve_uri_query(&parsed.query, &mut item);
        ensure_address_port("anytls", &item)?;
        ensure_nonempty("anytls", "password", &item.password)?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("anytls", item, ConfigType::Anytls)?;
        ensure_address_port("anytls", item)?;
        ensure_nonempty("anytls", "password", &item.password)?;
        let mut query = Vec::new();
        to_uri_query(item, Some(NONE), &mut query);
        Ok(to_uri(
            ConfigType::Anytls,
            &item.address,
            item.port,
            &item.password,
            &query,
            &item.remarks,
        ))
    }
}

impl ShareFmt for NaiveFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::Naive
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed =
            parse_uri_with_schemes(input, "naive", &["naive", "naive+https", "naive+quic"])?;
        let mut item = profile_from_uri(ConfigType::Naive, &parsed);
        if parsed.scheme.contains("quic") {
            item.protocol_extra.naive_quic = Some(true);
        }
        if let Some((username, password)) = parsed.user_info.split_once(':') {
            item.username = username.to_string();
            item.password = password.to_string();
        } else {
            item.password = parsed.user_info;
        }
        resolve_uri_query(&parsed.query, &mut item);
        if let Some(value) = parse_positive_i32(&parsed.query.value_or("insecure-concurrency", ""))
        {
            item.protocol_extra.insecure_concurrency = Some(value);
        }
        ensure_address_port("naive", &item)?;
        ensure_nonempty("naive", "password", &item.password)?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("naive", item, ConfigType::Naive)?;
        ensure_address_port("naive", item)?;
        ensure_nonempty("naive", "password", &item.password)?;
        let mut query = Vec::new();
        to_uri_query(item, Some(NONE), &mut query);
        if let Some(concurrency) = item
            .protocol_extra
            .insecure_concurrency
            .filter(|value| *value > 0)
        {
            query.push(("insecure-concurrency".to_string(), concurrency.to_string()));
        }
        let user_info = if item.username.is_empty() {
            url_encode(&item.password)
        } else {
            format!(
                "{}:{}",
                url_encode(&item.username),
                url_encode(&item.password)
            )
        };
        let scheme = if item.protocol_extra.naive_quic == Some(true) {
            NAIVE_QUIC_SCHEME
        } else {
            NAIVE_HTTPS_SCHEME
        };
        Ok(format!(
            "{scheme}{}",
            to_uri_without_scheme_preencoded_userinfo(
                &item.address,
                item.port,
                &user_info,
                &query,
                &item.remarks
            )
        ))
    }
}

pub fn parse_share_link(input: &str) -> Result<ProfileItem, ShareError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ShareError::EmptyInput);
    }
    if starts_with_ci(trimmed, "vmess://") {
        VmessFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "ss://") {
        ShadowsocksFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "socks://")
        || starts_with_ci(trimmed, "socks5://")
        || starts_with_ci(trimmed, "socks4://")
    {
        SocksFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "trojan://") {
        TrojanFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "vless://") {
        VlessFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, HYSTERIA2_DEFAULT_SCHEME)
        || starts_with_ci(trimmed, HYSTERIA2_ALT_SCHEME)
    {
        Hysteria2Fmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "tuic://") {
        TuicFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "wireguard://") {
        WireguardFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "anytls://") {
        AnytlsFmt.parse(trimmed)
    } else if starts_with_ci(trimmed, "naive://")
        || starts_with_ci(trimmed, NAIVE_HTTPS_SCHEME)
        || starts_with_ci(trimmed, NAIVE_QUIC_SCHEME)
    {
        NaiveFmt.parse(trimmed)
    } else {
        Err(ShareError::UnsupportedProtocol)
    }
}

pub fn export_share_link(item: &ProfileItem) -> Result<String, ShareError> {
    match item.config_type {
        ConfigType::VMess => VmessFmt.export(item),
        ConfigType::Shadowsocks => ShadowsocksFmt.export(item),
        ConfigType::SOCKS => SocksFmt.export(item),
        ConfigType::Trojan => TrojanFmt.export(item),
        ConfigType::VLESS => VlessFmt.export(item),
        ConfigType::Hysteria2 => Hysteria2Fmt.export(item),
        ConfigType::TUIC => TuicFmt.export(item),
        ConfigType::WireGuard => WireguardFmt.export(item),
        ConfigType::Anytls => AnytlsFmt.export(item),
        ConfigType::Naive => NaiveFmt.export(item),
        actual => Err(ShareError::WrongConfigType {
            protocol: "share",
            actual,
        }),
    }
}

pub fn parse_share_lines(input: &str) -> Vec<Result<ProfileItem, ShareError>> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_share_link)
        .collect()
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
        let mut item = ProfileItem {
            config_type: ConfigType::Shadowsocks,
            remarks: string_field(object, "remarks"),
            password: string_field(object, "password"),
            address: string_field(object, "server"),
            port: string_field(object, "server_port").parse().unwrap_or(0),
            ..ProfileItem::default()
        };
        item.protocol_extra.ss_method = nonempty(string_field(object, "method"));
        ensure_address_port("ss-sip008", &item)?;
        result.push(item);
    }
    Ok(result)
}

pub fn parse_wireguard_config(input: &str) -> Result<Vec<ProfileItem>, ShareError> {
    let mut interface = BTreeMap::<String, String>::new();
    let mut peers = Vec::<BTreeMap<String, String>>::new();
    let mut in_peer = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("[Interface]") {
            in_peer = false;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("[Peer]") {
            peers.push(BTreeMap::new());
            in_peer = true;
            continue;
        }
        if trimmed.starts_with('[') || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let mut value = raw_value.trim().to_string();
        if let Some(position) = value.find(['#', ';']) {
            value.truncate(position);
            value = value.trim_end().to_string();
        }
        if in_peer {
            if let Some(peer) = peers.last_mut() {
                peer.insert(key.trim().to_ascii_lowercase(), value);
            }
        } else {
            interface.insert(key.trim().to_ascii_lowercase(), value);
        }
    }

    let private_key = nonempty_str(interface.get("privatekey").map(String::as_str)).ok_or(
        ShareError::MissingField {
            protocol: "wireguard-config",
            field: "PrivateKey",
        },
    )?;
    let wg_mtu = interface
        .get("mtu")
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value > 0);
    let wg_interface_address = interface.get("address").cloned().unwrap_or_default();

    let mut result = Vec::new();
    for peer in peers {
        let Some(endpoint) = nonempty_str(peer.get("endpoint").map(String::as_str)) else {
            continue;
        };
        let Some((address, port)) = parse_wireguard_endpoint(endpoint) else {
            continue;
        };
        let item = ProfileItem {
            remarks: format!("WireGuard Peer {}", result.len() + 1),
            config_type: ConfigType::WireGuard,
            address,
            port,
            password: private_key.to_string(),
            protocol_extra: ProtocolExtraItem {
                wg_public_key: peer
                    .get("publickey")
                    .and_then(|value| nonempty(value.clone())),
                wg_preshared_key: peer
                    .get("presharedkey")
                    .and_then(|value| nonempty(value.clone())),
                wg_interface_address: nonempty(wg_interface_address.clone()),
                wg_allowed_ips: peer
                    .get("allowedips")
                    .and_then(|value| nonempty(value.clone())),
                wg_reserved: peer
                    .get("reserved")
                    .and_then(|value| nonempty(value.clone())),
                wg_mtu,
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };
        result.push(item);
    }

    if result.is_empty() {
        Err(ShareError::InvalidFullConfig)
    } else {
        Ok(result)
    }
}

pub fn parse_full_custom_config(
    input: &str,
    sub_remarks: Option<&str>,
) -> Result<Vec<CustomConfigImport>, ShareError> {
    let trimmed = input.trim();
    if trimmed.is_empty() || is_html_page(trimmed) {
        return Err(ShareError::InvalidFullConfig);
    }

    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
        let mut imports = Vec::new();
        for value in items {
            let object =
                serde_json::to_string(&value).map_err(|error| ShareError::InvalidJson {
                    protocol: "custom",
                    reason: error.to_string(),
                })?;
            if let Ok(mut nested) = parse_full_custom_config(&object, sub_remarks) {
                imports.append(&mut nested);
            }
        }
        if imports.is_empty() {
            return Err(ShareError::InvalidFullConfig);
        }
        return Ok(imports);
    }

    if let Some(import) = parse_singbox_custom(trimmed, sub_remarks)? {
        return Ok(vec![import]);
    }
    if contains_all_ci(trimmed, &["server", "auth", "up", "down", "listen"]) {
        return Ok(vec![custom_import(
            CustomConfigKind::Hysteria2,
            "json",
            trimmed,
            sub_remarks.unwrap_or("hysteria2_custom"),
        )]);
    }

    Err(ShareError::InvalidFullConfig)
}

pub fn export_inner_share_links(items: &[ProfileItem]) -> Result<String, ShareError> {
    let mut id_map = BTreeMap::<String, String>::new();
    for item in items
        .iter()
        .filter(|item| item.config_type != ConfigType::Custom)
    {
        if !item.index_id.is_empty() {
            let export_id = format!("inner-export-{}", id_map.len() + 1);
            id_map.entry(item.index_id.clone()).or_insert(export_id);
        }
    }

    let mut lines = Vec::new();
    for item in items
        .iter()
        .filter(|item| item.config_type != ConfigType::Custom)
    {
        let mut clone = item.clone();
        if let Some(mapped) = id_map.get(&clone.index_id) {
            clone.index_id.clone_from(mapped);
        }
        if is_group_type(clone.config_type) {
            if nonempty_option(&clone.protocol_extra.sub_child_items).is_some() {
                clone.protocol_extra.sub_child_items = Some("self".to_string());
            }
            if let Some(children) = nonempty_option(&clone.protocol_extra.child_items) {
                let mapped_children = split_csv(children)
                    .into_iter()
                    .filter_map(|child| id_map.get(&child).cloned())
                    .collect::<Vec<_>>();
                clone.protocol_extra.child_items = if mapped_children.is_empty() {
                    None
                } else {
                    Some(mapped_children.join(","))
                };
            }
        }
        lines.push(export_inner_single(&clone)?);
    }

    if lines.is_empty() {
        Err(ShareError::InvalidInner {
            reason: "no exportable profiles".to_string(),
        })
    } else {
        Ok(format!("{}\n", lines.join("\n")))
    }
}

pub fn parse_inner_share_links(input: &str, subid: &str) -> Result<Vec<ProfileItem>, ShareError> {
    let mut parsed = Vec::<ProfileItem>::new();
    let mut id_map = BTreeMap::<String, String>::new();

    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if !starts_with_ci(line, INNER_URI_PROTOCOL) {
            continue;
        }
        let mut item = parse_inner_single(line)?;
        if item.config_type == ConfigType::Custom {
            continue;
        }
        let new_id = format!("inner-import-{}", parsed.len() + 1);
        if !item.index_id.is_empty() {
            id_map.insert(item.index_id.clone(), new_id.clone());
        }
        item.index_id = new_id;
        parsed.push(item);
    }

    let mut result = Vec::new();
    for mut item in parsed {
        if is_group_type(item.config_type) {
            if item.protocol_extra.sub_child_items.as_deref() == Some("self") {
                item.protocol_extra.sub_child_items = Some(subid.to_string());
            } else {
                item.protocol_extra.sub_child_items = None;
            }

            item.protocol_extra.child_items =
                item.protocol_extra
                    .child_items
                    .as_deref()
                    .and_then(|children| {
                        let mapped = split_csv(children)
                            .into_iter()
                            .filter_map(|id| id_map.get(&id).cloned())
                            .collect::<Vec<_>>();
                        if mapped.is_empty() {
                            None
                        } else {
                            Some(mapped.join(","))
                        }
                    });

            if item.protocol_extra.sub_child_items.is_none()
                && item.protocol_extra.child_items.is_none()
            {
                continue;
            }
        }
        result.push(item);
    }

    if result.is_empty() {
        Err(ShareError::InvalidInner {
            reason: "no valid profiles".to_string(),
        })
    } else {
        Ok(result)
    }
}

fn parse_vmess_standard(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri(input, "vmess")?;
    let mut item = profile_from_uri(ConfigType::VMess, &parsed);
    item.password = parsed.user_info;
    item.protocol_extra.vmess_security = Some(DEFAULT_SECURITY.to_string());
    resolve_uri_query(&parsed.query, &mut item);
    ensure_address_port("vmess", &item)?;
    ensure_nonempty("vmess", "password", &item.password)?;
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

    let mut item = ProfileItem {
        config_type: ConfigType::VMess,
        network: DEFAULT_NETWORK.to_string(),
        remarks: value_string(object, "ps"),
        address: value_string(object, "add"),
        port: value_i32(object, "port").unwrap_or(0),
        password: value_string(object, "id"),
        stream_security: value_string(object, "tls"),
        sni: value_string(object, "sni"),
        alpn: value_string(object, "alpn"),
        fingerprint: value_string(object, "fp"),
        allow_insecure: if value_string(object, "insecure") == "1" {
            ALLOW_INSECURE_TRUE.to_string()
        } else {
            String::new()
        },
        protocol_extra: ProtocolExtraItem {
            alter_id: Some(value_i32(object, "aid").unwrap_or(0).to_string()),
            vmess_security: Some({
                let security = value_string(object, "scy");
                if security.is_empty() {
                    DEFAULT_SECURITY.to_string()
                } else {
                    security
                }
            }),
            ..ProtocolExtraItem::default()
        },
        transport_extra: TransportExtraItem {
            raw_header_type: Some(NONE.to_string()),
            ..TransportExtraItem::default()
        },
        ..ProfileItem::default()
    };

    let network = value_string(object, "net");
    if !network.is_empty() {
        item.network = if network == RAW_NETWORK_ALIAS {
            DEFAULT_NETWORK.to_string()
        } else {
            network
        };
    }
    let vmess_type = value_string(object, "type");
    if !vmess_type.is_empty() {
        match item.network.as_str() {
            "raw" => item.transport_extra.raw_header_type = Some(vmess_type),
            "kcp" => item.transport_extra.kcp_header_type = Some(vmess_type),
            "xhttp" => item.transport_extra.xhttp_mode = Some(vmess_type),
            "grpc" => item.transport_extra.grpc_mode = Some(vmess_type),
            _ => {}
        }
    }
    let host = value_string(object, "host");
    let path = value_string(object, "path");
    match item.network.as_str() {
        "raw" => {
            item.transport_extra.host = nonempty(host);
            item.transport_extra.path = nonempty(path);
        }
        "kcp" => item.transport_extra.kcp_seed = nonempty(path),
        "ws" | "httpupgrade" | "xhttp" => {
            item.transport_extra.host = nonempty(host);
            item.transport_extra.path = nonempty(path);
        }
        "grpc" => {
            item.transport_extra.grpc_authority = nonempty(host);
            item.transport_extra.grpc_service_name = nonempty(path);
        }
        _ => {}
    }

    ensure_address_port("vmess", &item)?;
    ensure_nonempty("vmess", "password", &item.password)?;
    Ok(item)
}

fn parse_shadowsocks_legacy(input: &str) -> Result<ProfileItem, ShareError> {
    let mut rest = input
        .trim()
        .strip_prefix_ci("ss://")
        .ok_or(ShareError::UnsupportedProtocol)?
        .to_string();
    let mut remarks = String::new();
    if let Some((before, after)) = rest.split_once('#') {
        remarks = url_decode(after);
        rest = before.to_string();
    }
    if rest.contains('@') {
        return Err(ShareError::InvalidUri {
            protocol: "ss",
            reason: "not a legacy shadowsocks link".to_string(),
        });
    }
    let decoded = base64_decode(rest.trim_end_matches('/'), "ss")?;
    let Some((method_password, address_port)) = decoded.split_once('@') else {
        return Err(ShareError::InvalidUri {
            protocol: "ss",
            reason: "missing @".to_string(),
        });
    };
    let Some((method, password)) = method_password.split_once(':') else {
        return Err(ShareError::InvalidUri {
            protocol: "ss",
            reason: "missing method/password".to_string(),
        });
    };
    let Some((address, port)) = rsplit_host_port(address_port) else {
        return Err(ShareError::InvalidUri {
            protocol: "ss",
            reason: "missing host/port".to_string(),
        });
    };

    Ok(ProfileItem {
        config_type: ConfigType::Shadowsocks,
        remarks,
        address,
        port: parse_port("ss", port)?,
        password: password.to_string(),
        protocol_extra: ProtocolExtraItem {
            ss_method: Some(method.to_string()),
            ..ProtocolExtraItem::default()
        },
        ..ProfileItem::default()
    })
}

fn parse_shadowsocks_sip002(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri(input, "ss")?;
    let mut item = profile_from_uri(ConfigType::Shadowsocks, &parsed);
    if parsed.user_info.contains(':') {
        let Some((method, password)) = parsed.user_info.split_once(':') else {
            return Err(ShareError::InvalidUri {
                protocol: "ss",
                reason: "invalid user info".to_string(),
            });
        };
        item.protocol_extra.ss_method = Some(method.to_string());
        item.password = url_decode(password);
    } else {
        let decoded = base64_decode(&parsed.user_info, "ss")?;
        let Some((method, password)) = decoded.split_once(':') else {
            return Err(ShareError::InvalidUri {
                protocol: "ss",
                reason: "invalid encoded user info".to_string(),
            });
        };
        item.protocol_extra.ss_method = Some(method.to_string());
        item.password = password.to_string();
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
            item.network = DEFAULT_NETWORK.to_string();
            item.transport_extra.raw_header_type = Some(RAW_HEADER_HTTP.to_string());
            item.transport_extra.host = obfs_host.map(str::to_string);
        }
    } else if plugin_name == "v2ray-plugin" {
        let mode = plugin_parts
            .iter()
            .find_map(|part| part.strip_prefix("mode="))
            .unwrap_or("websocket");
        if mode == "websocket" {
            item.network = "ws".to_string();
            if let Some(host) = plugin_parts
                .iter()
                .find_map(|part| part.strip_prefix("host="))
            {
                item.transport_extra.host = Some(host.to_string());
                item.sni = host.to_string();
            }
            if let Some(path) = plugin_parts
                .iter()
                .find_map(|part| part.strip_prefix("path="))
            {
                item.transport_extra.path = Some(
                    path.replace("\\=", "=")
                        .replace("\\,", ",")
                        .replace("\\\\", "\\"),
                );
            }
        }
        if plugin_parts.contains(&"tls") {
            item.stream_security = STREAM_SECURITY_TLS.to_string();
            if let Some(cert) = plugin_parts
                .iter()
                .find_map(|part| part.strip_prefix("certRaw="))
            {
                let cert = cert.replace("\\=", "=");
                item.cert =
                    format!("-----BEGIN CERTIFICATE-----\n{cert}\n-----END CERTIFICATE-----");
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

fn shadowsocks_plugin(item: &ProfileItem) -> Option<String> {
    let transport = &item.transport_extra;
    let mut plugin = String::new();
    let mut plugin_args = String::new();

    if item.network == DEFAULT_NETWORK
        && transport.raw_header_type.as_deref() == Some(RAW_HEADER_HTTP)
    {
        plugin = "obfs-local".to_string();
        plugin_args = format!(
            "obfs=http;obfs-host={};",
            transport.host.as_deref().unwrap_or("")
        );
    } else {
        if item.network == "ws" {
            plugin_args.push_str("mode=websocket;");
            plugin_args.push_str(&format!(
                "host={};",
                transport.host.as_deref().unwrap_or("")
            ));
            let path = transport
                .path
                .as_deref()
                .unwrap_or("")
                .replace('\\', "\\\\")
                .replace('=', "\\=")
                .replace(',', "\\,");
            plugin_args.push_str(&format!("path={path};"));
        }
        if item.stream_security == STREAM_SECURITY_TLS {
            plugin_args.push_str("tls;");
            if let Some(cert_raw) = extract_first_pem_body(&item.cert) {
                plugin_args.push_str(&format!("certRaw={};", cert_raw.replace('=', "\\=")));
            }
        }
        if !plugin_args.is_empty() {
            plugin = "v2ray-plugin".to_string();
            plugin_args.push_str("mux=0;");
        }
    }

    if plugin.is_empty() {
        None
    } else {
        let mut result = format!("{plugin};{plugin_args}");
        if result.ends_with(';') {
            result.pop();
        }
        Some(result)
    }
}

fn parse_socks_new(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri_with_schemes(input, "socks", &["socks", "socks5", "socks4"])?;
    let mut item = profile_from_uri(ConfigType::SOCKS, &parsed);
    if !parsed.user_info.is_empty() {
        if let Some((username, password)) = parsed.user_info.split_once(':') {
            item.username = username.to_string();
            item.password = password.to_string();
        } else {
            let decoded = base64_decode(&parsed.user_info, "socks")?;
            if let Some((username, password)) = decoded.split_once(':') {
                item.username = username.to_string();
                item.password = password.to_string();
            }
        }
    }
    Ok(item)
}

fn parse_socks_legacy(input: &str) -> Result<ProfileItem, ShareError> {
    let mut rest = input
        .trim()
        .strip_prefix_ci("socks://")
        .ok_or(ShareError::UnsupportedProtocol)?
        .to_string();
    let mut remarks = String::new();
    if let Some((before, after)) = rest.split_once('#') {
        remarks = url_decode(after);
        rest = before.to_string();
    }
    if !rest.contains('@') {
        rest = base64_decode(&rest, "socks")?;
    }
    let Some((user_pass, address_port)) = rest.split_once('@') else {
        return Err(ShareError::InvalidUri {
            protocol: "socks",
            reason: "missing @".to_string(),
        });
    };
    let Some((username, password)) = user_pass.split_once(':') else {
        return Err(ShareError::InvalidUri {
            protocol: "socks",
            reason: "missing username/password".to_string(),
        });
    };
    let Some((address, port)) = rsplit_host_port(address_port) else {
        return Err(ShareError::InvalidUri {
            protocol: "socks",
            reason: "missing host/port".to_string(),
        });
    };
    Ok(ProfileItem {
        config_type: ConfigType::SOCKS,
        remarks,
        address,
        port: parse_port("socks", port)?,
        username: username.to_string(),
        password: password.to_string(),
        ..ProfileItem::default()
    })
}

fn parse_uri(input: &str, protocol: &'static str) -> Result<ParsedUri, ShareError> {
    parse_uri_with_schemes(input, protocol, &[protocol])
}

fn parse_uri_with_schemes(
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
    let address = url
        .host_str()
        .unwrap_or("")
        .trim_matches(['[', ']'])
        .to_string();
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

fn profile_from_uri(config_type: ConfigType, parsed: &ParsedUri) -> ProfileItem {
    ProfileItem {
        config_type,
        address: parsed.address.clone(),
        port: parsed.port,
        remarks: parsed.remarks.clone(),
        ..ProfileItem::default()
    }
}

fn to_uri(
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
        to_uri_without_scheme(address, port, user_info, query, remark)
    )
}

fn to_uri_without_scheme(
    address: &str,
    port: i32,
    user_info: &str,
    query: &[(String, String)],
    remark: &str,
) -> String {
    to_uri_without_scheme_preencoded_userinfo(address, port, &url_encode(user_info), query, remark)
}

fn to_uri_without_scheme_preencoded_userinfo(
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
    format!("{user_info}@{}:{port}{query}{remark}", ipv6_host(address))
}

fn to_uri_query(item: &ProfileItem, security_default: Option<&str>, query: &mut QueryPairs) {
    let transport = &item.transport_extra;
    if !item.stream_security.is_empty() {
        query.push(("security".to_string(), item.stream_security.clone()));
    } else if let Some(default_value) = security_default {
        query.push(("security".to_string(), default_value.to_string()));
    }
    push_encoded_str(query, "sni", &item.sni);
    push_encoded_str(query, "fp", &item.fingerprint);
    push_encoded_str(query, "pbk", &item.public_key);
    push_encoded_str(query, "sid", &item.short_id);
    push_encoded_str(query, "spx", &item.spider_x);
    push_encoded_str(query, "pqv", &item.mldsa65_verify);

    if item.stream_security == STREAM_SECURITY_TLS {
        push_encoded_str(query, "alpn", &item.alpn);
        to_uri_query_allow_insecure(item, query);
    }
    push_encoded_str(query, "ech", &item.ech_config_list);
    push_encoded_str(query, "pcs", &item.cert_sha);
    if !item.finalmask.is_empty() {
        query.push((
            "fm".to_string(),
            url_encode(&compact_json_or_self(&item.finalmask)),
        ));
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
            query.push((
                "headerType".to_string(),
                nonempty_option(&transport.raw_header_type)
                    .unwrap_or(NONE)
                    .to_string(),
            ));
            push_encoded_opt(query, "host", &transport.host);
            push_encoded_opt(query, "path", &transport.path);
        }
        "kcp" => {
            query.push((
                "headerType".to_string(),
                nonempty_option(&transport.kcp_header_type)
                    .unwrap_or(NONE)
                    .to_string(),
            ));
            push_encoded_opt(query, "seed", &transport.kcp_seed);
            if let Some(mtu) = transport.kcp_mtu.filter(|value| *value > 0) {
                query.push(("mtu".to_string(), mtu.to_string()));
            }
        }
        "ws" | "httpupgrade" => {
            push_encoded_opt(query, "host", &transport.host);
            push_encoded_opt(query, "path", &transport.path);
        }
        "xhttp" => {
            push_encoded_opt(query, "host", &transport.host);
            push_encoded_opt(query, "path", &transport.path);
            if let Some(mode) = nonempty_option(&transport.xhttp_mode) {
                if XHTTP_MODES.contains(&mode) {
                    query.push(("mode".to_string(), url_encode(mode)));
                }
            }
            if let Some(extra) = nonempty_option(&transport.xhttp_extra) {
                query.push((
                    "extra".to_string(),
                    url_encode(&compact_json_or_self(extra)),
                ));
            }
        }
        "grpc" if nonempty_option(&transport.grpc_service_name).is_some() => {
            query.push((
                "authority".to_string(),
                url_encode(transport.grpc_authority.as_deref().unwrap_or("")),
            ));
            query.push((
                "serviceName".to_string(),
                url_encode(transport.grpc_service_name.as_deref().unwrap_or("")),
            ));
            if let Some(mode) = nonempty_option(&transport.grpc_mode) {
                if mode == GRPC_GUN_MODE || mode == GRPC_MULTI_MODE {
                    query.push(("mode".to_string(), url_encode(mode)));
                }
            }
        }
        _ => {}
    }
}

fn to_uri_query_lite(item: &ProfileItem, query: &mut QueryPairs) {
    push_encoded_str(query, "sni", &item.sni);
    push_encoded_str(query, "alpn", &item.alpn);
    to_uri_query_allow_insecure(item, query);
}

fn to_uri_query_allow_insecure(item: &ProfileItem, query: &mut QueryPairs) {
    let value = if item.allow_insecure == ALLOW_INSECURE_TRUE {
        "1"
    } else {
        "0"
    };
    query.push(("insecure".to_string(), value.to_string()));
    query.push(("allowInsecure".to_string(), value.to_string()));
}

fn resolve_uri_query(query: &Query, item: &mut ProfileItem) {
    item.stream_security = query.value_or("security", "");
    item.sni = query.value_or("sni", "");
    item.alpn = query.decoded_or("alpn", "");
    item.fingerprint = query.decoded_or("fp", "");
    item.public_key = query.decoded_or("pbk", "");
    item.short_id = query.decoded_or("sid", "");
    item.spider_x = query.decoded_or("spx", "");
    item.mldsa65_verify = query.decoded_or("pqv", "");
    item.ech_config_list = query.decoded_or("ech", "");
    item.cert_sha = query.decoded_or("pcs", "");

    let finalmask = query.decoded_or("fm", "");
    item.finalmask = if finalmask.is_empty() {
        String::new()
    } else {
        pretty_json_or_self(&finalmask)
    };

    if ["insecure", "allowInsecure", "allow_insecure"]
        .iter()
        .any(|key| query.decoded_or(key, "") == "1")
    {
        item.allow_insecure = ALLOW_INSECURE_TRUE.to_string();
    } else if ["insecure", "allowInsecure", "allow_insecure"]
        .iter()
        .any(|key| query.decoded_or(key, "") == "0")
    {
        item.allow_insecure = ALLOW_INSECURE_FALSE.to_string();
    } else {
        item.allow_insecure.clear();
    }

    let mut network = query.value_or("type", DEFAULT_NETWORK);
    if network == RAW_NETWORK_ALIAS {
        network = DEFAULT_NETWORK.to_string();
    }
    if !NETWORKS.contains(&network.as_str()) {
        network = DEFAULT_NETWORK.to_string();
    }
    item.network = network;

    match item.network.as_str() {
        "raw" => {
            item.transport_extra.raw_header_type = Some(query.value_or("headerType", NONE));
            item.transport_extra.host = Some(query.decoded_or("host", ""));
            item.transport_extra.path = Some(query.decoded_or("path", ""));
        }
        "kcp" => {
            item.transport_extra.kcp_header_type = Some(query.value_or("headerType", NONE));
            item.transport_extra.kcp_seed = Some(query.decoded_or("seed", ""));
            item.transport_extra.kcp_mtu = parse_positive_i32(&query.value_or("mtu", ""));
        }
        "ws" | "httpupgrade" => {
            item.transport_extra.host = Some(query.decoded_or("host", ""));
            item.transport_extra.path = Some(query.decoded_or("path", "/"));
        }
        "xhttp" => {
            let xhttp_extra = query.decoded_or("extra", "");
            item.transport_extra.host = Some(query.decoded_or("host", ""));
            item.transport_extra.path = Some(query.decoded_or("path", "/"));
            item.transport_extra.xhttp_mode = Some(query.decoded_or("mode", ""));
            item.transport_extra.xhttp_extra = Some(if xhttp_extra.is_empty() {
                String::new()
            } else {
                pretty_json_or_self(&xhttp_extra)
            });
        }
        "grpc" => {
            item.transport_extra.grpc_authority = Some(query.decoded_or("authority", ""));
            item.transport_extra.grpc_service_name = Some(query.decoded_or("serviceName", ""));
            item.transport_extra.grpc_mode = Some(query.decoded_or("mode", GRPC_GUN_MODE));
        }
        _ => {
            item.network = DEFAULT_NETWORK.to_string();
        }
    }
}

fn parse_singbox_custom(
    input: &str,
    sub_remarks: Option<&str>,
) -> Result<Option<CustomConfigImport>, ShareError> {
    let value = match serde_json::from_str::<Value>(input) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    if !(object.contains_key("inbounds")
        && object.contains_key("outbounds")
        && object.contains_key("route")
        && object.contains_key("dns"))
    {
        return Ok(None);
    }
    Ok(Some(custom_import(
        CustomConfigKind::SingBox,
        "json",
        input,
        sub_remarks.unwrap_or("singbox_custom"),
    )))
}

fn custom_import(
    kind: CustomConfigKind,
    extension: &str,
    contents: &str,
    remarks: &str,
) -> CustomConfigImport {
    CustomConfigImport {
        kind,
        extension: extension.to_string(),
        contents: contents.to_string(),
        profile: ProfileItem {
            config_type: ConfigType::Custom,
            address: String::new(),
            remarks: remarks.to_string(),
            ..ProfileItem::default()
        },
    }
}

fn export_inner_single(item: &ProfileItem) -> Result<String, ShareError> {
    let mut value = serde_json::to_value(item).map_err(|error| ShareError::InvalidInner {
        reason: error.to_string(),
    })?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ShareError::InvalidInner {
            reason: "profile must serialize to object".to_string(),
        })?;
    if let Some(protocol_extra) = object.remove("ProtocolExtra") {
        object.insert("ProtoExtraObj".to_string(), protocol_extra);
    }
    if let Some(transport_extra) = object.remove("TransportExtra") {
        object.insert("TransportExtraObj".to_string(), transport_extra);
    }
    object.remove("Subid");
    object.remove("IsSub");
    remove_empty_json(&mut value);
    let json = serde_json::to_string(&value).map_err(|error| ShareError::InvalidInner {
        reason: error.to_string(),
    })?;
    let encoded = base64_encode(&json, false)
        .replace('+', "-")
        .replace('/', "_")
        .replace('=', "");
    Ok(format!(
        "{}{}/{}",
        INNER_URI_PROTOCOL,
        config_type_name(item.config_type),
        encoded
    ))
}

fn parse_inner_single(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = Url::parse(input).map_err(|error| ShareError::InvalidInner {
        reason: error.to_string(),
    })?;
    if !parsed.scheme().eq_ignore_ascii_case("v2rayn") {
        return Err(ShareError::InvalidInner {
            reason: "invalid scheme".to_string(),
        });
    }
    let segment = parsed
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .ok_or_else(|| ShareError::InvalidInner {
            reason: "missing payload".to_string(),
        })?;
    let decoded = base64_decode(segment, "inner").map_err(|error| ShareError::InvalidInner {
        reason: error.to_string(),
    })?;
    let mut value: Value =
        serde_json::from_str(&decoded).map_err(|error| ShareError::InvalidInner {
            reason: error.to_string(),
        })?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ShareError::InvalidInner {
            reason: "profile JSON must be an object".to_string(),
        })?;
    if let Some(protocol_extra) = object.remove("ProtoExtraObj") {
        object.insert("ProtocolExtra".to_string(), protocol_extra);
    } else if let Some(protocol_extra) = object.remove("ProtoExtra") {
        if let Some(protocol_extra) = decode_json_string_value(protocol_extra)? {
            object.insert("ProtocolExtra".to_string(), protocol_extra);
        }
    }
    if let Some(transport_extra) = object.remove("TransportExtraObj") {
        object.insert("TransportExtra".to_string(), transport_extra);
    } else if let Some(transport_extra) = object.remove("TransportExtra") {
        if let Some(transport_extra) = decode_json_string_value(transport_extra)? {
            object.insert("TransportExtra".to_string(), transport_extra);
        }
    }
    let item: ProfileItem =
        serde_json::from_value(value).map_err(|error| ShareError::InvalidInner {
            reason: error.to_string(),
        })?;
    if item.config_version != 4 {
        return Err(ShareError::InvalidInner {
            reason: "unsupported config version".to_string(),
        });
    }
    if item.protocol_extra.multiple_load.is_some_and(|load| {
        !matches!(
            load,
            MultipleLoad::LeastPing
                | MultipleLoad::Fallback
                | MultipleLoad::Random
                | MultipleLoad::RoundRobin
                | MultipleLoad::LeastLoad
        )
    }) {
        return Err(ShareError::InvalidInner {
            reason: "unsupported multiple load".to_string(),
        });
    }
    Ok(item)
}

fn decode_json_string_value(value: Value) -> Result<Option<Value>, ShareError> {
    match value {
        Value::String(text) if !text.is_empty() => {
            serde_json::from_str(&text)
                .map(Some)
                .map_err(|error| ShareError::InvalidInner {
                    reason: error.to_string(),
                })
        }
        Value::Object(_) => Ok(Some(value)),
        _ => Ok(None),
    }
}

fn remove_empty_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for child in object.values_mut() {
                remove_empty_json(child);
            }
            object.retain(|_, child| !is_empty_json(child));
        }
        Value::Array(array) => {
            for child in array.iter_mut() {
                remove_empty_json(child);
            }
            array.retain(|child| !is_empty_json(child));
        }
        _ => {}
    }
}

fn is_empty_json(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(object) => object.is_empty(),
        _ => false,
    }
}

fn json_object<const N: usize>(items: [(&str, Value); N]) -> Value {
    Value::Object(
        items
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Map<_, _>>(),
    )
}

fn value_string(object: &Map<String, Value>, key: &str) -> String {
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

fn value_i32(object: &Map<String, Value>, key: &str) -> Option<i32> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_i64().and_then(|value| i32::try_from(value).ok()),
        Value::String(text) => text.parse().ok(),
        _ => None,
    })
}

fn string_field(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn parse_wireguard_endpoint(endpoint: &str) -> Option<(String, i32)> {
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
            .unwrap_or(2408);
        return Some((address, port));
    }

    if let Some((address, port_text)) = endpoint.rsplit_once(':') {
        if address.trim().is_empty() {
            return None;
        }
        if let Ok(port) = port_text.trim().parse::<i32>() {
            if (1..=65535).contains(&port) {
                return Some((address.trim().to_string(), port));
            }
        }
    }
    Some((endpoint.to_string(), 2408))
}

fn rsplit_host_port(input: &str) -> Option<(String, &str)> {
    let (host, port) = input.rsplit_once(':')?;
    let host = host.trim();
    if host.is_empty() {
        return None;
    }
    Some((host.trim_matches(['[', ']']).to_string(), port))
}

fn parse_port(protocol: &'static str, port: &str) -> Result<i32, ShareError> {
    let parsed = port.parse::<i32>().map_err(|_| ShareError::InvalidPort {
        protocol,
        port: port.to_string(),
    })?;
    if !(1..=65535).contains(&parsed) {
        return Err(ShareError::InvalidPort {
            protocol,
            port: port.to_string(),
        });
    }
    Ok(parsed)
}

fn parse_positive_i32(value: &str) -> Option<i32> {
    value.parse::<i32>().ok().filter(|value| *value > 0)
}

fn ensure_type(
    protocol: &'static str,
    item: &ProfileItem,
    expected: ConfigType,
) -> Result<(), ShareError> {
    if item.config_type == expected {
        Ok(())
    } else {
        Err(ShareError::WrongConfigType {
            protocol,
            actual: item.config_type,
        })
    }
}

fn ensure_address_port(protocol: &'static str, item: &ProfileItem) -> Result<(), ShareError> {
    ensure_nonempty(protocol, "address", &item.address)?;
    if !valid_host(&item.address) {
        return Err(ShareError::InvalidUri {
            protocol,
            reason: format!("invalid host {}", item.address),
        });
    }
    if !(1..=65535).contains(&item.port) {
        return Err(ShareError::InvalidPort {
            protocol,
            port: item.port.to_string(),
        });
    }
    Ok(())
}

fn ensure_nonempty(
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

fn valid_host(host: &str) -> bool {
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
    !domain.is_empty()
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn protocol_share(config_type: ConfigType) -> &'static str {
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
        _ => "",
    }
}

fn config_type_name(config_type: ConfigType) -> &'static str {
    match config_type {
        ConfigType::VMess => "vmess",
        ConfigType::Custom => "custom",
        ConfigType::Shadowsocks => "shadowsocks",
        ConfigType::SOCKS => "socks",
        ConfigType::VLESS => "vless",
        ConfigType::Trojan => "trojan",
        ConfigType::Hysteria2 => "hysteria2",
        ConfigType::TUIC => "tuic",
        ConfigType::WireGuard => "wireguard",
        ConfigType::HTTP => "http",
        ConfigType::Anytls => "anytls",
        ConfigType::Naive => "naive",
        ConfigType::PolicyGroup => "policygroup",
        ConfigType::ProxyChain => "proxychain",
    }
}

fn item_network(item: &ProfileItem) -> &str {
    if item.network.is_empty() || !NETWORKS.contains(&item.network.as_str()) {
        DEFAULT_NETWORK
    } else {
        item.network.trim()
    }
}

fn is_group_type(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::PolicyGroup | ConfigType::ProxyChain
    )
}

fn option_or(value: &Option<String>, default_value: &str) -> String {
    nonempty_option(value).unwrap_or(default_value).to_string()
}

fn nonempty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn nonempty_option(value: &Option<String>) -> Option<&str> {
    nonempty_str(value.as_deref())
}

fn nonempty_str(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

fn push_encoded_opt(query: &mut QueryPairs, key: &str, value: &Option<String>) {
    if let Some(value) = nonempty_option(value) {
        query.push((key.to_string(), url_encode(value)));
    }
}

fn push_encoded_str(query: &mut QueryPairs, key: &str, value: &str) {
    if !value.is_empty() {
        query.push((key.to_string(), url_encode(value)));
    }
}

fn format_query(query: &[(String, String)]) -> String {
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

fn ipv6_host(address: &str) -> String {
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

fn compact_json_or_self(input: &str) -> String {
    serde_json::from_str::<Value>(input)
        .and_then(|value| serde_json::to_string(&value))
        .unwrap_or_else(|_| input.to_string())
}

fn pretty_json_or_self(input: &str) -> String {
    serde_json::from_str::<Value>(input)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| input.to_string())
}

fn extract_first_pem_body(cert: &str) -> Option<String> {
    let begin = "-----BEGIN CERTIFICATE-----";
    let end = "-----END CERTIFICATE-----";
    let start = cert.find(begin)? + begin.len();
    let rest = &cert[start..];
    let finish = rest.find(end)?;
    Some(rest[..finish].trim().replace('\r', ""))
}

fn base64_encode(input: &str, remove_padding: bool) -> String {
    let mut encoded = STANDARD.encode(input.as_bytes());
    if remove_padding {
        encoded = encoded.trim_end_matches('=').to_string();
    }
    encoded
}

fn base64_decode(input: &str, protocol: &'static str) -> Result<String, ShareError> {
    if input.trim().len() > MAX_BASE64_DECODE_INPUT {
        return Err(ShareError::InvalidBase64 { protocol });
    }

    let mut normalized = input
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .replace('_', "/")
        .replace('-', "+");
    if normalized.len() % 4 != 0 {
        let pad = 4 - (normalized.len() % 4);
        normalized.extend(std::iter::repeat_n('=', pad));
    }
    let bytes = STANDARD
        .decode(normalized.as_bytes())
        .map_err(|_| ShareError::InvalidBase64 { protocol })?;
    String::from_utf8(bytes).map_err(|_| ShareError::InvalidBase64 { protocol })
}

fn url_encode(input: &str) -> String {
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

fn url_decode(input: &str) -> String {
    percent_decode_str(input).decode_utf8_lossy().into_owned()
}

fn split_csv(input: &str) -> Vec<String> {
    input
        .replace(['\r', '\n'], "")
        .split(',')
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>()
}

fn starts_with_ci(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn contains_all_ci(value: &str, needles: &[&str]) -> bool {
    let lower = value.to_ascii_lowercase();
    needles
        .iter()
        .all(|needle| lower.contains(&needle.to_ascii_lowercase()))
}

fn is_html_page(value: &str) -> bool {
    contains_all_ci(value, &["<html", "<!doctype html", "<head"])
}

trait StripPrefixCi {
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

#[cfg(test)]
mod share_tests {
    use std::{collections::BTreeMap, panic};

    use proptest::prelude::*;

    use super::*;
    use crate::{generate_singbox_config_value, AppConfig, CoreConfigContext, CoreType, PROXY_TAG};

    #[test]
    fn fmt_share_round_trips_all_supported_protocols() {
        for source in sample_profiles() {
            let uri = export_share_link(&source).expect("export share link");
            let parsed = parse_share_link(&uri).expect("parse exported share link");

            assert_eq!(parsed.config_type, source.config_type, "{uri}");
            assert_eq!(parsed.address, source.address, "{uri}");
            assert_eq!(parsed.port, source.port, "{uri}");
            assert_eq!(parsed.remarks, source.remarks, "{uri}");
            assert_eq!(parsed.password, source.password, "{uri}");
            assert_eq!(parsed.username, source.username, "{uri}");
        }
    }

    #[test]
    fn fmt_base_query_round_trips_transport_security_and_masks() {
        let source = ProfileItem {
            config_type: ConfigType::VLESS,
            remarks: "advanced vless".to_string(),
            address: "vless.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000001".to_string(),
            network: "xhttp".to_string(),
            stream_security: "reality".to_string(),
            sni: "sni.example".to_string(),
            fingerprint: "chrome".to_string(),
            public_key: "public-key".to_string(),
            short_id: "abcd".to_string(),
            spider_x: "/spider".to_string(),
            mldsa65_verify: "pqv-token".to_string(),
            ech_config_list: "https://ech.example/config".to_string(),
            cert_sha: "sha256-pin".to_string(),
            finalmask: r#"{"tcp":{"fragment":{"packets":"tlshello"}}}"#.to_string(),
            protocol_extra: ProtocolExtraItem {
                vless_encryption: Some(NONE.to_string()),
                flow: Some("xtls-rprx-vision".to_string()),
                ..ProtocolExtraItem::default()
            },
            transport_extra: TransportExtraItem {
                host: Some("cdn.example".to_string()),
                path: Some("/xhttp".to_string()),
                xhttp_mode: Some("stream-one".to_string()),
                xhttp_extra: Some(r#"{"downloadSettings":{"address":"cdn2.example"}}"#.to_string()),
                ..TransportExtraItem::default()
            },
            ..ProfileItem::default()
        };

        let uri = export_share_link(&source).expect("export advanced vless");
        let parsed = parse_share_link(&uri).expect("parse advanced vless");

        assert_eq!(parsed.stream_security, "reality");
        assert_eq!(parsed.network, "xhttp");
        assert_eq!(parsed.mldsa65_verify, "pqv-token");
        assert_eq!(parsed.ech_config_list, "https://ech.example/config");
        assert_eq!(parsed.cert_sha, "sha256-pin");
        assert!(parsed.finalmask.contains("\"fragment\""));
        assert_eq!(
            parsed.transport_extra.xhttp_mode.as_deref(),
            Some("stream-one")
        );
        assert!(parsed
            .transport_extra
            .xhttp_extra
            .as_deref()
            .unwrap_or_default()
            .contains("downloadSettings"));
    }

    #[test]
    fn fmt_query_parser_preserves_values_containing_equals() {
        let parsed = parse_share_link(
            "vless://00000000-0000-0000-0000-000000000001@example.com:443?encryption=none&type=xhttp&extra=left=right==#eq",
        )
        .expect("parse vless with equals in query value");

        assert_eq!(parsed.network, "xhttp");
        assert_eq!(
            parsed.transport_extra.xhttp_extra.as_deref(),
            Some("left=right==")
        );
    }

    #[test]
    fn fmt_hostile_subscription_tls_flags_are_not_trusted_by_generators() {
        let mut node = parse_share_link(
            "vless://00000000-0000-0000-0000-000000000099@hostile.example:443?encryption=none&security=tls&type=ws&host=cdn.example&path=/ws&insecure=1&fp=definitely-not-utls#hostile",
        )
        .expect("parse hostile vless share");
        node.index_id = "hostile-vless".to_string();

        assert_eq!(node.allow_insecure, ALLOW_INSECURE_TRUE);
        assert_eq!(node.fingerprint, "definitely-not-utls");

        let mut app_config = AppConfig::default();
        app_config.core_basic_item.def_fingerprint = "firefox".to_string();

        let singbox_value =
            generate_singbox_config_value(&fmt_test_context(CoreType::sing_box, app_config, node))
                .expect("sing-box config should generate");
        let singbox_proxy = proxy_outbound(&singbox_value);
        assert_eq!(
            singbox_proxy
                .pointer("/tls/insecure")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            singbox_proxy
                .pointer("/tls/utls/fingerprint")
                .and_then(Value::as_str),
            Some("firefox")
        );
    }

    #[test]
    fn fmt_negative_inputs_return_typed_errors_without_panicking() {
        for bad in [
            "",
            "not-a-share-uri",
            "vmess://%%%%",
            "vless://uuid@example.com",
            "ss://not-base64",
            "wireguard://key@example.com:notaport",
            "tuic://onlyuser@example.com:443",
            "v2rayn://vless/not-base64",
        ] {
            let result = panic::catch_unwind(|| {
                if starts_with_ci(bad, INNER_URI_PROTOCOL) {
                    parse_inner_share_links(bad, "sub").map(|_| ())
                } else {
                    parse_share_link(bad).map(|_| ())
                }
            });
            assert!(result.is_ok(), "{bad} panicked");
            assert!(result.expect("panic checked").is_err(), "{bad} parsed");
        }
    }

    #[test]
    fn fmt_negative_inputs_cover_port_host_and_large_base64_edges() {
        let bad_port_vmess = base64_encode(
            r#"{"v":"2","ps":"bad-port","add":"example.com","port":"70000","id":"00000000-0000-0000-0000-000000000012"}"#,
            false,
        );
        let bad_host_vmess = base64_encode(
            r#"{"v":"2","ps":"bad-host","add":"bad=host","port":"443","id":"00000000-0000-0000-0000-000000000013"}"#,
            false,
        );
        let oversized_vmess = format!("vmess://{}", "A".repeat(MAX_BASE64_DECODE_INPUT + 4));
        let bad_inputs = vec![
            "vless://00000000-0000-0000-0000-000000000014@example.com:0?encryption=none"
                .to_string(),
            "vless://00000000-0000-0000-0000-000000000014@example.com:65536?encryption=none"
                .to_string(),
            "vless://00000000-0000-0000-0000-000000000014@bad=host:443?encryption=none".to_string(),
            format!("vmess://{bad_port_vmess}"),
            format!("vmess://{bad_host_vmess}"),
            oversized_vmess,
        ];

        for bad in bad_inputs {
            let result = panic::catch_unwind(|| parse_share_link(&bad));
            assert!(result.is_ok(), "{bad} panicked");
            assert!(result.expect("panic checked").is_err(), "{bad} parsed");
        }
    }

    #[test]
    fn fmt_shadowsocks_legacy_and_plugins_parse() {
        let legacy_payload = base64_encode("aes-128-gcm:pass@example.com:8388", false);
        let legacy = format!("ss://{legacy_payload}#legacy");
        let parsed = parse_share_link(&legacy).expect("parse legacy ss");
        assert_eq!(parsed.config_type, ConfigType::Shadowsocks);
        assert_eq!(
            parsed.protocol_extra.ss_method.as_deref(),
            Some("aes-128-gcm")
        );

        let plugin =
            url_encode("v2ray-plugin;mode=websocket;host=ws.example;path=/a\\=b\\,c;tls;mux=0");
        let sip002 = format!(
            "ss://{}@example.com:8388?plugin={plugin}#plugin",
            base64_encode("aes-256-gcm:pass", true)
        );
        let parsed = parse_share_link(&sip002).expect("parse plugin ss");
        assert_eq!(parsed.network, "ws");
        assert_eq!(parsed.stream_security, STREAM_SECURITY_TLS);
        assert_eq!(parsed.transport_extra.path.as_deref(), Some("/a=b,c"));
    }

    #[test]
    fn fmt_parses_common_multiline_import_shapes() {
        let vmess_json = r#"{
            "v": "2",
            "ps": "JMS-TEST@example.test:17701",
            "add": "node-vmess.example.test",
            "port": "17701",
            "id": "00000000-0000-0000-0000-000000000001",
            "aid": "0",
            "scy": "auto",
            "net": "tcp",
            "type": "none",
            "host": "",
            "path": "",
            "tls": "",
            "sni": "",
            "alpn": "",
            "fp": "",
            "insecure": "0"
        }"#;
        let vmess = format!("vmess://{}", base64_encode(vmess_json, false));
        let parsed = parse_share_link(&vmess).expect("parse vmess base64 json");
        assert_eq!(parsed.config_type, ConfigType::VMess);
        assert_eq!(parsed.remarks, "JMS-TEST@example.test:17701");
        assert_eq!(parsed.address, "node-vmess.example.test");
        assert_eq!(parsed.port, 17701);
        assert_eq!(parsed.network, DEFAULT_NETWORK);
        assert_eq!(
            parsed.protocol_extra.vmess_security.as_deref(),
            Some(DEFAULT_SECURITY)
        );

        let paddingless_vmess = format!("vmess://{}", base64_encode(vmess_json, true));
        let parsed = parse_share_link(&paddingless_vmess).expect("parse paddingless vmess");
        assert_eq!(parsed.address, "node-vmess.example.test");

        let vless = "vless://00000000-0000-0000-0000-000000000002@node-vless.example.test:443?encryption=none&security=tls&sni=node-vless.example.test&fp=randomized&insecure=0&allowInsecure=0&type=ws&host=node-vless.example.test&path=%2F%3Fed%3D2048#node-vless.example.test";
        let parsed = parse_share_link(vless).expect("parse vless ws tls");
        assert_eq!(parsed.config_type, ConfigType::VLESS);
        assert_eq!(parsed.address, "node-vless.example.test");
        assert_eq!(parsed.network, "ws");
        assert_eq!(parsed.stream_security, STREAM_SECURITY_TLS);
        assert_eq!(
            parsed.transport_extra.host.as_deref(),
            Some("node-vless.example.test")
        );
        assert_eq!(parsed.transport_extra.path.as_deref(), Some("/?ed=2048"));

        let ss_user_info = base64_encode("aes-256-gcm:test-password", true);
        let ss = format!(
            "ss://{ss_user_info}@node-ss.example.test:17701?#JMS-TEST%40node-ss.example.test%3A17701"
        );
        let parsed = parse_share_link(&ss).expect("parse sip002 ss with empty query");
        assert_eq!(parsed.config_type, ConfigType::Shadowsocks);
        assert_eq!(parsed.address, "node-ss.example.test");
        assert_eq!(parsed.port, 17701);
        assert_eq!(parsed.remarks, "JMS-TEST@node-ss.example.test:17701");
        assert_eq!(
            parsed.protocol_extra.ss_method.as_deref(),
            Some("aes-256-gcm")
        );
    }

    #[test]
    fn fmt_wireguard_config_parses_peers_and_inline_comments() {
        let config = r#"
            [Interface]
            PrivateKey = interface-private-key
            Address = 10.0.0.2/32, fd00::2/128 ; inline comment
            MTU = 1420

            [Peer]
            PublicKey = peer-public-key
            PresharedKey = peer-preshared-key
            AllowedIPs = 10.0.0.0/8, 192.168.0.0/16
            Reserved = 1, 2, 3 # inline comment
            Endpoint = [2001:db8::1]:51820 # inline comment

            [Peer]
            PublicKey = peer-public-key-2
            Endpoint = example.com:12345
        "#;

        let resolved = parse_wireguard_config(config).expect("wireguard config");
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].address, "2001:db8::1");
        assert_eq!(resolved[0].port, 51820);
        assert_eq!(resolved[0].password, "interface-private-key");
        assert_eq!(
            resolved[0].protocol_extra.wg_reserved.as_deref(),
            Some("1, 2, 3")
        );
        assert_eq!(
            resolved[0].protocol_extra.wg_allowed_ips.as_deref(),
            Some("10.0.0.0/8, 192.168.0.0/16")
        );
        assert_eq!(
            resolved[0].protocol_extra.wg_interface_address.as_deref(),
            Some("10.0.0.2/32, fd00::2/128")
        );
        assert_eq!(resolved[0].protocol_extra.wg_mtu, Some(1420));
        assert_eq!(resolved[1].address, "example.com");
        assert_eq!(resolved[1].port, 12345);
    }

    #[test]
    fn share_inner_format_round_trips_group_references() {
        let child_a = ProfileItem {
            index_id: "child-a".to_string(),
            config_type: ConfigType::SOCKS,
            remarks: "child-a".to_string(),
            address: "127.0.0.1".to_string(),
            port: 1080,
            username: "u".to_string(),
            password: "p".to_string(),
            ..ProfileItem::default()
        };
        let child_b = ProfileItem {
            index_id: "child-b".to_string(),
            config_type: ConfigType::VMess,
            remarks: "child-b".to_string(),
            address: "vmess.example".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000002".to_string(),
            ..ProfileItem::default()
        };
        let group = ProfileItem {
            index_id: "group-1".to_string(),
            config_type: ConfigType::PolicyGroup,
            remarks: "group-1".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some("child-a,child-b".to_string()),
                sub_child_items: Some("original-sub".to_string()),
                multiple_load: Some(MultipleLoad::LeastPing),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };

        let uri = export_inner_share_links(&[group, child_a, child_b]).expect("export inner");
        let resolved = parse_inner_share_links(&uri, "sub-123").expect("parse inner");
        assert_eq!(resolved.len(), 3);
        let resolved_group = resolved
            .iter()
            .find(|item| item.remarks == "group-1")
            .expect("resolved group");
        assert_eq!(
            resolved_group.protocol_extra.sub_child_items.as_deref(),
            Some("sub-123")
        );
        let child_ids = resolved_group
            .protocol_extra
            .child_items
            .as_deref()
            .unwrap_or_default();
        assert!(child_ids.contains("inner-import-2"));
        assert!(child_ids.contains("inner-import-3"));
    }

    #[test]
    fn share_full_custom_import_helpers_classify_configs_without_file_writes() {
        let unsupported =
            r#"{"remarks":"legacy custom","inbounds":[],"outbounds":[],"routing":{}}"#;
        assert!(parse_full_custom_config(unsupported, None).is_err());

        let singbox_array = r#"[{"inbounds":[],"outbounds":[],"route":{},"dns":{}}]"#;
        let imports =
            parse_full_custom_config(singbox_array, Some("sub")).expect("singbox array custom");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].kind, CustomConfigKind::SingBox);

        let html = "<!doctype html><html><head></head></html>";
        assert!(parse_full_custom_config(html, None).is_err());
    }

    proptest! {
        #[test]
        fn share_url_component_property_round_trips(value in "[A-Za-z0-9 _./:@,=+\\-]{0,80}") {
            prop_assert_eq!(url_decode(&url_encode(&value)), value);
        }

        #[test]
        fn share_base64_property_round_trips(value in "[A-Za-z0-9 _./:@,=+\\-]{0,80}") {
            let encoded = base64_encode(&value, true);
            prop_assert_eq!(base64_decode(&encoded, "test").expect("decode"), value);
        }
    }

    fn sample_profiles() -> Vec<ProfileItem> {
        vec![
            ProfileItem {
                config_type: ConfigType::VMess,
                remarks: "vmess demo".to_string(),
                address: "example.com".to_string(),
                port: 443,
                password: "00000000-0000-0000-0000-000000000003".to_string(),
                network: DEFAULT_NETWORK.to_string(),
                protocol_extra: ProtocolExtraItem {
                    alter_id: Some("0".to_string()),
                    vmess_security: Some(DEFAULT_SECURITY.to_string()),
                    ..ProtocolExtraItem::default()
                },
                transport_extra: TransportExtraItem {
                    raw_header_type: Some(NONE.to_string()),
                    ..TransportExtraItem::default()
                },
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::VLESS,
                remarks: "vless demo".to_string(),
                address: "vless.example".to_string(),
                port: 8443,
                password: "00000000-0000-0000-0000-000000000004".to_string(),
                network: "ws".to_string(),
                stream_security: STREAM_SECURITY_TLS.to_string(),
                allow_insecure: ALLOW_INSECURE_TRUE.to_string(),
                sni: "vless.example".to_string(),
                alpn: "h2,http/1.1".to_string(),
                protocol_extra: ProtocolExtraItem {
                    vless_encryption: Some(NONE.to_string()),
                    ..ProtocolExtraItem::default()
                },
                transport_extra: TransportExtraItem {
                    host: Some("vless.example".to_string()),
                    path: Some("/ws".to_string()),
                    ..TransportExtraItem::default()
                },
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::Trojan,
                remarks: "trojan demo".to_string(),
                address: "trojan.example".to_string(),
                port: 443,
                password: "trojan-pass".to_string(),
                network: "grpc".to_string(),
                stream_security: STREAM_SECURITY_TLS.to_string(),
                protocol_extra: ProtocolExtraItem {
                    flow: Some("xtls-rprx-vision".to_string()),
                    ..ProtocolExtraItem::default()
                },
                transport_extra: TransportExtraItem {
                    grpc_authority: Some("trojan.example".to_string()),
                    grpc_service_name: Some("svc".to_string()),
                    grpc_mode: Some(GRPC_MULTI_MODE.to_string()),
                    ..TransportExtraItem::default()
                },
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::Shadowsocks,
                remarks: "ss demo".to_string(),
                address: "1.2.3.4".to_string(),
                port: 8388,
                password: "pass123".to_string(),
                network: DEFAULT_NETWORK.to_string(),
                protocol_extra: ProtocolExtraItem {
                    ss_method: Some("aes-128-gcm".to_string()),
                    ..ProtocolExtraItem::default()
                },
                transport_extra: TransportExtraItem {
                    raw_header_type: Some(NONE.to_string()),
                    ..TransportExtraItem::default()
                },
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::SOCKS,
                remarks: "socks demo".to_string(),
                address: "127.0.0.1".to_string(),
                port: 1080,
                username: "user".to_string(),
                password: "pass".to_string(),
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::Hysteria2,
                remarks: "hy2 demo".to_string(),
                address: "hy2.example".to_string(),
                port: 443,
                password: "hy2-pass".to_string(),
                sni: "hy2.example".to_string(),
                cert_sha: "sha-pin,second".to_string(),
                protocol_extra: ProtocolExtraItem {
                    salamander_pass: Some("obfs-pass".to_string()),
                    ports: Some("1000:2000".to_string()),
                    ..ProtocolExtraItem::default()
                },
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::TUIC,
                remarks: "tuic demo".to_string(),
                address: "tuic.example".to_string(),
                port: 443,
                username: "uuid".to_string(),
                password: "tuic-pass".to_string(),
                protocol_extra: ProtocolExtraItem {
                    congestion_control: Some("bbr".to_string()),
                    ..ProtocolExtraItem::default()
                },
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::WireGuard,
                remarks: "wg demo".to_string(),
                address: "2001:db8::1".to_string(),
                port: 51820,
                password: "private-key".to_string(),
                protocol_extra: ProtocolExtraItem {
                    wg_public_key: Some("public-key".to_string()),
                    wg_preshared_key: Some("psk".to_string()),
                    wg_reserved: Some("1,2,3".to_string()),
                    wg_interface_address: Some("10.0.0.2/32".to_string()),
                    wg_mtu: Some(1420),
                    ..ProtocolExtraItem::default()
                },
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::Anytls,
                remarks: "anytls demo".to_string(),
                address: "anytls.example".to_string(),
                port: 443,
                password: "anytls-pass".to_string(),
                stream_security: STREAM_SECURITY_TLS.to_string(),
                sni: "anytls.example".to_string(),
                ..ProfileItem::default()
            },
            ProfileItem {
                config_type: ConfigType::Naive,
                remarks: "naive demo".to_string(),
                address: "naive.example".to_string(),
                port: 443,
                username: "user".to_string(),
                password: "pass".to_string(),
                protocol_extra: ProtocolExtraItem {
                    naive_quic: Some(true),
                    insecure_concurrency: Some(4),
                    ..ProtocolExtraItem::default()
                },
                ..ProfileItem::default()
            },
        ]
    }

    fn fmt_test_context(
        run_core_type: CoreType,
        app_config: AppConfig,
        node: ProfileItem,
    ) -> CoreConfigContext {
        let mut all_proxies_map = BTreeMap::new();
        all_proxies_map.insert(node.index_id.clone(), node.clone());
        let simple_dns_item = app_config.simple_dns_item.clone();
        CoreConfigContext {
            node,
            run_core_type,
            app_config,
            simple_dns_item,
            all_proxies_map,
            ..CoreConfigContext::default()
        }
    }

    fn proxy_outbound(config: &Value) -> &Value {
        config
            .get("outbounds")
            .and_then(Value::as_array)
            .and_then(|outbounds| {
                outbounds
                    .iter()
                    .find(|outbound| outbound.get("tag").and_then(Value::as_str) == Some(PROXY_TAG))
            })
            .expect("proxy outbound should be generated")
    }
}
