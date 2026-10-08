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
    /// Downloads through `proxy_url` first when there is one, then directly.
    pub async fn prepare_subscription_update(
        &self,
        subscription_id: Option<&str>,
        proxy_url: Option<&str>,
    ) -> Result<PreparedSubscriptionUpdate> {
        let subscriptions = self.database.subscriptions().list().await?;
        prepare_subscription_snapshot(subscriptions, subscription_id, proxy_url).await
    }

    /// [`Self::prepare_subscription_update`] for one subscription the caller
    /// has already read. The scheduler lists the table once per pass; asking
    /// by id made it list the whole table again for every subscription due.
    /// A row edited or deleted since is caught where the update is applied.
    pub(crate) async fn prepare_update_of(
        item: SubItem,
        proxy_url: Option<&str>,
    ) -> Result<PreparedSubscriptionUpdate> {
        prepare_subscription_snapshot(vec![item], None, proxy_url).await
    }

    /// Persists the fetch failures of a prepared update that will not be
    /// applied because nothing came back to import, so a restart does not
    /// forget the subscription ever failed. Returns how many were written.
    pub(crate) async fn persist_prepared_failures(
        &self,
        prepared: &PreparedSubscriptionUpdate,
    ) -> Result<usize> {
        for attempt in &prepared.failed_attempts {
            persist_failed_attempt(self.database, attempt).await?;
        }
        Ok(prepared.failed_attempts.len())
    }

    pub async fn apply_prepared_subscription_update(
        &self,
        config: &mut AppConfig,
        prepared: PreparedSubscriptionUpdate,
    ) -> Result<SubscriptionUpdateResult> {
        // Failures recorded while fetching (a dead URL, an empty body) have no
        // import to carry them, so their outcome is persisted up front —
        // otherwise a restart would forget the subscription ever failed.
        self.persist_prepared_failures(&prepared).await?;
        let PreparedSubscriptionUpdate {
            imports,
            mut result,
            ..
        } = prepared;
        for prepared_import in imports {
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
                    persist_subscription_metadata(self.database, &prepared_import, None).await?;
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
                    self.skip_import(
                        &mut result,
                        &prepared_import,
                        SubscriptionUpdateReason::NoImportableNodes,
                        "no nodes were imported",
                    )
                    .await?;
                }
                Err(SubscriptionManagerError::NoImportableProfiles) => {
                    self.skip_import(
                        &mut result,
                        &prepared_import,
                        SubscriptionUpdateReason::NoImportableNodes,
                        "no importable nodes were found",
                    )
                    .await?;
                }
                // A bad filter belongs to one subscription; failing the whole
                // batch would roll back every sibling's successful import.
                Err(SubscriptionManagerError::InvalidFilter(reason)) => {
                    self.skip_import(
                        &mut result,
                        &prepared_import,
                        SubscriptionUpdateReason::InvalidFilter,
                        &format!("subscription filter is invalid: {reason}"),
                    )
                    .await?;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(result)
    }

    /// Records a subscription that was fetched but left nothing to import: a
    /// failed outcome, the stored message, and a line in the summary.
    async fn skip_import(
        &self,
        result: &mut SubscriptionUpdateResult,
        import: &PreparedSubscriptionImport,
        reason: SubscriptionUpdateReason,
        message: &str,
    ) -> Result<()> {
        record_outcome(
            result,
            &import.item.id,
            SubscriptionUpdateStatus::Failed,
            reason,
            0,
            0,
        );
        persist_subscription_metadata(self.database, import, Some(message)).await?;
        result.skipped = result.skipped.saturating_add(1);
        result
            .messages
            .push(format!("{}->{message}", import.item.remarks));

        Ok(())
    }
}

#[derive(Debug)]
pub struct PreparedSubscriptionUpdate {
    imports: Vec<PreparedSubscriptionImport>,
    result: SubscriptionUpdateResult,
    failed_attempts: Vec<FailedSubscriptionAttempt>,
}

/// A subscription whose update already failed while fetching, before any
/// import could be prepared for it.
#[derive(Debug)]
pub struct FailedSubscriptionAttempt {
    pub subscription_id: String,
    pub attempted_at_unix: i64,
    pub error: String,
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
    proxy_url: Option<&str>,
) -> Result<PreparedSubscriptionUpdate> {
    let client = SubscriptionClient::new();
    let mut result = SubscriptionUpdateResult::default();
    let mut imports = Vec::new();
    let mut failed_attempts = Vec::new();
    let subscription_id = subscription_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let options = SubscriptionFetchOptions {
        proxy_url: proxy_url.map(str::to_string),
    };

    let mut fetchable = Vec::new();
    for item in subscriptions {
        if subscription_id.is_some_and(|wanted| wanted != item.id) {
            continue;
        }
        if item.id.trim().is_empty() || item.url.trim().is_empty() || !is_http_url(&item.url) {
            record_failed_fetch(
                &mut result,
                &mut failed_attempts,
                &item,
                SubscriptionUpdateReason::InvalidSource,
                "subscription URL is empty or not an http(s) link",
                None,
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
                record_failed_fetch(
                    &mut result,
                    &mut failed_attempts,
                    &item,
                    SubscriptionUpdateReason::EmptyContent,
                    EMPTY_FETCH_MESSAGE,
                    None,
                );
                // The warnings precede the outcome note because
                // `unusable_update_message` reports the last message as the
                // reason this subscription failed, and a dead mirror is not it.
                push_failed_more_url_warnings(&mut result, &item.remarks, &fetch.failed_more_urls);
                result
                    .messages
                    .push(subscription_message(&item.remarks, EMPTY_FETCH_MESSAGE));
            }
            Err(error) => {
                let diagnostic = redact_urls(&fetch_failure_message(&error));
                record_failed_fetch(
                    &mut result,
                    &mut failed_attempts,
                    &item,
                    if error.is_empty_response() {
                        SubscriptionUpdateReason::EmptyContent
                    } else {
                        SubscriptionUpdateReason::DownloadFailed
                    },
                    &diagnostic,
                    Some(diagnostic.clone()),
                );
                result
                    .messages
                    .push(subscription_message(&item.remarks, &diagnostic));
            }
        }
    }

    Ok(PreparedSubscriptionUpdate {
        imports,
        result,
        failed_attempts,
    })
}

/// The bookkeeping every subscription that brought nothing shares: a failed
/// outcome, the attempt to persist against it, one more skipped, and a line in
/// the app's logs, which otherwise had nothing to show for a failed update.
///
/// `error` is already free of URLs; the subscription's own URL carries its
/// access token in the query, which the log layers' userinfo redaction keeps.
fn record_failed_fetch(
    result: &mut SubscriptionUpdateResult,
    failed_attempts: &mut Vec<FailedSubscriptionAttempt>,
    item: &SubItem,
    reason: SubscriptionUpdateReason,
    error: &str,
    diagnostic: Option<String>,
) {
    tracing::warn!(
        subscription = %redact_urls(&item.remarks),
        error = %error,
        "subscription update failed"
    );
    let id = item.id.as_str();
    result.skipped = result.skipped.saturating_add(1);
    result.outcomes.push(SubscriptionUpdateOutcome {
        subscription_id: id.to_string(),
        status: SubscriptionUpdateStatus::Failed,
        reason,
        imported: 0,
        removed_existing: 0,
        diagnostic,
    });
    failed_attempts.push(FailedSubscriptionAttempt {
        subscription_id: id.to_string(),
        attempted_at_unix: unix_now_seconds(),
        error: error.to_string(),
    });
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

/// Records server-reported usage headers for a fetched subscription, plus the
/// outcome of this attempt. The `subscription-userinfo` values replace the
/// stored figures only when the header was present. `last_update_at` moves
/// only when the attempt succeeded, because the auto-update scheduler treats
/// it as "this subscription is current" and would otherwise stop retrying a
/// source that keeps returning junk. A server-suggested update interval is
/// adopted only while the user has not configured one.
async fn persist_subscription_metadata(
    database: DatabaseSession<'_>,
    prepared: &PreparedSubscriptionImport,
    attempt_error: Option<&str>,
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
    metadata.last_attempt_at = Some(prepared.fetched_at_unix);
    metadata.last_attempt_failed = Some(attempt_error.is_some());
    metadata.last_attempt_error = attempt_error.map(str::to_string);
    if attempt_error.is_none() {
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

/// Records a fetch that failed before anything could be imported, so the
/// failure survives a restart and the list can badge the subscription.
async fn persist_failed_attempt(
    database: DatabaseSession<'_>,
    attempt: &FailedSubscriptionAttempt,
) -> Result<()> {
    let repository = database.subscription_metadata();
    let mut metadata = repository
        .get(&attempt.subscription_id)
        .await?
        .unwrap_or_else(|| SubMetadataItem {
            subscription_id: attempt.subscription_id.clone(),
            ..SubMetadataItem::default()
        });
    metadata.last_attempt_at = Some(attempt.attempted_at_unix);
    metadata.last_attempt_failed = Some(true);
    metadata.last_attempt_error = Some(attempt.error.clone());
    repository.upsert(&metadata).await?;

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

    /// A failed update reaches the logs page, named by the subscription and
    /// with the redacted diagnostic only: the URL's query holds the token.
    #[test]
    fn a_failed_fetch_is_logged_without_the_subscription_url() {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::layer::SubscriberExt as _;
        use voya_contracts::{LogLevel, LogLineBody};

        use crate::logging::LogPanelLayer;

        let queued: Arc<Mutex<Vec<(LogLevel, LogLineBody)>>> = Arc::default();
        let subscriber = tracing_subscriber::registry().with(LogPanelLayer::new({
            let queued = Arc::clone(&queued);
            move |level, body| queued.lock().expect("queue lock").push((level, body))
        }));
        let item = SubItem {
            id: "sub-1".to_string(),
            remarks: "MySub".to_string(),
            url: "https://sub.example.test/link?token=abc123".to_string(),
            ..SubItem::default()
        };
        let diagnostic = redact_urls(
            "download failed for https://sub.example.test/link?token=abc123: timed out",
        );
        let mut result = SubscriptionUpdateResult::default();
        let mut failed_attempts = Vec::new();

        tracing::subscriber::with_default(subscriber, || {
            record_failed_fetch(
                &mut result,
                &mut failed_attempts,
                &item,
                SubscriptionUpdateReason::DownloadFailed,
                &diagnostic,
                Some(diagnostic.clone()),
            );
        });

        let queued = queued.lock().expect("queue lock");
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].0, LogLevel::Warn);
        let LogLineBody::Diagnostic { line } = &queued[0].1 else {
            panic!("expected a diagnostic line, got {:?}", queued[0].1);
        };
        assert!(line.contains("subscription update failed"), "{line}");
        assert!(line.contains("MySub"), "{line}");
        assert!(!line.contains("token=abc123"), "{line}");
        assert!(!line.contains("sub.example.test"), "{line}");
        assert_eq!(failed_attempts.len(), 1);
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
