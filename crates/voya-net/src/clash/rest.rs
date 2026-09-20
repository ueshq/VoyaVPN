//! The Clash REST API: the HTTP transport (a seam for tests) and the typed
//! client the app drives the running core through.

use std::{future::Future, pin::Pin};

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::Method;
use serde_json::json;

use super::*;

const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');
/// Query values also escape the characters that would end or split them.
const QUERY_VALUE_ENCODE_SET: &AsciiSet = &PATH_SEGMENT_ENCODE_SET
    .add(b':')
    .add(b'&')
    .add(b'=')
    .add(b'+');
pub(super) const CLASH_HTTP_RESPONSE_LIMIT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClashHttpMethod {
    Get,
    Put,
    Patch,
    Delete,
}

impl From<ClashHttpMethod> for Method {
    fn from(value: ClashHttpMethod) -> Self {
        match value {
            ClashHttpMethod::Get => Self::GET,
            ClashHttpMethod::Put => Self::PUT,
            ClashHttpMethod::Patch => Self::PATCH,
            ClashHttpMethod::Delete => Self::DELETE,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct ClashHttpRequest {
    pub method: ClashHttpMethod,
    pub url: String,
    pub body: Option<Value>,
    pub bearer_token: Option<String>,
}

impl fmt::Debug for ClashHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClashHttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("body", &self.body)
            .field("bearer_token", &redacted(self.bearer_token.as_deref()))
            .finish()
    }
}

pub trait ClashHttpTransport: Clone + Send + Sync + 'static {
    /// Sends `request` and returns the response body as text, empty for a
    /// bodiless reply. The client parses it straight into the type it asks
    /// for: `/proxies` of a large group is hundreds of kilobytes, polled every
    /// few seconds, and a `Value` tree in between doubled that work.
    fn send<'transport>(
        &'transport self,
        request: ClashHttpRequest,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'transport>>;
}

#[derive(Debug, Clone)]
pub struct ReqwestClashHttpTransport {
    client: std::result::Result<reqwest::Client, String>,
}

impl Default for ReqwestClashHttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestClashHttpTransport {
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: crate::build_http_client(None).map_err(|error| error.to_string()),
        }
    }
}

impl ClashHttpTransport for ReqwestClashHttpTransport {
    fn send<'transport>(
        &'transport self,
        request: ClashHttpRequest,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'transport>> {
        Box::pin(async move {
            let client = self.client.as_ref().map_err(|error| {
                ClashError::Request(format!("failed to build HTTP client: {error}"))
            })?;
            let mut builder = client.request(Method::from(request.method), &request.url);
            if let Some(token) = request
                .bearer_token
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                builder = builder.bearer_auth(token);
            }
            if let Some(body) = &request.body {
                builder = builder.json(body);
            }

            let response = builder
                .send()
                .await
                .map_err(|error| ClashError::Request(error.to_string()))?
                .error_for_status()
                .map_err(|error| match error.status() {
                    Some(status) => ClashError::Status(status.as_u16()),
                    None => ClashError::Request(error.to_string()),
                })?;

            crate::read_response_text_limited(response, CLASH_HTTP_RESPONSE_LIMIT_BYTES)
                .await
                .map_err(|error| clash_body_error(&request.url, error))
        })
    }
}

fn clash_body_error(url: &str, error: crate::LimitedBodyReadError) -> ClashError {
    match error {
        crate::LimitedBodyReadError::TooLarge {
            limit,
            content_length,
            received,
        } => ClashError::ResponseTooLarge {
            url: url.to_string(),
            limit,
            content_length,
            received,
        },
        crate::LimitedBodyReadError::Read { source } => ClashError::Request(source.to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct ClashRestClient<T = ReqwestClashHttpTransport> {
    endpoint: ClashApiEndpoint,
    transport: T,
}

impl ClashRestClient<ReqwestClashHttpTransport> {
    #[must_use]
    pub fn new(endpoint: ClashApiEndpoint) -> Self {
        Self::with_transport(endpoint, ReqwestClashHttpTransport::new())
    }
}

impl<T> ClashRestClient<T>
where
    T: ClashHttpTransport,
{
    #[must_use]
    pub fn with_transport(endpoint: ClashApiEndpoint, transport: T) -> Self {
        Self {
            endpoint,
            transport,
        }
    }

    pub async fn get_connections(&self) -> Result<ClashConnections> {
        self.request(ClashHttpMethod::Get, "/connections", None)
            .await
    }
    pub async fn set_rule_mode(&self, mode: &str) -> Result<()> {
        self.send(
            ClashHttpMethod::Patch,
            "/configs",
            Some(json!({ "mode": mode })),
        )
        .await
        .map(drop)
    }
    pub async fn close_connection(&self, connection_id: Option<&str>) -> Result<()> {
        let path = connection_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| format!("/connections/{}", encode_segment(id)))
            .unwrap_or_else(|| "/connections".to_string());

        self.send(ClashHttpMethod::Delete, &path, None)
            .await
            .map(drop)
    }

    /// Every outbound and group the running core knows, keyed by tag.
    pub async fn get_proxies(&self) -> Result<ClashProxiesResponse> {
        self.request(ClashHttpMethod::Get, "/proxies", None).await
    }

    /// Switches the selector `group` to its member `member`.
    pub async fn select_proxy(&self, group: &str, member: &str) -> Result<()> {
        let path = format!("/proxies/{}", encode_segment(group));
        self.send(ClashHttpMethod::Put, &path, Some(json!({ "name": member })))
            .await
            .map(drop)
    }

    /// Probes every member of `group` through the core. The result maps each
    /// member tag to its delay; a member whose probe failed is absent.
    pub async fn group_delay(
        &self,
        group: &str,
        test_url: &str,
        timeout_ms: u32,
    ) -> Result<BTreeMap<String, u32>> {
        let path = format!(
            "/group/{}/delay?url={}&timeout={timeout_ms}",
            encode_segment(group),
            encode_query_value(test_url)
        );
        self.request(ClashHttpMethod::Get, &path, None).await
    }

    /// Measures one outbound through the core (a HEAD of `test_url`, dial
    /// included). A tag the core does not know fails with `Status(404)`, a
    /// probe past `timeout_ms` with `Status(504)`, any other failed probe with
    /// `Status(503)`.
    pub async fn proxy_delay(&self, tag: &str, test_url: &str, timeout_ms: u32) -> Result<u32> {
        let path = format!(
            "/proxies/{}/delay?url={}&timeout={timeout_ms}",
            encode_segment(tag),
            encode_query_value(test_url)
        );
        let response: ClashDelayResponse = self.request(ClashHttpMethod::Get, &path, None).await?;
        Ok(response.delay)
    }

    async fn request<R>(
        &self,
        method: ClashHttpMethod,
        path_and_query: &str,
        body: Option<Value>,
    ) -> Result<R>
    where
        R: for<'de> Deserialize<'de>,
    {
        let text = self.send(method, path_and_query, body).await?;
        // A bodiless reply reads as JSON `null`, as it did through `Value`.
        let text = if text.trim().is_empty() {
            "null"
        } else {
            &text
        };
        serde_json::from_str(text).map_err(|error| ClashError::Decode(error.to_string()))
    }

    /// The raw reply; commands that answer `204 No Content` stop here.
    async fn send(
        &self,
        method: ClashHttpMethod,
        path_and_query: &str,
        body: Option<Value>,
    ) -> Result<String> {
        self.transport
            .send(ClashHttpRequest {
                method,
                url: self.endpoint.http_url(path_and_query),
                body,
                bearer_token: self.endpoint.secret.clone(),
            })
            .await
    }
}

#[derive(Debug, Deserialize)]
struct ClashDelayResponse {
    delay: u32,
}

fn encode_segment(value: &str) -> String {
    utf8_percent_encode(value, PATH_SEGMENT_ENCODE_SET).to_string()
}

fn encode_query_value(value: &str) -> String {
    utf8_percent_encode(value, QUERY_VALUE_ENCODE_SET).to_string()
}
