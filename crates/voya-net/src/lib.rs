//! Network service clients for downloads, subscriptions, Clash API,
//! geo assets, and rulesets.

mod download;
pub mod portmap;
pub mod probe;
mod subscription;

pub mod clash;
pub mod ruleset;

pub use download::{
    DownloadAttempt, DownloadBytesResponse, DownloadClient, DownloadError, DownloadRequest,
    DownloadResponse, Result, DEFAULT_BINARY_RESPONSE_LIMIT_BYTES,
    DEFAULT_TEXT_RESPONSE_LIMIT_BYTES, EMPTY_RESPONSE_ATTEMPT_ERROR,
};
pub use subscription::{
    FailedSubscriptionSource, SubscriptionClient, SubscriptionFetchOptions,
    SubscriptionFetchResult, SubscriptionFetchSource,
};

pub(crate) use download::{
    build_http_client, is_denied_local_host, read_response_text_limited, LimitedBodyReadError,
};
