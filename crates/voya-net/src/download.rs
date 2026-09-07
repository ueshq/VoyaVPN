use reqwest::{Client, Proxy};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error;

pub(crate) mod redirect;

pub(crate) use redirect::is_denied_local_host;
use redirect::redirect_policy;

/// Shared user agent prefix for network clients.
pub const USER_AGENT_PREFIX: &str = "VoyaVPN";

/// Whole-request deadline. It bounds the Clash REST client and text downloads, which are always
/// small — subscriptions and routing templates are capped at `DEFAULT_TEXT_RESPONSE_LIMIT_BYTES`.
pub(crate) const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Idle deadline applied to every download client. Byte downloads carry geo databases and rule
/// sets of many megabytes, which exceed `HTTP_REQUEST_TIMEOUT` over a throttled proxy even while
/// data keeps flowing, so they are bounded by silence rather than by total elapsed time.
pub(crate) const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Absolute ceiling for a byte download. It exists only so a server that dribbles one byte just
/// inside every read timeout cannot pin the task forever; a real asset transfer finishes far
/// inside it even on a heavily throttled link.
pub(crate) const HTTP_ASSET_REQUEST_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub(crate) const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_TEXT_RESPONSE_LIMIT_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_BINARY_RESPONSE_LIMIT_BYTES: usize = 512 * 1024 * 1024;

/// Recorded on a `DownloadAttempt` whose request succeeded but returned no bytes.
///
/// Callers distinguish "the server answered with nothing" (an expired or emptied subscription)
/// from a transport failure, so the marker is a shared constant matched through
/// [`DownloadAttempt::is_empty_response`] rather than a message that can be reworded.
pub const EMPTY_RESPONSE_ATTEMPT_ERROR: &str = "empty response";

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
    #[error("subscription URL {url} is not allowed: {reason}")]
    ForbiddenSubscriptionUrl { url: String, reason: String },
    #[error("all download attempts failed for {url}: {attempts:?}")]
    AttemptsFailed {
        url: String,
        attempts: Vec<DownloadAttempt>,
    },
}

impl DownloadError {
    /// Reports whether every attempt reached the server and came back empty.
    ///
    /// Callers use this to tell an emptied subscription apart from a transport failure without
    /// reproducing the [`EMPTY_RESPONSE_ATTEMPT_ERROR`] literal.
    #[must_use]
    pub fn is_empty_response(&self) -> bool {
        match self {
            Self::AttemptsFailed { attempts, .. } => {
                !attempts.is_empty() && attempts.iter().all(DownloadAttempt::is_empty_response)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadAttempt {
    pub url: String,
    pub via_proxy: bool,
    pub bytes: usize,
    pub error: Option<String>,
}

impl DownloadAttempt {
    /// Reports whether this attempt returned a successful but empty body.
    #[must_use]
    pub fn is_empty_response(&self) -> bool {
        self.bytes == 0 && self.error.as_deref() == Some(EMPTY_RESPONSE_ATTEMPT_ERROR)
    }
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

/// Response headers surfaced to callers, restricted to this allowlist so the
/// download layer never leaks arbitrary server headers upward.
pub const CAPTURED_RESPONSE_HEADERS: [&str; 6] = [
    "subscription-userinfo",
    "profile-update-interval",
    "profile-title",
    "profile-web-page-url",
    "support-url",
    "content-disposition",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResponse {
    pub body: String,
    /// Allowlisted response headers as lowercased `(name, value)` pairs.
    pub headers: Vec<(String, String)>,
    pub used_proxy: bool,
    pub attempts: Vec<DownloadAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadBytesResponse {
    pub body: Vec<u8>,
    pub used_proxy: bool,
    pub attempts: Vec<DownloadAttempt>,
}

trait DownloadBody {
    fn byte_len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

impl DownloadBody for String {
    fn byte_len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        String::is_empty(self)
    }
}

impl DownloadBody for Vec<u8> {
    fn byte_len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        Vec::is_empty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BodyWithHeaders<T> {
    body: T,
    headers: Vec<(String, String)>,
}

impl<T: DownloadBody> DownloadBody for BodyWithHeaders<T> {
    fn byte_len(&self) -> usize {
        self.body.byte_len()
    }

    fn is_empty(&self) -> bool {
        self.body.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DownloadOutput<T> {
    body: T,
    used_proxy: bool,
    attempts: Vec<DownloadAttempt>,
}

type DownloadRequestFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

type DownloadBodyRequest<T> =
    for<'a> fn(&'a Client, &'a str, Option<&'a str>, usize) -> DownloadRequestFuture<'a, T>;

#[derive(Debug, Clone)]
pub struct DownloadClient {
    direct_client: std::result::Result<Client, String>,
    proxy_clients: Arc<Mutex<HashMap<String, Client>>>,
    read_timeout: Duration,
}

impl Default for DownloadClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadClient {
    #[must_use]
    pub fn new() -> Self {
        Self::with_read_timeout(HTTP_READ_TIMEOUT)
    }

    /// Builds a client whose transfers are bounded by silence instead of by total elapsed time,
    /// so a slow multi-megabyte asset download can be exercised in milliseconds.
    #[cfg(any(test, feature = "test-utils"))]
    #[must_use]
    pub fn with_read_timeout_for_tests(read_timeout: Duration) -> Self {
        Self::with_read_timeout(read_timeout)
    }

    fn with_read_timeout(read_timeout: Duration) -> Self {
        Self {
            direct_client: build_download_http_client(None, read_timeout)
                .map_err(|error| error.to_string()),
            proxy_clients: Arc::new(Mutex::new(HashMap::new())),
            read_timeout,
        }
    }

    pub async fn download_text(&self, request: DownloadRequest) -> Result<DownloadResponse> {
        let response = self
            .request(
                request,
                DEFAULT_TEXT_RESPONSE_LIMIT_BYTES,
                request_text_boxed,
            )
            .await?;

        Ok(DownloadResponse {
            body: response.body.body,
            headers: response.body.headers,
            used_proxy: response.used_proxy,
            attempts: response.attempts,
        })
    }

    pub async fn download_bytes(&self, request: DownloadRequest) -> Result<DownloadBytesResponse> {
        let response = self
            .request(
                request,
                DEFAULT_BINARY_RESPONSE_LIMIT_BYTES,
                request_bytes_boxed,
            )
            .await?;

        Ok(DownloadBytesResponse {
            body: response.body,
            used_proxy: response.used_proxy,
            attempts: response.attempts,
        })
    }

    async fn request<T>(
        &self,
        request: DownloadRequest,
        default_response_body_limit: usize,
        request_body: DownloadBodyRequest<T>,
    ) -> Result<DownloadOutput<T>>
    where
        T: DownloadBody,
    {
        let mut attempts = Vec::new();
        let response_body_limit = request
            .response_body_limit
            .unwrap_or(default_response_body_limit);

        if request.prefer_proxy {
            if let Some(proxy_url) = request
                .proxy_url
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                match self.proxy_client(&request.url, proxy_url) {
                    Ok(client) => {
                        match request_body(
                            &client,
                            &request.url,
                            request.user_agent.as_deref(),
                            response_body_limit,
                        )
                        .await
                        {
                            Ok(body) if !body.is_empty() => {
                                let bytes = body.byte_len();
                                attempts.push(DownloadAttempt {
                                    url: request.url.clone(),
                                    via_proxy: true,
                                    bytes,
                                    error: None,
                                });

                                return Ok(DownloadOutput {
                                    body,
                                    used_proxy: true,
                                    attempts,
                                });
                            }
                            Ok(body) => attempts.push(DownloadAttempt {
                                url: request.url.clone(),
                                via_proxy: true,
                                bytes: body.byte_len(),
                                error: Some(EMPTY_RESPONSE_ATTEMPT_ERROR.to_string()),
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

        match request_body(
            &client,
            &request.url,
            request.user_agent.as_deref(),
            response_body_limit,
        )
        .await
        {
            Ok(body) if !body.is_empty() => {
                let bytes = body.byte_len();
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes,
                    error: None,
                });

                Ok(DownloadOutput {
                    body,
                    used_proxy: false,
                    attempts,
                })
            }
            Ok(body) => {
                attempts.push(DownloadAttempt {
                    url: request.url.clone(),
                    via_proxy: false,
                    bytes: body.byte_len(),
                    error: Some(EMPTY_RESPONSE_ATTEMPT_ERROR.to_string()),
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
            build_download_http_client(Some(proxy_url), self.read_timeout).map_err(|source| {
                DownloadError::Request {
                    url: url.to_string(),
                    source,
                }
            })?;
        clients.insert(proxy_url.to_string(), client.clone());

        Ok(client)
    }
}

/// Builds the client used outside the download stack (currently the Clash REST transport), which
/// only ever exchanges small payloads and therefore keeps a whole-request deadline.
pub(crate) fn build_http_client(
    proxy_url: Option<&str>,
) -> std::result::Result<Client, reqwest::Error> {
    http_client_builder(proxy_url, HTTP_READ_TIMEOUT)?
        .timeout(HTTP_REQUEST_TIMEOUT)
        .build()
}

/// Builds a download client without a whole-request deadline: `download_text` applies its own
/// per-request one, while byte downloads rely on `read_timeout` so a large asset transfer that
/// keeps making progress is never cut off mid-flight.
fn build_download_http_client(
    proxy_url: Option<&str>,
    read_timeout: Duration,
) -> std::result::Result<Client, reqwest::Error> {
    http_client_builder(proxy_url, read_timeout)?.build()
}

fn http_client_builder(
    proxy_url: Option<&str>,
    read_timeout: Duration,
) -> std::result::Result<reqwest::ClientBuilder, reqwest::Error> {
    // Trust policy shared with `certificates::client_config`: the bundled webpki roots plus
    // the roots the operating system trusts, so self-hosted servers behind a private CA the
    // OS already trusts work in both paths. Both flags default to true; setting them keeps the
    // policy explicit and fails the build if the reqwest root features are ever dropped.
    let builder = Client::builder()
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .read_timeout(read_timeout)
        .tls_built_in_webpki_certs(true)
        .tls_built_in_native_certs(true)
        .redirect(redirect_policy());

    Ok(if let Some(proxy_url) = proxy_url {
        builder.proxy(Proxy::all(proxy_url)?)
    } else {
        builder.no_proxy()
    })
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

fn request_text_boxed<'a>(
    client: &'a Client,
    url: &'a str,
    user_agent: Option<&'a str>,
    response_body_limit: usize,
) -> DownloadRequestFuture<'a, BodyWithHeaders<String>> {
    Box::pin(request_text(client, url, user_agent, response_body_limit))
}

fn request_bytes_boxed<'a>(
    client: &'a Client,
    url: &'a str,
    user_agent: Option<&'a str>,
    response_body_limit: usize,
) -> DownloadRequestFuture<'a, Vec<u8>> {
    Box::pin(request_bytes(client, url, user_agent, response_body_limit))
}

async fn request<T, ExtractBody, ExtractFuture>(
    client: &Client,
    url: &str,
    user_agent: Option<&str>,
    response_body_limit: usize,
    total_timeout: Option<Duration>,
    extract_body: ExtractBody,
) -> Result<T>
where
    ExtractBody: FnOnce(reqwest::Response, usize) -> ExtractFuture,
    ExtractFuture: Future<Output = std::result::Result<T, LimitedBodyReadError>>,
{
    let user_agent = user_agent
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(USER_AGENT_PREFIX);

    let mut builder = client
        .get(url)
        .header(reqwest::header::USER_AGENT, user_agent);
    if let Some(total_timeout) = total_timeout {
        builder = builder.timeout(total_timeout);
    }

    let response = builder
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

    extract_body(response, response_body_limit)
        .await
        .map_err(|error| map_download_body_error(url, error))
}

async fn request_text(
    client: &Client,
    url: &str,
    user_agent: Option<&str>,
    response_body_limit: usize,
) -> Result<BodyWithHeaders<String>> {
    request(
        client,
        url,
        user_agent,
        response_body_limit,
        Some(HTTP_REQUEST_TIMEOUT),
        |response, limit| async move {
            let headers = capture_response_headers(&response);
            let body = read_response_text_limited(response, limit).await?;
            Ok(BodyWithHeaders { body, headers })
        },
    )
    .await
}

fn capture_response_headers(response: &reqwest::Response) -> Vec<(String, String)> {
    CAPTURED_RESPONSE_HEADERS
        .iter()
        .filter_map(|name| {
            response
                .headers()
                .get(*name)
                .and_then(|value| value.to_str().ok())
                .map(|value| ((*name).to_string(), value.to_string()))
        })
        .collect()
}

async fn request_bytes(
    client: &Client,
    url: &str,
    user_agent: Option<&str>,
    response_body_limit: usize,
) -> Result<Vec<u8>> {
    // Geo databases and rule sets run to hundreds of megabytes, so a transfer that keeps
    // delivering is bounded by the client's read timeout rather than by `HTTP_REQUEST_TIMEOUT`;
    // the far larger ceiling below only stops a pathologically slow server.
    request(
        client,
        url,
        user_agent,
        response_body_limit,
        Some(HTTP_ASSET_REQUEST_TIMEOUT),
        read_response_bytes_limited,
    )
    .await
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::{collections::HashMap, sync::Arc};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };

    pub(crate) async fn spawn_http_fixture(
        routes: HashMap<String, String>,
        max_requests: usize,
        seen_user_agents: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let routes = routes
            .into_iter()
            .map(|(path, body)| (path, body.into_bytes()))
            .collect();

        spawn_http_bytes_fixture(routes, max_requests, seen_user_agents).await
    }

    pub(crate) async fn spawn_http_bytes_fixture(
        routes: HashMap<String, Vec<u8>>,
        max_requests: usize,
        seen_user_agents: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("HTTP fixture should bind");
        let address = listener.local_addr().expect("HTTP fixture address");
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
                    let path = request_path(&request);
                    let user_agent = request_header(&request, "user-agent").unwrap_or_default();
                    seen_user_agents.lock().await.push(user_agent);
                    let body = routes.get(path).cloned().unwrap_or_default();
                    let status = if routes.contains_key(path) {
                        "200 OK"
                    } else {
                        "404 Not Found"
                    };
                    let header = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });

        format!("http://{address}")
    }

    pub(crate) async fn spawn_redirect_chain_fixture(redirects: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("HTTP fixture should bind");
        let address = listener.local_addr().expect("HTTP fixture address");

        tokio::spawn(async move {
            for _ in 0..=redirects {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buffer = vec![0; 4096];
                    let bytes_read = socket.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let path = request_path(&request);
                    let index = path
                        .strip_prefix("/r")
                        .and_then(|value| value.parse::<usize>().ok());
                    let response = match index {
                        Some(index) if index < redirects => {
                            let next = index.saturating_add(1);
                            format!(
                                "HTTP/1.1 302 Found\r\nLocation: /r{next}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            )
                        }
                        Some(index) if index == redirects => {
                            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                                .to_string()
                        }
                        _ => {
                            "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found"
                                .to_string()
                        }
                    };
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        format!("http://{address}")
    }

    #[derive(Clone)]
    pub(crate) struct RawFixtureResponse {
        pub(crate) status: String,
        pub(crate) content_length: Option<usize>,
        pub(crate) extra_headers: Vec<(String, String)>,
        pub(crate) body: Vec<u8>,
    }

    pub(crate) async fn spawn_raw_http_fixture(
        routes: HashMap<String, RawFixtureResponse>,
        max_requests: usize,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("HTTP fixture should bind");
        let address = listener.local_addr().expect("HTTP fixture address");
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
                    let path = request_path(&request);
                    let response = routes.get(path).cloned().unwrap_or(RawFixtureResponse {
                        status: "404 Not Found".to_string(),
                        content_length: Some(9),
                        extra_headers: Vec::new(),
                        body: b"not found".to_vec(),
                    });
                    let extra_headers = response
                        .extra_headers
                        .iter()
                        .map(|(name, value)| format!("{name}: {value}\r\n"))
                        .collect::<String>();
                    let header = match response.content_length {
                        Some(length) => format!(
                            "HTTP/1.1 {}\r\nContent-Length: {length}\r\n{extra_headers}Connection: close\r\n\r\n",
                            response.status
                        ),
                        None => {
                            format!(
                                "HTTP/1.1 {}\r\n{extra_headers}Connection: close\r\n\r\n",
                                response.status
                            )
                        }
                    };
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(&response.body).await;
                });
            }
        });

        format!("http://{address}")
    }

    /// Announces `declared_chunks` copies of `chunk` and then delivers `sent_chunks` of them,
    /// one every `gap`. Equal counts mirror a slow but healthy transfer; a `sent_chunks` of zero
    /// mirrors a server that answers and then goes silent.
    pub(crate) async fn spawn_dripping_http_fixture(
        declared_chunks: usize,
        sent_chunks: usize,
        chunk: &'static [u8],
        gap: std::time::Duration,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("HTTP fixture should bind");
        let address = listener.local_addr().expect("HTTP fixture address");

        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = vec![0; 4096];
            let _ = socket.read(&mut buffer).await;
            let length = declared_chunks.saturating_mul(chunk.len());
            let header =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.flush().await;
            for _ in 0..sent_chunks {
                tokio::time::sleep(gap).await;
                if socket.write_all(chunk).await.is_err() || socket.flush().await.is_err() {
                    return;
                }
            }
            // Hold the connection open so a withheld body stalls the client instead of ending it.
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        });

        format!("http://{address}")
    }

    fn request_path(request: &str) -> &str {
        request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|target| target.split('?').next())
            .unwrap_or("/")
    }

    fn request_header(request: &str, header_name: &str) -> Option<String> {
        request.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case(header_name)
                .then(|| value.trim().to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use tokio::sync::Mutex;

    use super::*;
    use crate::download::test_support::{
        spawn_dripping_http_fixture, spawn_http_fixture, spawn_raw_http_fixture,
        spawn_redirect_chain_fixture, RawFixtureResponse,
    };

    #[test]
    fn user_agent_prefix_names_the_app() {
        assert_eq!(USER_AGENT_PREFIX, "VoyaVPN");
    }

    /// Byte downloads carry geo databases and rule sets that outlast any fixed whole-request
    /// deadline over a throttled link, so progress must keep the transfer alive and only silence
    /// may end it.
    #[tokio::test]
    async fn download_bytes_tolerates_a_slow_transfer_but_fails_on_a_stalled_one() {
        let read_timeout = Duration::from_millis(400);
        let slow = spawn_dripping_http_fixture(8, 8, b"chunk", Duration::from_millis(80)).await;

        let response = DownloadClient::with_read_timeout_for_tests(read_timeout)
            .download_bytes(DownloadRequest::direct(format!("{slow}/asset.dat")))
            .await
            .expect("a transfer that keeps making progress should not be cut off");

        assert_eq!(response.body, b"chunk".repeat(8));

        let stalled = spawn_dripping_http_fixture(8, 0, b"chunk", Duration::from_millis(80)).await;
        let error = DownloadClient::with_read_timeout_for_tests(read_timeout)
            .download_bytes(DownloadRequest::direct(format!("{stalled}/asset.dat")))
            .await
            .expect_err("a stalled transfer should fail on the read timeout");

        assert!(
            matches!(error, DownloadError::AttemptsFailed { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn empty_responses_are_recognised_without_matching_the_message_text() {
        let empty = DownloadError::AttemptsFailed {
            url: "https://sub.example.test/link".to_string(),
            attempts: vec![DownloadAttempt {
                url: "https://sub.example.test/link".to_string(),
                via_proxy: false,
                bytes: 0,
                error: Some(EMPTY_RESPONSE_ATTEMPT_ERROR.to_string()),
            }],
        };
        assert!(empty.is_empty_response());

        let refused = DownloadError::AttemptsFailed {
            url: "https://sub.example.test/link".to_string(),
            attempts: vec![DownloadAttempt {
                url: "https://sub.example.test/link".to_string(),
                via_proxy: false,
                bytes: 0,
                error: Some("connection refused".to_string()),
            }],
        };
        assert!(!refused.is_empty_response());

        assert!(!DownloadError::AttemptsFailed {
            url: "https://sub.example.test/link".to_string(),
            attempts: Vec::new(),
        }
        .is_empty_response());
    }

    #[tokio::test]
    async fn empty_body_downloads_report_an_empty_response_attempt() {
        let base = spawn_http_fixture(
            HashMap::from([("/sub".to_string(), String::new())]),
            1,
            Arc::new(Mutex::new(Vec::new())),
        )
        .await;

        let error = DownloadClient::new()
            .download_text(DownloadRequest::direct(format!("{base}/sub")))
            .await
            .expect_err("an empty body should fail the download");

        assert!(error.is_empty_response(), "{error:?}");
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
    async fn download_text_captures_allowlisted_headers_case_insensitively() {
        let base = spawn_raw_http_fixture(
            HashMap::from([(
                "/sub".to_string(),
                RawFixtureResponse {
                    status: "200 OK".to_string(),
                    content_length: Some(4),
                    extra_headers: vec![
                        (
                            "Subscription-Userinfo".to_string(),
                            "upload=1; download=2; total=3; expire=4".to_string(),
                        ),
                        ("PROFILE-TITLE".to_string(), "Demo".to_string()),
                        ("x-secret-header".to_string(), "must-not-leak".to_string()),
                    ],
                    body: b"body".to_vec(),
                },
            )]),
            1,
        )
        .await;

        let response = DownloadClient::new()
            .download_text(DownloadRequest::direct(format!("{base}/sub")))
            .await
            .expect("download should succeed");

        assert_eq!(
            response.headers,
            vec![
                (
                    "subscription-userinfo".to_string(),
                    "upload=1; download=2; total=3; expire=4".to_string()
                ),
                ("profile-title".to_string(), "Demo".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn download_text_rejects_declared_response_above_limit() {
        let base = spawn_raw_http_fixture(
            HashMap::from([(
                "/oversize".to_string(),
                RawFixtureResponse {
                    status: "200 OK".to_string(),
                    content_length: Some(6),
                    extra_headers: Vec::new(),
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
                    extra_headers: Vec::new(),
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

    #[test]
    fn download_clients_build_with_webpki_and_native_trust_roots() {
        build_http_client(None).expect("direct client trusts webpki and native roots");
        build_http_client(Some("socks5://127.0.0.1:1080"))
            .expect("proxy client trusts webpki and native roots");
    }

    #[tokio::test]
    async fn download_rejects_redirect_chain_above_limit() {
        let base = spawn_redirect_chain_fixture(redirect::HTTP_REDIRECT_LIMIT + 1).await;

        let error = DownloadClient::new()
            .download_text(DownloadRequest::direct(format!("{base}/r0")))
            .await
            .expect_err("redirect chain above limit should fail");

        match error {
            DownloadError::AttemptsFailed { attempts, .. } => {
                assert!(
                    attempts.iter().any(|attempt| attempt
                        .error
                        .as_deref()
                        .is_some_and(|error| error.contains("error following redirect"))),
                    "{attempts:?}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
