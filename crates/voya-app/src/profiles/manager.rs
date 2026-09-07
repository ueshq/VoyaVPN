use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use voya_core::{
    profile_items_match, AppConfig, MoveAction, ProfileDedupeResult, ProfileExItem, ProfileItem,
    ProfileListItem, ProfileProtocol, ProfileSortKey, ProfileTransport, ServerEndpoint,
    ServerStatItem,
};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};

use super::{ProfileExManager, DEFAULT_PROFILE_SORT_STEP};

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
    database: DatabaseSession<'db>,
}

impl<'db> ProfileManager<'db> {
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

    #[must_use]
    pub fn profile_ex(&self) -> ProfileExManager<'db> {
        ProfileExManager::from_session(self.database)
    }

    pub async fn list_profiles(
        &self,
        config: &AppConfig,
        subscription_id: Option<&str>,
        filter: Option<&str>,
    ) -> Result<Vec<ProfileListItem>> {
        let items = self
            .database
            .profiles()
            .list_with_profile_ex(subscription_id)
            .await?;
        let stats = self.server_stats_by_index_id().await?;
        let filter = filter.map(str::trim).filter(|value| !value.is_empty());

        Ok(items
            .into_iter()
            .filter(|(profile, _)| {
                filter.is_none_or(|filter| {
                    contains_case_insensitive(&profile.remarks, filter)
                        || contains_case_insensitive(profile.address(), filter)
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
        subscription_id: Option<&str>,
        index_id: &str,
        action: MoveAction,
        position: Option<i32>,
    ) -> Result<Vec<ProfileListItem>> {
        let items = self
            .database
            .profiles()
            .list_with_profile_ex(subscription_id)
            .await?;
        let Some(index) = items
            .iter()
            .position(|(profile, _)| profile.index_id == index_id)
        else {
            return Err(ProfileManagerError::ProfileNotFound(index_id.to_string()));
        };

        self.renumber_sort(&items).await?;

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

        self.list_profiles(config, subscription_id, None).await
    }

    pub async fn sort_profiles(
        &self,
        config: &AppConfig,
        subscription_id: Option<&str>,
        sort_key: ProfileSortKey,
        ascending: bool,
    ) -> Result<Vec<ProfileListItem>> {
        let mut items = self
            .database
            .profiles()
            .list_with_profile_ex(subscription_id)
            .await?;
        sort_profile_pairs(&mut items, sort_key, ascending);
        self.renumber_sort(&items).await?;

        self.list_profiles(config, subscription_id, None).await
    }

    /// Rewrites the gap-based sort keys so the list reads `10, 20, 30, …`.
    ///
    /// The gaps are what let `move_profile` place a row between two neighbours
    /// with a single `±1` write. Rows that already carry their target value are
    /// left out of the batch, because after the first renumber a move only
    /// actually shifts the rows between the old and the new position.
    ///
    /// Every remaining row goes out through `set_sort_many`, which applies the
    /// whole ordering in one transaction. Writing them one at a time cost an
    /// autocommit — and its fsync — per profile, so reordering a large
    /// subscription paid hundreds of commits for a single user gesture.
    async fn renumber_sort(&self, items: &[(ProfileItem, ProfileExItem)]) -> Result<()> {
        let reordered = items
            .iter()
            .enumerate()
            .filter_map(|(offset, (profile, profile_ex))| {
                let sort =
                    (i32::try_from(offset).unwrap_or(i32::MAX - 1) + 1) * DEFAULT_PROFILE_SORT_STEP;
                (profile_ex.sort != sort).then_some((profile.index_id.as_str(), sort))
            })
            .collect::<Vec<_>>();

        self.profile_ex().set_sort_many(&reordered).await
    }

    pub async fn dedupe_profiles(
        &self,
        config: &mut AppConfig,
        subscription_id: Option<&str>,
        keep_older: bool,
    ) -> Result<ProfileDedupeResult> {
        let mut profiles = self
            .database
            .profiles()
            .list_by_subscription_id(subscription_id)
            .await?;
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
            .find(|profile| profile.port() > 0)
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

pub(crate) fn normalize_profile(_config: &AppConfig, profile: &mut ProfileItem) {
    profile.index_id = profile.index_id.trim().to_string();
    trim_string(&mut profile.remarks);
    normalize_protocol(&mut profile.protocol);
    if let Some(transport) = &mut profile.transport {
        normalize_transport(transport);
    }
    if let Some(tls) = &mut profile.tls {
        trim_option(&mut tls.server_name);
        trim_option(&mut tls.reality_public_key);
        trim_option(&mut tls.reality_short_id);
        trim_option(&mut tls.reality_spider_x);
        trim_option(&mut tls.mldsa65_verify);
        trim_option(&mut tls.certificate_pem);
        trim_option(&mut tls.final_mask);
        normalize_values(&mut tls.alpn);
        normalize_values(&mut tls.certificate_sha256);
        normalize_values(&mut tls.ech_config);
    }
}

fn normalize_protocol(protocol: &mut ProfileProtocol) {
    match protocol {
        ProfileProtocol::Vmess {
            server,
            uuid,
            cipher,
        } => {
            normalize_server(server);
            trim_string(uuid);
            trim_option(cipher);
        }
        ProfileProtocol::Custom { source, filter } => {
            trim_string(source);
            trim_option(filter);
        }
        ProfileProtocol::Shadowsocks {
            server,
            password,
            method,
            ..
        } => {
            normalize_server(server);
            trim_string(password);
            trim_string(method);
        }
        ProfileProtocol::Socks {
            server,
            username,
            password,
        }
        | ProfileProtocol::Http {
            server,
            username,
            password,
        } => {
            normalize_server(server);
            trim_string(username);
            trim_string(password);
        }
        ProfileProtocol::Vless {
            server,
            uuid,
            flow,
            encryption,
        } => {
            normalize_server(server);
            trim_string(uuid);
            trim_option(flow);
            trim_option(encryption);
        }
        ProfileProtocol::Trojan { server, password }
        | ProfileProtocol::Anytls { server, password } => {
            normalize_server(server);
            trim_string(password);
        }
        ProfileProtocol::Hysteria2 {
            server,
            password,
            port_hops,
            obfuscation_password,
        } => {
            normalize_server(server);
            trim_string(password);
            trim_option(port_hops);
            trim_option(obfuscation_password);
        }
        ProfileProtocol::Tuic {
            server,
            uuid,
            password,
            congestion_control,
        } => {
            normalize_server(server);
            trim_string(uuid);
            trim_string(password);
            trim_option(congestion_control);
        }
        ProfileProtocol::WireGuard {
            server,
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            ..
        } => {
            normalize_server(server);
            trim_string(private_key);
            trim_option(peer_public_key);
            trim_option(preshared_key);
            trim_option(interface_address);
            trim_option(allowed_ips);
            trim_option(reserved);
        }
        ProfileProtocol::Naive {
            server,
            username,
            password,
            congestion_control,
            ..
        } => {
            normalize_server(server);
            trim_string(username);
            trim_string(password);
            trim_option(congestion_control);
        }
        ProfileProtocol::PolicyGroup {
            child_profile_ids,
            source_subscription_id,
            filter,
            ..
        } => {
            normalize_values(child_profile_ids);
            trim_option(source_subscription_id);
            trim_option(filter);
        }
        ProfileProtocol::ProxyChain { child_profile_ids } => {
            normalize_values(child_profile_ids);
        }
    }
}

fn normalize_transport(transport: &mut ProfileTransport) {
    match transport {
        ProfileTransport::Tcp { header, host, path } => {
            trim_option(header);
            trim_option(host);
            trim_option(path);
        }
        ProfileTransport::Kcp { header, seed, .. } => {
            trim_option(header);
            trim_option(seed);
        }
        ProfileTransport::Websocket { host, path }
        | ProfileTransport::HttpUpgrade { host, path }
        | ProfileTransport::Http2 { host, path }
        | ProfileTransport::Quic { host, path } => {
            trim_option(host);
            trim_option(path);
        }
        ProfileTransport::Xhttp {
            host,
            path,
            mode,
            extra,
        } => {
            trim_option(host);
            trim_option(path);
            trim_option(mode);
            trim_option(extra);
        }
        ProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        } => {
            trim_option(authority);
            trim_option(service_name);
            trim_option(mode);
        }
    }
}

fn normalize_server(server: &mut ServerEndpoint) {
    trim_string(&mut server.address);
}

fn normalize_values(values: &mut Vec<String>) {
    for value in values.iter_mut() {
        trim_string(value);
    }
    values.retain(|value| !value.is_empty());
}

fn trim_option(value: &mut Option<String>) {
    if let Some(value) = value {
        trim_string(value);
    }
    if value.as_ref().is_some_and(String::is_empty) {
        *value = None;
    }
}

fn trim_string(value: &mut String) {
    *value = value.trim().to_string();
}

fn sort_profile_pairs(
    items: &mut Vec<(ProfileItem, ProfileExItem)>,
    sort_key: ProfileSortKey,
    ascending: bool,
) {
    match sort_key {
        ProfileSortKey::Sort => items.sort_by_key(|(_, profile_ex)| profile_ex.sort),
        ProfileSortKey::ConfigType => {
            items.sort_by_key(|(profile, _)| profile.config_type().sort_rank());
        }
        ProfileSortKey::Remarks => {
            items.sort_by(|(left, _), (right, _)| text_cmp(&left.remarks, &right.remarks));
        }
        ProfileSortKey::Address => {
            items.sort_by(|(left, _), (right, _)| text_cmp(left.address(), right.address()));
        }
        ProfileSortKey::Port => items.sort_by_key(|(profile, _)| profile.port()),
        ProfileSortKey::Network => {
            items.sort_by(|(left, _), (right, _)| text_cmp(left.network(), right.network()));
        }
        ProfileSortKey::StreamSecurity => {
            items.sort_by(|(left, _), (right, _)| {
                text_cmp(left.stream_security(), right.stream_security())
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
        ProfileSortKey::SubscriptionId => {
            items.sort_by(|(left, _), (right, _)| {
                text_cmp(
                    left.subscription_id.as_deref().unwrap_or(""),
                    right.subscription_id.as_deref().unwrap_or(""),
                )
            });
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
    use voya_core::{
        MoveAction, MultipleLoad, ProfileProtocol, ProfileSortKey, ProfileTransport,
        ServerEndpoint, SubItem, TlsMode, TlsSettings,
    };

    use super::*;

    #[tokio::test]
    async fn profile_crud_defaults_active_and_persists_order() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();

        let first = manager
            .save_profile(&mut config, sample_profile("first", "A", 443))
            .await
            .expect("profile manager test operation should succeed");
        let second = manager
            .save_profile(&mut config, sample_profile("second", "B", 8443))
            .await
            .expect("profile manager test operation should succeed");

        assert_eq!(config.index_id, first.profile.index_id);
        assert_eq!(first.profile.network(), "raw");
        assert!(first.profile_ex.sort < second.profile_ex.sort);

        let listed = manager
            .list_profiles(&config, None, None)
            .await
            .expect("profile manager test operation should succeed");
        assert_eq!(listed.len(), 2);
        assert!(listed[0].is_active);
        assert_eq!(listed[1].profile.remarks, "B");
    }

    #[tokio::test]
    async fn profile_active_selection_moves_when_active_profile_is_deleted() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let first = manager
            .save_profile(&mut config, sample_profile("first", "A", 443))
            .await
            .expect("profile manager test operation should succeed");
        let second = manager
            .save_profile(&mut config, sample_profile("second", "B", 8443))
            .await
            .expect("profile manager test operation should succeed");

        manager
            .set_active_profile(&mut config, &second.profile.index_id)
            .await
            .expect("profile manager test operation should succeed");
        manager
            .delete_profiles(&mut config, &[second.profile.index_id])
            .await
            .expect("profile manager test operation should succeed");

        assert_eq!(config.index_id, first.profile.index_id);
    }

    #[tokio::test]
    async fn profile_copy_move_and_sort_update_profile_ex_state() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let a = manager
            .save_profile(&mut config, sample_profile("a", "A", 1000))
            .await
            .expect("profile manager test operation should succeed");
        manager
            .save_profile(&mut config, sample_profile("b", "B", 2000))
            .await
            .expect("profile manager test operation should succeed");
        let c = manager
            .save_profile(&mut config, sample_profile("c", "C", 3000))
            .await
            .expect("profile manager test operation should succeed");

        manager
            .move_profile(&config, None, &c.profile.index_id, MoveAction::Top, None)
            .await
            .expect("profile manager test operation should succeed");
        let moved = manager
            .list_profiles(&config, None, None)
            .await
            .expect("profile manager test operation should succeed");
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
            .expect("profile manager test operation should succeed");
        let copied = manager
            .copy_profiles(&mut config, std::slice::from_ref(&a.profile.index_id))
            .await
            .expect("profile manager test operation should succeed");
        assert_eq!(copied[0].profile.remarks, "A-clone");
        assert_eq!(copied[0].server_stat.total_up, 100);
        assert_eq!(copied[0].server_stat.total_down, 200);

        manager
            .sort_profiles(&config, None, ProfileSortKey::Port, false)
            .await
            .expect("profile manager test operation should succeed");
        let sorted = manager
            .list_profiles(&config, None, None)
            .await
            .expect("profile manager test operation should succeed");
        assert_eq!(sorted[0].profile.remarks, "C");
    }

    /// Reordering inside a subscription must renumber only that subscription's
    /// rows, and `MoveAction::Position` must land the row at the requested
    /// index rather than anywhere the gap-based numbering happens to allow.
    #[tokio::test]
    async fn scoped_move_and_sort_reorder_only_the_selected_subscription() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        database
            .subscriptions()
            .upsert(&SubItem {
                id: "sub-scope".to_string(),
                remarks: "Scoped".to_string(),
                url: "https://example.test/scope".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let outsider = manager
            .save_profile(&mut config, sample_profile("outsider", "Outsider", 500))
            .await
            .expect("profile manager test operation should succeed");
        for (index_id, remarks, port) in
            [("s1", "S1", 1000), ("s2", "S2", 2000), ("s3", "S3", 3000)]
        {
            let mut profile = sample_profile(index_id, remarks, port);
            profile.subscription_id = Some("sub-scope".to_string());
            manager
                .save_profile(&mut config, profile)
                .await
                .expect("profile manager test operation should succeed");
        }

        let moved = manager
            .move_profile(
                &config,
                Some("sub-scope"),
                "s3",
                MoveAction::Position,
                Some(0),
            )
            .await
            .expect("profile manager test operation should succeed");
        assert_eq!(
            moved
                .iter()
                .map(|item| item.profile.remarks.as_str())
                .collect::<Vec<_>>(),
            vec!["S3", "S1", "S2"],
            "a scoped move must list only the subscription's own profiles"
        );

        let sorted = manager
            .sort_profiles(&config, Some("sub-scope"), ProfileSortKey::Port, false)
            .await
            .expect("profile manager test operation should succeed");
        assert_eq!(
            sorted
                .iter()
                .map(|item| item.profile.remarks.as_str())
                .collect::<Vec<_>>(),
            vec!["S3", "S2", "S1"]
        );

        let outsider_sort = manager
            .profile_ex()
            .ensure(&outsider.profile.index_id)
            .await
            .expect("profile manager test operation should succeed")
            .sort;
        assert_eq!(
            outsider_sort, outsider.profile_ex.sort,
            "profiles outside the scope keep their sort"
        );
    }

    #[tokio::test]
    async fn profile_dedupe_respects_keep_older_and_ignores_complex_profiles() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let old = manager
            .save_profile(&mut config, sample_profile("old", "Old", 443))
            .await
            .expect("profile manager test operation should succeed");
        let mut duplicate = sample_profile("new", "New", 443);
        duplicate.index_id = "new".to_string();
        manager
            .save_profile(&mut config, duplicate)
            .await
            .expect("profile manager test operation should succeed");
        let group = ProfileItem {
            index_id: "group".to_string(),
            remarks: "Group".to_string(),
            protocol: ProfileProtocol::PolicyGroup {
                child_profile_ids: vec![old.profile.index_id.clone()],
                source_subscription_id: None,
                filter: None,
                strategy: MultipleLoad::LeastPing,
            },
            ..ProfileItem::default()
        };
        manager
            .save_profile(&mut config, group)
            .await
            .expect("profile manager test operation should succeed");

        let result = manager
            .dedupe_profiles(&mut config, None, true)
            .await
            .expect("profile manager test operation should succeed");
        assert_eq!(result.total, 3);
        assert_eq!(result.kept, 2);
        assert_eq!(result.removed_index_ids, vec!["new".to_string()]);
        assert!(database
            .profiles()
            .get("group")
            .await
            .expect("profile manager test operation should succeed")
            .is_some());
    }

    #[tokio::test]
    async fn profile_ex_manager_updates_delay_speed_message_and_ip_info() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let profile = manager
            .save_profile(&mut config, sample_profile("profile", "A", 443))
            .await
            .expect("profile manager test operation should succeed");

        manager
            .profile_ex()
            .set_test_delay(&profile.profile.index_id, 123)
            .await
            .expect("profile manager test operation should succeed");
        manager
            .profile_ex()
            .set_test_speed(&profile.profile.index_id, 45.0)
            .await
            .expect("profile manager test operation should succeed");
        manager
            .profile_ex()
            .set_test_message(&profile.profile.index_id, "ok")
            .await
            .expect("profile manager test operation should succeed");
        let updated = manager
            .profile_ex()
            .set_test_ip_info(&profile.profile.index_id, "US")
            .await
            .expect("profile manager test operation should succeed");

        assert_eq!(updated.delay, 123);
        assert_eq!(updated.speed, 45.0);
        assert_eq!(updated.message.as_deref(), Some("ok"));
        assert_eq!(updated.ip_info.as_deref(), Some("US"));
    }

    /// `renumber_sort` pushes the whole ordering through one batched write, so
    /// the gap-based keys it hands out and the speedtest results it must leave
    /// alone are pinned here.
    #[tokio::test]
    async fn renumber_sort_rewrites_gap_based_keys_without_touching_measurements() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        for (index_id, remarks, port) in [
            ("r1", "R1", 4000),
            ("r2", "R2", 3000),
            ("r3", "R3", 2000),
            ("r4", "R4", 1000),
        ] {
            manager
                .save_profile(&mut config, sample_profile(index_id, remarks, port))
                .await
                .expect("profile manager test operation should succeed");
        }
        manager
            .profile_ex()
            .set_test_delay("r1", 123)
            .await
            .expect("profile manager test operation should succeed");

        let sorted = manager
            .sort_profiles(&config, None, ProfileSortKey::Port, true)
            .await
            .expect("profile manager test operation should succeed");

        assert_eq!(
            sorted
                .iter()
                .map(|item| (item.profile.remarks.as_str(), item.profile_ex.sort))
                .collect::<Vec<_>>(),
            vec![("R4", 10), ("R3", 20), ("R2", 30), ("R1", 40)],
            "a renumber must hand out the gap-based keys in listing order"
        );
        assert_eq!(
            manager
                .profile_ex()
                .ensure("r1")
                .await
                .expect("profile manager test operation should succeed")
                .delay,
            123,
            "a batched renumber must not discard speedtest results"
        );

        // Re-running the same sort has nothing left to move, and must still
        // report the same order rather than shifting rows by a step.
        let resorted = manager
            .sort_profiles(&config, None, ProfileSortKey::Port, true)
            .await
            .expect("profile manager test operation should succeed");
        assert_eq!(
            resorted
                .iter()
                .map(|item| (item.profile.remarks.as_str(), item.profile_ex.sort))
                .collect::<Vec<_>>(),
            vec![("R4", 10), ("R3", 20), ("R2", 30), ("R1", 40)]
        );
    }

    fn sample_profile(index_id: &str, remarks: &str, port: i32) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            remarks: remarks.to_string(),
            protocol: ProfileProtocol::Vmess {
                server: ServerEndpoint {
                    address: " example.com ".to_string(),
                    port,
                },
                uuid: "uuid".to_string(),
                cipher: Some("auto".to_string()),
            },
            transport: Some(ProfileTransport::Tcp {
                header: None,
                host: None,
                path: None,
            }),
            tls: Some(TlsSettings {
                mode: TlsMode::Tls,
                server_name: None,
                alpn: Vec::new(),
                reality_public_key: None,
                reality_short_id: None,
                reality_spider_x: None,
                mldsa65_verify: None,
                certificate_pem: None,
                certificate_sha256: Vec::new(),
                ech_config: Vec::new(),
                final_mask: None,
            }),
            ..ProfileItem::default()
        }
    }
}
