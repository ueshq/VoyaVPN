//! One expansion of a group's children, shared by group validation
//! ([`crate::groups`]) and runtime context building ([`crate::context`]).
//!
//! Both surfaces feed the same group preview: validation supplies
//! `childIndexIds` and generation supplies the `proxy-N-…` selector members. A
//! second implementation of the ordering, filtering and dedupe rules is how the
//! two drift apart, so the rules live here and each caller only supplies the
//! lookups and decides how loudly to report each issue.

use regex::Regex;

use crate::{validate_node, CoreType, ProfileItem, ProfileProtocol};

/// Lookups the shared resolver needs. Implementors differ only in where the
/// profiles come from (an in-memory list for validation, `CoreGenEnv` for
/// generation).
pub(crate) trait GroupChildSource {
    /// Explicit children, in `child_profile_ids` order. Ids with no profile are
    /// dropped; the resolver reports them separately.
    fn children_by_index_ids(&self, index_ids: &[String]) -> Vec<ProfileItem>;

    /// Members of a subscription, in the source's own order.
    fn children_by_subscription_id(&self, subscription_id: &str) -> Vec<ProfileItem>;
}

/// Outcome of one expansion. Issues are returned as data so each caller keeps
/// its own severity: a missing explicit child is an error in the group editor
/// but only a warning while building a runtime config.
#[derive(Debug, Default)]
pub(crate) struct GroupChildResolution {
    /// Subscription-sourced children first, then explicit ones, deduped by
    /// index id. This is the order the sing-box selector is generated in, so
    /// validation follows generation rather than the other way round.
    pub children: Vec<ProfileItem>,
    pub missing_child_ids: Vec<String>,
    pub duplicate_child_ids: Vec<String>,
    /// The `filter` pattern that failed to compile, if any.
    pub invalid_filter: Option<String>,
}

pub(crate) fn resolve_group_children(
    protocol: &ProfileProtocol,
    source: &impl GroupChildSource,
) -> GroupChildResolution {
    let mut resolution = GroupChildResolution::default();
    let explicit_ids = protocol.child_profile_ids();
    let explicit = source.children_by_index_ids(explicit_ids);
    for index_id in explicit_ids {
        if !explicit.iter().any(|profile| &profile.index_id == index_id) {
            resolution.missing_child_ids.push(index_id.clone());
        }
    }

    let mut children = subscription_children(protocol, source, &mut resolution);
    children.extend(explicit);

    let mut seen = Vec::new();
    for child in children {
        if seen.iter().any(|index_id| index_id == &child.index_id) {
            if !resolution.duplicate_child_ids.contains(&child.index_id) {
                resolution.duplicate_child_ids.push(child.index_id.clone());
            }
            continue;
        }
        seen.push(child.index_id.clone());
        resolution.children.push(child);
    }

    resolution
}

fn subscription_children(
    protocol: &ProfileProtocol,
    source: &impl GroupChildSource,
    resolution: &mut GroupChildResolution,
) -> Vec<ProfileItem> {
    let ProfileProtocol::PolicyGroup {
        source_subscription_id: Some(subscription_id),
        filter,
        ..
    } = protocol
    else {
        return Vec::new();
    };

    // An unparsable filter must select nothing: falling back to "no filter"
    // would silently turn a filtered group into an all-nodes group.
    let filter = match filter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(pattern) => match Regex::new(pattern) {
            Ok(filter) => Some(filter),
            Err(_) => {
                resolution.invalid_filter = Some(pattern.to_string());
                return Vec::new();
            }
        },
        None => None,
    };

    source
        .children_by_subscription_id(subscription_id)
        .into_iter()
        .filter(|profile| {
            !profile.config_type().is_complex_type()
                && validate_node(profile, CoreType::sing_box).success()
                && filter
                    .as_ref()
                    .is_none_or(|filter| filter.is_match(&profile.remarks))
        })
        .collect()
}
