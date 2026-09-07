use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use regex::Regex;
use thiserror::Error;
use voya_core::{
    parse_full_custom_config, parse_share_link, parse_ss_sip008, parse_voya_profile_bundle,
    parse_wireguard_config, profile_items_match, AppConfig, ConfigType, ImportProfilesResult,
    ProfileExItem, ProfileItem, ProfileProtocol, SubItem, SubMetadataItem,
    SubscriptionUpdateResult,
};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};
use voya_net::{decode_base64_payload, DownloadError};

use crate::groups::GroupManagerError;
use crate::profiles::{normalize_profile, ProfileManager, ProfileManagerError};

use super::update_flow::{
    ensure_subscription_auto_group, persist_subscription_metadata, prepare_subscription_snapshot,
    PreparedSubscriptionUpdate,
};

static SUBSCRIPTION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub type Result<T> = std::result::Result<T, SubscriptionManagerError>;

#[derive(Debug, Error)]
pub enum SubscriptionManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Profile(#[from] ProfileManagerError),
    #[error(transparent)]
    Group(#[from] Box<GroupManagerError>),
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

        // Auto groups keep their own subscription_id NULL so re-imports never
        // prune them; clean up any group whose source subscription is gone.
        let orphaned_group_ids = self
            .database
            .profiles()
            .list()
            .await?
            .into_iter()
            .filter(|profile| {
                matches!(
                    &profile.protocol,
                    ProfileProtocol::PolicyGroup {
                        source_subscription_id: Some(source),
                        ..
                    } if ids.contains(source)
                )
            })
            .map(|profile| profile.index_id)
            .collect::<Vec<_>>();
        if !orphaned_group_ids.is_empty() {
            self.database
                .profiles()
                .delete_many(&orphaned_group_ids)
                .await?;
        }

        ProfileManager::from_session(self.database)
            .ensure_active_profile(config)
            .await?;

        Ok(deleted)
    }

    pub async fn import_profiles_from_text(
        &self,
        config: &mut AppConfig,
        text: &str,
        subscription_id: Option<&str>,
    ) -> Result<ImportProfilesResult> {
        let subscription_id = subscription_id
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let no_active_profile_at_entry = config.index_id.trim().is_empty();
        let sub_item = match subscription_id {
            Some(id) => self.database.subscriptions().get(id).await?,
            None => None,
        };
        let filter = sub_item.as_ref().and_then(|item| item.filter.as_deref());
        let regex = compile_filter(filter)?;
        let old_profiles = if subscription_id.is_some() {
            self.database
                .profiles()
                .list_by_subscription_id(subscription_id)
                .await?
        } else {
            Vec::new()
        };

        let parsed_import = self
            .parse_import_text(text, subscription_id.unwrap_or_default())
            .await?;
        let mut profiles = parsed_import.profiles;
        let parsed = profiles.len();
        let before_filter = profiles.len();
        if let Some(regex) = &regex {
            profiles.retain(|profile| regex.is_match(&profile.remarks));
        }
        let filtered = before_filter.saturating_sub(profiles.len());

        for profile in &mut profiles {
            profile.subscription_id = subscription_id.map(str::to_string);
            normalize_profile(config, profile);
        }

        let before_dedupe = profiles.len();
        profiles = dedupe_profiles(profiles);
        let deduped = before_dedupe.saturating_sub(profiles.len());
        let skipped = filtered
            .saturating_add(deduped)
            .saturating_add(parsed_import.failed_lines);
        if profiles.is_empty() {
            ProfileManager::from_session(self.database)
                .ensure_active_profile(config)
                .await?;
            return Ok(ImportProfilesResult {
                imported: 0,
                updated: 0,
                skipped: u32::try_from(skipped).unwrap_or(u32::MAX),
                parsed: u32::try_from(parsed).unwrap_or(u32::MAX),
                filtered: u32::try_from(filtered).unwrap_or(u32::MAX),
                deduped: u32::try_from(deduped).unwrap_or(u32::MAX),
                failed: u32::try_from(parsed_import.failed_lines).unwrap_or(u32::MAX),
                removed_existing: 0,
                removed_duplicates: 0,
                discarded_node_overrides: u32::try_from(parsed_import.discarded_node_overrides)
                    .unwrap_or(u32::MAX),
                subscription_id: subscription_id.map(str::to_string),
                imported_index_ids: Vec::new(),
                updated_index_ids: Vec::new(),
                messages: parsed_import.messages,
            });
        }

        let profile_manager = ProfileManager::from_session(self.database);
        let mut existing_profiles = self.database.profiles().list_with_profile_ex(None).await?;
        let mut imported_index_ids = Vec::new();
        let mut updated_index_ids = Vec::new();
        let mut duplicate_index_ids_to_remove = Vec::new();
        for mut profile in profiles {
            let match_indices = existing_profiles
                .iter()
                .enumerate()
                .filter_map(|(index, (existing, _))| {
                    profile_items_match(existing, &profile, false).then_some(index)
                })
                .collect::<Vec<_>>();

            if let Some(canonical_index) = choose_canonical_match_index(
                &match_indices,
                &existing_profiles,
                &config.index_id,
                subscription_id,
            ) {
                let canonical_index_id = existing_profiles[canonical_index].0.index_id.clone();
                let duplicate_index_ids = match_indices
                    .iter()
                    .filter_map(|index| {
                        let index_id = &existing_profiles[*index].0.index_id;
                        (index_id != &canonical_index_id).then(|| index_id.clone())
                    })
                    .collect::<Vec<_>>();

                profile.index_id.clone_from(&canonical_index_id);
                let saved = profile_manager.save_profile(config, profile).await?;
                update_existing_profile_cache(
                    &mut existing_profiles,
                    saved.profile.clone(),
                    saved.profile_ex.clone(),
                    &duplicate_index_ids,
                );
                duplicate_index_ids_to_remove.extend(duplicate_index_ids);
                updated_index_ids.push(saved.profile.index_id.clone());
                imported_index_ids.push(saved.profile.index_id.clone());
            } else {
                let saved = profile_manager.save_profile(config, profile).await?;
                existing_profiles.push((saved.profile.clone(), saved.profile_ex.clone()));
                imported_index_ids.push(saved.profile.index_id.clone());
            }
        }

        let removed_duplicates = if duplicate_index_ids_to_remove.is_empty() {
            0
        } else {
            self.database
                .profiles()
                .delete_many(&duplicate_index_ids_to_remove)
                .await?
        };

        let removed_existing = if subscription_id.is_some() {
            if let Some(id) = subscription_id {
                let retained_current_sub_index_ids: BTreeSet<&str> =
                    imported_index_ids.iter().map(String::as_str).collect();
                let stale_index_ids = old_profiles
                    .iter()
                    .filter(|profile| {
                        profile.subscription_id.as_deref() == Some(id)
                            && !retained_current_sub_index_ids.contains(profile.index_id.as_str())
                    })
                    .map(|profile| profile.index_id.clone())
                    .collect::<Vec<_>>();
                self.database
                    .profiles()
                    .delete_many(&stale_index_ids)
                    .await?
            } else {
                0
            }
        } else {
            0
        };

        let mut messages = parsed_import.messages;
        if let Some(id) = subscription_id {
            if !imported_index_ids.is_empty() && config.gui_item.auto_create_subscription_group {
                if let Some(group_remarks) = ensure_subscription_auto_group(
                    self.database,
                    config,
                    id,
                    no_active_profile_at_entry,
                )
                .await?
                {
                    messages.push(format!("{group_remarks}->auto group created"));
                }
            }
        }
        profile_manager.ensure_active_profile(config).await?;

        Ok(ImportProfilesResult {
            imported: u32::try_from(imported_index_ids.len()).unwrap_or(u32::MAX),
            updated: u32::try_from(updated_index_ids.len()).unwrap_or(u32::MAX),
            skipped: u32::try_from(skipped).unwrap_or(u32::MAX),
            parsed: u32::try_from(parsed).unwrap_or(u32::MAX),
            filtered: u32::try_from(filtered).unwrap_or(u32::MAX),
            deduped: u32::try_from(deduped).unwrap_or(u32::MAX),
            failed: u32::try_from(parsed_import.failed_lines).unwrap_or(u32::MAX),
            removed_existing: u32::try_from(removed_existing).unwrap_or(u32::MAX),
            removed_duplicates: u32::try_from(removed_duplicates).unwrap_or(u32::MAX),
            discarded_node_overrides: u32::try_from(parsed_import.discarded_node_overrides)
                .unwrap_or(u32::MAX),
            subscription_id: subscription_id.map(str::to_string),
            imported_index_ids,
            updated_index_ids,
            messages,
        })
    }

    pub async fn update_subscriptions(
        &self,
        config: &mut AppConfig,
        subscription_id: Option<&str>,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
    ) -> Result<SubscriptionUpdateResult> {
        let prepared = self
            .prepare_subscription_update(config, subscription_id, prefer_proxy, proxy_url)
            .await?;
        self.apply_prepared_subscription_update(config, prepared)
            .await
    }

    pub async fn prepare_subscription_update(
        &self,
        config: &AppConfig,
        subscription_id: Option<&str>,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
    ) -> Result<PreparedSubscriptionUpdate> {
        let subscriptions = self.database.subscriptions().list().await?;
        prepare_subscription_snapshot(
            config,
            subscriptions,
            subscription_id,
            prefer_proxy,
            proxy_url,
        )
        .await
    }

    pub async fn apply_prepared_subscription_update(
        &self,
        config: &mut AppConfig,
        prepared: PreparedSubscriptionUpdate,
    ) -> Result<SubscriptionUpdateResult> {
        let mut result = prepared.result;
        for prepared_import in prepared.imports {
            let current = self
                .database
                .subscriptions()
                .get(&prepared_import.item.id)
                .await?;
            if current.as_ref() != Some(&prepared_import.item) {
                result.skipped = result.skipped.saturating_add(1);
                result.messages.push(format!(
                    "{}->subscription changed while the update was downloading; prepared content was discarded",
                    prepared_import.item.remarks
                ));
                continue;
            }
            // The import runs first so the recorded `last_update_at` only
            // advances for a subscription that actually produced profiles.
            let import = self
                .import_profiles_from_text(
                    config,
                    &prepared_import.content,
                    Some(&prepared_import.item.id),
                )
                .await;
            match import {
                Ok(import) if import.imported > 0 => {
                    persist_subscription_metadata(self.database, &prepared_import, true).await?;
                    result.updated = result.updated.saturating_add(1);
                    result.imported = result.imported.saturating_add(import.imported);
                    result.removed_existing = result
                        .removed_existing
                        .saturating_add(import.removed_existing);
                    result.messages.push(format!(
                        "{}->imported {} profiles",
                        prepared_import.item.remarks, import.imported
                    ));
                }
                Ok(_) => {
                    persist_subscription_metadata(self.database, &prepared_import, false).await?;
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!(
                        "{}->no profiles were imported",
                        prepared_import.item.remarks
                    ));
                }
                Err(SubscriptionManagerError::NoImportableProfiles) => {
                    persist_subscription_metadata(self.database, &prepared_import, false).await?;
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!(
                        "{}->no importable profiles were found",
                        prepared_import.item.remarks
                    ));
                }
                // A bad filter belongs to one subscription; failing the whole
                // batch would roll back every sibling's successful import.
                Err(SubscriptionManagerError::InvalidFilter(reason)) => {
                    persist_subscription_metadata(self.database, &prepared_import, false).await?;
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!(
                        "{}->subscription filter is invalid: {reason}",
                        prepared_import.item.remarks
                    ));
                }
                Err(error) => return Err(error),
            }
        }

        Ok(result)
    }

    async fn parse_import_text(
        &self,
        text: &str,
        subscription_id: &str,
    ) -> Result<ParsedImportText> {
        let mut profiles = Vec::new();
        let mut added_subscription = false;
        let mut failed_lines = 0_usize;
        let mut discarded_node_overrides = 0_usize;
        let mut messages = Vec::new();
        let allow_subscription_import = subscription_id.trim().is_empty();
        let mut contents = Vec::new();
        if let Some(decoded) = decode_base64_payload(text) {
            contents.push(decoded);
        }
        contents.push(text.to_string());

        for content in contents {
            discarded_node_overrides =
                discarded_node_overrides.saturating_add(count_discarded_node_overrides(&content));
            let mut lines_seen = BTreeSet::new();
            for (line_index, line) in content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .enumerate()
            {
                if !subscription_id.is_empty() && !lines_seen.insert(line.to_string()) {
                    continue;
                }
                if allow_subscription_import && is_http_url(line) {
                    self.add_subscription_from_url(line).await?;
                    added_subscription = true;
                    messages.push(format!(
                        "Line {} added as a subscription source; run subscription update to import its profiles.",
                        line_index + 1
                    ));
                    continue;
                }
                match parse_share_link(line) {
                    Ok(profile) => profiles.push(profile),
                    Err(error) if should_report_line_parse_error(line) => {
                        failed_lines = failed_lines.saturating_add(1);
                        messages.push(format!("Line {} was skipped: {error}", line_index + 1));
                    }
                    Err(_) => {}
                }
            }

            if let Ok(mut bundle) = parse_voya_profile_bundle(&content, subscription_id) {
                profiles.append(&mut bundle);
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
                    if let ProfileProtocol::Custom { source, .. } = &mut profile.protocol {
                        *source = import.contents;
                    }
                    profile
                }));
            }
        }

        if profiles.is_empty() && !added_subscription && failed_lines == 0 {
            Err(SubscriptionManagerError::NoImportableProfiles)
        } else {
            Ok(ParsedImportText {
                failed_lines,
                discarded_node_overrides,
                messages,
                profiles,
            })
        }
    }
}

#[derive(Debug, Default)]
struct ParsedImportText {
    profiles: Vec<ProfileItem>,
    failed_lines: usize,
    discarded_node_overrides: usize,
    messages: Vec<String>,
}

fn count_discarded_node_overrides(content: &str) -> usize {
    let query_count = Regex::new(
        r"(?i)(?:\?|&)(?:allowinsecure|allow_insecure|insecure|fp|muxenabled|mux_enabled|upmbps|downmbps|hopinterval)=",
    )
    .map(|pattern| pattern.find_iter(content).count())
    .unwrap_or(0);
    let json_count = Regex::new(
        r#"(?i)"(?:fingerprint|fp|allowinsecure|allow_insecure|muxenabled|mux_enabled|upmbps|downmbps|hopinterval)"\s*:"#,
    )
    .map(|pattern| pattern.find_iter(content).count())
    .unwrap_or(0);

    query_count.saturating_add(json_count)
}

// Share-link schemes recognized by `parse_share_link`. A parse failure on a
// line starting with one of these is worth surfacing to the user; any other
// line is treated as noise and skipped silently. Kept in sync with the scheme
// dispatch in `voya_core::parse_share_link`.
const REPORTABLE_SHARE_LINK_SCHEMES: [&str; 15] = [
    "vmess://",
    "ss://",
    "socks://",
    "socks4://",
    "socks5://",
    "trojan://",
    "vless://",
    "hysteria2://",
    "hy2://",
    "tuic://",
    "wireguard://",
    "anytls://",
    "naive://",
    "naive+https://",
    "naive+quic://",
];

fn should_report_line_parse_error(line: &str) -> bool {
    let line = line.trim();
    REPORTABLE_SHARE_LINK_SCHEMES
        .iter()
        .any(|prefix| line_has_prefix_ci(line, prefix))
}

fn line_has_prefix_ci(line: &str, prefix: &str) -> bool {
    line.get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
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
        if profile.config_type() != ConfigType::Custom
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

fn choose_canonical_match_index(
    match_indices: &[usize],
    existing_profiles: &[(ProfileItem, ProfileExItem)],
    active_index_id: &str,
    target_subscription_id: Option<&str>,
) -> Option<usize> {
    match_indices.iter().copied().min_by_key(|index| {
        let (profile, profile_ex) = &existing_profiles[*index];
        let active_rank = if !active_index_id.is_empty() && profile.index_id == active_index_id {
            0
        } else {
            1
        };
        let target_subscription_id_rank = if target_subscription_id.is_some_and(|subscription_id| {
            profile.subscription_id.as_deref() == Some(subscription_id)
        }) {
            0
        } else {
            1
        };

        (
            active_rank,
            target_subscription_id_rank,
            profile_ex.sort,
            *index,
        )
    })
}

fn update_existing_profile_cache(
    existing_profiles: &mut Vec<(ProfileItem, ProfileExItem)>,
    saved_profile: ProfileItem,
    saved_profile_ex: ProfileExItem,
    removed_index_ids: &[String],
) {
    let saved_index_id = saved_profile.index_id.clone();
    existing_profiles.retain(|(profile, _)| {
        profile.index_id == saved_index_id
            || !removed_index_ids
                .iter()
                .any(|index_id| index_id == &profile.index_id)
    });

    if let Some((profile, profile_ex)) = existing_profiles
        .iter_mut()
        .find(|(profile, _)| profile.index_id == saved_index_id)
    {
        *profile = saved_profile;
        *profile_ex = saved_profile_ex;
    } else {
        existing_profiles.push((saved_profile, saved_profile_ex));
    }
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
    use std::{collections::HashMap, sync::Arc};

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };
    use voya_core::{ProfileProtocol, ProfileTransport, ServerEndpoint};

    use super::*;

    #[tokio::test]
    async fn subscription_import_filters_dedupes_persists_and_updates_active_profile() {
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
            .import_profiles_from_text(&mut config, &text, Some(&sub.id))
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
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        config.gui_item.auto_create_subscription_group = false;
        config.const_item.sub_convert_url = Some(format!("{base}/convert?url={{0}}"));
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
        manager
            .save_subscription(SubItem {
                id: "sub-convert".to_string(),
                remarks: "Convert".to_string(),
                url: format!("{base}/raw"),
                more_url: format!("{base}/should-not-fetch"),
                user_agent: "SubUA/3".to_string(),
                convert_target: Some("clash".to_string()),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");

        let result = manager
            .update_subscriptions(&mut config, None, false, None)
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(result.updated, 2);
        assert_eq!(result.imported, 3);

        let profiles = database
            .profiles()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 3);
        assert!(profiles.iter().any(|profile| profile.remarks == "US A"));
        assert!(profiles.iter().any(|profile| profile.remarks == "US B"));
        assert!(profiles.iter().any(|profile| profile.remarks == "US C"));
        assert_eq!(
            seen_user_agents.lock().await.as_slice(),
            ["SubUA/3", "SubUA/3", "SubUA/3"]
        );
    }

    #[tokio::test]
    async fn subscription_import_creates_auto_group_once_and_activates_on_first_import() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let sub = manager
            .save_subscription(SubItem {
                id: "sub-auto".to_string(),
                remarks: "Airport".to_string(),
                url: "https://example.test/auto".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");

        let result = manager
            .import_profiles_from_text(
                &mut config,
                "vless://uuid-a@example.test:443#US%20A\nvless://uuid-b@example.test:443#US%20B",
                Some(&sub.id),
            )
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(result.imported, 2);
        assert!(result
            .messages
            .iter()
            .any(|message| message.contains("auto group created")));

        let auto_groups = database
            .profiles()
            .list()
            .await
            .expect("profiles should list")
            .into_iter()
            .filter(|profile| {
                matches!(
                    &profile.protocol,
                    ProfileProtocol::PolicyGroup {
                        source_subscription_id: Some(source),
                        ..
                    } if source == &sub.id
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(auto_groups.len(), 1);
        let auto_group = &auto_groups[0];
        assert_eq!(auto_group.remarks, "Airport · Auto");
        assert_eq!(
            auto_group.subscription_id, None,
            "auto group must survive subscription re-imports"
        );
        assert_eq!(
            config.index_id, auto_group.index_id,
            "first import should activate the auto group"
        );

        // A second import neither duplicates the group nor steals activation.
        config.index_id = "some-user-choice".to_string();
        let second = manager
            .import_profiles_from_text(
                &mut config,
                "vless://uuid-c@example.test:443#US%20C",
                Some(&sub.id),
            )
            .await
            .expect("subscription manager test operation should succeed");
        assert!(!second
            .messages
            .iter()
            .any(|message| message.contains("auto group created")));
        let group_count = database
            .profiles()
            .list()
            .await
            .expect("profiles should list")
            .into_iter()
            .filter(|profile| matches!(&profile.protocol, ProfileProtocol::PolicyGroup { .. }))
            .count();
        assert_eq!(group_count, 1);

        // Deleting the subscription removes the auto group.
        manager
            .delete_subscriptions(&mut config, std::slice::from_ref(&sub.id))
            .await
            .expect("subscription manager test operation should succeed");
        let remaining_groups = database
            .profiles()
            .list()
            .await
            .expect("profiles should list")
            .into_iter()
            .filter(|profile| matches!(&profile.protocol, ProfileProtocol::PolicyGroup { .. }))
            .count();
        assert_eq!(remaining_groups, 0);
    }

    #[tokio::test]
    async fn subscription_import_skips_auto_group_when_setting_disabled() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        config.gui_item.auto_create_subscription_group = false;
        let sub = manager
            .save_subscription(SubItem {
                id: "sub-noauto".to_string(),
                remarks: "Plain".to_string(),
                url: "https://example.test/plain".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("subscription manager test operation should succeed");

        manager
            .import_profiles_from_text(
                &mut config,
                "vless://uuid-a@example.test:443#US%20A",
                Some(&sub.id),
            )
            .await
            .expect("subscription manager test operation should succeed");

        let group_count = database
            .profiles()
            .list()
            .await
            .expect("profiles should list")
            .into_iter()
            .filter(|profile| matches!(&profile.protocol, ProfileProtocol::PolicyGroup { .. }))
            .count();
        assert_eq!(group_count, 0);
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
                .any(|message| message.contains("Junk->no importable profiles were found")),
            "{:?}",
            result.messages
        );
        assert!(
            result
                .messages
                .iter()
                .any(|message| message.contains("Filtered->no profiles were imported")),
            "{:?}",
            result.messages
        );
        assert!(
            result
                .messages
                .iter()
                .any(|message| message.contains("URL Only->no importable profiles were found")),
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
            .prepare_subscription_update(&config, Some("empty-network"), false, None)
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
            .prepare_subscription_update(&config, Some("changing-source"), false, None)
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
    async fn manual_import_accepts_full_json_custom_config() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        let json = r#"{"remarks":"custom-json","inbounds":[],"outbounds":[],"route":{},"dns":{}}"#;

        let result = manager
            .import_profiles_from_text(&mut config, json, None)
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(result.imported, 1);
        let profiles = database
            .profiles()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles[0].config_type(), ConfigType::Custom);
        assert_eq!(profiles[0].remarks, "singbox_custom");
        assert_eq!(profiles[0].address(), json);
    }

    #[tokio::test]
    async fn manual_import_reports_bad_share_lines_without_exposing_payloads() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();

        let result = manager
            .import_profiles_from_text(&mut config, "vmess://%%%%", None)
            .await
            .expect("bad share line should return diagnostics");

        assert_eq!(result.imported, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.failed, 1);
        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].contains("Line 1 was skipped"));
        assert!(!result.messages[0].contains("%%%%"));
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
            .import_profiles_from_text(&mut config, &text, None)
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
        assert!(result.messages.is_empty());

        let profiles = database
            .profiles()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 7);

        let visible_profiles = ProfileManager::new(&database)
            .list_profiles(&config, None, None)
            .await
            .expect("subscription manager test operation should succeed");
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
            .import_profiles_from_text(&mut config, &text, None)
            .await
            .expect("subscription manager test operation should succeed");
        let second = manager
            .import_profiles_from_text(&mut config, &text, None)
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
    async fn subscription_import_updates_existing_manual_duplicate_globally() {
        let database = Database::connect_in_memory()
            .await
            .expect("subscription manager test operation should succeed");
        let manager = SubscriptionManager::new(&database);
        let mut config = AppConfig::default();
        config.gui_item.auto_create_subscription_group = false;
        let text = test_vless_link("same.example.test", "manual node");
        let manual = manager
            .import_profiles_from_text(&mut config, &text, None)
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
            .import_profiles_from_text(&mut config, &text, Some(&sub.id))
            .await
            .expect("subscription manager test operation should succeed");

        assert_eq!(result.imported, 1);
        assert_eq!(result.updated, 1);
        assert_eq!(result.imported_index_ids, manual.imported_index_ids);
        let profiles = database
            .profiles()
            .list()
            .await
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].index_id, manual.imported_index_ids[0]);
        assert_eq!(
            profiles[0].subscription_id.as_deref(),
            Some(sub.id.as_str())
        );
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
            .import_profiles_from_text(&mut config, &first_text, Some(&sub.id))
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
            .import_profiles_from_text(&mut config, &second_text, Some(&sub.id))
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
            .import_profiles_from_text(&mut config, text, None)
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
            .save_profile(&mut config, active_duplicate)
            .await
            .expect("subscription manager test operation should succeed");
        profile_manager
            .profile_ex()
            .set_sort(&original_index_id, 10)
            .await
            .expect("subscription manager test operation should succeed");
        profile_manager
            .profile_ex()
            .set_sort("active", 20)
            .await
            .expect("subscription manager test operation should succeed");
        profile_manager
            .profile_ex()
            .set_test_speed("active", 42.0)
            .await
            .expect("subscription manager test operation should succeed");
        config.index_id = "active".to_string();

        let result = manager
            .import_profiles_from_text(&mut config, text, None)
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
            .expect("subscription manager test operation should succeed");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].0.index_id, "active");
        assert_eq!(profiles[0].0.remarks, "Imported");
        assert_eq!(profiles[0].1.sort, 20);
        assert_eq!(profiles[0].1.speed, 42.0);
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
