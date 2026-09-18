//! Golden cases for the self-hosted node: its standalone server config, and
//! the client outbounds its share links produce under default settings.

use serde_json::Value;

use crate::testutil::linux_context;
use crate::{
    generate_singbox_config_value, generate_singbox_selfhost_config, selfhost_share_profiles,
    AppConfig, ConfigType, CoreConfigContext, SelfHostClashApi, SelfHostEndpoint,
    SelfHostShadowsocksSpec, SelfHostSpec, SelfHostVlessSpec, PROXY_TAG,
};

/// A real keypair so `sing-box check` accepts the server config.
const REALITY_PRIVATE_KEY: &str = "sJ2_PK3Bd1use05cc9jK6gcEarznMKgXeVNz9Dt4VF0";
const REALITY_PUBLIC_KEY: &str = "Q5mEoK_fSpzT4d13YC4_HI_2Crte_pRkSElgYb5wCD8";

fn vless_spec() -> SelfHostVlessSpec {
    SelfHostVlessSpec {
        port: 42_443,
        uuid: "bd3a7c33-98cb-4faf-b0b5-853e2707be3f".to_string(),
        reality_private_key: REALITY_PRIVATE_KEY.to_string(),
        reality_short_id: "751998bfb8ed69a6".to_string(),
        reality_server_name: "www.apple.com".to_string(),
        reality_server_port: 443,
    }
}

fn shadowsocks_spec() -> SelfHostShadowsocksSpec {
    SelfHostShadowsocksSpec {
        port: 42_444,
        password: "2oYz+Tnxj/q1Y/fi4+DkkQ==".to_string(),
    }
}

fn vless_only_spec() -> SelfHostSpec {
    SelfHostSpec {
        vless: Some(vless_spec()),
        shadowsocks: None,
        allow_lan_access: false,
        block_bittorrent: false,
        clash_api: None,
        log_level: "warn".to_string(),
    }
}

fn shadowsocks_only_spec() -> SelfHostSpec {
    SelfHostSpec {
        vless: None,
        shadowsocks: Some(shadowsocks_spec()),
        ..vless_only_spec()
    }
}

fn dual_allow_lan_spec() -> SelfHostSpec {
    SelfHostSpec {
        vless: Some(vless_spec()),
        shadowsocks: Some(shadowsocks_spec()),
        allow_lan_access: true,
        block_bittorrent: true,
        clash_api: Some(SelfHostClashApi {
            port: 42_445,
            secret: "selfhost-golden-secret".to_string(),
        }),
        log_level: "info".to_string(),
    }
}

fn selfhost_config_value(spec: &SelfHostSpec) -> Value {
    serde_json::to_value(generate_singbox_selfhost_config(spec))
        .expect("self-hosted config serializes")
}

pub(super) fn selfhost_vless_reality() -> Value {
    selfhost_config_value(&vless_only_spec())
}

pub(super) fn selfhost_shadowsocks_2022() -> Value {
    selfhost_config_value(&shadowsocks_only_spec())
}

pub(super) fn selfhost_dual_allow_lan() -> Value {
    selfhost_config_value(&dual_allow_lan_spec())
}

/// The client context a peer ends up with after importing the node's link,
/// with every global setting at its default: no uTLS fingerprint configured.
fn client_context(config_type: ConfigType) -> CoreConfigContext {
    let endpoints = [SelfHostEndpoint {
        address: "203.0.113.7".to_string(),
        label: "IPv4".to_string(),
    }];
    let profile = selfhost_share_profiles(
        &dual_allow_lan_spec(),
        REALITY_PUBLIC_KEY,
        &endpoints,
        "Home",
    )
    .into_iter()
    .find(|profile| profile.config_type() == config_type)
    .expect("share profile for protocol");
    linux_context(AppConfig::default(), profile)
}

pub(super) fn vless_reality_vision_context() -> CoreConfigContext {
    client_context(ConfigType::VLESS)
}

pub(super) fn shadowsocks_2022_context() -> CoreConfigContext {
    client_context(ConfigType::Shadowsocks)
}

pub(super) fn proxy_outbound(context: &CoreConfigContext) -> Value {
    let config = generate_singbox_config_value(context).expect("client config generates");
    config["outbounds"]
        .as_array()
        .expect("outbounds")
        .iter()
        .find(|outbound| outbound["tag"] == PROXY_TAG)
        .cloned()
        .expect("proxy outbound")
}

pub(super) fn acceptance_configs(generated: &str) -> Option<Vec<Value>> {
    Some(match generated {
        "singbox.selfhost.vless_reality" => vec![selfhost_vless_reality()],
        "singbox.selfhost.shadowsocks_2022" => vec![selfhost_shadowsocks_2022()],
        "singbox.selfhost.dual_allow_lan" => vec![selfhost_dual_allow_lan()],
        "singbox.outbound.vless_reality_vision" => {
            vec![
                generate_singbox_config_value(&vless_reality_vision_context())
                    .expect("REALITY client config generates"),
            ]
        }
        "singbox.outbound.shadowsocks_2022" => {
            vec![generate_singbox_config_value(&shadowsocks_2022_context())
                .expect("SS-2022 client config generates")]
        }
        _ => return None,
    })
}
