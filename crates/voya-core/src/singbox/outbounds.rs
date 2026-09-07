use super::*;

mod groups;
mod protocols;
mod stream;

pub(super) use groups::*;
pub(super) use protocols::*;
pub(super) use stream::*;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SingboxServer {
    Outbound(Box<SingboxOutbound>),
    Endpoint(Box<SingboxEndpoint>),
}

impl SingboxServer {
    fn tag(&self) -> &str {
        match self {
            Self::Outbound(outbound) => &outbound.tag,
            Self::Endpoint(endpoint) => &endpoint.tag,
        }
    }

    fn set_tag(&mut self, tag: String) {
        match self {
            Self::Outbound(outbound) => outbound.tag = tag,
            Self::Endpoint(endpoint) => endpoint.tag = tag,
        }
    }

    fn detour(&self) -> Option<&str> {
        match self {
            Self::Outbound(outbound) => outbound.detour.as_deref(),
            Self::Endpoint(endpoint) => endpoint.detour.as_deref(),
        }
    }

    fn set_detour(&mut self, detour: &str) {
        match self {
            Self::Outbound(outbound) => outbound.detour = Some(detour.to_string()),
            Self::Endpoint(endpoint) => endpoint.detour = Some(detour.to_string()),
        }
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.tag().starts_with(prefix)
    }
}

pub(super) fn gen_outbounds(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let servers = build_all_proxy_servers(context, &context.node, PROXY_TAG, true);
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
