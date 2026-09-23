use std::sync::Mutex as StdMutex;

use voya_core::{ProfileExItem, ProfileProtocol, ServerEndpoint};

use super::*;
use crate::speedtest::tests::{RecordingCoreBackend, RecordingProbe};

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordedDelay {
    tag: String,
    test_url: String,
    timeout_ms: u32,
}

/// Answers by node id (the middle of a `probe:<id>:<fingerprint>` tag).
#[derive(Default)]
struct FakeCore {
    connected: bool,
    answers: Vec<(&'static str, std::result::Result<u32, u16>)>,
    /// Hold every request open until the run is cancelled.
    block_until_cancelled: Option<CancellationFlag>,
    requests: Arc<StdMutex<Vec<RecordedDelay>>>,
}

impl FakeCore {
    fn requests(&self) -> Vec<RecordedDelay> {
        self.requests.lock().expect("requests").clone()
    }
}

struct FakeDelay {
    answers: Vec<(&'static str, std::result::Result<u32, u16>)>,
    block_until_cancelled: Option<CancellationFlag>,
    requests: Arc<StdMutex<Vec<RecordedDelay>>>,
}

impl RunningCoreProbe for FakeCore {
    fn connect(&self) -> BoxFuture<'static, Option<Arc<dyn RunningCoreProbe>>> {
        let delay = self.connected.then(|| {
            Arc::new(FakeDelay {
                answers: self.answers.clone(),
                block_until_cancelled: self.block_until_cancelled.clone(),
                requests: Arc::clone(&self.requests),
            }) as Arc<dyn RunningCoreProbe>
        });
        Box::pin(async move { delay })
    }

    fn delay(
        &self,
        _tag: String,
        _test_url: String,
        _timeout_ms: u32,
    ) -> BoxFuture<'static, std::result::Result<u32, ClashError>> {
        Box::pin(async { Err(ClashError::WebSocketClosed) })
    }
}

impl RunningCoreProbe for FakeDelay {
    fn connect(&self) -> BoxFuture<'static, Option<Arc<dyn RunningCoreProbe>>> {
        Box::pin(async { None })
    }

    fn delay(
        &self,
        tag: String,
        test_url: String,
        timeout_ms: u32,
    ) -> BoxFuture<'static, std::result::Result<u32, ClashError>> {
        self.requests.lock().expect("requests").push(RecordedDelay {
            tag: tag.clone(),
            test_url,
            timeout_ms,
        });
        let answer = self
            .answers
            .iter()
            .find(|(id, _)| tag.starts_with(&format!("probe:{id}:")))
            .map_or(Err(500), |(_, answer)| *answer);
        let block = self.block_until_cancelled.clone();
        Box::pin(async move {
            if let Some(cancel) = block {
                cancelled(&cancel).await;
            }
            answer.map_err(ClashError::Status)
        })
    }
}

struct Harness {
    manager: SpeedtestManager,
    core: Arc<FakeCore>,
    probe: Arc<RecordingProbe>,
    probe_cores: Arc<RecordingCoreBackend>,
}

fn manager(core: FakeCore) -> Harness {
    let core = Arc::new(core);
    let probe = Arc::new(RecordingProbe::default());
    let probe_cores = Arc::new(RecordingCoreBackend::default());
    let paths = AppPaths::new(
        std::env::temp_dir().join(format!("voyavpn-running-core-{}", uuid::Uuid::new_v4())),
    );
    let manager = SpeedtestManager::with_probe_and_launcher(
        paths,
        Arc::clone(&probe) as Arc<dyn SpeedtestProbe>,
        Arc::clone(&probe_cores) as Arc<dyn ProbeCoreLauncher>,
    )
    .with_running_core(Arc::clone(&core) as Arc<dyn RunningCoreProbe>);
    Harness {
        manager,
        core,
        probe,
        probe_cores,
    }
}

async fn database_with(profiles: &[ProfileItem]) -> Database {
    let database = Database::connect_in_memory().await.expect("database");
    for profile in profiles {
        let profile_ex = ProfileExItem {
            index_id: profile.index_id.clone(),
            ..ProfileExItem::default()
        };
        database
            .profiles()
            .upsert_with_profile_ex(profile, &profile_ex)
            .await
            .expect("insert profile");
    }
    database
}

fn vmess(index_id: &str) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        remarks: index_id.to_string(),
        protocol: ProfileProtocol::Vmess {
            server: ServerEndpoint {
                address: "127.0.0.1".to_string(),
                port: 443,
            },
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            cipher: Some("auto".to_string()),
        },
        ..ProfileItem::default()
    }
}

fn wireguard(index_id: &str) -> ProfileItem {
    ProfileItem {
        index_id: index_id.to_string(),
        remarks: index_id.to_string(),
        protocol: ProfileProtocol::WireGuard {
            server: ServerEndpoint {
                address: "198.51.100.7".to_string(),
                port: 51820,
            },
            private_key: "key".to_string(),
            peer_public_key: Some("peer".to_string()),
            preshared_key: None,
            interface_address: None,
            allowed_ips: None,
            reserved: None,
            mtu: None,
        },
        ..ProfileItem::default()
    }
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

fn outcome_of(run: &SpeedtestRunResult, index_id: &str) -> (SpeedtestOutcome, Option<i32>) {
    let result = run
        .results
        .iter()
        .find(|result| result.index_id == index_id)
        .unwrap_or_else(|| panic!("no result for {index_id}"));
    (result.outcome, result.delay)
}

/// Disconnected, nothing tunnels the probe's traffic, so the packaged seed
/// measures every node directly.
#[tokio::test]
async fn running_core_speedtest_uses_probe_cores_while_disconnected() {
    let database = database_with(&[vmess("a"), vmess("b"), wireguard("wg")]).await;
    let Harness {
        manager,
        core,
        probe,
        probe_cores,
    } = manager(FakeCore::default());

    let run = manager
        .run_with_callback(
            &database,
            &AppConfig::default(),
            ids(&["a", "b", "wg"]),
            |_| {},
        )
        .await
        .expect("run");

    assert!(core.requests().is_empty());
    assert_eq!(probe_cores.starts().len(), 1);
    assert_eq!(probe.calls().len(), 3, "a probe core tests WireGuard too");
    for id in ["a", "b", "wg"] {
        assert_eq!(
            outcome_of(&run, id),
            (SpeedtestOutcome::Completed, Some(44))
        );
    }
    let stored = database
        .profile_exs()
        .get("a")
        .await
        .expect("read")
        .expect("row");
    assert_eq!(stored.delay, 44);
}

#[tokio::test]
async fn running_core_speedtest_maps_the_core_answers() {
    let database =
        database_with(&[vmess("ok"), vmess("stale"), vmess("slow"), wireguard("wg")]).await;
    let Harness {
        manager,
        core,
        probe_cores,
        ..
    } = manager(FakeCore {
        connected: true,
        answers: vec![("ok", Ok(88)), ("stale", Err(404)), ("slow", Err(504))],
        ..FakeCore::default()
    });
    let mut config = AppConfig::default();
    config.speed_test_item.speed_test_timeout = 90;
    config.speed_test_item.speed_ping_test_url = String::new();

    let run = manager
        .run_with_callback(
            &database,
            &config,
            ids(&["ok", "stale", "slow", "wg"]),
            |_| {},
        )
        .await
        .expect("run");

    assert_eq!(
        outcome_of(&run, "ok"),
        (SpeedtestOutcome::Completed, Some(88))
    );
    assert_eq!(
        outcome_of(&run, "stale").0,
        SpeedtestOutcome::ReconnectRequired
    );
    assert_eq!(outcome_of(&run, "slow").0, SpeedtestOutcome::TimedOut);
    assert_eq!(outcome_of(&run, "wg").0, SpeedtestOutcome::Skipped);
    assert!(
        probe_cores.starts().is_empty(),
        "a connected tunnel would carry a probe core's traffic"
    );
    let requests = core.requests();
    assert_eq!(
        requests.len(),
        3,
        "WireGuard carries no probe: {requests:?}"
    );
    assert!(requests.iter().all(|request| {
        request.test_url == REALPING_FALLBACK_URL && request.timeout_ms == 25_000
    }));
    let stored = database
        .profile_exs()
        .get("ok")
        .await
        .expect("read")
        .expect("row");
    assert_eq!(stored.delay, 88);
}

#[tokio::test]
async fn running_core_speedtest_stops_waiting_on_cancel() {
    let database = database_with(&[vmess("a"), vmess("b")]).await;
    let cancel_probe = Arc::new(AtomicBool::new(false));
    let Harness { manager, .. } = manager(FakeCore {
        connected: true,
        answers: vec![("a", Ok(10)), ("b", Ok(10))],
        block_until_cancelled: Some(Arc::clone(&cancel_probe)),
        ..FakeCore::default()
    });
    let run_manager = manager.clone();
    let run = tokio::spawn(async move {
        run_manager
            .run_with_callback(&database, &AppConfig::default(), ids(&["a", "b"]), |_| {})
            .await
            .expect("run")
    });

    while !manager.status().running {
        time::sleep(Duration::from_millis(5)).await;
    }
    time::sleep(Duration::from_millis(20)).await;
    assert!(manager.cancel());
    let run = time::timeout(Duration::from_secs(2), run)
        .await
        .expect("the run returns without waiting on the core")
        .expect("join");
    cancel_probe.store(true, Ordering::SeqCst);

    assert!(run.cancelled);
    assert_eq!(outcome_of(&run, "a").0, SpeedtestOutcome::Cancelled);
    assert_eq!(outcome_of(&run, "b").0, SpeedtestOutcome::Cancelled);
}
