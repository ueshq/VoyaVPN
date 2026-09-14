//! Share-link parsers and exporters.
//!
//! Standard protocol links can also be exchanged as a versioned node bundle.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use percent_encoding::percent_decode_str;
use serde_json::{Map, Value};
use thiserror::Error;
use url::Url;

use crate::{
    host::is_dns_label,
    protocol_common::{shadowsocks_plugin_for, split_csv, DEFAULT_SECURITY, RAW_HEADER_HTTP},
    text::{decode_base64_text, nonempty_str},
    ConfigType, ProfileItem, ProfileProtocol, ProfileTransport, ServerEndpoint, TlsMode,
    TlsSettings, DEFAULT_NETWORK, STREAM_SECURITY_TLS,
};

const RAW_NETWORK_ALIAS: &str = "tcp";
const NONE: &str = "none";
const STREAM_SECURITY_REALITY: &str = "reality";
const GRPC_GUN_MODE: &str = "gun";
const GRPC_MULTI_MODE: &str = "multi";
const HYSTERIA2_DEFAULT_SCHEME: &str = "hysteria2://";
const HYSTERIA2_ALT_SCHEME: &str = "hy2://";
const NAIVE_HTTPS_SCHEME: &str = "naive+https://";
const NAIVE_QUIC_SCHEME: &str = "naive+quic://";
const VOYA_PROFILE_BUNDLE_PREFIX: &str = "voya://profiles/v1/";
const MAX_BASE64_DECODE_INPUT: usize = 1024 * 1024;

const HTTP2_NETWORK: &str = "h2";
/// Xray still names the HTTP/2 transport `http` in share links.
const HTTP2_NETWORK_ALIAS: &str = "http";
const QUIC_NETWORK: &str = "quic";

const NETWORKS: &[&str] = &[
    "raw",
    "grpc",
    "ws",
    "httpupgrade",
    HTTP2_NETWORK,
    QUIC_NETWORK,
];
/// Transports sing-box cannot carry. A share link naming one is rejected
/// outright instead of importing a node that could never connect.
const RETIRED_NETWORKS: &[&str] = &["xhttp", "splithttp", "kcp", "mkcp"];

mod anytls;
mod common;
mod entry;
mod hysteria2;
mod naive;
mod shadowsocks;
mod socks;
mod trojan;
mod tuic;
mod uri;
mod vless;
mod vmess;
mod wireguard;

use common::*;
use uri::*;

pub use entry::{
    export_share_link_with_options, parse_share_link, parse_voya_profile_bundle, ShareLinkOptions,
};
pub use shadowsocks::parse_ss_sip008;
pub use wireguard::parse_wireguard_config;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShareError {
    #[error("share link is empty")]
    EmptyInput,
    #[error("unsupported share protocol")]
    UnsupportedProtocol,
    #[error("unsupported transport {transport}")]
    UnsupportedTransport { transport: String },
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
    #[error("invalid WireGuard config")]
    InvalidWireGuardConfig,
    #[error("invalid Voya node bundle: {reason}")]
    InvalidVoyaBundle { reason: String },
}

#[cfg(test)]
mod tests;
