use std::{
    collections::HashSet,
    fs, io,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use futures_util::{
    future::BoxFuture,
    stream::{FuturesUnordered, StreamExt},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use tokio::{
    net::{lookup_host, TcpStream},
    time,
};
use voya_core::{
    generate_singbox_speedtest_config_json, generate_xray_speedtest_config_json, AppConfig,
    ConfigType, CoreConfigContextBuilder, CoreType, InboundProtocol, ProfileItem, SpeedActionType,
    SpeedtestConfigEntry, DEFAULT_LOCAL_PORT,
};
use voya_db::{Database, DbError};
use voya_platform::{
    coreinfo::{copy_seed_core_asset, discover_executable, get_core_info, CoreInfoError, TargetOs},
    paths::{AppPaths, PathError, StorageMode},
    process::{ProcessError, ProcessHandle, ProcessRole, ProcessRunner, ProcessSpawn},
};
use voya_udptest::{UdpTestError, UdpTestService};

use crate::profiles::ProfileExManager;
use crate::runtime::{core_launch_plan, load_runtime_core_gen_env};

const TCPING_TIMEOUT: Duration = Duration::from_secs(5);
const REALPING_FALLBACK_URL: &str = "https://www.google.com/generate_204";
const SPEEDTEST_CONFIG_PREFIX: &str = "configTest";
const SPEEDTEST_READY_TIMEOUT: Duration = Duration::from_secs(3);
const SPEEDTEST_READY_INTERVAL: Duration = Duration::from_millis(50);
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
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Udp(#[from] UdpTestError),
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
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpeedTestResult {
    pub action: SpeedActionType,
    pub index_id: String,
    pub delay: Option<i32>,
    pub speed: Option<f64>,
    pub message: Option<String>,
    pub ip_info: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpeedtestRunResult {
    pub action: SpeedActionType,
    pub cancelled: bool,
    pub selected_count: u32,
    pub completed_count: u32,
    pub results: Vec<SpeedTestResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpeedtestStatus {
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerTestItem {
    pub index_id: String,
    pub address: String,
    pub server_port: i32,
    pub socks_port: u16,
    pub config_type: ConfigType,
    pub queue_num: usize,
    pub profile: ProfileItem,
    pub core_type: CoreType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealPingProbeResult {
    pub delay: i32,
    pub ip_info: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedSpeedtestItem {
    item: ServerTestItem,
    entry: SpeedtestConfigEntry,
}

pub trait SpeedtestProbe: Send + Sync {
    fn tcping(
        &self,
        item: ServerTestItem,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>>;

    fn realping(
        &self,
        socks_port: u16,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>>;

    fn download_speed(
        &self,
        socks_port: u16,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<f64>>;

    fn udp_test(
        &self,
        socks_port: u16,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>>;
}

#[derive(Clone, Default)]
pub struct ReqwestSpeedtestProbe;

impl SpeedtestProbe for ReqwestSpeedtestProbe {
    fn tcping(
        &self,
        item: ServerTestItem,
        _config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            tcp_connect_delay(&item.address, item.server_port, TCPING_TIMEOUT, &cancel).await
        })
    }

    fn realping(
        &self,
        socks_port: u16,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let client = proxied_client(socks_port)?;
            let url = if config.speed_test_item.speed_ping_test_url.trim().is_empty() {
                REALPING_FALLBACK_URL
            } else {
                config.speed_test_item.speed_ping_test_url.as_str()
            };
            let timeout = Duration::from_secs(
                u64::try_from(config.speed_test_item.speed_test_timeout.max(1)).unwrap_or(1),
            );
            let mut best_delay = None;
            let mut last_error = None;
            for attempt in 0..2 {
                let started = Instant::now();
                match client.get(url).timeout(timeout).send().await {
                    Ok(response) => match response.error_for_status() {
                        Ok(_) => {
                            let delay = millis_i32(started.elapsed());
                            best_delay =
                                Some(best_delay.map_or(delay, |current: i32| current.min(delay)));
                        }
                        Err(error) => last_error = Some(error),
                    },
                    Err(error) => last_error = Some(error),
                }
                check_cancelled(&cancel)?;
                if attempt == 0 {
                    time::sleep(Duration::from_millis(100)).await;
                }
            }
            let delay = match best_delay {
                Some(delay) if delay > 0 => delay,
                Some(delay) => delay,
                None => {
                    if let Some(error) = last_error {
                        return Err(error.into());
                    }
                    return Err(SpeedtestError::Cancelled);
                }
            };

            let ip_info = if config.speed_test_item.ipapi_url.trim().is_empty() {
                None
            } else {
                match client
                    .get(config.speed_test_item.ipapi_url.as_str())
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                {
                    Ok(response) => match response.error_for_status() {
                        Ok(response) => response
                            .text()
                            .await
                            .ok()
                            .filter(|value| !value.trim().is_empty()),
                        Err(_) => None,
                    },
                    Err(_) => None,
                }
            };

            Ok(RealPingProbeResult { delay, ip_info })
        })
    }

    fn download_speed(
        &self,
        socks_port: u16,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<f64>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let client = proxied_client(socks_port)?;
            let timeout = Duration::from_secs(
                u64::try_from(config.speed_test_item.speed_test_timeout.max(1)).unwrap_or(1),
            );
            let started = Instant::now();
            let mut response = match time::timeout(
                timeout,
                client
                    .get(config.speed_test_item.speed_test_url.as_str())
                    .send(),
            )
            .await
            {
                Ok(response) => response?.error_for_status()?,
                Err(_) => {
                    return Err(SpeedtestError::Io(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "speedtest request timed out",
                    )));
                }
            };
            let mut total_bytes = 0_u64;
            let mut window_bytes = 0_u64;
            let mut window_started = Instant::now();
            let mut max_speed = 0.0_f64;
            let deadline = started + timeout;

            loop {
                check_cancelled(&cancel)?;
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let chunk = match time::timeout(remaining, response.chunk()).await {
                    Ok(Ok(Some(chunk))) => chunk,
                    Ok(Ok(None)) | Err(_) => break,
                    Ok(Err(error)) => return Err(error.into()),
                };
                let len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
                total_bytes = total_bytes.saturating_add(len);
                window_bytes = window_bytes.saturating_add(len);
                let window_elapsed = window_started.elapsed().as_secs_f64();
                if window_elapsed >= 1.0 {
                    max_speed = max_speed.max(window_bytes as f64 / window_elapsed);
                    window_started = Instant::now();
                    window_bytes = 0;
                }
            }
            check_cancelled(&cancel)?;
            let elapsed = started.elapsed().as_secs_f64().max(0.001);
            let average_speed = total_bytes as f64 / elapsed;

            Ok(max_speed.max(average_speed))
        })
    }

    fn udp_test(
        &self,
        socks_port: u16,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let (service, target) =
                UdpTestService::from_target(Some(&config.speed_test_item.udp_test_target));
            let elapsed = service
                .send_via_socks5(LOOPBACK_ADDR, socks_port, &target, Duration::from_secs(5))
                .await?;
            check_cancelled(&cancel)?;

            Ok(millis_i32(elapsed))
        })
    }
}

fn proxied_client(socks_port: u16) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!(
            "socks5h://{LOOPBACK_ADDR}:{socks_port}"
        ))?)
        .build()?)
}

pub trait SpeedtestCoreSession: Send {}

pub trait SpeedtestCoreBackend: Send + Sync {
    fn start(
        &self,
        core_type: CoreType,
        entries: Vec<SpeedtestConfigEntry>,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<Box<dyn SpeedtestCoreSession>>>;
}

#[derive(Clone)]
pub struct ProcessSpeedtestCoreBackend {
    paths: AppPaths,
    core_seed_resource_dir: Option<PathBuf>,
    runner: Arc<dyn ProcessRunner>,
}

impl ProcessSpeedtestCoreBackend {
    #[must_use]
    pub fn new(
        paths: AppPaths,
        core_seed_resource_dir: Option<PathBuf>,
        runner: Arc<dyn ProcessRunner>,
    ) -> Self {
        Self {
            paths,
            core_seed_resource_dir,
            runner,
        }
    }
}

impl SpeedtestCoreBackend for ProcessSpeedtestCoreBackend {
    fn start(
        &self,
        core_type: CoreType,
        entries: Vec<SpeedtestConfigEntry>,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<Box<dyn SpeedtestCoreSession>>> {
        let paths = self.paths.clone();
        let core_seed_resource_dir = self.core_seed_resource_dir.clone();
        let runner = Arc::clone(&self.runner);
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let config_file_name =
                format!("{SPEEDTEST_CONFIG_PREFIX}-{}.json", uuid::Uuid::new_v4());
            let config_path =
                write_speedtest_config(&paths, &config_file_name, core_type, &entries)?;
            let mut session = ProcessSpeedtestCoreSession {
                config_path,
                handle: None,
                runner: Arc::clone(&runner),
            };
            let core_info =
                get_core_info(core_type).ok_or(SpeedtestError::MissingCoreInfo(core_type))?;
            if let Some(seed_dir) = &core_seed_resource_dir {
                let _ = copy_seed_core_asset(&paths, seed_dir, core_type)?;
            }
            let executable = discover_executable(&paths, core_info)?;
            let launch = core_launch_plan(core_type, executable, &paths, &config_file_name)
                .ok_or(SpeedtestError::MissingCoreInfo(core_type))?;
            let spawn = ProcessSpawn::from_core_launch(ProcessRole::Probe, &launch, true)?;
            let handle = runner.spawn(spawn)?;
            session.handle = Some(handle);
            wait_for_speedtest_ports(&entries, &cancel).await?;

            Ok(Box::new(session) as Box<dyn SpeedtestCoreSession>)
        })
    }
}

struct ProcessSpeedtestCoreSession {
    config_path: PathBuf,
    handle: Option<ProcessHandle>,
    runner: Arc<dyn ProcessRunner>,
}

impl SpeedtestCoreSession for ProcessSpeedtestCoreSession {}

impl Drop for ProcessSpeedtestCoreSession {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            if let Err(error) = self.runner.stop(&handle) {
                tracing::warn!(?error, "failed to stop speedtest core process");
            }
        }
        if let Err(error) = fs::remove_file(&self.config_path) {
            if error.kind() != io::ErrorKind::NotFound {
                tracing::warn!(
                    path = %self.config_path.display(),
                    ?error,
                    "failed to remove speedtest config"
                );
            }
        }
    }
}

#[derive(Default)]
struct NoopSpeedtestCoreBackend;

impl SpeedtestCoreBackend for NoopSpeedtestCoreBackend {
    fn start(
        &self,
        _core_type: CoreType,
        _entries: Vec<SpeedtestConfigEntry>,
        _cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<Box<dyn SpeedtestCoreSession>>> {
        Box::pin(async { Ok(Box::new(NoopSpeedtestCoreSession) as Box<dyn SpeedtestCoreSession>) })
    }
}

struct NoopSpeedtestCoreSession;

impl SpeedtestCoreSession for NoopSpeedtestCoreSession {}

#[derive(Clone)]
pub struct SpeedtestManager {
    probe: Arc<dyn SpeedtestProbe>,
    core_backend: Arc<dyn SpeedtestCoreBackend>,
    paths: AppPaths,
    target_os: TargetOs,
    active_cancel: Arc<Mutex<Option<CancellationFlag>>>,
}

impl SpeedtestManager {
    #[must_use]
    pub fn new(
        paths: AppPaths,
        core_seed_resource_dir: Option<PathBuf>,
        runner: Arc<dyn ProcessRunner>,
    ) -> Self {
        cleanup_stale_speedtest_configs(&paths);
        Self::with_probe_and_backend(
            paths.clone(),
            Arc::new(ReqwestSpeedtestProbe),
            Arc::new(ProcessSpeedtestCoreBackend::new(
                paths,
                core_seed_resource_dir,
                runner,
            )),
        )
    }

    #[must_use]
    pub fn with_probe(probe: Arc<dyn SpeedtestProbe>) -> Self {
        let paths = AppPaths::new(
            std::env::temp_dir().join("voyavpn-speedtest-tests"),
            StorageMode::UserData,
        );
        Self::with_probe_and_backend(paths, probe, Arc::new(NoopSpeedtestCoreBackend))
    }

    #[must_use]
    pub fn with_probe_and_backend(
        paths: AppPaths,
        probe: Arc<dyn SpeedtestProbe>,
        core_backend: Arc<dyn SpeedtestCoreBackend>,
    ) -> Self {
        Self {
            probe,
            core_backend,
            paths,
            target_os: TargetOs::current(),
            active_cancel: Arc::new(Mutex::new(None)),
        }
    }

    #[must_use]
    pub fn with_target_os(mut self, target_os: TargetOs) -> Self {
        self.target_os = target_os;
        self
    }

    pub async fn run(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        index_ids: Vec<String>,
    ) -> Result<SpeedtestRunResult> {
        self.run_with_callback(database, config, action, index_ids, |_| {})
            .await
    }

    pub async fn run_with_callback<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        index_ids: Vec<String>,
        on_result: F,
    ) -> Result<SpeedtestRunResult>
    where
        F: Fn(SpeedTestResult) + Send + Sync,
    {
        let cancel = self.begin_job()?;
        let result = self
            .run_inner(
                database,
                config,
                action,
                index_ids,
                Arc::clone(&cancel),
                on_result,
            )
            .await;
        self.finish_job(&cancel)?;

        result
    }

    async fn run_inner<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        index_ids: Vec<String>,
        cancel: CancellationFlag,
        on_result: F,
    ) -> Result<SpeedtestRunResult>
    where
        F: Fn(SpeedTestResult) + Send + Sync,
    {
        let selected = select_test_items(database, config, action, &index_ids).await?;
        clear_previous_results(database, action, &selected, &on_result).await?;

        let mut results = Vec::new();
        let mut completed_count = 0_u32;

        match normalize_action(action) {
            SpeedActionType::Tcping => {
                for item in &selected {
                    if is_cancelled(&cancel) {
                        break;
                    }
                    let item_results = self
                        .run_item(
                            database,
                            config,
                            action,
                            item.clone(),
                            Arc::clone(&cancel),
                            &on_result,
                        )
                        .await?;
                    if !item_results.is_empty() {
                        completed_count = completed_count.saturating_add(1);
                    }
                    results.extend(item_results);
                }
            }
            SpeedActionType::Realping
            | SpeedActionType::UdpTest
            | SpeedActionType::FastRealping => {
                let item_results = self
                    .run_batch_items(
                        database,
                        config,
                        action,
                        selected.clone(),
                        Arc::clone(&cancel),
                        &on_result,
                    )
                    .await?;
                completed_count = completed_count.saturating_add(
                    u32::try_from(unique_result_count(&item_results)).unwrap_or(u32::MAX),
                );
                results.extend(item_results);
            }
            SpeedActionType::Speedtest | SpeedActionType::Mixedtest => {
                let item_results = self
                    .run_concurrent_dedicated_items(
                        database,
                        config,
                        action,
                        selected.clone(),
                        Arc::clone(&cancel),
                        &on_result,
                    )
                    .await?;
                completed_count = completed_count.saturating_add(
                    u32::try_from(unique_result_count(&item_results)).unwrap_or(u32::MAX),
                );
                results.extend(item_results);
            }
        }

        let cancelled = is_cancelled(&cancel);

        Ok(SpeedtestRunResult {
            action,
            cancelled,
            selected_count: u32::try_from(selected.len()).unwrap_or(u32::MAX),
            completed_count,
            results,
        })
    }

    pub fn cancel(&self) -> Result<bool> {
        let active = self
            .active_cancel
            .lock()
            .map_err(|_| SpeedtestError::JobLockPoisoned)?;
        if let Some(cancel) = active.as_ref() {
            cancel.store(true, Ordering::SeqCst);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn status(&self) -> Result<SpeedtestStatus> {
        Ok(SpeedtestStatus {
            running: self
                .active_cancel
                .lock()
                .map_err(|_| SpeedtestError::JobLockPoisoned)?
                .is_some(),
        })
    }

    async fn run_item<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        item: ServerTestItem,
        cancel: CancellationFlag,
        on_result: &F,
    ) -> Result<Vec<SpeedTestResult>>
    where
        F: Fn(SpeedTestResult) + Send + Sync,
    {
        let normalized = normalize_action(action);
        let mut results = Vec::new();
        match normalized {
            SpeedActionType::Tcping => {
                let result = self
                    .run_tcping(database, config, action, item, cancel)
                    .await?;
                on_result(result.clone());
                results.push(result);
            }
            SpeedActionType::Realping | SpeedActionType::FastRealping => {
                let result = self
                    .run_realping(database, config, action, item, cancel)
                    .await?;
                on_result(result.clone());
                results.push(result);
            }
            SpeedActionType::UdpTest => {
                let result = self.run_udp(database, config, action, item, cancel).await?;
                on_result(result.clone());
                results.push(result);
            }
            SpeedActionType::Speedtest => {
                let realping = self
                    .run_realping(database, config, action, item.clone(), Arc::clone(&cancel))
                    .await?;
                on_result(realping.clone());
                let can_continue = realping.delay.unwrap_or_default() > 0 && !is_cancelled(&cancel);
                results.push(realping);

                if can_continue {
                    let speed = self
                        .run_download(database, config, action, item, cancel)
                        .await?;
                    on_result(speed.clone());
                    results.push(speed);
                }
            }
            SpeedActionType::Mixedtest => {
                let realping = self
                    .run_realping(database, config, action, item.clone(), Arc::clone(&cancel))
                    .await?;
                on_result(realping.clone());
                let can_continue = realping.delay.unwrap_or_default() > 0 && !is_cancelled(&cancel);
                results.push(realping);

                if can_continue {
                    let speed = self
                        .run_download(database, config, action, item.clone(), Arc::clone(&cancel))
                        .await?;
                    on_result(speed.clone());
                    results.push(speed);
                }
            }
        }

        Ok(results)
    }

    async fn run_batch_items<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        items: Vec<ServerTestItem>,
        cancel: CancellationFlag,
        on_result: &F,
    ) -> Result<Vec<SpeedTestResult>>
    where
        F: Fn(SpeedTestResult) + Send + Sync,
    {
        let prepared = self
            .prepare_speedtest_items(database, config, items)
            .await?;
        let mut results = Vec::new();
        for (core_type, group) in group_prepared_items(prepared) {
            if is_cancelled(&cancel) {
                break;
            }
            let page_size = speedtest_page_size(config, group.len());
            let batch_count = group.chunks(page_size).len();
            for (batch_index, batch) in group.chunks(page_size).enumerate() {
                if is_cancelled(&cancel) {
                    break;
                }
                let entries = batch
                    .iter()
                    .map(|prepared| prepared.entry.clone())
                    .collect::<Vec<_>>();
                let _session = self
                    .core_backend
                    .start(core_type, entries, Arc::clone(&cancel))
                    .await?;
                for prepared in batch {
                    if is_cancelled(&cancel) {
                        break;
                    }
                    let item_results = self
                        .run_item(
                            database,
                            config,
                            action,
                            prepared.item.clone(),
                            Arc::clone(&cancel),
                            on_result,
                        )
                        .await?;
                    results.extend(item_results);
                }
                if batch_index + 1 < batch_count && !is_cancelled(&cancel) {
                    time::sleep(speedtest_delay_interval(config)).await;
                }
            }
        }

        Ok(results)
    }

    async fn run_dedicated_item<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        item: ServerTestItem,
        cancel: CancellationFlag,
        on_result: &F,
    ) -> Result<Vec<SpeedTestResult>>
    where
        F: Fn(SpeedTestResult) + Send + Sync,
    {
        let Some(prepared) = self
            .prepare_speedtest_items(database, config, vec![item])
            .await?
            .into_iter()
            .next()
        else {
            return Ok(Vec::new());
        };
        let core_type = prepared.entry.context.run_core_type;
        let _session = self
            .core_backend
            .start(core_type, vec![prepared.entry], Arc::clone(&cancel))
            .await?;
        self.run_item(database, config, action, prepared.item, cancel, on_result)
            .await
    }

    async fn run_concurrent_dedicated_items<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        items: Vec<ServerTestItem>,
        cancel: CancellationFlag,
        on_result: &F,
    ) -> Result<Vec<SpeedTestResult>>
    where
        F: Fn(SpeedTestResult) + Send + Sync,
    {
        let concurrency = dedicated_concurrency_count(action, config, items.len());
        let mut pending = items.into_iter();
        let mut in_flight = FuturesUnordered::new();
        let mut results = Vec::new();

        while in_flight.len() < concurrency {
            let Some(item) = pending.next() else {
                break;
            };
            if is_cancelled(&cancel) {
                break;
            }
            in_flight.push(self.run_dedicated_item(
                database,
                config,
                action,
                item,
                Arc::clone(&cancel),
                on_result,
            ));
        }

        while let Some(item_results) = in_flight.next().await {
            results.extend(item_results?);
            while in_flight.len() < concurrency {
                let Some(item) = pending.next() else {
                    break;
                };
                if is_cancelled(&cancel) {
                    break;
                }
                in_flight.push(self.run_dedicated_item(
                    database,
                    config,
                    action,
                    item,
                    Arc::clone(&cancel),
                    on_result,
                ));
            }
        }

        Ok(results)
    }

    async fn prepare_speedtest_items(
        &self,
        database: &Database,
        config: &AppConfig,
        items: Vec<ServerTestItem>,
    ) -> Result<Vec<PreparedSpeedtestItem>> {
        let env = load_runtime_core_gen_env(database, &self.paths, config, self.target_os).await?;
        let builder = CoreConfigContextBuilder::new(&env);
        let mut used_ports = HashSet::new();
        let mut prepared = Vec::new();

        for mut item in items {
            let socks_port = find_free_speedtest_port(i32::from(item.socks_port), &mut used_ports)?;
            item.socks_port = socks_port;
            let build = builder.build(config, &item.profile);
            if !build.success() {
                return Err(SpeedtestError::Validation {
                    index_id: item.index_id,
                    message: build.validator_result.errors.join("; "),
                });
            }
            item.core_type = build.context.run_core_type;
            prepared.push(PreparedSpeedtestItem {
                entry: SpeedtestConfigEntry {
                    index_id: item.index_id.clone(),
                    port: i32::from(socks_port),
                    context: build.context,
                },
                item,
            });
        }

        Ok(prepared)
    }

    async fn run_tcping(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        item: ServerTestItem,
        cancel: CancellationFlag,
    ) -> Result<SpeedTestResult> {
        let index_id = item.index_id.clone();
        let delay = self
            .probe
            .tcping(item, config.clone(), cancel)
            .await
            .unwrap_or(-1);
        let result = SpeedTestResult {
            action,
            index_id,
            delay: Some(delay),
            speed: None,
            message: Some(delay.to_string()),
            ip_info: None,
        };
        persist_speedtest_result(database, &result).await?;

        Ok(result)
    }

    async fn run_realping(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        item: ServerTestItem,
        cancel: CancellationFlag,
    ) -> Result<SpeedTestResult> {
        let index_id = item.index_id.clone();
        let result = match self
            .probe
            .realping(item.socks_port, config.clone(), cancel)
            .await
        {
            Ok(realping) => SpeedTestResult {
                action,
                index_id,
                delay: Some(realping.delay),
                speed: None,
                message: Some(realping.delay.to_string()),
                ip_info: realping.ip_info,
            },
            Err(error) => {
                tracing::warn!(index_id = %index_id, ?error, "speedtest realping failed");
                SpeedTestResult {
                    action,
                    index_id,
                    delay: Some(-1),
                    speed: None,
                    message: Some(speedtest_error_message(&error)),
                    ip_info: Some("Skipped".to_string()),
                }
            }
        };
        persist_speedtest_result(database, &result).await?;

        Ok(result)
    }

    async fn run_download(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        item: ServerTestItem,
        cancel: CancellationFlag,
    ) -> Result<SpeedTestResult> {
        let index_id = item.index_id.clone();
        let result = match self
            .probe
            .download_speed(item.socks_port, config.clone(), cancel)
            .await
        {
            Ok(speed) => SpeedTestResult {
                action,
                index_id,
                delay: None,
                speed: Some(speed),
                message: Some(format!("{speed:.0}")),
                ip_info: None,
            },
            Err(error) => {
                tracing::warn!(index_id = %index_id, ?error, "speedtest download failed");
                SpeedTestResult {
                    action,
                    index_id,
                    delay: None,
                    speed: Some(0.0),
                    message: Some(speedtest_error_message(&error)),
                    ip_info: None,
                }
            }
        };
        persist_speedtest_result(database, &result).await?;

        Ok(result)
    }

    async fn run_udp(
        &self,
        database: &Database,
        config: &AppConfig,
        action: SpeedActionType,
        item: ServerTestItem,
        cancel: CancellationFlag,
    ) -> Result<SpeedTestResult> {
        let index_id = item.index_id.clone();
        let delay = self
            .probe
            .udp_test(item.socks_port, config.clone(), cancel)
            .await
            .unwrap_or(-1);
        let result = SpeedTestResult {
            action,
            index_id,
            delay: Some(delay),
            speed: None,
            message: Some(delay.to_string()),
            ip_info: None,
        };
        persist_speedtest_result(database, &result).await?;

        Ok(result)
    }

    fn begin_job(&self) -> Result<CancellationFlag> {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut active = self
            .active_cancel
            .lock()
            .map_err(|_| SpeedtestError::JobLockPoisoned)?;
        if let Some(previous) = active.replace(Arc::clone(&cancel)) {
            previous.store(true, Ordering::SeqCst);
        }

        Ok(cancel)
    }

    fn finish_job(&self, cancel: &CancellationFlag) -> Result<()> {
        let mut active = self
            .active_cancel
            .lock()
            .map_err(|_| SpeedtestError::JobLockPoisoned)?;
        if active
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, cancel))
        {
            *active = None;
        }

        Ok(())
    }
}

async fn select_test_items(
    database: &Database,
    config: &AppConfig,
    action: SpeedActionType,
    index_ids: &[String],
) -> Result<Vec<ServerTestItem>> {
    let profiles = if index_ids.is_empty() || matches!(action, SpeedActionType::FastRealping) {
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
        + InboundProtocol::speedtest.as_i32();

    profiles
        .into_iter()
        .enumerate()
        .filter(|(_, profile)| {
            profile.config_type != ConfigType::Custom
                && (profile.config_type.is_complex_type() || profile.port > 0)
        })
        .map(|(queue_num, profile)| {
            let socks_port_i32 =
                base_port.saturating_add(i32::try_from(queue_num).unwrap_or(i32::MAX));
            let socks_port = u16::try_from(socks_port_i32)
                .map_err(|_| SpeedtestError::InvalidSocksPort(socks_port_i32))?;
            Ok(ServerTestItem {
                index_id: profile.index_id.clone(),
                address: profile.address.clone(),
                server_port: profile.port,
                socks_port,
                config_type: profile.config_type,
                queue_num,
                core_type: profile
                    .core_type
                    .or_else(|| configured_core_type(config, profile.config_type))
                    .unwrap_or_else(|| default_core_type(profile.config_type)),
                profile,
            })
        })
        .collect()
}

const fn default_core_type(config_type: ConfigType) -> CoreType {
    match config_type {
        ConfigType::TUIC | ConfigType::Anytls | ConfigType::Naive => CoreType::sing_box,
        _ => CoreType::Xray,
    }
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

fn unique_result_count(results: &[SpeedTestResult]) -> usize {
    results
        .iter()
        .map(|result| result.index_id.as_str())
        .collect::<HashSet<_>>()
        .len()
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

fn speedtest_delay_interval(config: &AppConfig) -> Duration {
    config
        .speed_test_item
        .speed_test_delay_interval
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0)
        .map(Duration::from_secs)
        .unwrap_or(SPEEDTEST_DELAY_INTERVAL)
}

fn mixed_concurrency_count(config: &AppConfig, selected_count: usize) -> usize {
    let configured = usize::try_from(config.speed_test_item.mixed_concurrency_count)
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(1);
    configured.min(selected_count.max(1))
}

fn dedicated_concurrency_count(
    action: SpeedActionType,
    config: &AppConfig,
    selected_count: usize,
) -> usize {
    if normalize_action(action) == SpeedActionType::Mixedtest {
        mixed_concurrency_count(config, selected_count)
    } else {
        1
    }
}

fn find_free_speedtest_port(start: i32, used_ports: &mut HashSet<u16>) -> Result<u16> {
    let mut port = u16::try_from(start).map_err(|_| SpeedtestError::InvalidSocksPort(start))?;
    loop {
        if !used_ports.contains(&port) && local_port_available(port) {
            used_ports.insert(port);
            return Ok(port);
        }
        if port == u16::MAX {
            return Err(SpeedtestError::NoAvailablePort(start));
        }
        port = port.saturating_add(1);
    }
}

fn local_port_available(port: u16) -> bool {
    TcpListener::bind((LOOPBACK_ADDR, port)).is_ok()
}

fn write_speedtest_config(
    paths: &AppPaths,
    file_name: &str,
    core_type: CoreType,
    entries: &[SpeedtestConfigEntry],
) -> Result<PathBuf> {
    let json = if core_type == CoreType::sing_box {
        generate_singbox_speedtest_config_json(entries)?
    } else {
        generate_xray_speedtest_config_json(entries)
    };
    let path = paths.bin_config_file(file_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SpeedtestError::CreateConfigDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&path, json).map_err(|source| SpeedtestError::WriteConfig {
        path: path.clone(),
        source,
    })?;

    Ok(path)
}

fn cleanup_stale_speedtest_configs(paths: &AppPaths) {
    let Ok(entries) = fs::read_dir(paths.bin_config_dir()) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.starts_with(SPEEDTEST_CONFIG_PREFIX) || !file_name.ends_with(".json") {
            continue;
        }
        if let Err(error) = fs::remove_file(&path) {
            if error.kind() != io::ErrorKind::NotFound {
                tracing::warn!(
                    path = %path.display(),
                    ?error,
                    "failed to remove stale speedtest config"
                );
            }
        }
    }
}

async fn wait_for_speedtest_ports(
    entries: &[SpeedtestConfigEntry],
    cancel: &CancellationFlag,
) -> Result<()> {
    let started = Instant::now();
    loop {
        check_cancelled(cancel)?;
        let mut all_ready = true;
        for entry in entries {
            let port = u16::try_from(entry.port)
                .map_err(|_| SpeedtestError::InvalidSocksPort(entry.port))?;
            if TcpStream::connect((LOOPBACK_ADDR, port)).await.is_err() {
                all_ready = false;
                break;
            }
        }
        if all_ready {
            return Ok(());
        }
        if started.elapsed() >= SPEEDTEST_READY_TIMEOUT {
            return Err(SpeedtestError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                "temporary speedtest core did not expose a local SOCKS port",
            )));
        }
        time::sleep(SPEEDTEST_READY_INTERVAL).await;
    }
}

fn speedtest_error_message(error: &SpeedtestError) -> String {
    match error {
        SpeedtestError::Cancelled => "cancelled".to_string(),
        SpeedtestError::Io(source) if source.kind() == io::ErrorKind::TimedOut => {
            "request timed out".to_string()
        }
        SpeedtestError::Http(source) if source.is_timeout() => "request timed out".to_string(),
        SpeedtestError::Http(source) if source.is_connect() => {
            "proxy connection failed".to_string()
        }
        SpeedtestError::Udp(_) => "UDP test failed".to_string(),
        _ => {
            let raw = error.to_string();
            let lower = raw.to_ascii_lowercase();
            if lower.contains("timed out") || lower.contains("timeout") {
                "request timed out".to_string()
            } else if lower.contains("connection refused") {
                "proxy connection refused".to_string()
            } else if lower.contains("connection reset") || lower.contains("connection closed") {
                "proxy connection closed".to_string()
            } else {
                raw
            }
        }
    }
}

async fn clear_previous_results<F>(
    database: &Database,
    action: SpeedActionType,
    selected: &[ServerTestItem],
    on_result: &F,
) -> Result<()>
where
    F: Fn(SpeedTestResult) + Send + Sync,
{
    for item in selected {
        let profile_ex = ProfileExManager::new(database);
        match normalize_action(action) {
            SpeedActionType::Tcping
            | SpeedActionType::Realping
            | SpeedActionType::UdpTest
            | SpeedActionType::FastRealping => {
                profile_ex.set_test_delay(&item.index_id, 0).await?;
                profile_ex
                    .set_test_message(&item.index_id, "Speedtesting")
                    .await?;
                on_result(SpeedTestResult {
                    action,
                    index_id: item.index_id.clone(),
                    delay: Some(0),
                    speed: None,
                    message: Some("Speedtesting".to_string()),
                    ip_info: None,
                });
            }
            SpeedActionType::Speedtest => {
                profile_ex.set_test_speed(&item.index_id, 0.0).await?;
                profile_ex
                    .set_test_message(&item.index_id, "Speedtesting wait")
                    .await?;
                on_result(SpeedTestResult {
                    action,
                    index_id: item.index_id.clone(),
                    delay: None,
                    speed: Some(0.0),
                    message: Some("Speedtesting wait".to_string()),
                    ip_info: None,
                });
            }
            SpeedActionType::Mixedtest => {
                profile_ex.set_test_delay(&item.index_id, 0).await?;
                profile_ex.set_test_speed(&item.index_id, 0.0).await?;
                profile_ex
                    .set_test_message(&item.index_id, "Speedtesting wait")
                    .await?;
                on_result(SpeedTestResult {
                    action,
                    index_id: item.index_id.clone(),
                    delay: Some(0),
                    speed: Some(0.0),
                    message: Some("Speedtesting wait".to_string()),
                    ip_info: None,
                });
            }
        }
    }

    Ok(())
}

async fn persist_speedtest_result(database: &Database, result: &SpeedTestResult) -> Result<()> {
    let profile_ex = ProfileExManager::new(database);
    if let Some(delay) = result.delay {
        profile_ex.set_test_delay(&result.index_id, delay).await?;
    }
    if let Some(speed) = result.speed {
        profile_ex.set_test_speed(&result.index_id, speed).await?;
    }
    if let Some(message) = result.message.as_ref() {
        profile_ex
            .set_test_message(&result.index_id, message.clone())
            .await?;
    }
    if let Some(ip_info) = result.ip_info.as_ref() {
        profile_ex
            .set_test_ip_info(&result.index_id, ip_info.clone())
            .await?;
    }

    Ok(())
}

fn configured_core_type(config: &AppConfig, config_type: ConfigType) -> Option<CoreType> {
    config
        .core_type_item
        .iter()
        .find(|item| item.config_type == config_type)
        .map(|item| item.core_type)
}

fn normalize_action(action: SpeedActionType) -> SpeedActionType {
    match action {
        SpeedActionType::FastRealping => SpeedActionType::Realping,
        other => other,
    }
}

async fn tcp_connect_delay(
    host: &str,
    port: i32,
    timeout: Duration,
    cancel: &CancellationFlag,
) -> Result<i32> {
    check_cancelled(cancel)?;
    let port = u16::try_from(port).map_err(|_| SpeedtestError::InvalidSocksPort(port))?;
    let mut addresses = lookup_host((host, port)).await?;
    let Some(address) = addresses.next() else {
        return Ok(-1);
    };

    connect_address_delay(address, timeout, cancel).await
}

async fn connect_address_delay(
    address: SocketAddr,
    timeout: Duration,
    cancel: &CancellationFlag,
) -> Result<i32> {
    let started = Instant::now();
    let result = time::timeout(timeout, TcpStream::connect(address)).await;
    check_cancelled(cancel)?;

    match result {
        Ok(Ok(_stream)) => Ok(millis_i32(started.elapsed())),
        Ok(Err(error)) => Err(error.into()),
        Err(_) => Ok(-1),
    }
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

fn millis_i32(duration: Duration) -> i32 {
    i32::try_from(duration.as_millis()).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{atomic::AtomicUsize, Mutex as StdMutex},
    };

    use voya_core::{ProfileExItem, ProtocolExtraItem};
    use voya_db::Database;

    use super::*;

    #[derive(Default)]
    struct RecordingProbe {
        calls: Arc<StdMutex<Vec<String>>>,
        block_realping: bool,
        download_delay: Duration,
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
        fn tcping(
            &self,
            item: ServerTestItem,
            _config: AppConfig,
            _cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<i32>> {
            let calls = Arc::clone(&self.calls);
            Box::pin(async move {
                calls
                    .lock()
                    .expect("speedtest test operation should succeed")
                    .push(format!("tcping:{}", item.index_id));
                Ok(11)
            })
        }

        fn realping(
            &self,
            socks_port: u16,
            _config: AppConfig,
            cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<RealPingProbeResult>> {
            let calls = Arc::clone(&self.calls);
            let block = self.block_realping;
            Box::pin(async move {
                calls
                    .lock()
                    .expect("speedtest test operation should succeed")
                    .push(format!("realping:{socks_port}"));
                if block {
                    while !is_cancelled(&cancel) {
                        time::sleep(Duration::from_millis(10)).await;
                    }
                    return Err(SpeedtestError::Cancelled);
                }
                Ok(RealPingProbeResult {
                    delay: 44,
                    ip_info: Some("US".to_string()),
                })
            })
        }

        fn download_speed(
            &self,
            socks_port: u16,
            _config: AppConfig,
            _cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<f64>> {
            let calls = Arc::clone(&self.calls);
            let delay = self.download_delay;
            Box::pin(async move {
                calls
                    .lock()
                    .expect("speedtest test operation should succeed")
                    .push(format!("speedtest:{socks_port}"));
                if !delay.is_zero() {
                    time::sleep(delay).await;
                }
                Ok(2048.0)
            })
        }

        fn udp_test(
            &self,
            socks_port: u16,
            _config: AppConfig,
            _cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<i32>> {
            let calls = Arc::clone(&self.calls);
            Box::pin(async move {
                calls
                    .lock()
                    .expect("speedtest test operation should succeed")
                    .push(format!("udp:{socks_port}"));
                Ok(55)
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
        max_active: Arc<AtomicUsize>,
    }

    impl RecordingCoreBackend {
        fn starts(&self) -> Vec<RecordedCoreStart> {
            self.starts
                .lock()
                .expect("speedtest test operation should succeed")
                .clone()
        }

        fn max_active(&self) -> usize {
            self.max_active.load(Ordering::SeqCst)
        }
    }

    impl SpeedtestCoreBackend for RecordingCoreBackend {
        fn start(
            &self,
            core_type: CoreType,
            entries: Vec<SpeedtestConfigEntry>,
            _cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<Box<dyn SpeedtestCoreSession>>> {
            let starts = Arc::clone(&self.starts);
            let active = Arc::clone(&self.active);
            let max_active = Arc::clone(&self.max_active);
            Box::pin(async move {
                starts
                    .lock()
                    .expect("speedtest test operation should succeed")
                    .push(RecordedCoreStart {
                        core_type,
                        ports: entries.iter().map(|entry| entry.port).collect(),
                    });
                let active_now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(active_now, Ordering::SeqCst);
                Ok(Box::new(RecordingCoreSession { active }) as Box<dyn SpeedtestCoreSession>)
            })
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

    #[test]
    fn speedtest_action_type_covers_six_v2rayn_values() {
        assert_eq!(SpeedActionType::Tcping.as_i32(), 0);
        assert_eq!(SpeedActionType::Realping.as_i32(), 1);
        assert_eq!(SpeedActionType::UdpTest.as_i32(), 2);
        assert_eq!(SpeedActionType::Speedtest.as_i32(), 3);
        assert_eq!(SpeedActionType::Mixedtest.as_i32(), 4);
        assert_eq!(SpeedActionType::FastRealping.as_i32(), 5);
    }

    #[tokio::test]
    async fn speedtest_manager_mixedtest_combines_realping_and_speedtest() {
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
            .run(
                &database,
                &config,
                SpeedActionType::Mixedtest,
                vec!["a".to_string()],
            )
            .await
            .expect("speedtest test operation should succeed");

        assert!(!run.cancelled);
        assert_eq!(run.completed_count, 1);
        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 1);
        let port = starts[0].ports[0];
        assert_eq!(
            probe.calls(),
            vec![format!("realping:{port}"), format!("speedtest:{port}")]
        );
        let profile_ex = database
            .profile_exs()
            .get("a")
            .await
            .expect("speedtest test operation should succeed")
            .expect("speedtest test operation should succeed");
        assert_eq!(profile_ex.delay, 44);
        assert_eq!(profile_ex.speed, 2048.0);
        assert_eq!(profile_ex.ip_info.as_deref(), Some("US"));
    }

    #[tokio::test]
    async fn speedtest_manager_fast_realping_uses_all_profiles_as_realping() {
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
            .run(
                &database,
                &AppConfig::default(),
                SpeedActionType::FastRealping,
                Vec::new(),
            )
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
    async fn speedtest_manager_speedtest_realpings_before_download() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        let probe = Arc::new(RecordingProbe::default());
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());

        manager
            .run(
                &database,
                &AppConfig::default(),
                SpeedActionType::Speedtest,
                vec!["a".to_string()],
            )
            .await
            .expect("speedtest test operation should succeed");

        let starts = backend.starts();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].ports.len(), 1);
        let port = starts[0].ports[0];
        assert_eq!(
            probe.calls(),
            vec![format!("realping:{port}"), format!("speedtest:{port}")]
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
            .run(
                &database,
                &AppConfig::default(),
                SpeedActionType::Realping,
                Vec::new(),
            )
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
    async fn speedtest_manager_udp_batches_one_temp_core() {
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
            .run(
                &database,
                &AppConfig::default(),
                SpeedActionType::UdpTest,
                Vec::new(),
            )
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
                .map(|port| format!("udp:{port}"))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn speedtest_manager_speedtest_runs_dedicated_cores_serially() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        insert_profile(&database, "c", 9443).await;
        let probe = Arc::new(RecordingProbe {
            calls: Arc::new(StdMutex::new(Vec::new())),
            download_delay: Duration::from_millis(50),
            ..RecordingProbe::default()
        });
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe, backend.clone());
        let mut config = AppConfig::default();
        config.speed_test_item.mixed_concurrency_count = 2;

        manager
            .run(&database, &config, SpeedActionType::Speedtest, Vec::new())
            .await
            .expect("speedtest test operation should succeed");

        let starts = backend.starts();
        assert_eq!(starts.len(), 3);
        assert!(starts.iter().all(|start| start.ports.len() == 1));
        assert_eq!(backend.max_active(), 1);
    }

    #[tokio::test]
    async fn speedtest_manager_mixedtest_uses_configured_dedicated_core_concurrency() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        insert_profile(&database, "c", 9443).await;
        let probe = Arc::new(RecordingProbe {
            calls: Arc::new(StdMutex::new(Vec::new())),
            download_delay: Duration::from_millis(50),
            ..RecordingProbe::default()
        });
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe, backend.clone());
        let mut config = AppConfig::default();
        config.speed_test_item.mixed_concurrency_count = 2;

        manager
            .run(&database, &config, SpeedActionType::Mixedtest, Vec::new())
            .await
            .expect("speedtest test operation should succeed");

        let starts = backend.starts();
        assert_eq!(starts.len(), 3);
        assert!(starts.iter().all(|start| start.ports.len() == 1));
        assert_eq!(backend.max_active(), 2);
    }

    #[test]
    fn cleanup_stale_speedtest_configs_removes_only_speedtest_json_files() {
        let paths = test_paths();
        fs::create_dir_all(paths.bin_config_dir())
            .expect("speedtest test operation should succeed");
        let stale = paths.bin_config_file("configTest-old.json");
        let current_style_stale = paths.bin_config_file("configTest-123.json");
        let runtime_config = paths.bin_config_file("config.json");
        let similar_name = paths.bin_config_file("configTest-not-json.txt");
        fs::write(&stale, "{}").expect("speedtest test operation should succeed");
        fs::write(&current_style_stale, "{}").expect("speedtest test operation should succeed");
        fs::write(&runtime_config, "{}").expect("speedtest test operation should succeed");
        fs::write(&similar_name, "{}").expect("speedtest test operation should succeed");

        cleanup_stale_speedtest_configs(&paths);

        assert!(!stale.exists());
        assert!(!current_style_stale.exists());
        assert!(runtime_config.exists());
        assert!(similar_name.exists());
    }

    #[tokio::test]
    async fn speedtest_manager_cancel_stops_active_jobs() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe {
            calls: Arc::new(StdMutex::new(Vec::new())),
            block_realping: true,
            ..RecordingProbe::default()
        });
        let backend = Arc::new(RecordingCoreBackend::default());
        let manager =
            SpeedtestManager::with_probe_and_backend(test_paths(), probe.clone(), backend.clone());
        let task_manager = manager.clone();
        let mut config = AppConfig::default();
        config.speed_test_item.mixed_concurrency_count = 1;

        let handle = tokio::spawn(async move {
            task_manager
                .run(&database, &config, SpeedActionType::Mixedtest, Vec::new())
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
        assert_eq!(starts[0].ports.len(), 1);
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

    async fn insert_profile(database: &Database, index_id: &str, port: i32) {
        let profile = ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VMess,
            core_type: Some(CoreType::Xray),
            remarks: index_id.to_string(),
            address: "127.0.0.1".to_string(),
            port,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            protocol_extra: ProtocolExtraItem {
                vmess_security: Some("auto".to_string()),
                ..ProtocolExtraItem::default()
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
            StorageMode::UserData,
        )
    }
}
