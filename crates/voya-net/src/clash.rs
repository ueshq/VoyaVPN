use std::{collections::BTreeMap, future::Future, pin::Pin};

use futures_util::StreamExt;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::Method;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

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
const QUERY_VALUE_ENCODE_SET: &AsciiSet = &PATH_SEGMENT_ENCODE_SET
    .add(b'&')
    .add(b'+')
    .add(b':')
    .add(b'=');
const CLASH_HTTP_RESPONSE_LIMIT_BYTES: usize = 16 * 1024 * 1024;

pub type Result<T> = std::result::Result<T, ClashError>;

#[derive(Debug, Error)]
pub enum ClashError {
    #[error("Clash request failed: {0}")]
    Request(String),
    #[error(
        "Clash response body too large for {url}: limit {limit} bytes, content length {content_length:?}, received {received}"
    )]
    ResponseTooLarge {
        url: String,
        limit: usize,
        content_length: Option<u64>,
        received: usize,
    },
    #[error("Clash response decode failed: {0}")]
    Decode(String),
    #[error("Clash websocket failed: {0}")]
    WebSocket(String),
    #[error("Clash websocket closed")]
    WebSocketClosed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClashApiEndpoint {
    pub host: String,
    pub port: u16,
    pub secret: Option<String>,
}

impl ClashApiEndpoint {
    #[must_use]
    pub fn loopback(port: u16) -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port,
            secret: None,
        }
    }

    #[must_use]
    pub fn http_url(&self, path_and_query: &str) -> String {
        format!(
            "http://{}:{}{}",
            normalize_host(&self.host),
            self.port,
            normalize_path(path_and_query)
        )
    }

    #[must_use]
    pub fn ws_url(&self, path_and_query: &str) -> String {
        format!(
            "ws://{}:{}{}",
            normalize_host(&self.host),
            self.port,
            normalize_path(path_and_query)
        )
    }
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct ClashHttpRequest {
    pub method: ClashHttpMethod,
    pub url: String,
    pub body: Option<Value>,
    pub bearer_token: Option<String>,
}

pub trait ClashHttpTransport: Clone + Send + Sync + 'static {
    fn send_json<'transport>(
        &'transport self,
        request: ClashHttpRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'transport>>;
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
    fn send_json<'transport>(
        &'transport self,
        request: ClashHttpRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'transport>> {
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
                .map_err(|error| ClashError::Request(error.to_string()))?;

            let text = crate::read_response_text_limited(response, CLASH_HTTP_RESPONSE_LIMIT_BYTES)
                .await
                .map_err(|error| clash_body_error(&request.url, error))?;
            if text.trim().is_empty() {
                Ok(Value::Null)
            } else {
                serde_json::from_str(&text).map_err(|error| ClashError::Decode(error.to_string()))
            }
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

    #[must_use]
    pub fn endpoint(&self) -> &ClashApiEndpoint {
        &self.endpoint
    }

    pub async fn get_proxies(&self) -> Result<ClashProxiesResponse> {
        self.request(ClashHttpMethod::Get, "/proxies", None).await
    }

    pub async fn get_proxy_providers(&self) -> Result<ClashProvidersResponse> {
        self.request(ClashHttpMethod::Get, "/providers/proxies", None)
            .await
    }

    pub async fn get_connections(&self) -> Result<ClashConnections> {
        self.request(ClashHttpMethod::Get, "/connections", None)
            .await
    }

    pub async fn delay_proxy(
        &self,
        proxy_name: &str,
        timeout_ms: u32,
        test_url: &str,
    ) -> Result<ClashDelayResponse> {
        let path = format!(
            "/proxies/{}/delay?timeout={timeout_ms}&url={}",
            encode_segment(proxy_name),
            encode_query_value(test_url)
        );

        self.request(ClashHttpMethod::Get, &path, None).await
    }

    pub async fn select_proxy(&self, group_name: &str, proxy_name: &str) -> Result<()> {
        let path = format!("/proxies/{}", encode_segment(group_name));
        self.request_value(
            ClashHttpMethod::Put,
            &path,
            Some(json!({
                "name": proxy_name,
            })),
        )
        .await
        .map(drop)
    }

    pub async fn patch_configs(&self, body: Value) -> Result<()> {
        self.request_value(ClashHttpMethod::Patch, "/configs", Some(body))
            .await
            .map(drop)
    }

    pub async fn set_rule_mode(&self, mode: &str) -> Result<()> {
        self.patch_configs(json!({ "mode": mode })).await
    }

    pub async fn reload_config(&self, path: Option<&str>) -> Result<()> {
        let body = path
            .filter(|path| !path.trim().is_empty())
            .map(|path| json!({ "path": path }))
            .unwrap_or_else(|| json!({}));

        self.request_value(ClashHttpMethod::Put, "/configs?force=true", Some(body))
            .await
            .map(drop)
    }

    pub async fn close_connection(&self, connection_id: Option<&str>) -> Result<()> {
        let path = connection_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| format!("/connections/{}", encode_segment(id)))
            .unwrap_or_else(|| "/connections".to_string());

        self.request_value(ClashHttpMethod::Delete, &path, None)
            .await
            .map(drop)
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
        let value = self.request_value(method, path_and_query, body).await?;
        serde_json::from_value(value).map_err(|error| ClashError::Decode(error.to_string()))
    }

    async fn request_value(
        &self,
        method: ClashHttpMethod,
        path_and_query: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        self.transport
            .send_json(ClashHttpRequest {
                method,
                url: self.endpoint.http_url(path_and_query),
                body,
                bearer_token: self.endpoint.secret.clone(),
            })
            .await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClashWebSocketResource {
    Traffic,
    Connections,
}

#[derive(Debug)]
pub struct ClashWebSocketClient {
    endpoint: ClashApiEndpoint,
}

impl ClashWebSocketClient {
    #[must_use]
    pub fn new(endpoint: ClashApiEndpoint) -> Self {
        Self { endpoint }
    }

    #[must_use]
    pub fn url(&self, resource: ClashWebSocketResource) -> String {
        self.endpoint.ws_url(match resource {
            ClashWebSocketResource::Traffic => "/traffic",
            ClashWebSocketResource::Connections => "/connections",
        })
    }

    pub async fn connect(&self, resource: ClashWebSocketResource) -> Result<ClashWebSocketSession> {
        let (stream, _) = connect_async(self.url(resource))
            .await
            .map_err(|error| ClashError::WebSocket(error.to_string()))?;

        Ok(ClashWebSocketSession { resource, stream })
    }
}

pub struct ClashWebSocketSession {
    resource: ClashWebSocketResource,
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl ClashWebSocketSession {
    pub async fn next_event(&mut self) -> Result<ClashWebSocketEvent> {
        loop {
            let Some(message) = self.stream.next().await else {
                return Err(ClashError::WebSocketClosed);
            };
            let message = message.map_err(|error| ClashError::WebSocket(error.to_string()))?;
            match message {
                Message::Text(text) => return decode_ws_event(self.resource, &text),
                Message::Binary(bytes) => {
                    let text = String::from_utf8(bytes.to_vec())
                        .map_err(|error| ClashError::Decode(error.to_string()))?;
                    return decode_ws_event(self.resource, &text);
                }
                Message::Close(_) => return Err(ClashError::WebSocketClosed),
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashProxiesResponse {
    pub proxies: BTreeMap<String, ClashProxy>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashProxy {
    pub all: Vec<String>,
    pub history: Vec<ClashHistoryItem>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub udp: bool,
    pub now: Option<String>,
    pub delay: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashHistoryItem {
    pub time: String,
    pub delay: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashProvidersResponse {
    pub providers: BTreeMap<String, ClashProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashProvider {
    pub name: Option<String>,
    pub proxies: Vec<ClashProxy>,
    #[serde(rename = "type")]
    pub provider_type: Option<String>,
    pub vehicle_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashDelayResponse {
    pub delay: Option<i32>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashConnections {
    #[serde(deserialize_with = "deserialize_u64_lossy")]
    pub download_total: u64,
    #[serde(deserialize_with = "deserialize_u64_lossy")]
    pub upload_total: u64,
    #[serde(deserialize_with = "deserialize_connections_lossy")]
    pub connections: Vec<ClashConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashConnection {
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub id: Option<String>,
    #[serde(deserialize_with = "deserialize_metadata_lossy")]
    pub metadata: ClashConnectionMetadata,
    #[serde(deserialize_with = "deserialize_u64_lossy")]
    pub upload: u64,
    #[serde(deserialize_with = "deserialize_u64_lossy")]
    pub download: u64,
    #[serde(deserialize_with = "deserialize_string_lossy")]
    pub start: String,
    #[serde(deserialize_with = "deserialize_string_vec_lossy")]
    pub chains: Vec<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub rule: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub rule_payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClashConnectionMetadata {
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub network: Option<String>,
    #[serde(
        rename = "type",
        deserialize_with = "deserialize_optional_string_lossy"
    )]
    pub metadata_type: Option<String>,
    #[serde(
        rename = "sourceIP",
        alias = "sourceIp",
        deserialize_with = "deserialize_optional_string_lossy"
    )]
    pub source_ip: Option<String>,
    #[serde(
        rename = "destinationIP",
        alias = "destinationIp",
        deserialize_with = "deserialize_optional_string_lossy"
    )]
    pub destination_ip: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub source_port: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub destination_port: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub host: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub ns_mode: Option<String>,
    pub uid: Option<Value>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub process: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub process_path: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub remote_destination: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ClashTraffic {
    #[serde(alias = "up", deserialize_with = "deserialize_u64_lossy")]
    pub up: u64,
    #[serde(alias = "down", deserialize_with = "deserialize_u64_lossy")]
    pub down: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClashWebSocketEvent {
    Traffic(ClashTraffic),
    Connections(ClashConnections),
}

#[must_use]
pub fn decode_traffic_message(source: &str) -> Option<ClashTraffic> {
    serde_json::from_str(source).ok()
}

#[must_use]
pub fn decode_connections_message(source: &str) -> Option<ClashConnections> {
    serde_json::from_str(source).ok()
}

fn decode_ws_event(resource: ClashWebSocketResource, source: &str) -> Result<ClashWebSocketEvent> {
    match resource {
        ClashWebSocketResource::Traffic => decode_traffic_message(source)
            .map(ClashWebSocketEvent::Traffic)
            .ok_or_else(|| ClashError::Decode("invalid Clash traffic event".to_string())),
        ClashWebSocketResource::Connections => decode_connections_message(source)
            .map(ClashWebSocketEvent::Connections)
            .ok_or_else(|| ClashError::Decode("invalid Clash connections event".to_string())),
    }
}

fn deserialize_connections_lossy<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<ClashConnection>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer).map(|value| match value {
        Some(Value::Array(items)) => items
            .into_iter()
            .enumerate()
            .filter_map(
                |(index, item)| match serde_json::from_value::<ClashConnection>(item) {
                    Ok(connection) => Some(connection),
                    Err(error) => {
                        tracing::debug!(
                            index,
                            error = %error,
                            "dropping malformed Clash connection"
                        );
                        None
                    }
                },
            )
            .collect(),
        _ => Vec::new(),
    })
}

fn deserialize_metadata_lossy<'de, D>(
    deserializer: D,
) -> std::result::Result<ClashConnectionMetadata, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer).map(|value| {
        value
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default()
    })
}

fn deserialize_optional_string_lossy<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer).map(|value| value.and_then(value_to_string))
}

fn deserialize_string_lossy<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_string_lossy(deserializer).map(|value| value.unwrap_or_default())
}

fn deserialize_u64_lossy<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer).map(|value| {
        value
            .and_then(|value| match value {
                Value::Number(number) => number
                    .as_u64()
                    .or_else(|| number.as_i64().and_then(|value| u64::try_from(value).ok()))
                    .or_else(|| number.as_f64().and_then(f64_to_u64)),
                Value::String(value) => parse_u64_string(&value),
                _ => None,
            })
            .unwrap_or_default()
    })
}

fn deserialize_string_vec_lossy<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Value>::deserialize(deserializer).map(|value| match value {
        Some(Value::Array(items)) => items.into_iter().filter_map(value_to_string).collect(),
        Some(value) => value_to_string(value).into_iter().collect(),
        None => Vec::new(),
    })
}

fn value_to_string(value: Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

fn parse_u64_string(value: &str) -> Option<u64> {
    let value = value.trim();
    value
        .parse::<u64>()
        .ok()
        .or_else(|| value.parse::<f64>().ok().and_then(f64_to_u64))
}

fn f64_to_u64(value: f64) -> Option<u64> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| *value >= 0.0 && *value <= u64::MAX as f64)
        .map(|value| value.trunc() as u64)
}

fn normalize_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn normalize_path(path_and_query: &str) -> String {
    if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{path_and_query}")
    }
}

fn encode_segment(value: &str) -> String {
    utf8_percent_encode(value, PATH_SEGMENT_ENCODE_SET).to_string()
}

fn encode_query_value(value: &str) -> String {
    utf8_percent_encode(value, QUERY_VALUE_ENCODE_SET).to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    #[derive(Clone, Default)]
    struct MockTransport {
        requests: Arc<Mutex<Vec<ClashHttpRequest>>>,
        responses: Arc<Mutex<BTreeMap<String, Value>>>,
    }

    impl MockTransport {
        fn respond(&self, path: &str, value: Value) {
            self.responses
                .lock()
                .expect("responses lock")
                .insert(format!("http://127.0.0.1:9090{path}"), value);
        }

        fn requests(&self) -> Vec<ClashHttpRequest> {
            self.requests.lock().expect("requests lock").clone()
        }
    }

    impl ClashHttpTransport for MockTransport {
        fn send_json<'transport>(
            &'transport self,
            request: ClashHttpRequest,
        ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'transport>> {
            Box::pin(async move {
                self.requests
                    .lock()
                    .expect("requests lock")
                    .push(request.clone());
                self.responses
                    .lock()
                    .expect("responses lock")
                    .get(&request.url)
                    .cloned()
                    .ok_or_else(|| ClashError::Request(format!("no response for {}", request.url)))
            })
        }
    }

    #[tokio::test]
    async fn clash_rule_mode_uses_patch_configs() {
        let transport = MockTransport::default();
        transport.respond("/configs", Value::Null);
        let client =
            ClashRestClient::with_transport(ClashApiEndpoint::loopback(9090), transport.clone());

        client
            .set_rule_mode("direct")
            .await
            .expect("rule mode patch");

        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, ClashHttpMethod::Patch);
        assert_eq!(requests[0].url, "http://127.0.0.1:9090/configs");
        assert_eq!(requests[0].body, Some(json!({ "mode": "direct" })));
    }

    #[tokio::test]
    async fn clash_reload_uses_force_query() {
        let transport = MockTransport::default();
        transport.respond("/configs?force=true", Value::Null);
        let client =
            ClashRestClient::with_transport(ClashApiEndpoint::loopback(9090), transport.clone());

        client
            .reload_config(Some("/tmp/config.yaml"))
            .await
            .expect("reload");

        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, ClashHttpMethod::Put);
        assert_eq!(requests[0].url, "http://127.0.0.1:9090/configs?force=true");
        assert_eq!(
            requests[0].body,
            Some(json!({ "path": "/tmp/config.yaml" }))
        );
    }

    #[tokio::test]
    async fn clash_delay_test_encodes_proxy_name_and_url() {
        let transport = MockTransport::default();
        transport.respond(
            "/proxies/HK%20%2F%201/delay?timeout=10000&url=https%3A%2F%2Fexample.com%2Fgenerate_204",
            json!({ "delay": 42 }),
        );
        let client =
            ClashRestClient::with_transport(ClashApiEndpoint::loopback(9090), transport.clone());

        let delay = client
            .delay_proxy("HK / 1", 10_000, "https://example.com/generate_204")
            .await
            .expect("delay");

        assert_eq!(delay.delay, Some(42));
        let requests = transport.requests();
        assert_eq!(requests[0].method, ClashHttpMethod::Get);
    }

    #[tokio::test]
    async fn clash_reqwest_transport_reads_small_json_under_limit() {
        let port = spawn_clash_http_response(
            "/proxies",
            "200 OK",
            Some(r#"{"proxies":{}}"#.len()),
            br#"{"proxies":{}}"#.to_vec(),
        )
        .await;
        let client = ClashRestClient::new(ClashApiEndpoint::loopback(port));

        let response = client.get_proxies().await.expect("proxies");

        assert!(response.proxies.is_empty());
    }

    #[tokio::test]
    async fn clash_reqwest_transport_rejects_declared_response_above_limit() {
        let declared_length = CLASH_HTTP_RESPONSE_LIMIT_BYTES + 1;
        let port = spawn_clash_http_response(
            "/proxies",
            "200 OK",
            Some(declared_length),
            br#"{"proxies":{}}"#.to_vec(),
        )
        .await;
        let client = ClashRestClient::new(ClashApiEndpoint::loopback(port));

        let error = client
            .get_proxies()
            .await
            .expect_err("oversized Clash response should fail");

        match error {
            ClashError::ResponseTooLarge {
                limit,
                content_length,
                received,
                ..
            } => {
                assert_eq!(limit, CLASH_HTTP_RESPONSE_LIMIT_BYTES);
                assert_eq!(
                    content_length,
                    Some(u64::try_from(declared_length).expect("declared length"))
                );
                assert_eq!(received, 0);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn clash_websocket_decodes_traffic_and_connections() {
        let traffic = decode_traffic_message(r#"{ "Up": 12, "Down": 34 }"#).expect("traffic event");
        assert_eq!(traffic, ClashTraffic { up: 12, down: 34 });

        let connections = decode_connections_message(
            r#"{
                "downloadTotal": 100,
                "uploadTotal": 50,
                "connections": [{
                    "id": "abc",
                    "metadata": {
                        "network": "tcp",
                        "type": "HTTP",
                        "sourceIP": "127.0.0.1",
                        "destinationIP": "93.184.216.34",
                        "destinationPort": "443",
                        "host": "example.com"
                    },
                    "upload": 1,
                    "download": 2,
                    "start": "2026-06-01T00:00:00Z",
                    "chains": ["proxy"],
                    "rule": "MATCH"
                }]
            }"#,
        )
        .expect("connections event");

        assert_eq!(connections.download_total, 100);
        assert_eq!(
            connections.connections[0].metadata.host.as_deref(),
            Some("example.com")
        );
        assert_eq!(
            connections.connections[0]
                .metadata
                .destination_ip
                .as_deref(),
            Some("93.184.216.34")
        );
    }

    #[test]
    fn clash_connections_decode_lossy_runtime_field_variants() {
        let connections = decode_connections_message(
            r#"{
                "downloadTotal": "2048",
                "uploadTotal": 50.9,
                "connections": [{
                    "id": 42,
                    "metadata": {
                        "network": "tcp",
                        "type": null,
                        "sourceIP": "127.0.0.1",
                        "destinationIP": "93.184.216.34",
                        "sourcePort": 61558,
                        "destinationPort": 443,
                        "host": 12345,
                        "process": 6789
                    },
                    "upload": "12",
                    "download": null,
                    "start": null,
                    "chains": ["proxy", 1, null],
                    "rule": null,
                    "rulePayload": 99
                }, {
                    "metadata": null,
                    "chains": null
                }, "malformed"]
            }"#,
        )
        .expect("connections event");

        assert_eq!(connections.download_total, 2048);
        assert_eq!(connections.upload_total, 50);
        assert_eq!(connections.connections.len(), 2);

        let first = &connections.connections[0];
        assert_eq!(first.id.as_deref(), Some("42"));
        assert_eq!(first.upload, 12);
        assert_eq!(first.download, 0);
        assert_eq!(first.start, "");
        assert_eq!(first.chains, vec!["proxy".to_string(), "1".to_string()]);
        assert_eq!(first.rule_payload.as_deref(), Some("99"));
        assert_eq!(first.metadata.source_port.as_deref(), Some("61558"));
        assert_eq!(first.metadata.destination_port.as_deref(), Some("443"));
        assert_eq!(first.metadata.host.as_deref(), Some("12345"));
        assert_eq!(first.metadata.process.as_deref(), Some("6789"));

        assert_eq!(
            connections.connections[1].metadata,
            ClashConnectionMetadata::default()
        );
        assert!(connections.connections[1].chains.is_empty());
    }

    #[test]
    fn clash_websocket_client_builds_resource_urls() {
        let endpoint = ClashApiEndpoint::loopback(9090);
        let client = ClashWebSocketClient::new(endpoint);

        assert_eq!(
            client.url(ClashWebSocketResource::Traffic),
            "ws://127.0.0.1:9090/traffic"
        );
        assert_eq!(
            client.url(ClashWebSocketResource::Connections),
            "ws://127.0.0.1:9090/connections"
        );
    }

    async fn spawn_clash_http_response(
        expected_path: &str,
        status: &str,
        content_length: Option<usize>,
        body: Vec<u8>,
    ) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("address").port();
        let expected_path = expected_path.to_string();
        let status = status.to_string();

        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = vec![0; 4096];
            let bytes_read = socket.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..bytes_read]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|target| target.split('?').next())
                .unwrap_or("/");
            let (status, body, content_length) = if path == expected_path {
                (status, body, content_length)
            } else {
                ("404 Not Found".to_string(), b"not found".to_vec(), Some(9))
            };
            let header = match content_length {
                Some(length) => {
                    format!("HTTP/1.1 {status}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n")
                }
                None => format!("HTTP/1.1 {status}\r\nConnection: close\r\n\r\n"),
            };
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.write_all(&body).await;
        });

        port
    }
}
