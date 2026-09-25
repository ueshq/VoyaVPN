use std::collections::{BTreeMap, BTreeSet};

use voya_core::{
    AppConfig, ContextPolicyGroup, CoreGenEnv, CoreGenPlatform, InboundProtocol, PolicyGroupItem,
    ProfileItem, RoutingItem,
};

use crate::supervisor::ClashApiSecret;

#[derive(Debug, Clone)]
pub(crate) struct SnapshotCoreGenEnv {
    local_socks_port: i32,
    platform: CoreGenPlatform,
    profiles: Vec<ProfileItem>,
    routings: Vec<RoutingItem>,
    policy_groups: Vec<PolicyGroupItem>,
    singbox_ruleset_paths: BTreeMap<String, String>,
    clash_api_secret: Option<String>,
    ipv6_unsupported_nodes: BTreeSet<String>,
}

impl SnapshotCoreGenEnv {
    /// Every node this snapshot was taken with.
    pub(crate) fn profiles(&self) -> &[ProfileItem] {
        &self.profiles
    }

    pub(crate) fn new(
        config: &AppConfig,
        platform: CoreGenPlatform,
        profiles: Vec<ProfileItem>,
        routings: Vec<RoutingItem>,
    ) -> Self {
        Self {
            local_socks_port: config
                .inbounds
                .first()
                .map_or(voya_core::DEFAULT_LOCAL_PORT, |inbound| inbound.local_port),
            platform,
            profiles,
            routings,
            policy_groups: Vec::new(),
            singbox_ruleset_paths: BTreeMap::new(),
            clash_api_secret: None,
            ipv6_unsupported_nodes: BTreeSet::new(),
        }
    }

    /// The policy groups routing rules can name.
    pub(crate) fn with_policy_groups(mut self, policy_groups: Vec<PolicyGroupItem>) -> Self {
        self.policy_groups = policy_groups;
        self
    }

    pub(crate) fn with_singbox_ruleset_paths(
        mut self,
        singbox_ruleset_paths: BTreeMap<String, String>,
    ) -> Self {
        self.singbox_ruleset_paths = singbox_ruleset_paths;
        self
    }

    /// Nodes recorded as having no IPv6 egress (see [`crate::ipv6_egress`]).
    pub(crate) fn with_ipv6_unsupported_nodes(mut self, nodes: BTreeSet<String>) -> Self {
        self.ipv6_unsupported_nodes = nodes;
        self
    }

    /// Requires the generated `experimental.clash_api` to present this bearer
    /// token.
    ///
    /// Only the runtime launch path sets it: an exported client config or a
    /// preview must stay reproducible, and a golden fixture must keep
    /// generating byte-identical JSON, so the default is no secret at all.
    pub(crate) fn with_clash_api_secret(mut self, clash_api_secret: ClashApiSecret) -> Self {
        self.clash_api_secret = Some(clash_api_secret.into_token());
        self
    }
}

impl CoreGenEnv for SnapshotCoreGenEnv {
    fn platform(&self) -> CoreGenPlatform {
        self.platform
    }

    fn get_profile_by_remarks(&self, remarks: &str) -> Option<ProfileItem> {
        self.profiles
            .iter()
            .find(|profile| profile.remarks == remarks)
            .cloned()
    }
    fn get_policy_group(&self, id: &str) -> Option<ContextPolicyGroup> {
        let group = self.policy_groups.iter().find(|group| group.id == id)?;
        Some(ContextPolicyGroup {
            members: voya_core::resolve_group_members(group, &self.profiles)
                .into_iter()
                .cloned()
                .collect(),
            group: group.clone(),
        })
    }

    fn get_default_routing(&self, config: &AppConfig) -> Option<RoutingItem> {
        self.routings
            .iter()
            .find(|routing| routing.id == config.active_routing_id)
            .or_else(|| self.routings.first())
            .cloned()
    }

    fn get_local_port(&self, protocol: InboundProtocol) -> i32 {
        match protocol {
            InboundProtocol::socks => self.local_socks_port,
            _ => self.local_socks_port + protocol.port_offset(),
        }
    }

    fn get_singbox_ruleset_paths(&self) -> BTreeMap<String, String> {
        self.singbox_ruleset_paths.clone()
    }

    fn get_clash_api_secret(&self) -> Option<String> {
        self.clash_api_secret.clone()
    }

    fn ipv6_unsupported_nodes(&self) -> BTreeSet<String> {
        self.ipv6_unsupported_nodes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coregen_env_emits_no_clash_api_secret_by_default() {
        // Golden fixtures and exported client configs go through the same env,
        // so an unrequested secret would change generated JSON everywhere.
        let env = SnapshotCoreGenEnv::new(
            &AppConfig::default(),
            CoreGenPlatform::Linux,
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(env.get_clash_api_secret(), None);
    }

    #[test]
    fn coregen_env_carries_an_injected_clash_api_secret() {
        let secret = ClashApiSecret::generate();
        let env = SnapshotCoreGenEnv::new(
            &AppConfig::default(),
            CoreGenPlatform::Linux,
            Vec::new(),
            Vec::new(),
        )
        .with_clash_api_secret(secret.clone());

        assert_eq!(env.get_clash_api_secret().as_deref(), Some(secret.as_str()));
    }
}
