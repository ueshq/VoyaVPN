use super::*;

pub(crate) fn build_outbound(context: &CoreConfigContext, node: &ProfileItem) -> SingboxOutbound {
    let mut outbound = SingboxOutbound {
        r#type: singbox_protocol_type(node.config_type()),
        tag: PROXY_TAG.to_string(),
        server: Some(node.address().to_string()),
        server_port: Some(node.port()),
        ..SingboxOutbound::default()
    };

    match &node.protocol {
        ProfileProtocol::Vmess { uuid, .. } => {
            outbound.uuid = Some(uuid.clone());
            outbound.alter_id = Some(0);
            outbound.security = Some(vmess_security(&node.protocol));
            fill_outbound_mux(&mut outbound, context);
            fill_outbound_transport(&mut outbound, context, node);
        }
        ProfileProtocol::Shadowsocks {
            password,
            udp_over_tcp,
            ..
        } => {
            outbound.method = Some(shadowsocks_method(&node.protocol));
            outbound.password = Some(password.clone());
            outbound.udp_over_tcp = (*udp_over_tcp).then_some(true);
            fill_shadowsocks_plugin(&mut outbound, node);
            fill_outbound_mux(&mut outbound, context);
        }
        ProfileProtocol::Socks {
            username, password, ..
        } => {
            outbound.version = Some("5".to_string());
            if !username.trim().is_empty() && !password.trim().is_empty() {
                outbound.username = Some(username.clone());
                outbound.password = Some(password.clone());
            }
        }
        ProfileProtocol::Http {
            username, password, ..
        } => {
            if !username.trim().is_empty() && !password.trim().is_empty() {
                outbound.username = Some(username.clone());
                outbound.password = Some(password.clone());
            }
        }
        ProfileProtocol::Vless { uuid, flow, .. } => {
            outbound.uuid = Some(uuid.clone());
            outbound.packet_encoding = Some("xudp".to_string());
            if let Some(flow) = nonempty_string(flow.as_deref()) {
                outbound.flow = Some(flow);
            } else {
                fill_outbound_mux(&mut outbound, context);
            }
            fill_outbound_transport(&mut outbound, context, node);
        }
        ProfileProtocol::Trojan { password, .. } => {
            outbound.password = Some(password.clone());
            fill_outbound_mux(&mut outbound, context);
            fill_outbound_transport(&mut outbound, context, node);
        }
        ProfileProtocol::Hysteria2 {
            password,
            port_hops,
            obfuscation_password,
            ..
        } => {
            outbound.password = Some(password.clone());
            fill_hysteria2_fields(
                &mut outbound,
                context,
                port_hops.as_deref(),
                obfuscation_password.as_deref(),
            );
        }
        ProfileProtocol::Tuic {
            uuid,
            password,
            congestion_control,
            ..
        } => {
            outbound.uuid = nonempty_string(Some(uuid));
            outbound.password = Some(password.clone());
            outbound.congestion_control = nonempty_string(congestion_control.as_deref());
        }
        ProfileProtocol::Anytls { password, .. } => {
            outbound.password = Some(password.clone());
        }
        ProfileProtocol::Naive {
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
            ..
        } => {
            outbound.username = nonempty_string(Some(username));
            outbound.password = Some(password.clone());
            if *quic {
                outbound.quic = Some(true);
                outbound.quic_congestion_control = nonempty_string(congestion_control.as_deref());
            }
            outbound.insecure_concurrency = insecure_concurrency.filter(|value| *value > 0);
            outbound.udp_over_tcp = (*udp_over_tcp).then_some(true);
        }
        ProfileProtocol::WireGuard { .. } => {}
    }

    fill_outbound_tls(&mut outbound, context, node);
    outbound
}

pub(crate) fn build_wireguard_endpoint(node: &ProfileItem) -> Option<SingboxEndpoint> {
    let ProfileProtocol::WireGuard {
        server,
        private_key,
        preshared_key,
        interface_address,
        reserved,
        mtu,
        ..
    } = &node.protocol
    else {
        return None;
    };
    let public_key = wireguard_public_key(&node.protocol)?;
    Some(SingboxEndpoint {
        r#type: singbox_protocol_type(node.config_type()),
        tag: PROXY_TAG.to_string(),
        address: {
            let items = split_csv(interface_address.as_deref().unwrap_or_default());
            if items.is_empty() {
                vec![WIREGUARD_DEFAULT_ADDRESS.to_string()]
            } else {
                items
            }
        },
        private_key: private_key.clone(),
        mtu: Some(mtu.filter(|mtu| *mtu > 0).unwrap_or(WIREGUARD_DEFAULT_MTU)),
        peers: vec![SingboxPeer {
            address: server.address.clone(),
            port: server.port,
            public_key,
            pre_shared_key: preshared_key.clone(),
            allowed_ips: wireguard_allowed_ips(&node.protocol),
            reserved: parse_wireguard_reserved(reserved.as_deref()),
            persistent_keepalive_interval: None,
        }],
        ..SingboxEndpoint::default()
    })
}

fn fill_shadowsocks_plugin(outbound: &mut SingboxOutbound, node: &ProfileItem) {
    // `shadowsocks_plugin_for` owns the option list; sing-box only differs from
    // the share-link form in splitting the plugin name out of the options.
    let Some(plugin) = shadowsocks_plugin_for(node) else {
        return;
    };
    outbound.plugin = Some(plugin.name.to_string());
    outbound.plugin_opts = Some(plugin.render_opts());
}

fn fill_hysteria2_fields(
    outbound: &mut SingboxOutbound,
    context: &CoreConfigContext,
    port_hops: Option<&str>,
    obfuscation_password: Option<&str>,
) {
    if let Some(salamander_pass) = nonempty_str(obfuscation_password) {
        outbound.obfs = Some(SingboxHyObfs {
            r#type: Some("salamander".to_string()),
            password: Some(salamander_pass.to_string()),
        });
    }

    let up_mbps = context.app_config.hysteria_item.up_mbps;
    let down_mbps = context.app_config.hysteria_item.down_mbps;
    outbound.up_mbps = (up_mbps > 0).then_some(up_mbps);
    outbound.down_mbps = (down_mbps > 0).then_some(down_mbps);

    let Some(ports) = nonempty_str(port_hops) else {
        return;
    };
    if !ports.contains([':', '-', ',']) {
        return;
    }

    let server_ports = ports
        .split(',')
        .map(str::trim)
        .filter(|port| !port.is_empty())
        .map(|port| {
            let port = port.replace('-', ":");
            if port.contains(':') {
                port
            } else {
                format!("{port}:{port}")
            }
        })
        .collect::<Vec<_>>();
    if !server_ports.is_empty() {
        outbound.server_port = None;
        outbound.server_ports = Some(server_ports);
    }

    let default_interval = if context.app_config.hysteria_item.hop_interval >= 5 {
        context.app_config.hysteria_item.hop_interval
    } else {
        DEFAULT_HYSTERIA2_HOP_INTERVAL
    };
    outbound.hop_interval = Some(format!("{default_interval}s"));
}

fn fill_outbound_mux(outbound: &mut SingboxOutbound, context: &CoreConfigContext) {
    if !context.app_config.core_basic_item.mux_enabled {
        return;
    }
    let protocol = context.app_config.mux4_sbox_item.protocol.trim();
    if protocol.is_empty() {
        return;
    }
    outbound.multiplex = Some(SingboxMultiplex {
        enabled: true,
        protocol: protocol.to_string(),
        max_connections: context.app_config.mux4_sbox_item.max_connections,
        padding: context.app_config.mux4_sbox_item.padding,
    });
}
