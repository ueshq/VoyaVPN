use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use voya_core::{
    AppConfig, CoreType, InboundProtocol, ServerStatItem, DEFAULT_LOCAL_PORT, DIRECT_TAG, LOOPBACK,
    PROXY_TAG,
};
use voya_db::{Database, DbError};

use crate::supervisor::CoreSupervisor;

const STATISTICS_CHANNEL_SIZE: usize = 64;
const COALESCE_INTERVAL: Duration = Duration::from_secs(1);
const XRAY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SINGBOX_RECONNECT_INTERVAL: Duration = Duration::from_secs(1);
const SINGBOX_INITIAL_DELAY: Duration = Duration::from_secs(5);

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
            tokio::spawn(run_xray_statistics_service(
                Arc::clone(&config_source),
                supervisor.clone(),
                sample_tx.clone(),
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

#[derive(Debug, Default)]
pub struct XrayDebugVarsParser {
    previous: Option<ServerSpeedSample>,
}

impl XrayDebugVarsParser {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.previous = None;
    }

    #[must_use]
    pub fn parse_next(&mut self, source: &str) -> Option<ServerSpeedSample> {
        let current = parse_xray_debug_vars_totals(source)?;
        let previous = self.previous.unwrap_or_default();
        self.previous = Some(current);

        if counters_rolled_back(previous, current) {
            return None;
        }

        Some(ServerSpeedSample {
            proxy_up_bytes: current
                .proxy_up_bytes
                .saturating_sub(previous.proxy_up_bytes),
            proxy_down_bytes: current
                .proxy_down_bytes
                .saturating_sub(previous.proxy_down_bytes),
            direct_up_bytes: current
                .direct_up_bytes
                .saturating_sub(previous.direct_up_bytes),
            direct_down_bytes: current
                .direct_down_bytes
                .saturating_sub(previous.direct_down_bytes),
        })
    }
}

#[must_use]
pub fn xray_state_port(config: &AppConfig) -> u16 {
    clamp_port(inbound_port(config, InboundProtocol::api))
}

#[must_use]
pub fn singbox_state_port2(config: &AppConfig) -> u16 {
    clamp_port(
        inbound_port(config, InboundProtocol::api2) + i32::from(config.tun_mode_item.enable_tun),
    )
}

#[must_use]
pub fn core_matches_xray(core_type: Option<CoreType>) -> bool {
    matches!(
        core_type,
        Some(CoreType::Xray | CoreType::v2fly | CoreType::v2fly_v5)
    )
}

#[must_use]
pub fn core_matches_singbox(core_type: Option<CoreType>) -> bool {
    matches!(core_type, Some(CoreType::sing_box | CoreType::mihomo))
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

async fn run_xray_statistics_service(
    config_source: Arc<dyn StatisticsConfigSource>,
    supervisor: CoreSupervisor,
    sample_tx: mpsc::Sender<ServerSpeedSample>,
    mut shutdown: watch::Receiver<bool>,
) {
    let client = reqwest::Client::new();
    let mut parser = XrayDebugVarsParser::new();
    let mut interval = time::interval(XRAY_POLL_INTERVAL);

    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = interval.tick() => {
                let config = config_source.snapshot();
                if !config.enabled() || !is_running_xray(&supervisor).await {
                    parser.reset();
                    continue;
                }

                let url = format!("http://{LOOPBACK}:{}/debug/vars", config.state_port);
                match client.get(url).send().await {
                    Ok(response) => match response.text().await {
                        Ok(body) => {
                            if let Some(sample) = parser.parse_next(&body) {
                                let _ = sample_tx.send(sample).await;
                            }
                        }
                        Err(error) => tracing::debug!(?error, "failed to read Xray statistics body"),
                    },
                    Err(error) => tracing::debug!(?error, "failed to poll Xray statistics"),
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

    loop {
        if *shutdown.borrow() {
            break;
        }

        let config = config_source.snapshot();
        if !config.enabled() || !is_running_singbox(&supervisor).await {
            if sleep_or_shutdown(SINGBOX_RECONNECT_INTERVAL, &mut shutdown).await {
                break;
            }
            continue;
        }

        let url = format!("ws://{LOOPBACK}:{}/traffic", config.state_port2);
        match connect_async(&url).await {
            Ok((mut stream, _)) => loop {
                if !is_running_singbox(&supervisor).await {
                    break;
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
                                    let _ = sample_tx.send(sample).await;
                                }
                            }
                            Ok(Some(Ok(Message::Binary(bytes)))) => {
                                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                    if let Some(sample) = parse_singbox_traffic_sample(&text) {
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
            Err(error) => {
                tracing::debug!(?error, "failed to connect sing-box statistics websocket");
            }
        }

        if sleep_or_shutdown(SINGBOX_RECONNECT_INTERVAL, &mut shutdown).await {
            break;
        }
    }
}

async fn is_running_xray(supervisor: &CoreSupervisor) -> bool {
    supervisor
        .status()
        .await
        .map(|snapshot| core_matches_xray(snapshot.running_core_type))
        .unwrap_or(false)
}

async fn is_running_singbox(supervisor: &CoreSupervisor) -> bool {
    supervisor
        .status()
        .await
        .map(|snapshot| core_matches_singbox(snapshot.running_core_type))
        .unwrap_or(false)
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

fn parse_xray_debug_vars_totals(source: &str) -> Option<ServerSpeedSample> {
    #[derive(Deserialize)]
    struct V2rayMetricsVars {
        stats: Option<V2rayMetricsVarsStats>,
    }

    #[derive(Deserialize)]
    struct V2rayMetricsVarsStats {
        outbound: Option<BTreeMap<String, Value>>,
    }

    let source = serde_json::from_str::<V2rayMetricsVars>(source).ok()?;
    let outbound = source.stats?.outbound?;
    let mut sample = ServerSpeedSample::default();

    for (key, value) in outbound {
        if key.contains(">>>traffic>>>") {
            apply_xray_flat_counter(&mut sample, &key, &value);
            continue;
        }

        let up = json_counter_field(&value, "uplink");
        let down = json_counter_field(&value, "downlink");
        if key.starts_with(PROXY_TAG) {
            sample.proxy_up_bytes = sample.proxy_up_bytes.saturating_add(up);
            sample.proxy_down_bytes = sample.proxy_down_bytes.saturating_add(down);
        } else if key == DIRECT_TAG {
            sample.direct_up_bytes = sample.direct_up_bytes.saturating_add(up);
            sample.direct_down_bytes = sample.direct_down_bytes.saturating_add(down);
        }
    }

    Some(sample)
}

fn apply_xray_flat_counter(sample: &mut ServerSpeedSample, key: &str, value: &Value) {
    let mut parts = key.split(">>>");
    let Some(tag) = parts.next() else {
        return;
    };
    let direction = key.rsplit(">>>").next().unwrap_or_default();
    let amount = json_counter_value(value);

    match (tag, direction) {
        (tag, "uplink") if tag.starts_with(PROXY_TAG) => {
            sample.proxy_up_bytes = sample.proxy_up_bytes.saturating_add(amount);
        }
        (tag, "downlink") if tag.starts_with(PROXY_TAG) => {
            sample.proxy_down_bytes = sample.proxy_down_bytes.saturating_add(amount);
        }
        (DIRECT_TAG, "uplink") => {
            sample.direct_up_bytes = sample.direct_up_bytes.saturating_add(amount);
        }
        (DIRECT_TAG, "downlink") => {
            sample.direct_down_bytes = sample.direct_down_bytes.saturating_add(amount);
        }
        _ => {}
    }
}

fn json_counter_field(value: &Value, field: &str) -> i64 {
    value.get(field).map_or(0, json_counter_value)
}

fn json_counter_value(value: &Value) -> i64 {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.get("value").and_then(json_counter_value_checked))
        .unwrap_or(0)
}

fn json_counter_value_checked(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
}

fn counters_rolled_back(previous: ServerSpeedSample, current: ServerSpeedSample) -> bool {
    current.proxy_up_bytes < previous.proxy_up_bytes
        || current.proxy_down_bytes < previous.proxy_down_bytes
        || current.direct_up_bytes < previous.direct_up_bytes
        || current.direct_down_bytes < previous.direct_down_bytes
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
    use voya_core::{ConfigType, GuiItem, InItem, ProfileItem, TunModeItem};

    use super::*;

    #[test]
    fn statistics_xray_debug_vars_parser_sums_proxy_and_direct_deltas() {
        let mut parser = XrayDebugVarsParser::new();
        let first = parser
            .parse_next(
                r#"{
                    "stats": {
                        "outbound": {
                            "proxy": { "uplink": 4096, "downlink": 8192 },
                            "proxy10808": { "uplink": 1024, "downlink": 2048 },
                            "direct": { "uplink": 512, "downlink": 256 }
                        }
                    }
                }"#,
            )
            .expect("first sample");
        assert_eq!(
            first,
            ServerSpeedSample {
                proxy_up_bytes: 5120,
                proxy_down_bytes: 10240,
                direct_up_bytes: 512,
                direct_down_bytes: 256,
            }
        );

        let second = parser
            .parse_next(
                r#"{
                    "stats": {
                        "outbound": {
                            "proxy": { "uplink": 5120, "downlink": 12288 },
                            "proxy10808": { "uplink": 2048, "downlink": 4096 },
                            "direct": { "uplink": 768, "downlink": 512 }
                        }
                    }
                }"#,
            )
            .expect("second sample");
        assert_eq!(
            second,
            ServerSpeedSample {
                proxy_up_bytes: 2048,
                proxy_down_bytes: 6144,
                direct_up_bytes: 256,
                direct_down_bytes: 256,
            }
        );
    }

    #[test]
    fn statistics_xray_debug_vars_parser_reads_flat_expvar_counters() {
        let mut parser = XrayDebugVarsParser::new();
        let sample = parser
            .parse_next(
                r#"{
                    "stats": {
                        "outbound": {
                            "proxy>>>traffic>>>uplink": { "value": 100 },
                            "proxy>>>traffic>>>downlink": { "value": 200 },
                            "direct>>>traffic>>>uplink": 30,
                            "direct>>>traffic>>>downlink": 40
                        }
                    }
                }"#,
            )
            .expect("flat counters");

        assert_eq!(
            sample,
            ServerSpeedSample {
                proxy_up_bytes: 100,
                proxy_down_bytes: 200,
                direct_up_bytes: 30,
                direct_down_bytes: 40,
            }
        );
    }

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
    fn statistics_config_uses_state_port_and_state_port2() {
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

        assert_eq!(xray_state_port(&config), 12004);
        assert_eq!(singbox_state_port2(&config), 12006);

        config.tun_mode_item.enable_tun = false;
        assert_eq!(singbox_state_port2(&config), 12005);
    }

    #[test]
    fn statistics_core_type_matching_follows_v2rayn_groups() {
        assert!(core_matches_xray(Some(CoreType::Xray)));
        assert!(core_matches_xray(Some(CoreType::v2fly)));
        assert!(core_matches_xray(Some(CoreType::v2fly_v5)));
        assert!(!core_matches_xray(Some(CoreType::sing_box)));
        assert!(core_matches_singbox(Some(CoreType::sing_box)));
        assert!(core_matches_singbox(Some(CoreType::mihomo)));
        assert!(!core_matches_singbox(Some(CoreType::Xray)));
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

    #[allow(dead_code)]
    fn enabled_config(index_id: &str) -> AppConfig {
        AppConfig {
            index_id: index_id.to_string(),
            gui_item: GuiItem {
                enable_statistics: true,
                display_real_time_speed: true,
                ..GuiItem::default()
            },
            ..AppConfig::default()
        }
    }
}
