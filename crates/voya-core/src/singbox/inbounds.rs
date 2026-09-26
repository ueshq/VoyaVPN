use super::*;

pub(super) fn gen_inbounds(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let inbound = context
        .app_config
        .inbounds
        .first()
        .cloned()
        .unwrap_or_default();
    let listen_port = inbound_port(&context.app_config, LocalPort::Primary);
    let is_using_local_mixed_port = context
        .active_outbound_nodes()
        .iter()
        .any(|node| node.address() == LOOPBACK && node.port() == listen_port);
    let mixed_inbound_available = !context.is_tun_enabled || !is_using_local_mixed_port;

    config.inbounds.clear();
    if mixed_inbound_available {
        let mut primary = build_mixed_inbound(&inbound, LocalPort::Primary);
        if inbound.lan_connections_allowed && !inbound.separate_lan_port {
            primary.listen = Some("0.0.0.0".to_string());
        }
        config.inbounds.push(primary.clone());

        if inbound.secondary_port_enabled {
            config
                .inbounds
                .push(build_mixed_inbound(&inbound, LocalPort::Secondary));
        }

        if inbound.lan_connections_allowed && inbound.separate_lan_port {
            let mut lan = build_mixed_inbound(&inbound, LocalPort::Lan);
            lan.listen = Some("0.0.0.0".to_string());
            if !inbound.username.trim().is_empty() && !inbound.password.trim().is_empty() {
                lan.users = Some(vec![SingboxUser {
                    username: Some(inbound.username.clone()),
                    password: Some(inbound.password.clone()),
                    ..SingboxUser::default()
                }]);
            }
            config.inbounds.push(lan);
        }
    }

    if context.is_tun_enabled {
        config.inbounds.push(build_tun_inbound(
            context,
            mixed_inbound_available.then_some(listen_port),
        ));
    }
}

fn build_mixed_inbound(inbound: &InboundConfig, port: LocalPort) -> SingboxInbound {
    build_mixed_inbound_with(
        local_port_tag(port),
        inbound.local_port + port.port_offset(),
    )
}

pub(super) fn build_mixed_inbound_with(tag: impl Into<String>, port: i32) -> SingboxInbound {
    SingboxInbound {
        r#type: "mixed".to_string(),
        tag: tag.into(),
        listen: Some(LOOPBACK.to_string()),
        listen_port: Some(port),
        ..SingboxInbound::default()
    }
}

fn build_tun_inbound(context: &CoreConfigContext, http_proxy_port: Option<i32>) -> SingboxInbound {
    let mtu = tun_mtu(context);
    let address = tun_addresses(context);
    let stack = nonempty_str(Some(&context.app_config.tun.stack))
        .unwrap_or(DEFAULT_TUN_STACK)
        .to_string();

    SingboxInbound {
        r#type: "tun".to_string(),
        tag: SINGBOX_TUN_INBOUND_TAG.to_string(),
        listen: None,
        listen_port: None,
        interface_name: tun_interface_name(context),
        address: Some(address),
        mtu: Some(mtu),
        auto_route: Some(context.app_config.tun.auto_route),
        strict_route: Some(tun_strict_route(context)),
        stack: Some(stack),
        platform: tun_platform(context, http_proxy_port),
        ..SingboxInbound::default()
    }
}

fn tun_addresses(_context: &CoreConfigContext) -> Vec<String> {
    // Always dual-stack: without a local IPv6 address the TUN cannot capture
    // literal IPv6 destinations, so the reject rule would never see them and
    // they would leak out of the physical NIC.
    vec![
        "172.18.0.1/30".to_string(),
        "fdfe:dcba:9876::1/126".to_string(),
    ]
}

fn tun_platform(
    context: &CoreConfigContext,
    http_proxy_port: Option<i32>,
) -> Option<SingboxTunPlatform> {
    if !context.platform.is_macos() {
        return None;
    }
    let port = http_proxy_port?;
    Some(SingboxTunPlatform {
        http_proxy: Some(SingboxTunHttpProxy {
            enabled: true,
            server: Some(LOOPBACK.to_string()),
            server_port: Some(port),
        }),
    })
}

fn tun_interface_name(context: &CoreConfigContext) -> Option<String> {
    (!context.platform.is_macos()).then(|| "singbox_tun".to_string())
}

fn tun_mtu(context: &CoreConfigContext) -> i32 {
    let configured = if context.app_config.tun.mtu > 0 {
        context.app_config.tun.mtu
    } else {
        WIREGUARD_DEFAULT_MTU
    };
    if context.platform.is_macos() && configured > MACOS_TUN_SAFE_MTU {
        MACOS_TUN_SAFE_MTU
    } else {
        configured
    }
}

fn tun_strict_route(context: &CoreConfigContext) -> bool {
    !context.platform.is_macos() && context.app_config.tun.strict_route
}
