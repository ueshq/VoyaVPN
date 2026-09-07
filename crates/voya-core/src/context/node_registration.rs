use super::*;

pub(super) fn pre_socks_item<E: CoreGenEnv>(
    config: &AppConfig,
    node: &ProfileItem,
    env: &E,
) -> Option<ProfileItem> {
    // The topology is an injected platform fact, not something derived from
    // `CoreGenPlatform`: macOS runs TUN inside the single NetworkExtension
    // config (ADR 0005), so `build_all` must not synthesize a pre-socks context
    // there even though macOS is "non-Windows".
    if node.config_type() != ConfigType::Custom
        && config.tun_mode_item.enable_tun
        && env.tun_topology().is_pre_socks()
    {
        return Some(socks_profile(env.get_local_port(InboundProtocol::socks)));
    }

    let subscription_pre_socks_port = node
        .subscription_id
        .as_deref()
        .and_then(|id| env.get_subscription(id))
        .and_then(|subscription| subscription.pre_socks_port);
    if node.config_type() == ConfigType::Custom
        && matches!(subscription_pre_socks_port, Some(1..=65535))
    {
        return Some(socks_profile(
            subscription_pre_socks_port.unwrap_or_default(),
        ));
    }

    None
}

pub(super) fn register_single_node(
    context: &mut CoreConfigContext,
    node: &ProfileItem,
) -> NodeValidatorResult {
    if node.config_type().is_group_type() {
        return NodeValidatorResult::empty();
    }

    let result = validate_node(node, context.run_core_type);
    if !result.success() {
        return result;
    }

    context
        .all_proxies_map
        .insert(node.index_id.clone(), node.clone());

    push_domain_if_needed(&mut context.protect_domain_list, node.address());

    if let Some(tls) = &node.tls {
        if !tls.ech_config.is_empty() {
            let server_name = tls.server_name.as_deref().unwrap_or_default();
            let ech_query_sni = if tls.mode == TlsMode::Tls
                && tls.ech_config.iter().any(|value| value.contains("://"))
            {
                tls.ech_config
                    .iter()
                    .find(|value| !value.contains("://"))
                    .map(|value| value.split_once('+').map_or(value.as_str(), |(sni, _)| sni))
                    .unwrap_or(server_name)
            } else {
                server_name
            };
            push_domain_if_needed(&mut context.protect_domain_list, ech_query_sni);
        }
    }

    if let Some(download_address) = xhttp_download_settings_address(node) {
        push_domain_if_needed(&mut context.protect_domain_list, &download_address);
    }

    result
}

fn socks_profile(port: i32) -> ProfileItem {
    ProfileItem {
        protocol: ProfileProtocol::Socks {
            server: ServerEndpoint {
                address: LOOPBACK.to_string(),
                port,
            },
            username: String::new(),
            password: String::new(),
        },
        ..ProfileItem::default()
    }
}
