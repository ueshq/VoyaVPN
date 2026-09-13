//! Saving, activating and selecting policy groups.
//!
//! Groups are stored apart from nodes. What a group contains is resolved
//! against the current node list whenever it is shown or connected, so a
//! subscription update never leaves a group pointing at stale nodes.

use std::collections::BTreeSet;

use thiserror::Error;
use voya_core::{
    resolve_group_members, AppConfig, PolicyGroupItem, ProfileItem, GROUP_INTERVAL_SECONDS_RANGE,
    GROUP_TOLERANCE_MS_RANGE,
};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};

/// Longest group name the editor accepts.
pub const GROUP_NAME_MAX_CHARS: usize = 64;
const GROUP_TEST_URL_MAX_CHARS: usize = 2048;
const GROUP_SORT_STEP: i32 = 10;

pub type Result<T> = std::result::Result<T, PolicyGroupManagerError>;

#[derive(Debug, Error)]
pub enum PolicyGroupManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("policy group {0} was not found")]
    GroupNotFound(String),
    #[error("a policy group needs a name")]
    NameRequired,
    #[error("a policy group name is at most {max} characters")]
    NameTooLong { max: usize },
    #[error("a policy group name cannot contain control characters")]
    NameControlCharacters,
    #[error("a policy group needs at least one node or a subscription")]
    WithoutMembers,
    #[error("node {0} was not found")]
    MemberNotFound(String),
    #[error("subscription {0} was not found")]
    SubscriptionNotFound(String),
    #[error("the probe interval must be between {min} and {max} seconds")]
    IntervalOutOfRange { min: i32, max: i32 },
    #[error("the tolerance must be between {min} and {max} ms")]
    ToleranceOutOfRange { min: i32, max: i32 },
    #[error("the test URL must be an http or https URL")]
    TestUrlInvalid,
    #[error("node {profile_id} is not a member of policy group {group_id}")]
    NotAMember {
        group_id: String,
        profile_id: String,
    },
}

/// A group with its members resolved against the current node list.
#[derive(Debug, Clone)]
pub struct PolicyGroupEntry {
    pub group: PolicyGroupItem,
    pub members: Vec<ProfileItem>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PolicyGroupManager<'db> {
    database: DatabaseSession<'db>,
}

impl<'db> PolicyGroupManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self::from_session(DatabaseSession::from_database(database))
    }

    #[must_use]
    pub fn new_in(unit_of_work: &'db UnitOfWork) -> Self {
        Self::from_session(DatabaseSession::from_unit_of_work(unit_of_work))
    }

    #[must_use]
    pub(crate) const fn from_session(database: DatabaseSession<'db>) -> Self {
        Self { database }
    }

    /// Every group in display order with its resolved members.
    pub async fn list(&self, config: &AppConfig) -> Result<Vec<PolicyGroupEntry>> {
        let nodes = self.database.profiles().list().await?;
        let groups = self.database.policy_groups().list().await?;
        Ok(groups
            .into_iter()
            .map(|group| PolicyGroupEntry {
                members: resolve_group_members(&group, &nodes)
                    .into_iter()
                    .cloned()
                    .collect(),
                is_active: config.active_group_id == group.id,
                group,
            })
            .collect())
    }

    /// The group and its members, in the order connecting uses them.
    pub async fn resolve(&self, id: &str) -> Result<(PolicyGroupItem, Vec<ProfileItem>)> {
        let group = self.get(id).await?;
        let nodes = self.database.profiles().list().await?;
        let members = resolve_group_members(&group, &nodes)
            .into_iter()
            .cloned()
            .collect();
        Ok((group, members))
    }

    /// Validates and stores a group; an empty `id` creates one.
    ///
    /// Explicit members that do not exist are rejected rather than dropped, so
    /// an editor opened before a node was deleted cannot quietly save a smaller
    /// group than the one on screen.
    pub async fn save(&self, draft: PolicyGroupItem) -> Result<PolicyGroupItem> {
        let mut group = normalized(draft);
        validate(&group)?;
        for member_id in &group.member_ids {
            if !self.database.profiles().exists(member_id).await? {
                return Err(PolicyGroupManagerError::MemberNotFound(member_id.clone()));
            }
        }
        if let Some(subscription_id) = &group.source_subscription_id {
            if self
                .database
                .subscriptions()
                .get(subscription_id)
                .await?
                .is_none()
            {
                return Err(PolicyGroupManagerError::SubscriptionNotFound(
                    subscription_id.clone(),
                ));
            }
        }
        if group.id.is_empty() {
            group.id = uuid::Uuid::new_v4().simple().to_string();
            group.sort = self
                .database
                .policy_groups()
                .max_sort()
                .await?
                .saturating_add(GROUP_SORT_STEP);
            group.auto_created = false;
        } else {
            let existing = self.get(&group.id).await?;
            group.sort = existing.sort;
            group.auto_created = existing.auto_created;
        }
        let nodes = self.database.profiles().list().await?;
        let members = resolve_group_members(&group, &nodes);
        // A saved choice that is no longer a member would be ignored by the
        // generator anyway; dropping it keeps the stored group honest.
        if group
            .selected_profile_id
            .as_deref()
            .is_some_and(|selected| !members.iter().any(|member| member.index_id == selected))
        {
            group.selected_profile_id = None;
        }
        self.database.policy_groups().upsert(&group).await?;
        Ok(group)
    }

    /// Deletes groups. Deleting the active one leaves nothing active.
    pub async fn delete(&self, config: &mut AppConfig, ids: &[String]) -> Result<u64> {
        let deleted = self.database.policy_groups().delete_many(ids).await?;
        if ids.contains(&config.active_group_id) {
            config.active_group_id.clear();
        }
        Ok(deleted)
    }

    /// Makes the group what connecting uses, replacing an active node.
    pub async fn set_active(&self, config: &mut AppConfig, id: &str) -> Result<PolicyGroupItem> {
        let group = self.get(id).await?;
        config.set_active_group(id);
        Ok(group)
    }

    /// Stores the member a selector group uses.
    pub async fn select_member(&self, group_id: &str, profile_id: &str) -> Result<PolicyGroupItem> {
        let (mut group, members) = self.resolve(group_id).await?;
        if !members.iter().any(|member| member.index_id == profile_id) {
            return Err(PolicyGroupManagerError::NotAMember {
                group_id: group_id.to_string(),
                profile_id: profile_id.to_string(),
            });
        }
        self.database
            .policy_groups()
            .set_selected_member(group_id, Some(profile_id))
            .await?;
        group.selected_profile_id = Some(profile_id.to_string());
        Ok(group)
    }

    async fn get(&self, id: &str) -> Result<PolicyGroupItem> {
        self.database
            .policy_groups()
            .get(id)
            .await?
            .ok_or_else(|| PolicyGroupManagerError::GroupNotFound(id.to_string()))
    }
}

fn normalized(mut group: PolicyGroupItem) -> PolicyGroupItem {
    let trimmed = |value: Option<String>| {
        value
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    group.id = group.id.trim().to_string();
    group.name = group.name.trim().to_string();
    group.source_subscription_id = trimmed(group.source_subscription_id);
    group.selected_profile_id = trimmed(group.selected_profile_id);
    group.test_url = trimmed(group.test_url);
    let mut seen = BTreeSet::new();
    group.member_ids = group
        .member_ids
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect();
    group
}

fn validate(group: &PolicyGroupItem) -> Result<()> {
    if group.name.is_empty() {
        return Err(PolicyGroupManagerError::NameRequired);
    }
    if group.name.chars().count() > GROUP_NAME_MAX_CHARS {
        return Err(PolicyGroupManagerError::NameTooLong {
            max: GROUP_NAME_MAX_CHARS,
        });
    }
    if group.name.chars().any(char::is_control) {
        return Err(PolicyGroupManagerError::NameControlCharacters);
    }
    if group.member_ids.is_empty() && group.source_subscription_id.is_none() {
        return Err(PolicyGroupManagerError::WithoutMembers);
    }
    let (min_interval, max_interval) = GROUP_INTERVAL_SECONDS_RANGE;
    if group
        .interval_seconds
        .is_some_and(|value| !(min_interval..=max_interval).contains(&value))
    {
        return Err(PolicyGroupManagerError::IntervalOutOfRange {
            min: min_interval,
            max: max_interval,
        });
    }
    let (min_tolerance, max_tolerance) = GROUP_TOLERANCE_MS_RANGE;
    if group
        .tolerance_ms
        .is_some_and(|value| !(min_tolerance..=max_tolerance).contains(&value))
    {
        return Err(PolicyGroupManagerError::ToleranceOutOfRange {
            min: min_tolerance,
            max: max_tolerance,
        });
    }
    if let Some(url) = &group.test_url {
        let valid = url.chars().count() <= GROUP_TEST_URL_MAX_CHARS
            && !url.chars().any(char::is_whitespace)
            && ["http://", "https://"].iter().any(|scheme| {
                url.strip_prefix(scheme)
                    .is_some_and(|rest| !rest.is_empty())
            });
        if !valid {
            return Err(PolicyGroupManagerError::TestUrlInvalid);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use voya_core::{ActiveTarget, GroupStrategy};

    use super::*;
    use crate::profiles::ProfileManager;

    async fn database_with_nodes(ids: &[&str]) -> Database {
        let database = Database::connect_in_memory().await.expect("database");
        for id in ids {
            database
                .profiles()
                .upsert(&ProfileItem {
                    index_id: (*id).to_string(),
                    remarks: id.to_uppercase(),
                    ..ProfileItem::default()
                })
                .await
                .expect("node");
        }
        database
    }

    fn draft(members: &[&str]) -> PolicyGroupItem {
        PolicyGroupItem {
            name: "  Asia ".to_string(),
            strategy: GroupStrategy::Selector,
            member_ids: members.iter().map(|id| (*id).to_string()).collect(),
            ..PolicyGroupItem::default()
        }
    }

    #[tokio::test]
    async fn saving_normalizes_assigns_an_id_and_drops_a_stale_selection() {
        let database = database_with_nodes(&["a", "b", "c"]).await;
        let manager = PolicyGroupManager::new(&database);
        let mut group = draft(&["c", " a ", "c", ""]);
        group.selected_profile_id = Some("a".to_string());
        let saved = manager.save(group).await.expect("save");
        assert!(!saved.id.is_empty());
        assert_eq!(saved.name, "Asia");
        assert_eq!(saved.member_ids, ["c", "a"]);
        assert_eq!(saved.selected_profile_id.as_deref(), Some("a"));
        assert_eq!(saved.sort, GROUP_SORT_STEP);

        let mut edited = saved.clone();
        edited.member_ids = vec!["b".to_string()];
        let resaved = manager.save(edited).await.expect("resave");
        assert_eq!(resaved.sort, saved.sort);
        assert_eq!(resaved.selected_profile_id, None);

        let mut config = AppConfig::default();
        config.set_active_group(&resaved.id);
        let entries = manager.list(&config).await.expect("list");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_active);
        assert_eq!(
            entries[0]
                .members
                .iter()
                .map(|member| member.index_id.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }

    #[tokio::test]
    async fn invalid_groups_are_rejected_with_the_reason() {
        let database = database_with_nodes(&["a"]).await;
        let manager = PolicyGroupManager::new(&database);
        let rejected = |group| manager.save(group);

        assert!(matches!(
            rejected(PolicyGroupItem {
                name: " ".to_string(),
                ..draft(&["a"])
            })
            .await,
            Err(PolicyGroupManagerError::NameRequired)
        ));
        assert!(matches!(
            rejected(draft(&[])).await,
            Err(PolicyGroupManagerError::WithoutMembers)
        ));
        assert!(matches!(
            rejected(draft(&["missing"])).await,
            Err(PolicyGroupManagerError::MemberNotFound(id)) if id == "missing"
        ));
        assert!(matches!(
            rejected(PolicyGroupItem {
                interval_seconds: Some(5),
                ..draft(&["a"])
            })
            .await,
            Err(PolicyGroupManagerError::IntervalOutOfRange { .. })
        ));
        assert!(matches!(
            rejected(PolicyGroupItem {
                tolerance_ms: Some(-1),
                ..draft(&["a"])
            })
            .await,
            Err(PolicyGroupManagerError::ToleranceOutOfRange { .. })
        ));
        assert!(matches!(
            rejected(PolicyGroupItem {
                test_url: Some("ftp://probe.example".to_string()),
                ..draft(&["a"])
            })
            .await,
            Err(PolicyGroupManagerError::TestUrlInvalid)
        ));
        assert!(matches!(
            rejected(PolicyGroupItem {
                source_subscription_id: Some("nope".to_string()),
                ..draft(&[])
            })
            .await,
            Err(PolicyGroupManagerError::SubscriptionNotFound(_))
        ));
        assert!(matches!(
            rejected(PolicyGroupItem {
                id: "unknown".to_string(),
                ..draft(&["a"])
            })
            .await,
            Err(PolicyGroupManagerError::GroupNotFound(_))
        ));
        assert!(manager
            .list(&AppConfig::default())
            .await
            .expect("list")
            .is_empty());
    }

    #[tokio::test]
    async fn activating_selecting_and_deleting_keep_the_config_consistent() {
        let database = database_with_nodes(&["a", "b"]).await;
        let manager = PolicyGroupManager::new(&database);
        let group = manager.save(draft(&["a", "b"])).await.expect("save");
        let mut config = AppConfig::default();
        config.set_active_node("a");

        manager
            .set_active(&mut config, &group.id)
            .await
            .expect("activate");
        assert_eq!(config.active_target(), ActiveTarget::Group(&group.id));
        assert!(!ProfileManager::new(&database)
            .ensure_active_profile(&mut config)
            .await
            .expect("ensure"));
        assert_eq!(config.active_group_id, group.id);

        assert!(matches!(
            manager.select_member(&group.id, "zzz").await,
            Err(PolicyGroupManagerError::NotAMember { .. })
        ));
        let selected = manager.select_member(&group.id, "b").await.expect("select");
        assert_eq!(selected.selected_profile_id.as_deref(), Some("b"));

        assert_eq!(
            manager
                .delete(&mut config, std::slice::from_ref(&group.id))
                .await
                .expect("delete"),
            1
        );
        assert!(config.active_group_id.is_empty());
        assert!(matches!(
            manager.set_active(&mut config, &group.id).await,
            Err(PolicyGroupManagerError::GroupNotFound(_))
        ));
    }
}
