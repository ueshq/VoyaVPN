//! Policy groups: named node sets the core chooses between.

use serde::{Deserialize, Serialize};
use specta::Type;

/// How a group picks the member traffic goes through.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PolicyGroupStrategy {
    /// The user picks the member.
    #[default]
    Selector,
    /// The member with the lowest measured delay, within a tolerance.
    UrlTest,
    /// The first reachable member, in list order.
    Fallback,
}

/// A stored group as the editor reads and writes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyGroup {
    /// Empty when creating a group.
    pub id: String,
    pub name: String,
    pub strategy: PolicyGroupStrategy,
    /// Every node of this subscription joins after the explicit members.
    pub source_subscription_id: Option<String>,
    /// Created by a subscription import rather than by the user.
    pub auto_created: bool,
    /// The member a selector uses; `None` means the first member.
    pub selected_profile_id: Option<String>,
    pub test_url: Option<String>,
    pub interval_seconds: Option<u32>,
    pub tolerance_ms: Option<u32>,
    /// Explicit members, in order.
    pub member_ids: Vec<String>,
}

/// One resolved member of a group.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyGroupMember {
    pub profile_id: String,
    pub remarks: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyGroupEntry {
    pub group: PolicyGroup,
    /// Members resolved against the current node list, in connection order.
    pub members: Vec<PolicyGroupMember>,
    /// Whether connecting uses this group.
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyGroupListing {
    pub entries: Vec<PolicyGroupEntry>,
}

/// One member of the running group, with the core's last measurement.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyGroupRuntimeMember {
    pub profile_id: String,
    pub remarks: String,
    /// `None` before a probe, or after a failed one.
    pub delay_ms: Option<u32>,
}

/// The running group as the core reports it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyGroupRuntime {
    pub group_id: String,
    /// The member traffic goes through right now.
    pub now_profile_id: Option<String>,
    pub members: Vec<PolicyGroupRuntimeMember>,
}
