use std::{
    collections::{HashMap, HashSet},
    io,
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};

use futures_util::future::BoxFuture;
use thiserror::Error;
use tokio::time;
use voya_contracts::{SpeedtestOutcome, SpeedtestResult, SpeedtestRunResult, SpeedtestStatus};
use voya_core::{
    generate_singbox_speedtest_config_json, AppConfig, CoreConfigContextBuilder, LocalPort,
    ProfileItem, SpeedtestConfig, SpeedtestConfigEntry, DEFAULT_LOCAL_PORT,
    DEFAULT_SPEED_PING_TEST_URL, LOOPBACK,
};
use voya_db::{Database, DbError};
use voya_net::probe::{is_cancelled, tcp_port_is_open, NetworkProbeError, SocksHttpProbe};
use voya_platform::{
    coreinfo::{CoreInfoError, TargetOs},
    filesystem,
    paths::{AppPaths, PathError},
    process::{ProcessError, ProcessHandle, ProcessRole, ProcessRunner, ProcessSpawn},
};

use crate::backoff::exponential_delay;
use crate::redaction::redact_urls;
use crate::runtime::load_runtime_core_gen_env;

const SPEEDTEST_BATCH_PAGE_SIZE: usize = 1000;
const SPEEDTEST_DELAY_INTERVAL: Duration = Duration::from_secs(1);
/// Latency probes in flight at once, through probe cores or the running core;
/// sing-box's own group test runs ten.
const SPEEDTEST_CONCURRENCY: usize = 8;
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);

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
    #[error("failed to create speedtest config directory {path}: {source}")]
    CreateConfigDir { path: PathBuf, source: io::Error },
    #[error("failed to write speedtest config {path}: {source}")]
    WriteConfig { path: PathBuf, source: io::Error },
    #[error("no available speedtest port at or after {0}")]
    NoAvailablePort(i32),
    #[error("speedtest local SOCKS port {0} is outside the valid range")]
    InvalidSocksPort(i32),
    #[error("speedtest background task failed: {0}")]
    BackgroundTask(String),
    /// The platform's own answer to "start a probe core": a phone's host runs
    /// one in its process rather than spawning a child, so its failures arrive
    /// as a message rather than as a [`ProcessError`].
    #[error("the host could not run a probe core: {0}")]
    ProbeCoreHost(String),
    #[error("select at least one node to test")]
    EmptySelection,
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

/// A selected profile the run could not test. One unusable profile must not
/// abort the run, so validation and port-reservation failures are reported as
/// per-item results instead.
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
        speed_test: SpeedtestConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>>;
}

#[derive(Clone, Default)]
pub struct ReqwestSpeedtestProbe;

impl SpeedtestProbe for ReqwestSpeedtestProbe {
    fn realping(
        &self,
        socks_port: u16,
        speed_test: SpeedtestConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let client = SocksHttpProbe::new(socks_port)?;
            let url = latency_test_url(&speed_test);
            let timeout =
                Duration::from_secs(u64::try_from(speed_test.timeout_seconds.max(1)).unwrap_or(1));
            let delay = client.best_latency(url, timeout, 2, &cancel).await?;

            let lookup = client
                .lookup_country(&speed_test.ip_lookup_url, Duration::from_secs(5), &cancel)
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

/// The URL a latency probe fetches: the configured one, else the default.
fn latency_test_url(item: &SpeedtestConfig) -> &str {
    if item.latency_url.trim().is_empty() {
        DEFAULT_SPEED_PING_TEST_URL
    } else {
        item.latency_url.as_str()
    }
}

/// Starts the throwaway sing-box a disconnected probe run measures through.
///
/// The one thing that is not portable about a probe core. The desktop spawns a
/// child process; a phone may not spawn anything, so its host runs Libbox
/// inside the app process and hands back something it can stop. Everything
/// around it — generating the config, reserving and waiting for the SOCKS
/// ports, tearing the core down — is the same on both, and lives in
/// [`SpeedtestManager`].
pub trait ProbeCoreLauncher: Send + Sync {
    /// Starts a probe core for a generated sing-box configuration.
    ///
    /// Blocking is expected: the caller runs this on a blocking thread, the
    /// way writing a config and spawning a child process demand.
    fn start(&self, config_json: String) -> Result<Box<dyn ProbeCore>>;

    /// Stops every probe core this launcher still has running. Tauri ends the
    /// process with `std::process::exit`, so neither `Drop` nor the pending
    /// speedtest future ever reaps them — the shell has to ask for it
    /// explicitly.
    fn stop_all(&self) {}
}

/// One running probe core, as the thing that stops it.
pub trait ProbeCore: Send + Sync {
    /// Stops the core and releases whatever starting it took — a config file
    /// on the desktop, an in-process instance on a phone. Blocking, for the
    /// same reason [`ProbeCoreLauncher::start`] is.
    fn stop(self: Box<Self>);
}

/// Lock a speedtest mutex even after a panicking holder poisoned it. Both
/// guarded values survive a panic intact: the probe registry still holds the
/// child handles it exists to reap, and the job slot is a single `Option`.
/// Refusing the lock would leak probe cores, or break cancel and status for
/// the rest of the process.
fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

mod core_backend;
mod manager;
mod running_core;

pub use core_backend::ProcessProbeCoreLauncher;
pub use manager::{start_probe_core_page, ProbeCoreSession, SpeedtestManager};
pub use running_core::{RunningCoreProbe, SupervisorRunningCoreProbe};

async fn select_test_items(
    database: &Database,
    config: &AppConfig,
    index_ids: &[String],
) -> Result<Vec<ServerTestItem>> {
    // Both come back in list order, which the port assignment below relies on.
    let profiles = if index_ids.is_empty() {
        database.profiles().list().await?
    } else {
        database.profiles().list_by_ids(index_ids).await?
    };

    let base_port = config
        .inbounds
        .first()
        .map_or(DEFAULT_LOCAL_PORT, |inbound| inbound.local_port)
        + LocalPort::Speedtest.port_offset();

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

fn speedtest_page_size(config: &AppConfig, selected_count: usize) -> usize {
    let configured = config
        .speed_test
        .page_size
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
        .speed_test
        .delay_interval_seconds
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0)
        .map(Duration::from_secs)
        .unwrap_or(SPEEDTEST_DELAY_INTERVAL)
}

/// Classify a probe failure into a persisted, translatable outcome.
///
/// Exhaustive on purpose. The previous version ended in `_ => raw`, which put
/// the error's own `Display` in front of the user and into `profile_ex` — and
/// for `WriteConfig`/`CreateConfigDir` that text embeds the
/// app-data path, which embeds the OS user name.
fn speedtest_outcome(error: &SpeedtestError) -> SpeedtestOutcome {
    match error {
        SpeedtestError::Cancelled => SpeedtestOutcome::Cancelled,
        SpeedtestError::Network(source) => network_probe_outcome(source),
        SpeedtestError::Io(source) => io_outcome(source.kind()),
        // The profile itself could not be turned into a config.
        SpeedtestError::SingboxConfig(_) => SpeedtestOutcome::InvalidProfile,
        // The test core could not be found, written out, or launched.
        SpeedtestError::CoreInfo(_)
        | SpeedtestError::Path(_)
        | SpeedtestError::Process(_)
        | SpeedtestError::ProbeCoreHost(_)
        | SpeedtestError::CreateConfigDir { .. }
        | SpeedtestError::WriteConfig { .. } => SpeedtestOutcome::CoreUnavailable,
        SpeedtestError::NoAvailablePort(_) | SpeedtestError::InvalidSocksPort(_) => {
            SpeedtestOutcome::NoAvailablePort
        }
        SpeedtestError::Database(_)
        | SpeedtestError::Profile(_)
        | SpeedtestError::EmptySelection
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
    on_results: &F,
) -> Result<()>
where
    F: Fn(Vec<SpeedtestResult>) + Send + Sync,
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
    if !pending.is_empty() {
        on_results(pending);
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

fn check_cancelled(cancel: &CancellationFlag) -> Result<()> {
    if is_cancelled(cancel) {
        Err(SpeedtestError::Cancelled)
    } else {
        Ok(())
    }
}

/// Resolves once `cancel` is set, for racing a run's result stream.
async fn cancelled(cancel: &CancellationFlag) {
    while !is_cancelled(cancel) {
        time::sleep(CANCEL_POLL_INTERVAL).await;
    }
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
    pub(super) struct RecordingProbe {
        calls: Arc<StdMutex<Vec<String>>>,
        /// How many `realping` calls hold open until the run is cancelled, so
        /// a test can block the first run and let a superseding one through.
        blocking_realpings: Arc<AtomicUsize>,
    }

    impl RecordingProbe {
        pub(super) fn calls(&self) -> Vec<String> {
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
            _speed_test: SpeedtestConfig,
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
    pub(super) struct RecordedCoreStart {
        pub(super) ports: Vec<i32>,
    }

    fn ports_from_config(config_json: &str) -> Vec<i32> {
        let value: serde_json::Value =
            serde_json::from_str(config_json).expect("generated speedtest config");
        value["inbounds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|inbound| inbound["listen_port"].as_i64())
            .map(|port| i32::try_from(port).unwrap_or(i32::MAX))
            .collect()
    }

    #[derive(Default)]
    pub(super) struct RecordingCoreBackend {
        starts: Arc<StdMutex<Vec<RecordedCoreStart>>>,
        active: Arc<AtomicUsize>,
        stop_all_calls: Arc<AtomicUsize>,
        /// Makes `start` fail the way a missing core binary or a readiness
        /// timeout does.
        start_failure: bool,
        /// Cancels from inside `start`, the way the real backend does while it
        /// waits for the probe core's SOCKS ports.
        cancel_in_start: bool,
        /// Binds the port after a page's last one and keeps it, the way some
        /// other program could while the page runs.
        occupy_next_port: bool,
        occupied: Arc<StdMutex<Vec<StdTcpListener>>>,
    }

    impl RecordingCoreBackend {
        pub(super) fn starts(&self) -> Vec<RecordedCoreStart> {
            self.starts
                .lock()
                .expect("speedtest test operation should succeed")
                .clone()
        }

        fn stop_all_calls(&self) -> usize {
            self.stop_all_calls.load(Ordering::SeqCst)
        }
    }

    impl ProbeCoreLauncher for RecordingCoreBackend {
        fn start(&self, config_json: String) -> Result<Box<dyn ProbeCore>> {
            let ports = ports_from_config(&config_json);
            if self.occupy_next_port && self.starts().is_empty() {
                let next = ports
                    .iter()
                    .max()
                    .and_then(|port| u16::try_from(port + 1).ok())
                    .expect("a page has ports");
                let listener = StdTcpListener::bind((LOOPBACK, next))
                    .expect("the fixture must actually hold the next port");
                self.occupied.lock().expect("occupied").push(listener);
            }
            // Bind each page port so the readiness wait succeeds the way a real
            // probe core's SOCKS listeners do.
            let mut listeners = Vec::new();
            for port in &ports {
                if let Ok(listener) =
                    StdTcpListener::bind((LOOPBACK, u16::try_from(*port).unwrap_or(0)))
                {
                    listeners.push(listener);
                }
            }
            self.starts
                .lock()
                .expect("speedtest test operation should succeed")
                .push(RecordedCoreStart {
                    ports: ports.clone(),
                });
            if self.cancel_in_start {
                return Err(SpeedtestError::Cancelled);
            }
            if self.start_failure {
                return Err(SpeedtestError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    "no probe core",
                )));
            }
            self.active.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(RecordingCoreSession {
                active: Arc::clone(&self.active),
                _listeners: listeners,
            }))
        }

        fn stop_all(&self) {
            self.stop_all_calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct RecordingCoreSession {
        active: Arc<AtomicUsize>,
        _listeners: Vec<StdTcpListener>,
    }

    impl Drop for RecordingCoreSession {
        fn drop(&mut self) {
            self.active.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl ProbeCore for RecordingCoreSession {
        fn stop(self: Box<Self>) {}
    }

    struct EditingProbe {
        database: Database,
        delete: bool,
    }

    impl SpeedtestProbe for EditingProbe {
        fn realping(
            &self,
            _: u16,
            _: SpeedtestConfig,
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
            let manager = SpeedtestManager::with_probe_and_launcher(
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
                    |results| events.lock().expect("events").extend(results),
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());
        let config = AppConfig::default();

        let run = manager
            .run_with_callback(&database, &config, vec!["a".to_string()], |_| {})
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());

        let run = manager
            .run_with_callback(&database, &AppConfig::default(), Vec::new(), |_| {})
            .await
            .expect("speedtest test operation should succeed");

        assert_eq!(run.selected_count, 2);
        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 2);
        assert_eq!(
            sorted(probe.calls()),
            sorted(
                starts[0]
                    .ports
                    .iter()
                    .map(|port| format!("realping:{port}"))
                    .collect()
            )
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());

        manager
            .run_with_callback(&database, &AppConfig::default(), Vec::new(), |_| {})
            .await
            .expect("speedtest test operation should succeed");

        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 2);
        assert_eq!(
            sorted(probe.calls()),
            sorted(
                starts[0]
                    .ports
                    .iter()
                    .map(|port| format!("realping:{port}"))
                    .collect()
            )
        );
    }

    #[tokio::test]
    async fn speedtest_manager_reserves_each_page_ports_when_the_page_starts() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let backend = Arc::new(RecordingCoreBackend {
            occupy_next_port: true,
            ..RecordingCoreBackend::default()
        });
        let manager = SpeedtestManager::with_probe_and_launcher(
            test_paths(),
            Arc::new(RecordingProbe::default()),
            backend.clone(),
        );
        let mut config = AppConfig::default();
        config.speed_test.page_size = Some(1);
        // Keep this two-port fixture outside both the shared default 108xx
        // range and the OS ephemeral range: concurrent connect() calls can
        // consume the port immediately after a bind(..., 0) allocation.
        let pair = (20000_u16..30000)
            .find_map(|port| {
                let first = StdTcpListener::bind((LOOPBACK, port)).ok()?;
                let next = StdTcpListener::bind((LOOPBACK, port + 1)).ok()?;
                Some((port, first, next))
            })
            .expect("two available fixture ports");
        config.inbounds[0].local_port = i32::from(pair.0) - LocalPort::Speedtest.port_offset();
        drop(pair);

        manager
            .run_with_callback(&database, &config, Vec::new(), |_| {})
            .await
            .expect("speedtest test operation should succeed");

        let starts = backend.starts();
        assert_eq!(starts.len(), 2, "one core per page");
        let taken_while_first_page_ran = starts[0].ports[0] + 1;
        assert_ne!(
            starts[1].ports[0], taken_while_first_page_ran,
            "the second page must not reuse a port taken while the first ran"
        );
    }

    #[tokio::test]
    async fn speedtest_manager_reserves_unique_free_ports_within_a_page() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe, backend.clone());
        let mut config = AppConfig::default();

        let reserved_base = reserve_speedtest_base_port(&mut config);
        let reserved_port = i32::from(
            reserved_base
                .local_addr()
                .expect("speedtest test operation should succeed")
                .port(),
        );

        manager
            .run_with_callback(&database, &config, Vec::new(), |_| {})
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());
        let task_manager = manager.clone();
        let config = AppConfig::default();
        let database_check = database.clone();

        let handle = tokio::spawn(async move {
            task_manager
                .run_with_callback(&database, &config, Vec::new(), |_| {})
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

        assert!(manager.cancel());
        let run = handle
            .await
            .expect("speedtest test operation should succeed");

        assert!(run.cancelled);
        // The run stops waiting on in-flight probes at the cancel, so both are
        // written back as cancelled by the finalize pass, not by a probe.
        assert_eq!(run.completed_count, 0);
        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 2);
        assert_eq!(backend.active.load(Ordering::SeqCst), 0);
        for index_id in ["a", "b"] {
            let profile_ex = profile_ex_row(&database_check, index_id).await;
            assert_eq!(profile_ex.message.as_deref(), Some("cancelled"));
        }
        assert!(!manager.status().running);
    }

    #[tokio::test]
    async fn speedtest_manager_probes_a_page_concurrently_up_to_the_limit() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        let node_count = SPEEDTEST_CONCURRENCY + 4;
        for index in 0..node_count {
            let port = i32::try_from(1000 + index).expect("port fits");
            insert_profile(&database, &format!("n{index}"), port).await;
        }
        let probe = Arc::new(RecordingProbe {
            blocking_realpings: Arc::new(AtomicUsize::new(usize::MAX)),
            ..RecordingProbe::default()
        });
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());
        let task_manager = manager.clone();
        let task_database = database.clone();

        let handle = tokio::spawn(async move {
            task_manager
                .run_with_callback(&task_database, &AppConfig::default(), Vec::new(), |_| {})
                .await
                .expect("speedtest test operation should succeed")
        });

        while probe.calls().len() < SPEEDTEST_CONCURRENCY {
            time::sleep(Duration::from_millis(10)).await;
        }
        // Every probe blocks, so a slot only frees up at the cancel: nothing
        // past the limit may have started.
        time::sleep(Duration::from_millis(100)).await;
        assert_eq!(probe.calls().len(), SPEEDTEST_CONCURRENCY);

        assert!(manager.cancel());
        let run = handle
            .await
            .expect("speedtest test operation should succeed");

        assert!(run.cancelled);
        assert_eq!(
            probe.calls().len(),
            SPEEDTEST_CONCURRENCY,
            "queued probes never start after a cancel"
        );
        assert_eq!(run.results.len(), node_count);
        for index in 0..node_count {
            let profile_ex = profile_ex_row(&database, &format!("n{index}")).await;
            assert_eq!(profile_ex.message.as_deref(), Some("cancelled"));
        }
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());
        let deliveries = StdMutex::new(Vec::<Vec<(String, SpeedtestOutcome)>>::new());

        let run = manager
            .run_with_callback(&database, &AppConfig::default(), Vec::new(), |results| {
                deliveries.lock().expect("deliveries").push(
                    results
                        .into_iter()
                        .map(|result| (result.index_id, result.outcome))
                        .collect(),
                );
            })
            .await
            .expect("a core that will not start must not abort the run");

        assert!(!run.cancelled);
        assert!(
            probe.calls().is_empty(),
            "no profile can be probed without a core"
        );
        assert_eq!(run.results.len(), 2);
        // The pending markers arrive together, and so does the failed page:
        // one delivery each, not one event per node.
        let pending = SpeedtestOutcome::Testing;
        // The fake backend fails with an I/O `NotFound`.
        let failed = SpeedtestOutcome::ProxyConnectFailed;
        assert_eq!(
            deliveries.into_inner().expect("deliveries"),
            [
                vec![("a".to_string(), pending), ("b".to_string(), pending)],
                vec![("a".to_string(), failed), ("b".to_string(), failed)],
            ]
        );
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());

        let run = manager
            .run_with_callback(&database, &AppConfig::default(), Vec::new(), |_| {})
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());

        let run = manager
            .run_with_callback(&database, &AppConfig::default(), Vec::new(), |_| {})
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

    /// Pages are built as they come up, and an unusable profile does not take
    /// one of a page's slots: page one is `a`, page two skips `b` for `c`.
    #[tokio::test]
    async fn speedtest_manager_reports_an_invalid_profile_when_its_page_comes_up() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile_with_uuid(&database, "b", 8443, "not-a-guid").await;
        insert_profile(&database, "c", 9443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());
        let mut config = AppConfig::default();
        config.speed_test.page_size = Some(1);
        config.speed_test.delay_interval_seconds = Some(1);
        let deliveries = StdMutex::new(Vec::<Vec<(String, SpeedtestOutcome)>>::new());

        let run = manager
            .run_with_callback(&database, &config, Vec::new(), |results| {
                deliveries.lock().expect("deliveries").push(
                    results
                        .into_iter()
                        .map(|result| (result.index_id, result.outcome))
                        .collect(),
                );
            })
            .await
            .expect("an invalid profile must not abort the run");

        assert_eq!(run.selected_count, 3);
        let starts = backend.starts();
        assert_eq!(starts.len(), 2, "the invalid profile does not get a page");
        assert!(starts.iter().all(|start| start.ports.len() == 1));
        assert_eq!(probe.calls().len(), 2);
        // After the pending markers: `a`'s result, then `b` with `c`'s page.
        let deliveries = deliveries.into_inner().expect("deliveries");
        assert_eq!(
            deliveries[1..],
            [
                vec![("a".to_string(), SpeedtestOutcome::Completed)],
                vec![("b".to_string(), SpeedtestOutcome::InvalidProfile)],
                vec![("c".to_string(), SpeedtestOutcome::Completed)],
            ]
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());
        let mut config = AppConfig::default();
        config.speed_test.page_size = Some(1);
        config.speed_test.delay_interval_seconds = Some(1);

        let started = Instant::now();
        manager
            .run_with_callback(&database, &config, Vec::new(), |_| {})
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
            SpeedtestManager::with_probe_and_launcher(test_paths(), probe.clone(), backend.clone());
        let config = AppConfig::default();

        let first_manager = manager.clone();
        let first_database = database.clone();
        let first_config = config.clone();
        let first = tokio::spawn(async move {
            first_manager
                .run_with_callback(&first_database, &first_config, Vec::new(), |_| {})
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
        assert!(manager.status().running);

        let second = manager
            .run_with_callback(&database, &config, Vec::new(), |_| {})
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
            !manager.status().running,
            "only the run that owns the slot may release it"
        );
        // The superseded run writes its untested node back as cancelled on the
        // way out; that must land before the new run's result, not over it.
        let stored = profile_ex_row(&database, "a").await;
        assert_eq!(stored.delay, 44);
        assert_eq!(second.results.len(), 1);
    }

    #[tokio::test]
    async fn speedtest_manager_skips_a_run_superseded_while_queued() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager = SpeedtestManager::with_probe_and_launcher(
            test_paths(),
            Arc::new(RecordingProbe::default()),
            backend.clone(),
        );
        // Hold the run slot so the next run queues behind it.
        let held = Arc::clone(&manager.run_lock).lock_owned().await;

        let queued_manager = manager.clone();
        let queued_database = database.clone();
        let queued = tokio::spawn(async move {
            queued_manager
                .run_with_callback(&queued_database, &AppConfig::default(), Vec::new(), |_| {})
                .await
                .expect("a superseded run is not an error")
        });
        while !manager.status().running {
            time::sleep(Duration::from_millis(10)).await;
        }
        assert!(manager.cancel());
        drop(held);

        let run = queued
            .await
            .expect("speedtest test operation should succeed");
        assert!(run.cancelled);
        assert!(run.results.is_empty());
        assert!(backend.starts().is_empty(), "a skipped run starts no core");
        assert!(
            profile_ex_row(&database, "a").await.message.is_none(),
            "a skipped run leaves no pending marker to clean up"
        );
    }

    #[test]
    fn speedtest_manager_shutdown_cancels_and_reaps_probe_cores() {
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager = SpeedtestManager::with_probe_and_launcher(
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

        config.speed_test.delay_interval_seconds = Some(3);
        assert_eq!(
            speedtest_delay_interval(&config),
            Duration::from_secs(3),
            "the delay interval is configured in seconds"
        );

        config.speed_test.delay_interval_seconds = Some(0);
        assert_eq!(speedtest_delay_interval(&config), SPEEDTEST_DELAY_INTERVAL);
    }

    /// Probes start concurrently, so call order is not part of the contract.
    fn sorted(mut calls: Vec<String>) -> Vec<String> {
        calls.sort();
        calls
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
        let speedtest_offset = LocalPort::Speedtest.port_offset();
        for _ in 0..100 {
            let listener = StdTcpListener::bind((LOOPBACK, 0))
                .expect("speedtest test operation should succeed");
            let base_port = listener
                .local_addr()
                .expect("speedtest test operation should succeed")
                .port();
            let local_port = i32::from(base_port) - speedtest_offset;
            if local_port > 0 && base_port < u16::MAX - 4 {
                config.inbounds[0].local_port = local_port;
                return listener;
            }
        }
        panic!("speedtest test operation should succeed");
    }
}
