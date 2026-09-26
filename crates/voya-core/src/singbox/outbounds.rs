use crate::{ConfigType, CoreConfigContext, ProfileItem, PROXY_TAG};

use super::{SingboxConfig, SingboxEndpoint, SingboxOutbound};

mod groups;
mod latency_probes;
mod protocols;
mod stream;

pub(super) use groups::{build_policy_group_servers, rule_group_tag};
pub(super) use latency_probes::gen_latency_probes;
pub use latency_probes::{is_latency_probe_candidate, latency_probe_tag};

pub(super) use protocols::*;
pub(super) use stream::*;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SingboxServer {
    Outbound(Box<SingboxOutbound>),
    Endpoint(Box<SingboxEndpoint>),
}

pub(super) fn gen_outbounds(config: &mut SingboxConfig, context: &CoreConfigContext) {
    match &context.policy_group {
        Some(active) => prepend_servers(
            config,
            build_policy_group_servers(context, active, PROXY_TAG, None),
        ),
        None => prepend_servers(
            config,
            build_proxy_server(context, &context.node, PROXY_TAG),
        ),
    }
}

fn prepend_servers(config: &mut SingboxConfig, servers: impl IntoIterator<Item = SingboxServer>) {
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

pub(super) fn append_servers(
    config: &mut SingboxConfig,
    servers: impl IntoIterator<Item = SingboxServer>,
) {
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
