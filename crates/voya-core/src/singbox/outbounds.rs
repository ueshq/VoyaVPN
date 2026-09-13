use super::*;

mod groups;
mod protocols;
mod stream;

pub(super) use protocols::*;
pub(super) use stream::*;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SingboxServer {
    Outbound(Box<SingboxOutbound>),
    Endpoint(Box<SingboxEndpoint>),
}

pub(super) fn gen_outbounds(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let servers = match &context.policy_group {
        Some(active) => groups::build_policy_group_servers(context, active),
        None => build_proxy_servers(context, &context.node, PROXY_TAG),
    };
    prepend_servers(config, servers);
}

fn prepend_servers(config: &mut SingboxConfig, servers: Vec<SingboxServer>) {
    let mut outbounds = Vec::new();
    let mut endpoints = Vec::new();
    for server in servers {
        match server {
            SingboxServer::Outbound(outbound) => outbounds.push(*outbound),
            SingboxServer::Endpoint(endpoint) => endpoints.push(*endpoint),
        }
    }
    config.outbounds.splice(0..0, outbounds);
    config.endpoints.splice(0..0, endpoints);
}

pub(super) fn append_servers(config: &mut SingboxConfig, servers: Vec<SingboxServer>) {
    for server in servers {
        match server {
            SingboxServer::Outbound(outbound) => config.outbounds.push(*outbound),
            SingboxServer::Endpoint(endpoint) => config.endpoints.push(*endpoint),
        }
    }
}

pub(super) fn build_proxy_server(
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

pub(super) fn build_proxy_servers(
    context: &CoreConfigContext,
    node: &ProfileItem,
    tag: &str,
) -> Vec<SingboxServer> {
    build_proxy_server(context, node, tag).into_iter().collect()
}
