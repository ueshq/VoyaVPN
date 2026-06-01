use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use regex::Regex;
use thiserror::Error;
use voya_core::{
    parse_full_custom_config, parse_inner_share_links, parse_share_link, parse_ss_sip008,
    parse_wireguard_config, profile_items_match, AppConfig, ConfigType, ImportProfilesResult,
    ProfileItem, SubItem, SubscriptionUpdateResult,
};
use voya_db::{Database, DbError};
use voya_net::{
    decode_base64_payload, DownloadError, SubscriptionClient, SubscriptionFetchOptions,
    SubscriptionFetchSource,
};

use crate::profiles::{ProfileManager, ProfileManagerError};

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
    #[error("no importable profiles were found")]
    NoImportableProfiles,
}

#[derive(Debug, Clone, Copy)]
pub struct SubscriptionManager<'db> {
    database: &'db Database,
}

impl<'db> SubscriptionManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self { database }
    }

    pub async fn list_subscriptions(&self) -> Result<Vec<SubItem>> {
        Ok(self.database.subscriptions().list().await?)
    }

    pub async fn get_subscription(&self, id: &str) -> Result<Option<SubItem>> {
        Ok(self.database.subscriptions().get(id).await?)
    }

    pub async fn save_subscription(
        &self,
        config: &mut AppConfig,
        mut item: SubItem,
    ) -> Result<SubItem> {
        normalize_subscription(&mut item);
        if item.remarks.is_empty() {
            return Err(SubscriptionManagerError::MissingRemarks);
        }
        if !item.url.is_empty() && !is_http_url(&item.url) {
            return Err(SubscriptionManagerError::InvalidSubscriptionUrl);
        }

        if item.id.is_empty() {
            item.id = generate_subscription_id();
        }
        if item.sort <= 0 {
            item.sort = self.database.subscriptions().max_sort().await? + 1;
        }

        self.database.subscriptions().upsert(&item).await?;
        if config.sub_index_id.is_empty() {
            config.sub_index_id.clone_from(&item.id);
        }

        Ok(item)
    }

    pub async fn add_subscription_from_url(
        &self,
        config: &mut AppConfig,
        url: &str,
    ) -> Result<SubItem> {
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

        self.save_subscription(
            config,
            SubItem {
                remarks: extract_remarks_from_url(url).unwrap_or_else(|| "import_sub".to_string()),
                url: url.to_string(),
                ..SubItem::default()
            },
        )
        .await
    }

    pub async fn delete_subscriptions(
        &self,
        config: &mut AppConfig,
        ids: &[String],
    ) -> Result<u32> {
        let mut deleted = 0_u32;
        for id in ids {
            if self.database.subscriptions().delete(id).await? {
                deleted = deleted.saturating_add(1);
                self.database.profiles().delete_by_subid(id, false).await?;
            }
        }

        let subs = self.database.subscriptions().list().await?;
        if !config.sub_index_id.is_empty()
            && !subs.iter().any(|item| item.id == config.sub_index_id)
        {
            config.sub_index_id = subs.last().map(|item| item.id.clone()).unwrap_or_default();
        }
        ProfileManager::new(self.database)
            .ensure_active_profile(config)
            .await?;

        Ok(deleted)
    }

    pub async fn import_profiles_from_text(
        &self,
        config: &mut AppConfig,
        text: &str,
        subid: Option<&str>,
        is_sub: bool,
    ) -> Result<ImportProfilesResult> {
        let subid = subid.map(str::trim).filter(|value| !value.is_empty());
        let sub_item = match (is_sub, subid) {
            (true, Some(id)) => self.database.subscriptions().get(id).await?,
            _ => None,
        };
        let filter = sub_item.as_ref().and_then(|item| item.filter.as_deref());
        let regex = compile_filter(filter)?;
        let pre_socks_port = sub_item.as_ref().and_then(|item| item.pre_socks_port);
        let old_profiles = if is_sub {
            self.database.profiles().list_by_subid(subid).await?
        } else {
            Vec::new()
        };
        let active_profile = old_profiles
            .iter()
            .find(|profile| !config.index_id.is_empty() && profile.index_id == config.index_id)
            .cloned();
        let removed_existing = if is_sub {
            if let Some(id) = subid {
                self.database.profiles().delete_by_subid(id, true).await?
            } else {
                0
            }
        } else {
            0
        };

        let mut profiles = self
            .parse_import_text(config, text, subid.unwrap_or_default(), is_sub)
            .await?;
        let before_filter = profiles.len();
        if let Some(regex) = &regex {
            profiles.retain(|profile| regex.is_match(&profile.remarks));
        }
        let mut skipped = before_filter.saturating_sub(profiles.len());

        for profile in &mut profiles {
            profile.subid = subid.unwrap_or_default().to_string();
            profile.is_sub = is_sub;
            profile.pre_socks_port = pre_socks_port;
        }

        let before_dedupe = profiles.len();
        profiles = dedupe_profiles(profiles);
        skipped += before_dedupe.saturating_sub(profiles.len());
        if profiles.is_empty() {
            ProfileManager::new(self.database)
                .ensure_active_profile(config)
                .await?;
            return Ok(ImportProfilesResult {
                imported: 0,
                skipped: u32::try_from(skipped).unwrap_or(u32::MAX),
                removed_existing: u32::try_from(removed_existing).unwrap_or(u32::MAX),
                subid: subid.map(str::to_string),
                imported_index_ids: Vec::new(),
            });
        }

        let profile_manager = ProfileManager::new(self.database);
        let mut imported_index_ids = Vec::new();
        let mut active_replacement = None;
        for profile in profiles {
            let matches_old_active = active_profile
                .as_ref()
                .is_some_and(|old| profile_items_match(old, &profile, false));
            let saved = profile_manager.save_profile(config, profile).await?;
            if matches_old_active {
                active_replacement = Some(saved.profile.index_id.clone());
            }
            imported_index_ids.push(saved.profile.index_id);
        }

        if let Some(active_replacement) = active_replacement {
            config.index_id = active_replacement;
        }
        profile_manager.ensure_active_profile(config).await?;

        Ok(ImportProfilesResult {
            imported: u32::try_from(imported_index_ids.len()).unwrap_or(u32::MAX),
            skipped: u32::try_from(skipped).unwrap_or(u32::MAX),
            removed_existing: u32::try_from(removed_existing).unwrap_or(u32::MAX),
            subid: subid.map(str::to_string),
            imported_index_ids,
        })
    }

    pub async fn update_subscriptions(
        &self,
        config: &mut AppConfig,
        subid: Option<&str>,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
        now: i64,
    ) -> Result<SubscriptionUpdateResult> {
        let client = SubscriptionClient::new();
        let mut result = SubscriptionUpdateResult::default();
        let subscriptions = self.database.subscriptions().list().await?;

        for mut item in subscriptions {
            if subid.is_some_and(|wanted| wanted != item.id) {
                continue;
            }
            if item.id.trim().is_empty() || item.url.trim().is_empty() || !is_http_url(&item.url) {
                result.skipped = result.skipped.saturating_add(1);
                continue;
            }
            if !item.enabled {
                result.skipped = result.skipped.saturating_add(1);
                result
                    .messages
                    .push(format!("{}->subscription update skipped", item.remarks));
                continue;
            }

            let fetch = client
                .fetch(
                    &SubscriptionFetchSource {
                        url: item.url.clone(),
                        more_url: item.more_url.clone(),
                        user_agent: item.user_agent.clone(),
                        convert_target: item.convert_target.clone(),
                        sub_convert_url: config.const_item.sub_convert_url.clone(),
                    },
                    &SubscriptionFetchOptions {
                        prefer_proxy,
                        proxy_url: proxy_url.map(str::to_string),
                    },
                )
                .await;

            match fetch {
                Ok(fetch) => {
                    let import = self
                        .import_profiles_from_text(config, &fetch.content, Some(&item.id), true)
                        .await?;
                    item.update_time = now;
                    self.database.subscriptions().upsert(&item).await?;
                    result.updated = result.updated.saturating_add(1);
                    result.imported = result.imported.saturating_add(import.imported);
                    result.removed_existing = result
                        .removed_existing
                        .saturating_add(import.removed_existing);
                    result.messages.push(format!(
                        "{}->imported {} profiles",
                        item.remarks, import.imported
                    ));
                }
                Err(error) => {
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!("{}->{}", item.remarks, error));
                }
            }
        }

        Ok(result)
    }

    pub async fn run_due_updates(
        &self,
        config: &mut AppConfig,
        now: i64,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
    ) -> Result<SubscriptionUpdateResult> {
        let due_ids = self
            .database
            .subscriptions()
            .list()
            .await?
            .into_iter()
            .filter(|item| item.auto_update_interval > 0)
            .filter(|item| {
                now.saturating_sub(item.update_time) >= i64::from(item.auto_update_interval) * 60
            })
            .map(|item| item.id)
            .collect::<Vec<_>>();

        let mut combined = SubscriptionUpdateResult::default();
        for id in due_ids {
            let result = self
                .update_subscriptions(config, Some(&id), prefer_proxy, proxy_url, now)
                .await?;
            combined.updated = combined.updated.saturating_add(result.updated);
            combined.skipped = combined.skipped.saturating_add(result.skipped);
            combined.imported = combined.imported.saturating_add(result.imported);
            combined.removed_existing = combined
                .removed_existing
                .saturating_add(result.removed_existing);
            combined.messages.extend(result.messages);
        }

        Ok(combined)
    }

    async fn parse_import_text(
        &self,
        config: &mut AppConfig,
        text: &str,
        subid: &str,
        is_sub: bool,
    ) -> Result<Vec<ProfileItem>> {
        let mut profiles = Vec::new();
        let mut added_subscription = false;
        let mut contents = Vec::new();
        if let Some(decoded) = decode_base64_payload(text) {
            contents.push(decoded);
        }
        contents.push(text.to_string());

        for content in contents {
            let mut lines_seen = BTreeSet::new();
            for line in content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                if is_sub && !lines_seen.insert(line.to_string()) {
                    continue;
                }
                if !is_sub && is_http_url(line) {
                    self.add_subscription_from_url(config, line).await?;
                    added_subscription = true;
                    continue;
                }
                if let Ok(profile) = parse_share_link(line) {
                    profiles.push(profile);
                }
            }

            if let Ok(mut inner) = parse_inner_share_links(&content, subid) {
                profiles.append(&mut inner);
            }
            if let Ok(mut ss) = parse_ss_sip008(&content) {
                profiles.append(&mut ss);
            }
            if let Ok(mut wireguard) = parse_wireguard_config(&content) {
                profiles.append(&mut wireguard);
            }
            if let Ok(custom_imports) = parse_full_custom_config(&content, None) {
                profiles.extend(custom_imports.into_iter().map(|import| {
                    let mut profile = import.profile;
                    profile.address = import.contents;
                    profile
                }));
            }
        }

        if profiles.is_empty() && !added_subscription {
            Err(SubscriptionManagerError::NoImportableProfiles)
        } else {
            Ok(profiles)
        }
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
    item.prev_profile = trimmed_option(item.prev_profile.take());
    item.next_profile = trimmed_option(item.next_profile.take());
    item.memo = trimmed_option(item.memo.take());
}

fn trimmed_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn compile_filter(filter: Option<&str>) -> Result<Option<Regex>> {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    Regex::new(filter)
        .map(Some)
        .map_err(|error| SubscriptionManagerError::InvalidFilter(error.to_string()))
}

fn dedupe_profiles(profiles: Vec<ProfileItem>) -> Vec<ProfileItem> {
    let mut kept = Vec::<ProfileItem>::new();
    for profile in profiles {
        if profile.config_type != ConfigType::Custom
            && !profile.is_complex()
            && kept
                .iter()
                .any(|existing| profile_items_match(existing, &profile, false))
        {
            continue;
        }
        kept.push(profile);
    }
    kept
}

fn is_http_url(value: &str) -> bool {
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
    use std::{collections::HashMap, sync::Arc};

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };
    use voya_core::CoreType;

    use super::*;

    #[tokio::test]
    async fn subscription_import_filters_dedupes_persists_and_updates_active_profile() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let sub = manager
            .save_subscription(
                &mut config,
                SubItem {
                    id: "sub-us".to_string(),
                    remarks: "US sub".to_string(),
                    url: "https://example.test/sub".to_string(),
                    filter: Some("US".to_string()),
                    ..SubItem::default()
                },
            )
            .await
            .unwrap();
        let old = ProfileManager::new(&database)
            .save_profile(&mut config, sample_profile("old", "US old"))
            .await
            .unwrap();
        let mut old_profile = old.profile.clone();
        old_profile.subid.clone_from(&sub.id);
        old_profile.is_sub = true;
        database.profiles().upsert(&old_profile).await.unwrap();
        config.index_id = old_profile.index_id.clone();

        let text = [
            "vless://uuid@example.test:443?security=tls#US%20node",
            "vless://uuid@example.test:443?security=tls#US%20duplicate",
            "trojan://secret@example.test:443#JP%20node",
        ]
        .join("\n");
        let result = manager
            .import_profiles_from_text(&mut config, &text, Some(&sub.id), true)
            .await
            .unwrap();

        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 2);
        assert_eq!(result.removed_existing, 1);
        let profiles = database
            .profiles()
            .list_by_subid(Some(&sub.id))
            .await
            .unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].remarks, "US node");
        assert_eq!(profiles[0].subid, sub.id);
        assert!(profiles[0].is_sub);
        assert_eq!(config.index_id, profiles[0].index_id);
    }

    #[tokio::test]
    async fn subscription_update_downloads_base64_more_url_and_conversion_target() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let main = STANDARD.encode("vless://uuid-a@example.test:443#US%20A");
        let extra = "trojan://secret@example.test:443#US%20B".to_string();
        let converted = "vless://uuid-c@example.test:443#US%20C".to_string();
        let base = spawn_http_fixture(
            HashMap::from([
                ("/main".to_string(), main),
                ("/extra".to_string(), extra),
                ("/convert".to_string(), converted),
            ]),
            3,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory().await.unwrap();
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        config.const_item.sub_convert_url = Some(format!("{base}/convert?url={{0}}"));
        manager
            .save_subscription(
                &mut config,
                SubItem {
                    id: "sub-plain".to_string(),
                    remarks: "Plain".to_string(),
                    url: format!("{base}/main"),
                    more_url: format!("{base}/extra"),
                    user_agent: "SubUA/3".to_string(),
                    ..SubItem::default()
                },
            )
            .await
            .unwrap();
        manager
            .save_subscription(
                &mut config,
                SubItem {
                    id: "sub-convert".to_string(),
                    remarks: "Convert".to_string(),
                    url: format!("{base}/raw"),
                    more_url: format!("{base}/should-not-fetch"),
                    user_agent: "SubUA/3".to_string(),
                    convert_target: Some("clash".to_string()),
                    ..SubItem::default()
                },
            )
            .await
            .unwrap();

        let result = manager
            .update_subscriptions(&mut config, None, false, None, 900)
            .await
            .unwrap();
        assert_eq!(result.updated, 2);
        assert_eq!(result.imported, 3);

        let profiles = database.profiles().list().await.unwrap();
        assert_eq!(profiles.len(), 3);
        assert!(profiles.iter().any(|profile| profile.remarks == "US A"));
        assert!(profiles.iter().any(|profile| profile.remarks == "US B"));
        assert!(profiles.iter().any(|profile| profile.remarks == "US C"));
        assert_eq!(
            database
                .subscriptions()
                .get("sub-plain")
                .await
                .unwrap()
                .unwrap()
                .update_time,
            900
        );
        assert_eq!(
            seen_user_agents.lock().await.as_slice(),
            ["SubUA/3", "SubUA/3", "SubUA/3"]
        );
    }

    #[tokio::test]
    async fn due_update_only_runs_subscriptions_past_their_interval() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([(
                "/due".to_string(),
                "vless://uuid@example.test:443#Due".to_string(),
            )]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let database = Database::connect_in_memory().await.unwrap();
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        manager
            .save_subscription(
                &mut config,
                SubItem {
                    id: "due".to_string(),
                    remarks: "Due".to_string(),
                    url: format!("{base}/due"),
                    auto_update_interval: 1,
                    update_time: 0,
                    ..SubItem::default()
                },
            )
            .await
            .unwrap();
        manager
            .save_subscription(
                &mut config,
                SubItem {
                    id: "later".to_string(),
                    remarks: "Later".to_string(),
                    url: format!("{base}/later"),
                    auto_update_interval: 60,
                    update_time: 100,
                    ..SubItem::default()
                },
            )
            .await
            .unwrap();

        let result = manager
            .run_due_updates(&mut config, 120, false, None)
            .await
            .unwrap();

        assert_eq!(result.updated, 1);
        assert_eq!(result.imported, 1);
        assert_eq!(database.profiles().list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn manual_import_accepts_full_json_custom_config() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let json = r#"{"remarks":"custom-json","inbounds":[],"outbounds":[],"routing":{}}"#;

        let result = manager
            .import_profiles_from_text(&mut config, json, None, false)
            .await
            .unwrap();

        assert_eq!(result.imported, 1);
        let profiles = database.profiles().list().await.unwrap();
        assert_eq!(profiles[0].config_type, ConfigType::Custom);
        assert_eq!(profiles[0].remarks, "custom-json");
        assert_eq!(profiles[0].address, json);
    }

    fn sample_profile(index_id: &str, remarks: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VLESS,
            core_type: Some(CoreType::Xray),
            remarks: remarks.to_string(),
            address: "example.test".to_string(),
            port: 443,
            password: "uuid".to_string(),
            ..ProfileItem::default()
        }
    }

    async fn spawn_http_fixture(
        routes: HashMap<String, String>,
        max_requests: usize,
        seen_user_agents: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
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
                    let body = routes.get(path).cloned().unwrap_or_default();
                    let status = if routes.contains_key(path) {
                        "200 OK"
                    } else {
                        "404 Not Found"
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        format!("http://{address}")
    }
}
