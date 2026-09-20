use std::{collections::BTreeMap, fmt};

use futures_util::StreamExt;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        handshake::client::Request as HandshakeRequest,
        http::{header::AUTHORIZATION, HeaderValue},
        Message,
    },
    MaybeTlsStream, WebSocketStream,
};

mod rest;
pub use rest::{
    ClashHttpMethod, ClashHttpRequest, ClashHttpTransport, ClashRestClient,
    ReqwestClashHttpTransport,
};

pub type Result<T> = std::result::Result<T, ClashError>;

#[derive(Debug, Error)]
pub enum ClashError {
    #[error("Clash request failed: {0}")]
    Request(String),
    /// The core answered with a non-success HTTP status. Kept apart from
    /// `Request` because callers branch on it: a delay test reports a missing
    /// outbound as 404, a timeout as 504 and an unreachable node as 503.
    #[error("Clash request returned HTTP {0}")]
    Status(u16),
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

#[derive(Clone, PartialEq, Eq)]
pub struct ClashApiEndpoint {
    pub host: String,
    pub port: u16,
    pub secret: Option<String>,
}

impl fmt::Debug for ClashApiEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClashApiEndpoint")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("secret", &redacted(self.secret.as_deref()))
            .finish()
    }
}

impl ClashApiEndpoint {
    #[must_use]
    pub fn loopback(port: u16) -> Self {
        Self {
            host: voya_core::LOOPBACK.to_string(),
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
    fn ws_url(&self, path_and_query: &str) -> String {
        format!(
            "ws://{}:{}{}",
            normalize_host(&self.host),
            self.port,
            normalize_path(path_and_query)
        )
    }
}

/// Keeps Clash API secrets out of `Debug` output and therefore out of tracing fields.
fn redacted(secret: Option<&str>) -> &'static str {
    match secret {
        Some(secret) if !secret.is_empty() => "<redacted>",
        _ => "None",
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
        let (stream, _) = connect_async(self.upgrade_request(resource)?)
            .await
            .map_err(|error| ClashError::WebSocket(error.to_string()))?;

        Ok(ClashWebSocketSession { resource, stream })
    }

    /// Builds the upgrade request, authenticating it the same way the REST transport does.
    ///
    /// sing-box rejects `/traffic` and `/connections` upgrades with 401 when the Clash API is
    /// configured with a secret and the request carries no bearer token.
    fn upgrade_request(&self, resource: ClashWebSocketResource) -> Result<HandshakeRequest> {
        let mut request = self
            .url(resource)
            .into_client_request()
            .map_err(|error| ClashError::WebSocket(error.to_string()))?;
        if let Some(secret) = self
            .endpoint
            .secret
            .as_deref()
            .filter(|secret| !secret.is_empty())
        {
            let mut value = HeaderValue::from_str(&format!("Bearer {secret}"))
                .map_err(|error| ClashError::WebSocket(error.to_string()))?;
            value.set_sensitive(true);
            request.headers_mut().insert(AUTHORIZATION, value);
        }

        Ok(request)
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
#[serde(default)]
pub struct ClashProxiesResponse {
    pub proxies: BTreeMap<String, ClashProxy>,
}

/// One outbound as the Clash API reports it; groups also carry `all` and `now`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct ClashProxy {
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub all: Vec<String>,
    pub now: Option<String>,
    pub history: Vec<ClashHistoryItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct ClashHistoryItem {
    pub time: String,
    pub delay: u32,
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
    pub uid: Option<Value>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub process: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_string_lossy")]
    pub process_path: Option<String>,
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

fn decode_traffic_message(source: &str) -> Option<ClashTraffic> {
    serde_json::from_str(source).ok()
}

fn decode_connections_message(source: &str) -> Option<ClashConnections> {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::{future::Future, pin::Pin};

    use serde_json::json;

    use super::rest::CLASH_HTTP_RESPONSE_LIMIT_BYTES;

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
        fn send<'transport>(
            &'transport self,
            request: ClashHttpRequest,
        ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'transport>> {
            Box::pin(async move {
                self.requests
                    .lock()
                    .expect("requests lock")
                    .push(request.clone());
                self.responses
                    .lock()
                    .expect("responses lock")
                    .get(&request.url)
                    .map(Value::to_string)
                    .ok_or_else(|| ClashError::Request(format!("no response for {}", request.url)))
            })
        }
    }

    #[tokio::test]
    async fn clash_group_calls_encode_tags_and_decode_members() {
        let transport = MockTransport::default();
        transport.respond(
            "/proxies",
            json!({
                "proxies": {
                    "proxy": {
                        "name": "proxy",
                        "type": "Selector",
                        "all": ["Tokyo [a1]", "Osaka [b2]"],
                        "now": "Osaka [b2]",
                        "history": []
                    },
                    "Osaka [b2]": {
                        "name": "Osaka [b2]",
                        "type": "Socks",
                        "history": [{ "time": "2026-09-13T00:00:00Z", "delay": 88 }]
                    }
                }
            }),
        );
        transport.respond("/proxies/proxy", Value::Null);
        transport.respond(
            "/group/proxy/delay?url=https%3A%2F%2Fprobe.example%2Fgenerate_204%3Fa%3D1%26b%3D2&timeout=5000",
            json!({ "Tokyo [a1]": 120 }),
        );
        let client =
            ClashRestClient::with_transport(ClashApiEndpoint::loopback(9090), transport.clone());

        let proxies = client.get_proxies().await.expect("proxies");
        assert_eq!(proxies.proxies["proxy"].all, ["Tokyo [a1]", "Osaka [b2]"]);
        assert_eq!(proxies.proxies["proxy"].now.as_deref(), Some("Osaka [b2]"));
        assert_eq!(proxies.proxies["Osaka [b2]"].history[0].delay, 88);

        client
            .select_proxy("proxy", "Tokyo [a1]")
            .await
            .expect("select");
        let delays = client
            .group_delay("proxy", "https://probe.example/generate_204?a=1&b=2", 5_000)
            .await
            .expect("group delay");
        assert_eq!(delays.get("Tokyo [a1]"), Some(&120));

        let requests = transport.requests();
        let select = requests
            .iter()
            .find(|request| request.url.ends_with("/proxies/proxy"))
            .expect("select request");
        assert_eq!(select.method, ClashHttpMethod::Put);
        assert_eq!(select.body, Some(json!({ "name": "Tokyo [a1]" })));
    }

    #[tokio::test]
    async fn clash_proxy_delay_encodes_the_tag_and_url() {
        let transport = MockTransport::default();
        transport.respond(
            "/proxies/probe:node%2F1:00ff/delay?url=https%3A%2F%2Fprobe.example%2Fgenerate_204&timeout=4000",
            json!({ "delay": 142 }),
        );
        let client =
            ClashRestClient::with_transport(ClashApiEndpoint::loopback(9090), transport.clone());

        let delay = client
            .proxy_delay(
                "probe:node/1:00ff",
                "https://probe.example/generate_204",
                4_000,
            )
            .await
            .expect("proxy delay");

        assert_eq!(delay, 142);
        assert_eq!(transport.requests()[0].method, ClashHttpMethod::Get);
    }

    #[tokio::test]
    async fn clash_reqwest_transport_reports_the_status_of_a_failed_request() {
        let port = spawn_clash_http_response("/other", "200 OK", Some(2), b"{}".to_vec()).await;
        let client = ClashRestClient::new(ClashApiEndpoint::loopback(port));

        let error = client
            .proxy_delay("probe:missing:00", "https://probe.example/", 1_000)
            .await
            .expect_err("unknown outbound");

        assert!(matches!(error, ClashError::Status(404)), "{error:?}");
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
    async fn clash_reqwest_transport_reads_small_json_under_limit() {
        let port = spawn_clash_http_response(
            "/connections",
            "200 OK",
            Some(r#"{"connections":[]}"#.len()),
            br#"{"connections":[]}"#.to_vec(),
        )
        .await;
        let client = ClashRestClient::new(ClashApiEndpoint::loopback(port));

        let response = client.get_connections().await.expect("connections");

        assert!(response.connections.is_empty());
    }

    #[tokio::test]
    async fn clash_reqwest_transport_rejects_declared_response_above_limit() {
        let declared_length = CLASH_HTTP_RESPONSE_LIMIT_BYTES + 1;
        let port = spawn_clash_http_response(
            "/connections",
            "200 OK",
            Some(declared_length),
            br#"{"connections":[]}"#.to_vec(),
        )
        .await;
        let client = ClashRestClient::new(ClashApiEndpoint::loopback(port));

        let error = client
            .get_connections()
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

    #[test]
    fn clash_websocket_upgrade_request_carries_the_endpoint_secret() {
        let client = ClashWebSocketClient::new(secret_endpoint(9090));

        let request = client
            .upgrade_request(ClashWebSocketResource::Traffic)
            .expect("upgrade request");

        assert_eq!(request.uri().to_string(), "ws://127.0.0.1:9090/traffic");
        assert_eq!(
            request
                .headers()
                .get(AUTHORIZATION)
                .map(|value| value.to_str().expect("header is ASCII")),
            Some("Bearer s3cret")
        );
    }

    #[test]
    fn clash_websocket_upgrade_request_omits_missing_or_empty_secret() {
        for secret in [None, Some(String::new())] {
            let client = ClashWebSocketClient::new(ClashApiEndpoint {
                host: "127.0.0.1".to_string(),
                port: 9090,
                secret,
            });

            let request = client
                .upgrade_request(ClashWebSocketResource::Connections)
                .expect("upgrade request");

            assert!(request.headers().get(AUTHORIZATION).is_none());
        }
    }

    #[tokio::test]
    async fn clash_websocket_connect_sends_the_authorization_header() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("address").port();
        let upgrade = tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return String::new();
            };
            let mut request = Vec::new();
            let mut buffer = vec![0u8; 512];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                match socket.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => request.extend_from_slice(&buffer[..read]),
                }
            }
            String::from_utf8_lossy(&request).into_owned()
        });

        // `ClashWebSocketSession` is deliberately not `Debug` (it owns the live
        // stream), so unwrap the error by hand instead of `expect_err`.
        let error = match ClashWebSocketClient::new(secret_endpoint(port))
            .connect(ClashWebSocketResource::Connections)
            .await
        {
            Ok(_) => panic!("fixture closes without completing the handshake"),
            Err(error) => error,
        };
        let request = upgrade.await.expect("upgrade task");

        assert!(matches!(error, ClashError::WebSocket(_)), "{error:?}");
        assert!(
            request.starts_with("GET /connections HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer s3cret\r\n"),
            "{request}"
        );
    }

    #[test]
    fn clash_secrets_are_redacted_in_debug_output() {
        let endpoint = secret_endpoint(9090);
        let request = ClashHttpRequest {
            method: ClashHttpMethod::Get,
            url: endpoint.http_url("/connections"),
            body: None,
            bearer_token: endpoint.secret.clone(),
        };

        let endpoint_debug = format!("{endpoint:?}");
        let request_debug = format!("{request:?}");

        assert!(!endpoint_debug.contains("s3cret"), "{endpoint_debug}");
        assert!(endpoint_debug.contains("<redacted>"), "{endpoint_debug}");
        assert!(!request_debug.contains("s3cret"), "{request_debug}");
        assert!(request_debug.contains("<redacted>"), "{request_debug}");
        assert!(
            format!("{:?}", ClashApiEndpoint::loopback(9090)).contains("secret: \"None\""),
            "an endpoint without a secret keeps reporting that it has none"
        );
    }

    fn secret_endpoint(port: u16) -> ClashApiEndpoint {
        ClashApiEndpoint {
            host: "127.0.0.1".to_string(),
            port,
            secret: Some("s3cret".to_string()),
        }
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
