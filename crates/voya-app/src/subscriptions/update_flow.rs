//! Prepare-then-commit machinery for subscription updates: the network fetch
//! snapshot (built outside any mutation lock), server-reported metadata
//! persistence. Manual group membership is independent of subscriptions.

use std::time::{SystemTime, UNIX_EPOCH};

use voya_core::{
    parse_profile_update_interval_minutes, parse_subscription_userinfo, SubItem, SubMetadataItem,
    SubscriptionUpdateResult, SubscriptionUserInfo,
};
use voya_db::DatabaseSession;
use voya_net::{
    DownloadError, FailedSubscriptionSource, SubscriptionClient, SubscriptionFetchOptions,
    SubscriptionFetchResult, SubscriptionFetchSource,
};

use crate::redaction::redact_urls;

use super::manager::{is_http_url, Result};

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
            sub_convert_url: None,
        };
        let options = SubscriptionFetchOptions {
            prefer_proxy,
            proxy_url: proxy_url.map(str::to_string),
        };
        match fetch_subscription(&client, &source, &options).await {
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
                result.skipped = result.skipped.saturating_add(1);
                result.messages.push(subscription_message(
                    &item.remarks,
                    &fetch_failure_message(&error),
                ));
            }
        }
    }

    Ok(PreparedSubscriptionUpdate { imports, result })
}

/// Records server-reported usage headers for a fetched subscription. The
/// `subscription-userinfo` values replace the stored figures only when the
/// header was present. `last_update_at` moves only when `imported` says the
/// fetch actually produced profiles, because the auto-update scheduler treats
/// it as "this subscription is current" and would otherwise stop retrying a
/// source that keeps returning junk. A server-suggested update interval is
/// adopted only while the user has not configured one.
pub(super) async fn persist_subscription_metadata(
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
