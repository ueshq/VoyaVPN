//! Subscription updates, prepared then committed: every source is fetched
//! outside the configuration lock, and only a snapshot whose subscription is
//! unchanged when the lock is taken is imported. Server-reported metadata is
//! persisted beside the import. Manual group membership is independent of
//! subscriptions.

use futures_util::{stream, StreamExt};
use voya_contracts::{
    SubscriptionUpdateOutcome, SubscriptionUpdateReason, SubscriptionUpdateResult,
    SubscriptionUpdateStatus,
};
use voya_core::{
    parse_profile_update_interval_minutes, parse_subscription_userinfo, AppConfig, SubItem,
    SubMetadataItem, SubscriptionUserInfo,
};
use voya_db::DatabaseSession;
use voya_net::{
    DownloadError, FailedSubscriptionSource, SubscriptionClient, SubscriptionFetchOptions,
    SubscriptionFetchResult, SubscriptionFetchSource,
};

use crate::redaction::redact_urls;
use crate::subscriptions::unix_now_seconds;

use super::{is_http_url, Result, SubscriptionManager, SubscriptionManagerError};

impl SubscriptionManager<'_> {
    pub async fn prepare_subscription_update(
        &self,
        subscription_id: Option<&str>,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
    ) -> Result<PreparedSubscriptionUpdate> {
        let subscriptions = self.database.subscriptions().list().await?;
        prepare_subscription_snapshot(subscriptions, subscription_id, prefer_proxy, proxy_url).await
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
                record_outcome(
                    &mut result,
                    &prepared_import.item.id,
                    SubscriptionUpdateStatus::Skipped,
                    SubscriptionUpdateReason::SourceChanged,
                    0,
                    0,
                );
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
                .import_subscription_content(
                    config,
                    &prepared_import.content,
                    Some(&prepared_import.item.id),
                )
                .await;
            match import {
                Ok(import) if import.imported > 0 => {
                    record_outcome(
                        &mut result,
                        &prepared_import.item.id,
                        SubscriptionUpdateStatus::Success,
                        SubscriptionUpdateReason::Updated,
                        import.imported,
                        import.removed_existing,
                    );
                    persist_subscription_metadata(self.database, &prepared_import, true).await?;
                    result.updated = result.updated.saturating_add(1);
                    result.imported = result.imported.saturating_add(import.imported);
                    result.removed_existing = result
                        .removed_existing
                        .saturating_add(import.removed_existing);
                    result.messages.push(format!(
                        "{}->imported {} nodes",
                        prepared_import.item.remarks, import.imported
                    ));
                }
                Ok(_) => {
                    record_outcome(
                        &mut result,
                        &prepared_import.item.id,
                        SubscriptionUpdateStatus::Failed,
                        SubscriptionUpdateReason::NoImportableNodes,
                        0,
                        0,
                    );
                    persist_subscription_metadata(self.database, &prepared_import, false).await?;
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!(
                        "{}->no nodes were imported",
                        prepared_import.item.remarks
                    ));
                }
                Err(SubscriptionManagerError::NoImportableProfiles) => {
                    record_outcome(
                        &mut result,
                        &prepared_import.item.id,
                        SubscriptionUpdateStatus::Failed,
                        SubscriptionUpdateReason::NoImportableNodes,
                        0,
                        0,
                    );
                    persist_subscription_metadata(self.database, &prepared_import, false).await?;
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!(
                        "{}->no importable nodes were found",
                        prepared_import.item.remarks
                    ));
                }
                // A bad filter belongs to one subscription; failing the whole
                // batch would roll back every sibling's successful import.
                Err(SubscriptionManagerError::InvalidFilter(reason)) => {
                    record_outcome(
                        &mut result,
                        &prepared_import.item.id,
                        SubscriptionUpdateStatus::Failed,
                        SubscriptionUpdateReason::InvalidFilter,
                        0,
                        0,
                    );
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
}

#[derive(Debug)]
pub struct PreparedSubscriptionUpdate {
    imports: Vec<PreparedSubscriptionImport>,
    result: SubscriptionUpdateResult,
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
struct PreparedSubscriptionImport {
    item: SubItem,
    content: String,
    user_info: Option<SubscriptionUserInfo>,
    profile_title: Option<String>,
    suggested_interval_minutes: Option<i32>,
    fetched_at_unix: i64,
}

/// Subscriptions fetched at once. Each fetch is network-bound and can take up
/// to the full request timeout, so one at a time made an update of several
/// subscriptions wait on each slow server in turn.
const SUBSCRIPTION_FETCH_CONCURRENCY: usize = 4;

async fn prepare_subscription_snapshot(
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
    let options = SubscriptionFetchOptions {
        prefer_proxy,
        proxy_url: proxy_url.map(str::to_string),
    };

    let mut fetchable = Vec::new();
    for item in subscriptions {
        if subscription_id.is_some_and(|wanted| wanted != item.id) {
            continue;
        }
        if item.id.trim().is_empty() || item.url.trim().is_empty() || !is_http_url(&item.url) {
            result.skipped = result.skipped.saturating_add(1);
            record_outcome(
                &mut result,
                &item.id,
                SubscriptionUpdateStatus::Failed,
                SubscriptionUpdateReason::InvalidSource,
                0,
                0,
            );
            continue;
        }
        // `enabled` only switches automatic updates, which the scheduler checks
        // itself; an update the user asks for always runs.
        fetchable.push(item);
    }

    // `buffered`, not `buffer_unordered`: results are handled in subscription
    // order, so messages and imports read the same as a sequential update.
    let (client, options) = (&client, &options);
    let mut fetches = stream::iter(fetchable.into_iter().map(|item| async move {
        let source = SubscriptionFetchSource {
            url: item.url.clone(),
            more_url: item.more_url.clone(),
            user_agent: item.user_agent.clone(),
            convert_target: item.convert_target.clone(),
        };
        let fetched = fetch_subscription(client, &source, options).await;
        (item, fetched)
    }))
    .buffered(SUBSCRIPTION_FETCH_CONCURRENCY);

    while let Some((item, fetched)) = fetches.next().await {
        match fetched {
            Ok(fetch) if !fetch.content.trim().is_empty() => {
                push_failed_more_url_warnings(&mut result, &item.remarks, &fetch.failed_more_urls);
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
            Ok(fetch) => {
                record_outcome(
                    &mut result,
                    &item.id,
                    SubscriptionUpdateStatus::Failed,
                    SubscriptionUpdateReason::EmptyContent,
                    0,
                    0,
                );
                result.skipped = result.skipped.saturating_add(1);
                // The warnings precede the outcome note because
                // `unusable_update_message` reports the last message as the
                // reason this subscription failed, and a dead mirror is not it.
                push_failed_more_url_warnings(&mut result, &item.remarks, &fetch.failed_more_urls);
                result
                    .messages
                    .push(subscription_message(&item.remarks, EMPTY_FETCH_MESSAGE));
            }
            Err(error) => {
                record_outcome(
                    &mut result,
                    &item.id,
                    SubscriptionUpdateStatus::Failed,
                    if error.is_empty_response() {
                        SubscriptionUpdateReason::EmptyContent
                    } else {
                        SubscriptionUpdateReason::DownloadFailed
                    },
                    0,
                    0,
                );
                result.skipped = result.skipped.saturating_add(1);
                let diagnostic = redact_urls(&fetch_failure_message(&error));
                if let Some(outcome) = result.outcomes.last_mut() {
                    outcome.diagnostic = Some(diagnostic.clone());
                }
                result
                    .messages
                    .push(subscription_message(&item.remarks, &diagnostic));
            }
        }
    }

    Ok(PreparedSubscriptionUpdate { imports, result })
}

fn record_outcome(
    result: &mut SubscriptionUpdateResult,
    id: &str,
    status: SubscriptionUpdateStatus,
    reason: SubscriptionUpdateReason,
    imported: u32,
    removed_existing: u32,
) {
    result.outcomes.push(SubscriptionUpdateOutcome {
        subscription_id: id.to_string(),
        status,
        reason,
        imported,
        removed_existing,
        diagnostic: None,
    });
}

/// Records server-reported usage headers for a fetched subscription. The
/// `subscription-userinfo` values replace the stored figures only when the
/// header was present. `last_update_at` moves only when `imported` says the
/// fetch actually produced profiles, because the auto-update scheduler treats
/// it as "this subscription is current" and would otherwise stop retrying a
/// source that keeps returning junk. A server-suggested update interval is
/// adopted only while the user has not configured one.
async fn persist_subscription_metadata(
    database: DatabaseSession<'_>,
    prepared: &PreparedSubscriptionImport,
    imported: bool,
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
    if imported {
        metadata.last_update_at = Some(prepared.fetched_at_unix);
    }
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

fn header_value<'headers>(
    headers: &'headers [(String, String)],
    name: &str,
) -> Option<&'headers str> {
    headers
        .iter()
        .find(|(header, _)| header == name)
        .map(|(_, value)| value.as_str())
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

const EMPTY_FETCH_MESSAGE: &str = "fetched empty subscription content";

/// Prefixes a per-subscription update message with its remarks and strips any
/// URL it carries.
///
/// `DownloadError` embeds the subscription URL in every network-failure variant
/// (and repeats it inside the `Debug`-formatted attempt list), while these
/// messages end up in `AutoUpdateOutcome::error`, which the shell shows as a
/// toast and appends to the Logs panel. Subscription links carry the account
/// token, so the URL never survives into the message.
fn subscription_message(remarks: &str, message: &str) -> String {
    format!("{remarks}->{}", redact_urls(message))
}

/// Turns a failed fetch into the note shown for the subscription.
///
/// "The server answered with nothing" is a product state (an expired or emptied
/// plan), not a transport failure, so it gets the same wording as a successful
/// but empty body. The distinction comes from `DownloadError::is_empty_response`
/// so the two crates cannot drift apart over the wording of an attempt error.
fn fetch_failure_message(error: &DownloadError) -> String {
    if error.is_empty_response() {
        EMPTY_FETCH_MESSAGE.to_string()
    } else {
        error.to_string()
    }
}

/// Reports every `more_url` mirror that could not be fetched.
///
/// The counters are deliberately untouched: the primary list decides whether the
/// subscription updated, so a dead mirror is a warning the user can act on
/// rather than a failure that would mark a successful import as skipped.
fn push_failed_more_url_warnings(
    result: &mut SubscriptionUpdateResult,
    remarks: &str,
    failed: &[FailedSubscriptionSource],
) {
    for failure in failed {
        result.messages.push(subscription_message(
            remarks,
            &format!("additional subscription URL failed: {}", failure.error),
        ));
    }
}

#[cfg(test)]
mod tests {
    use voya_net::{DownloadAttempt, EMPTY_RESPONSE_ATTEMPT_ERROR};

    use super::*;

    #[test]
    fn fetch_failure_messages_never_carry_the_subscription_url() {
        let message = subscription_message(
            "MySub",
            "download failed for https://sub.example.test/link?token=abc123: timed out",
        );

        assert!(!message.contains("token=abc123"));
        assert!(!message.contains("sub.example.test"));
        assert!(message.starts_with("MySub->download failed for [redacted URL]"));
    }

    #[test]
    fn fetch_failure_messages_keep_the_subscription_name() {
        assert_eq!(
            subscription_message("MySub", EMPTY_FETCH_MESSAGE),
            "MySub->fetched empty subscription content"
        );
    }

    /// Pins the empty-body classification to `voya-net`'s marker instead of a
    /// copy of its text, so rewording the attempt error cannot silently turn an
    /// emptied subscription back into a raw transport-failure toast.
    #[test]
    fn an_all_empty_attempt_list_is_reported_as_empty_content() {
        let empty = DownloadError::AttemptsFailed {
            url: "https://sub.example.test/link".to_string(),
            attempts: vec![DownloadAttempt {
                url: "https://sub.example.test/link".to_string(),
                via_proxy: false,
                bytes: 0,
                error: Some(EMPTY_RESPONSE_ATTEMPT_ERROR.to_string()),
            }],
        };
        assert_eq!(fetch_failure_message(&empty), EMPTY_FETCH_MESSAGE);

        let refused = DownloadError::AttemptsFailed {
            url: "https://sub.example.test/link".to_string(),
            attempts: vec![DownloadAttempt {
                url: "https://sub.example.test/link".to_string(),
                via_proxy: false,
                bytes: 0,
                error: Some("connection refused".to_string()),
            }],
        };
        assert_ne!(fetch_failure_message(&refused), EMPTY_FETCH_MESSAGE);
        assert!(fetch_failure_message(&refused).contains("connection refused"));
    }

    #[test]
    fn failed_more_url_mirrors_are_reported_as_warnings() {
        let mut result = SubscriptionUpdateResult::default();

        push_failed_more_url_warnings(
            &mut result,
            "MySub",
            &[FailedSubscriptionSource {
                url: "https://mirror.example.test/link?token=abc123".to_string(),
                error:
                    "download failed for https://mirror.example.test/link?token=abc123: timed out"
                        .to_string(),
            }],
        );

        assert_eq!(result.messages.len(), 1);
        let message = &result.messages[0];
        assert!(
            message.starts_with("MySub->additional subscription URL failed:"),
            "{message}"
        );
        assert!(
            message.contains(crate::redaction::REDACTED_URL),
            "{message}"
        );
        assert!(!message.contains("token=abc123"), "{message}");
        assert_eq!(
            (result.skipped, result.updated, result.imported),
            (0, 0, 0),
            "a dead mirror is a warning, not a failed subscription"
        );
    }
}
