//! Data-ownership rules for subscription imports and deletions.
//!
//! Two rules keep a subscription from damaging rows it does not own:
//!
//! * An import may adopt a profile only when the row is manual or already
//!   belongs to the subscription being imported. Re-homing a row owned by a
//!   *different* subscription makes a server offered by two providers flip
//!   between them — and between their `Auto` groups — on every update.
//! * Deleting a subscription removes only the policy groups it created. A
//!   group the user built (explicit children, or a filter) merely loses its
//!   dynamic source instead of being destroyed with the subscription.

use voya_core::{ProfileItem, ProfileProtocol};

/// Whether an import targeting `target_subscription_id` may reuse `existing`
/// as the canonical row for an incoming profile.
///
/// Manual rows (`subscription_id == None`) are adoptable by any import, which
/// is how a hand-added node folds into the subscription that also offers it.
/// A row owned by another subscription is never adopted, so the incoming
/// profile becomes a new row of the target subscription instead.
pub(super) fn profile_is_adoptable(
    existing: &ProfileItem,
    target_subscription_id: Option<&str>,
) -> bool {
    match existing.subscription_id.as_deref() {
        None => true,
        Some(owner) => target_subscription_id == Some(owner),
    }
}

/// What deleting a set of subscriptions should do to the policy groups that
/// point at them.
#[derive(Debug, Default)]
pub(super) struct SubscriptionGroupCleanup {
    /// Auto groups created by `ensure_subscription_auto_group`, which carry no
    /// user-authored content and are meaningless without their source.
    pub(super) deleted_index_ids: Vec<String>,
    /// User-authored groups, rewritten with `source_subscription_id` cleared so
    /// their explicit children and filters survive the deletion.
    pub(super) detached_groups: Vec<ProfileItem>,
}

pub(super) fn plan_subscription_group_cleanup(
    profiles: &[ProfileItem],
    removed_subscription_ids: &[String],
) -> SubscriptionGroupCleanup {
    let mut cleanup = SubscriptionGroupCleanup::default();
    for profile in profiles {
        let ProfileProtocol::PolicyGroup {
            child_profile_ids,
            source_subscription_id: Some(source),
            filter,
            ..
        } = &profile.protocol
        else {
            continue;
        };
        if !removed_subscription_ids.contains(source) {
            continue;
        }

        if child_profile_ids.is_empty() && filter.is_none() {
            cleanup.deleted_index_ids.push(profile.index_id.clone());
            continue;
        }

        let mut detached = profile.clone();
        if let ProfileProtocol::PolicyGroup {
            source_subscription_id,
            ..
        } = &mut detached.protocol
        {
            *source_subscription_id = None;
        }
        cleanup.detached_groups.push(detached);
    }

    cleanup
}

#[cfg(test)]
mod tests {
    use voya_core::MultipleLoad;

    use super::*;

    fn group(
        index_id: &str,
        source: Option<&str>,
        children: &[&str],
        filter: Option<&str>,
    ) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            remarks: index_id.to_string(),
            protocol: ProfileProtocol::PolicyGroup {
                child_profile_ids: children.iter().copied().map(str::to_string).collect(),
                source_subscription_id: source.map(str::to_string),
                filter: filter.map(str::to_string),
                strategy: MultipleLoad::LeastPing,
            },
            ..ProfileItem::default()
        }
    }

    fn node(index_id: &str, subscription_id: Option<&str>) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            subscription_id: subscription_id.map(str::to_string),
            ..ProfileItem::default()
        }
    }

    #[test]
    fn only_manual_rows_and_the_target_subscriptions_own_rows_are_adoptable() {
        assert!(profile_is_adoptable(&node("manual", None), Some("a")));
        assert!(profile_is_adoptable(&node("manual", None), None));
        assert!(profile_is_adoptable(&node("owned", Some("a")), Some("a")));
        assert!(
            !profile_is_adoptable(&node("owned", Some("b")), Some("a")),
            "a row owned by another subscription must never be re-homed"
        );
        assert!(
            !profile_is_adoptable(&node("owned", Some("b")), None),
            "a manual import must not detach a subscription's row"
        );
    }

    #[test]
    fn deleting_a_subscription_drops_auto_groups_but_only_detaches_user_groups() {
        let profiles = vec![
            group("auto", Some("gone"), &[], None),
            group("with-children", Some("gone"), &["node-1"], None),
            group("with-filter", Some("gone"), &[], Some("US")),
            group("other-source", Some("kept"), &[], None),
            group("sourceless", None, &["node-1"], None),
            node("plain", Some("gone")),
        ];

        let cleanup = plan_subscription_group_cleanup(&profiles, &["gone".to_string()]);

        assert_eq!(cleanup.deleted_index_ids, vec!["auto".to_string()]);
        assert_eq!(
            cleanup
                .detached_groups
                .iter()
                .map(|group| group.index_id.as_str())
                .collect::<Vec<_>>(),
            vec!["with-children", "with-filter"]
        );
        assert!(cleanup.detached_groups.iter().all(|group| matches!(
            &group.protocol,
            ProfileProtocol::PolicyGroup {
                source_subscription_id: None,
                ..
            }
        )));
        assert_eq!(
            match &cleanup.detached_groups[0].protocol {
                ProfileProtocol::PolicyGroup {
                    child_profile_ids, ..
                } => child_profile_ids.clone(),
                _ => Vec::new(),
            },
            vec!["node-1".to_string()],
            "detaching must keep the user's explicit children"
        );
    }
}
