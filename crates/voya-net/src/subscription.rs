use base64::{engine::general_purpose::STANDARD, Engine as _};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::{
    is_denied_local_host, DownloadClient, DownloadError, DownloadRequest, DownloadResponse, Result,
    DEFAULT_TEXT_RESPONSE_LIMIT_BYTES,
};

pub const DEFAULT_SUB_CONVERT_URL: &str = "https://sub.xeton.dev/sub?url={0}";
pub const DEFAULT_SUB_CONVERT_CONFIG: &str =
    "https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/config/ACL4SSR_Online.ini";
const SUBSCRIPTION_RESPONSE_LIMIT_BYTES: usize = DEFAULT_TEXT_RESPONSE_LIMIT_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionFetchSource {
    pub url: String,
    pub more_url: String,
    pub user_agent: String,
    pub convert_target: Option<String>,
    pub sub_convert_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionFetchOptions {
    pub prefer_proxy: bool,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionFetchResult {
    pub content: String,
    pub downloads: Vec<DownloadResponse>,
    /// Extra `more_url` mirrors that could not be fetched, in source order.
    ///
    /// A dead mirror is reported rather than propagated: the primary list has already been
    /// downloaded and decoded at that point, and discarding it would block every update for the
    /// subscription until the user edited the entry.
    pub failed_more_urls: Vec<FailedSubscriptionSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedSubscriptionSource {
    pub url: String,
    pub error: String,
}

#[derive(Debug, Clone, Default)]
pub struct SubscriptionClient {
    download: DownloadClient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubscriptionUrlPolicy {
    DenyLocal,
    #[cfg(any(test, feature = "test-utils"))]
    AllowLocalForTests,
}

impl SubscriptionClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            download: DownloadClient::new(),
        }
    }

    pub async fn fetch(
        &self,
        source: &SubscriptionFetchSource,
        options: &SubscriptionFetchOptions,
    ) -> Result<SubscriptionFetchResult> {
        self.fetch_with_url_policy(source, options, SubscriptionUrlPolicy::DenyLocal)
            .await
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub async fn fetch_allowing_local_for_tests(
        &self,
        source: &SubscriptionFetchSource,
        options: &SubscriptionFetchOptions,
    ) -> Result<SubscriptionFetchResult> {
        self.fetch_with_url_policy(source, options, SubscriptionUrlPolicy::AllowLocalForTests)
            .await
    }

    async fn fetch_with_url_policy(
        &self,
        source: &SubscriptionFetchSource,
        options: &SubscriptionFetchOptions,
        url_policy: SubscriptionUrlPolicy,
    ) -> Result<SubscriptionFetchResult> {
        let raw_url = source.url.trim();
        validate_subscription_url(raw_url, url_policy)?;
        let main_url = build_subscription_url(
            raw_url,
            source.convert_target.as_deref(),
            source.sub_convert_url.as_deref(),
        )?;
        if main_url.trim() != raw_url {
            validate_subscription_url(&main_url, url_policy)?;
        }
        let mut downloads = Vec::new();
        let main = self
            .download
            .download_text(DownloadRequest {
                url: main_url,
                user_agent: nonempty(source.user_agent.clone()),
                prefer_proxy: options.prefer_proxy,
                proxy_url: options.proxy_url.clone(),
                response_body_limit: Some(SUBSCRIPTION_RESPONSE_LIMIT_BYTES),
            })
            .await?;
        let mut content = if source
            .convert_target
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            main.body.clone()
        } else {
            decode_base64_payload(&main.body).unwrap_or_else(|| main.body.clone())
        };
        downloads.push(main);

        let mut failed_more_urls = Vec::new();
        if source
            .convert_target
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Ok(SubscriptionFetchResult {
                content,
                downloads,
                failed_more_urls,
            });
        }

        for url in source
            .more_url
            .split(',')
            .map(str::trim)
            .filter(|url| !url.is_empty())
        {
            // The primary list is already downloaded and decoded here, so a rejected or dead
            // mirror is recorded and skipped instead of failing the whole subscription.
            let additional = match self.fetch_more_url(url, source, options, url_policy).await {
                Ok(additional) => additional,
                Err(error) => {
                    failed_more_urls.push(FailedSubscriptionSource {
                        url: url.to_string(),
                        error: error.to_string(),
                    });
                    continue;
                }
            };
            let body =
                decode_base64_payload(&additional.body).unwrap_or_else(|| additional.body.clone());
            if !body.is_empty() {
                if !content.ends_with('\n') && !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(&body);
            }
            downloads.push(additional);
        }

        Ok(SubscriptionFetchResult {
            content,
            downloads,
            failed_more_urls,
        })
    }

    async fn fetch_more_url(
        &self,
        url: &str,
        source: &SubscriptionFetchSource,
        options: &SubscriptionFetchOptions,
        url_policy: SubscriptionUrlPolicy,
    ) -> Result<DownloadResponse> {
        validate_subscription_url(url, url_policy)?;
        self.download
            .download_text(DownloadRequest {
                url: url.to_string(),
                user_agent: nonempty(source.user_agent.clone()),
                prefer_proxy: options.prefer_proxy,
                proxy_url: options.proxy_url.clone(),
                response_body_limit: Some(SUBSCRIPTION_RESPONSE_LIMIT_BYTES),
            })
            .await
    }
}

fn validate_subscription_url(url: &str, policy: SubscriptionUrlPolicy) -> Result<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(forbidden_subscription_url(trimmed, "URL is empty"));
    }

    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|source| forbidden_subscription_url(trimmed, format!("invalid URL: {source}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(forbidden_subscription_url(
            trimmed,
            "scheme must be http or https",
        ));
    }
    let Some(host) = parsed.host_str() else {
        return Err(forbidden_subscription_url(trimmed, "host is required"));
    };

    if policy == SubscriptionUrlPolicy::DenyLocal && is_denied_local_host(host) {
        return Err(forbidden_subscription_url(
            trimmed,
            "loopback and link-local hosts are not allowed",
        ));
    }

    Ok(())
}

fn forbidden_subscription_url(url: &str, reason: impl Into<String>) -> DownloadError {
    DownloadError::ForbiddenSubscriptionUrl {
        url: url.to_string(),
        reason: reason.into(),
    }
}

pub fn build_subscription_url(
    raw_url: &str,
    convert_target: Option<&str>,
    sub_convert_url: Option<&str>,
) -> Result<String> {
    let Some(target) = convert_target
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(raw_url.trim().to_string());
    };

    let template = sub_convert_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_SUB_CONVERT_URL);
    let has_source_placeholder = template.contains("{0}");
    let rendered = if has_source_placeholder {
        let encoded_url = utf8_percent_encode(raw_url.trim(), NON_ALPHANUMERIC).to_string();
        template.replace("{0}", &encoded_url)
    } else {
        template.to_string()
    };
    let mut url = reqwest::Url::parse(&rendered).map_err(|source| {
        forbidden_subscription_url(template, format!("invalid converter URL: {source}"))
    })?;
    let query_keys = url
        .query_pairs()
        .map(|(key, _)| key.into_owned())
        .collect::<Vec<_>>();
    let mut query = url.query_pairs_mut();

    if !has_source_placeholder && !query_keys.iter().any(|key| key == "url") {
        query.append_pair("url", raw_url.trim());
    }
    if !query_keys.iter().any(|key| key == "target") {
        query.append_pair("target", target);
    }
    if !query_keys.iter().any(|key| key == "config") {
        query.append_pair("config", DEFAULT_SUB_CONVERT_CONFIG);
    }
    drop(query);

    Ok(url.into())
}

#[must_use]
pub fn decode_base64_payload(input: &str) -> Option<String> {
    let mut normalized = input
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '_' | '-' | '='))
    {
        return None;
    }

    normalized = normalized.replace('_', "/").replace('-', "+");
    if normalized.len() % 4 != 0 {
        normalized.extend(std::iter::repeat_n('=', 4 - normalized.len() % 4));
    }

    let bytes = STANDARD.decode(normalized.as_bytes()).ok()?;
    let decoded = String::from_utf8(bytes).ok()?;
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn nonempty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use tokio::sync::Mutex;

    use super::*;
    use crate::download::test_support::spawn_http_fixture;

    #[test]
    fn subscription_url_guard_rejects_loopback_and_link_local() {
        for url in [
            "http://127.0.0.1/sub",
            "https://localhost/sub",
            "http://169.254.1.10/sub",
            "https://[::1]/sub",
            "http://[fe80::1]/sub",
            "http://0.0.0.0/sub",
            "http://[::]/sub",
            "http://[::ffff:127.0.0.1]/sub",
            "http://[::ffff:169.254.1.1]/sub",
            "http://[::127.0.0.1]/sub",
        ] {
            let error = validate_subscription_url(url, SubscriptionUrlPolicy::DenyLocal)
                .expect_err("local subscription URL should fail");
            assert!(
                matches!(error, DownloadError::ForbiddenSubscriptionUrl { .. }),
                "{error:?}"
            );
        }

        validate_subscription_url("https://example.com/sub", SubscriptionUrlPolicy::DenyLocal)
            .expect("public HTTPS subscription URL");
        validate_subscription_url("http://192.168.1.10/sub", SubscriptionUrlPolicy::DenyLocal)
            .expect("private non-loopback subscription URL remains allowed");
        validate_subscription_url(
            "http://[::ffff:1.1.1.1]/sub",
            SubscriptionUrlPolicy::DenyLocal,
        )
        .expect("IPv4-mapped public subscription URL remains allowed");
    }

    #[tokio::test]
    async fn subscription_fetch_rejects_loopback_main_url() {
        let error = SubscriptionClient::new()
            .fetch(
                &SubscriptionFetchSource {
                    url: "http://127.0.0.1/sub".to_string(),
                    more_url: String::new(),
                    user_agent: String::new(),
                    convert_target: None,
                    sub_convert_url: None,
                },
                &SubscriptionFetchOptions {
                    prefer_proxy: false,
                    proxy_url: None,
                },
            )
            .await
            .expect_err("loopback subscription URL should fail");

        assert!(
            matches!(error, DownloadError::ForbiddenSubscriptionUrl { .. }),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn subscription_fetch_decodes_base64_and_merges_more_urls() {
        let main = STANDARD.encode("vless://id-a@example.test:443#A");
        let extra = STANDARD.encode("trojan://secret@example.test:443#B");
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([("/main".to_string(), main), ("/extra".to_string(), extra)]),
            2,
            Arc::clone(&seen_user_agents),
        )
        .await;

        let result = SubscriptionClient::new()
            .fetch_with_url_policy(
                &SubscriptionFetchSource {
                    url: format!("{base}/main"),
                    more_url: format!("{base}/extra"),
                    user_agent: "SubUA/2".to_string(),
                    convert_target: None,
                    sub_convert_url: None,
                },
                &SubscriptionFetchOptions {
                    prefer_proxy: false,
                    proxy_url: None,
                },
                SubscriptionUrlPolicy::AllowLocalForTests,
            )
            .await
            .expect("subscription content");

        assert_eq!(
            result.content,
            "vless://id-a@example.test:443#A\ntrojan://secret@example.test:443#B"
        );
        assert_eq!(result.downloads.len(), 2);
        assert_eq!(
            seen_user_agents.lock().await.as_slice(),
            ["SubUA/2", "SubUA/2"]
        );
    }

    /// One dead mirror must not cost the user every update for the subscription: the primary
    /// list is already downloaded by then, so the failure is reported alongside it.
    #[tokio::test]
    async fn subscription_fetch_keeps_primary_content_when_a_more_url_fails() {
        let main = STANDARD.encode("vless://id-a@example.test:443#A");
        let good = STANDARD.encode("trojan://secret@example.test:443#B");
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([("/main".to_string(), main), ("/good".to_string(), good)]),
            3,
            Arc::clone(&seen_user_agents),
        )
        .await;

        let result = SubscriptionClient::new()
            .fetch_with_url_policy(
                &SubscriptionFetchSource {
                    url: format!("{base}/main"),
                    more_url: format!("{base}/missing, {base}/good"),
                    user_agent: String::new(),
                    convert_target: None,
                    sub_convert_url: None,
                },
                &SubscriptionFetchOptions {
                    prefer_proxy: false,
                    proxy_url: None,
                },
                SubscriptionUrlPolicy::AllowLocalForTests,
            )
            .await
            .expect("a failing more_url should not discard the primary content");

        assert_eq!(
            result.content,
            "vless://id-a@example.test:443#A\ntrojan://secret@example.test:443#B"
        );
        assert_eq!(result.downloads.len(), 2);
        assert_eq!(result.failed_more_urls.len(), 1);
        assert_eq!(result.failed_more_urls[0].url, format!("{base}/missing"));
    }

    #[tokio::test]
    async fn conversion_target_rewrites_main_url_and_skips_more_urls() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([(
                "/convert".to_string(),
                "mixed-converted-subscription".to_string(),
            )]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let source_url = format!("{base}/raw-sub");

        let result = SubscriptionClient::new()
            .fetch_with_url_policy(
                &SubscriptionFetchSource {
                    url: source_url.clone(),
                    more_url: format!("{base}/should-not-fetch"),
                    user_agent: String::new(),
                    convert_target: Some("clash".to_string()),
                    sub_convert_url: Some(format!("{base}/convert?url={{0}}")),
                },
                &SubscriptionFetchOptions {
                    prefer_proxy: false,
                    proxy_url: None,
                },
                SubscriptionUrlPolicy::AllowLocalForTests,
            )
            .await
            .expect("converted subscription content");

        assert_eq!(result.content, "mixed-converted-subscription");
        assert_eq!(result.downloads.len(), 1);

        let rewritten = build_subscription_url(
            &source_url,
            Some("clash"),
            Some(&format!("{base}/convert?url={{0}}")),
        )
        .expect("subscription conversion URL");
        assert!(rewritten.contains("/convert?url=http%3A%2F%2F127%2E0%2E0%2E1"));
        assert!(rewritten.contains("&target=clash"));
        assert!(rewritten.contains("&config="));
    }

    #[test]
    fn conversion_url_without_placeholder_uses_query_parameters() {
        let rewritten = build_subscription_url(
            "https://source.example/sub?id=one",
            Some("clash&config=override"),
            Some("https://converter.example/sub"),
        )
        .expect("subscription conversion URL");
        let parsed = reqwest::Url::parse(&rewritten).expect("converted URL should parse");
        let query = parsed.query_pairs().collect::<HashMap<_, _>>();

        assert_eq!(
            query.get("url").map(AsRef::as_ref),
            Some("https://source.example/sub?id=one")
        );
        assert_eq!(
            query.get("target").map(AsRef::as_ref),
            Some("clash&config=override")
        );
        assert_eq!(
            query.get("config").map(AsRef::as_ref),
            Some(DEFAULT_SUB_CONVERT_CONFIG)
        );
    }
}
