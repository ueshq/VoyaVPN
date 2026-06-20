//! Network service clients for downloads, updates, subscriptions, Clash API,
//! WebDAV, geo assets, and rulesets.

pub mod clash;
pub mod ruleset;
pub mod update;
pub mod webdav;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::{Client, Proxy};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error;

/// Shared user agent prefix for network clients.
pub const USER_AGENT_PREFIX: &str = "VoyaVPN";
pub const DEFAULT_SUB_CONVERT_URL: &str = "https://sub.xeton.dev/sub?url={0}";
pub const DEFAULT_SUB_CONVERT_CONFIG: &str =
    "https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/config/ACL4SSR_Online.ini";
pub const RUSSIA_GEO_SOURCE_URL: &str =
    "https://github.com/runetfreedom/russia-v2ray-rules-dat/releases/latest/download/{0}.dat";
pub const IRAN_GEO_SOURCE_URL: &str =
    "https://github.com/Chocolate4U/Iran-v2ray-rules/releases/latest/download/{0}.dat";
pub const RUSSIA_SRS_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-rules-dat/release/sing-box/rule-set-{0}/{1}.srs";
pub const IRAN_SRS_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/chocolate4u/Iran-sing-box-rules/rule-set/{1}.srs";
pub const RUSSIA_ROUTING_RULES_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-custom-routing-list/main/v2rayN/template.json";
pub const IRAN_ROUTING_RULES_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/Chocolate4U/Iran-v2ray-rules/main/v2rayN/template.json";
pub const RUSSIA_DNS_TEMPLATE_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-custom-routing-list/main/v2rayN/";
pub const IRAN_DNS_TEMPLATE_SOURCE_URL: &str =
    "https://raw.githubusercontent.com/Chocolate4U/Iran-v2ray-rules/main/v2rayN/";

pub(crate) const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_TEXT_RESPONSE_LIMIT_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_BINARY_RESPONSE_LIMIT_BYTES: usize = 512 * 1024 * 1024;
const SUBSCRIPTION_RESPONSE_LIMIT_BYTES: usize = DEFAULT_TEXT_RESPONSE_LIMIT_BYTES;
const PRESET_DNS_TEMPLATE_RESPONSE_LIMIT_BYTES: usize = 1024 * 1024;

pub type Result<T> = std::result::Result<T, DownloadError>;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("failed to build download HTTP client for {url}: {reason}")]
    ClientBuild { url: String, reason: String },
    #[error("download failed for {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error(
        "download response for {url} exceeds {limit} bytes (content length: {content_length:?}, received: {received})"
    )]
    ResponseTooLarge {
        url: String,
        limit: usize,
        content_length: Option<u64>,
        received: usize,
    },
    #[error("all download attempts failed for {url}: {attempts:?}")]
    AttemptsFailed {
        url: String,
        attempts: Vec<DownloadAttempt>,
    },
    #[error("failed to write download to {path}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("download target {path} has no parent directory")]
    MissingParent { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadAttempt {
    pub url: String,
    pub via_proxy: bool,
    pub bytes: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRequest {
    pub url: String,
    pub user_agent: Option<String>,
    pub prefer_proxy: bool,
    pub proxy_url: Option<String>,
    pub response_body_limit: Option<usize>,
}

impl DownloadRequest {
    #[must_use]
    pub fn direct(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            user_agent: None,
            prefer_proxy: false,
            proxy_url: None,
            response_body_limit: None,
        }
    }

    #[must_use]
    pub fn with_response_body_limit(mut self, limit: usize) -> Self {
        self.response_body_limit = Some(limit);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResponse {
    pub body: String,
    pub used_proxy: bool,
    pub attempts: Vec<DownloadAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadBytesResponse {
    pub body: Vec<u8>,
    pub used_proxy: bool,
    pub attempts: Vec<DownloadAttempt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionalPreset {
    Russia,
    Iran,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalPresetSources {
    pub geo_source_url: String,
    pub srs_source_url: String,
    pub route_rules_template_source_url: String,
    pub dns_template_source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalPresetCatalog {
    pub russia: RegionalPresetSources,
    pub iran: RegionalPresetSources,
}

impl Default for RegionalPresetCatalog {
    fn default() -> Self {
        Self {
            russia: RegionalPresetSources {
                geo_source_url: RUSSIA_GEO_SOURCE_URL.to_string(),
                srs_source_url: RUSSIA_SRS_SOURCE_URL.to_string(),
                route_rules_template_source_url: RUSSIA_ROUTING_RULES_SOURCE_URL.to_string(),
                dns_template_source_url: RUSSIA_DNS_TEMPLATE_SOURCE_URL.to_string(),
            },
            iran: RegionalPresetSources {
                geo_source_url: IRAN_GEO_SOURCE_URL.to_string(),
                srs_source_url: IRAN_SRS_SOURCE_URL.to_string(),
                route_rules_template_source_url: IRAN_ROUTING_RULES_SOURCE_URL.to_string(),
                dns_template_source_url: IRAN_DNS_TEMPLATE_SOURCE_URL.to_string(),
            },
        }
    }
}

impl RegionalPresetCatalog {
    #[must_use]
    pub fn sources(&self, preset: RegionalPreset) -> &RegionalPresetSources {
        match preset {
            RegionalPreset::Russia => &self.russia,
            RegionalPreset::Iran => &self.iran,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresetDnsTemplateFetchOptions {
    pub prefer_proxy: bool,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresetDnsTemplates {
    pub xray_template: Option<String>,
    pub singbox_template: Option<String>,
    pub simple_template: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PresetDnsTemplateClient {
    download: DownloadClient,
}

impl PresetDnsTemplateClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            download: DownloadClient::new(),
        }
    }

    pub async fn fetch(
        &self,
        source_url: &str,
        options: &PresetDnsTemplateFetchOptions,
    ) -> PresetDnsTemplates {
        let source_url = source_url.trim();
        if source_url.is_empty() {
            return PresetDnsTemplates::default();
        }

        let xray_url = join_url_path(source_url, "v2ray.json");
        let singbox_url = join_url_path(source_url, "sing_box.json");
        let simple_url = join_url_path(source_url, "simple_dns.json");

        PresetDnsTemplates {
            xray_template: self.fetch_optional(&xray_url, options).await,
            singbox_template: self.fetch_optional(&singbox_url, options).await,
            simple_template: self.fetch_optional(&simple_url, options).await,
        }
    }

    pub async fn fetch_optional(
        &self,
        url: &str,
        options: &PresetDnsTemplateFetchOptions,
    ) -> Option<String> {
        match self
            .download
            .download_text(DownloadRequest {
                url: url.to_string(),
                user_agent: None,
                prefer_proxy: options.prefer_proxy,
                proxy_url: options.proxy_url.clone(),
                response_body_limit: Some(PRESET_DNS_TEMPLATE_RESPONSE_LIMIT_BYTES),
            })
            .await
        {
            Ok(response) => Some(response.body),
            Err(error) => {
                tracing::warn!(?error, %url, "preset DNS template fetch failed");
                None
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadClient {
    direct_client: std::result::Result<Client, String>,
    proxy_clients: Arc<Mutex<HashMap<String, Client>>>,
}

impl Default for DownloadClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            direct_client: build_http_client(None).map_err(|error| error.to_string()),
            proxy_clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn download_text(&self, request: DownloadRequest) -> Result<DownloadResponse> {
        let mut attempts = Vec::new();
        let response_body_limit = request
            .response_body_limit
            .unwrap_or(DEFAULT_TEXT_RESPONSE_LIMIT_BYTES);

        if request.prefer_proxy {
            if let Some(proxy_url) = request
                .proxy_url
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                match self.proxy_client(&request.url, proxy_url) {
                    Ok(client) => {
                        match request_text(
                            &client,
                            &request.url,
                            request.user_agent.as_deref(),
                            response_body_limit,
                        )
                        .await
                        {
                            Ok(body) if !body.is_empty() => {
                                attempts.push(DownloadAttempt {
                                    url: request.url.clone(),
                                    via_proxy: true,
                                    bytes: body.len(),
                                    error: None,
                                });

                                return Ok(DownloadResponse {
                                    body,
                                    used_proxy: true,
                                    attempts,
                                });
                            }
                            Ok(body) => attempts.push(DownloadAttempt {
                                url: request.url.clone(),
                                via_proxy: true,
                                bytes: body.len(),
                                error: Some("empty response".to_string()),
                            }),
                            Err(error) => attempts.push(DownloadAttempt {
                                url: request.url.clone(),
                                via_proxy: true,
                                bytes: 0,
                                error: Some(error.to_string()),
                            }),
                        }
                    }
                    Err(error) => attempts.push(DownloadAttempt {
                        url: request.url.clone(),
                        via_proxy: true,
                        bytes: 0,
                        error: Some(error.to_string()),
                    }),
                }
            }
        }

        let client = match self.direct_client(&request.url) {
            Ok(client) => client,
            Err(error) => {
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: 0,
                    error: Some(error.to_string()),
                });
                return Err(DownloadError::AttemptsFailed {
                    url: request.url,
                    attempts,
                });
            }
        };

        match request_text(
            &client,
            &request.url,
            request.user_agent.as_deref(),
            response_body_limit,
        )
        .await
        {
            Ok(body) if !body.is_empty() => {
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: body.len(),
                    error: None,
                });

                Ok(DownloadResponse {
                    body,
                    used_proxy: false,
                    attempts,
                })
            }
            Ok(body) => {
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: body.len(),
                    error: Some("empty response".to_string()),
                });
                Err(DownloadError::AttemptsFailed {
                    url: request.url,
                    attempts,
                })
            }
            Err(error) => {
                let response_too_large = matches!(&error, DownloadError::ResponseTooLarge { .. });
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: 0,
                    error: Some(error.to_string()),
                });
                if response_too_large {
                    Err(error)
                } else {
                    Err(DownloadError::AttemptsFailed {
                        url: request.url,
                        attempts,
                    })
                }
            }
        }
    }

    pub async fn download_bytes(&self, request: DownloadRequest) -> Result<DownloadBytesResponse> {
        let mut attempts = Vec::new();
        let response_body_limit = request
            .response_body_limit
            .unwrap_or(DEFAULT_BINARY_RESPONSE_LIMIT_BYTES);

        if request.prefer_proxy {
            if let Some(proxy_url) = request
                .proxy_url
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                match self.proxy_client(&request.url, proxy_url) {
                    Ok(client) => {
                        match request_bytes(
                            &client,
                            &request.url,
                            request.user_agent.as_deref(),
                            response_body_limit,
                        )
                        .await
                        {
                            Ok(body) if !body.is_empty() => {
                                attempts.push(DownloadAttempt {
                                    url: request.url.clone(),
                                    via_proxy: true,
                                    bytes: body.len(),
                                    error: None,
                                });

                                return Ok(DownloadBytesResponse {
                                    body,
                                    used_proxy: true,
                                    attempts,
                                });
                            }
                            Ok(body) => attempts.push(DownloadAttempt {
                                url: request.url.clone(),
                                via_proxy: true,
                                bytes: body.len(),
                                error: Some("empty response".to_string()),
                            }),
                            Err(error) => attempts.push(DownloadAttempt {
                                url: request.url.clone(),
                                via_proxy: true,
                                bytes: 0,
                                error: Some(error.to_string()),
                            }),
                        }
                    }
                    Err(error) => attempts.push(DownloadAttempt {
                        url: request.url.clone(),
                        via_proxy: true,
                        bytes: 0,
                        error: Some(error.to_string()),
                    }),
                }
            }
        }

        let client = match self.direct_client(&request.url) {
            Ok(client) => client,
            Err(error) => {
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: 0,
                    error: Some(error.to_string()),
                });
                return Err(DownloadError::AttemptsFailed {
                    url: request.url,
                    attempts,
                });
            }
        };

        match request_bytes(
            &client,
            &request.url,
            request.user_agent.as_deref(),
            response_body_limit,
        )
        .await
        {
            Ok(body) if !body.is_empty() => {
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: body.len(),
                    error: None,
                });

                Ok(DownloadBytesResponse {
                    body,
                    used_proxy: false,
                    attempts,
                })
            }
            Ok(body) => {
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: body.len(),
                    error: Some("empty response".to_string()),
                });
                Err(DownloadError::AttemptsFailed {
                    url: request.url,
                    attempts,
                })
            }
            Err(error) => {
                let response_too_large = matches!(&error, DownloadError::ResponseTooLarge { .. });
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: 0,
                    error: Some(error.to_string()),
                });
                if response_too_large {
                    Err(error)
                } else {
                    Err(DownloadError::AttemptsFailed {
                        url: request.url,
                        attempts,
                    })
                }
            }
        }
    }

    fn direct_client(&self, url: &str) -> Result<Client> {
        self.direct_client
            .as_ref()
            .cloned()
            .map_err(|reason| DownloadError::ClientBuild {
                url: url.to_string(),
                reason: reason.clone(),
            })
    }

    fn proxy_client(&self, url: &str, proxy_url: &str) -> Result<Client> {
        let mut clients = match self.proxy_clients.lock() {
            Ok(clients) => clients,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(client) = clients.get(proxy_url) {
            return Ok(client.clone());
        }

        let client =
            build_http_client(Some(proxy_url)).map_err(|source| DownloadError::Request {
                url: url.to_string(),
                source,
            })?;
        clients.insert(proxy_url.to_string(), client.clone());

        Ok(client)
    }

    pub async fn download_file(
        &self,
        request: DownloadRequest,
        target: impl AsRef<Path>,
    ) -> Result<DownloadBytesResponse> {
        let target = target.as_ref();
        let parent = target
            .parent()
            .ok_or_else(|| DownloadError::MissingParent {
                path: target.to_path_buf(),
            })?;
        fs::create_dir_all(parent).map_err(|source| DownloadError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;

        let response = self.download_bytes(request).await?;
        let temp = target.with_extension(format!(
            "{}.download",
            target
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("tmp")
        ));
        fs::write(&temp, &response.body).map_err(|source| DownloadError::WriteFile {
            path: temp.clone(),
            source,
        })?;
        fs::rename(&temp, target).map_err(|source| DownloadError::WriteFile {
            path: target.to_path_buf(),
            source,
        })?;

        Ok(response)
    }
}

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
}

#[derive(Debug, Clone, Default)]
pub struct SubscriptionClient {
    download: DownloadClient,
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
        let main_url = build_subscription_url(
            source.url.trim(),
            source.convert_target.as_deref(),
            source.sub_convert_url.as_deref(),
        );
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

        if source
            .convert_target
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Ok(SubscriptionFetchResult { content, downloads });
        }

        for url in source
            .more_url
            .split(',')
            .map(str::trim)
            .filter(|url| !url.is_empty())
        {
            let additional = self
                .download
                .download_text(DownloadRequest {
                    url: url.to_string(),
                    user_agent: nonempty(source.user_agent.clone()),
                    prefer_proxy: options.prefer_proxy,
                    proxy_url: options.proxy_url.clone(),
                    response_body_limit: Some(SUBSCRIPTION_RESPONSE_LIMIT_BYTES),
                })
                .await?;
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

        Ok(SubscriptionFetchResult { content, downloads })
    }
}

#[must_use]
pub fn build_subscription_url(
    raw_url: &str,
    convert_target: Option<&str>,
    sub_convert_url: Option<&str>,
) -> String {
    let Some(target) = convert_target
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return raw_url.trim().to_string();
    };

    let template = sub_convert_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_SUB_CONVERT_URL);
    let encoded_url = utf8_percent_encode(raw_url.trim(), NON_ALPHANUMERIC).to_string();
    let mut url = if template.contains("{0}") {
        template.replace("{0}", &encoded_url)
    } else {
        format!("{template}{encoded_url}")
    };

    if !url.contains("target=") {
        url.push_str("&target=");
        url.push_str(target);
    }
    if !url.contains("config=") {
        url.push_str("&config=");
        url.push_str(DEFAULT_SUB_CONVERT_CONFIG);
    }

    url
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

pub(crate) fn build_http_client(
    proxy_url: Option<&str>,
) -> std::result::Result<Client, reqwest::Error> {
    let mut builder = Client::builder()
        .timeout(HTTP_REQUEST_TIMEOUT)
        .connect_timeout(HTTP_CONNECT_TIMEOUT);
    builder = if let Some(proxy_url) = proxy_url {
        builder.proxy(Proxy::all(proxy_url)?)
    } else {
        builder.no_proxy()
    };

    builder.build()
}

#[derive(Debug, Error)]
pub(crate) enum LimitedBodyReadError {
    #[error(
        "response body exceeds {limit} bytes (content length: {content_length:?}, received: {received})"
    )]
    TooLarge {
        limit: usize,
        content_length: Option<u64>,
        received: usize,
    },
    #[error("failed to read response body: {source}")]
    Read {
        #[source]
        source: reqwest::Error,
    },
}

pub(crate) async fn read_response_bytes_limited(
    mut response: reqwest::Response,
    limit: usize,
) -> std::result::Result<Vec<u8>, LimitedBodyReadError> {
    let content_length = response.content_length();
    let limit_u64 = u64::try_from(limit).unwrap_or(u64::MAX);
    if content_length.is_some_and(|length| length > limit_u64) {
        return Err(LimitedBodyReadError::TooLarge {
            limit,
            content_length,
            received: 0,
        });
    }

    let capacity = match content_length {
        Some(length) => match usize::try_from(length) {
            Ok(length) => length.min(limit),
            Err(_) => 0,
        },
        None => 0,
    };
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|source| LimitedBodyReadError::Read { source })?
    {
        let received = body.len().saturating_add(chunk.len());
        if received > limit {
            return Err(LimitedBodyReadError::TooLarge {
                limit,
                content_length,
                received,
            });
        }
        body.extend_from_slice(&chunk);
    }

    Ok(body)
}

pub(crate) async fn read_response_text_limited(
    response: reqwest::Response,
    limit: usize,
) -> std::result::Result<String, LimitedBodyReadError> {
    let bytes = read_response_bytes_limited(response, limit).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn map_download_body_error(url: &str, error: LimitedBodyReadError) -> DownloadError {
    match error {
        LimitedBodyReadError::TooLarge {
            limit,
            content_length,
            received,
        } => DownloadError::ResponseTooLarge {
            url: url.to_string(),
            limit,
            content_length,
            received,
        },
        LimitedBodyReadError::Read { source } => DownloadError::Request {
            url: url.to_string(),
            source,
        },
    }
}

async fn request_text(
    client: &Client,
    url: &str,
    user_agent: Option<&str>,
    response_body_limit: usize,
) -> Result<String> {
    let user_agent = user_agent
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(USER_AGENT_PREFIX);

    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .send()
        .await
        .map_err(|source| DownloadError::Request {
            url: url.to_string(),
            source,
        })?
        .error_for_status()
        .map_err(|source| DownloadError::Request {
            url: url.to_string(),
            source,
        })?;

    read_response_text_limited(response, response_body_limit)
        .await
        .map_err(|error| map_download_body_error(url, error))
}

async fn request_bytes(
    client: &Client,
    url: &str,
    user_agent: Option<&str>,
    response_body_limit: usize,
) -> Result<Vec<u8>> {
    let user_agent = user_agent
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(USER_AGENT_PREFIX);

    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .send()
        .await
        .map_err(|source| DownloadError::Request {
            url: url.to_string(),
            source,
        })?
        .error_for_status()
        .map_err(|source| DownloadError::Request {
            url: url.to_string(),
            source,
        })?;

    read_response_bytes_limited(response, response_body_limit)
        .await
        .map_err(|error| map_download_body_error(url, error))
}

fn nonempty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn join_url_path(base: &str, file_name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{file_name}")
    } else {
        format!("{base}/{file_name}")
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use base64::engine::general_purpose::STANDARD;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };

    use super::*;

    #[test]
    fn user_agent_prefix_names_the_app() {
        assert_eq!(USER_AGENT_PREFIX, "VoyaVPN");
    }

    #[tokio::test]
    async fn download_uses_custom_user_agent_and_falls_back_to_direct() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([(
                "/sub".to_string(),
                "vless://id@example.test:443#A".to_string(),
            )]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let response = DownloadClient::new()
            .download_text(DownloadRequest {
                url: format!("{base}/sub"),
                user_agent: Some("VoyaTest/1".to_string()),
                prefer_proxy: true,
                proxy_url: Some("http://127.0.0.1:9".to_string()),
                response_body_limit: None,
            })
            .await
            .expect("download should fall back to direct");

        assert!(!response.used_proxy);
        assert_eq!(response.attempts.len(), 2);
        assert_eq!(response.body, "vless://id@example.test:443#A");
        assert_eq!(seen_user_agents.lock().await.as_slice(), ["VoyaTest/1"]);
    }

    #[tokio::test]
    async fn download_text_rejects_declared_response_above_limit() {
        let base = spawn_raw_http_fixture(
            HashMap::from([(
                "/oversize".to_string(),
                RawFixtureResponse {
                    status: "200 OK".to_string(),
                    content_length: Some(6),
                    body: b"abcdef".to_vec(),
                },
            )]),
            1,
        )
        .await;

        let error = DownloadClient::new()
            .download_text(
                DownloadRequest::direct(format!("{base}/oversize")).with_response_body_limit(4),
            )
            .await
            .expect_err("oversized response should fail");

        assert!(
            matches!(
                error,
                DownloadError::ResponseTooLarge {
                    limit: 4,
                    content_length: Some(6),
                    received: 0,
                    ..
                }
            ),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn download_bytes_rejects_chunked_response_above_limit() {
        let base = spawn_raw_http_fixture(
            HashMap::from([(
                "/stream".to_string(),
                RawFixtureResponse {
                    status: "200 OK".to_string(),
                    content_length: None,
                    body: b"abcdef".to_vec(),
                },
            )]),
            1,
        )
        .await;

        let error = DownloadClient::new()
            .download_bytes(
                DownloadRequest::direct(format!("{base}/stream")).with_response_body_limit(4),
            )
            .await
            .expect_err("oversized response should fail");

        assert!(
            matches!(
                error,
                DownloadError::ResponseTooLarge {
                    limit: 4,
                    content_length: None,
                    received,
                    ..
                } if received > 4
            ),
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
            .fetch(
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
            .fetch(
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
            )
            .await
            .expect("converted subscription content");

        assert_eq!(result.content, "mixed-converted-subscription");
        assert_eq!(result.downloads.len(), 1);

        let rewritten = build_subscription_url(
            &source_url,
            Some("clash"),
            Some(&format!("{base}/convert?url={{0}}")),
        );
        assert!(rewritten.contains("/convert?url=http%3A%2F%2F127%2E0%2E0%2E1"));
        assert!(rewritten.contains("&target=clash"));
        assert!(rewritten.contains("&config="));
    }

    #[tokio::test]
    async fn preset_dns_template_client_fetches_regional_templates() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([
                (
                    "/preset/v2ray.json".to_string(),
                    r#"{"NormalDNS":"xray"}"#.to_string(),
                ),
                (
                    "/preset/sing_box.json".to_string(),
                    r#"{"NormalDNS":"sing-box"}"#.to_string(),
                ),
                (
                    "/preset/simple_dns.json".to_string(),
                    r#"{"DirectDNS":"1.1.1.1"}"#.to_string(),
                ),
            ]),
            3,
            Arc::clone(&seen_user_agents),
        )
        .await;

        let templates = PresetDnsTemplateClient::new()
            .fetch(
                &format!("{base}/preset"),
                &PresetDnsTemplateFetchOptions::default(),
            )
            .await;

        assert_eq!(
            templates.xray_template.as_deref(),
            Some(r#"{"NormalDNS":"xray"}"#)
        );
        assert_eq!(
            templates.singbox_template.as_deref(),
            Some(r#"{"NormalDNS":"sing-box"}"#)
        );
        assert_eq!(
            templates.simple_template.as_deref(),
            Some(r#"{"DirectDNS":"1.1.1.1"}"#)
        );
        assert_eq!(
            seen_user_agents.lock().await.as_slice(),
            [USER_AGENT_PREFIX, USER_AGENT_PREFIX, USER_AGENT_PREFIX]
        );
    }

    #[tokio::test]
    async fn preset_dns_template_client_returns_none_for_missing_templates() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(HashMap::new(), 3, Arc::clone(&seen_user_agents)).await;

        let templates = PresetDnsTemplateClient::new()
            .fetch(
                &format!("{base}/missing/"),
                &PresetDnsTemplateFetchOptions::default(),
            )
            .await;

        assert_eq!(templates, PresetDnsTemplates::default());
        assert_eq!(seen_user_agents.lock().await.len(), 3);
    }

    async fn spawn_http_fixture(
        routes: HashMap<String, String>,
        max_requests: usize,
        seen_user_agents: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("update test operation should succeed");
        let address = listener
            .local_addr()
            .expect("update test operation should succeed");
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

    #[derive(Clone)]
    struct RawFixtureResponse {
        status: String,
        content_length: Option<usize>,
        body: Vec<u8>,
    }

    async fn spawn_raw_http_fixture(
        routes: HashMap<String, RawFixtureResponse>,
        max_requests: usize,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("update test operation should succeed");
        let address = listener
            .local_addr()
            .expect("update test operation should succeed");
        let routes = Arc::new(routes);

        tokio::spawn(async move {
            for _ in 0..max_requests {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let routes = Arc::clone(&routes);
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
                    let response = routes.get(path).cloned().unwrap_or(RawFixtureResponse {
                        status: "404 Not Found".to_string(),
                        content_length: Some(9),
                        body: b"not found".to_vec(),
                    });
                    let header = match response.content_length {
                        Some(length) => format!(
                            "HTTP/1.1 {}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n",
                            response.status
                        ),
                        None => {
                            format!("HTTP/1.1 {}\r\nConnection: close\r\n\r\n", response.status)
                        }
                    };
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(&response.body).await;
                });
            }
        });

        format!("http://{address}")
    }
}
