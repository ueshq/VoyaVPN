//! Policy group domain types <-> contract DTOs.

use voya_contracts::{
    PolicyGroup, PolicyGroupEntry, PolicyGroupMember, PolicyGroupRuntime, PolicyGroupRuntimeMember,
    PolicyGroupStrategy,
};
use voya_core::{GroupStrategy, PolicyGroupItem};

use crate::{
    policy_groups::PolicyGroupEntry as AppPolicyGroupEntry, proxy_runtime::RuntimeGroupState,
};

const fn strategy_to_contract(value: GroupStrategy) -> PolicyGroupStrategy {
    match value {
        GroupStrategy::Selector => PolicyGroupStrategy::Selector,
        GroupStrategy::UrlTest => PolicyGroupStrategy::UrlTest,
        GroupStrategy::Fallback => PolicyGroupStrategy::Fallback,
    }
}

const fn strategy_from_contract(value: PolicyGroupStrategy) -> GroupStrategy {
    match value {
        PolicyGroupStrategy::Selector => GroupStrategy::Selector,
        PolicyGroupStrategy::UrlTest => GroupStrategy::UrlTest,
        PolicyGroupStrategy::Fallback => GroupStrategy::Fallback,
    }
}

#[must_use]
pub fn policy_group_to_contract(group: PolicyGroupItem) -> PolicyGroup {
    PolicyGroup {
        id: group.id,
        name: group.name,
        strategy: strategy_to_contract(group.strategy),
        source_subscription_id: group.source_subscription_id,
        auto_created: group.auto_created,
        selected_profile_id: group.selected_profile_id,
        test_url: group.test_url,
        interval_seconds: group
            .interval_seconds
            .and_then(|value| u32::try_from(value).ok()),
        tolerance_ms: group
            .tolerance_ms
            .and_then(|value| u32::try_from(value).ok()),
        member_ids: group.member_ids,
    }
}

/// The editor's group as a domain group. Values past `i32::MAX` saturate, and
/// the manager's range checks then reject them with the accepted range.
#[must_use]
pub fn policy_group_from_contract(group: PolicyGroup) -> PolicyGroupItem {
    PolicyGroupItem {
        id: group.id,
        name: group.name,
        strategy: strategy_from_contract(group.strategy),
        source_subscription_id: group.source_subscription_id,
        auto_created: group.auto_created,
        selected_profile_id: group.selected_profile_id,
        test_url: group.test_url,
        interval_seconds: group.interval_seconds.map(saturating_i32),
        tolerance_ms: group.tolerance_ms.map(saturating_i32),
        sort: 0,
        member_ids: group.member_ids,
    }
}

fn saturating_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[must_use]
pub fn policy_group_entry_to_contract(entry: AppPolicyGroupEntry) -> PolicyGroupEntry {
    PolicyGroupEntry {
        members: entry
            .members
            .into_iter()
            .map(|member| PolicyGroupMember {
                profile_id: member.index_id,
                remarks: member.remarks,
            })
            .collect(),
        is_active: entry.is_active,
        group: policy_group_to_contract(entry.group),
    }
}

#[must_use]
pub fn policy_group_runtime_to_contract(
    group_id: String,
    state: RuntimeGroupState,
) -> PolicyGroupRuntime {
    PolicyGroupRuntime {
        group_id,
        now_profile_id: state.now_profile_id,
        members: state
            .members
            .into_iter()
            .map(|member| PolicyGroupRuntimeMember {
                profile_id: member.profile_id,
                remarks: member.remarks,
                delay_ms: member.delay_ms,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_round_trip_through_the_contract() {
        for strategy in [
            GroupStrategy::Selector,
            GroupStrategy::UrlTest,
            GroupStrategy::Fallback,
        ] {
            let group = PolicyGroupItem {
                id: "g".to_string(),
                name: "Asia".to_string(),
                strategy,
                source_subscription_id: Some("sub".to_string()),
                auto_created: true,
                selected_profile_id: Some("a".to_string()),
                test_url: Some("https://probe.example/".to_string()),
                interval_seconds: Some(300),
                tolerance_ms: Some(80),
                sort: 0,
                member_ids: vec!["a".to_string(), "b".to_string()],
            };
            assert_eq!(
                policy_group_from_contract(policy_group_to_contract(group.clone())),
                group
            );
        }

        let negative = PolicyGroupItem {
            interval_seconds: Some(-1),
            ..PolicyGroupItem::default()
        };
        assert_eq!(policy_group_to_contract(negative).interval_seconds, None);
        let huge = PolicyGroup {
            tolerance_ms: Some(u32::MAX),
            ..policy_group_to_contract(PolicyGroupItem::default())
        };
        assert_eq!(
            policy_group_from_contract(huge).tolerance_ms,
            Some(i32::MAX)
        );
    }
}
