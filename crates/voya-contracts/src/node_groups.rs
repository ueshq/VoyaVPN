use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeGroup {
    pub id: String,
    pub name: String,
    pub sort: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeGroupMembership {
    pub profile_id: String,
    pub group_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeGroupsSnapshot {
    pub groups: Vec<NodeGroup>,
    pub memberships: Vec<NodeGroupMembership>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeGroupAssignment {
    pub profile_id: String,
    pub group_id: Option<String>,
}
