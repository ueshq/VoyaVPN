//! Prepare-then-commit machinery for subscription updates: the network fetch
//! snapshot (built outside any mutation lock), server-reported metadata
//! persistence, and the auto-group bootstrap that runs after a first import.

use std::time::{SystemTime, UNIX_EPOCH};

use voya_core::{
    parse_profile_update_interval_minutes, parse_subscription_userinfo, AppConfig, MultipleLoad,
    ProfileItem, ProfileProtocol, SubItem, SubMetadataItem, SubscriptionUpdateResult,
    SubscriptionUserInfo,
};
use voya_db::DatabaseSession;
use voya_net::{
    DownloadError, SubscriptionClient, SubscriptionFetchOptions, SubscriptionFetchResult,
    SubscriptionFetchSource,
};

use crate::groups::GroupManager;

use super::manager::{is_http_url, Result, SubscriptionManagerError};

#[derive(Debug)]
pub struct PreparedSubscriptionUpdate {
    pub(super) imports: Vec<PreparedSubscriptionImport>,
    pub(super) result: SubscriptionUpdateResult,
}

impl PreparedSubscriptionUpdate {
    #[must_use]
    pub fn has_imports(&self) -> bool {
        !self.imports.is_empty()
    }

    #[must_use]
    pub fn into_result(self) -> SubscriptionUpdateResult {
        self.result
    }
}

#[derive(Debug)]
pub(super) struct PreparedSubscriptionImport {
    pub(super) item: SubItem,
    pub(super) content: String,
    pub(super) user_info: Option<SubscriptionUserInfo>,
    pub(super) profile_title: Option<String>,
    pub(super) suggested_interval_minutes: Option<i32>,
    pub(super) fetched_at_unix: i64,
}

pub(super) async fn prepare_subscription_snapshot(
    config: &AppConfig,
    subscriptions: Vec<SubItem>,
    subscription_id: Option<&str>,
    prefer_proxy: bool,
    proxy_url: Option<&str>,
) -> Result<PreparedSubscriptionUpdate> {
    let client = SubscriptionClient::new();
    let mut result = SubscriptionUpdateResult::default();
    let mut imports = Vec::new();
    let subscription_id = subscription_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    for item in subscriptions {
        if subscription_id.is_some_and(|wanted| wanted != item.id) {
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

        let source = SubscriptionFetchSource {
            url: item.url.clone(),
            more_url: item.more_url.clone(),
            user_agent: item.user_agent.clone(),
            convert_target: item.convert_target.clone(),
            sub_convert_url: config.const_item.sub_convert_url.clone(),
        };
        let options = SubscriptionFetchOptions {
            prefer_proxy,
            proxy_url: proxy_url.map(str::to_string),
        };
        match fetch_subscription(&client, &source, &options).await {
            Ok(fetch) if !fetch.content.trim().is_empty() => {
                let headers = fetch
                    .downloads
                    .first()
                    .map(|download| download.headers.as_slice())
                    .unwrap_or_default();
                imports.push(PreparedSubscriptionImport {
                    user_info: header_value(headers, "subscription-userinfo")
                        .and_then(parse_subscription_userinfo),
                    profile_title: header_value(headers, "profile-title")
                        .map(str::trim)
                        .filter(|title| !title.is_empty())
                        .map(str::to_string),
                    suggested_interval_minutes: header_value(headers, "profile-update-interval")
                        .and_then(parse_profile_update_interval_minutes),
                    fetched_at_unix: unix_now_seconds(),
                    item,
                    content: fetch.content,
                });
            }
            Ok(_) => {
                result.skipped = result.skipped.saturating_add(1);
                result.messages.push(format!(
                    "{}->fetched empty subscription content",
                    item.remarks
                ));
            }
            Err(error) => {
                result.skipped = result.skipped.saturating_add(1);
                let message = if is_empty_download_error(&error) {
                    "fetched empty subscription content".to_string()
                } else {
                    error.to_string()
                };
                result.messages.push(format!("{}->{message}", item.remarks));
            }
        }
    }

    Ok(PreparedSubscriptionUpdate { imports, result })
}

/// Records server-reported usage headers for a fetched subscription. The
/// `subscription-userinfo` values replace the stored figures only when the
/// header was present; `last_update_at` always reflects the fetch. A
/// server-suggested update interval is adopted only while the user has not
/// configured one.
pub(super) async fn persist_subscription_metadata(
    database: DatabaseSession<'_>,
    prepared: &PreparedSubscriptionImport,
) -> Result<()> {
    let repository = database.subscription_metadata();
    let mut metadata =
        repository
            .get(&prepared.item.id)
            .await?
            .unwrap_or_else(|| SubMetadataItem {
                subscription_id: prepared.item.id.clone(),
                ..SubMetadataItem::default()
            });
    if let Some(user_info) = prepared.user_info {
        metadata.upload_bytes = user_info.upload_bytes;
        metadata.download_bytes = user_info.download_bytes;
        metadata.total_bytes = user_info.total_bytes;
        metadata.expire_at = user_info.expire_unix_seconds;
    }
    if let Some(title) = &prepared.profile_title {
        metadata.profile_title = Some(title.clone());
    }
    metadata.last_update_at = Some(prepared.fetched_at_unix);
    repository.upsert(&metadata).await?;

    if prepared.item.auto_update_interval_minutes.is_none() {
        if let Some(minutes) = prepared.suggested_interval_minutes {
            let mut item = prepared.item.clone();
            item.auto_update_interval_minutes = Some(minutes);
            database.subscriptions().upsert(&item).await?;
        }
    }

    Ok(())
}

/// Creates the delay-based "Auto" policy group for a subscription on its
/// first successful import. Children resolve dynamically from the
/// subscription at config-generation time, so the group is created once and
/// never refreshed; a user-modified group with the same source is left
/// untouched. Returns the group remarks when one was created.
pub(super) async fn ensure_subscription_auto_group(
    database: DatabaseSession<'_>,
    config: &mut AppConfig,
    subscription_id: &str,
    activate: bool,
) -> Result<Option<String>> {
    let profiles = database.profiles().list().await?;
    let already_exists = profiles.iter().any(|profile| {
        matches!(
            &profile.protocol,
            ProfileProtocol::PolicyGroup {
                source_subscription_id: Some(source),
                ..
            } if source == subscription_id
        )
    });
    if already_exists {
        return Ok(None);
    }
    let Some(subscription) = database.subscriptions().get(subscription_id).await? else {
        return Ok(None);
    };

    let remarks = format!("{} · Auto", subscription.remarks);
    let group = ProfileItem {
        remarks: remarks.clone(),
        protocol: ProfileProtocol::PolicyGroup {
            child_profile_ids: Vec::new(),
            source_subscription_id: Some(subscription_id.to_string()),
            filter: None,
            strategy: MultipleLoad::LeastPing,
        },
        ..ProfileItem::default()
    };
    let saved = GroupManager::from_session(database)
        .save_group_profile(config, group)
        .await
        .map_err(|error| SubscriptionManagerError::Group(Box::new(error)))?;
    if activate {
        config.index_id.clone_from(&saved.profile.index_id);
    }

    Ok(Some(remarks))
}

fn header_value<'headers>(
    headers: &'headers [(String, String)],
    name: &str,
) -> Option<&'headers str> {
    headers
        .iter()
        .find(|(header, _)| header == name)
        .map(|(_, value)| value.as_str())
}

fn unix_now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0))
}

async fn fetch_subscription(
    client: &SubscriptionClient,
    source: &SubscriptionFetchSource,
    options: &SubscriptionFetchOptions,
) -> std::result::Result<SubscriptionFetchResult, DownloadError> {
    #[cfg(not(test))]
    let result = client.fetch(source, options).await;
    #[cfg(test)]
    let result = client.fetch_allowing_local_for_tests(source, options).await;
    result
}

fn is_empty_download_error(error: &DownloadError) -> bool {
    match error {
        DownloadError::AttemptsFailed { attempts, .. } => {
            !attempts.is_empty()
                && attempts.iter().all(|attempt| {
                    attempt.bytes == 0 && attempt.error.as_deref() == Some("empty response")
                })
        }
        _ => false,
    }
}
