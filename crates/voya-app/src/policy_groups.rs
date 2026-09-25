//! Saving, activating and selecting policy groups.
//!
//! Groups are stored apart from nodes. What a group contains is resolved
//! against the current node list whenever it is shown or connected, so a
//! subscription update never leaves a group pointing at stale nodes.

use std::collections::BTreeSet;

use thiserror::Error;
use voya_core::{
    resolve_group_members, AppConfig, GroupStrategy, PolicyGroupItem, ProfileIdentity,
    GROUP_INTERVAL_SECONDS_RANGE, GROUP_TOLERANCE_MS_RANGE,
};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};

use crate::config_mutation::{CommittedMutation, ConfigMutationCoordinator};
use voya_contracts::AppError;

/// Longest group name the editor accepts.
pub const GROUP_NAME_MAX_CHARS: usize = 64;
const GROUP_TEST_URL_MAX_CHARS: usize = 2048;
const GROUP_SORT_STEP: i32 = 10;
/// Appended to a subscription's name for the group its first import creates.
const AUTO_GROUP_SUFFIX: &str = " · Auto";

pub type Result<T> = std::result::Result<T, PolicyGroupManagerError>;

#[derive(Debug, Error)]
pub enum PolicyGroupManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    ProxyRuntime(#[from] crate::proxy_runtime::ProxyRuntimeError),
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
///
/// Members are resolved from [`ProfileIdentity`] rows, which are read without
/// decoding the stored payloads. A node this build cannot decode therefore
/// still counts as a member here although the node list and a generated config
/// leave it out; such rows exist only after a downgrade.
#[derive(Debug, Clone)]
pub struct PolicyGroupEntry {
    pub group: PolicyGroupItem,
    pub members: Vec<ProfileIdentity>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PolicyGroupManager<'db> {
    database: DatabaseSession<'db>,
}

impl<'db> PolicyGroupManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self::from_session(DatabaseSession::Database(database))
    }

    #[must_use]
    pub fn new_in(unit_of_work: &'db UnitOfWork) -> Self {
        Self::from_session(DatabaseSession::UnitOfWork(unit_of_work))
    }

    #[must_use]
    pub(crate) const fn from_session(database: DatabaseSession<'db>) -> Self {
        Self { database }
    }

    /// Every group in display order with its resolved members.
    pub async fn list(&self, config: &AppConfig) -> Result<Vec<PolicyGroupEntry>> {
        let nodes = self.database.profiles().list_identities().await?;
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

    /// The group and its members, in the order connecting uses them: what the
    /// running group's status, selection and delay test are spoken in.
    pub async fn resolve(&self, id: &str) -> Result<(PolicyGroupItem, Vec<ProfileIdentity>)> {
        let group = self.get(id).await?;
        let nodes = self.database.profiles().list_identities().await?;
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
        // One listing answers for every member instead of a query each. It
        // decodes nothing, so it also holds rows the node list cannot show.
        let nodes = self.database.profiles().list_identities().await?;
        let listed = nodes
            .iter()
            .map(|node| node.index_id.as_str())
            .collect::<BTreeSet<_>>();
        if let Some(missing) = group
            .member_ids
            .iter()
            .find(|member_id| !listed.contains(member_id.as_str()))
        {
            return Err(PolicyGroupManagerError::MemberNotFound(missing.clone()));
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

    /// Creates the lowest-latency group for a subscription's nodes.
    ///
    /// Callers invoke this only for a subscription's first successful import,
    /// so a group the user deleted does not come back on the next update.
    /// Nothing is created when the setting is off, when an import-created
    /// group for this subscription already exists (even a renamed one), or
    /// when the subscription has no nodes. The group is never activated.
    pub async fn ensure_subscription_auto_group(
        &self,
        config: &AppConfig,
        subscription_id: &str,
    ) -> std::result::Result<Option<PolicyGroupItem>, DbError> {
        if !config.gui_item.auto_create_subscription_group
            || self
                .database
                .policy_groups()
                .auto_created_for_subscription(subscription_id)
                .await?
                .is_some()
            || self
                .database
                .profiles()
                .list_by_subscription_id(Some(subscription_id))
                .await?
                .is_empty()
        {
            return Ok(None);
        }
        let Some(subscription) = self.database.subscriptions().get(subscription_id).await? else {
            return Ok(None);
        };
        let name = auto_group_name(
            if subscription.remarks.trim().is_empty() {
                &subscription.id
            } else {
                &subscription.remarks
            },
            &config.ui_item.current_language,
        );
        let group = PolicyGroupItem {
            id: uuid::Uuid::new_v4().simple().to_string(),
            name,
            strategy: GroupStrategy::UrlTest,
            source_subscription_id: Some(subscription.id),
            auto_created: true,
            sort: self
                .database
                .policy_groups()
                .max_sort()
                .await?
                .saturating_add(GROUP_SORT_STEP),
            ..PolicyGroupItem::default()
        };
        self.database.policy_groups().upsert(&group).await?;
        Ok(Some(group))
    }

    /// Deletes the import-created groups of these subscriptions before the
    /// subscriptions themselves go. Groups the user built keep existing and
    /// only lose their binding. Nothing stays active if one of them was.
    pub async fn delete_auto_groups_for_subscriptions(
        &self,
        config: &mut AppConfig,
        subscription_ids: &[String],
    ) -> std::result::Result<u64, DbError> {
        let deleted = self
            .database
            .policy_groups()
            .delete_auto_created_for_subscriptions(subscription_ids)
            .await?;
        if !config.active_group_id.is_empty()
            && !self
                .database
                .policy_groups()
                .exists(&config.active_group_id)
                .await?
        {
            config.active_group_id.clear();
        }
        Ok(deleted)
    }

    async fn get(&self, id: &str) -> Result<PolicyGroupItem> {
        self.database
            .policy_groups()
            .get(id)
            .await?
            .ok_or_else(|| PolicyGroupManagerError::GroupNotFound(id.to_string()))
    }
}

/// The URL a urltest or fallback group probes with.
#[must_use]
pub fn group_test_url(group: &PolicyGroupItem) -> &str {
    group
        .test_url
        .as_deref()
        .unwrap_or(voya_core::DEFAULT_GROUP_TEST_URL)
}

/// `"{subscription} · Auto"` in the interface language, kept within the
/// editor's name limit.
fn auto_group_name(subscription: &str, language: &str) -> String {
    let suffix = match crate::language::ui_language_for_locale(language) {
        "zh-Hant" => " · 自動",
        "zh-Hans" => " · 自动",
        _ => AUTO_GROUP_SUFFIX,
    };
    let budget = GROUP_NAME_MAX_CHARS - suffix.chars().count();
    let base: String = subscription
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(budget)
        .collect();
    format!("{}{suffix}", base.trim_end())
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

/// Default delay-probe timeout shared by both hosts.
pub const GROUP_DELAY_TIMEOUT_MS: u32 = 5_000;

/// Stores a selector's member and switches the running group live when that
/// is the group the core is using.
///
/// Returns the saved group and, when the live switch failed, the message each
/// host reports its way; the stored choice is kept either way.
pub async fn select_member_use_case(
    mutations: &ConfigMutationCoordinator,
    supervisor: &crate::supervisor::CoreSupervisor,
    policy_groups: &PolicyGroupManager<'_>,
    proxy_runtime: &crate::proxy_runtime::ProxyRuntimeManager,
    group_id: &str,
    profile_id: &str,
) -> std::result::Result<(voya_contracts::PolicyGroup, Option<String>), AppError> {
    let selected = mutations
        .mutate(
            async |unit_of_work, _config| -> std::result::Result<PolicyGroupItem, AppError> {
                Ok(PolicyGroupManager::new_in(unit_of_work)
                    .select_member(group_id, profile_id)
                    .await?)
            },
        )
        .await?;
    let snapshot = supervisor.status().await.map_err(AppError::from)?;
    let live_error = select_member_live_if_running(
        &snapshot,
        policy_groups,
        proxy_runtime,
        group_id,
        profile_id,
    )
    .await
    .map_err(AppError::from)?;
    Ok((
        crate::contract_map::policy_group_to_contract(selected.value),
        live_error,
    ))
}

/// After a member selection is stored, switch the running group live when that
/// is the group the core is using. `Ok(None)` when there was nothing to switch
/// or the switch landed; `Ok(Some(message))` when the live switch failed —
/// the choice is kept either way, and each host reports the message its way.
pub async fn select_member_live_if_running(
    snapshot: &crate::supervisor::SupervisorSnapshot,
    policy_groups: &PolicyGroupManager<'_>,
    proxy_runtime: &crate::proxy_runtime::ProxyRuntimeManager,
    group_id: &str,
    profile_id: &str,
) -> Result<Option<String>> {
    if snapshot.running_group_id().as_deref() != Some(group_id) {
        return Ok(None);
    }
    let (_, members) = policy_groups.resolve(group_id).await?;
    match proxy_runtime
        .select_group_member(&snapshot.clash_api_access(), &members, profile_id)
        .await
    {
        Ok(()) => Ok(None),
        Err(error) => Ok(Some(format!("{error:?}"))),
    }
}

/// The running group as the core sees it, or `None` while no group runs.
///
/// Shared by both hosts so the "which members and delays" body cannot drift;
/// each host keeps only its envelope (Tauri result vs JSON `null`).
pub async fn running_policy_group_runtime(
    snapshot: &crate::supervisor::SupervisorSnapshot,
    policy_groups: &PolicyGroupManager<'_>,
    proxy_runtime: &crate::proxy_runtime::ProxyRuntimeManager,
) -> Result<Option<voya_contracts::PolicyGroupRuntime>> {
    let Some(group_id) = snapshot.running_group_id() else {
        return Ok(None);
    };
    let (_, members) = policy_groups.resolve(&group_id).await?;
    let runtime = proxy_runtime
        .group_state(&snapshot.clash_api_access(), &members)
        .await?;
    Ok(Some(crate::contract_map::policy_group_runtime_to_contract(
        group_id, runtime,
    )))
}

/// Probes every member of the running group and returns the runtime with those
/// delays; `None` while no group runs.
pub async fn test_running_policy_group_delay(
    snapshot: &crate::supervisor::SupervisorSnapshot,
    policy_groups: &PolicyGroupManager<'_>,
    proxy_runtime: &crate::proxy_runtime::ProxyRuntimeManager,
) -> Result<Option<voya_contracts::PolicyGroupRuntime>> {
    let Some(group_id) = snapshot.running_group_id() else {
        return Ok(None);
    };
    let (group, members) = policy_groups.resolve(&group_id).await?;
    let access = snapshot.clash_api_access();
    let delays = proxy_runtime
        .test_group_delay(
            &access,
            &members,
            group_test_url(&group),
            GROUP_DELAY_TIMEOUT_MS,
        )
        .await?;
    let mut runtime = proxy_runtime.group_state(&access, &members).await?;
    for member in &mut runtime.members {
        if let Some(delay) = delays.get(&member.profile_id) {
            member.delay_ms = Some(*delay);
        }
    }
    Ok(Some(crate::contract_map::policy_group_runtime_to_contract(
        group_id, runtime,
    )))
}

/// Saves a group definition.
pub async fn save_policy_group_use_case(
    mutations: &ConfigMutationCoordinator,
    group: voya_contracts::PolicyGroup,
) -> std::result::Result<CommittedMutation<voya_contracts::PolicyGroup>, AppError> {
    use crate::input_safety::{self, map_ipc_input, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS};
    map_ipc_input(
        input_safety::validate_text_list(&group.member_ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "node id",
        voya_contracts::AppErrorSubsystem::PolicyGroup,
    )?;
    mutations
        .mutate(
            async |unit_of_work, _config| -> std::result::Result<_, AppError> {
                Ok(crate::contract_map::policy_group_to_contract(
                    PolicyGroupManager::new_in(unit_of_work)
                        .save(crate::contract_map::policy_group_from_contract(group))
                        .await?,
                ))
            },
        )
        .await
}

/// Deletes groups; the active one may be among them.
pub async fn delete_policy_groups_use_case(
    mutations: &ConfigMutationCoordinator,
    ids: Vec<String>,
) -> std::result::Result<CommittedMutation<u32>, AppError> {
    use crate::input_safety::{self, map_ipc_input, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS};
    map_ipc_input(
        input_safety::validate_text_list(&ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "policy group id",
        voya_contracts::AppErrorSubsystem::PolicyGroup,
    )?;
    mutations
        .mutate(
            async |unit_of_work, config| -> std::result::Result<_, AppError> {
                let deleted = PolicyGroupManager::new_in(unit_of_work)
                    .delete(config, &ids)
                    .await?;
                Ok(u32::try_from(deleted).unwrap_or(u32::MAX))
            },
        )
        .await
}

/// Makes a group what connecting uses.
pub async fn set_active_policy_group_use_case(
    mutations: &ConfigMutationCoordinator,
    id: String,
) -> std::result::Result<CommittedMutation<voya_contracts::PolicyGroup>, AppError> {
    use crate::input_safety::{self, map_ipc_input, IPC_ID_MAX_CHARS};
    map_ipc_input(
        input_safety::validate_required_text(&id, IPC_ID_MAX_CHARS),
        "policy group id",
        voya_contracts::AppErrorSubsystem::PolicyGroup,
    )?;
    mutations
        .mutate(
            async |unit_of_work, config| -> std::result::Result<_, AppError> {
                Ok(crate::contract_map::policy_group_to_contract(
                    PolicyGroupManager::new_in(unit_of_work)
                        .set_active(config, &id)
                        .await?,
                ))
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use voya_core::{ActiveTarget, ProfileItem, SubItem};

    use super::*;
    use crate::profiles::ProfileManager;
    use crate::subscriptions::SubscriptionManager;

    async fn subscription_with_node(database: &Database, id: &str, remarks: &str) {
        database
            .subscriptions()
            .upsert(&SubItem {
                id: id.to_string(),
                remarks: remarks.to_string(),
                url: format!("https://{id}.example/sub"),
                ..SubItem::default()
            })
            .await
            .expect("subscription");
        database
            .profiles()
            .upsert(&ProfileItem {
                index_id: format!("{id}-node"),
                remarks: format!("{remarks} node"),
                subscription_id: Some(id.to_string()),
                ..ProfileItem::default()
            })
            .await
            .expect("subscription node");
    }

    #[tokio::test]
    async fn an_auto_group_is_created_once_only_when_enabled_and_never_activated() {
        let database = database_with_nodes(&[]).await;
        subscription_with_node(&database, "work", "Work").await;
        database
            .subscriptions()
            .upsert(&SubItem {
                id: "empty".to_string(),
                remarks: "Empty".to_string(),
                url: "https://empty.example/sub".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("empty subscription");
        let manager = PolicyGroupManager::new(&database);
        let mut config = AppConfig::default();
        assert!(config.gui_item.auto_create_subscription_group);

        config.gui_item.auto_create_subscription_group = false;
        assert!(manager
            .ensure_subscription_auto_group(&config, "work")
            .await
            .expect("disabled")
            .is_none());
        config.gui_item.auto_create_subscription_group = true;
        let created = manager
            .ensure_subscription_auto_group(&config, "work")
            .await
            .expect("enabled")
            .expect("a group");
        assert_eq!(created.name, "Work · Auto");
        assert_eq!(created.strategy, GroupStrategy::UrlTest);
        assert!(created.auto_created);
        assert!(config.active_group_id.is_empty());
        assert!(manager
            .ensure_subscription_auto_group(&config, "work")
            .await
            .expect("again")
            .is_none());
        assert!(manager
            .ensure_subscription_auto_group(&config, "empty")
            .await
            .expect("no nodes")
            .is_none());
        let entries = manager.list(&config).await.expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].members.len(), 1);
    }

    #[test]
    fn auto_group_names_follow_the_interface_language() {
        assert_eq!(auto_group_name("Work", "en"), "Work · Auto");
        assert_eq!(auto_group_name("机场", "zh-Hans"), "机场 · 自动");
        assert_eq!(auto_group_name("機場", "zh-Hant"), "機場 · 自動");
    }

    #[test]
    fn auto_group_names_stay_within_the_limit() {
        let long = "界".repeat(GROUP_NAME_MAX_CHARS * 2);
        let name = auto_group_name(&long, "en");
        assert_eq!(name.chars().count(), GROUP_NAME_MAX_CHARS);
        assert!(name.ends_with(AUTO_GROUP_SUFFIX));
        assert_eq!(auto_group_name(" Work\n ", "en"), "Work · Auto");
    }

    #[tokio::test]
    async fn deleting_a_subscription_removes_only_its_auto_group() {
        let database = database_with_nodes(&["a"]).await;
        subscription_with_node(&database, "work", "Work").await;
        let manager = PolicyGroupManager::new(&database);
        let mut config = AppConfig::default();
        let auto = manager
            .ensure_subscription_auto_group(&config, "work")
            .await
            .expect("auto group")
            .expect("created");
        let user = manager
            .save(PolicyGroupItem {
                source_subscription_id: Some("work".to_string()),
                ..draft(&["a"])
            })
            .await
            .expect("user group");
        config.set_active_group(&auto.id);

        SubscriptionManager::new(&database)
            .delete_subscriptions(&mut config, &["work".to_string()])
            .await
            .expect("delete subscription");

        let remaining = manager.list(&config).await.expect("list");
        assert_eq!(
            remaining
                .iter()
                .map(|entry| entry.group.id.as_str())
                .collect::<Vec<_>>(),
            [user.id.as_str()]
        );
        assert_eq!(remaining[0].group.source_subscription_id, None);
        assert!(config.active_group_id.is_empty());
    }

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

    /// A row this build cannot decode is skipped by the node listings but is
    /// still a node: it has an id, it counts as a member, and a downgrade must
    /// not make every group holding one unsavable. Validating against
    /// `list_identities` (which decodes nothing) is what makes this work — it
    /// is why the extra per-member `exists()` query could go.
    #[tokio::test]
    async fn a_member_that_only_exists_as_an_undecodable_row_is_accepted() {
        let database = database_with_nodes(&["a"]).await;
        // A `config_type` this build has no enum value for, as a row written by
        // a newer build would look.
        sqlx::query(
            r#"INSERT INTO profile_items (index_id, config_type, remarks, protocol)
               VALUES ('from-the-future', 'quantum', 'From the future',
                       '{"kind":"trojan","server":{"address":"t.example.com","port":443},"password":"secret"}')"#,
        )
        .execute(database.pool())
        .await
        .expect("the raw row should be stored");
        assert!(
            database
                .profiles()
                .list()
                .await
                .expect("listing")
                .iter()
                .all(|node| node.index_id != "from-the-future"),
            "the row has to be undecodable for this test to mean anything"
        );

        let saved = PolicyGroupManager::new(&database)
            .save(draft(&["a", "from-the-future"]))
            .await
            .expect("an undecodable member must not block the save");

        assert_eq!(saved.member_ids, ["a", "from-the-future"]);
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
