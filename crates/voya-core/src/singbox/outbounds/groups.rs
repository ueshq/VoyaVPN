use super::*;

pub(crate) fn build_all_proxy_servers(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
    with_selector: bool,
) -> Vec<SingboxServer> {
    let mut proxy_servers = if node.config_type().is_group_type() {
        build_group_proxy_servers(context, node, base_tag_name)
    } else {
        build_proxy_server(context, node, base_tag_name)
            .into_iter()
            .collect()
    };

    if with_selector {
        let proxy_tags = ordered_proxy_tags(&proxy_servers, base_tag_name);
        if proxy_tags.len() > 1 {
            let mut selectors = build_selector_servers(node, &proxy_tags, base_tag_name);
            selectors.extend(proxy_servers);
            proxy_servers = selectors;
        }
    }

    proxy_servers
}

fn build_proxy_server(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Option<SingboxServer> {
    if node.config_type() == ConfigType::WireGuard {
        let mut endpoint = build_wireguard_endpoint(node)?;
        endpoint.tag = base_tag_name.to_string();
        return Some(SingboxServer::Endpoint(Box::new(endpoint)));
    }

    let mut outbound = build_outbound(context, node);
    outbound.tag = base_tag_name.to_string();
    Some(SingboxServer::Outbound(Box::new(outbound)))
}

fn build_group_proxy_servers(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<SingboxServer> {
    match node.config_type() {
        ConfigType::PolicyGroup => build_outbounds_list(context, node, base_tag_name),
        ConfigType::ProxyChain => build_chain_outbounds_list(context, node, base_tag_name),
        _ => Vec::new(),
    }
}

fn build_outbounds_list(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<SingboxServer> {
    let nodes = buildable_child_nodes(context, node);
    let mut result: Vec<SingboxServer> = Vec::new();

    for (index, child_node) in nodes.iter().enumerate() {
        let current_tag = if nodes.len() == 1 {
            base_tag_name.to_string()
        } else {
            format!("{base_tag_name}-{}-{}", index + 1, child_node.remarks)
        };

        if child_node.config_type().is_group_type() {
            result.extend(build_group_proxy_servers(context, child_node, &current_tag));
            continue;
        }

        if let Some(server) = build_proxy_server(context, child_node, &current_tag) {
            result.push(server);
        }
    }

    result
}

fn build_chain_outbounds_list(
    context: &CoreConfigContext,
    node: &ProfileItem,
    base_tag_name: &str,
) -> Vec<SingboxServer> {
    let nodes = buildable_child_nodes(context, node);
    let nodes_reverse = nodes.into_iter().rev().collect::<Vec<_>>();
    let mut result: Vec<SingboxServer> = Vec::new();

    for (index, child_node) in nodes_reverse.iter().enumerate() {
        let current_tag = if index == 0 {
            base_tag_name.to_string()
        } else {
            format!("chain-{base_tag_name}-{index}-{}", child_node.remarks)
        };
        let detour_tag = (index != nodes_reverse.len().saturating_sub(1)).then(|| {
            format!(
                "chain-{base_tag_name}-{}-{}",
                index + 1,
                nodes_reverse[index + 1].remarks
            )
        });

        if child_node.config_type().is_group_type() {
            let mut child_profiles = build_group_proxy_servers(context, child_node, &current_tag);
            if let Some(detour_tag) = detour_tag.as_deref() {
                for server in child_profiles
                    .iter_mut()
                    .filter(|server| server.detour().is_none_or(str::is_empty))
                {
                    server.set_detour(detour_tag);
                }
            }

            if index != 0 {
                let chain_start_nodes = child_profiles
                    .iter()
                    .filter(|server| server.starts_with(&current_tag))
                    .cloned()
                    .collect::<Vec<_>>();
                if chain_start_nodes.len() == 1 {
                    let first_chain_tag = chain_start_nodes[0].tag().to_string();
                    for server in &mut result {
                        if server.detour() == Some(current_tag.as_str()) {
                            server.set_detour(&first_chain_tag);
                        }
                    }
                } else if chain_start_nodes.len() > 1 {
                    let existed_chain_nodes = result.clone();
                    result.clear();
                    for (branch_index, chain_start_node) in chain_start_nodes.iter().enumerate() {
                        let mut existed_chain_nodes_clone = existed_chain_nodes.clone();
                        for existed_chain_node in &mut existed_chain_nodes_clone {
                            existed_chain_node.set_tag(format!(
                                "{}-clone-{}",
                                existed_chain_node.tag(),
                                branch_index + 1
                            ));
                        }
                        for chain_index in 0..existed_chain_nodes_clone.len() {
                            let previous_detour = existed_chain_nodes_clone[chain_index]
                                .detour()
                                .map(str::to_string);
                            let next_tag = if chain_index + 1 < existed_chain_nodes_clone.len() {
                                existed_chain_nodes_clone[chain_index + 1].tag().to_string()
                            } else {
                                chain_start_node.tag().to_string()
                            };
                            let next_detour =
                                if previous_detour.as_deref() == Some(current_tag.as_str()) {
                                    chain_start_node.tag()
                                } else {
                                    &next_tag
                                };
                            existed_chain_nodes_clone[chain_index].set_detour(next_detour);
                            result.push(existed_chain_nodes_clone[chain_index].clone());
                        }
                    }
                }
            }

            result.extend(child_profiles);
            continue;
        }

        let Some(mut outbound) = build_proxy_server(context, child_node, &current_tag) else {
            continue;
        };
        if let Some(detour_tag) = detour_tag {
            outbound.set_detour(&detour_tag);
        }
        result.push(outbound);
    }

    result
}

fn build_selector_servers(
    node: &ProfileItem,
    proxy_tags: &[String],
    base_tag_name: &str,
) -> Vec<SingboxServer> {
    let multiple_load = match &node.protocol {
        ProfileProtocol::PolicyGroup { strategy, .. } => *strategy,
        _ => MultipleLoad::LeastPing,
    };
    let auto_tag = format!("{base_tag_name}-auto");
    let out_urltest = SingboxOutbound {
        r#type: "urltest".to_string(),
        tag: auto_tag.clone(),
        outbounds: Some(proxy_tags.to_vec()),
        interrupt_exist_connections: Some(false),
        tolerance: (multiple_load == MultipleLoad::Fallback).then_some(5000),
        ..SingboxOutbound::default()
    };
    let mut selector_outbounds = proxy_tags.to_vec();
    selector_outbounds.insert(0, auto_tag);
    let out_selector = SingboxOutbound {
        r#type: "selector".to_string(),
        tag: base_tag_name.to_string(),
        outbounds: Some(selector_outbounds),
        interrupt_exist_connections: Some(false),
        ..SingboxOutbound::default()
    };

    vec![
        SingboxServer::Outbound(Box::new(out_selector)),
        SingboxServer::Outbound(Box::new(out_urltest)),
    ]
}

fn ordered_proxy_tags(servers: &[SingboxServer], base_tag_name: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut tags = Vec::new();
    for server in servers {
        let tag = server.tag();
        if tag.starts_with(base_tag_name) && seen.insert(tag.to_string()) {
            tags.push(tag.to_string());
        }
    }
    tags
}
