use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use tokio::{
    net::{lookup_host, TcpStream},
    time,
};
use voya_core::{
    AppConfig, ConfigType, CoreType, InboundProtocol, ProfileItem, SpeedActionType,
    DEFAULT_LOCAL_PORT,
};
use voya_db::{Database, DbError};
use voya_udptest::{UdpTestError, UdpTestService};

use crate::profiles::ProfileExManager;

const TCPING_TIMEOUT: Duration = Duration::from_secs(5);
const REALPING_FALLBACK_URL: &str = "https://www.google.com/generate_204";

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
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Udp(#[from] UdpTestError),
    #[error("speedtest was cancelled")]
    Cancelled,
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

pub trait SpeedtestProbe: Send + Sync {
    fn tcping(
        &self,
        item: ServerTestItem,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>>;

    fn realping(
        &self,
        item: ServerTestItem,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>>;

    fn download_speed(
        &self,
        item: ServerTestItem,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<f64>>;

    fn udp_test(
        &self,
        item: ServerTestItem,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>>;
}

#[derive(Clone)]
pub struct ReqwestSpeedtestProbe {
    client: reqwest::Client,
}

impl Default for ReqwestSpeedtestProbe {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

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
        _item: ServerTestItem,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<RealPingProbeResult>> {
        let client = self.client.clone();
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let url = if config.speed_test_item.speed_ping_test_url.trim().is_empty() {
                REALPING_FALLBACK_URL
            } else {
                config.speed_test_item.speed_ping_test_url.as_str()
            };
            let started = Instant::now();
            let request = client
                .get(url)
                .timeout(Duration::from_secs(
                    u64::try_from(config.speed_test_item.speed_test_timeout.max(1)).unwrap_or(1),
                ))
                .send()
                .await;
            check_cancelled(&cancel)?;
            request?.error_for_status()?;

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

            Ok(RealPingProbeResult {
                delay: millis_i32(started.elapsed()),
                ip_info,
            })
        })
    }

    fn download_speed(
        &self,
        _item: ServerTestItem,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<f64>> {
        let client = self.client.clone();
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let started = Instant::now();
            let response = client
                .get(config.speed_test_item.speed_test_url.as_str())
                .timeout(Duration::from_secs(
                    u64::try_from(config.speed_test_item.speed_test_timeout.max(1)).unwrap_or(1),
                ))
                .send()
                .await?
                .error_for_status()?;
            let bytes = response.bytes().await?;
            check_cancelled(&cancel)?;
            let elapsed = started.elapsed().as_secs_f64().max(0.001);

            Ok(bytes.len() as f64 / elapsed)
        })
    }

    fn udp_test(
        &self,
        item: ServerTestItem,
        config: AppConfig,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<i32>> {
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let (service, target) =
                UdpTestService::from_target(Some(&config.speed_test_item.udp_test_target));
            let elapsed = service
                .send_via_socks5(
                    "127.0.0.1",
                    item.socks_port,
                    &target,
                    Duration::from_secs(5),
                )
                .await?;
            check_cancelled(&cancel)?;

            Ok(millis_i32(elapsed))
        })
    }
}

#[derive(Clone)]
pub struct SpeedtestManager {
    probe: Arc<dyn SpeedtestProbe>,
    active_cancel: Arc<Mutex<Option<CancellationFlag>>>,
}

impl Default for SpeedtestManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedtestManager {
    #[must_use]
    pub fn new() -> Self {
        Self::with_probe(Arc::new(ReqwestSpeedtestProbe::default()))
    }

    #[must_use]
    pub fn with_probe(probe: Arc<dyn SpeedtestProbe>) -> Self {
        Self {
            probe,
            active_cancel: Arc::new(Mutex::new(None)),
        }
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
        let selected = select_test_items(database, config, action, &index_ids).await?;
        clear_previous_results(database, action, &selected, &on_result).await?;

        let mut results = Vec::new();
        let mut completed_count = 0_u32;

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

        let cancelled = is_cancelled(&cancel);
        self.finish_job(&cancel)?;

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
            SpeedActionType::Realping => {
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

                if !is_cancelled(&cancel) {
                    let udp = self.run_udp(database, config, action, item, cancel).await?;
                    on_result(udp.clone());
                    results.push(udp);
                }
            }
            SpeedActionType::FastRealping => {
                unreachable!("normalized action cannot be FastRealping")
            }
        }

        Ok(results)
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
        let result = match self.probe.realping(item, config.clone(), cancel).await {
            Ok(realping) => SpeedTestResult {
                action,
                index_id,
                delay: Some(realping.delay),
                speed: None,
                message: Some(realping.delay.to_string()),
                ip_info: realping.ip_info,
            },
            Err(error) => SpeedTestResult {
                action,
                index_id,
                delay: Some(-1),
                speed: None,
                message: Some(error.to_string()),
                ip_info: Some("Skipped".to_string()),
            },
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
            .download_speed(item, config.clone(), cancel)
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
            Err(error) => SpeedTestResult {
                action,
                index_id,
                delay: None,
                speed: Some(0.0),
                message: Some(error.to_string()),
                ip_info: None,
            },
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
            .udp_test(item, config.clone(), cancel)
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
                    .unwrap_or(CoreType::Xray),
                profile,
            })
        })
        .collect()
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
    use std::sync::Mutex as StdMutex;

    use voya_core::{ProfileExItem, ProtocolExtraItem};
    use voya_db::Database;

    use super::*;

    #[derive(Default)]
    struct RecordingProbe {
        calls: Arc<StdMutex<Vec<String>>>,
        block_realping: bool,
    }

    impl RecordingProbe {
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
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
                    .unwrap()
                    .push(format!("tcping:{}", item.index_id));
                Ok(11)
            })
        }

        fn realping(
            &self,
            item: ServerTestItem,
            _config: AppConfig,
            cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<RealPingProbeResult>> {
            let calls = Arc::clone(&self.calls);
            let block = self.block_realping;
            Box::pin(async move {
                calls
                    .lock()
                    .unwrap()
                    .push(format!("realping:{}", item.index_id));
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
            item: ServerTestItem,
            _config: AppConfig,
            _cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<f64>> {
            let calls = Arc::clone(&self.calls);
            Box::pin(async move {
                calls
                    .lock()
                    .unwrap()
                    .push(format!("speedtest:{}", item.index_id));
                Ok(2048.0)
            })
        }

        fn udp_test(
            &self,
            item: ServerTestItem,
            _config: AppConfig,
            _cancel: CancellationFlag,
        ) -> BoxFuture<'static, Result<i32>> {
            let calls = Arc::clone(&self.calls);
            Box::pin(async move {
                calls.lock().unwrap().push(format!("udp:{}", item.index_id));
                Ok(55)
            })
        }
    }

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
    async fn speedtest_manager_mixedtest_combines_realping_speedtest_and_udp() {
        let database = Database::connect_in_memory().await.unwrap();
        insert_profile(&database, "a", 443).await;
        let probe = Arc::new(RecordingProbe::default());
        let manager = SpeedtestManager::with_probe(probe.clone());
        let config = AppConfig::default();

        let run = manager
            .run(
                &database,
                &config,
                SpeedActionType::Mixedtest,
                vec!["a".to_string()],
            )
            .await
            .unwrap();

        assert!(!run.cancelled);
        assert_eq!(run.completed_count, 1);
        assert_eq!(
            probe.calls(),
            vec![
                "realping:a".to_string(),
                "speedtest:a".to_string(),
                "udp:a".to_string(),
            ]
        );
        let profile_ex = database.profile_exs().get("a").await.unwrap().unwrap();
        assert_eq!(profile_ex.delay, 55);
        assert_eq!(profile_ex.speed, 2048.0);
        assert_eq!(profile_ex.ip_info.as_deref(), Some("US"));
    }

    #[tokio::test]
    async fn speedtest_manager_fast_realping_uses_all_profiles_as_realping() {
        let database = Database::connect_in_memory().await.unwrap();
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe::default());
        let manager = SpeedtestManager::with_probe(probe.clone());

        let run = manager
            .run(
                &database,
                &AppConfig::default(),
                SpeedActionType::FastRealping,
                Vec::new(),
            )
            .await
            .unwrap();

        assert_eq!(run.selected_count, 2);
        assert_eq!(
            probe.calls(),
            vec!["realping:a".to_string(), "realping:b".to_string()]
        );
    }

    #[tokio::test]
    async fn speedtest_manager_speedtest_realpings_before_download() {
        let database = Database::connect_in_memory().await.unwrap();
        insert_profile(&database, "a", 443).await;
        let probe = Arc::new(RecordingProbe::default());
        let manager = SpeedtestManager::with_probe(probe.clone());

        manager
            .run(
                &database,
                &AppConfig::default(),
                SpeedActionType::Speedtest,
                vec!["a".to_string()],
            )
            .await
            .unwrap();

        assert_eq!(
            probe.calls(),
            vec!["realping:a".to_string(), "speedtest:a".to_string()]
        );
    }

    #[tokio::test]
    async fn speedtest_manager_cancel_stops_active_jobs() {
        let database = Database::connect_in_memory().await.unwrap();
        insert_profile(&database, "a", 443).await;
        insert_profile(&database, "b", 8443).await;
        let probe = Arc::new(RecordingProbe {
            calls: Arc::new(StdMutex::new(Vec::new())),
            block_realping: true,
        });
        let manager = SpeedtestManager::with_probe(probe.clone());
        let task_manager = manager.clone();

        let handle = tokio::spawn(async move {
            task_manager
                .run(
                    &database,
                    &AppConfig::default(),
                    SpeedActionType::Mixedtest,
                    Vec::new(),
                )
                .await
                .unwrap()
        });

        loop {
            if probe.calls().iter().any(|call| call == "realping:a") {
                break;
            }
            time::sleep(Duration::from_millis(10)).await;
        }

        assert!(manager.cancel().unwrap());
        let run = handle.await.unwrap();

        assert!(run.cancelled);
        assert_eq!(run.completed_count, 1);
        assert_eq!(probe.calls(), vec!["realping:a".to_string()]);
        assert!(!manager.status().unwrap().running);
    }

    async fn insert_profile(database: &Database, index_id: &str, port: i32) {
        let profile = ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VMess,
            core_type: Some(CoreType::Xray),
            remarks: index_id.to_string(),
            address: "127.0.0.1".to_string(),
            port,
            password: "uuid".to_string(),
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
            .unwrap();
    }
}
