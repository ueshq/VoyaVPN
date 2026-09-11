use std::{
    collections::HashSet,
    io,
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use futures_util::future::BoxFuture;
use thiserror::Error;
use tokio::time;
pub use voya_contracts::{SpeedtestOutcome, SpeedtestResult, SpeedtestRunResult, SpeedtestStatus};
use voya_core::{
    generate_singbox_speedtest_config_json, AppConfig, CoreConfigContextBuilder, CoreType,
    InboundProtocol, ProfileItem, SpeedTestItem, SpeedtestConfigEntry, DEFAULT_LOCAL_PORT,
};
use voya_db::{Database, DbError};
use voya_net::probe::{tcp_port_is_open, NetworkProbeError, SocksHttpProbe};
use voya_platform::{
    coreinfo::{get_core_info, CoreInfoError, TargetOs},
    filesystem,
    paths::{AppPaths, PathError},
    process::{ProcessError, ProcessHandle, ProcessRole, ProcessRunner, ProcessSpawn},
};

use crate::redaction::redact_urls;
use crate::runtime::{core_launch_plan, load_runtime_core_gen_env};

const REALPING_FALLBACK_URL: &str = "https://www.google.com/generate_204";
const SPEEDTEST_BATCH_PAGE_SIZE: usize = 1000;
const SPEEDTEST_DELAY_INTERVAL: Duration = Duration::from_secs(1);
const LOOPBACK_ADDR: &str = "127.0.0.1";

pub type CancellationFlag = Arc<AtomicBool>;
pub type Result<T> = std::result::Result<T, SpeedtestError>;

#[derive(Debug, Error)]
pub enum SpeedtestError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Profile(#[from] crate::profiles::ProfileManagerError),
    #[error(transparent)]
    Network(#[from] NetworkProbeError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    CoreInfo(#[from] CoreInfoError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    SingboxConfig(#[from] voya_core::SingboxConfigError),
    #[error("speedtest was cancelled")]
    Cancelled,
    #[error("no core info entry for {0:?}")]
    MissingCoreInfo(CoreType),
    #[error("failed to create speedtest config directory {path}: {source}")]
    CreateConfigDir { path: PathBuf, source: io::Error },
    #[error("failed to write speedtest config {path}: {source}")]
    WriteConfig { path: PathBuf, source: io::Error },
    #[error("failed to remove speedtest config {path}: {source}")]
    RemoveConfig { path: PathBuf, source: io::Error },
    #[error("speedtest config validation failed for {index_id}: {message}")]
    Validation { index_id: String, message: String },
    #[error("no available speedtest port at or after {0}")]
    NoAvailablePort(i32),
    #[error("speedtest local SOCKS port {0} is outside the valid range")]
    InvalidSocksPort(i32),
    #[error("speedtest job lock is poisoned")]
    JobLockPoisoned,
    #[error("speedtest background task failed: {0}")]
    BackgroundTask(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerTestItem {
    pub index_id: String,
    pub socks_port: u16,
    pub profile: ProfileItem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealPingProbeResult {
    pub delay: i32,
    pub ip_info: Option<String>,
    pub country_code: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedSpeedtestItem {
    item: ServerTestItem,
    entry: SpeedtestConfigEntry,
}

/// Outcome of preparing a selection. One unusable profile must not abort the
/// run, so validation and port-reservation failures travel alongside the items
/// that are ready to test and are reported as per-item results instead.
#[derive(Debug, Clone, Default)]
struct PreparedSpeedtestBatch {
    prepared: Vec<PreparedSpeedtestItem>,
    failures: Vec<SpeedtestItemFailure>,
}

#[derive(Debug, Clone)]
struct SpeedtestItemFailure {
    index_id: String,
    outcome: SpeedtestOutcome,
    detail: Option<String>,
}

impl SpeedtestItemFailure {
    fn new(index_id: String, outcome: SpeedtestOutcome) -> Self {
        Self {
            index_id,
            outcome,
            detail: None,
        }
    }

    fn from_error(index_id: String, error: &SpeedtestError) -> Self {
        Self {
            index_id,
            outcome: speedtest_outcome(error),
            detail: speedtest_detail(error),
        }
    }
}

pub trait SpeedtestProbe: Send + Sync {
    fn realping(
        &self,
        socks_port: u16,
        speed_test_item: SpeedTestItem,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>>;
}

#[derive(Clone, Default)]
pub struct ReqwestSpeedtestProbe;

impl SpeedtestProbe for ReqwestSpeedtestProbe {
    fn realping(
        &self,
        socks_port: u16,
        speed_test_item: SpeedTestItem,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let client = SocksHttpProbe::new(socks_port)?;
            let url = if speed_test_item.speed_ping_test_url.trim().is_empty() {
                REALPING_FALLBACK_URL
            } else {
                speed_test_item.speed_ping_test_url.as_str()
            };
            let timeout = Duration::from_secs(
                u64::try_from(speed_test_item.speed_test_timeout.max(1)).unwrap_or(1),
            );
            let delay = client.best_latency(url, timeout, 2, &cancel).await?;

            let lookup = client
                .lookup_country(&speed_test_item.ipapi_url, Duration::from_secs(5), &cancel)
                .await;
            Ok(RealPingProbeResult {
                delay,
                country_code: lookup
                    .as_ref()
                    .and_then(|result| result.country_code.clone()),
                ip_info: lookup.map(|result| result.text),
            })
        })
    }
}

pub trait SpeedtestCoreSession: Send {
    /// Tears the probe core down off the Tokio workers; stopping a child blocks
    /// until the reaper thread has killed and waited it. `Drop` stays as a
    /// best-effort fallback for panics and early returns, and is what the
    /// default implementation falls back to.
    fn close(self: Box<Self>) -> BoxFuture<'static, ()> {
        drop(self);
        Box::pin(async {})
    }
}

pub trait SpeedtestCoreBackend: Send + Sync {
    fn start(
        &self,
        core_type: CoreType,
        entries: Vec<SpeedtestConfigEntry>,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<Box<dyn SpeedtestCoreSession>>>;

    /// Kills every probe core that is still running. Tauri ends the process
    /// with `std::process::exit`, so neither `Drop` nor the pending speedtest
    /// future ever reaps them — the shell has to ask for it explicitly.
    fn stop_all(&self) {}
}

#[derive(Clone)]
pub struct SpeedtestManager {
    probe: Arc<dyn SpeedtestProbe>,
    core_backend: Arc<dyn SpeedtestCoreBackend>,
    paths: AppPaths,
    target_os: TargetOs,
    active_cancel: Arc<Mutex<Option<CancellationFlag>>>,
}

mod core_backend;
mod manager;

pub use core_backend::ProcessSpeedtestCoreBackend;

async fn select_test_items(
    database: &Database,
    config: &AppConfig,
    index_ids: &[String],
) -> Result<Vec<ServerTestItem>> {
    let profiles = if index_ids.is_empty() {
        database.profiles().list().await?
    } else {
        let ids = index_ids.iter().collect::<HashSet<_>>();
        let mut selected = Vec::new();
        for profile in database.profiles().list().await? {
            if ids.contains(&profile.index_id) {
                selected.push(profile);
            }
        }
        selected
    };

    let base_port = config
        .inbound
        .first()
        .map_or(DEFAULT_LOCAL_PORT, |inbound| inbound.local_port)
        + InboundProtocol::speedtest.port_offset();

    profiles
        .into_iter()
        .enumerate()
        .filter(|(_, profile)| profile.port() > 0)
        .map(|(queue_num, profile)| {
            let socks_port_i32 =
                base_port.saturating_add(i32::try_from(queue_num).unwrap_or(i32::MAX));
            let socks_port = u16::try_from(socks_port_i32)
                .map_err(|_| SpeedtestError::InvalidSocksPort(socks_port_i32))?;
            Ok(ServerTestItem {
                index_id: profile.index_id.clone(),
                socks_port,
                profile,
            })
        })
        .collect()
}

fn group_prepared_items(
    prepared: Vec<PreparedSpeedtestItem>,
) -> Vec<(CoreType, Vec<PreparedSpeedtestItem>)> {
    let mut groups: Vec<(CoreType, Vec<PreparedSpeedtestItem>)> = Vec::new();
    for item in prepared {
        let core_type = item.entry.context.run_core_type;
        if let Some((_, items)) = groups
            .iter_mut()
            .find(|(candidate, _)| *candidate == core_type)
        {
            items.push(item);
        } else {
            groups.push((core_type, vec![item]));
        }
    }
    groups
}

fn speedtest_page_size(config: &AppConfig, selected_count: usize) -> usize {
    let configured = config
        .speed_test_item
        .speed_test_page_size
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(SPEEDTEST_BATCH_PAGE_SIZE);
    configured.min(selected_count.max(1))
}

/// Pause between batch pages, in **seconds**. The contract field
/// (`SpeedtestSettings::delay_interval_seconds`), the domain field and this
/// conversion all agree on the unit; `speedtest_delay_interval_is_seconds`
/// pins that.
fn speedtest_delay_interval(config: &AppConfig) -> Duration {
    config
        .speed_test_item
        .speed_test_delay_interval_seconds
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0)
        .map(Duration::from_secs)
        .unwrap_or(SPEEDTEST_DELAY_INTERVAL)
}

/// Classify a probe failure into a persisted, translatable outcome.
///
/// Exhaustive on purpose. The previous version ended in `_ => raw`, which put
/// the error's own `Display` in front of the user and into `profile_ex` — and
/// for `WriteConfig`/`CreateConfigDir`/`RemoveConfig` that text embeds the
/// app-data path, which embeds the OS user name.
fn speedtest_outcome(error: &SpeedtestError) -> SpeedtestOutcome {
    match error {
        SpeedtestError::Cancelled => SpeedtestOutcome::Cancelled,
        SpeedtestError::Network(source) => network_probe_outcome(source),
        SpeedtestError::Io(source) => io_outcome(source.kind()),
        // The profile itself could not be turned into a config.
        SpeedtestError::Validation { .. } | SpeedtestError::SingboxConfig(_) => {
            SpeedtestOutcome::InvalidProfile
        }
        // The test core could not be found, written out, or launched.
        SpeedtestError::CoreInfo(_)
        | SpeedtestError::MissingCoreInfo(_)
        | SpeedtestError::Path(_)
        | SpeedtestError::Process(_)
        | SpeedtestError::CreateConfigDir { .. }
        | SpeedtestError::WriteConfig { .. }
        | SpeedtestError::RemoveConfig { .. } => SpeedtestOutcome::CoreUnavailable,
        SpeedtestError::NoAvailablePort(_) | SpeedtestError::InvalidSocksPort(_) => {
            SpeedtestOutcome::NoAvailablePort
        }
        SpeedtestError::Database(_)
        | SpeedtestError::Profile(_)
        | SpeedtestError::JobLockPoisoned
        | SpeedtestError::BackgroundTask(_) => SpeedtestOutcome::Failed,
    }
}

fn network_probe_outcome(error: &NetworkProbeError) -> SpeedtestOutcome {
    match error {
        NetworkProbeError::Cancelled => SpeedtestOutcome::Cancelled,
        NetworkProbeError::Http(source) if source.is_timeout() => SpeedtestOutcome::TimedOut,
        NetworkProbeError::Http(source) if source.is_connect() => {
            SpeedtestOutcome::ProxyConnectFailed
        }
        _ => SpeedtestOutcome::Failed,
    }
}

const fn io_outcome(kind: io::ErrorKind) -> SpeedtestOutcome {
    match kind {
        io::ErrorKind::TimedOut => SpeedtestOutcome::TimedOut,
        io::ErrorKind::ConnectionRefused => SpeedtestOutcome::ProxyConnectionRefused,
        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted => {
            SpeedtestOutcome::ProxyConnectionClosed
        }
        _ => SpeedtestOutcome::ProxyConnectFailed,
    }
}

/// A finished measurement is `Completed`; the probes report `-1` when they
/// gave up without raising, and that is a plain failure.
const fn measured_outcome(delay: i32) -> SpeedtestOutcome {
    if delay < 0 {
        SpeedtestOutcome::Failed
    } else {
        SpeedtestOutcome::Completed
    }
}

/// The optional technical line shown under a failed probe.
///
/// Only HTTP errors whose `Display` cannot name a local path qualify,
/// and they are redacted: a probe URL carries the user's
/// own test endpoint. Everything else contributes its code and nothing more.
fn speedtest_detail(error: &SpeedtestError) -> Option<String> {
    match error {
        SpeedtestError::Network(source @ NetworkProbeError::Http(_)) => {
            Some(redact_urls(&source.to_string()))
        }
        _ => None,
    }
}

/// Marks the whole selection as pending before the first probe starts.
///
/// Conditional writes share one transaction so large selections do not pay
/// for a journalled commit per profile. Callbacks fire after commit, and only
/// for connections whose configuration still matches the selected snapshot.
async fn clear_previous_results<F>(
    database: &Database,

    selected: &[ServerTestItem],
    on_result: &F,
) -> Result<()>
where
    F: Fn(SpeedtestResult) + Send + Sync,
{
    let unit_of_work = database.begin().await?;
    let mut pending = Vec::new();
    for item in selected {
        let result = make_pending_result(item.index_id.clone());
        if unit_of_work
            .profile_exs()
            .set_probe_result(&item.profile, &result)
            .await?
        {
            pending.push(result);
        }
    }
    unit_of_work.commit().await?;
    for result in pending {
        on_result(result);
    }

    Ok(())
}

fn make_pending_result(index_id: String) -> SpeedtestResult {
    SpeedtestResult {
        index_id,
        delay: Some(0),
        outcome: SpeedtestOutcome::Testing,
        detail: None,
        ip_info: None,
        country_code: None,
    }
}

/// Terminal result for a profile that never got tested. It clears the pending
/// `Testing`/`Waiting` marker `clear_previous_results` wrote, so a failed or
/// cancelled run cannot leave rows stuck in that state across restarts.
fn make_failure_result(
    index_id: String,
    outcome: SpeedtestOutcome,
    detail: Option<String>,
) -> SpeedtestResult {
    SpeedtestResult {
        index_id,
        delay: Some(-1),

        outcome,
        detail,
        ip_info: None,
        country_code: None,
    }
}

async fn persist_speedtest_result(
    database: &Database,
    result: &SpeedtestResult,
    profile: &ProfileItem,
) -> Result<bool> {
    Ok(database
        .profile_exs()
        .set_probe_result(profile, result)
        .await?)
}

fn check_cancelled(cancel: &CancellationFlag) -> Result<()> {
    if is_cancelled(cancel) {
        Err(SpeedtestError::Cancelled)
    } else {
        Ok(())
    }
}

fn is_cancelled(cancel: &CancellationFlag) -> bool {
    cancel.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet as StdHashSet,
        net::TcpListener as StdTcpListener,
        sync::{atomic::AtomicUsize, Mutex as StdMutex},
    };

    use voya_core::{ProfileExItem, ProfileProtocol, ServerEndpoint};
    use voya_db::Database;

    use super::*;

    #[derive(Default)]
    struct RecordingProbe {
        calls: Arc<StdMutex<Vec<String>>>,
        /// How many `realping` calls hold open until the run is cancelled, so
        /// a test can block the first run and let a superseding one through.
        blocking_realpings: Arc<AtomicUsize>,
    }

    impl RecordingProbe {
        fn calls(&self) -> Vec<String> {
            self.calls
                .lock()
                .expect("speedtest test operation should succeed")
                .clone()
        }
    }

    impl SpeedtestProbe for RecordingProbe {
        fn realping(
            &self,
            socks_port: u16,
            _speed_test_item: SpeedTestItem,
            cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<RealPingProbeResult>> {
            let calls = Arc::clone(&self.calls);
            let blocking = Arc::clone(&self.blocking_realpings);
            Box::pin(async move {
                calls
                    .lock()
                    .expect("speedtest test operation should succeed")
                    .push(format!("realping:{socks_port}"));
                let blocks = blocking
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |budget| {
                        budget.checked_sub(1)
                    })
                    .is_ok();
                if blocks {
                    while !is_cancelled(&cancel) {
                        time::sleep(Duration::from_millis(10)).await;
                    }
                    return Err(SpeedtestError::Cancelled);
                }
                Ok(RealPingProbeResult {
                    delay: 44,
                    ip_info: Some("US".to_string()),
                    country_code: Some("US".to_string()),
                })
            })
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct RecordedCoreStart {
        core_type: CoreType,
        ports: Vec<i32>,
    }

    #[derive(Default)]
    struct RecordingCoreBackend {
        starts: Arc<StdMutex<Vec<RecordedCoreStart>>>,
        active: Arc<AtomicUsize>,
        stop_all_calls: Arc<AtomicUsize>,
        /// Makes `start` fail the way a missing core binary or a readiness
        /// timeout does.
        start_failure: bool,
        /// Cancels from inside `start`, the way the real backend does while it
        /// waits for the probe core's SOCKS ports.
        cancel_in_start: bool,
    }

    impl RecordingCoreBackend {
        fn starts(&self) -> Vec<RecordedCoreStart> {
            self.starts
                .lock()
                .expect("speedtest test operation should succeed")
                .clone()
        }

        fn stop_all_calls(&self) -> usize {
            self.stop_all_calls.load(Ordering::SeqCst)
        }
    }

    impl SpeedtestCoreBackend for RecordingCoreBackend {
        fn start(
            &self,
            core_type: CoreType,
            entries: Vec<SpeedtestConfigEntry>,
            cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<Box<dyn SpeedtestCoreSession>>> {
            let starts = Arc::clone(&self.starts);
            let active = Arc::clone(&self.active);
            let start_failure = self.start_failure;
            let cancel_in_start = self.cancel_in_start;
            Box::pin(async move {
                starts
                    .lock()
                    .expect("speedtest test operation should succeed")
                    .push(RecordedCoreStart {
                        core_type,
                        ports: entries.iter().map(|entry| entry.port).collect(),
                    });
                if cancel_in_start {
                    cancel.store(true, Ordering::SeqCst);
                    return Err(SpeedtestError::Cancelled);
                }
                if start_failure {
                    return Err(SpeedtestError::Io(io::Error::new(
                        io::ErrorKind::NotFound,
                        "no probe core",
                    )));
                }
                active.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(RecordingCoreSession { active }) as Box<dyn SpeedtestCoreSession>)
            })
        }

        fn stop_all(&self) {
            self.stop_all_calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct RecordingCoreSession {
        active: Arc<AtomicUsize>,
    }

    impl Drop for RecordingCoreSession {
        fn drop(&mut self) {
            self.active.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl SpeedtestCoreSession for RecordingCoreSession {}

    struct EditingProbe {
        database: Database,
        delete: bool,
    }

    impl SpeedtestProbe for EditingProbe {
        fn realping(
            &self,
            _: u16,
            _: SpeedTestItem,
            _: CancellationFlag,
        ) -> BoxFuture<'static, Result<RealPingProbeResult>> {
            let database = self.database.clone();
            let delete = self.delete;
            Box::pin(async move {
                if delete {
                    database.profiles().delete("a").await?;
                    database.profile_exs().delete_orphans().await?;
                } else {
                    let mut profile = database.profiles().get("a").await?.expect("test profile");
                    profile.transport = Some(voya_core::ProfileTransport::Websocket {
                        host: None,
                        path: Some("/changed".into()),
                    });
                    database.profiles().upsert(&profile).await?;
                }
                Ok(RealPingProbeResult {
                    delay: 44,
                    ip_info: None,
                    country_code: Some("US".into()),
                })
            })
        }
    }

    #[tokio::test]
    async fn speedtest_discards_results_for_nodes_edited_or_deleted_during_the_probe() {
        for delete in [false, true] {
            let database = Database::connect_in_memory().await.expect("database");
            insert_profile(&database, "a", 443).await;
            let backend = Arc::new(RecordingCoreBackend::default());
            let manager = SpeedtestManager::with_probe_and_backend(
                test_paths(),
                Arc::new(EditingProbe {
                    database: database.clone(),
                    delete,
                }),
                backend.clone(),
            );
            let events = StdMutex::new(Vec::new());
            let run = manager
                .run_with_callback(
                    &database,
                    &AppConfig::default(),
                    vec!["a".into()],
                    |result| events.lock().expect("events").push(result),
                )
                .await
                .expect("run");
            assert!(run.results.is_empty());
            assert!(events
                .lock()
                .expect("events")
                .iter()
                .all(|result| result.country_code.is_none()));
            let stored = database.profile_exs().get("a").await.expect("read");
            assert!(stored.as_ref().is_none_or(|row| row.country_code.is_none()));
            if let Some(row) = stored {
                assert_eq!(
                    row.message, None,
                    "an edited node cannot remain stuck testing"
                );
            }
            assert_eq!(backend.active.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn speedtest_manager_realping_persists_latency_and_ip_info() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());
        let config = AppConfig::default();

        let run = manager
            .run(&database, &config, vec!["a".to_string()])
            .await
            .expect("speedtest test operation should succeed");

        assert!(!run.cancelled);
        assert_eq!(run.completed_count, 1);
        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 1);
        let port = starts[0].ports[0];
        assert_eq!(probe.calls(), vec![format!("realping:{port}")]);
        let profile_ex = database
            .profile_exs()
            .get("a")
            .await
            .expect("speedtest test operation should succeed")
            .expect("speedtest test operation should succeed");
        assert_eq!(profile_ex.delay, 44);
        assert_eq!(profile_ex.ip_info.as_deref(), Some("US"));
        assert_eq!(profile_ex.country_code.as_deref(), Some("US"));
        assert_eq!(run.results[0].country_code.as_deref(), Some("US"));
    }

    #[tokio::test]
    async fn speedtest_manager_latency_with_empty_selection_uses_all_profiles() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());

        let run = manager
            .run(&database, &AppConfig::default(), Vec::new())
            .await
            .expect("speedtest test operation should succeed");

        assert_eq!(run.selected_count, 2);
        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 2);
        assert_eq!(
            probe.calls(),
            starts[0]
                .ports
                .iter()
                .map(|port| format!("realping:{port}"))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn speedtest_manager_realping_batches_one_temp_core() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());

        manager
            .run(&database, &AppConfig::default(), Vec::new())
            .await
            .expect("speedtest test operation should succeed");

        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 2);
        assert_eq!(
            probe.calls(),
            starts[0]
                .ports
                .iter()
                .map(|port| format!("realping:{port}"))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn speedtest_manager_prepare_reserves_ports_across_batch() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe, backend.clone());
        let mut config = AppConfig::default();

        let reserved_base = reserve_speedtest_base_port(&mut config);
        let reserved_port = i32::from(
            reserved_base
                .local_addr()
                .expect("speedtest test operation should succeed")
                .port(),
        );

        manager
            .run(&database, &config, Vec::new())
            .await
            .expect("speedtest test operation should succeed");

        let starts = backend.starts();
        let ports = starts
            .iter()
            .flat_map(|start| start.ports.iter().copied())
            .collect::<Vec<_>>();
        let unique_ports = ports.iter().copied().collect::<StdHashSet<_>>();
        assert_eq!(ports.len(), 2);
        assert_eq!(unique_ports.len(), ports.len());
        assert!(!unique_ports.contains(&reserved_port));
    }

    #[tokio::test]
    async fn speedtest_manager_cancel_stops_active_jobs() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe {
            blocking_realpings: Arc::new(AtomicUsize::new(usize::MAX)),
            ..RecordingProbe::default()
        });
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());
        let task_manager = manager.clone();
        let config = AppConfig::default();

        let handle = tokio::spawn(async move {
            task_manager
                .run(&database, &config, Vec::new())
                .await
                .expect("speedtest test operation should succeed")
        });

        loop {
            if probe
                .calls()
                .iter()
                .any(|call| call.starts_with("realping:"))
            {
                break;
            }
            time::sleep(Duration::from_millis(10)).await;
        }

        assert!(manager
            .cancel()
            .expect("speedtest test operation should succeed"));
        let run = handle
            .await
            .expect("speedtest test operation should succeed");

        assert!(run.cancelled);
        assert_eq!(run.completed_count, 1);
        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 2);
        assert_eq!(backend.active.load(Ordering::SeqCst), 0);
        assert_eq!(
            probe.calls(),
            vec![format!("realping:{}", starts[0].ports[0])]
        );
        assert!(
            !manager
                .status()
                .expect("speedtest test operation should succeed")
                .running
        );
    }

    #[tokio::test]
    async fn speedtest_manager_records_a_failed_core_start_per_profile() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend {
            start_failure: true,
            ..RecordingCoreBackend::default()
        });
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());

        let run = manager
            .run(&database, &AppConfig::default(), Vec::new())
            .await
            .expect("a core that will not start must not abort the run");

        assert!(!run.cancelled);
        assert!(
            probe.calls().is_empty(),
            "no profile can be probed without a core"
        );
        assert_eq!(run.results.len(), 2);
        for index_id in ["a", "b"] {
            let profile_ex = profile_ex_row(&database, index_id).await;
            assert_eq!(profile_ex.delay, -1);
            assert_ne!(
                profile_ex.message.as_deref(),
                Some(SpeedtestOutcome::Testing.as_stored()),
                "{index_id} must not stay pending after a failed run"
            );
        }
    }

    #[tokio::test]
    async fn speedtest_manager_reports_a_cancel_during_core_start_as_a_cancelled_run() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend {
            cancel_in_start: true,
            ..RecordingCoreBackend::default()
        });
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());

        let run = manager
            .run(&database, &AppConfig::default(), Vec::new())
            .await
            .expect("a cancel while a core starts is a cancelled run, not an error");

        assert!(run.cancelled);
        assert_eq!(run.completed_count, 0);
        assert!(probe.calls().is_empty());
        for index_id in ["a", "b"] {
            let profile_ex = profile_ex_row(&database, index_id).await;
            assert_eq!(profile_ex.delay, -1);
            assert_eq!(profile_ex.message.as_deref(), Some("cancelled"));
        }
    }

    #[tokio::test]
    async fn speedtest_manager_skips_invalid_profiles_and_tests_the_rest() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "good", 443).await;
        insert_profile_with_uuid(&database, "bad", 8443, "not-a-guid").await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());

        let run = manager
            .run(&database, &AppConfig::default(), Vec::new())
            .await
            .expect("an invalid profile must not abort the run");

        assert_eq!(run.selected_count, 2);
        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(
            starts[0].ports.len(),
            1,
            "only the valid profile reaches the temporary core"
        );
        assert_eq!(probe.calls().len(), 1);
        let good = profile_ex_row(&database, "good").await;
        assert_eq!(good.delay, 44);
        let bad = profile_ex_row(&database, "bad").await;
        assert_eq!(bad.delay, -1);
        assert_eq!(
            bad.message.as_deref(),
            Some(SpeedtestOutcome::InvalidProfile.as_stored()),
            "the validator rejection is reported as the profile's outcome"
        );
    }

    #[tokio::test]
    async fn speedtest_manager_pages_batches_and_waits_between_them_in_seconds() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());
        let mut config = AppConfig::default();
        config.speed_test_item.speed_test_page_size = Some(1);
        config.speed_test_item.speed_test_delay_interval_seconds = Some(1);

        let started = Instant::now();
        manager
            .run(&database, &config, Vec::new())
            .await
            .expect("speedtest test operation should succeed");
        let elapsed = started.elapsed();

        let starts = backend.starts();
        assert_eq!(starts.len(), 2, "one temporary core per page");
        assert!(starts.iter().all(|start| start.ports.len() == 1));
        assert_eq!(probe.calls().len(), 2);
        assert!(
            elapsed >= Duration::from_secs(1),
            "the configured delay interval is seconds, not milliseconds ({elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_secs(10),
            "one page boundary means exactly one pause ({elapsed:?})"
        );
    }

    #[tokio::test]
    async fn speedtest_manager_supersedes_an_overlapping_run() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        // Only the first run's probe blocks, so the superseding run can finish
        // while its predecessor is still parked.
        let probe = Arc::new(RecordingProbe {
            blocking_realpings: Arc::new(AtomicUsize::new(1)),
            ..RecordingProbe::default()
        });
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());
        let config = AppConfig::default();

        let first_manager = manager.clone();
        let first_database = database.clone();
        let first_config = config.clone();
        let first = tokio::spawn(async move {
            first_manager
                .run(&first_database, &first_config, Vec::new())
                .await
                .expect("the superseded run still completes")
        });

        loop {
            if probe
                .calls()
                .iter()
                .any(|call| call.starts_with("realping:"))
            {
                break;
            }
            time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            manager
                .status()
                .expect("speedtest test operation should succeed")
                .running
        );

        let second = manager
            .run(&database, &config, Vec::new())
            .await
            .expect("the superseding run completes");
        let first = first
            .await
            .expect("speedtest test operation should succeed");

        assert!(
            first.cancelled,
            "starting a run supersedes the previous one"
        );
        assert!(
            !second.cancelled,
            "the superseding run must not inherit its predecessor's cancel flag"
        );
        assert!(
            !manager
                .status()
                .expect("speedtest test operation should succeed")
                .running,
            "only the run that owns the slot may release it"
        );
    }

    #[test]
    fn speedtest_manager_shutdown_cancels_and_reaps_probe_cores() {
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager = SpeedtestManager::with_probe_and_backend(
            test_paths(),
            Arc::new(RecordingProbe::default()),
            backend.clone(),
        );

        manager.shutdown();

        assert_eq!(
            backend.stop_all_calls(),
            1,
            "exit must reap probe cores; Drop never runs under std::process::exit"
        );
    }

    #[test]
    fn speedtest_delay_interval_is_seconds() {
        let mut config = AppConfig::default();
        assert_eq!(speedtest_delay_interval(&config), SPEEDTEST_DELAY_INTERVAL);

        config.speed_test_item.speed_test_delay_interval_seconds = Some(3);
        assert_eq!(
            speedtest_delay_interval(&config),
            Duration::from_secs(3),
            "the delay interval is configured in seconds"
        );

        config.speed_test_item.speed_test_delay_interval_seconds = Some(0);
        assert_eq!(speedtest_delay_interval(&config), SPEEDTEST_DELAY_INTERVAL);
    }

    async fn profile_ex_row(database: &Database, index_id: &str) -> ProfileExItem {
        database
            .profile_exs()
            .get(index_id)
            .await
            .expect("speedtest test operation should succeed")
            .expect("speedtest test operation should succeed")
    }

    async fn insert_profile(database: &Database, index_id: &str, port: i32) {
        insert_profile_with_uuid(
            database,
            index_id,
            port,
            "00000000-0000-0000-0000-000000000000",
        )
        .await;
    }

    async fn insert_profile_with_uuid(database: &Database, index_id: &str, port: i32, uuid: &str) {
        let profile = ProfileItem {
            index_id: index_id.to_string(),
            remarks: index_id.to_string(),
            protocol: ProfileProtocol::Vmess {
                server: ServerEndpoint {
                    address: "127.0.0.1".to_string(),
                    port,
                },
                uuid: uuid.to_string(),
                cipher: Some("auto".to_string()),
            },
            ..ProfileItem::default()
        };
        let profile_ex = ProfileExItem {
            index_id: index_id.to_string(),
            ..ProfileExItem::default()
        };
        database
            .profiles()
            .upsert_with_profile_ex(&profile, &profile_ex)
            .await
            .expect("speedtest test operation should succeed");
    }

    fn test_paths() -> AppPaths {
        AppPaths::new(
            std::env::temp_dir().join(format!("voyavpn-speedtest-tests-{}", uuid::Uuid::new_v4())),
        )
    }

    fn reserve_speedtest_base_port(config: &mut AppConfig) -> StdTcpListener {
        let speedtest_offset = InboundProtocol::speedtest.port_offset();
        for _ in 0..100 {
            let listener = StdTcpListener::bind((LOOPBACK_ADDR, 0))
                .expect("speedtest test operation should succeed");
            let base_port = listener
                .local_addr()
                .expect("speedtest test operation should succeed")
                .port();
            let local_port = i32::from(base_port) - speedtest_offset;
            if local_port > 0 && base_port < u16::MAX - 4 {
                config.inbound[0].local_port = local_port;
                return listener;
            }
        }
        panic!("speedtest test operation should succeed");
    }
}
