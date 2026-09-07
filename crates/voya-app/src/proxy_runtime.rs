use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{stream, StreamExt};
use thiserror::Error;
use tokio::{runtime::Handle, sync::watch, task::JoinHandle, time};
pub use voya_contracts::{
    ProxyConnectionItem, ProxyConnectionsSnapshot, ProxyDelayTestResult, ProxyGroup,
    ProxyGroupsSnapshot, ProxyMonitorState, ProxyMonitorStatus, ProxyNode, ProxyTrafficEvent,
};
use voya_core::{AppConfig, TrafficMode};
use voya_net::clash::{
    ClashApiEndpoint, ClashConnection as NetClashConnection,
    ClashConnectionMetadata as NetClashConnectionMetadata, ClashConnections as NetClashConnections,
    ClashDelayResponse, ClashError, ClashHttpTransport, ClashProvidersResponse,
    ClashProxiesResponse, ClashProxy, ClashRestClient, ClashTraffic as NetClashTraffic,
    ClashWebSocketClient, ClashWebSocketEvent, ClashWebSocketResource, ReqwestClashHttpTransport,
};

use crate::{
    backoff::{sleep_or_shutdown, WebSocketReconnectBackoff},
    statistics::available_state_port,
    supervisor::ClashApiAccess,
};

/// Fallback per-node latency budget when the configured speed-test timeout is
/// unusable; matches `SpeedTestItem::default().speed_test_timeout`.
const DEFAULT_DELAY_TIMEOUT_MS: u32 = 10_000;
const PROXY_WS_RECONNECT_INITIAL_DELAY: Duration = Duration::from_secs(1);
const PROXY_WS_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(30);
const PROXY_WS_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
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

pub type Result<T> = std::result::Result<T, ProxyRuntimeError>;

#[derive(Debug, Error)]
pub enum ProxyRuntimeError {
    #[error(transparent)]
    Api(#[from] ClashError),
    #[error("proxy group {0} was not found")]
    GroupNotFound(String),
    #[error("proxy node {0} was not found")]
    NodeNotFound(String),
    #[error("proxy group {0} is not a selector")]
    GroupNotSelector(String),
    #[error("invalid traffic mode {0:?}")]
    InvalidTrafficMode(TrafficMode),
    #[error("proxy monitor lock is poisoned")]
    MonitorLockPoisoned,
    #[error("proxy monitor requires a Tokio runtime")]
    MonitorRuntimeUnavailable,
    #[error("proxy runtime API state port is unavailable")]
    InvalidStatePort,
}

pub trait ProxyRuntimeEventSink: Send + Sync {
    fn emit_traffic(&self, event: ProxyTrafficEvent);
    fn emit_connections(&self, event: ProxyConnectionsSnapshot);
}

#[derive(Debug, Clone)]
pub struct ProxyRuntimeManager<T = ReqwestClashHttpTransport> {
    transport: T,
}

impl Default for ProxyRuntimeManager<ReqwestClashHttpTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyRuntimeManager<ReqwestClashHttpTransport> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            transport: ReqwestClashHttpTransport::new(),
        }
    }
}

impl<T> ProxyRuntimeManager<T>
where
    T: ClashHttpTransport,
{
    #[must_use]
    pub fn with_transport(transport: T) -> Self {
        Self { transport }
    }

    pub async fn groups(
        &self,
        config: &AppConfig,
        access: &ClashApiAccess,
    ) -> Result<ProxyGroupsSnapshot> {
        let client = self.client(access)?;
        let proxies = client.get_proxies().await?;
        let providers = client.get_proxy_providers().await.unwrap_or_default();

        Ok(build_proxy_groups_snapshot(
            &proxies,
            &providers,
            config.proxy_ui_item.node_sorting,
            config.proxy_ui_item.traffic_mode,
        ))
    }

    pub async fn connections(&self, access: &ClashApiAccess) -> Result<ProxyConnectionsSnapshot> {
        self.client(access)?
            .get_connections()
            .await
            .map(connections_snapshot)
            .map_err(Into::into)
    }

    pub async fn select_node(
        &self,
        config: &AppConfig,
        access: &ClashApiAccess,
        group_name: &str,
        node_name: &str,
    ) -> Result<ProxyGroupsSnapshot> {
        let client = self.client(access)?;
        let proxies = client.get_proxies().await?;
        let group = proxies
            .proxies
            .get(group_name)
            .ok_or_else(|| ProxyRuntimeError::GroupNotFound(group_name.to_string()))?;
        if !group.proxy_type.eq_ignore_ascii_case("selector") {
            return Err(ProxyRuntimeError::GroupNotSelector(group_name.to_string()));
        }
        if !group.all.iter().any(|name| name == node_name) {
            return Err(ProxyRuntimeError::NodeNotFound(node_name.to_string()));
        }

        client.select_proxy(group_name, node_name).await?;
        self.groups(config, access).await
    }

    pub async fn test_delay(
        &self,
        config: &AppConfig,
        access: &ClashApiAccess,
        node_names: Vec<String>,
    ) -> Result<Vec<ProxyDelayTestResult>> {
        let client = self.client(access)?;
        let names = if node_names.is_empty() {
            client
                .get_proxies()
                .await?
                .proxies
                .into_iter()
                .filter_map(|(name, proxy)| is_testable_type(&proxy.proxy_type).then_some(name))
                .collect::<Vec<_>>()
        } else {
            node_names
        };

        let timeout_ms = delay_timeout_ms(config);
        let test_url = config.speed_test_item.speed_ping_test_url.as_str();
        let concurrency = delay_test_concurrency(config, names.len());
        let client = &client;

        // Testing one node at a time costs a full timeout per unreachable
        // node, so "test all" takes minutes on a large subscription. `buffered`
        // overlaps the requests while still yielding results in the order the
        // caller asked for them.
        let results = stream::iter(names)
            .map(|name| async move {
                let response = client
                    .delay_proxy(&name, timeout_ms, test_url)
                    .await
                    .unwrap_or_else(|error| ClashDelayResponse {
                        delay: None,
                        message: Some(error.to_string()),
                    });
                ProxyDelayTestResult {
                    name,
                    delay: response.delay,
                    message: response.message,
                }
            })
            .buffered(concurrency)
            .collect::<Vec<_>>()
            .await;

        Ok(results)
    }

    pub async fn set_traffic_mode(&self, access: &ClashApiAccess, mode: TrafficMode) -> Result<()> {
        let Some(mode) = traffic_mode_api_value(mode) else {
            return Err(ProxyRuntimeError::InvalidTrafficMode(mode));
        };

        self.client(access)?
            .set_rule_mode(mode)
            .await
            .map_err(Into::into)
    }

    pub async fn reload_config(&self, access: &ClashApiAccess, path: Option<&str>) -> Result<()> {
        let client = self.client(access)?;
        let _ = client.close_connection(None).await;
        client.reload_config(path).await.map_err(Into::into)
    }

    pub async fn close_connection(
        &self,
        access: &ClashApiAccess,
        connection_id: Option<&str>,
    ) -> Result<ProxyConnectionsSnapshot> {
        let client = self.client(access)?;
        client.close_connection(connection_id).await?;
        client
            .get_connections()
            .await
            .map(connections_snapshot)
            .map_err(Into::into)
    }

    fn client(&self, access: &ClashApiAccess) -> Result<ClashRestClient<T>> {
        let endpoint = proxy_runtime_endpoint(access).ok_or(ProxyRuntimeError::InvalidStatePort)?;
        Ok(ClashRestClient::with_transport(
            endpoint,
            self.transport.clone(),
        ))
    }
}

#[derive(Clone, Default)]
pub struct ProxyMonitorController {
    handle: Arc<Mutex<Option<ProxyMonitorHandle>>>,
}

impl ProxyMonitorController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(
        &self,
        access: &ClashApiAccess,
        sink: Arc<dyn ProxyRuntimeEventSink>,
    ) -> Result<ProxyMonitorStatus> {
        let mut guard = self
            .handle
            .lock()
            .map_err(|_| ProxyRuntimeError::MonitorLockPoisoned)?;
        let Some(endpoint) = proxy_runtime_endpoint(access) else {
            if let Some(handle) = guard.take() {
                handle.stop();
            }
            tracing::debug!("skipping proxy monitor because state port is unavailable");
            return Ok(ProxyMonitorStatus::stopped());
        };
        if guard
            .as_ref()
            .is_some_and(|handle| handle.endpoint == endpoint)
        {
            return Ok(ProxyMonitorStatus::running());
        }
        let runtime =
            Handle::try_current().map_err(|_| ProxyRuntimeError::MonitorRuntimeUnavailable)?;
        if let Some(handle) = guard.take() {
            handle.stop();
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        // Only the connections stream is opened here. The statistics service
        // already holds a /traffic websocket against the same sing-box state
        // port, so a second one would decode every frame twice for numbers the
        // proxy screens do not read.
        let connections_task = runtime.spawn(run_proxy_ws_monitor(
            endpoint.clone(),
            ClashWebSocketResource::Connections,
            sink,
            shutdown_rx,
        ));
        *guard = Some(ProxyMonitorHandle {
            endpoint,
            shutdown: shutdown_tx,
            tasks: vec![connections_task],
        });

        Ok(ProxyMonitorStatus::running())
    }

    pub fn stop(&self) -> Result<ProxyMonitorStatus> {
        let mut guard = self
            .handle
            .lock()
            .map_err(|_| ProxyRuntimeError::MonitorLockPoisoned)?;
        if let Some(handle) = guard.take() {
            handle.stop();
        }

        Ok(ProxyMonitorStatus::stopped())
    }
}

struct ProxyMonitorHandle {
    endpoint: ClashApiEndpoint,
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl ProxyMonitorHandle {
    fn stop(self) {
        let _ = self.shutdown.send(true);
        for task in self.tasks {
            task.abort();
        }
    }
}

async fn run_proxy_ws_monitor(
    endpoint: ClashApiEndpoint,
    resource: ClashWebSocketResource,
    sink: Arc<dyn ProxyRuntimeEventSink>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut reconnect_backoff = WebSocketReconnectBackoff::new(
        PROXY_WS_RECONNECT_INITIAL_DELAY,
        PROXY_WS_RECONNECT_MAX_DELAY,
    );

    loop {
        if *shutdown.borrow() {
            break;
        }

        let client = ClashWebSocketClient::new(endpoint.clone());
        match time::timeout(PROXY_WS_CONNECT_TIMEOUT, client.connect(resource)).await {
            Ok(Ok(mut session)) => loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return;
                        }
                    }
                    event = session.next_event() => match event {
                        Ok(event) => {
                            reconnect_backoff.reset();
                            route_proxy_ws_event(sink.as_ref(), event);
                        }
                        Err(error) => {
                            tracing::debug!(?error, ?resource, "proxy websocket monitor read failed");
                            break;
                        }
                    }
                }
            },
            Ok(Err(error)) => {
                tracing::debug!(
                    ?error,
                    ?resource,
                    "failed to connect proxy websocket monitor"
                );
            }
            Err(error) => {
                tracing::debug!(
                    ?error,
                    ?resource,
                    "timed out connecting proxy websocket monitor"
                );
            }
        }

        if sleep_or_shutdown(reconnect_backoff.next_delay(), &mut shutdown).await {
            break;
        }
    }
}

pub fn route_proxy_ws_event(sink: &dyn ProxyRuntimeEventSink, event: ClashWebSocketEvent) {
    match event {
        ClashWebSocketEvent::Traffic(event) => sink.emit_traffic(proxy_traffic_event(event)),
        ClashWebSocketEvent::Connections(event) => {
            sink.emit_connections(connections_snapshot(event))
        }
    }
}

/// Resolves the Clash API endpoint of the core that is *running*.
///
/// `access` is what the supervisor reports for the live core: the port the
/// generated main config wrote into `experimental.clash_api`, and the bearer
/// token it wrote into `experimental.clash_api.secret`. That snapshot is the
/// only authority — on a pre-socks topology the builder clears
/// `is_tun_enabled` on the main context, so recomputing the port from
/// `tun_mode_item.enable_tun` would address the pre-socks process instead, and
/// the token exists only in the config that launch generated. `None` means no
/// core is running, so there is no Clash API to talk to.
#[must_use]
pub fn proxy_runtime_endpoint(access: &ClashApiAccess) -> Option<ClashApiEndpoint> {
    let port = available_state_port(access.port?)?;

    Some(ClashApiEndpoint {
        secret: access
            .secret
            .as_ref()
            .map(|secret| secret.as_str().to_string()),
        ..ClashApiEndpoint::loopback(port)
    })
}

#[must_use]
pub fn traffic_mode_api_value(mode: TrafficMode) -> Option<&'static str> {
    match mode {
        TrafficMode::Rule => Some("rule"),
        TrafficMode::Global => Some("global"),
        TrafficMode::Direct => Some("direct"),
        TrafficMode::Unchanged => None,
    }
}

fn build_proxy_groups_snapshot(
    proxies: &ClashProxiesResponse,
    providers: &ClashProvidersResponse,
    sorting: i32,
    traffic_mode: TrafficMode,
) -> ProxyGroupsSnapshot {
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

            ProxyGroup {
                name: proxy.name.clone().unwrap_or_else(|| name.clone()),
                proxy_type: proxy.proxy_type.clone(),
                now: proxy.now.clone(),
                nodes,
            }
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.name.cmp(&right.name));

    ProxyGroupsSnapshot {
        groups,
        traffic_mode: crate::contract_map::traffic_mode_to_contract(traffic_mode),
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

fn proxy_node(name: &str, proxy: &ClashProxy, active: bool) -> ProxyNode {
    let delay = proxy
        .history
        .last()
        .map(|item| item.delay)
        .filter(|delay| *delay > 0)
        .or_else(|| (proxy.delay > 0).then_some(proxy.delay));
    ProxyNode {
        name: name.to_string(),
        proxy_type: proxy.proxy_type.clone(),
        delay,
        delay_label: delay.map_or_else(String::new, |value| format!("{value}ms")),
        udp: proxy.udp,
        active,
        testable: is_testable_type(&proxy.proxy_type),
    }
}

fn sort_nodes(nodes: &mut [ProxyNode], sorting: i32) {
    match sorting {
        0 => nodes.sort_by_key(|node| node.delay.unwrap_or(i32::MAX)),
        1 => nodes.sort_by(|left, right| left.name.cmp(&right.name)),
        _ => {}
    }
}

fn connections_snapshot(connections: NetClashConnections) -> ProxyConnectionsSnapshot {
    ProxyConnectionsSnapshot {
        download_total: connections.download_total,
        upload_total: connections.upload_total,
        connections: connections
            .connections
            .into_iter()
            .map(connection_item)
            .collect(),
    }
}

fn connection_item(connection: NetClashConnection) -> ProxyConnectionItem {
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

    ProxyConnectionItem {
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

fn proxy_traffic_event(event: NetClashTraffic) -> ProxyTrafficEvent {
    ProxyTrafficEvent {
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

/// Per-node latency budget in milliseconds, taken from the configured
/// speed-test timeout so one setting governs every latency probe.
fn delay_timeout_ms(config: &AppConfig) -> u32 {
    u32::try_from(config.speed_test_item.speed_test_timeout)
        .ok()
        .filter(|seconds| *seconds > 0)
        .and_then(|seconds| seconds.checked_mul(1_000))
        .unwrap_or(DEFAULT_DELAY_TIMEOUT_MS)
}

/// How many latency probes may be in flight at once. Reuses the shared
/// speed-test concurrency setting, mirroring the speedtest manager, and never
/// returns zero because `buffered(0)` would stall the stream.
fn delay_test_concurrency(config: &AppConfig, node_count: usize) -> usize {
    let configured = usize::try_from(config.speed_test_item.mixed_concurrency_count)
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(1);
    configured.min(node_count.max(1))
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
    use crate::supervisor::ClashApiSecret;

    #[derive(Clone)]
    struct NoopProxyRuntimeEventSink;

    impl ProxyRuntimeEventSink for NoopProxyRuntimeEventSink {
        fn emit_traffic(&self, _event: ProxyTrafficEvent) {}
        fn emit_connections(&self, _event: ProxyConnectionsSnapshot) {}
    }

    /// A running core reachable on `port` with no bearer token, which is what
    /// every test that only cares about routing and payloads needs.
    fn access(port: u16) -> ClashApiAccess {
        ClashApiAccess::unauthenticated(port)
    }

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

    /// Records how many delay probes overlap so the bounded-concurrency
    /// behaviour can be asserted without depending on wall-clock timing.
    #[derive(Clone, Default)]
    struct ConcurrencyProbeTransport {
        state: Arc<Mutex<ConcurrencyProbe>>,
    }

    #[derive(Default)]
    struct ConcurrencyProbe {
        in_flight: usize,
        max_in_flight: usize,
        urls: Vec<String>,
    }

    impl ClashHttpTransport for ConcurrencyProbeTransport {
        fn send_json<'transport>(
            &'transport self,
            request: ClashHttpRequest,
        ) -> Pin<Box<dyn Future<Output = voya_net::clash::Result<Value>> + Send + 'transport>>
        {
            Box::pin(async move {
                {
                    let mut state = self.state.lock().expect("probe lock");
                    state.in_flight += 1;
                    state.max_in_flight = state.max_in_flight.max(state.in_flight);
                    state.urls.push(request.url.clone());
                }
                // Yield so every probe the stream started is counted before
                // the first one completes.
                tokio::task::yield_now().await;
                tokio::task::yield_now().await;
                self.state.lock().expect("probe lock").in_flight -= 1;

                Ok(json!({ "delay": 20 }))
            })
        }
    }

    #[derive(Default)]
    struct CaptureSink {
        traffic: Mutex<Vec<ProxyTrafficEvent>>,
        connections: Mutex<Vec<ProxyConnectionsSnapshot>>,
    }

    impl ProxyRuntimeEventSink for CaptureSink {
        fn emit_traffic(&self, event: ProxyTrafficEvent) {
            self.traffic.lock().expect("traffic lock").push(event);
        }

        fn emit_connections(&self, event: ProxyConnectionsSnapshot) {
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

    /// The port a running core reports. It must match the URLs `MockTransport`
    /// registers, which are built from the same `DEFAULT_LOCAL_PORT + 5`
    /// offset the generated `experimental.clash_api` uses.
    const RUNTIME_PORT: u16 = (DEFAULT_LOCAL_PORT + 5) as u16;

    fn monitor_handle_snapshot(
        controller: &ProxyMonitorController,
    ) -> (ClashApiEndpoint, watch::Sender<bool>) {
        let guard = controller.handle.lock().expect("monitor lock");
        let handle = guard.as_ref().expect("monitor handle");
        (handle.endpoint.clone(), handle.shutdown.clone())
    }

    fn monitor_handle_is_none(controller: &ProxyMonitorController) -> bool {
        controller.handle.lock().expect("monitor lock").is_none()
    }

    fn monitor_task_count(controller: &ProxyMonitorController) -> usize {
        let guard = controller.handle.lock().expect("monitor lock");
        guard.as_ref().map_or(0, |handle| handle.tasks.len())
    }

    fn shutdown_requested(shutdown: &watch::Sender<bool>) -> bool {
        let receiver = shutdown.subscribe();
        let requested = *receiver.borrow();
        requested
    }

    #[tokio::test]
    async fn proxy_runtime_traffic_mode_uses_patch_configs() {
        let transport = MockTransport::default();
        transport.respond("/configs", Value::Null);
        let manager = ProxyRuntimeManager::with_transport(transport.clone());

        manager
            .set_traffic_mode(&access(RUNTIME_PORT), TrafficMode::Direct)
            .await
            .expect("set traffic mode");

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
    async fn proxy_runtime_reload_uses_force_configs() {
        let transport = MockTransport::default();
        transport.respond("/connections", Value::Null);
        transport.respond("/configs?force=true", Value::Null);
        let manager = ProxyRuntimeManager::with_transport(transport.clone());

        manager
            .reload_config(&access(RUNTIME_PORT), Some("/tmp/config.yaml"))
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
    async fn proxy_runtime_selects_active_node_with_put() {
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
        let manager = ProxyRuntimeManager::with_transport(transport.clone());

        let snapshot = manager
            .select_node(&config(), &access(RUNTIME_PORT), "Proxy", "B")
            .await
            .expect("select proxy");

        let requests = transport.requests();
        assert_eq!(requests[1].method, ClashHttpMethod::Put);
        assert_eq!(requests[1].body, Some(json!({ "name": "B" })));
        assert_eq!(snapshot.groups[0].nodes[0].name, "B");
    }

    #[tokio::test]
    async fn proxy_runtime_tests_delay_for_named_nodes() {
        let transport = MockTransport::default();
        transport.respond(
            "/proxies/A/delay?timeout=10000&url=https%3A%2F%2Fexample.com%2Fgenerate_204",
            json!({ "delay": 37 }),
        );
        let manager = ProxyRuntimeManager::with_transport(transport);

        let results = manager
            .test_delay(&config(), &access(RUNTIME_PORT), vec!["A".to_string()])
            .await
            .expect("delay");

        assert_eq!(
            results,
            vec![ProxyDelayTestResult {
                name: "A".to_string(),
                delay: Some(37),
                message: None,
            }]
        );
    }

    #[tokio::test]
    async fn proxy_runtime_tests_delay_with_bounded_concurrency_in_request_order() {
        let transport = ConcurrencyProbeTransport::default();
        let manager = ProxyRuntimeManager::with_transport(transport.clone());
        let names = (0..8)
            .map(|index| format!("node-{index}"))
            .collect::<Vec<_>>();

        let results = manager
            .test_delay(&config(), &access(RUNTIME_PORT), names.clone())
            .await
            .expect("delay");

        assert_eq!(
            results
                .iter()
                .map(|result| result.name.clone())
                .collect::<Vec<_>>(),
            names,
            "results must follow the requested order"
        );
        assert!(results.iter().all(|result| result.delay == Some(20)));

        let probe = transport.state.lock().expect("probe lock");
        assert_eq!(probe.urls.len(), names.len());
        assert!(
            probe.max_in_flight > 1,
            "delay probes must overlap instead of running one at a time"
        );
        assert!(
            probe.max_in_flight <= 5,
            "concurrency must stay within the configured speed-test limit, saw {}",
            probe.max_in_flight
        );
    }

    #[test]
    fn proxy_runtime_delay_budget_follows_the_speed_test_settings() {
        let mut config = config();
        assert_eq!(delay_timeout_ms(&config), 10_000);
        assert_eq!(delay_test_concurrency(&config, 40), 5);
        assert_eq!(
            delay_test_concurrency(&config, 2),
            2,
            "never start more probes than there are nodes"
        );

        config.speed_test_item.speed_test_timeout = 3;
        config.speed_test_item.mixed_concurrency_count = 0;
        assert_eq!(delay_timeout_ms(&config), 3_000);
        assert_eq!(
            delay_test_concurrency(&config, 40),
            1,
            "buffered(0) would stall the stream"
        );

        config.speed_test_item.speed_test_timeout = -1;
        assert_eq!(delay_timeout_ms(&config), DEFAULT_DELAY_TIMEOUT_MS);
    }

    #[tokio::test]
    async fn proxy_runtime_rejects_zero_state_port_without_request() {
        let transport = MockTransport::default();
        let manager = ProxyRuntimeManager::with_transport(transport.clone());

        let error = manager
            .connections(&ClashApiAccess::default())
            .await
            .expect_err("no running core means no Clash API to dial");

        assert!(matches!(error, ProxyRuntimeError::InvalidStatePort));
        assert!(transport.requests().is_empty());
        assert_eq!(proxy_runtime_endpoint(&ClashApiAccess::default()), None);
        assert_eq!(proxy_runtime_endpoint(&access(0)), None);
    }

    #[test]
    fn proxy_websocket_events_update_event_sink_payloads() {
        let sink = CaptureSink::default();

        route_proxy_ws_event(
            &sink,
            ClashWebSocketEvent::Traffic(NetClashTraffic { up: 10, down: 20 }),
        );
        route_proxy_ws_event(
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
            &[ProxyTrafficEvent { up: 10, down: 20 }]
        );
        let connections = sink.connections.lock().expect("connections lock");
        assert_eq!(connections[0].connections[0].host, "example.com:443");
    }

    #[test]
    fn proxy_monitor_start_without_tokio_runtime_returns_error() {
        let controller = ProxyMonitorController::new();

        let error = controller
            .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
            .expect_err("monitor start should require a runtime");

        assert!(matches!(
            error,
            ProxyRuntimeError::MonitorRuntimeUnavailable
        ));
        assert!(monitor_handle_is_none(&controller));
    }

    #[test]
    fn proxy_monitor_status_contract_marks_stale_states() {
        assert_eq!(
            ProxyMonitorStatus::running(),
            ProxyMonitorStatus {
                state: ProxyMonitorState::Running,
                running: true,
                stale: false,
                message: None,
            }
        );
        assert_eq!(
            ProxyMonitorStatus::stopped(),
            ProxyMonitorStatus {
                state: ProxyMonitorState::Stopped,
                running: false,
                stale: true,
                message: None,
            }
        );
        assert_eq!(
            ProxyMonitorStatus::failed("start failed"),
            ProxyMonitorStatus {
                state: ProxyMonitorState::Failed,
                running: false,
                stale: true,
                message: Some("start failed".to_string()),
            }
        );
    }

    #[test]
    fn proxy_monitor_stop_is_idempotent_and_stale() {
        let controller = ProxyMonitorController::new();

        assert_eq!(
            controller.stop().expect("first monitor stop"),
            ProxyMonitorStatus::stopped()
        );
        assert_eq!(
            controller.stop().expect("second monitor stop"),
            ProxyMonitorStatus::stopped()
        );
    }

    #[tokio::test]
    async fn proxy_monitor_starts_inside_tokio_runtime() {
        let controller = ProxyMonitorController::new();

        let status = controller
            .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
            .expect("monitor start");

        assert_eq!(status, ProxyMonitorStatus::running());
        assert_eq!(
            controller.stop().expect("monitor stop"),
            ProxyMonitorStatus::stopped()
        );
    }

    #[tokio::test]
    async fn proxy_monitor_opens_only_the_connections_websocket() {
        let controller = ProxyMonitorController::new();

        controller
            .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
            .expect("monitor start");

        assert_eq!(
            monitor_task_count(&controller),
            1,
            "the statistics service already streams /traffic from the same port"
        );
        assert_eq!(
            controller.stop().expect("monitor stop"),
            ProxyMonitorStatus::stopped()
        );
    }

    #[test]
    fn proxy_runtime_endpoint_follows_the_running_core_port() {
        assert_eq!(
            proxy_runtime_endpoint(&access(RUNTIME_PORT)),
            Some(ClashApiEndpoint::loopback(RUNTIME_PORT)),
            "the monitor and the statistics service must dial the port the \
             running core reported, not one recomputed from the config"
        );
        assert_eq!(proxy_runtime_endpoint(&ClashApiAccess::default()), None);
        assert_eq!(proxy_runtime_endpoint(&access(0)), None);
    }

    /// Without the token every REST call and websocket upgrade against a
    /// secured core answers 401, so the app would lose its own proxy screens
    /// the moment the Clash API stopped being anonymous.
    #[test]
    fn proxy_runtime_endpoint_carries_the_running_core_secret() {
        let secret = ClashApiSecret::generate();

        let endpoint = proxy_runtime_endpoint(&ClashApiAccess::new(
            Some(RUNTIME_PORT),
            Some(secret.clone()),
        ))
        .expect("a connected core exposes an endpoint");

        assert_eq!(endpoint.secret.as_deref(), Some(secret.as_str()));
        assert_eq!(
            proxy_runtime_endpoint(&access(RUNTIME_PORT))
                .expect("an unsecured core still resolves")
                .secret,
            None
        );
    }

    /// Every REST call goes out with `Authorization: Bearer <token>`; the
    /// transport turns `bearer_token` into that header.
    #[tokio::test]
    async fn proxy_runtime_rest_requests_present_the_bearer_token() {
        let secret = ClashApiSecret::generate();
        let transport = MockTransport::default();
        transport.respond("/configs", Value::Null);
        let manager = ProxyRuntimeManager::with_transport(transport.clone());

        manager
            .set_traffic_mode(
                &ClashApiAccess::new(Some(RUNTIME_PORT), Some(secret.clone())),
                TrafficMode::Direct,
            )
            .await
            .expect("set traffic mode");

        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].bearer_token.as_deref(), Some(secret.as_str()));
    }

    #[tokio::test]
    async fn proxy_monitor_zero_state_port_stops_without_endpoint() {
        let controller = ProxyMonitorController::new();

        controller
            .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
            .expect("initial monitor start");
        let (_, first_shutdown) = monitor_handle_snapshot(&controller);

        assert_eq!(
            controller
                .start(
                    &ClashApiAccess::default(),
                    Arc::new(NoopProxyRuntimeEventSink)
                )
                .expect("zero port monitor start"),
            ProxyMonitorStatus::stopped()
        );

        assert!(shutdown_requested(&first_shutdown));
        assert!(monitor_handle_is_none(&controller));
    }

    #[tokio::test]
    async fn proxy_monitor_start_is_idempotent_for_same_endpoint() {
        let controller = ProxyMonitorController::new();

        assert_eq!(
            controller
                .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
                .expect("first monitor start"),
            ProxyMonitorStatus::running()
        );
        let (first_endpoint, first_shutdown) = monitor_handle_snapshot(&controller);

        assert_eq!(
            controller
                .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
                .expect("second monitor start"),
            ProxyMonitorStatus::running()
        );
        let (second_endpoint, second_shutdown) = monitor_handle_snapshot(&controller);

        assert_eq!(first_endpoint, second_endpoint);
        assert!(first_shutdown.same_channel(&second_shutdown));
        assert!(!shutdown_requested(&first_shutdown));
        assert_eq!(
            controller.stop().expect("monitor stop"),
            ProxyMonitorStatus::stopped()
        );
    }

    #[tokio::test]
    async fn proxy_monitor_start_after_stop_creates_fresh_handle() {
        let controller = ProxyMonitorController::new();

        controller
            .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
            .expect("first monitor start");
        let (first_endpoint, first_shutdown) = monitor_handle_snapshot(&controller);
        assert_eq!(
            controller.stop().expect("monitor stop"),
            ProxyMonitorStatus::stopped()
        );
        assert!(monitor_handle_is_none(&controller));
        assert!(shutdown_requested(&first_shutdown));

        assert_eq!(
            controller
                .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
                .expect("restart after stop"),
            ProxyMonitorStatus::running()
        );
        let (restarted_endpoint, restarted_shutdown) = monitor_handle_snapshot(&controller);

        assert_eq!(first_endpoint, restarted_endpoint);
        assert!(!first_shutdown.same_channel(&restarted_shutdown));
        assert!(!shutdown_requested(&restarted_shutdown));
        assert_eq!(
            controller.stop().expect("monitor stop"),
            ProxyMonitorStatus::stopped()
        );
    }

    #[tokio::test]
    async fn proxy_monitor_different_endpoint_replaces_previous_handle() {
        let controller = ProxyMonitorController::new();
        let replacement_port = RUNTIME_PORT + 100;

        assert_eq!(
            controller
                .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
                .expect("initial monitor start"),
            ProxyMonitorStatus::running()
        );
        let (initial_endpoint, initial_shutdown) = monitor_handle_snapshot(&controller);

        assert_eq!(
            controller
                .start(
                    &access(replacement_port),
                    Arc::new(NoopProxyRuntimeEventSink)
                )
                .expect("replacement monitor start"),
            ProxyMonitorStatus::running()
        );
        let (replacement_endpoint, replacement_shutdown) = monitor_handle_snapshot(&controller);

        assert_eq!(
            proxy_runtime_endpoint(&access(RUNTIME_PORT)).as_ref(),
            Some(&initial_endpoint)
        );
        assert_eq!(
            proxy_runtime_endpoint(&access(replacement_port)).as_ref(),
            Some(&replacement_endpoint)
        );
        assert_ne!(initial_endpoint, replacement_endpoint);
        assert!(shutdown_requested(&initial_shutdown));
        assert!(!initial_shutdown.same_channel(&replacement_shutdown));
        assert!(!shutdown_requested(&replacement_shutdown));
        assert_eq!(
            controller.stop().expect("monitor stop"),
            ProxyMonitorStatus::stopped()
        );
    }

    #[tokio::test]
    async fn proxy_monitor_clones_share_handle_state() {
        let controller = ProxyMonitorController::new();
        let clone = controller.clone();

        clone
            .start(&access(RUNTIME_PORT), Arc::new(NoopProxyRuntimeEventSink))
            .expect("monitor start through clone");
        assert!(!monitor_handle_is_none(&controller));

        assert_eq!(
            controller.stop().expect("monitor stop through original"),
            ProxyMonitorStatus::stopped()
        );
        assert!(monitor_handle_is_none(&clone));
    }
}
