//! [`CoreConfigContextBuilder::build`] split in two, for runs that build one
//! context per node.

use std::collections::BTreeMap;

use super::{
    resolve_node, validation::merge_protect_domains, ContextPolicyGroup, CoreConfigContext,
    CoreConfigContextBuilderResult, NodeValidatorResult,
};
use crate::{ProfileItem, RoutingItem};

/// Everything [`super::CoreConfigContextBuilder::build`] resolves that does
/// not depend on the node: the default routing and the outbounds its rules
/// name. Made by [`super::CoreConfigContextBuilder::prepare`].
///
/// A speedtest builds one context per node. Resolving the rules for each of
/// them searched the whole node list for every rule naming a node and copied
/// every member of every group a rule names, so a single rule pointing at a
/// large subscription's group made a large run quadratic in time and memory.
#[derive(Debug, Clone)]
pub struct PreparedContextBuilder {
    /// The context before any node is registered, without its routing.
    pub(super) base: CoreConfigContext,
    pub(super) routing_item: Option<RoutingItem>,
    pub(super) rules: RuleOutbounds,
}

/// What resolving the routing rules adds to a context.
#[derive(Debug, Clone, Default)]
pub(super) struct RuleOutbounds {
    pub(super) all_proxies_map: BTreeMap<String, ProfileItem>,
    pub(super) protect_domain_list: Vec<String>,
    pub(super) policy_groups: Vec<ContextPolicyGroup>,
    pub(super) result: NodeValidatorResult,
}

impl PreparedContextBuilder {
    /// Exactly what [`super::CoreConfigContextBuilder::build`] returns for
    /// `node`.
    #[must_use]
    pub fn build(&self, node: &ProfileItem) -> CoreConfigContextBuilderResult {
        let mut context = self.base.clone();
        context.routing_item.clone_from(&self.routing_item);
        self.finish(context, node, true)
    }

    /// [`Self::build`] for a config that holds only `node`'s own outbound, a
    /// speedtest page or a latency probe tag: the same validation outcome,
    /// but the context carries neither the routing nor the outbounds its rules
    /// resolved, which such a config never reads.
    #[must_use]
    pub fn build_node_outbound(&self, node: &ProfileItem) -> CoreConfigContextBuilderResult {
        self.finish(self.base.clone(), node, false)
    }

    fn finish(
        &self,
        mut context: CoreConfigContext,
        node: &ProfileItem,
        with_rules: bool,
    ) -> CoreConfigContextBuilderResult {
        context.node = node.clone();
        let node_result = resolve_node(&mut context, node);
        if !node_result.success() {
            return CoreConfigContextBuilderResult {
                context,
                validator_result: node_result,
            };
        }

        // The node's warnings come first, then the rules' findings: the order
        // a single pass over node and rules reported them in.
        let mut validator_result = NodeValidatorResult {
            errors: self.rules.result.errors.clone(),
            warnings: node_result.warnings,
        };
        validator_result
            .warnings
            .extend_from_slice(&self.rules.result.warnings);
        if with_rules {
            // A rule outbound replaces an entry the node registered under the
            // same key, as registering it after the node did.
            context.all_proxies_map.extend(
                self.rules
                    .all_proxies_map
                    .iter()
                    .map(|(tag, outbound)| (tag.clone(), outbound.clone())),
            );
            merge_protect_domains(
                &mut context.protect_domain_list,
                &self.rules.protect_domain_list,
            );
            context
                .rule_policy_groups
                .clone_from(&self.rules.policy_groups);
        }

        CoreConfigContextBuilderResult {
            context,
            validator_result,
        }
    }
}
