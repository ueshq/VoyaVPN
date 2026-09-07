//! Network service clients for downloads, subscriptions, Clash API,
//! geo assets, and rulesets.

mod download;
pub mod probe;
mod subscription;
mod url;

pub mod certificates;
pub mod clash;
pub mod ruleset;

pub use download::{
    DownloadAttempt, DownloadBytesResponse, DownloadClient, DownloadError, DownloadRequest,
    DownloadResponse, Result, DEFAULT_BINARY_RESPONSE_LIMIT_BYTES,
    DEFAULT_TEXT_RESPONSE_LIMIT_BYTES, EMPTY_RESPONSE_ATTEMPT_ERROR, USER_AGENT_PREFIX,
};
pub use subscription::{
    build_subscription_url, decode_base64_payload, FailedSubscriptionSource, SubscriptionClient,
    SubscriptionFetchOptions, SubscriptionFetchResult, SubscriptionFetchSource,
    DEFAULT_SUB_CONVERT_CONFIG, DEFAULT_SUB_CONVERT_URL,
};
pub use url::{validate_absolute_http_url, validate_absolute_https_url, UrlValidationError};

pub(crate) use download::{
    build_http_client, is_denied_local_host, read_response_text_limited, LimitedBodyReadError,
};
