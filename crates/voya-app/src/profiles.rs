use std::collections::HashMap;

use thiserror::Error;
use voya_core::{
    AppConfig, MoveAction, ProfileExItem, ProfileItem, ProfileListItem, ProfileProtocol,
    ProfileTransport, ServerEndpoint, ServerStatItem,
};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};

const DEFAULT_PROFILE_SORT_STEP: i32 = 10;

/// A profile listing plus the count of stored profiles this build could not
/// read.
///
/// `voya-db` skips a row whose stored payload it cannot decode instead of
/// failing the whole listing, which keeps the rest of the servers usable. The
/// count rides along with the rows so the profiles screen can state it: a list
/// that is quietly short otherwise looks like data loss.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfileListing {
    pub items: Vec<ProfileListItem>,
    pub undecodable_profiles: usize,
}

pub type Result<T> = std::result::Result<T, ProfileManagerError>;

#[derive(Debug, Error)]
pub enum ProfileManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("node {0} was not found")]
    ProfileNotFound(String),
    #[error("node id is required")]
    MissingProfileId,
    #[error("subscription {0} owns this node; update the subscription instead")]
    SubscriptionReadOnly(String),
    #[error("cannot move node {index_id}: {reason}")]
    InvalidMove { index_id: String, reason: String },
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileManager<'db> {
    database: DatabaseSession<'db>,
}

impl<'db> ProfileManager<'db> {
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

    /// The listing behind every profile view, with the undecodable-row count
    /// the storage layer reported.
    ///
    /// The count is deliberately *not* narrowed by `filter`: a row that could
    /// not be decoded has no remarks or address to match a filter against, so
    /// hiding the count while a filter is typed would make the statement blink
    /// out exactly when the list gets shorter.
    pub async fn list_profiles(
        &self,
        config: &AppConfig,
        subscription_id: Option<&str>,
        filter: Option<&str>,
    ) -> Result<ProfileListing> {
        let listing = self
            .database
            .profiles()
            .list_with_profile_ex(subscription_id)
            .await?;
        let stats = self.server_stats_by_index_id().await?;
        let filter = filter.map(str::trim).filter(|value| !value.is_empty());

        Ok(ProfileListing {
            items: listing
                .items
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
                .collect(),
            undecodable_profiles: listing.undecodable_rows,
        })
    }

    pub async fn save_profile(
        &self,
        config: &mut AppConfig,
        profile: ProfileItem,
    ) -> Result<ProfileListItem> {
        self.require_manual(std::slice::from_ref(&profile.index_id))
            .await?;
        if let Some(owner) = &profile.subscription_id {
            return Err(ProfileManagerError::SubscriptionReadOnly(owner.clone()));
        }
        self.save_imported_profile(config, profile).await
    }

    /// Only the subscription importer may write source-owned node parameters.
    pub(crate) async fn save_imported_profile(
        &self,
        config: &mut AppConfig,
        mut profile: ProfileItem,
    ) -> Result<ProfileListItem> {
        let is_new = if profile.index_id.trim().is_empty() {
            profile.index_id = uuid::Uuid::new_v4().simple().to_string();
            true
        } else {
            !self.database.profiles().exists(&profile.index_id).await?
        };

        normalize_profile(&mut profile);

        let profile_ex = if is_new {
            ProfileExItem {
                index_id: profile.index_id.clone(),
                sort: self.database.profile_exs().max_sort().await? + DEFAULT_PROFILE_SORT_STEP,
                ..ProfileExItem::default()
            }
        } else {
            let mut existing = self
                .database
                .profile_exs()
                .ensure(&profile.index_id)
                .await?;
            existing.index_id.clone_from(&profile.index_id);
            if self
                .database
                .profiles()
                .get(&profile.index_id)
                .await?
                .is_some_and(|previous| !voya_core::profile_items_match(&previous, &profile, false))
            {
                existing.country_code = None;
                existing.delay = 0;
                existing.message = None;
                existing.ip_info = None;
            }
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
        self.require_manual(index_ids).await?;
        let deleted = self.database.profiles().delete_many(index_ids).await?;
        self.ensure_active_profile(config).await?;

        Ok(deleted)
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
        let profile_ex = self.database.profile_exs().ensure(index_id).await?;
        let server_stat = self
            .database
            .server_stats()
            .get(index_id)
            .await?
            .unwrap_or_else(|| empty_server_stat(index_id));
        config.set_active_node(index_id);

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
        self.require_manual(&[index_id.to_string()]).await?;
        let items = self
            .database
            .profiles()
            .list_with_profile_ex(None)
            .await?
            .items;
        if !items.iter().any(|(p, _)| p.index_id == index_id) {
            return Err(ProfileManagerError::ProfileNotFound(index_id.to_string()));
        }
        let members = items
            .iter()
            .enumerate()
            .filter(|(_, (p, _))| {
                p.subscription_id.is_none()
                    && subscription_id.is_none_or(|id| p.subscription_id.as_deref() == Some(id))
            })
            .collect::<Vec<_>>();
        let from = members
            .iter()
            .position(|(_, (p, _))| p.index_id == index_id)
            .ok_or_else(|| ProfileManagerError::ProfileNotFound(index_id.to_string()))?;
        let to = match action {
            MoveAction::Top => 0,
            MoveAction::Up => from.saturating_sub(1),
            MoveAction::Down => (from + 1).min(members.len() - 1),
            MoveAction::Bottom => members.len() - 1,
            MoveAction::Position => usize::try_from(position.unwrap_or_default())
                .unwrap_or(0)
                .min(members.len() - 1),
        };
        let mut ordered = members
            .iter()
            .map(|(_, (p, _))| p.index_id.as_str())
            .collect::<Vec<_>>();
        let moved = ordered.remove(from);
        ordered.insert(to, moved);
        self.renumber_sort(&items).await?;
        let updates = members
            .iter()
            .zip(ordered)
            .map(|((slot, _), id)| {
                (
                    id,
                    (i32::try_from(*slot).unwrap_or(i32::MAX / DEFAULT_PROFILE_SORT_STEP - 1) + 1)
                        * DEFAULT_PROFILE_SORT_STEP,
                )
            })
            .collect::<Vec<_>>();
        self.database.profile_exs().set_sort_many(&updates).await?;
        Ok(self
            .list_profiles(config, subscription_id, None)
            .await?
            .items)
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
                (profile.subscription_id.is_none() && profile_ex.sort != sort)
                    .then_some((profile.index_id.as_str(), sort))
            })
            .collect::<Vec<_>>();

        Ok(self
            .database
            .profile_exs()
            .set_sort_many(&reordered)
            .await?)
    }

    /// Validate a complete user mutation before its first write.
    pub(crate) async fn require_manual(&self, ids: &[String]) -> Result<()> {
        for id in ids {
            if let Some(profile) = self.database.profiles().get(id).await? {
                if let Some(owner) = profile.subscription_id {
                    return Err(ProfileManagerError::SubscriptionReadOnly(owner));
                }
            }
        }
        Ok(())
    }

    pub async fn ensure_active_profile(&self, config: &mut AppConfig) -> Result<bool> {
        // An active group stands in for the node; only a group that no longer
        // exists is cleared, and no node is picked in its place.
        if !config.active_group_id.is_empty() {
            if self
                .database
                .policy_groups()
                .exists(&config.active_group_id)
                .await?
            {
                return Ok(false);
            }
            config.active_group_id.clear();
            return Ok(true);
        }
        if !config.index_id.is_empty() && self.database.profiles().exists(&config.index_id).await? {
            return Ok(false);
        }

        let changed = !config.index_id.is_empty();
        config.index_id.clear();
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

pub(crate) fn normalize_profile(profile: &mut ProfileItem) {
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
        trim_option(&mut tls.certificate_pem);
        normalize_values(&mut tls.alpn);
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
    }
}

fn normalize_transport(transport: &mut ProfileTransport) {
    match transport {
        ProfileTransport::Tcp { header, host, path } => {
            trim_option(header);
            trim_option(host);
            trim_option(path);
        }
        ProfileTransport::Websocket { host, path }
        | ProfileTransport::HttpUpgrade { host, path }
        | ProfileTransport::Http2 { host, path }
        | ProfileTransport::Quic { host, path } => {
            trim_option(host);
            trim_option(path);
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

#[cfg(test)]
mod tests {
    use voya_core::{
        MoveAction, ProfileProtocol, ProfileTransport, ServerEndpoint, SubItem, TlsMode,
        TlsSettings,
    };

    use super::*;

    #[tokio::test]
    async fn profile_crud_leaves_selection_empty_and_persists_order() {
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

        assert!(config.index_id.is_empty());
        assert_eq!(first.profile.network(), "raw");
        assert!(first.profile_ex.sort < second.profile_ex.sort);

        let listed = manager
            .list_profiles(&config, None, None)
            .await
            .expect("profile manager test operation should succeed")
            .items;
        assert_eq!(listed.len(), 2);
        assert!(!listed[0].is_active);
        assert_eq!(listed[1].profile.remarks, "B");
    }

    #[tokio::test]
    async fn profile_active_selection_clears_when_active_profile_is_deleted() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let _first = manager
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

        assert!(config.index_id.is_empty());
    }

    #[tokio::test]
    async fn local_sorting_ignores_retired_memberships() {
        let database = Database::connect_in_memory()
            .await
            .expect("profile manager test operation should succeed");
        let manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        manager
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

        config.index_id = "a".to_string();
        manager
            .move_profile(&config, None, &c.profile.index_id, MoveAction::Top, None)
            .await
            .expect("profile manager test operation should succeed");
        let moved = manager
            .list_profiles(&config, None, None)
            .await
            .expect("profile manager test operation should succeed")
            .items;
        assert_eq!(
            moved
                .iter()
                .map(|item| item.profile.index_id.as_str())
                .collect::<Vec<_>>(),
            ["c", "a", "b"]
        );
        assert_eq!(config.index_id, "a");
    }

    /// Persisted ownership is authoritative, including mixed batch requests.
    #[tokio::test]
    async fn subscription_nodes_reject_all_manual_mutations_before_batch_writes() {
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
                .save_imported_profile(&mut config, profile)
                .await
                .expect("profile manager test operation should succeed");
        }

        let mixed = vec!["outsider".to_string(), "s1".to_string()];
        assert!(matches!(
            manager.delete_profiles(&mut config, &mixed).await,
            Err(ProfileManagerError::SubscriptionReadOnly(_))
        ));
        assert!(matches!(
            manager
                .move_profile(
                    &config,
                    Some("sub-scope"),
                    "s3",
                    MoveAction::Position,
                    Some(0)
                )
                .await,
            Err(ProfileManagerError::SubscriptionReadOnly(_))
        ));
        let forged_manual = sample_profile("s1", "Override", 443);
        assert!(matches!(
            manager.save_profile(&mut config, forged_manual).await,
            Err(ProfileManagerError::SubscriptionReadOnly(_))
        ));
        assert_eq!(database.profiles().list().await.expect("profiles").len(), 4);
        let outsider_sort = database
            .profile_exs()
            .ensure(&outsider.profile.index_id)
            .await
            .expect("profile manager test operation should succeed")
            .sort;
        assert_eq!(
            outsider_sort, outsider.profile_ex.sort,
            "profiles outside the scope keep their sort"
        );
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
        let mut measured = database.profile_exs().ensure("r1").await.expect("metrics");
        measured.delay = 123;
        database
            .profile_exs()
            .upsert(&measured)
            .await
            .expect("seed measurement");

        let mut items = database
            .profiles()
            .list_with_profile_ex(None)
            .await
            .expect("profile manager test operation should succeed")
            .items;
        items.reverse();
        manager
            .renumber_sort(&items)
            .await
            .expect("profile manager test operation should succeed");
        let renumbered = manager
            .list_profiles(&config, None, None)
            .await
            .expect("profile manager test operation should succeed")
            .items;

        assert_eq!(
            renumbered
                .iter()
                .map(|item| (item.profile.remarks.as_str(), item.profile_ex.sort))
                .collect::<Vec<_>>(),
            vec![("R4", 10), ("R3", 20), ("R2", 30), ("R1", 40)],
            "a renumber must hand out the gap-based keys in listing order"
        );
        assert_eq!(
            database
                .profile_exs()
                .ensure("r1")
                .await
                .expect("profile manager test operation should succeed")
                .delay,
            123,
            "a batched renumber must not discard speedtest results"
        );

        // Re-numbering the persisted order has nothing left to move and must
        // preserve the existing gap-based keys.
        let items = database
            .profiles()
            .list_with_profile_ex(None)
            .await
            .expect("profile manager test operation should succeed")
            .items;
        manager
            .renumber_sort(&items)
            .await
            .expect("profile manager test operation should succeed");
        let renumbered_again = manager
            .list_profiles(&config, None, None)
            .await
            .expect("profile manager test operation should succeed")
            .items;
        assert_eq!(
            renumbered_again
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
                certificate_pem: None,
                ech_config: Vec::new(),
            }),
            ..ProfileItem::default()
        }
    }
}
