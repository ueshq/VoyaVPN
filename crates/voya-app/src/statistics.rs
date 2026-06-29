use std::{
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use serde::Deserialize;
use thiserror::Error;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use voya_core::{
    AppConfig, CoreType, InboundProtocol, ServerStatItem, DEFAULT_LOCAL_PORT, LOOPBACK,
};
use voya_db::{Database, DbError};

use crate::supervisor::{CoreSupervisor, SupervisorSnapshot};

const STATISTICS_CHANNEL_SIZE: usize = 64;
const COALESCE_INTERVAL: Duration = Duration::from_secs(1);
const SINGBOX_RECONNECT_INITIAL_DELAY: Duration = Duration::from_secs(1);
const SINGBOX_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(30);
const SINGBOX_INITIAL_DELAY: Duration = Duration::from_secs(5);
const SINGBOX_WS_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const WS_RECONNECT_JITTER_DIVISOR: u32 = 4;

pub type Result<T> = std::result::Result<T, StatisticsError>;

#[derive(Debug, Error)]
pub enum StatisticsError {
    #[error(transparent)]
    Database(#[from] DbError),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServerSpeedSample {
    pub proxy_up_bytes: i64,
    pub proxy_down_bytes: i64,
    pub direct_up_bytes: i64,
    pub direct_down_bytes: i64,
}

impl ServerSpeedSample {
    #[must_use]
    pub const fn has_traffic(self) -> bool {
        self.proxy_up_bytes > 0
            || self.proxy_down_bytes > 0
            || self.direct_up_bytes > 0
            || self.direct_down_bytes > 0
    }

    fn add(&mut self, sample: Self) {
        self.proxy_up_bytes = self
            .proxy_up_bytes
            .saturating_add(sample.proxy_up_bytes.max(0));
        self.proxy_down_bytes = self
            .proxy_down_bytes
            .saturating_add(sample.proxy_down_bytes.max(0));
        self.direct_up_bytes = self
            .direct_up_bytes
            .saturating_add(sample.direct_up_bytes.max(0));
        self.direct_down_bytes = self
            .direct_down_bytes
            .saturating_add(sample.direct_down_bytes.max(0));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatisticsSnapshot {
    pub active_profile_id: Option<String>,
    pub proxy_upload_bytes_per_second: f64,
    pub proxy_download_bytes_per_second: f64,
    pub direct_upload_bytes_per_second: f64,
    pub direct_download_bytes_per_second: f64,
    pub upload_bytes_per_second: f64,
    pub download_bytes_per_second: f64,
    pub server_stat: Option<ServerStatItem>,
}

impl StatisticsSnapshot {
    #[must_use]
    pub fn zero() -> Self {
        Self {
            active_profile_id: None,
            proxy_upload_bytes_per_second: 0.0,
            proxy_download_bytes_per_second: 0.0,
            direct_upload_bytes_per_second: 0.0,
            direct_download_bytes_per_second: 0.0,
            upload_bytes_per_second: 0.0,
            download_bytes_per_second: 0.0,
            server_stat: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatisticsConfigSnapshot {
    pub enable_statistics: bool,
    pub display_real_time_speed: bool,
    pub active_profile_id: Option<String>,
    pub state_port: u16,
    pub state_port2: u16,
}

impl StatisticsConfigSnapshot {
    #[must_use]
    pub fn from_app_config(config: &AppConfig) -> Self {
        Self {
            enable_statistics: config.gui_item.enable_statistics,
            display_real_time_speed: config.gui_item.display_real_time_speed,
            active_profile_id: nonempty(config.index_id.clone()),
            state_port: clamp_port(inbound_port(config, InboundProtocol::api)),
            state_port2: clamp_port(
                inbound_port(config, InboundProtocol::api2)
                    + i32::from(config.tun_mode_item.enable_tun),
            ),
        }
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enable_statistics || self.display_real_time_speed
    }
}

pub trait StatisticsConfigSource: Send + Sync {
    fn snapshot(&self) -> StatisticsConfigSnapshot;
}

pub trait StatisticsEventSink: Send + Sync {
    fn emit_statistics(&self, snapshot: StatisticsSnapshot);
}

#[derive(Clone)]
pub struct SharedAppConfigSource {
    config: Arc<RwLock<AppConfig>>,
}

impl SharedAppConfigSource {
    #[must_use]
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        Self { config }
    }
}

impl StatisticsConfigSource for SharedAppConfigSource {
    fn snapshot(&self) -> StatisticsConfigSnapshot {
        self.config
            .read()
            .map(|config| StatisticsConfigSnapshot::from_app_config(&config))
            .unwrap_or_else(|_| StatisticsConfigSnapshot::from_app_config(&AppConfig::default()))
    }
}

#[derive(Clone)]
pub struct NoopStatisticsEventSink;

impl StatisticsEventSink for NoopStatisticsEventSink {
    fn emit_statistics(&self, _snapshot: StatisticsSnapshot) {}
}

pub struct StatisticsManager {
    shutdown: watch::Sender<bool>,
    handles: Vec<JoinHandle<()>>,
}

impl StatisticsManager {
    pub fn spawn(
        database: Database,
        supervisor: CoreSupervisor,
        config_source: Arc<dyn StatisticsConfigSource>,
        event_sink: Arc<dyn StatisticsEventSink>,
    ) -> Self {
        let (sample_tx, sample_rx) = mpsc::channel(STATISTICS_CHANNEL_SIZE);
        let (shutdown, shutdown_rx) = watch::channel(false);

        let handles = vec![
            tokio::spawn(run_statistics_aggregator(
                database,
                Arc::clone(&config_source),
                event_sink,
                sample_rx,
                shutdown_rx.clone(),
            )),
            tokio::spawn(run_singbox_statistics_service(
                config_source,
                supervisor,
                sample_tx,
                shutdown_rx,
            )),
        ];

        Self { shutdown, handles }
    }

    pub fn close(&self) {
        let _ = self.shutdown.send(true);
    }

    pub async fn initialize_data(database: &Database, date_now: i64) -> Result<()> {
        database.server_stats().delete_orphans().await?;
        database.server_stats().reset_rollover(date_now).await?;

        Ok(())
    }
}

impl Drop for StatisticsManager {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        for handle in &self.handles {
            handle.abort();
        }
    }
}

pub async fn apply_statistics_sample(
    database: &Database,
    config: &StatisticsConfigSnapshot,
    sample: ServerSpeedSample,
    date_now: i64,
) -> Result<Option<StatisticsSnapshot>> {
    if !config.enabled() || !sample.has_traffic() {
        return Ok(None);
    }

    let server_stat = if let Some(active_profile_id) = &config.active_profile_id {
        Some(
            database
                .server_stats()
                .add_traffic(
                    active_profile_id,
                    date_now,
                    sample.proxy_up_bytes,
                    sample.proxy_down_bytes,
                )
                .await?,
        )
    } else {
        None
    };

    Ok(Some(snapshot_from_sample(config, sample, server_stat)))
}

#[must_use]
pub fn parse_singbox_traffic_sample(source: &str) -> Option<ServerSpeedSample> {
    #[derive(Deserialize)]
    struct TrafficItem {
        #[serde(alias = "Up")]
        up: u64,
        #[serde(alias = "Down")]
        down: u64,
    }

    let traffic = serde_json::from_str::<TrafficItem>(source).ok()?;

    Some(ServerSpeedSample {
        proxy_up_bytes: i64::try_from(traffic.up).unwrap_or(i64::MAX),
        proxy_down_bytes: i64::try_from(traffic.down).unwrap_or(i64::MAX),
        direct_up_bytes: 0,
        direct_down_bytes: 0,
    })
}

#[must_use]
pub fn singbox_state_port2(config: &AppConfig) -> u16 {
    clamp_port(
        inbound_port(config, InboundProtocol::api2) + i32::from(config.tun_mode_item.enable_tun),
    )
}

#[must_use]
pub fn core_matches_singbox(core_type: Option<CoreType>) -> bool {
    core_type.is_some_and(core_type_matches_singbox)
}

#[must_use]
pub fn current_day_marker() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs() / 86_400).unwrap_or(i64::MAX)
        })
}

async fn run_statistics_aggregator(
    database: Database,
    config_source: Arc<dyn StatisticsConfigSource>,
    event_sink: Arc<dyn StatisticsEventSink>,
    mut sample_rx: mpsc::Receiver<ServerSpeedSample>,
    mut shutdown: watch::Receiver<bool>,
) {
    if let Err(error) = StatisticsManager::initialize_data(&database, current_day_marker()).await {
        tracing::warn!(?error, "failed to initialize server statistics");
    }

    let mut interval = time::interval(COALESCE_INTERVAL);
    let mut pending = ServerSpeedSample::default();

    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            sample = sample_rx.recv() => {
                let Some(sample) = sample else {
                    break;
                };
                pending.add(sample);
            }
            _ = interval.tick() => {
                let sample = pending;
                pending = ServerSpeedSample::default();
                let config = config_source.snapshot();
                match apply_statistics_sample(&database, &config, sample, current_day_marker()).await {
                    Ok(Some(snapshot)) => event_sink.emit_statistics(snapshot),
                    Ok(None) => {}
                    Err(error) => tracing::warn!(?error, "failed to apply statistics sample"),
                }
            }
        }
    }
}

async fn run_singbox_statistics_service(
    config_source: Arc<dyn StatisticsConfigSource>,
    supervisor: CoreSupervisor,
    sample_tx: mpsc::Sender<ServerSpeedSample>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut initial_delay = Box::pin(time::sleep(SINGBOX_INITIAL_DELAY));
    tokio::select! {
        changed = shutdown.changed() => {
            if changed.is_err() || *shutdown.borrow() {
                return;
            }
        }
        _ = &mut initial_delay => {}
    }

    let mut reconnect_backoff = WebSocketReconnectBackoff::new(
        SINGBOX_RECONNECT_INITIAL_DELAY,
        SINGBOX_RECONNECT_MAX_DELAY,
    );
    let mut active_identity = None;

    loop {
        if *shutdown.borrow() {
            break;
        }

        let config = config_source.snapshot();
        if !config.enabled() {
            active_identity = None;
            reconnect_backoff.reset();
            if sleep_or_shutdown(SINGBOX_RECONNECT_INITIAL_DELAY, &mut shutdown).await {
                break;
            }
            continue;
        }
        let Some(identity) = singbox_process_identity(&supervisor).await else {
            active_identity = None;
            reconnect_backoff.reset();
            if sleep_or_shutdown(SINGBOX_RECONNECT_INITIAL_DELAY, &mut shutdown).await {
                break;
            }
            continue;
        };
        if update_active_identity(&mut active_identity, identity) {
            reconnect_backoff.reset();
        }
        let Some(state_port) = available_state_port(config.state_port2) else {
            active_identity = None;
            reconnect_backoff.reset();
            tracing::debug!("skipping sing-box statistics because state port is unavailable");
            if sleep_or_shutdown(SINGBOX_RECONNECT_INITIAL_DELAY, &mut shutdown).await {
                break;
            }
            continue;
        };

        let url = format!("ws://{LOOPBACK}:{state_port}/traffic");
        match time::timeout(SINGBOX_WS_CONNECT_TIMEOUT, connect_async(&url)).await {
            Ok(Ok((mut stream, _))) => loop {
                match singbox_process_identity(&supervisor).await {
                    Some(current_identity) if current_identity == identity => {}
                    Some(_) | None => break,
                }

                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return;
                        }
                    }
                    message = time::timeout(COALESCE_INTERVAL, stream.next()) => {
                        match message {
                            Ok(Some(Ok(Message::Text(text)))) => {
                                if let Some(sample) = parse_singbox_traffic_sample(&text) {
                                    reconnect_backoff.reset();
                                    let _ = sample_tx.send(sample).await;
                                }
                            }
                            Ok(Some(Ok(Message::Binary(bytes)))) => {
                                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                    if let Some(sample) = parse_singbox_traffic_sample(&text) {
                                        reconnect_backoff.reset();
                                        let _ = sample_tx.send(sample).await;
                                    }
                                }
                            }
                            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
                            Ok(Some(Ok(_))) | Err(_) => {}
                            Ok(Some(Err(error))) => {
                                tracing::debug!(?error, "sing-box statistics websocket read failed");
                                break;
                            }
                        }
                    }
                }
            },
            Ok(Err(error)) => {
                tracing::debug!(?error, "failed to connect sing-box statistics websocket");
            }
            Err(error) => {
                tracing::debug!(?error, "timed out connecting sing-box statistics websocket");
            }
        }

        if sleep_or_shutdown(reconnect_backoff.next_delay(), &mut shutdown).await {
            break;
        }
    }
}

async fn singbox_process_identity(supervisor: &CoreSupervisor) -> Option<CoreProcessIdentity> {
    supervisor
        .status()
        .await
        .ok()
        .and_then(|snapshot| core_process_identity(snapshot, core_type_matches_singbox))
}

async fn sleep_or_shutdown(duration: Duration, shutdown: &mut watch::Receiver<bool>) -> bool {
    let sleep = time::sleep(duration);
    tokio::pin!(sleep);

    tokio::select! {
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
        _ = &mut sleep => false,
    }
}

fn snapshot_from_sample(
    config: &StatisticsConfigSnapshot,
    sample: ServerSpeedSample,
    server_stat: Option<ServerStatItem>,
) -> StatisticsSnapshot {
    StatisticsSnapshot {
        active_profile_id: config.active_profile_id.clone(),
        proxy_upload_bytes_per_second: sample.proxy_up_bytes.max(0) as f64,
        proxy_download_bytes_per_second: sample.proxy_down_bytes.max(0) as f64,
        direct_upload_bytes_per_second: sample.direct_up_bytes.max(0) as f64,
        direct_download_bytes_per_second: sample.direct_down_bytes.max(0) as f64,
        upload_bytes_per_second: sample.proxy_up_bytes.max(0) as f64
            + sample.direct_up_bytes.max(0) as f64,
        download_bytes_per_second: sample.proxy_down_bytes.max(0) as f64
            + sample.direct_down_bytes.max(0) as f64,
        server_stat,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoreProcessIdentity {
    core_type: CoreType,
    main_pid: u32,
    pre_pid: Option<u32>,
}

fn core_process_identity(
    snapshot: SupervisorSnapshot,
    matches_core_type: fn(CoreType) -> bool,
) -> Option<CoreProcessIdentity> {
    let core_type = snapshot.running_core_type?;
    let main_pid = snapshot.main_pid?;
    matches_core_type(core_type).then_some(CoreProcessIdentity {
        core_type,
        main_pid,
        pre_pid: snapshot.pre_pid,
    })
}

fn update_active_identity(
    active_identity: &mut Option<CoreProcessIdentity>,
    identity: CoreProcessIdentity,
) -> bool {
    if active_identity.as_ref() == Some(&identity) {
        return false;
    }

    *active_identity = Some(identity);
    true
}

fn core_type_matches_singbox(core_type: CoreType) -> bool {
    let _ = core_type;
    true
}

fn available_state_port(port: u16) -> Option<u16> {
    (port != 0).then_some(port)
}

#[derive(Debug, Clone)]
struct WebSocketReconnectBackoff {
    attempt: u32,
    initial: Duration,
    max: Duration,
}

impl WebSocketReconnectBackoff {
    const fn new(initial: Duration, max: Duration) -> Self {
        Self {
            attempt: 0,
            initial,
            max,
        }
    }

    fn reset(&mut self) {
        self.attempt = 0;
    }

    fn next_delay(&mut self) -> Duration {
        let delay = websocket_reconnect_delay(
            self.attempt,
            self.initial,
            self.max,
            reconnect_jitter_seed(),
        );
        self.attempt = self.attempt.saturating_add(1);
        delay
    }
}

fn websocket_reconnect_delay(
    attempt: u32,
    initial: Duration,
    max: Duration,
    jitter_seed: u64,
) -> Duration {
    let multiplier = 1_u32.checked_shl(attempt.min(16)).unwrap_or(u32::MAX);
    let scaled = initial.saturating_mul(multiplier);
    let base = if scaled > max { max } else { scaled };

    base.saturating_add(reconnect_jitter(base, jitter_seed))
}

fn reconnect_jitter(base: Duration, jitter_seed: u64) -> Duration {
    let jitter_limit_nanos =
        (base.as_nanos() / u128::from(WS_RECONNECT_JITTER_DIVISOR)).min(u128::from(u64::MAX));
    if jitter_limit_nanos == 0 {
        return Duration::ZERO;
    }

    let jitter_nanos = u128::from(jitter_seed) % (jitter_limit_nanos + 1);
    Duration::from_nanos(u64::try_from(jitter_nanos).unwrap_or(u64::MAX))
}

fn reconnect_jitter_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX) ^ u64::from(std::process::id())
        })
}

fn inbound_port(app_config: &AppConfig, protocol: InboundProtocol) -> i32 {
    app_config
        .inbound
        .iter()
        .find(|item| item.protocol == "socks")
        .map(|item| item.local_port)
        .or_else(|| app_config.inbound.first().map(|item| item.local_port))
        .unwrap_or(DEFAULT_LOCAL_PORT)
        + protocol.as_i32()
}

fn clamp_port(port: i32) -> u16 {
    u16::try_from(port.clamp(0, i32::from(u16::MAX))).unwrap_or(u16::MAX)
}

fn nonempty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use crate::supervisor::SupervisorConnectionState;
    use voya_core::{ConfigType, InItem, ProfileItem, TunModeItem};

    use super::*;

    #[test]
    fn statistics_singbox_traffic_parser_reads_ws_payload() {
        assert_eq!(
            parse_singbox_traffic_sample(r#"{"up":1234,"down":5678}"#),
            Some(ServerSpeedSample {
                proxy_up_bytes: 1234,
                proxy_down_bytes: 5678,
                direct_up_bytes: 0,
                direct_down_bytes: 0,
            })
        );
        assert_eq!(parse_singbox_traffic_sample("not-json"), None);
    }

    #[test]
    fn statistics_config_uses_singbox_state_port2() {
        let mut config = AppConfig {
            inbound: vec![InItem {
                local_port: 12000,
                protocol: "socks".to_string(),
                ..InItem::default()
            }],
            tun_mode_item: TunModeItem {
                enable_tun: true,
                ..TunModeItem::default()
            },
            ..AppConfig::default()
        };

        assert_eq!(singbox_state_port2(&config), 12006);

        config.tun_mode_item.enable_tun = false;
        assert_eq!(singbox_state_port2(&config), 12005);
    }

    #[test]
    fn statistics_core_type_matching_follows_singbox_only() {
        assert!(core_matches_singbox(Some(CoreType::sing_box)));
    }

    #[test]
    fn statistics_core_process_identity_tracks_pid_changes() {
        let first = SupervisorSnapshot {
            state: SupervisorConnectionState::Connected,
            active_profile_id: Some("profile-a".to_string()),
            main_pid: Some(100),
            pre_pid: None,
            running_core_type: Some(CoreType::sing_box),
        };
        let restarted = SupervisorSnapshot {
            main_pid: Some(101),
            ..first.clone()
        };
        let disconnected = SupervisorSnapshot {
            main_pid: None,
            ..first.clone()
        };

        assert_ne!(
            core_process_identity(first, core_type_matches_singbox),
            core_process_identity(restarted, core_type_matches_singbox)
        );
        assert_eq!(
            core_process_identity(disconnected, core_type_matches_singbox),
            None
        );
    }

    #[test]
    fn statistics_state_port_zero_is_unavailable() {
        let mut config = AppConfig {
            inbound: vec![InItem {
                local_port: -4,
                protocol: "socks".to_string(),
                ..InItem::default()
            }],
            ..AppConfig::default()
        };

        config
            .inbound
            .first_mut()
            .expect("test config has an inbound")
            .local_port = -5;
        assert_eq!(singbox_state_port2(&config), 0);
        assert_eq!(available_state_port(singbox_state_port2(&config)), None);
        assert_eq!(available_state_port(1), Some(1));
    }

    #[test]
    fn statistics_ws_reconnect_delay_backs_off_with_cap_and_jitter() {
        let initial = Duration::from_secs(1);
        let max = Duration::from_secs(8);

        let first = websocket_reconnect_delay(0, initial, max, 0);
        let second = websocket_reconnect_delay(1, initial, max, 0);
        let capped = websocket_reconnect_delay(12, initial, max, u64::MAX);

        assert_eq!(first, Duration::from_secs(1));
        assert_eq!(second, Duration::from_secs(2));
        assert!(capped >= max);
        assert!(capped <= max + Duration::from_secs(2));
    }

    #[tokio::test]
    async fn statistics_apply_sample_keys_persistence_to_active_server_and_sums_display() {
        let database = Database::connect_in_memory()
            .await
            .expect("statistics test operation should succeed");
        database
            .profiles()
            .upsert(&sample_profile("active"))
            .await
            .expect("statistics test operation should succeed");
        database
            .profiles()
            .upsert(&sample_profile("inactive"))
            .await
            .expect("statistics test operation should succeed");
        let config = StatisticsConfigSnapshot {
            enable_statistics: true,
            display_real_time_speed: true,
            active_profile_id: Some("active".to_string()),
            state_port: 10812,
            state_port2: 10813,
        };

        let snapshot = apply_statistics_sample(
            &database,
            &config,
            ServerSpeedSample {
                proxy_up_bytes: 1000,
                proxy_down_bytes: 2000,
                direct_up_bytes: 300,
                direct_down_bytes: 400,
            },
            10,
        )
        .await
        .expect("statistics test operation should succeed")
        .expect("snapshot");

        assert_eq!(snapshot.upload_bytes_per_second, 1300.0);
        assert_eq!(snapshot.download_bytes_per_second, 2400.0);
        assert_eq!(
            snapshot
                .server_stat
                .as_ref()
                .expect("statistics test operation should succeed")
                .index_id,
            "active"
        );
        assert_eq!(
            snapshot
                .server_stat
                .as_ref()
                .expect("statistics test operation should succeed")
                .total_up,
            1000
        );
        assert_eq!(
            snapshot
                .server_stat
                .as_ref()
                .expect("statistics test operation should succeed")
                .total_down,
            2000
        );
        assert!(database
            .server_stats()
            .get("inactive")
            .await
            .expect("statistics test operation should succeed")
            .is_none());
    }

    #[tokio::test]
    async fn statistics_apply_sample_rolls_today_at_date_boundary() {
        let database = Database::connect_in_memory()
            .await
            .expect("statistics test operation should succeed");
        database
            .profiles()
            .upsert(&sample_profile("active"))
            .await
            .expect("statistics test operation should succeed");
        database
            .server_stats()
            .upsert(&ServerStatItem {
                index_id: "active".to_string(),
                total_up: 100,
                total_down: 200,
                today_up: 90,
                today_down: 180,
                date_now: 1,
            })
            .await
            .expect("statistics test operation should succeed");
        let config = StatisticsConfigSnapshot {
            enable_statistics: true,
            display_real_time_speed: false,
            active_profile_id: Some("active".to_string()),
            state_port: 10812,
            state_port2: 10813,
        };

        let snapshot = apply_statistics_sample(
            &database,
            &config,
            ServerSpeedSample {
                proxy_up_bytes: 5,
                proxy_down_bytes: 7,
                direct_up_bytes: 11,
                direct_down_bytes: 13,
            },
            2,
        )
        .await
        .expect("statistics test operation should succeed")
        .expect("snapshot");
        let stat = snapshot
            .server_stat
            .expect("statistics test operation should succeed");

        assert_eq!(stat.today_up, 5);
        assert_eq!(stat.today_down, 7);
        assert_eq!(stat.total_up, 105);
        assert_eq!(stat.total_down, 207);
        assert_eq!(stat.date_now, 2);
    }

    fn sample_profile(index_id: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VMess,
            remarks: index_id.to_string(),
            address: "example.test".to_string(),
            port: 443,
            ..ProfileItem::default()
        }
    }
}
