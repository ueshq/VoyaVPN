use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use voya_core::{
    profile_items_match, AppConfig, MoveAction, ProfileDedupeResult, ProfileExItem, ProfileItem,
    ProfileListItem, ProfileSortKey, ServerStatItem,
};
use voya_db::{Database, DbError};

use super::{
    ProfileExManager, DEFAULT_NETWORK, DEFAULT_PROFILE_SORT_STEP, STREAM_SECURITY_REALITY,
    STREAM_SECURITY_TLS, VALID_NETWORKS,
};

static PROFILE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub type Result<T> = std::result::Result<T, ProfileManagerError>;

#[derive(Debug, Error)]
pub enum ProfileManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("profile {0} was not found")]
    ProfileNotFound(String),
    #[error("profile id is required")]
    MissingProfileId,
    #[error("cannot move profile {index_id}: {reason}")]
    InvalidMove { index_id: String, reason: String },
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileManager<'db> {
    database: &'db Database,
}

impl<'db> ProfileManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self { database }
    }

    #[must_use]
    pub fn profile_ex(&self) -> ProfileExManager<'db> {
        ProfileExManager::new(self.database)
    }

    pub async fn list_profiles(
        &self,
        config: &AppConfig,
        subid: Option<&str>,
        filter: Option<&str>,
    ) -> Result<Vec<ProfileListItem>> {
        let items = self.database.profiles().list_with_profile_ex(subid).await?;
        let stats = self.server_stats_by_index_id().await?;
        let filter = filter.map(str::trim).filter(|value| !value.is_empty());

        Ok(items
            .into_iter()
            .filter(|(profile, _)| {
                filter.map_or(true, |filter| {
                    contains_case_insensitive(&profile.remarks, filter)
                        || contains_case_insensitive(&profile.address, filter)
                })
            })
            .map(|(profile, profile_ex)| {
                let server_stat = stats
                    .get(&profile.index_id)
                    .cloned()
                    .unwrap_or_else(|| empty_server_stat(&profile.index_id));
                to_list_item(profile, profile_ex, server_stat, &config.index_id)
            })
            .collect())
    }

    pub async fn get_profile(
        &self,
        config: &AppConfig,
        index_id: &str,
    ) -> Result<Option<ProfileListItem>> {
        let Some(profile) = self.database.profiles().get(index_id).await? else {
            return Ok(None);
        };
        let profile_ex = self.profile_ex().ensure(index_id).await?;
        let server_stat = self
            .database
            .server_stats()
            .get(index_id)
            .await?
            .unwrap_or_else(|| empty_server_stat(index_id));

        Ok(Some(to_list_item(
            profile,
            profile_ex,
            server_stat,
            &config.index_id,
        )))
    }

    pub async fn save_profile(
        &self,
        config: &mut AppConfig,
        mut profile: ProfileItem,
    ) -> Result<ProfileListItem> {
        let is_new = if profile.index_id.trim().is_empty() {
            profile.index_id = generate_profile_id();
            true
        } else {
            !self.database.profiles().exists(&profile.index_id).await?
        };

        normalize_profile(config, &mut profile);

        let profile_ex = if is_new {
            ProfileExItem {
                index_id: profile.index_id.clone(),
                sort: self.profile_ex().get_max_sort().await? + DEFAULT_PROFILE_SORT_STEP,
                ..ProfileExItem::default()
            }
        } else {
            let mut existing = self.profile_ex().ensure(&profile.index_id).await?;
            existing.index_id.clone_from(&profile.index_id);
            existing
        };

        self.database
            .profiles()
            .upsert_with_profile_ex(&profile, &profile_ex)
            .await?;
        self.ensure_active_profile(config).await?;

        let server_stat = self
            .database
            .server_stats()
            .get(&profile.index_id)
            .await?
            .unwrap_or_else(|| empty_server_stat(&profile.index_id));

        Ok(to_list_item(
            profile,
            profile_ex,
            server_stat,
            &config.index_id,
        ))
    }

    pub async fn delete_profiles(
        &self,
        config: &mut AppConfig,
        index_ids: &[String],
    ) -> Result<u64> {
        let deleted = self.database.profiles().delete_many(index_ids).await?;
        self.ensure_active_profile(config).await?;

        Ok(deleted)
    }

    pub async fn copy_profiles(
        &self,
        config: &mut AppConfig,
        index_ids: &[String],
    ) -> Result<Vec<ProfileListItem>> {
        let mut copied = Vec::new();
        let mut next_sort = self.profile_ex().get_max_sort().await? + DEFAULT_PROFILE_SORT_STEP;

        for index_id in index_ids {
            let Some(source) = self.database.profiles().get(index_id).await? else {
                continue;
            };
            let mut profile = source.clone();
            profile.index_id = generate_profile_id();
            profile.remarks = format!("{}-clone", source.remarks);
            normalize_profile(config, &mut profile);

            let profile_ex = ProfileExItem {
                index_id: profile.index_id.clone(),
                sort: next_sort,
                ..ProfileExItem::default()
            };
            next_sort += DEFAULT_PROFILE_SORT_STEP;
            self.database
                .profiles()
                .upsert_with_profile_ex(&profile, &profile_ex)
                .await?;
            let server_stat = self
                .database
                .server_stats()
                .clone_stat(&source.index_id, &profile.index_id)
                .await?
                .unwrap_or_else(|| empty_server_stat(&profile.index_id));
            copied.push(to_list_item(
                profile,
                profile_ex,
                server_stat,
                &config.index_id,
            ));
        }

        self.ensure_active_profile(config).await?;

        Ok(copied)
    }

    pub async fn set_active_profile(
        &self,
        config: &mut AppConfig,
        index_id: &str,
    ) -> Result<ProfileListItem> {
        if index_id.trim().is_empty() {
            return Err(ProfileManagerError::MissingProfileId);
        }

        let Some(profile) = self.database.profiles().get(index_id).await? else {
            return Err(ProfileManagerError::ProfileNotFound(index_id.to_string()));
        };
        let profile_ex = self.profile_ex().ensure(index_id).await?;
        let server_stat = self
            .database
            .server_stats()
            .get(index_id)
            .await?
            .unwrap_or_else(|| empty_server_stat(index_id));
        config.index_id = index_id.to_string();

        Ok(to_list_item(
            profile,
            profile_ex,
            server_stat,
            &config.index_id,
        ))
    }

    pub async fn move_profile(
        &self,
        config: &AppConfig,
        subid: Option<&str>,
        index_id: &str,
        action: MoveAction,
        position: Option<i32>,
    ) -> Result<Vec<ProfileListItem>> {
        let items = self.database.profiles().list_with_profile_ex(subid).await?;
        let Some(index) = items
            .iter()
            .position(|(profile, _)| profile.index_id == index_id)
        else {
            return Err(ProfileManagerError::ProfileNotFound(index_id.to_string()));
        };

        for (offset, (profile, _)) in items.iter().enumerate() {
            self.profile_ex()
                .set_sort(
                    &profile.index_id,
                    (i32::try_from(offset).unwrap_or(i32::MAX - 1) + 1) * DEFAULT_PROFILE_SORT_STEP,
                )
                .await?;
        }

        let count = items.len();
        let next_sort = match action {
            MoveAction::Top if index == 0 => None,
            MoveAction::Top => Some(DEFAULT_PROFILE_SORT_STEP - 1),
            MoveAction::Up if index == 0 => None,
            MoveAction::Up => Some(
                i32::try_from(index).unwrap_or(i32::MAX / DEFAULT_PROFILE_SORT_STEP)
                    * DEFAULT_PROFILE_SORT_STEP
                    - 1,
            ),
            MoveAction::Down if index + 1 >= count => None,
            MoveAction::Down => Some(
                (i32::try_from(index).unwrap_or(i32::MAX / DEFAULT_PROFILE_SORT_STEP) + 2)
                    * DEFAULT_PROFILE_SORT_STEP
                    + 1,
            ),
            MoveAction::Bottom if index + 1 >= count => None,
            MoveAction::Bottom => Some(
                i32::try_from(count).unwrap_or(i32::MAX / DEFAULT_PROFILE_SORT_STEP)
                    * DEFAULT_PROFILE_SORT_STEP
                    + 1,
            ),
            MoveAction::Position => {
                Some(position.unwrap_or_default() * DEFAULT_PROFILE_SORT_STEP + 1)
            }
        };

        if let Some(sort) = next_sort {
            self.profile_ex().set_sort(index_id, sort).await?;
        }

        self.list_profiles(&config, subid, None).await
    }

    pub async fn sort_profiles(
        &self,
        config: &AppConfig,
        subid: Option<&str>,
        sort_key: ProfileSortKey,
        ascending: bool,
    ) -> Result<Vec<ProfileListItem>> {
        let mut items = self.database.profiles().list_with_profile_ex(subid).await?;
        sort_profile_pairs(&mut items, sort_key, ascending);

        for (offset, (profile, _)) in items.iter().enumerate() {
            self.profile_ex()
                .set_sort(
                    &profile.index_id,
                    (i32::try_from(offset).unwrap_or(i32::MAX - 1) + 1) * DEFAULT_PROFILE_SORT_STEP,
                )
                .await?;
        }

        self.list_profiles(config, subid, None).await
    }

    pub async fn move_profiles_to_group(&self, index_ids: &[String], subid: &str) -> Result<u64> {
        Ok(self
            .database
            .profiles()
            .update_subid_many(index_ids, subid)
            .await?)
    }

    pub async fn dedupe_profiles(
        &self,
        config: &mut AppConfig,
        subid: Option<&str>,
        keep_older: bool,
    ) -> Result<ProfileDedupeResult> {
        let mut profiles = self.database.profiles().list_by_subid(subid).await?;
        let total = profiles.len();
        if !keep_older {
            profiles.reverse();
        }

        let mut kept = Vec::<ProfileItem>::new();
        let mut removed_index_ids = Vec::new();

        for profile in profiles {
            if profile.is_complex() {
                kept.push(profile);
                continue;
            }

            if kept
                .iter()
                .any(|existing| profile_items_match(existing, &profile, false))
            {
                removed_index_ids.push(profile.index_id);
            } else {
                kept.push(profile);
            }
        }

        self.database
            .profiles()
            .delete_many(&removed_index_ids)
            .await?;
        self.ensure_active_profile(config).await?;

        Ok(ProfileDedupeResult {
            total: u32::try_from(total).unwrap_or(u32::MAX),
            kept: u32::try_from(total.saturating_sub(removed_index_ids.len())).unwrap_or(u32::MAX),
            removed_index_ids,
        })
    }

    pub async fn ensure_active_profile(&self, config: &mut AppConfig) -> Result<bool> {
        if !config.index_id.is_empty() && self.database.profiles().exists(&config.index_id).await? {
            return Ok(false);
        }

        let profiles = self.database.profiles().list().await?;
        let next_active = profiles
            .iter()
            .find(|profile| profile.port > 0)
            .or_else(|| profiles.first())
            .map(|profile| profile.index_id.clone())
            .unwrap_or_default();
        let changed = config.index_id != next_active;
        config.index_id = next_active;

        Ok(changed)
    }

    async fn server_stats_by_index_id(&self) -> Result<HashMap<String, ServerStatItem>> {
        Ok(self
            .database
            .server_stats()
            .list()
            .await?
            .into_iter()
            .map(|item| (item.index_id.clone(), item))
            .collect())
    }
}

fn normalize_profile(config: &AppConfig, profile: &mut ProfileItem) {
    profile.index_id = profile.index_id.trim().to_string();
    profile.config_version = 4;
    profile.address = profile.address.trim().to_string();
    profile.password = profile.password.trim().to_string();
    profile.username = profile.username.trim().to_string();
    profile.network = profile.network.trim().to_string();
    profile.stream_security = profile.stream_security.trim().to_string();

    if !profile.stream_security.is_empty() {
        if profile.stream_security != STREAM_SECURITY_TLS
            && profile.stream_security != STREAM_SECURITY_REALITY
        {
            profile.stream_security.clear();
        } else {
            if profile.allow_insecure.is_empty() {
                profile.allow_insecure = config.core_basic_item.def_allow_insecure.to_string();
            }
            if profile.fingerprint.is_empty() && profile.stream_security == STREAM_SECURITY_REALITY
            {
                profile
                    .fingerprint
                    .clone_from(&config.core_basic_item.def_fingerprint);
            }
        }
    }

    if !profile.network.is_empty() && !VALID_NETWORKS.contains(&profile.network.as_str()) {
        profile.network = DEFAULT_NETWORK.to_string();
    }
}

fn sort_profile_pairs(
    items: &mut Vec<(ProfileItem, ProfileExItem)>,
    sort_key: ProfileSortKey,
    ascending: bool,
) {
    match sort_key {
        ProfileSortKey::Sort => items.sort_by_key(|(_, profile_ex)| profile_ex.sort),
        ProfileSortKey::ConfigType => {
            items.sort_by_key(|(profile, _)| profile.config_type.as_i32());
        }
        ProfileSortKey::Remarks => {
            items.sort_by(|(left, _), (right, _)| text_cmp(&left.remarks, &right.remarks));
        }
        ProfileSortKey::Address => {
            items.sort_by(|(left, _), (right, _)| text_cmp(&left.address, &right.address));
        }
        ProfileSortKey::Port => items.sort_by_key(|(profile, _)| profile.port),
        ProfileSortKey::Network => {
            items.sort_by(|(left, _), (right, _)| text_cmp(&left.network, &right.network));
        }
        ProfileSortKey::StreamSecurity => {
            items.sort_by(|(left, _), (right, _)| {
                text_cmp(&left.stream_security, &right.stream_security)
            });
        }
        ProfileSortKey::Delay => {
            items.sort_by_key(|(_, profile_ex)| profile_ex.delay);
            move_invalid_delay_to_end(items);
        }
        ProfileSortKey::Speed => {
            items.sort_by(|(_, left), (_, right)| numeric_cmp(left.speed, right.speed));
            move_invalid_speed_to_end(items);
        }
        ProfileSortKey::IpInfo => {
            items.sort_by(|(_, left), (_, right)| {
                text_cmp(
                    left.ip_info.as_deref().unwrap_or(""),
                    right.ip_info.as_deref().unwrap_or(""),
                )
            });
        }
        ProfileSortKey::Subid => {
            items.sort_by(|(left, _), (right, _)| text_cmp(&left.subid, &right.subid));
        }
    }

    if !ascending {
        items.reverse();
        if sort_key == ProfileSortKey::Delay {
            move_invalid_delay_to_end(items);
        }
        if sort_key == ProfileSortKey::Speed {
            move_invalid_speed_to_end(items);
        }
    }
}

fn move_invalid_delay_to_end(items: &mut Vec<(ProfileItem, ProfileExItem)>) {
    let (mut valid, invalid): (Vec<_>, Vec<_>) = items
        .drain(..)
        .partition(|(_, profile_ex)| profile_ex.delay > 0);
    valid.extend(invalid);
    *items = valid;
}

fn move_invalid_speed_to_end(items: &mut Vec<(ProfileItem, ProfileExItem)>) {
    let (mut valid, invalid): (Vec<_>, Vec<_>) = items
        .drain(..)
        .partition(|(_, profile_ex)| profile_ex.speed > 0.0);
    valid.extend(invalid);
    *items = valid;
}

fn numeric_cmp(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

fn text_cmp(left: &str, right: &str) -> Ordering {
    left.to_lowercase().cmp(&right.to_lowercase())
}

fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    value.to_lowercase().contains(&needle.to_lowercase())
}

fn to_list_item(
    profile: ProfileItem,
    profile_ex: ProfileExItem,
    server_stat: ServerStatItem,
    active_index_id: &str,
) -> ProfileListItem {
    ProfileListItem {
        is_active: !active_index_id.is_empty() && profile.index_id == active_index_id,
        profile,
        profile_ex,
        server_stat,
    }
}

fn empty_server_stat(index_id: &str) -> ServerStatItem {
    ServerStatItem {
        index_id: index_id.to_string(),
        ..ServerStatItem::default()
    }
}

fn generate_profile_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = PROFILE_ID_COUNTER.fetch_add(1, AtomicOrdering::Relaxed) as u128;
    let pid = u128::from(std::process::id());

    format!("{:032x}", nanos ^ (counter << 64) ^ pid)
}

#[cfg(test)]
mod tests {
    use voya_core::{ConfigType, CoreType, MoveAction, ProfileSortKey, ProtocolExtraItem};

    use super::*;

    #[tokio::test]
    async fn profile_crud_defaults_active_and_persists_order() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();

        let first = manager
            .save_profile(&mut config, sample_profile("first", "A", 443))
            .await
            .unwrap();
        let second = manager
            .save_profile(&mut config, sample_profile("second", "B", 8443))
            .await
            .unwrap();

        assert_eq!(config.index_id, first.profile.index_id);
        assert_eq!(first.profile.config_version, 4);
        assert_eq!(first.profile.network, DEFAULT_NETWORK);
        assert!(first.profile_ex.sort < second.profile_ex.sort);

        let listed = manager.list_profiles(&config, None, None).await.unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed[0].is_active);
        assert_eq!(listed[1].profile.remarks, "B");
    }

    #[tokio::test]
    async fn profile_active_selection_moves_when_active_profile_is_deleted() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let first = manager
            .save_profile(&mut config, sample_profile("first", "A", 443))
            .await
            .unwrap();
        let second = manager
            .save_profile(&mut config, sample_profile("second", "B", 8443))
            .await
            .unwrap();

        manager
            .set_active_profile(&mut config, &second.profile.index_id)
            .await
            .unwrap();
        manager
            .delete_profiles(&mut config, &[second.profile.index_id])
            .await
            .unwrap();

        assert_eq!(config.index_id, first.profile.index_id);
    }

    #[tokio::test]
    async fn profile_copy_move_group_and_sort_update_profile_ex_state() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let a = manager
            .save_profile(&mut config, sample_profile("a", "A", 1000))
            .await
            .unwrap();
        let b = manager
            .save_profile(&mut config, sample_profile("b", "B", 2000))
            .await
            .unwrap();
        let c = manager
            .save_profile(&mut config, sample_profile("c", "C", 3000))
            .await
            .unwrap();

        manager
            .move_profile(&config, None, &c.profile.index_id, MoveAction::Top, None)
            .await
            .unwrap();
        let moved = manager.list_profiles(&config, None, None).await.unwrap();
        assert_eq!(moved[0].profile.remarks, "C");

        database
            .server_stats()
            .upsert(&ServerStatItem {
                index_id: a.profile.index_id.clone(),
                total_up: 100,
                total_down: 200,
                today_up: 10,
                today_down: 20,
                date_now: 1,
            })
            .await
            .unwrap();
        let copied = manager
            .copy_profiles(&mut config, &[a.profile.index_id.clone()])
            .await
            .unwrap();
        assert_eq!(copied[0].profile.remarks, "A-clone");
        assert_eq!(copied[0].server_stat.total_up, 100);
        assert_eq!(copied[0].server_stat.total_down, 200);

        manager
            .move_profiles_to_group(
                &[
                    b.profile.index_id.clone(),
                    copied[0].profile.index_id.clone(),
                ],
                "group-1",
            )
            .await
            .unwrap();
        let group = manager
            .list_profiles(&config, Some("group-1"), None)
            .await
            .unwrap();
        assert_eq!(group.len(), 2);

        manager
            .sort_profiles(&config, None, ProfileSortKey::Port, false)
            .await
            .unwrap();
        let sorted = manager.list_profiles(&config, None, None).await.unwrap();
        assert_eq!(sorted[0].profile.remarks, "C");
    }

    #[tokio::test]
    async fn profile_dedupe_respects_keep_older_and_ignores_complex_profiles() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let old = manager
            .save_profile(&mut config, sample_profile("old", "Old", 443))
            .await
            .unwrap();
        let mut duplicate = sample_profile("new", "New", 443);
        duplicate.index_id = "new".to_string();
        manager.save_profile(&mut config, duplicate).await.unwrap();
        let mut group = ProfileItem {
            index_id: "group".to_string(),
            config_type: ConfigType::PolicyGroup,
            remarks: "Group".to_string(),
            ..ProfileItem::default()
        };
        group.protocol_extra = ProtocolExtraItem {
            child_items: Some(old.profile.index_id.clone()),
            ..ProtocolExtraItem::default()
        };
        manager.save_profile(&mut config, group).await.unwrap();

        let result = manager
            .dedupe_profiles(&mut config, None, true)
            .await
            .unwrap();
        assert_eq!(result.total, 3);
        assert_eq!(result.kept, 2);
        assert_eq!(result.removed_index_ids, vec!["new".to_string()]);
        assert!(database.profiles().get("group").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn profile_ex_manager_updates_delay_speed_message_and_ip_info() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let profile = manager
            .save_profile(&mut config, sample_profile("profile", "A", 443))
            .await
            .unwrap();

        manager
            .profile_ex()
            .set_test_delay(&profile.profile.index_id, 123)
            .await
            .unwrap();
        manager
            .profile_ex()
            .set_test_speed(&profile.profile.index_id, 45.0)
            .await
            .unwrap();
        manager
            .profile_ex()
            .set_test_message(&profile.profile.index_id, "ok")
            .await
            .unwrap();
        let updated = manager
            .profile_ex()
            .set_test_ip_info(&profile.profile.index_id, "US")
            .await
            .unwrap();

        assert_eq!(updated.delay, 123);
        assert_eq!(updated.speed, 45.0);
        assert_eq!(updated.message.as_deref(), Some("ok"));
        assert_eq!(updated.ip_info.as_deref(), Some("US"));
    }

    fn sample_profile(index_id: &str, remarks: &str, port: i32) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VMess,
            core_type: Some(CoreType::Xray),
            remarks: remarks.to_string(),
            address: " example.com ".to_string(),
            port,
            password: "uuid".to_string(),
            network: "invalid-network".to_string(),
            stream_security: STREAM_SECURITY_TLS.to_string(),
            protocol_extra: ProtocolExtraItem {
                vmess_security: Some("auto".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        }
    }
}
