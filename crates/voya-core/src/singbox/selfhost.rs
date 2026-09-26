//! The sing-box config of a self-hosted exit node, and the client profiles its
//! share links carry.
//!
//! A self-hosted node runs as its own core beside the connection core, so its
//! config holds only the server inbounds, the `direct` outbound, and the rules
//! that keep peers off this device and its network. Keys, ports, and addresses
//! are injected: nothing here draws randomness or looks at the machine.

use crate::{
    ProfileItem, ProfileProtocol, ProfileTransport, TlsMode, TlsSettings, DIRECT_TAG, LOOPBACK,
};

use super::{
    singbox_log_level, SingboxClashApi, SingboxConfig, SingboxConfigError, SingboxDns,
    SingboxDnsServer, SingboxExperimental, SingboxInbound, SingboxInboundTls, SingboxLog,
    SingboxRealityHandshake, SingboxRealityServer, SingboxRoute, SingboxRule, SingboxUser,
};

pub const SELFHOST_VLESS_INBOUND_TAG: &str = "selfhost-vless";
pub const SELFHOST_SHADOWSOCKS_INBOUND_TAG: &str = "selfhost-ss";
pub const SELFHOST_VLESS_FLOW: &str = "xtls-rprx-vision";
/// The one Shadowsocks cipher a self-hosted node offers. Its key is 16 random
/// bytes in standard base64.
pub const SELFHOST_SHADOWSOCKS_METHOD: &str = "2022-blake3-aes-128-gcm";
/// Dual-stack wildcard: sing-box accepts IPv4 peers on it as well.
const SELFHOST_LISTEN: &str = "::";
const SELFHOST_DNS_TAG: &str = "local_dns";
const SELFHOST_VLESS_USER: &str = "voya";
const VLESS_ENCRYPTION_NONE: &str = "none";

/// Destinations a peer can never reach through the node, whatever the LAN
/// setting: this device itself, link-local space (which holds the cloud
/// instance metadata service at 169.254.169.254), multicast, broadcast, and
/// the metadata endpoints that live outside link-local space.
const SELFHOST_ALWAYS_DENIED_CIDRS: &[&str] = &[
    "0.0.0.0/8",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "224.0.0.0/4",
    "255.255.255.255/32",
    "100.100.100.200/32",
    "::/128",
    "::1/128",
    "fe80::/10",
    "ff00::/8",
    "fd00:ec2::254/128",
];

/// Shared address space (carrier-grade NAT, and overlay networks such as
/// Tailscale). sing-box's `ip_is_private` leaves it out, so the node treats it
/// as part of the local network explicitly.
const SELFHOST_SHARED_ADDRESS_CIDR: &str = "100.64.0.0/10";

/// Everything the self-hosted core's config is generated from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHostSpec {
    pub vless: Option<SelfHostVlessSpec>,
    pub shadowsocks: Option<SelfHostShadowsocksSpec>,
    /// Lets peers reach private addresses (RFC 1918, ULA, shared address
    /// space). This device's own loopback stays closed either way.
    pub allow_lan_access: bool,
    pub block_bittorrent: bool,
    pub clash_api: Option<SelfHostClashApi>,
    pub log_level: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHostVlessSpec {
    pub port: i32,
    pub uuid: String,
    /// base64url X25519 private key, as `sing-box generate reality-keypair`
    /// prints it.
    pub reality_private_key: String,
    pub reality_short_id: String,
    /// The public site whose TLS handshake the node borrows.
    pub reality_server_name: String,
    pub reality_server_port: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHostShadowsocksSpec {
    pub port: i32,
    pub password: String,
}

/// The loopback Clash API the app reads the node's traffic through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHostClashApi {
    pub port: i32,
    pub secret: String,
}

/// One address the node's share links point at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHostEndpoint {
    pub address: String,
    /// Last part of the link's remark: `IPv4`, `IPv6`, or the custom host.
    pub label: String,
}

impl SelfHostSpec {
    #[must_use]
    pub fn has_inbound(&self) -> bool {
        self.vless.is_some() || self.shadowsocks.is_some()
    }
}

#[must_use]
pub fn generate_singbox_selfhost_config(spec: &SelfHostSpec) -> SingboxConfig {
    let mut config = SingboxConfig::sample();
    let mut log = SingboxLog {
        level: singbox_log_level(&spec.log_level).to_string(),
        ..SingboxLog::default()
    };
    if spec.log_level.trim().eq_ignore_ascii_case("none") {
        log.disabled = Some(true);
    }
    config.log = Some(log);

    if let Some(vless) = &spec.vless {
        config.inbounds.push(vless_server_inbound(vless));
    }
    if let Some(shadowsocks) = &spec.shadowsocks {
        config
            .inbounds
            .push(shadowsocks_server_inbound(shadowsocks));
    }

    config.dns = Some(SingboxDns {
        servers: vec![SingboxDnsServer {
            r#type: "local".to_string(),
            tag: SELFHOST_DNS_TAG.to_string(),
            ..SingboxDnsServer::default()
        }],
        final_server: Some(SELFHOST_DNS_TAG.to_string()),
        ..SingboxDns::default()
    });
    config.route = SingboxRoute {
        default_domain_resolver: Some(SingboxRule {
            server: Some(SELFHOST_DNS_TAG.to_string()),
            ..SingboxRule::default()
        }),
        // Binds egress to the physical interface, so relayed traffic leaves
        // directly even while this device's own tunnel owns the default route.
        auto_detect_interface: Some(true),
        rules: selfhost_route_rules(spec),
        rule_set: None,
        final_outbound: Some(DIRECT_TAG.to_string()),
    };
    config.experimental = spec.clash_api.as_ref().map(|api| SingboxExperimental {
        cache_file: None,
        clash_api: Some(SingboxClashApi {
            external_controller: Some(format!("{LOOPBACK}:{}", api.port)),
            secret: Some(api.secret.clone()),
            store_selected: None,
        }),
    });
    config
}

/// The config as pretty JSON with sorted keys, like the connection config.
pub fn generate_singbox_selfhost_config_json(
    spec: &SelfHostSpec,
) -> Result<String, SingboxConfigError> {
    let value = serde_json::to_value(generate_singbox_selfhost_config(spec))
        .map_err(SingboxConfigError::Serialize)?;
    serde_json::to_string_pretty(&value).map_err(SingboxConfigError::Serialize)
}

fn vless_server_inbound(vless: &SelfHostVlessSpec) -> SingboxInbound {
    SingboxInbound {
        r#type: "vless".to_string(),
        tag: SELFHOST_VLESS_INBOUND_TAG.to_string(),
        listen: Some(SELFHOST_LISTEN.to_string()),
        listen_port: Some(vless.port),
        users: Some(vec![SingboxUser {
            name: Some(SELFHOST_VLESS_USER.to_string()),
            uuid: Some(vless.uuid.clone()),
            flow: Some(SELFHOST_VLESS_FLOW.to_string()),
            ..SingboxUser::default()
        }]),
        tls: Some(SingboxInboundTls {
            enabled: true,
            server_name: Some(vless.reality_server_name.clone()),
            reality: Some(SingboxRealityServer {
                enabled: true,
                handshake: SingboxRealityHandshake {
                    server: vless.reality_server_name.clone(),
                    server_port: vless.reality_server_port,
                },
                private_key: vless.reality_private_key.clone(),
                short_id: vec![vless.reality_short_id.clone()],
            }),
        }),
        ..SingboxInbound::default()
    }
}

fn shadowsocks_server_inbound(shadowsocks: &SelfHostShadowsocksSpec) -> SingboxInbound {
    SingboxInbound {
        r#type: "shadowsocks".to_string(),
        tag: SELFHOST_SHADOWSOCKS_INBOUND_TAG.to_string(),
        listen: Some(SELFHOST_LISTEN.to_string()),
        listen_port: Some(shadowsocks.port),
        method: Some(SELFHOST_SHADOWSOCKS_METHOD.to_string()),
        password: Some(shadowsocks.password.clone()),
        ..SingboxInbound::default()
    }
}

/// Sniff, resolve domains to addresses, then reject what peers must not reach
/// before everything else leaves through `direct`. Resolving first is what
/// stops a domain that points at a private or loopback address from slipping
/// past the address rules.
fn selfhost_route_rules(spec: &SelfHostSpec) -> Vec<SingboxRule> {
    let mut rules = vec![
        SingboxRule {
            action: Some("sniff".to_string()),
            ..SingboxRule::default()
        },
        SingboxRule {
            action: Some("resolve".to_string()),
            ..SingboxRule::default()
        },
        SingboxRule {
            ip_cidr: Some(
                SELFHOST_ALWAYS_DENIED_CIDRS
                    .iter()
                    .map(|cidr| (*cidr).to_string())
                    .collect(),
            ),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        },
    ];
    if !spec.allow_lan_access {
        rules.push(SingboxRule {
            ip_cidr: Some(vec![SELFHOST_SHARED_ADDRESS_CIDR.to_string()]),
            ip_is_private: Some(true),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        });
    }
    if spec.block_bittorrent {
        rules.push(SingboxRule {
            protocol: Some(vec!["bittorrent".to_string()]),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        });
    }
    rules
}

/// The client side of every enabled inbound at every endpoint. The share links
/// and the server config come from the same spec, so they cannot disagree.
///
/// `reality_public_key` is derived from the spec's private key by the caller:
/// this crate carries no key agreement code.
#[must_use]
pub fn selfhost_share_profiles(
    spec: &SelfHostSpec,
    reality_public_key: &str,
    endpoints: &[SelfHostEndpoint],
    label: &str,
) -> Vec<ProfileItem> {
    let mut profiles = Vec::new();
    for endpoint in endpoints {
        if let Some(vless) = &spec.vless {
            profiles.push(ProfileItem {
                remarks: selfhost_remarks(label, "VLESS", &endpoint.label),
                protocol: ProfileProtocol::Vless {
                    server: crate::ServerEndpoint {
                        address: endpoint.address.clone(),
                        port: vless.port,
                    },
                    uuid: vless.uuid.clone(),
                    flow: Some(SELFHOST_VLESS_FLOW.to_string()),
                    encryption: Some(VLESS_ENCRYPTION_NONE.to_string()),
                },
                transport: Some(ProfileTransport::Tcp {
                    header: None,
                    host: None,
                    path: None,
                }),
                tls: Some(TlsSettings {
                    mode: TlsMode::Reality,
                    server_name: Some(vless.reality_server_name.clone()),
                    reality_public_key: Some(reality_public_key.to_string()),
                    reality_short_id: Some(vless.reality_short_id.clone()),
                    ..TlsSettings::default()
                }),
                ..ProfileItem::default()
            });
        }
        if let Some(shadowsocks) = &spec.shadowsocks {
            profiles.push(ProfileItem {
                remarks: selfhost_remarks(label, "SS", &endpoint.label),
                protocol: ProfileProtocol::Shadowsocks {
                    server: crate::ServerEndpoint {
                        address: endpoint.address.clone(),
                        port: shadowsocks.port,
                    },
                    password: shadowsocks.password.clone(),
                    method: SELFHOST_SHADOWSOCKS_METHOD.to_string(),
                    udp_over_tcp: false,
                },
                ..ProfileItem::default()
            });
        }
    }
    profiles
}

fn selfhost_remarks(label: &str, protocol: &str, endpoint_label: &str) -> String {
    [label.trim(), protocol, endpoint_label.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests;
