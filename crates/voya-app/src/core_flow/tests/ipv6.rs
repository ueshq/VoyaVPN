use crate::ipv6_egress::{Ipv6Egress, Ipv6EgressStore, IPV6_PROBE_URLS};

use super::*;

/// Accepts the traffic-mode call and answers a delay probe with success only
/// for the listed test URLs.
#[derive(Clone, Default)]
struct DelayTransport {
    reachable: Vec<&'static str>,
    delay_requests: Arc<Mutex<Vec<String>>>,
}

impl ClashHttpTransport for DelayTransport {
    fn send<'transport>(
        &'transport self,
        request: voya_net::clash::ClashHttpRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = voya_net::clash::Result<String>> + Send + 'transport>,
    > {
        Box::pin(async move {
            if !request.url.contains("/delay?") {
                return Ok(String::new());
            }
            self.delay_requests
                .lock()
                .expect("delay requests")
                .push(request.url.clone());
            let reachable = self.reachable.iter().any(|url| {
                request
                    .url
                    .contains(&url.replace(':', "%3A").replace('/', "%2F"))
            });
            if reachable {
                Ok(r#"{"delay":80}"#.to_string())
            } else {
                Err(voya_net::clash::ClashError::Status(503))
            }
        })
    }
}

const CONTROL: &str = voya_core::DEFAULT_SPEED_PING_TEST_URL;

impl Harness {
    fn ipv6_flow(&self, transport: &DelayTransport) -> CoreFlow<'_> {
        self.flow_with_transport(transport.clone())
    }

    fn store(&self) -> Ipv6EgressStore {
        Ipv6EgressStore::new(&self.paths)
    }

    fn running_config(&self) -> serde_json::Value {
        let path = self
            .paths
            .bin_config_file(crate::runtime::MAIN_CONFIG_FILE_NAME);
        serde_json::from_slice(&fs::read(path).expect("running config")).expect("config JSON")
    }

    fn count(&self, prefix: &str) -> usize {
        self.sink
            .events()
            .iter()
            .filter(|event| event.starts_with(prefix))
            .count()
    }
}

fn rejects_proxied_ipv6(config: &serde_json::Value) -> bool {
    config["route"]["rules"]
        .as_array()
        .expect("route rules")
        .last()
        .is_some_and(|rule| rule["ip_version"] == 6 && rule["action"] == "reject")
}

#[tokio::test]
async fn a_node_without_ipv6_egress_is_recorded_announced_and_reconnected() {
    let harness = Harness::new().await;
    let transport = DelayTransport {
        reachable: vec![CONTROL],
        ..DelayTransport::default()
    };
    let flow = harness.ipv6_flow(&transport);
    let config = active_config();
    assert!(config.tun.ipv6_enabled, "IPv6 is on by default");

    flow.connect(&config).await.expect("connect");
    assert_eq!(harness.count("ipv6-check"), 1, "a connect asks for a check");
    assert!(!rejects_proxied_ipv6(&harness.running_config()));

    flow.check_ipv6_egress(|| config.clone()).await;

    let node = singbox_profile("active");
    assert_eq!(harness.store().get(&node), Some(Ipv6Egress::Unsupported));
    assert_eq!(
        harness.count("notice:Warning:nodeIpv6Unsupported(remarks=\"Core flow\")"),
        1
    );
    assert_eq!(
        harness.count("log:Info:restartedAfterChange(reason=\"ipv6EgressChanged\")"),
        1
    );
    // The reconnect it caused does not ask for another check.
    assert_eq!(harness.count("ipv6-check"), 1);
    let running = harness.running_config();
    assert!(rejects_proxied_ipv6(&running), "IPv6 stays off the node");
    assert_eq!(
        running["dns"].get("strategy"),
        None,
        "direct paths keep IPv6"
    );
    assert_eq!(
        transport.delay_requests.lock().expect("requests").len(),
        1 + IPV6_PROBE_URLS.len()
    );

    // The same answer again changes nothing.
    flow.check_ipv6_egress(|| config.clone()).await;
    assert_eq!(harness.count("notice:"), 1);
    assert_eq!(
        harness.count("log:Info:restartedAfterChange(reason=\"ipv6EgressChanged\")"),
        1
    );
    flow.disconnect(&config).await.expect("cleanup");
}

#[tokio::test]
async fn a_node_that_reaches_ipv6_again_is_restored() {
    let harness = Harness::new().await;
    let node = singbox_profile("active");
    harness
        .store()
        .record(&node, Ipv6Egress::Unsupported)
        .expect("earlier answer");
    let transport = DelayTransport {
        reachable: vec![CONTROL, IPV6_PROBE_URLS[1]],
        ..DelayTransport::default()
    };
    let flow = harness.ipv6_flow(&transport);
    let config = active_config();

    flow.connect(&config).await.expect("connect");
    assert!(
        rejects_proxied_ipv6(&harness.running_config()),
        "generated from the earlier answer"
    );
    flow.check_ipv6_egress(|| config.clone()).await;

    assert_eq!(harness.store().get(&node), Some(Ipv6Egress::Supported));
    assert_eq!(
        harness.count("notice:Info:nodeIpv6Restored(remarks=\"Core flow\")"),
        1
    );
    assert!(!rejects_proxied_ipv6(&harness.running_config()));
    flow.disconnect(&config).await.expect("cleanup");
}

#[tokio::test]
async fn a_capable_node_seen_for_the_first_time_needs_no_reconnect() {
    let harness = Harness::new().await;
    let transport = DelayTransport {
        reachable: vec![CONTROL, IPV6_PROBE_URLS[0]],
        ..DelayTransport::default()
    };
    let flow = harness.ipv6_flow(&transport);
    let config = active_config();
    flow.connect(&config).await.expect("connect");
    flow.check_ipv6_egress(|| config.clone()).await;

    assert_eq!(
        harness.store().get(&singbox_profile("active")),
        Some(Ipv6Egress::Supported)
    );
    assert_eq!(harness.count("notice:"), 0);
    assert_eq!(harness.count("log:Info:restartingAfterChange"), 0);
    flow.disconnect(&config).await.expect("cleanup");
}

#[tokio::test]
async fn an_untrustworthy_or_unwanted_probe_changes_nothing() {
    // The control page fails too: the node is not working at all right now.
    let harness = Harness::new().await;
    let transport = DelayTransport::default();
    let flow = harness.ipv6_flow(&transport);
    let config = active_config();
    flow.connect(&config).await.expect("connect");
    flow.check_ipv6_egress(|| config.clone()).await;
    assert_eq!(harness.store().get(&singbox_profile("active")), None);
    assert_eq!(harness.count("notice:"), 0);
    assert_eq!(transport.delay_requests.lock().expect("requests").len(), 1);
    flow.disconnect(&config).await.expect("cleanup");

    // IPv6 switched off: nothing to probe for.
    let harness = Harness::new().await;
    let transport = DelayTransport {
        reachable: vec![CONTROL],
        ..DelayTransport::default()
    };
    let flow = harness.ipv6_flow(&transport);
    let mut config = active_config();
    config.tun.ipv6_enabled = false;
    flow.connect(&config).await.expect("connect");
    flow.check_ipv6_egress(|| config.clone()).await;
    assert!(transport
        .delay_requests
        .lock()
        .expect("requests")
        .is_empty());
    flow.disconnect(&config).await.expect("cleanup");

    // Not connected.
    let harness = Harness::new().await;
    let transport = DelayTransport {
        reachable: vec![CONTROL],
        ..DelayTransport::default()
    };
    harness
        .ipv6_flow(&transport)
        .check_ipv6_egress(active_config)
        .await;
    assert!(transport
        .delay_requests
        .lock()
        .expect("requests")
        .is_empty());
}

#[tokio::test]
async fn only_a_reconnect_that_can_change_the_node_asks_for_a_check() {
    let harness = Harness::new().await;
    let transport = DelayTransport::default();
    let flow = harness.ipv6_flow(&transport);
    let config = active_config();
    flow.connect(&config).await.expect("connect");
    flow.restart_if_connected(&config, CoreFlowReason::RoutingChanged)
        .await
        .expect("routing restart");
    flow.restart_if_connected(&config, CoreFlowReason::DnsChanged)
        .await
        .expect("DNS restart");
    assert_eq!(harness.count("ipv6-check"), 1);
    flow.restart_if_connected(&config, CoreFlowReason::ActiveProfileChanged)
        .await
        .expect("node restart");
    flow.restart(&config).await.expect("restart");
    assert_eq!(harness.count("ipv6-check"), 3);
    flow.disconnect(&config).await.expect("cleanup");
}
