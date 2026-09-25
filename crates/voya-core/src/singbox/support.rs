use super::*;

pub(super) fn apply_outbound_bind_interface(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
) {
    let Some(bind_interface) = nonempty_string(context.app_config.core.bind_interface.as_deref())
    else {
        return;
    };
    if !(context.is_tun_enabled || context.platform.is_windows()) {
        return;
    }
    for outbound in &mut config.outbounds {
        if should_bind_outbound(outbound) {
            outbound.bind_interface = Some(bind_interface.clone());
        }
    }
}

pub(super) fn apply_outbound_send_through(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let Some(send_through) = nonempty_string(context.app_config.core.send_through.as_deref())
    else {
        return;
    };
    for outbound in &mut config.outbounds {
        if should_bind_outbound(outbound) {
            outbound.inet4_bind_address = Some(send_through.clone());
        }
    }
}

fn should_bind_outbound(outbound: &SingboxOutbound) -> bool {
    if matches!(outbound.r#type.as_str(), "direct" | "block" | "dns") {
        return false;
    }
    outbound
        .server
        .as_deref()
        .is_none_or(|server| !is_loopback_address(server))
}
/// The sing-box outbound `type` spelling for a protocol: the stored spelling
/// lowercased (`wireGuard` → `wireguard`).
pub(super) fn singbox_protocol_type(config_type: ConfigType) -> String {
    config_type.as_str().to_ascii_lowercase()
}

pub(super) fn vmess_security(protocol: &ProfileProtocol) -> String {
    let security = match protocol {
        ProfileProtocol::Vmess { cipher, .. } => cipher.as_deref().unwrap_or_default(),
        _ => "",
    };
    if VMESS_SECURITIES.contains(&security) {
        security.to_string()
    } else {
        DEFAULT_SECURITY.to_string()
    }
}

pub(super) fn shadowsocks_method(protocol: &ProfileProtocol) -> String {
    let method = match protocol {
        ProfileProtocol::Shadowsocks { method, .. } => method.as_str(),
        _ => "",
    };
    if SS_SECURITIES_IN_SINGBOX.contains(&method) {
        method.to_string()
    } else {
        "none".to_string()
    }
}

pub(super) fn allow_insecure(context: &CoreConfigContext) -> bool {
    context.app_config.core.default_allow_insecure
}

pub(super) fn effective_fingerprint(context: &CoreConfigContext) -> Option<String> {
    singbox_utls_fingerprint(&context.app_config.core.default_fingerprint)
}

fn singbox_utls_fingerprint(value: &str) -> Option<String> {
    let fingerprint = value.trim().to_ascii_lowercase();
    if SINGBOX_UTLS_FINGERPRINTS.contains(&fingerprint.as_str()) {
        Some(fingerprint)
    } else {
        None
    }
}

pub(super) fn transport_host_for_tls(node: &ProfileItem) -> Option<String> {
    let first_host = first_list_value(node.transport.as_ref().and_then(ProfileTransport::host));
    nonempty_string(Some(&first_host))
}

pub(crate) fn state_port2(app_config: &AppConfig, is_tun_enabled: bool) -> i32 {
    inbound_port(app_config, InboundProtocol::api2) + i32::from(is_tun_enabled)
}

pub(super) fn exe_name(process: &str) -> String {
    process
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(process)
        .trim_end_matches(".exe")
        .to_string()
}

fn is_loopback_address(address: &str) -> bool {
    let address = address.trim_matches(['[', ']']);
    address.eq_ignore_ascii_case("localhost")
        || address
            .parse::<IpAddr>()
            .is_ok_and(|ip_address| ip_address.is_loopback())
}
