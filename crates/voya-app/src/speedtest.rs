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

use futures_util::{
    future::BoxFuture,
    stream::{FuturesUnordered, StreamExt},
};
use thiserror::Error;
use tokio::time;
use voya_contracts::SpeedTestKind;
pub use voya_contracts::{SpeedTestResult, SpeedtestRunResult, SpeedtestStatus};
use voya_core::{
    generate_singbox_speedtest_config_json, AppConfig, ConfigType, CoreConfigContextBuilder,
    CoreType, InboundProtocol, ProfileItem, SpeedTestItem, SpeedtestConfigEntry,
    DEFAULT_LOCAL_PORT,
};
use voya_db::{Database, DbError};
use voya_net::probe::{tcp_connect_delay, tcp_port_is_open, NetworkProbeError, SocksHttpProbe};
use voya_platform::{
    coreinfo::{
        copy_seed_core_asset, discover_executable, discover_packaged_seed_executable,
        get_core_info, CoreInfo, CoreInfoError, TargetOs,
    },
    filesystem,
    paths::{AppPaths, PathError},
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
    Network(#[from] NetworkProbeError),
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
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>>;

    fn realping(
        &self,
        socks_port: u16,
        speed_test_item: SpeedTestItem,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>>;

    fn download_speed(
        &self,
        socks_port: u16,
        speed_test_item: SpeedTestItem,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<f64>>;

    fn udp_test(
        &self,
        socks_port: u16,
        speed_test_item: SpeedTestItem,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>>;
}

#[derive(Clone, Default)]
pub struct ReqwestSpeedtestProbe;

impl SpeedtestProbe for ReqwestSpeedtestProbe {
    fn tcping(
        &self,
        item: ServerTestItem,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            tcp_connect_delay(&item.address, item.server_port, TCPING_TIMEOUT, &cancel)
                .await
                .map_err(Into::into)
        })
    }

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

            let ip_info = if speed_test_item.ipapi_url.trim().is_empty() {
                None
            } else {
                client
                    .optional_text(speed_test_item.ipapi_url.as_str(), Duration::from_secs(5))
                    .await
            };

            Ok(RealPingProbeResult { delay, ip_info })
        })
    }

    fn download_speed(
        &self,
        socks_port: u16,
        speed_test_item: SpeedTestItem,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<f64>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let client = SocksHttpProbe::new(socks_port)?;
            let timeout = Duration::from_secs(
                u64::try_from(speed_test_item.speed_test_timeout.max(1)).unwrap_or(1),
            );
            client
                .download_speed(speed_test_item.speed_test_url.as_str(), timeout, &cancel)
                .await
                .map_err(Into::into)
        })
    }

    fn udp_test(
        &self,
        socks_port: u16,
        speed_test_item: SpeedTestItem,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let (service, target) =
                UdpTestService::from_target(Some(&speed_test_item.udp_test_target));
            let elapsed = service
                .send_via_socks5(LOOPBACK_ADDR, socks_port, &target, Duration::from_secs(5))
                .await?;
            check_cancelled(&cancel)?;

            Ok(millis_i32(elapsed))
        })
    }
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
    target_os: TargetOs,
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
            target_os: TargetOs::current(),
        }
    }

    #[must_use]
    pub fn with_target_os(mut self, target_os: TargetOs) -> Self {
        self.target_os = target_os;
        self
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
        let target_os = self.target_os;
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
            let executable = match packaged_seed_executable(
                core_seed_resource_dir.as_ref(),
                core_info,
                target_os,
            )? {
                Some(executable) => executable,
                None => {
                    if let Some(seed_dir) = &core_seed_resource_dir {
                        let _ = copy_seed_core_asset(&paths, seed_dir, core_type)?;
                    }
                    discover_executable(&paths, core_info)?
                }
            };
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

fn packaged_seed_executable(
    core_seed_resource_dir: Option<&PathBuf>,
    core_info: &CoreInfo,
    target_os: TargetOs,
) -> Result<Option<PathBuf>> {
    if target_os != TargetOs::Macos {
        return Ok(None);
    }

    let Some(seed_dir) = core_seed_resource_dir else {
        return Ok(None);
    };

    discover_packaged_seed_executable(seed_dir, core_info, target_os).map_err(Into::into)
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
        if let Err(error) = filesystem::remove_file_if_exists(&self.config_path) {
            tracing::warn!(
                path = %self.config_path.display(),
                ?error,
                "failed to remove speedtest config"
            );
        }
    }
}

#[derive(Clone)]
pub struct SpeedtestManager {
    probe: Arc<dyn SpeedtestProbe>,
    core_backend: Arc<dyn SpeedtestCoreBackend>,
    paths: AppPaths,
    target_os: TargetOs,
    active_cancel: Arc<Mutex<Option<CancellationFlag>>>,
}

mod manager;
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
        .filter(|(_, profile)| {
            profile.config_type() != ConfigType::Custom
                && (profile.config_type().is_complex_type() || profile.port() > 0)
        })
        .map(|(queue_num, profile)| {
            let socks_port_i32 =
                base_port.saturating_add(i32::try_from(queue_num).unwrap_or(i32::MAX));
            let socks_port = u16::try_from(socks_port_i32)
                .map_err(|_| SpeedtestError::InvalidSocksPort(socks_port_i32))?;
            Ok(ServerTestItem {
                index_id: profile.index_id.clone(),
                address: profile.address().to_string(),
                server_port: profile.port(),
                socks_port,
                config_type: profile.config_type(),
                queue_num,
                core_type: CoreType::sing_box,
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

fn unique_result_count(results: &[SpeedTestResult]) -> usize {
    let mut seen = HashSet::new();
    results
        .iter()
        .filter(|result| seen.insert(result.index_id.as_str()))
        .count()
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
    action: SpeedTestKind,
    config: &AppConfig,
    selected_count: usize,
) -> usize {
    if action == SpeedTestKind::Mixed {
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
    let _ = core_type;
    let json = generate_singbox_speedtest_config_json(entries)?;
    let path = paths.bin_config_file(file_name);
    // Speedtest configs carry the same outbound credentials as the runtime
    // config, so they get the same 0600 + O_NOFOLLOW treatment.
    filesystem::write_private_file_with_parent(&path, json).map_err(|source| {
        SpeedtestError::WriteConfig {
            path: path.clone(),
            source,
        }
    })?;

    Ok(path)
}

fn cleanup_stale_speedtest_configs(paths: &AppPaths) {
    if let Err(error) =
        filesystem::remove_matching_files(paths.bin_config_dir(), SPEEDTEST_CONFIG_PREFIX, ".json")
    {
        tracing::warn!(
            path = %paths.bin_config_dir().display(),
            ?error,
            "failed to remove stale speedtest configs"
        );
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
            if !tcp_port_is_open(LOOPBACK_ADDR, port).await {
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
        SpeedtestError::Network(NetworkProbeError::Http(source)) if source.is_timeout() => {
            "request timed out".to_string()
        }
        SpeedtestError::Network(NetworkProbeError::Http(source)) if source.is_connect() => {
            "proxy connection failed".to_string()
        }
        SpeedtestError::Network(NetworkProbeError::Timeout) => "request timed out".to_string(),
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
    action: SpeedTestKind,
    selected: &[ServerTestItem],
    on_result: &F,
) -> Result<()>
where
    F: Fn(SpeedTestResult) + Send + Sync,
{
    let profile_ex = ProfileExManager::new(database);
    for item in selected {
        match action {
            SpeedTestKind::TcpConnect | SpeedTestKind::Latency | SpeedTestKind::Udp => {
                profile_ex.set_test_delay(&item.index_id, 0).await?;
                profile_ex
                    .set_test_message(&item.index_id, "Speedtesting")
                    .await?;
            }
            SpeedTestKind::Download => {
                profile_ex.set_test_speed(&item.index_id, 0.0).await?;
                profile_ex
                    .set_test_message(&item.index_id, "Speedtesting wait")
                    .await?;
            }
            SpeedTestKind::Mixed => {
                profile_ex.set_test_delay(&item.index_id, 0).await?;
                profile_ex.set_test_speed(&item.index_id, 0.0).await?;
                profile_ex
                    .set_test_message(&item.index_id, "Speedtesting wait")
                    .await?;
            }
        }
        on_result(make_pending_result(action, item.index_id.clone()));
    }

    Ok(())
}

fn make_pending_result(action: SpeedTestKind, index_id: String) -> SpeedTestResult {
    match action {
        SpeedTestKind::TcpConnect | SpeedTestKind::Latency | SpeedTestKind::Udp => {
            SpeedTestResult {
                action,
                index_id,
                delay: Some(0),
                speed: None,
                message: Some("Speedtesting".to_string()),
                ip_info: None,
            }
        }
        SpeedTestKind::Download => SpeedTestResult {
            action,
            index_id,
            delay: None,
            speed: Some(0.0),
            message: Some("Speedtesting wait".to_string()),
            ip_info: None,
        },
        SpeedTestKind::Mixed => SpeedTestResult {
            action,
            index_id,
            delay: Some(0),
            speed: Some(0.0),
            message: Some("Speedtesting wait".to_string()),
            ip_info: None,
        },
    }
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
        collections::HashSet as StdHashSet,
        fs,
        net::TcpListener as StdTcpListener,
        sync::{atomic::AtomicUsize, Mutex as StdMutex},
    };

    use voya_core::{ProfileExItem, ProfileProtocol, ServerEndpoint};
    use voya_db::Database;
    use voya_platform::{
        coreinfo::{core_type_dir_name, executable_name_for_current_os},
        paths::core_seed_resources_dir,
        test_support::RecordingRunner,
    };

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
            _speed_test_item: SpeedTestItem,
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
            _speed_test_item: SpeedTestItem,
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
            _speed_test_item: SpeedTestItem,
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

    #[tokio::test]
    async fn process_speedtest_core_backend_uses_packaged_seed_directly_on_macos() {
        let paths = test_paths();
        let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
        let executable_name = executable_name_for_current_os("sing-box");
        let seed_exe = seed_root
            .join(core_type_dir_name(CoreType::sing_box))
            .join(&executable_name);
        fs::create_dir_all(seed_exe.parent().expect("seed core dir"))
            .expect("speedtest test operation should succeed");
        fs::write(&seed_exe, b"seed-sing-box").expect("speedtest test operation should succeed");
        let runner = RecordingRunner::default();
        let backend = ProcessSpeedtestCoreBackend::new(
            paths.clone(),
            Some(seed_root),
            Arc::new(runner.clone()),
        )
        .with_target_os(TargetOs::Macos);

        backend
            .start(
                CoreType::sing_box,
                Vec::new(),
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .expect("speedtest test operation should succeed");

        let app_data_exe =
            paths.core_bin_file(core_type_dir_name(CoreType::sing_box), executable_name);
        let spawns = runner.spawns();
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].executable, seed_exe);
        assert!(!app_data_exe.exists());
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
                SpeedTestKind::Mixed,
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
            .run(
                &database,
                &AppConfig::default(),
                SpeedTestKind::Latency,
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
                SpeedTestKind::Download,
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
                SpeedTestKind::Latency,
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
                SpeedTestKind::Udp,
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
            .run(&database, &config, SpeedTestKind::Download, Vec::new())
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
            .run(&database, &config, SpeedTestKind::Mixed, Vec::new())
            .await
            .expect("speedtest test operation should succeed");

        let starts = backend.starts();
        assert_eq!(starts.len(), 3);
        assert!(starts.iter().all(|start| start.ports.len() == 1));
        assert_eq!(backend.max_active(), 2);
    }

    #[tokio::test]
    async fn speedtest_manager_dedicated_prepare_reserves_ports_across_batch() {
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
        config.speed_test_item.mixed_concurrency_count = 2;
        let reserved_base = reserve_speedtest_base_port(&mut config);
        let reserved_port = i32::from(
            reserved_base
                .local_addr()
                .expect("speedtest test operation should succeed")
                .port(),
        );

        manager
            .run(&database, &config, SpeedTestKind::Mixed, Vec::new())
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
                .run(&database, &config, SpeedTestKind::Mixed, Vec::new())
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
            remarks: index_id.to_string(),
            protocol: ProfileProtocol::Vmess {
                server: ServerEndpoint {
                    address: "127.0.0.1".to_string(),
                    port,
                },
                uuid: "00000000-0000-0000-0000-000000000000".to_string(),
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
