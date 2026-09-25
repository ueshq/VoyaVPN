//! Shared node and context factories for the config-generation test suites.
//!
//! Each suite used to hand-roll its own copy of these fixtures with slightly
//! drifted signatures; the canonical shapes live here and the suites keep only
//! thin ergonomic wrappers where their call sites read better with defaults.

use std::collections::BTreeMap;

use crate::{
    AppConfig, CoreConfigContext, CoreGenPlatform, ProfileItem, ProfileProtocol, ProfileTransport,
    ServerEndpoint, TlsMode, TlsSettings, LOOPBACK,
};

pub(crate) fn endpoint(address: &str, port: i32) -> ServerEndpoint {
    ServerEndpoint {
        address: address.to_string(),
        port,
    }
}

pub(crate) fn raw_transport() -> ProfileTransport {
    ProfileTransport::Tcp {
        header: None,
        host: None,
        path: None,
    }
}

pub(crate) fn tls_settings(
    mode: TlsMode,
    server_name: Option<&str>,
    alpn: &[&str],
    ech_config: Vec<String>,
) -> TlsSettings {
    TlsSettings {
        mode,
        server_name: server_name.map(str::to_string),
        alpn: alpn.iter().map(|value| (*value).to_string()).collect(),
        reality_public_key: None,
        reality_short_id: None,
        certificate_pem: None,
        ech_config,
    }
}

/// A `CoreConfigContext` on the Linux platform with `node` as the only proxy.
pub(crate) fn linux_context(app_config: AppConfig, node: ProfileItem) -> CoreConfigContext {
    let mut all_proxies_map = BTreeMap::new();
    all_proxies_map.insert(node.index_id.clone(), node.clone());
    let dns = app_config.dns.clone();
    CoreConfigContext {
        node,
        app_config,
        dns,
        all_proxies_map,
        platform: CoreGenPlatform::Linux,
        ..CoreConfigContext::default()
    }
}

/// The remote VMess node every suite bases its outbound fixtures on.
pub(crate) fn base_remote_node() -> ProfileItem {
    ProfileItem {
        remarks: "remote".to_string(),
        protocol: ProfileProtocol::Vmess {
            server: endpoint("server.example", 443),
            uuid: String::new(),
            cipher: None,
        },
        transport: Some(raw_transport()),
        tls: Some(tls_settings(
            TlsMode::Tls,
            Some("server.example"),
            &[],
            Vec::new(),
        )),
        ..ProfileItem::default()
    }
}

pub(crate) fn socks_node(index_id: &str, remarks: &str) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        remarks: remarks.to_string(),
        protocol: ProfileProtocol::Socks {
            server: endpoint(LOOPBACK, 1080),
            username: "user".to_string(),
            password: "pass".to_string(),
        },
        transport: Some(raw_transport()),
        ..ProfileItem::default()
    }
}
