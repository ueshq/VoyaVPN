use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use thiserror::Error;
use tokio::{runtime::Handle, sync::watch, task::JoinHandle, time};
pub use voya_contracts::{
    ProxyConnectionItem, ProxyConnectionsSnapshot, ProxyMonitorState, ProxyMonitorStatus,
};
use voya_core::TrafficMode;
use voya_net::clash::{
    ClashApiEndpoint, ClashConnection as NetClashConnection,
    ClashConnectionMetadata as NetClashConnectionMetadata, ClashConnections as NetClashConnections,
    ClashError, ClashHttpTransport, ClashRestClient, ClashWebSocketClient, ClashWebSocketEvent,
    ClashWebSocketResource, ReqwestClashHttpTransport,
};

use crate::{
    backoff::{sleep_or_shutdown, WebSocketReconnectBackoff},
    statistics::available_state_port,
    supervisor::ClashApiAccess,
};

mod groups;
mod traffic_mode;
pub use groups::{RuntimeGroupMember, RuntimeGroupState};
pub use traffic_mode::{TrafficModeChangeError, TrafficModeChangeOutcome};

const PROXY_WS_RECONNECT_INITIAL_DELAY: Duration = Duration::from_secs(1);
const PROXY_WS_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(30);
const PROXY_WS_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub type Result<T> = std::result::Result<T, ProxyRuntimeError>;

#[derive(Debug, Error)]
pub enum ProxyRuntimeError {
    #[error(transparent)]
    Api(#[from] ClashError),
    #[error("invalid traffic mode {0:?}")]
    InvalidTrafficMode(TrafficMode),
    #[error("proxy monitor lock is poisoned")]
    MonitorLockPoisoned,
    #[error("proxy monitor requires a Tokio runtime")]
    MonitorRuntimeUnavailable,
    #[error("proxy runtime API state port is unavailable")]
    InvalidStatePort,
    #[error("node {0} is not a member of the running policy group")]
    UnknownGroupMember(String),
}

pub trait ProxyRuntimeEventSink: Send + Sync {
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
    pub async fn connections(&self, access: &ClashApiAccess) -> Result<ProxyConnectionsSnapshot> {
        self.client(access)?
            .get_connections()
            .await
            .map(connections_snapshot)
            .map_err(Into::into)
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
        // The monitor subscribes to /connections only — the statistics service
        // owns the /traffic stream against the same port — so a traffic frame
        // is unreachable here, and the proxy screens read their byte totals off
        // the connections snapshot anyway. Dropped rather than forwarded.
        ClashWebSocketEvent::Traffic(_) => {}
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
        TrafficMode::Unchanged => None,
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
#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    use serde_json::{json, Value};
    use voya_core::DEFAULT_LOCAL_PORT;
    use voya_net::clash::{ClashHttpMethod, ClashHttpRequest, ClashTraffic as NetClashTraffic};

    use super::*;
    use crate::supervisor::ClashApiSecret;

    #[derive(Clone)]
    struct NoopProxyRuntimeEventSink;

    impl ProxyRuntimeEventSink for NoopProxyRuntimeEventSink {
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

    #[derive(Default)]
    struct CaptureSink {
        connections: Mutex<Vec<ProxyConnectionsSnapshot>>,
    }

    impl ProxyRuntimeEventSink for CaptureSink {
        fn emit_connections(&self, event: ProxyConnectionsSnapshot) {
            self.connections
                .lock()
                .expect("connections lock")
                .push(event);
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
            .set_traffic_mode(&access(RUNTIME_PORT), TrafficMode::Global)
            .await
            .expect("set traffic mode");

        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, ClashHttpMethod::Patch);
        assert_eq!(
            requests[0].url,
            format!("http://127.0.0.1:{}/configs", DEFAULT_LOCAL_PORT + 5)
        );
        assert_eq!(requests[0].body, Some(json!({ "mode": "global" })));
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

        let connections = sink.connections.lock().expect("connections lock");
        assert_eq!(
            connections.len(),
            1,
            "a traffic frame has no sink to reach and must be dropped, not routed"
        );
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
                TrafficMode::Global,
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

    #[tokio::test]
    async fn group_state_selection_and_delays_speak_in_node_ids() {
        let transport = MockTransport::default();
        let port = u16::try_from(DEFAULT_LOCAL_PORT + 5).expect("state port");
        transport.respond(
            "/proxies",
            json!({ "proxies": {
                "proxy": { "name": "proxy", "type": "Selector", "all": ["Tokyo [tokyo]", "Osaka [osaka]"], "now": "Osaka [osaka]" },
                "Tokyo [tokyo]": { "name": "Tokyo [tokyo]", "type": "Socks", "history": [{ "time": "t", "delay": 95 }] },
                "Osaka [osaka]": { "name": "Osaka [osaka]", "type": "Socks", "history": [{ "time": "t", "delay": 0 }] }
            }}),
        );
        transport.respond("/proxies/proxy", Value::Null);
        transport.respond(
            "/group/proxy/delay?url=https%3A%2F%2Fprobe.example%2F&timeout=3000",
            json!({ "Osaka [osaka]": 120 }),
        );
        let manager = ProxyRuntimeManager::with_transport(transport.clone());
        let members = vec![
            voya_core::ProfileItem {
                index_id: "tokyo".to_string(),
                remarks: "Tokyo".to_string(),
                ..voya_core::ProfileItem::default()
            },
            voya_core::ProfileItem {
                index_id: "osaka".to_string(),
                remarks: "Osaka".to_string(),
                ..voya_core::ProfileItem::default()
            },
        ];

        let state = manager
            .group_state(&access(port), &members)
            .await
            .expect("group state");
        assert_eq!(state.now_profile_id.as_deref(), Some("osaka"));
        assert_eq!(
            state
                .members
                .iter()
                .map(|member| member.delay_ms)
                .collect::<Vec<_>>(),
            [Some(95), None]
        );

        manager
            .select_group_member(&access(port), &members, "tokyo")
            .await
            .expect("select");
        assert!(matches!(
            manager
                .select_group_member(&access(port), &members, "nope")
                .await,
            Err(ProxyRuntimeError::UnknownGroupMember(_))
        ));
        let delays = manager
            .test_group_delay(&access(port), &members, "https://probe.example/", 3_000)
            .await
            .expect("group delay");
        assert_eq!(delays.get("osaka"), Some(&120));
        assert!(transport
            .requests()
            .iter()
            .any(|request| request.method == ClashHttpMethod::Put
                && request.body == Some(json!({ "name": "Tokyo [tokyo]" }))));
    }
}
