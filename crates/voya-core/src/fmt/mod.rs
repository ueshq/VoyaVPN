//! Share-link parsers and exporters.
//!
//! Standard protocol links remain interoperable. Voya-only profile groups are
//! exchanged through the versioned `voya://profiles/v1/` bundle contract.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use percent_encoding::percent_decode_str;
use serde_json::{Map, Value};
use url::Url;

use crate::{
    ConfigType, MultipleLoad, ProfileItem, ProfileProtocol, ProfileTransport, ServerEndpoint,
    TlsMode, TlsSettings,
};

const DEFAULT_SECURITY: &str = "auto";
const DEFAULT_NETWORK: &str = "raw";
const RAW_NETWORK_ALIAS: &str = "tcp";
const RAW_HEADER_HTTP: &str = "http";
const NONE: &str = "none";
const STREAM_SECURITY_TLS: &str = "tls";
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
    "xhttp",
    "kcp",
    "grpc",
    "ws",
    "httpupgrade",
    HTTP2_NETWORK,
    QUIC_NETWORK,
];
const XHTTP_MODES: &[&str] = &["auto", "packet-up", "stream-up", "stream-one"];

mod anytls;
mod api;
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

pub use anytls::AnytlsFmt;
pub use api::{CustomConfigImport, CustomConfigKind, ShareError, ShareFmt};
pub use entry::{
    export_share_link, export_share_link_with_options, export_voya_profile_bundle,
    parse_full_custom_config, parse_share_lines, parse_share_link, parse_voya_profile_bundle,
    ShareLinkOptions,
};
pub use hysteria2::Hysteria2Fmt;
pub use naive::NaiveFmt;
pub use shadowsocks::{parse_ss_sip008, ShadowsocksFmt};
pub use socks::SocksFmt;
pub use trojan::TrojanFmt;
pub use tuic::TuicFmt;
pub use vless::VlessFmt;
pub use vmess::VmessFmt;
pub use wireguard::{parse_wireguard_config, WireguardFmt};

#[cfg(test)]
mod tests;
