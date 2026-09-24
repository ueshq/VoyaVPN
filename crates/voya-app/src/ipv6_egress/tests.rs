use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use voya_core::{ProfileProtocol, ServerEndpoint};
use voya_net::clash::{ClashError, ClashHttpRequest, ClashHttpTransport};

use super::*;
use crate::supervisor::ClashApiSecret;

fn node(index_id: &str, address: &str) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        remarks: index_id.to_string(),
        protocol: ProfileProtocol::Socks {
            server: ServerEndpoint {
                address: address.to_string(),
                port: 1080,
            },
            username: String::new(),
            password: String::new(),
        },
        ..ProfileItem::default()
    }
}

fn store() -> (tempfile::TempDir, Ipv6EgressStore) {
    let dir = tempfile::Builder::new()
        .prefix("voyavpn-ipv6-egress-")
        .tempdir()
        .expect("temp dir");
    let paths = AppPaths::new(dir.path());
    (dir, Ipv6EgressStore::new(&paths))
}

#[test]
fn a_record_belongs_to_the_node_and_the_server_it_was_probed_on() {
    let (_dir, store) = store();
    let tokyo = node("tokyo", "tokyo.example");
    let osaka = node("osaka", "osaka.example");
    assert_eq!(store.get(&tokyo), None);

    store
        .record(&tokyo, Ipv6Egress::Unsupported)
        .expect("record tokyo");
    store
        .record(&osaka, Ipv6Egress::Supported)
        .expect("record osaka");
    assert_eq!(store.get(&tokyo), Some(Ipv6Egress::Unsupported));
    assert_eq!(store.get(&osaka), Some(Ipv6Egress::Supported));
    assert_eq!(
        store.unsupported_nodes([&tokyo, &osaka]),
        BTreeSet::from(["tokyo".to_string()])
    );

    // Edited to another server: the old answer says nothing about it.
    let moved = node("tokyo", "elsewhere.example");
    assert_eq!(store.get(&moved), None);
    assert!(store.unsupported_nodes([&moved]).is_empty());

    // A later answer replaces the earlier one.
    store
        .record(&tokyo, Ipv6Egress::Supported)
        .expect("record tokyo again");
    assert!(store.unsupported_nodes([&tokyo, &osaka]).is_empty());
}

#[test]
fn a_damaged_record_counts_as_empty_and_is_replaced() {
    let (_dir, store) = store();
    filesystem::write_file_with_parent(&store.path, b"not json").expect("damage file");
    let tokyo = node("tokyo", "tokyo.example");

    assert_eq!(store.get(&tokyo), None);
    store
        .record(&tokyo, Ipv6Egress::Unsupported)
        .expect("record over damage");
    assert_eq!(store.get(&tokyo), Some(Ipv6Egress::Unsupported));
}

/// Answers a delay request with success for the listed test URLs and a
/// failed probe (503) for everything else.
#[derive(Clone, Default)]
struct DelayTransport {
    reachable: Vec<&'static str>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl ClashHttpTransport for DelayTransport {
    fn send<'transport>(
        &'transport self,
        request: ClashHttpRequest,
    ) -> Pin<Box<dyn Future<Output = voya_net::clash::Result<String>> + Send + 'transport>> {
        Box::pin(async move {
            self.requests
                .lock()
                .expect("requests")
                .push(request.url.clone());
            let reachable = self
                .reachable
                .iter()
                .any(|url| request.url.contains(&url_query(url)));
            if reachable {
                Ok(r#"{"delay":120}"#.to_string())
            } else {
                Err(ClashError::Status(503))
            }
        })
    }
}

fn url_query(url: &str) -> String {
    url.replace(':', "%3A").replace('/', "%2F")
}

const CONTROL: &str = "https://www.google.com/generate_204";

async fn probe(reachable: Vec<&'static str>) -> (Option<Ipv6Egress>, Vec<String>) {
    let transport = DelayTransport {
        reachable,
        ..DelayTransport::default()
    };
    let manager = ProxyRuntimeManager::with_transport(Arc::new(transport.clone()));
    let access = ClashApiAccess {
        port: Some(19371),
        secret: Some(ClashApiSecret::generate()),
    };
    let verdict = probe_ipv6_egress(&manager, &access, CONTROL).await;
    let requests = transport.requests.lock().expect("requests").clone();
    (verdict, requests)
}

#[tokio::test]
async fn a_node_that_loads_the_control_page_but_no_ipv6_page_has_no_ipv6_egress() {
    let (verdict, requests) = probe(vec![CONTROL]).await;
    assert_eq!(verdict, Some(Ipv6Egress::Unsupported));
    assert_eq!(requests.len(), 1 + IPV6_PROBE_URLS.len());
    assert!(requests
        .iter()
        .all(|url| url.contains("/proxies/proxy/delay?")));
}

#[tokio::test]
async fn one_reachable_ipv6_page_is_enough() {
    let (verdict, _) = probe(vec![CONTROL, IPV6_PROBE_URLS[2]]).await;
    assert_eq!(verdict, Some(Ipv6Egress::Supported));
}

#[tokio::test]
async fn a_failed_control_page_leaves_the_answer_unknown() {
    let (verdict, requests) = probe(vec![IPV6_PROBE_URLS[0]]).await;
    assert_eq!(verdict, None);
    assert_eq!(requests.len(), 1, "no IPv6 probe after a failed control");
}
