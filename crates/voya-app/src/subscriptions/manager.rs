use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use voya_core::{AppConfig, SubItem, SubMetadataItem};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};
use voya_net::DownloadError;

use crate::profiles::{ProfileManager, ProfileManagerError};

mod import;
mod parse;
mod update;

use import::compile_filter;

static SUBSCRIPTION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub type Result<T> = std::result::Result<T, SubscriptionManagerError>;

#[derive(Debug, Error)]
pub enum SubscriptionManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Profile(#[from] ProfileManagerError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("subscription {0} was not found")]
    SubscriptionNotFound(String),
    #[error("subscription remarks are required")]
    MissingRemarks,
    #[error("subscription URL is required")]
    MissingUrl,
    #[error("subscription URL must start with http:// or https://")]
    InvalidSubscriptionUrl,
    #[error("subscription filter is invalid: {0}")]
    InvalidFilter(String),
    #[error("no importable nodes were found")]
    NoImportableProfiles,
}

#[derive(Debug, Clone, Copy)]
pub struct SubscriptionManager<'db> {
    database: DatabaseSession<'db>,
}

impl<'db> SubscriptionManager<'db> {
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

    pub async fn list_subscription_metadata(&self) -> Result<Vec<SubMetadataItem>> {
        Ok(self.database.subscription_metadata().list().await?)
    }

    pub async fn list_subscriptions(&self) -> Result<Vec<SubItem>> {
        Ok(self.database.subscriptions().list().await?)
    }

    /// Subscription selection is not part of the persisted application
    /// configuration, so this mutation touches the database only.
    pub async fn save_subscription(&self, mut item: SubItem) -> Result<SubItem> {
        normalize_subscription(&mut item);
        if item.remarks.is_empty() {
            return Err(SubscriptionManagerError::MissingRemarks);
        }
        if !item.url.is_empty() && !is_http_url(&item.url) {
            return Err(SubscriptionManagerError::InvalidSubscriptionUrl);
        }
        // Compile the filter now so a bad regex is reported by the editor
        // instead of failing every later update of this subscription.
        compile_filter(item.filter.as_deref())?;

        if item.id.is_empty() {
            item.id = generate_subscription_id();
        }
        if item.sort <= 0 {
            item.sort = self.database.subscriptions().max_sort().await? + 1;
        }

        self.database.subscriptions().upsert(&item).await?;

        Ok(item)
    }

    pub async fn add_subscription_from_url(&self, url: &str) -> Result<SubItem> {
        let url = url.trim();
        if url.is_empty() {
            return Err(SubscriptionManagerError::MissingUrl);
        }
        if !is_http_url(url) {
            return Err(SubscriptionManagerError::InvalidSubscriptionUrl);
        }
        if let Some(existing) = self.database.subscriptions().get_by_url(url).await? {
            return Ok(existing);
        }

        self.save_subscription(SubItem {
            remarks: extract_remarks_from_url(url).unwrap_or_else(|| "import_sub".to_string()),
            url: url.to_string(),
            ..SubItem::default()
        })
        .await
    }

    pub async fn delete_subscriptions(
        &self,
        config: &mut AppConfig,
        ids: &[String],
    ) -> Result<u32> {
        // Before the subscriptions go: their foreign key would only null the
        // binding and leave an import-created group with nothing in it.
        crate::policy_groups::PolicyGroupManager::from_session(self.database)
            .delete_auto_groups_for_subscriptions(config, ids)
            .await?;
        let mut deleted = 0_u32;
        for id in ids {
            if self.database.subscriptions().delete(id).await? {
                deleted = deleted.saturating_add(1);
                self.database
                    .profiles()
                    .delete_by_subscription_id(id)
                    .await?;
            }
        }

        ProfileManager::from_session(self.database)
            .ensure_active_profile(config)
            .await?;

        Ok(deleted)
    }
}

fn normalize_subscription(item: &mut SubItem) {
    item.id = item.id.trim().to_string();
    item.remarks = item.remarks.trim().to_string();
    item.url = item.url.trim().to_string();
    item.more_url = item.more_url.trim().to_string();
    item.user_agent = item.user_agent.trim().to_string();
    item.filter = trimmed_option(item.filter.take());
    item.convert_target = trimmed_option(item.convert_target.take());
}

fn trimmed_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn is_http_url(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("https://") || value.starts_with("http://")
}

fn extract_remarks_from_url(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key.eq_ignore_ascii_case("remarks") && !value.is_empty()).then(|| value.to_string())
    })
}

fn generate_subscription_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = SUBSCRIPTION_ID_COUNTER.fetch_add(1, Ordering::Relaxed) as u128;
    let pid = u128::from(std::process::id());

    format!("sub-{:032x}", nanos ^ (counter << 64) ^ pid)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::{collections::HashMap, sync::Arc};
    use voya_core::ProfileItem;

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };
    use voya_core::{ProfileProtocol, ProfileTransport, ServerEndpoint};

    use super::*;

    #[tokio::test]
    async fn subscription_import_filters_dedupes_persists_without_selecting() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let sub = manager
            .save_subscription(SubItem {
                id: "sub-us".to_string(),
                remarks: "US sub".to_string(),
                url: "https://example.test/sub".to_string(),
                filter: Some("US".to_string()),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");
        let old = ProfileManager::new(&database)
            .save_profile(&mut config, sample_profile("old", "US old"))
            .await
            .expect("subscription manager test operation should succeed");
        let mut old_profile = old.profile.clone();
        old_profile.subscription_id = Some(sub.id.clone());
        database
            .profiles()
            .upsert(&old_profile)
            .await
            .expect("subscription manager test operation should succeed");
        config.index_id = old_profile.index_id.clone();

        let text = [
            "vless://uuid@example.test:443?security=tls#US%20node",
            "vless://uuid@example.test:443?security=tls#US%20duplicate",
            "trojan://secret@example.test:443#JP%20node",
        ]
        .join("\n");
        let result = manager
            .import_subscription_content(&mut config, &text, Some(&sub.id))
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 2);
        assert_eq!(result.removed_existing, 1);
        let profiles = database
            .profiles()
            .list_by_subscription_id(Some(&sub.id))
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].remarks, "US node");
        assert_eq!(
            profiles[0].subscription_id.as_deref(),
            Some(sub.id.as_str())
        );
        assert!(config.index_id.is_empty());
    }

    #[tokio::test]
    async fn subscription_update_downloads_base64_and_more_url() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let main = STANDARD.encode("vless://uuid-a@example.test:443#US%20A");
        let extra = "trojan://secret@example.test:443#US%20B".to_string();
        let base = spawn_http_fixture(
            HashMap::from([("/main".to_string(), main), ("/extra".to_string(), extra)]),
            2,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        manager
            .save_subscription(SubItem {
                id: "sub-plain".to_string(),
                remarks: "Plain".to_string(),
                url: format!("{base}/main"),
                more_url: format!("{base}/extra"),
                user_agent: "SubUA/3".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");
        let result = manager
            .update_subscriptions(&mut config, None, false, None)
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(result.updated, 1);
        assert_eq!(result.imported, 2);

        let profiles = database
            .profiles()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 2);
        assert!(profiles.iter().any(|profile| profile.remarks == "US A"));
        assert!(profiles.iter().any(|profile| profile.remarks == "US B"));
        assert_eq!(
            seen_user_agents.lock().await.as_slice(),
            ["SubUA/3", "SubUA/3"]
        );
    }

    /// A dead `more_url` mirror is invisible to the user unless the update
    /// reports it: the primary list still imports, so nothing else in the result
    /// would ever mention the missing source.
    #[tokio::test]
    async fn subscription_update_warns_about_a_failed_more_url_without_failing_the_update() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let main = STANDARD.encode("vless://uuid-a@example.test:443#US%20A");
        let base = spawn_http_fixture(
            HashMap::from([("/main".to_string(), main)]),
            2,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        manager
            .save_subscription(SubItem {
                id: "sub-mirror".to_string(),
                remarks: "Mirrored".to_string(),
                url: format!("{base}/main"),
                more_url: format!("{base}/missing"),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");

        let result = manager
            .update_subscriptions(&mut config, None, false, None)
            .await
            .expect("a failing mirror must not fail the subscription update");

        assert_eq!(result.updated, 1);
        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 0);
        assert!(
            result.messages.iter().any(|message| {
                message.starts_with("Mirrored->additional subscription URL failed:")
            }),
            "{:?}",
            result.messages
        );
        assert!(
            result
                .messages
                .iter()
                .any(|message| message == "Mirrored->imported 1 nodes"),
            "{:?}",
            result.messages
        );
    }

    #[tokio::test]
    async fn subscription_update_persists_userinfo_metadata_and_adopts_interval() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let node = "vless://uuid-a@example.test:443#US%20A".to_string();
        let base = spawn_http_fixture_with_headers(
            HashMap::from([(
                "/meta".to_string(),
                (
                    node.clone(),
                    vec![
                        (
                            "Subscription-Userinfo".to_string(),
                            "upload=100; download=200; total=1000; expire=1924992000".to_string(),
                        ),
                        ("Profile-Update-Interval".to_string(), "12".to_string()),
                        ("Profile-Title".to_string(), "Demo Plan".to_string()),
                    ],
                ),
            )]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let sub = manager
            .save_subscription(SubItem {
                id: "sub-meta".to_string(),
                remarks: "Meta".to_string(),
                url: format!("{base}/meta"),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");

        manager
            .update_subscriptions(&mut config, Some(&sub.id), false, None)
            .await
            .expect("subscription manager test operation should succeed");

        let metadata = database
            .subscription_metadata()
            .get(&sub.id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.upload_bytes, Some(100));
        assert_eq!(metadata.download_bytes, Some(200));
        assert_eq!(metadata.total_bytes, Some(1000));
        assert_eq!(metadata.expire_at, Some(1_924_992_000));
        assert_eq!(metadata.profile_title.as_deref(), Some("Demo Plan"));
        assert!(metadata.last_update_at.is_some_and(|at| at > 0));
        assert_eq!(
            database
                .subscriptions()
                .get(&sub.id)
                .await
                .expect("subscription should load")
                .expect("subscription should exist")
                .auto_update_interval_minutes,
            Some(720),
            "server-suggested interval should be adopted when the user has not set one"
        );

        // A later fetch without usage headers keeps the stored figures but
        // refreshes last_update_at, and never overrides a user-set interval.
        let plain_base = spawn_http_fixture(
            HashMap::from([("/meta".to_string(), node)]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let mut updated_sub = database
            .subscriptions()
            .get(&sub.id)
            .await
            .expect("subscription should load")
            .expect("subscription should exist");
        updated_sub.url = format!("{plain_base}/meta");
        updated_sub.auto_update_interval_minutes = Some(30);
        manager
            .save_subscription(updated_sub)
            .await
            .expect("subscription manager test operation should succeed");
        manager
            .update_subscriptions(&mut config, Some(&sub.id), false, None)
            .await
            .expect("subscription manager test operation should succeed");

        let metadata = database
            .subscription_metadata()
            .get(&sub.id)
            .await
            .expect("metadata should load")
            .expect("metadata should exist");
        assert_eq!(metadata.total_bytes, Some(1000));
        assert_eq!(metadata.profile_title.as_deref(), Some("Demo Plan"));
        assert_eq!(
            database
                .subscriptions()
                .get(&sub.id)
                .await
                .expect("subscription should load")
                .expect("subscription should exist")
                .auto_update_interval_minutes,
            Some(30)
        );
    }

    #[tokio::test]
    async fn subscription_update_skips_non_importable_fetches_without_touching_state() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([
                ("/empty".to_string(), String::new()),
                ("/junk".to_string(), "not a profile".to_string()),
                (
                    "/filtered".to_string(),
                    "vless://uuid@example.test:443#US%20Filtered".to_string(),
                ),
                (
                    "/url-only".to_string(),
                    "https://example.test/new?remarks=Injected".to_string(),
                ),
            ]),
            4,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        for (id, remarks, path, filter) in [
            ("empty", "Empty", "/empty", None),
            ("junk", "Junk", "/junk", None),
            ("filtered", "Filtered", "/filtered", Some("JP")),
            ("url-only", "URL Only", "/url-only", None),
        ] {
            manager
                .save_subscription(SubItem {
                    id: id.to_string(),
                    remarks: remarks.to_string(),
                    url: format!("{base}{path}"),
                    filter: filter.map(str::to_string),
                    ..SubItem::default()
                })
                .await
                .expect("subscription manager test operation should succeed");
        }

        let mut old_profile = sample_profile("filtered-old", "JP old");
        old_profile.subscription_id = Some("filtered".to_string());
        database
            .profiles()
            .upsert(&old_profile)
            .await
            .expect("subscription manager test operation should succeed");

        let result = manager
            .update_subscriptions(&mut config, None, false, None)
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(result.updated, 0);
        assert_eq!(result.skipped, 4);
        assert_eq!(result.imported, 0);
        assert_eq!(result.removed_existing, 0);
        assert!(
            result
                .messages
                .iter()
                .any(|message| message.contains("Empty->fetched empty subscription content")),
            "{:?}",
            result.messages
        );
        assert!(
            result
                .messages
                .iter()
                .any(|message| message.contains("Junk->no importable nodes were found")),
            "{:?}",
            result.messages
        );
        assert!(
            result
                .messages
                .iter()
                .any(|message| message.contains("Filtered->no nodes were imported")),
            "{:?}",
            result.messages
        );
        assert!(
            result
                .messages
                .iter()
                .any(|message| message.contains("URL Only->no importable nodes were found")),
            "{:?}",
            result.messages
        );

        let subscriptions = database
            .subscriptions()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(subscriptions.len(), 4);
        assert!(subscriptions
            .iter()
            .all(|subscription| subscription.url != "https://example.test/new?remarks=Injected"));

        let filtered_profiles = database
            .profiles()
            .list_by_subscription_id(Some("filtered"))
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(filtered_profiles.len(), 1);
        assert_eq!(filtered_profiles[0].remarks, "JP old");

        for id in ["empty", "junk", "filtered", "url-only"] {
            assert!(
                database
                    .subscription_metadata()
                    .get(id)
                    .await
                    .expect("metadata should load")
                    .and_then(|metadata| metadata.last_update_at)
                    .is_none(),
                "a fetch that imported nothing must stay due for the scheduler: {id}"
            );
        }
    }

    #[tokio::test]
    async fn saving_a_subscription_rejects_an_invalid_filter() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);

        let error = manager
            .save_subscription(SubItem {
                id: "bad-filter".to_string(),
                remarks: "Bad filter".to_string(),
                url: "https://example.test/sub".to_string(),
                filter: Some("US(".to_string()),
                ..SubItem::default()
            })
            .await
            .expect_err("an uncompilable filter should be rejected by the editor");

        assert!(matches!(error, SubscriptionManagerError::InvalidFilter(_)));
        assert!(database
            .subscriptions()
            .list()
            .await
            .expect("subscription manager test operation should succeed")
            .is_empty());
    }

    #[tokio::test]
    async fn an_invalid_filter_skips_only_its_own_subscription() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([
                (
                    "/good".to_string(),
                    "vless://uuid@example.test:443#Good".to_string(),
                ),
                (
                    "/bad".to_string(),
                    "vless://uuid@example.test:443#Bad".to_string(),
                ),
            ]),
            2,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        manager
            .save_subscription(SubItem {
                id: "good".to_string(),
                remarks: "Good".to_string(),
                url: format!("{base}/good"),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");
        // A row stored before filters were validated at save time.
        database
            .subscriptions()
            .upsert(&SubItem {
                id: "bad".to_string(),
                remarks: "Bad".to_string(),
                url: format!("{base}/bad"),
                filter: Some("US(".to_string()),
                sort: 2,
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");

        let result = manager
            .update_subscriptions(&mut config, None, false, None)
            .await
            .expect("one broken filter must not abort the whole batch");

        assert_eq!(result.updated, 1);
        assert_eq!(result.skipped, 1);
        assert!(
            result
                .messages
                .iter()
                .any(|message| message.contains("Bad->subscription filter is invalid")),
            "{:?}",
            result.messages
        );
        assert_eq!(
            database
                .profiles()
                .list_by_subscription_id(Some("good"))
                .await
                .expect("subscription manager test operation should succeed")
                .len(),
            1,
            "the healthy subscription keeps its imported profiles"
        );
        assert!(database
            .subscription_metadata()
            .get("good")
            .await
            .expect("metadata should load")
            .and_then(|metadata| metadata.last_update_at)
            .is_some());
        assert!(database
            .subscription_metadata()
            .get("bad")
            .await
            .expect("metadata should load")
            .and_then(|metadata| metadata.last_update_at)
            .is_none());
    }

    #[tokio::test]
    async fn failed_network_preparation_does_not_modify_profiles_or_config() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([("/empty".to_string(), String::new())]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("subscription database should connect");
        let manager = SubscriptionManager::new(&database);
        let config = AppConfig::default();
        manager
            .save_subscription(SubItem {
                id: "empty-network".to_string(),
                remarks: "Empty network".to_string(),
                url: format!("{base}/empty"),
                ..SubItem::default()
            })
            .await
            .expect("subscription should be saved");
        let original = config.clone();

        let prepared = manager
            .prepare_subscription_update(Some("empty-network"), false, None)
            .await
            .expect("empty response should produce a skipped result");

        assert!(!prepared.has_imports());
        assert_eq!(prepared.into_result().skipped, 1);
        assert_eq!(config, original);
        assert!(database
            .profiles()
            .list()
            .await
            .expect("profiles should load")
            .is_empty());
    }

    #[tokio::test]
    async fn prepared_subscription_content_is_discarded_when_source_changes() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([(
                "/profile".to_string(),
                "vless://uuid@example.test:443#Prepared".to_string(),
            )]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("subscription database should connect");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        manager
            .save_subscription(SubItem {
                id: "changing-source".to_string(),
                remarks: "Original".to_string(),
                url: format!("{base}/profile"),
                ..SubItem::default()
            })
            .await
            .expect("subscription should be saved");
        let prepared = manager
            .prepare_subscription_update(Some("changing-source"), false, None)
            .await
            .expect("subscription should be prepared");
        assert!(prepared.has_imports());
        manager
            .save_subscription(SubItem {
                id: "changing-source".to_string(),
                remarks: "Changed".to_string(),
                url: "https://changed.example.test/sub".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("subscription should change");

        let result = manager
            .apply_prepared_subscription_update(&mut config, prepared)
            .await
            .expect("stale preparation should be skipped");

        assert_eq!(result.imported, 0);
        assert_eq!(result.skipped, 1);
        assert!(result
            .messages
            .iter()
            .any(|message| message.contains("prepared content was discarded")));
        assert!(database
            .profiles()
            .list()
            .await
            .expect("profiles should load")
            .is_empty());
    }

    #[tokio::test]
    async fn manual_import_rejects_full_json_config() {
        let database = Database::connect_in_memory().await.expect("database");
        let mut config = AppConfig::default();
        let result = SubscriptionManager::new(&database)
            .import_subscription_content(
                &mut config,
                r#"{"remarks":"full-json","inbounds":[],"outbounds":[],"route":{},"dns":{}}"#,
                None,
            )
            .await;
        assert!(matches!(
            result,
            Err(SubscriptionManagerError::NoImportableProfiles)
        ));
        assert!(database
            .profiles()
            .list()
            .await
            .expect("profiles")
            .is_empty());
        assert!(config.index_id.is_empty());
    }

    #[tokio::test]
    async fn only_the_first_subscription_import_creates_its_auto_group() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        database
            .subscriptions()
            .upsert(&SubItem {
                id: "work".to_string(),
                remarks: "Work".to_string(),
                url: "https://work.example/sub".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("subscription");
        let manager = SubscriptionManager::new(&database);
        let groups = crate::policy_groups::PolicyGroupManager::new(&database);
        let mut config = AppConfig::default();
        let text = "trojan://secret@example.test:443#JP%20node";

        manager
            .import_subscription_content(&mut config, text, Some("work"))
            .await
            .expect("first import");
        let entries = groups.list(&config).await.expect("groups");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].group.name, "Work · Auto");
        assert!(config.active_group_id.is_empty());

        groups
            .delete(&mut config, &[entries[0].group.id.clone()])
            .await
            .expect("the user deletes it");
        manager
            .import_subscription_content(&mut config, text, Some("work"))
            .await
            .expect("update");
        assert!(groups.list(&config).await.expect("groups").is_empty());
    }

    #[tokio::test]
    async fn manual_import_reports_bad_share_lines_without_exposing_payloads() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();

        let result = manager
            .import_subscription_content(&mut config, "vmess://%%%%", None)
            .await
            .expect("bad share line should return diagnostics");

        assert_eq!(result.imported, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.line_issues.len(), 1);
        assert_eq!(result.line_issues[0].line, 1);
        let voya_core::ImportLineCode::ParseFailed { detail } = &result.line_issues[0].code else {
            panic!(
                "expected a parse failure, got {:?}",
                result.line_issues[0].code
            );
        };
        assert!(!detail.contains("%%%%"), "{detail}");
    }

    #[tokio::test]
    async fn manual_import_persists_mixed_share_links_for_profiles_screen() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let text = [
            test_vmess_link("node-vmess-1.example.test", "JMS-TEST@node-vmess-1.example.test:17701"),
            "vless://00000000-0000-0000-0000-000000000101@node-vless.example.test:443?encryption=none&security=tls&sni=node-vless.example.test&fp=randomized&insecure=0&allowInsecure=0&type=ws&host=node-vless.example.test&path=%2F%3Fed%3D2048#node-vless.example.test".to_string(),
            test_ss_link("node-ss-1.example.test", "JMS-TEST@node-ss-1.example.test:17701"),
            test_vmess_link("node-vmess-2.example.test", "JMS-TEST@node-vmess-2.example.test:17701"),
            test_ss_link("node-ss-2.example.test", "JMS-TEST@node-ss-2.example.test:17701"),
            test_vmess_link("node-vmess-3.example.test", "JMS-TEST@node-vmess-3.example.test:17701"),
            test_vmess_link("node-vmess-4.example.test", "JMS-TEST@node-vmess-4.example.test:17701"),
        ]
        .join("\n");

        let result = manager
            .import_subscription_content(&mut config, &text, None)
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(result.imported, 7);
        assert_eq!(result.parsed, 7);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.filtered, 0);
        assert_eq!(result.deduped, 0);
        assert_eq!(result.failed, 0);
        assert_eq!(result.discarded_node_overrides, 3);
        assert_eq!(result.imported_index_ids.len(), 7);
        assert!(result.line_issues.is_empty());

        let profiles = database
            .profiles()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 7);

        let visible_profiles = ProfileManager::new(&database)
            .list_profiles(&config, None, None)
            .await
            .expect("subscription manager test operation should succeed")
            .items;
        assert_eq!(visible_profiles.len(), 7);
        assert!(visible_profiles
            .iter()
            .any(|item| item.profile.remarks == "node-vless.example.test"));
    }

    #[tokio::test]
    async fn manual_import_updates_duplicate_profiles_instead_of_creating_rows() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let text = [
            test_vmess_link(
                "node-vmess-1.example.test",
                "JMS-TEST@node-vmess-1.example.test:17701",
            ),
            test_vless_link("node-vless.example.test", "node-vless.example.test"),
            test_ss_link(
                "node-ss-1.example.test",
                "JMS-TEST@node-ss-1.example.test:17701",
            ),
            test_vmess_link(
                "node-vmess-2.example.test",
                "JMS-TEST@node-vmess-2.example.test:17701",
            ),
            test_ss_link(
                "node-ss-2.example.test",
                "JMS-TEST@node-ss-2.example.test:17701",
            ),
            test_vmess_link(
                "node-vmess-3.example.test",
                "JMS-TEST@node-vmess-3.example.test:17701",
            ),
            test_vmess_link(
                "node-vmess-4.example.test",
                "JMS-TEST@node-vmess-4.example.test:17701",
            ),
        ]
        .join("\n");

        let first = manager
            .import_subscription_content(&mut config, &text, None)
            .await
            .expect("subscription manager test operation should succeed");
        let second = manager
            .import_subscription_content(&mut config, &text, None)
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(first.imported, 7);
        assert_eq!(first.updated, 0);
        assert_eq!(second.imported, 7);
        assert_eq!(second.updated, 7);
        assert_eq!(second.imported_index_ids, first.imported_index_ids);
        assert_eq!(second.updated_index_ids, first.imported_index_ids);

        let profiles = database
            .profiles()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 7);
    }

    #[tokio::test]
    async fn subscription_import_keeps_manual_duplicates_independent() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let text = test_vless_link("same.example.test", "manual node");
        let manual = manager
            .import_subscription_content(&mut config, &text, None)
            .await
            .expect("subscription manager test operation should succeed");
        let sub = manager
            .save_subscription(SubItem {
                id: "sub-same".to_string(),
                remarks: "Same".to_string(),
                url: "https://example.test/sub".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");

        let result = manager
            .import_subscription_content(&mut config, &text, Some(&sub.id))
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(result.imported, 1);
        assert_eq!(result.updated, 0);
        assert_ne!(result.imported_index_ids, manual.imported_index_ids);
        let profiles = database
            .profiles()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 2);
        assert!(profiles
            .iter()
            .any(|profile| profile.index_id == manual.imported_index_ids[0]
                && profile.subscription_id.is_none()));
        assert!(profiles
            .iter()
            .any(|profile| profile.subscription_id.as_deref() == Some(sub.id.as_str())));
        assert!(manager
            .import_profiles_from_text(&mut config, &text, Some(&sub.id))
            .await
            .is_err());
        assert_eq!(database.profiles().list().await.expect("profiles").len(), 2);
    }

    /// Providers commonly hand out several plan URLs that carry the same
    /// servers. Subscription ownership must remain stable on every update.
    #[tokio::test]
    async fn a_node_offered_by_two_subscriptions_stays_with_both() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        for id in ["sub-a", "sub-b"] {
            manager
                .save_subscription(SubItem {
                    id: id.to_string(),
                    remarks: id.to_string(),
                    url: format!("https://example.test/{id}"),
                    ..SubItem::default()
                })
                .await
                .expect("subscription manager test operation should succeed");
        }
        let text = test_vless_link("shared.example.test", "shared node");

        let first_a = manager
            .import_subscription_content(&mut config, &text, Some("sub-a"))
            .await
            .expect("subscription manager test operation should succeed");
        let first_b = manager
            .import_subscription_content(&mut config, &text, Some("sub-b"))
            .await
            .expect("subscription manager test operation should succeed");
        let second_a = manager
            .import_subscription_content(&mut config, &text, Some("sub-a"))
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(first_b.imported, 1);
        assert_eq!(
            first_b.updated, 0,
            "the second subscription must create its own row instead of stealing the first's"
        );
        assert_eq!(first_b.removed_duplicates, 0);
        assert_eq!(second_a.updated, 1);
        assert_eq!(second_a.imported_index_ids, first_a.imported_index_ids);
        assert_eq!(
            second_a.removed_existing, 0,
            "re-updating the first subscription must not disturb the second"
        );

        let owners = database
            .profiles()
            .list()
            .await
            .expect("profiles should list")
            .into_iter()
            .filter_map(|profile| profile.subscription_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            owners,
            BTreeSet::from(["sub-a".to_string(), "sub-b".to_string()])
        );
        for id in ["sub-a", "sub-b"] {
            assert_eq!(
                database
                    .profiles()
                    .list_by_subscription_id(Some(id))
                    .await
                    .expect("profiles should list")
                    .len(),
                1,
                "{id} should still own exactly one node"
            );
        }
    }

    #[tokio::test]
    async fn importing_into_an_unknown_subscription_names_the_missing_subscription() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();

        let error = manager
            .import_subscription_content(
                &mut config,
                &test_vless_link("ghost.example.test", "ghost"),
                Some("sub-missing"),
            )
            .await
            .expect_err("an unknown subscription id must not reach the foreign key");

        assert!(
            matches!(
                &error,
                SubscriptionManagerError::SubscriptionNotFound(id) if id == "sub-missing"
            ),
            "{error:?}"
        );
        assert!(database
            .profiles()
            .list()
            .await
            .expect("profiles should list")
            .is_empty());
    }

    #[tokio::test]
    async fn subscription_import_preserves_matching_ids_and_removes_stale_subscription_profiles() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let sub = manager
            .save_subscription(SubItem {
                id: "sub-refresh".to_string(),
                remarks: "Refresh".to_string(),
                url: "https://example.test/sub".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");
        let first_text = [
            test_vless_link("keep.example.test", "keep old"),
            test_vless_link("stale.example.test", "stale"),
        ]
        .join("\n");
        let first = manager
            .import_subscription_content(&mut config, &first_text, Some(&sub.id))
            .await
            .expect("subscription manager test operation should succeed");
        let keep_index_id = first.imported_index_ids[0].clone();
        let stale_index_id = first.imported_index_ids[1].clone();
        let second_text = [
            test_vless_link("keep.example.test", "keep renamed"),
            test_vless_link("new.example.test", "new"),
        ]
        .join("\n");

        let second = manager
            .import_subscription_content(&mut config, &second_text, Some(&sub.id))
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(second.imported, 2);
        assert_eq!(second.updated, 1);
        assert_eq!(second.removed_existing, 1);
        assert!(second.imported_index_ids.contains(&keep_index_id));
        assert!(!second.imported_index_ids.contains(&stale_index_id));
        let profiles = database
            .profiles()
            .list_by_subscription_id(Some(&sub.id))
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 2);
        assert!(profiles
            .iter()
            .any(|profile| profile.index_id == keep_index_id && profile.remarks == "keep renamed"));
        assert!(!profiles
            .iter()
            .any(|profile| profile.index_id == stale_index_id));
    }

    #[tokio::test]
    async fn duplicate_cleanup_prefers_active_profile_as_canonical() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let profile_manager = ProfileManager::new(&database);
        let mut config = AppConfig::default();
        let text = "vless://uuid@example.test:443#Imported";
        let initial = manager
            .import_subscription_content(&mut config, text, None)
            .await
            .expect("subscription manager test operation should succeed");
        let original_index_id = initial.imported_index_ids[0].clone();
        let original = database
            .profiles()
            .get(&original_index_id)
            .await
            .expect("subscription manager test operation should succeed")
            .expect("imported profile should exist");
        let mut active_duplicate = original.clone();
        active_duplicate.index_id = "active".to_string();
        active_duplicate.remarks = "Active".to_string();
        profile_manager
            .save_imported_profile(&mut config, active_duplicate)
            .await
            .expect("subscription manager test operation should succeed");
        database
            .profile_exs()
            .set_sort(&original_index_id, 10)
            .await
            .expect("subscription manager test operation should succeed");
        database
            .profile_exs()
            .set_sort("active", 20)
            .await
            .expect("subscription manager test operation should succeed");
        config.index_id = "active".to_string();

        let result = manager
            .import_subscription_content(&mut config, text, None)
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(result.imported, 1);
        assert_eq!(result.updated, 1);
        assert_eq!(result.removed_duplicates, 1);
        assert_eq!(result.imported_index_ids, vec!["active".to_string()]);
        assert_eq!(config.index_id, "active");

        let profiles = database
            .profiles()
            .list_with_profile_ex(None)
            .await
            .expect("subscription manager test operation should succeed")
            .items;
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].0.index_id, "active");
        assert_eq!(profiles[0].0.remarks, "Imported");
        assert_eq!(profiles[0].1.sort, 20);
    }

    fn sample_profile(index_id: &str, remarks: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            remarks: remarks.to_string(),
            protocol: ProfileProtocol::Vless {
                server: ServerEndpoint {
                    address: "example.test".to_string(),
                    port: 443,
                },
                uuid: "uuid".to_string(),
                flow: None,
                encryption: Some("none".to_string()),
            },
            transport: Some(ProfileTransport::Tcp {
                header: None,
                host: None,
                path: None,
            }),
            ..ProfileItem::default()
        }
    }

    fn test_vmess_link(address: &str, remarks: &str) -> String {
        let json = format!(
            r#"{{
                "v": "2",
                "ps": "{remarks}",
                "add": "{address}",
                "port": "17701",
                "id": "00000000-0000-0000-0000-000000000100",
                "aid": "0",
                "scy": "auto",
                "net": "tcp",
                "type": "none",
                "host": "",
                "path": "",
                "tls": "",
                "sni": "",
                "alpn": "",
                "fp": "",
                "insecure": "0"
            }}"#
        );
        format!("vmess://{}", STANDARD.encode(json).trim_end_matches('='))
    }

    fn test_vless_link(address: &str, remarks: &str) -> String {
        format!(
            "vless://00000000-0000-0000-0000-000000000101@{address}:443?encryption=none#{}",
            remarks.replace('@', "%40").replace(':', "%3A")
        )
    }

    fn test_ss_link(address: &str, remarks: &str) -> String {
        let user_info = STANDARD
            .encode("aes-256-gcm:test-password")
            .trim_end_matches('=')
            .to_string();
        format!(
            "ss://{user_info}@{address}:17701?#{}",
            remarks.replace('@', "%40").replace(':', "%3A")
        )
    }

    async fn spawn_http_fixture(
        routes: HashMap<String, String>,
        max_requests: usize,
        seen_user_agents: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let routes = routes
            .into_iter()
            .map(|(path, body)| (path, (body, Vec::new())))
            .collect();

        spawn_http_fixture_with_headers(routes, max_requests, seen_user_agents).await
    }

    async fn spawn_http_fixture_with_headers(
        routes: HashMap<String, (String, Vec<(String, String)>)>,
        max_requests: usize,
        seen_user_agents: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("subscription manager test operation should succeed");
        let address = listener
            .local_addr()
            .expect("subscription manager test operation should succeed");
        let routes = Arc::new(routes);

        tokio::spawn(async move {
            for _ in 0..max_requests {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let routes = Arc::clone(&routes);
                let seen_user_agents = Arc::clone(&seen_user_agents);
                tokio::spawn(async move {
                    let mut buffer = vec![0; 4096];
                    let bytes_read = socket.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .and_then(|target| target.split('?').next())
                        .unwrap_or("/");
                    let user_agent = request
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("user-agent")
                                .then(|| value.trim().to_string())
                        })
                        .unwrap_or_default();
                    seen_user_agents.lock().await.push(user_agent);
                    let (body, extra_headers) = routes.get(path).cloned().unwrap_or_default();
                    let status = if routes.contains_key(path) {
                        "200 OK"
                    } else {
                        "404 Not Found"
                    };
                    let extra_headers = extra_headers
                        .iter()
                        .map(|(name, value)| format!("{name}: {value}\r\n"))
                        .collect::<String>();
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        format!("http://{address}")
    }
}
