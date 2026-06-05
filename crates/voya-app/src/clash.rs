use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use tokio::{
    runtime::Handle,
    sync::watch,
    task::JoinHandle,
    time::{self, MissedTickBehavior},
};
use voya_core::{AppConfig, RuleMode};
use voya_net::clash::{
    ClashApiEndpoint, ClashConnection as NetClashConnection,
    ClashConnectionMetadata as NetClashConnectionMetadata, ClashConnections as NetClashConnections,
    ClashDelayResponse, ClashError, ClashHttpTransport, ClashProvidersResponse,
    ClashProxiesResponse, ClashProxy, ClashRestClient, ClashTraffic as NetClashTraffic,
    ClashWebSocketClient, ClashWebSocketEvent, ClashWebSocketResource, ReqwestClashHttpTransport,
};

use crate::statistics::singbox_state_port2;

const DELAY_TIMEOUT_MS: u32 = 10_000;
const CLASH_WS_RECONNECT_INTERVAL: Duration = Duration::from_secs(1);
const ALLOW_SELECT_TYPES: &[&str] = &["selector", "urltest", "loadbalance", "fallback"];
const NOT_ALLOW_TEST_TYPES: &[&str] = &[
    "selector",
    "urltest",
    "direct",
    "reject",
    "compatible",
    "pass",
    "loadbalance",
    "fallback",
];
const PROVIDER_PROXY_VEHICLE_TYPES: &[&str] = &["file", "http"];

pub type Result<T> = std::result::Result<T, ClashManagerError>;

#[derive(Debug, Error)]
pub enum ClashManagerError {
    #[error(transparent)]
    Api(#[from] ClashError),
    #[error("Clash group {0} was not found")]
    GroupNotFound(String),
    #[error("Clash proxy {0} was not found")]
    ProxyNotFound(String),
    #[error("Clash group {0} is not a selector")]
    GroupNotSelector(String),
    #[error("invalid Clash rule mode {0:?}")]
    InvalidRuleMode(RuleMode),
    #[error("Clash monitor lock is poisoned")]
    MonitorLockPoisoned,
    #[error("Clash monitor requires a Tokio runtime")]
    MonitorRuntimeUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClashProxiesSnapshot {
    pub groups: Vec<ClashProxyGroup>,
    pub all_nodes: Vec<ClashProxyNode>,
    pub rule_mode: RuleMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClashProxyGroup {
    pub name: String,
    pub proxy_type: String,
    pub now: Option<String>,
    pub nodes: Vec<ClashProxyNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClashProxyNode {
    pub name: String,
    pub proxy_type: String,
    pub delay: Option<i32>,
    pub delay_label: String,
    pub udp: bool,
    pub active: bool,
    pub testable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClashDelayTestResult {
    pub name: String,
    pub delay: Option<i32>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClashTrafficEvent {
    #[specta(type = f64)]
    pub up: u64,
    #[specta(type = f64)]
    pub down: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClashConnectionsSnapshot {
    #[specta(type = f64)]
    pub download_total: u64,
    #[specta(type = f64)]
    pub upload_total: u64,
    pub connections: Vec<ClashConnectionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClashConnectionItem {
    pub id: Option<String>,
    pub network: Option<String>,
    pub connection_type: Option<String>,
    pub host: String,
    pub source: String,
    pub destination: String,
    #[specta(type = f64)]
    pub upload: u64,
    #[specta(type = f64)]
    pub download: u64,
    pub start: String,
    pub chains: Vec<String>,
    pub rule: Option<String>,
    pub rule_payload: Option<String>,
    pub process: Option<String>,
    pub process_path: Option<String>,
}

pub trait ClashEventSink: Send + Sync {
    fn emit_traffic(&self, event: ClashTrafficEvent);
    fn emit_connections(&self, event: ClashConnectionsSnapshot);
}

#[derive(Clone)]
pub struct NoopClashEventSink;

impl ClashEventSink for NoopClashEventSink {
    fn emit_traffic(&self, _event: ClashTrafficEvent) {}
    fn emit_connections(&self, _event: ClashConnectionsSnapshot) {}
}

#[derive(Debug, Clone)]
pub struct ClashManager<T = ReqwestClashHttpTransport> {
    transport: T,
}

impl Default for ClashManager<ReqwestClashHttpTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl ClashManager<ReqwestClashHttpTransport> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            transport: ReqwestClashHttpTransport::new(),
        }
    }
}

impl<T> ClashManager<T>
where
    T: ClashHttpTransport,
{
    #[must_use]
    pub fn with_transport(transport: T) -> Self {
        Self { transport }
    }

    pub async fn proxies(&self, config: &AppConfig) -> Result<ClashProxiesSnapshot> {
        let client = self.client(config);
        let proxies = client.get_proxies().await?;
        let providers = client.get_proxy_providers().await.unwrap_or_default();

        Ok(build_proxy_snapshot(
            &proxies,
            &providers,
            config.clash_ui_item.proxies_sorting,
            config.clash_ui_item.rule_mode,
        ))
    }

    pub async fn connections(&self, config: &AppConfig) -> Result<ClashConnectionsSnapshot> {
        self.client(config)
            .get_connections()
            .await
            .map(connections_snapshot)
            .map_err(Into::into)
    }

    pub async fn select_proxy(
        &self,
        config: &AppConfig,
        group_name: &str,
        proxy_name: &str,
    ) -> Result<ClashProxiesSnapshot> {
        let client = self.client(config);
        let proxies = client.get_proxies().await?;
        let group = proxies
            .proxies
            .get(group_name)
            .ok_or_else(|| ClashManagerError::GroupNotFound(group_name.to_string()))?;
        if !group.proxy_type.eq_ignore_ascii_case("selector") {
            return Err(ClashManagerError::GroupNotSelector(group_name.to_string()));
        }
        if !group.all.iter().any(|name| name == proxy_name) {
            return Err(ClashManagerError::ProxyNotFound(proxy_name.to_string()));
        }

        client.select_proxy(group_name, proxy_name).await?;
        self.proxies(config).await
    }

    pub async fn test_delay(
        &self,
        config: &AppConfig,
        proxy_names: Vec<String>,
    ) -> Result<Vec<ClashDelayTestResult>> {
        let client = self.client(config);
        let names = if proxy_names.is_empty() {
            client
                .get_proxies()
                .await?
                .proxies
                .into_iter()
                .filter_map(|(name, proxy)| is_testable_type(&proxy.proxy_type).then_some(name))
                .collect::<Vec<_>>()
        } else {
            proxy_names
        };

        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let response = client
                .delay_proxy(
                    &name,
                    DELAY_TIMEOUT_MS,
                    &config.speed_test_item.speed_ping_test_url,
                )
                .await
                .unwrap_or_else(|error| ClashDelayResponse {
                    delay: None,
                    message: Some(error.to_string()),
                });
            results.push(ClashDelayTestResult {
                name,
                delay: response.delay,
                message: response.message,
            });
        }

        Ok(results)
    }

    pub async fn set_rule_mode(&self, config: &AppConfig, mode: RuleMode) -> Result<()> {
        let Some(mode) = rule_mode_api_value(mode) else {
            return Err(ClashManagerError::InvalidRuleMode(mode));
        };

        self.client(config)
            .set_rule_mode(mode)
            .await
            .map_err(Into::into)
    }

    pub async fn reload_config(&self, config: &AppConfig, path: Option<&str>) -> Result<()> {
        let client = self.client(config);
        let _ = client.close_connection(None).await;
        client.reload_config(path).await.map_err(Into::into)
    }

    pub async fn close_connection(
        &self,
        config: &AppConfig,
        connection_id: Option<&str>,
    ) -> Result<ClashConnectionsSnapshot> {
        let client = self.client(config);
        client.close_connection(connection_id).await?;
        client
            .get_connections()
            .await
            .map(connections_snapshot)
            .map_err(Into::into)
    }

    fn client(&self, config: &AppConfig) -> ClashRestClient<T> {
        ClashRestClient::with_transport(clash_endpoint(config), self.transport.clone())
    }
}

#[derive(Clone, Default)]
pub struct ClashMonitorController {
    handle: Arc<Mutex<Option<ClashMonitorHandle>>>,
}

impl ClashMonitorController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(
        &self,
        config: &AppConfig,
        sink: Arc<dyn ClashEventSink>,
    ) -> Result<ClashMonitorStatus> {
        let mut guard = self
            .handle
            .lock()
            .map_err(|_| ClashManagerError::MonitorLockPoisoned)?;
        let endpoint = clash_endpoint(config);
        if guard
            .as_ref()
            .is_some_and(|handle| handle.endpoint == endpoint)
        {
            return Ok(ClashMonitorStatus { running: true });
        }
        let runtime =
            Handle::try_current().map_err(|_| ClashManagerError::MonitorRuntimeUnavailable)?;
        if let Some(handle) = guard.take() {
            handle.stop();
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let traffic_task = runtime.spawn(run_clash_ws_monitor(
            endpoint.clone(),
            ClashWebSocketResource::Traffic,
            Arc::clone(&sink),
            shutdown_rx.clone(),
        ));
        let connections_task = runtime.spawn(run_clash_ws_monitor(
            endpoint.clone(),
            ClashWebSocketResource::Connections,
            sink,
            shutdown_rx,
        ));
        *guard = Some(ClashMonitorHandle {
            endpoint,
            shutdown: shutdown_tx,
            tasks: vec![traffic_task, connections_task],
        });

        Ok(ClashMonitorStatus { running: true })
    }

    pub fn stop(&self) -> Result<ClashMonitorStatus> {
        let mut guard = self
            .handle
            .lock()
            .map_err(|_| ClashManagerError::MonitorLockPoisoned)?;
        if let Some(handle) = guard.take() {
            handle.stop();
        }

        Ok(ClashMonitorStatus { running: false })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClashMonitorStatus {
    pub running: bool,
}

struct ClashMonitorHandle {
    endpoint: ClashApiEndpoint,
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl ClashMonitorHandle {
    fn stop(self) {
        let _ = self.shutdown.send(true);
        for task in self.tasks {
            task.abort();
        }
    }
}

async fn run_clash_ws_monitor(
    endpoint: ClashApiEndpoint,
    resource: ClashWebSocketResource,
    sink: Arc<dyn ClashEventSink>,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            break;
        }

        let client = ClashWebSocketClient::new(endpoint.clone());
        match client.connect(resource).await {
            Ok(mut session) => loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return;
                        }
                    }
                    event = session.next_event() => match event {
                        Ok(event) => route_clash_ws_event(sink.as_ref(), event),
                        Err(error) => {
                            tracing::debug!(?error, ?resource, "Clash websocket monitor read failed");
                            break;
                        }
                    }
                }
            },
            Err(error) => {
                tracing::debug!(
                    ?error,
                    ?resource,
                    "failed to connect Clash websocket monitor"
                );
            }
        }

        if sleep_or_shutdown(CLASH_WS_RECONNECT_INTERVAL, &mut shutdown).await {
            break;
        }
    }
}

async fn sleep_or_shutdown(duration: Duration, shutdown: &mut watch::Receiver<bool>) -> bool {
    let mut sleep = time::interval(duration);
    sleep.set_missed_tick_behavior(MissedTickBehavior::Delay);
    sleep.tick().await;

    tokio::select! {
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
        _ = sleep.tick() => false,
    }
}

pub fn route_clash_ws_event(sink: &dyn ClashEventSink, event: ClashWebSocketEvent) {
    match event {
        ClashWebSocketEvent::Traffic(event) => sink.emit_traffic(traffic_event(event)),
        ClashWebSocketEvent::Connections(event) => {
            sink.emit_connections(connections_snapshot(event))
        }
    }
}

#[must_use]
pub fn clash_endpoint(config: &AppConfig) -> ClashApiEndpoint {
    ClashApiEndpoint::loopback(singbox_state_port2(config))
}

#[must_use]
pub fn rule_mode_api_value(mode: RuleMode) -> Option<&'static str> {
    match mode {
        RuleMode::Rule => Some("rule"),
        RuleMode::Global => Some("global"),
        RuleMode::Direct => Some("direct"),
        RuleMode::Unchanged => None,
    }
}

fn build_proxy_snapshot(
    proxies: &ClashProxiesResponse,
    providers: &ClashProvidersResponse,
    sorting: i32,
    rule_mode: RuleMode,
) -> ClashProxiesSnapshot {
    let mut groups = proxies
        .proxies
        .iter()
        .filter(|(_, proxy)| is_selectable_type(&proxy.proxy_type))
        .map(|(name, proxy)| {
            let mut nodes = proxy
                .all
                .iter()
                .filter_map(|node_name| {
                    find_proxy(node_name, proxies, providers).map(|node| {
                        proxy_node(node_name, node, proxy.now.as_deref() == Some(node_name))
                    })
                })
                .collect::<Vec<_>>();
            sort_nodes(&mut nodes, sorting);

            ClashProxyGroup {
                name: proxy.name.clone().unwrap_or_else(|| name.clone()),
                proxy_type: proxy.proxy_type.clone(),
                now: proxy.now.clone(),
                nodes,
            }
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.name.cmp(&right.name));

    let mut seen = BTreeSet::new();
    let mut all_nodes = Vec::new();
    for (name, proxy) in &proxies.proxies {
        if seen.insert(name.clone()) {
            all_nodes.push(proxy_node(name, proxy, false));
        }
    }
    for provider in providers.providers.values() {
        if !provider
            .vehicle_type
            .as_deref()
            .is_some_and(is_provider_proxy_vehicle_type)
        {
            continue;
        }
        for proxy in &provider.proxies {
            let name = proxy.name.clone().unwrap_or_default();
            if !name.is_empty() && seen.insert(name.clone()) {
                all_nodes.push(proxy_node(&name, proxy, false));
            }
        }
    }
    sort_nodes(&mut all_nodes, sorting);

    ClashProxiesSnapshot {
        groups,
        all_nodes,
        rule_mode,
    }
}

fn find_proxy<'proxies>(
    name: &str,
    proxies: &'proxies ClashProxiesResponse,
    providers: &'proxies ClashProvidersResponse,
) -> Option<&'proxies ClashProxy> {
    proxies.proxies.get(name).or_else(|| {
        providers
            .providers
            .values()
            .filter(|provider| {
                provider
                    .vehicle_type
                    .as_deref()
                    .is_some_and(is_provider_proxy_vehicle_type)
            })
            .flat_map(|provider| provider.proxies.iter())
            .find(|proxy| proxy.name.as_deref() == Some(name))
    })
}

fn proxy_node(name: &str, proxy: &ClashProxy, active: bool) -> ClashProxyNode {
    let delay = proxy
        .history
        .last()
        .map(|item| item.delay)
        .filter(|delay| *delay > 0)
        .or_else(|| (proxy.delay > 0).then_some(proxy.delay));
    ClashProxyNode {
        name: name.to_string(),
        proxy_type: proxy.proxy_type.clone(),
        delay,
        delay_label: delay.map_or_else(String::new, |value| format!("{value}ms")),
        udp: proxy.udp,
        active,
        testable: is_testable_type(&proxy.proxy_type),
    }
}

fn sort_nodes(nodes: &mut [ClashProxyNode], sorting: i32) {
    match sorting {
        0 => nodes.sort_by_key(|node| node.delay.unwrap_or(i32::MAX)),
        1 => nodes.sort_by(|left, right| left.name.cmp(&right.name)),
        _ => {}
    }
}

fn connections_snapshot(connections: NetClashConnections) -> ClashConnectionsSnapshot {
    ClashConnectionsSnapshot {
        download_total: connections.download_total,
        upload_total: connections.upload_total,
        connections: connections
            .connections
            .into_iter()
            .map(connection_item)
            .collect(),
    }
}

fn connection_item(connection: NetClashConnection) -> ClashConnectionItem {
    let metadata = connection.metadata;
    let host = connection_host(&metadata);
    let source = endpoint_label(
        metadata.source_ip.as_deref(),
        metadata.source_port.as_deref(),
    );
    let destination = endpoint_label(
        metadata.destination_ip.as_deref(),
        metadata.destination_port.as_deref(),
    );

    ClashConnectionItem {
        id: connection.id,
        network: metadata.network,
        connection_type: metadata.metadata_type,
        host,
        source,
        destination,
        upload: connection.upload,
        download: connection.download,
        start: connection.start,
        chains: connection.chains,
        rule: connection.rule,
        rule_payload: connection.rule_payload,
        process: metadata.process,
        process_path: metadata.process_path,
    }
}

fn traffic_event(event: NetClashTraffic) -> ClashTrafficEvent {
    ClashTrafficEvent {
        up: event.up,
        down: event.down,
    }
}

fn connection_host(metadata: &NetClashConnectionMetadata) -> String {
    let host = metadata
        .host
        .as_deref()
        .filter(|host| !host.trim().is_empty())
        .or(metadata.destination_ip.as_deref())
        .unwrap_or_default();
    endpoint_label(Some(host), metadata.destination_port.as_deref())
}

fn endpoint_label(address: Option<&str>, port: Option<&str>) -> String {
    match (
        address.map(str::trim).filter(|value| !value.is_empty()),
        port.map(str::trim).filter(|value| !value.is_empty()),
    ) {
        (Some(address), Some(port)) => format!("{address}:{port}"),
        (Some(address), None) => address.to_string(),
        (None, Some(port)) => format!(":{port}"),
        (None, None) => String::new(),
    }
}

fn is_selectable_type(proxy_type: &str) -> bool {
    let proxy_type = proxy_type.to_ascii_lowercase();
    ALLOW_SELECT_TYPES.contains(&proxy_type.as_str())
}

fn is_testable_type(proxy_type: &str) -> bool {
    let proxy_type = proxy_type.to_ascii_lowercase();
    !NOT_ALLOW_TEST_TYPES.contains(&proxy_type.as_str())
}

fn is_provider_proxy_vehicle_type(vehicle_type: &str) -> bool {
    let vehicle_type = vehicle_type.to_ascii_lowercase();
    PROVIDER_PROXY_VEHICLE_TYPES.contains(&vehicle_type.as_str())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    use serde_json::{json, Value};
    use voya_core::{SpeedTestItem, DEFAULT_LOCAL_PORT};
    use voya_net::clash::{ClashHttpMethod, ClashHttpRequest};

    use super::*;

    #[derive(Clone, Default)]
    struct MockTransport {
        requests: Arc<Mutex<Vec<ClashHttpRequest>>>,
        responses: Arc<Mutex<BTreeMap<String, Value>>>,
    }

    impl MockTransport {
        fn respond(&self, path: &str, value: Value) {
            self.responses.lock().expect("responses lock").insert(
                format!("http://127.0.0.1:{}{path}", DEFAULT_LOCAL_PORT + 5),
                value,
            );
        }

        fn requests(&self) -> Vec<ClashHttpRequest> {
            self.requests.lock().expect("requests lock").clone()
        }
    }

    impl ClashHttpTransport for MockTransport {
        fn send_json<'transport>(
            &'transport self,
            request: ClashHttpRequest,
        ) -> Pin<Box<dyn Future<Output = voya_net::clash::Result<Value>> + Send + 'transport>>
        {
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

    #[derive(Default)]
    struct CaptureSink {
        traffic: Mutex<Vec<ClashTrafficEvent>>,
        connections: Mutex<Vec<ClashConnectionsSnapshot>>,
    }

    impl ClashEventSink for CaptureSink {
        fn emit_traffic(&self, event: ClashTrafficEvent) {
            self.traffic.lock().expect("traffic lock").push(event);
        }

        fn emit_connections(&self, event: ClashConnectionsSnapshot) {
            self.connections
                .lock()
                .expect("connections lock")
                .push(event);
        }
    }

    fn config() -> AppConfig {
        AppConfig {
            speed_test_item: SpeedTestItem {
                speed_ping_test_url: "https://example.com/generate_204".to_string(),
                ..SpeedTestItem::default()
            },
            ..AppConfig::default()
        }
    }

    #[tokio::test]
    async fn clash_manager_rule_mode_uses_patch_configs() {
        let transport = MockTransport::default();
        transport.respond("/configs", Value::Null);
        let manager = ClashManager::with_transport(transport.clone());

        manager
            .set_rule_mode(&config(), RuleMode::Direct)
            .await
            .expect("set rule mode");

        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, ClashHttpMethod::Patch);
        assert_eq!(
            requests[0].url,
            format!("http://127.0.0.1:{}/configs", DEFAULT_LOCAL_PORT + 5)
        );
        assert_eq!(requests[0].body, Some(json!({ "mode": "direct" })));
    }

    #[tokio::test]
    async fn clash_manager_reload_uses_force_configs() {
        let transport = MockTransport::default();
        transport.respond("/connections", Value::Null);
        transport.respond("/configs?force=true", Value::Null);
        let manager = ClashManager::with_transport(transport.clone());

        manager
            .reload_config(&config(), Some("/tmp/config.yaml"))
            .await
            .expect("reload");

        let requests = transport.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, ClashHttpMethod::Delete);
        assert_eq!(requests[1].method, ClashHttpMethod::Put);
        assert_eq!(
            requests[1].url,
            format!(
                "http://127.0.0.1:{}/configs?force=true",
                DEFAULT_LOCAL_PORT + 5
            )
        );
    }

    #[tokio::test]
    async fn clash_manager_selects_active_proxy_with_put() {
        let transport = MockTransport::default();
        transport.respond(
            "/proxies",
            json!({
                "proxies": {
                    "Proxy": { "name": "Proxy", "type": "Selector", "now": "A", "all": ["A", "B"] },
                    "A": { "name": "A", "type": "ss", "history": [{ "delay": 12 }] },
                    "B": { "name": "B", "type": "ss", "history": [{ "delay": 8 }] }
                }
            }),
        );
        transport.respond("/proxies/Proxy", Value::Null);
        transport.respond("/providers/proxies", json!({ "providers": {} }));
        let manager = ClashManager::with_transport(transport.clone());

        let snapshot = manager
            .select_proxy(&config(), "Proxy", "B")
            .await
            .expect("select proxy");

        let requests = transport.requests();
        assert_eq!(requests[1].method, ClashHttpMethod::Put);
        assert_eq!(requests[1].body, Some(json!({ "name": "B" })));
        assert_eq!(snapshot.groups[0].nodes[0].name, "B");
    }

    #[tokio::test]
    async fn clash_manager_tests_delay_for_named_proxies() {
        let transport = MockTransport::default();
        transport.respond(
            "/proxies/A/delay?timeout=10000&url=https%3A%2F%2Fexample.com%2Fgenerate_204",
            json!({ "delay": 37 }),
        );
        let manager = ClashManager::with_transport(transport);

        let results = manager
            .test_delay(&config(), vec!["A".to_string()])
            .await
            .expect("delay");

        assert_eq!(
            results,
            vec![ClashDelayTestResult {
                name: "A".to_string(),
                delay: Some(37),
                message: None,
            }]
        );
    }

    #[test]
    fn clash_ws_events_update_event_sink_payloads() {
        let sink = CaptureSink::default();

        route_clash_ws_event(
            &sink,
            ClashWebSocketEvent::Traffic(NetClashTraffic { up: 10, down: 20 }),
        );
        route_clash_ws_event(
            &sink,
            ClashWebSocketEvent::Connections(NetClashConnections {
                download_total: 5,
                upload_total: 3,
                connections: vec![NetClashConnection {
                    id: Some("id-1".to_string()),
                    metadata: NetClashConnectionMetadata {
                        host: Some("example.com".to_string()),
                        destination_port: Some("443".to_string()),
                        ..NetClashConnectionMetadata::default()
                    },
                    upload: 1,
                    download: 2,
                    start: "2026-06-01T00:00:00Z".to_string(),
                    chains: vec!["proxy".to_string()],
                    rule: Some("MATCH".to_string()),
                    rule_payload: None,
                }],
            }),
        );

        assert_eq!(
            sink.traffic.lock().expect("traffic lock").as_slice(),
            &[ClashTrafficEvent { up: 10, down: 20 }]
        );
        let connections = sink.connections.lock().expect("connections lock");
        assert_eq!(connections[0].connections[0].host, "example.com:443");
    }

    #[test]
    fn clash_monitor_start_without_tokio_runtime_returns_error() {
        let controller = ClashMonitorController::new();

        let error = controller
            .start(&config(), Arc::new(NoopClashEventSink))
            .expect_err("monitor start should require a runtime");

        assert!(matches!(
            error,
            ClashManagerError::MonitorRuntimeUnavailable
        ));
    }

    #[tokio::test]
    async fn clash_monitor_starts_inside_tokio_runtime() {
        let controller = ClashMonitorController::new();

        let status = controller
            .start(&config(), Arc::new(NoopClashEventSink))
            .expect("monitor start");

        assert!(status.running);
        assert!(!controller.stop().expect("monitor stop").running);
    }

    #[tokio::test]
    async fn clash_monitor_start_is_idempotent_for_same_endpoint() {
        let controller = ClashMonitorController::new();

        controller
            .start(&config(), Arc::new(NoopClashEventSink))
            .expect("first monitor start");
        let first_shutdown = controller
            .handle
            .lock()
            .expect("monitor lock")
            .as_ref()
            .expect("monitor handle")
            .shutdown
            .clone();

        controller
            .start(&config(), Arc::new(NoopClashEventSink))
            .expect("second monitor start");
        let second_shutdown = controller
            .handle
            .lock()
            .expect("monitor lock")
            .as_ref()
            .expect("monitor handle")
            .shutdown
            .clone();

        assert!(first_shutdown.same_channel(&second_shutdown));
        assert!(!controller.stop().expect("monitor stop").running);

        controller
            .start(&config(), Arc::new(NoopClashEventSink))
            .expect("restart after stop");
        let restarted_shutdown = controller
            .handle
            .lock()
            .expect("monitor lock")
            .as_ref()
            .expect("monitor handle")
            .shutdown
            .clone();

        assert!(!first_shutdown.same_channel(&restarted_shutdown));
        assert!(!controller.stop().expect("monitor stop").running);
    }
}
