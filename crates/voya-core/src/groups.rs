//! Policy groups: named sets of nodes the core chooses between.
//!
//! Groups live in their own tables rather than as a kind of node, and only the
//! active group is generated into a config. Members are the explicit nodes in
//! order, followed by every node of the bound subscription, resolved when the
//! config is built so a subscription update never leaves a group stale.

use std::collections::{BTreeMap, BTreeSet};

use crate::{ProfileItem, DEFAULT_SPEED_PING_TEST_URL};

/// How a group picks the member traffic goes through.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GroupStrategy {
    /// The user picks the member.
    #[default]
    Selector,
    /// The member with the lowest measured delay, within a tolerance.
    UrlTest,
    /// The first reachable member, in list order.
    Fallback,
}

impl GroupStrategy {
    /// The spelling stored in `policy_groups.strategy`.
    #[must_use]
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Selector => "selector",
            Self::UrlTest => "urltest",
            Self::Fallback => "fallback",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "selector" => Some(Self::Selector),
            "urltest" => Some(Self::UrlTest),
            "fallback" => Some(Self::Fallback),
            _ => None,
        }
    }
}

/// Probe URL a urltest or fallback group uses when none is set.
pub const DEFAULT_GROUP_TEST_URL: &str = DEFAULT_SPEED_PING_TEST_URL;
/// Seconds between probes when none is set.
pub const DEFAULT_GROUP_INTERVAL_SECONDS: i32 = 180;
/// Delay difference a urltest group ignores when none is set.
pub const DEFAULT_GROUP_TOLERANCE_MS: i32 = 50;
/// Accepted probe intervals, in seconds.
pub const GROUP_INTERVAL_SECONDS_RANGE: (i32, i32) = (30, 86_400);
/// Accepted urltest tolerances, in milliseconds.
pub const GROUP_TOLERANCE_MS_RANGE: (i32, i32) = (0, 5_000);

/// The tolerance that turns a sing-box urltest into a fallback.
///
/// sing-box has no fallback outbound. Its urltest keeps the current member
/// until another beats it by more than the tolerance, and with no current
/// member adopts the first one in list order that has a successful probe. A
/// tolerance no real delay can reach therefore keeps the earliest reachable
/// member until it fails. Delay and tolerance are added as `uint16`, so the
/// value stays far enough below 65 535 that the sum cannot wrap, which would
/// quietly turn the group back into lowest-delay selection.
pub const FALLBACK_TOLERANCE_MS: u16 = 30_000;

/// Id characters in a member tag before a collision forces the full id.
const MEMBER_TAG_ID_CHARS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PolicyGroupItem {
    pub id: String,
    pub name: String,
    pub strategy: GroupStrategy,
    /// Every node of this subscription joins after the explicit members.
    pub source_subscription_id: Option<String>,
    /// Created by a subscription import rather than by the user.
    pub auto_created: bool,
    /// The selector's chosen member; `None` means the first member.
    pub selected_profile_id: Option<String>,
    pub test_url: Option<String>,
    pub interval_seconds: Option<i32>,
    pub tolerance_ms: Option<i32>,
    pub sort: i32,
    /// Explicit members, in order.
    pub member_ids: Vec<String>,
}

/// What member resolution and member tags read from a node. A caller that
/// needs only a group's members, such as the running group's status polled
/// every few seconds, resolves them from [`ProfileIdentity`] rows instead of
/// decoding every stored profile.
pub trait GroupNode {
    fn index_id(&self) -> &str;
    fn remarks(&self) -> &str;
    fn subscription_id(&self) -> Option<&str>;
}

impl GroupNode for ProfileItem {
    fn index_id(&self) -> &str {
        &self.index_id
    }

    fn remarks(&self) -> &str {
        &self.remarks
    }

    fn subscription_id(&self) -> Option<&str> {
        self.subscription_id.as_deref()
    }
}

/// A node as a policy group sees it: who it is, what it is called, and which
/// subscription owns it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProfileIdentity {
    pub index_id: String,
    pub remarks: String,
    pub subscription_id: Option<String>,
}

impl GroupNode for ProfileIdentity {
    fn index_id(&self) -> &str {
        &self.index_id
    }

    fn remarks(&self) -> &str {
        &self.remarks
    }

    fn subscription_id(&self) -> Option<&str> {
        self.subscription_id.as_deref()
    }
}

/// The group's members as nodes: explicit members first (skipping ones that
/// no longer exist), then the bound subscription's nodes, each node once.
///
/// Explicit members are looked up by id through a map: a scan of `nodes` per
/// member made a group spanning a large subscription quadratic.
#[must_use]
pub fn resolve_group_members<'nodes, N: GroupNode>(
    group: &PolicyGroupItem,
    nodes: &'nodes [N],
) -> Vec<&'nodes N> {
    let mut by_id = BTreeMap::new();
    if !group.member_ids.is_empty() {
        // The first node with an id wins, as the scan it replaced found it.
        for node in nodes {
            by_id.entry(node.index_id()).or_insert(node);
        }
    }
    let explicit = group
        .member_ids
        .iter()
        .filter_map(|id| by_id.get(id.as_str()).copied());
    let bound = group
        .source_subscription_id
        .iter()
        .flat_map(|subscription| {
            nodes
                .iter()
                .filter(move |node| node.subscription_id() == Some(subscription.as_str()))
        });
    let mut seen = BTreeSet::new();
    explicit
        .chain(bound)
        .filter(|node| seen.insert(node.index_id()))
        .collect()
}

/// Outbound tags for `members`, in order: the node name plus a short id in
/// brackets, so a tag can never equal a reserved tag such as `proxy`. A short
/// tag two members would share falls back to the full id.
#[must_use]
pub fn unique_member_tags<N: GroupNode>(members: &[&N]) -> Vec<String> {
    let short: Vec<String> = members
        .iter()
        .map(|member| member_tag(*member, Some(MEMBER_TAG_ID_CHARS)))
        .collect();
    // Counted once up front; counting per member made large groups quadratic.
    let mut uses = BTreeMap::<&str, usize>::new();
    for tag in &short {
        *uses.entry(tag.as_str()).or_default() += 1;
    }
    short
        .iter()
        .zip(members)
        .map(|(tag, member)| {
            if uses.get(tag.as_str()).copied().unwrap_or_default() > 1 {
                member_tag(*member, None)
            } else {
                tag.clone()
            }
        })
        .collect()
}

fn member_tag(node: &impl GroupNode, id_chars: Option<usize>) -> String {
    name_id_tag(node.remarks(), node.index_id(), id_chars)
}

/// `name [id]`, or `[id]` for a blank name, keeping the first `id_chars`
/// characters of the id (all of them for `None`).
pub(crate) fn name_id_tag(name: &str, id: &str, id_chars: Option<usize>) -> String {
    let id: String = match id_chars {
        Some(count) => id.chars().take(count).collect(),
        None => id.to_string(),
    };
    let name = name.trim();
    if name.is_empty() {
        format!("[{id}]")
    } else {
        format!("{name} [{id}]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ActiveTarget, AppConfig};

    fn node(id: &str, remarks: &str, subscription: Option<&str>) -> ProfileItem {
        ProfileItem {
            index_id: id.to_string(),
            remarks: remarks.to_string(),
            subscription_id: subscription.map(str::to_string),
            ..ProfileItem::default()
        }
    }

    #[test]
    fn strategies_round_trip_through_their_stored_spelling() {
        for strategy in [
            GroupStrategy::Selector,
            GroupStrategy::UrlTest,
            GroupStrategy::Fallback,
        ] {
            assert_eq!(
                GroupStrategy::from_db_str(strategy.as_db_str()),
                Some(strategy)
            );
        }
        assert_eq!(GroupStrategy::from_db_str("loadbalance"), None);
    }

    #[test]
    fn members_are_explicit_nodes_then_the_bound_subscription_each_once() {
        let nodes = vec![
            node("a", "A", None),
            node("b", "B", Some("sub")),
            node("c", "C", Some("sub")),
            node("d", "D", Some("other")),
        ];
        let group = PolicyGroupItem {
            member_ids: vec!["c".to_string(), "missing".to_string(), "a".to_string()],
            source_subscription_id: Some("sub".to_string()),
            ..PolicyGroupItem::default()
        };
        let ids: Vec<&str> = resolve_group_members(&group, &nodes)
            .iter()
            .map(|member| member.index_id.as_str())
            .collect();

        assert_eq!(ids, ["c", "a", "b"]);
        assert!(resolve_group_members(&PolicyGroupItem::default(), &nodes).is_empty());
    }

    #[test]
    fn member_tags_stay_distinct_and_never_match_a_reserved_tag() {
        let first = node("12345678aaaa", "Tokyo", None);
        let second = node("12345678bbbb", "Tokyo", None);
        let unnamed = node("99999999", "  ", None);

        assert_eq!(
            unique_member_tags(&[&first, &second, &unnamed]),
            ["Tokyo [12345678aaaa]", "Tokyo [12345678bbbb]", "[99999999]"]
        );
        assert_eq!(unique_member_tags(&[&first]), ["Tokyo [12345678]"]);
    }

    /// The running group's status resolves members from identity rows while
    /// the generator uses full profiles; both must land on the same tags, or
    /// the status would look up delays and selections under the wrong names.
    #[test]
    fn identity_rows_resolve_and_tag_members_exactly_as_profiles_do() {
        let nodes = vec![
            node("12345678aaaa", "Tokyo", Some("sub")),
            node("12345678bbbb", "Tokyo", Some("sub")),
            node("99999999", "Osaka", None),
            node("elsewhere", "Seoul", Some("other")),
        ];
        let identities: Vec<ProfileIdentity> = nodes
            .iter()
            .map(|node| ProfileIdentity {
                index_id: node.index_id.clone(),
                remarks: node.remarks.clone(),
                subscription_id: node.subscription_id.clone(),
            })
            .collect();
        let group = PolicyGroupItem {
            member_ids: vec!["99999999".to_string()],
            source_subscription_id: Some("sub".to_string()),
            ..PolicyGroupItem::default()
        };

        let from_profiles = resolve_group_members(&group, &nodes);
        let from_identities = resolve_group_members(&group, &identities);
        assert_eq!(
            from_profiles
                .iter()
                .map(|node| node.index_id.as_str())
                .collect::<Vec<_>>(),
            from_identities
                .iter()
                .map(|node| node.index_id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            unique_member_tags(&from_profiles),
            unique_member_tags(&from_identities)
        );
    }

    #[test]
    fn a_node_and_a_group_are_never_the_active_target_together() {
        let mut config = AppConfig::default();
        assert_eq!(config.active_target(), ActiveTarget::None);
        config.set_active_node("node");
        assert_eq!(config.active_target(), ActiveTarget::Node("node"));
        config.set_active_group("group");
        assert_eq!(config.active_target(), ActiveTarget::Group("group"));
        assert!(config.index_id.is_empty());
        config.set_active_node("node");
        assert!(config.active_group_id.is_empty());
    }
}
