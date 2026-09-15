use std::{
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time,
};
use voya_core::{text::nonempty_string, AppConfig, ServerStatItem};
use voya_db::{Database, DbError};
use voya_net::clash::{
    ClashTraffic, ClashWebSocketClient, ClashWebSocketEvent, ClashWebSocketResource,
};

use crate::{
    backoff::{sleep_or_shutdown, WebSocketReconnectBackoff},
    config_mutation::SharedAppConfig,
    proxy_runtime::proxy_runtime_endpoint,
    supervisor::{CoreSupervisor, SupervisorSnapshot},
};

const STATISTICS_CHANNEL_SIZE: usize = 64;
const COALESCE_INTERVAL: Duration = Duration::from_secs(1);
/// How long measured bytes may sit in memory before SQLite sees them.
///
/// The per-second row rewrite this replaces cost 3600 write transactions an
/// hour for a counter nobody reads until the app is reopened, and every one of
/// them competed with the mutations the user is actually waiting on.
const TRAFFIC_FLUSH_INTERVAL: Duration = Duration::from_secs(10);
/// Coalesced ticks per flush. The aggregator's only clock is its tick.
const TRAFFIC_FLUSH_TICKS: u64 = TRAFFIC_FLUSH_INTERVAL.as_secs() / COALESCE_INTERVAL.as_secs();
const SINGBOX_RECONNECT_INITIAL_DELAY: Duration = Duration::from_secs(1);
const SINGBOX_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(30);
const SINGBOX_INITIAL_DELAY: Duration = Duration::from_secs(5);
const SINGBOX_WS_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

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
    const fn has_traffic(self) -> bool {
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
    pub active_profile_id: Option<String>,
}

impl StatisticsConfigSnapshot {
    #[must_use]
    fn from_app_config(config: &AppConfig) -> Self {
        Self {
            active_profile_id: nonempty_string(Some(config.index_id.as_str())),
        }
    }
}

pub trait StatisticsEventSink: Send + Sync {
    fn emit_statistics(&self, snapshot: StatisticsSnapshot);
}

/// The statistics toggles, read from the live configuration on every tick.
///
/// A poisoned lock reads as the defaults rather than stopping the loop.
fn statistics_config(config: &RwLock<AppConfig>) -> StatisticsConfigSnapshot {
    config
        .read()
        .map(|config| StatisticsConfigSnapshot::from_app_config(&config))
        .unwrap_or_else(|_| StatisticsConfigSnapshot::from_app_config(&AppConfig::default()))
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
        config: SharedAppConfig,
        event_sink: Arc<dyn StatisticsEventSink>,
    ) -> Self {
        let (sample_tx, sample_rx) = mpsc::channel(STATISTICS_CHANNEL_SIZE);
        let (shutdown, shutdown_rx) = watch::channel(false);

        let handles = vec![
            tokio::spawn(run_statistics_aggregator(
                database,
                config,
                event_sink,
                sample_rx,
                shutdown_rx.clone(),
            )),
            tokio::spawn(run_singbox_statistics_service(
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

    async fn initialize_data(database: &Database, date_now: i64) -> Result<()> {
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

async fn add_traffic(
    database: &Database,
    index_id: &str,
    date_now: i64,
    sample: ServerSpeedSample,
) -> Result<ServerStatItem> {
    database
        .server_stats()
        .add_traffic(
            index_id,
            date_now,
            sample.proxy_up_bytes,
            sample.proxy_down_bytes,
        )
        .await
        .map_err(Into::into)
}

/// Traffic measured but not yet written to SQLite.
///
/// The UI still gets a snapshot every second; only the database round trip is
/// batched. `baseline` is the last row the database returned, so the projected
/// totals keep climbing between flushes instead of freezing at the last write.
#[derive(Debug, Default)]
struct TrafficWriteBuffer {
    /// Row the buffered bytes belong to. Both halves matter: crediting a
    /// profile switch or a midnight rollover to the wrong row would move
    /// traffic between profiles or between days.
    target: Option<(String, i64)>,
    buffered: ServerSpeedSample,
    baseline: Option<ServerStatItem>,
}

impl TrafficWriteBuffer {
    fn targets(&self, index_id: &str, date_now: i64) -> bool {
        self.target
            .as_ref()
            .is_some_and(|(id, day)| id == index_id && *day == date_now)
    }

    fn push(&mut self, index_id: &str, date_now: i64, sample: ServerSpeedSample) {
        self.target = Some((index_id.to_string(), date_now));
        self.buffered.add(sample);
    }

    /// Totals as the database *would* report them once the buffer is flushed.
    fn projected(&self) -> Option<ServerStatItem> {
        let baseline = self.baseline.clone()?;

        Some(ServerStatItem {
            total_up: baseline
                .total_up
                .saturating_add(self.buffered.proxy_up_bytes),
            total_down: baseline
                .total_down
                .saturating_add(self.buffered.proxy_down_bytes),
            today_up: baseline
                .today_up
                .saturating_add(self.buffered.proxy_up_bytes),
            today_down: baseline
                .today_down
                .saturating_add(self.buffered.proxy_down_bytes),
            ..baseline
        })
    }
}

/// Writes whatever the buffer holds and adopts the row the database returns.
///
/// A buffer with no traffic is a no-op, so calling this on every profile
/// switch, day rollover and shutdown costs nothing when there is nothing to
/// save.
async fn flush_traffic_buffer(database: &Database, buffer: &mut TrafficWriteBuffer) -> Result<()> {
    let Some((index_id, date_now)) = buffer.target.clone() else {
        return Ok(());
    };
    if !buffer.buffered.has_traffic() {
        return Ok(());
    }

    let stat = add_traffic(database, &index_id, date_now, buffer.buffered).await?;
    buffer.buffered = ServerSpeedSample::default();
    buffer.baseline = Some(stat);

    Ok(())
}

/// Applies one coalesced tick.
///
/// Returns the snapshot the UI renders, which is unchanged in cadence and
/// content — the only difference is that most ticks no longer touch SQLite.
async fn record_statistics_tick(
    database: &Database,
    config: &StatisticsConfigSnapshot,
    buffer: &mut TrafficWriteBuffer,
    sample: ServerSpeedSample,
    date_now: i64,
    flush_due: bool,
) -> Result<StatisticsSnapshot> {
    let Some(index_id) = config.active_profile_id.clone() else {
        flush_traffic_buffer(database, buffer).await?;
        return Ok(snapshot_from_sample(config, sample, None));
    };
    if !buffer.targets(&index_id, date_now) {
        flush_traffic_buffer(database, buffer).await?;
        *buffer = TrafficWriteBuffer::default();
    }
    if sample.has_traffic() {
        buffer.push(&index_id, date_now, sample);
    }
    // The first write after a switch goes straight through: without a baseline
    // row there is nothing to project the running totals from, and the panel
    // would show no lifetime figure for the whole first flush window.
    if flush_due || buffer.baseline.is_none() {
        flush_traffic_buffer(database, buffer).await?;
    }

    Ok(snapshot_from_sample(config, sample, buffer.projected()))
}

impl From<ClashTraffic> for ServerSpeedSample {
    fn from(traffic: ClashTraffic) -> Self {
        Self {
            proxy_up_bytes: i64::try_from(traffic.up).unwrap_or(i64::MAX),
            proxy_down_bytes: i64::try_from(traffic.down).unwrap_or(i64::MAX),
            direct_up_bytes: 0,
            direct_down_bytes: 0,
        }
    }
}

#[must_use]
fn current_day_marker() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs() / 86_400).unwrap_or(i64::MAX)
        })
}

async fn run_statistics_aggregator(
    database: Database,
    config: SharedAppConfig,
    event_sink: Arc<dyn StatisticsEventSink>,
    mut sample_rx: mpsc::Receiver<ServerSpeedSample>,
    mut shutdown: watch::Receiver<bool>,
) {
    if let Err(error) = StatisticsManager::initialize_data(&database, current_day_marker()).await {
        tracing::warn!(?error, "failed to initialize server statistics");
    }

    let mut interval = time::interval(COALESCE_INTERVAL);
    let mut pending = ServerSpeedSample::default();
    let mut emitted_traffic = false;
    let mut day_marker = current_day_marker();
    let mut buffer = TrafficWriteBuffer::default();
    let mut ticks_since_flush = 0_u64;

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
                let config_snapshot = statistics_config(&config);
                let current_day = current_day_marker();
                if current_day != day_marker {
                    // Buffered bytes were measured yesterday, and the rollover
                    // is about to zero today's counters for every row.
                    if let Err(error) = flush_traffic_buffer(&database, &mut buffer).await {
                        tracing::warn!(?error, "failed to flush statistics before day rollover");
                    }
                }
                day_marker = roll_over_statistics_day(&database, day_marker, current_day).await;
                ticks_since_flush = ticks_since_flush.saturating_add(1);
                let flush_due = ticks_since_flush >= TRAFFIC_FLUSH_TICKS;
                if flush_due {
                    ticks_since_flush = 0;
                }
                match record_statistics_tick(
                    &database,
                    &config_snapshot,
                    &mut buffer,
                    sample,
                    day_marker,
                    flush_due,
                )
                .await
                {
                    Ok(snapshot) => {
                        if should_emit_statistics(sample.has_traffic(), emitted_traffic) {
                            emitted_traffic = sample.has_traffic();
                            event_sink.emit_statistics(snapshot);
                        }
                    }
                    Err(error) => tracing::warn!(?error, "failed to apply statistics sample"),
                }
            }
        }
    }

    // Best effort: `close()` only signals, so the process may still exit before
    // this lands — but on the graceful path it saves up to a flush window of
    // traffic that batching would otherwise have discarded.
    if let Err(error) = flush_traffic_buffer(&database, &mut buffer).await {
        tracing::warn!(?error, "failed to flush buffered statistics on shutdown");
    }
}

async fn run_singbox_statistics_service(
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

        let snapshot = supervisor.status().await.ok();
        // The supervisor reports the port the running main config actually
        // listens on, and the bearer token that config demands. Recomputing the
        // port from the TUN setting is wrong on a pre-socks topology, where the
        // main process keeps `api2` and the pre-socks one takes `api2 + 1`:
        // statistics would then be read from the pre-socks process, which has
        // no per-node counters at all. The token exists only in the config that
        // launch generated, so it can only come from here.
        let access = snapshot
            .as_ref()
            .map(SupervisorSnapshot::clash_api_access)
            .unwrap_or_default();
        let Some(identity) = snapshot.and_then(core_process_identity) else {
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
        let Some(endpoint) = proxy_runtime_endpoint(&access) else {
            active_identity = None;
            reconnect_backoff.reset();
            tracing::debug!("skipping sing-box statistics because state port is unavailable");
            if sleep_or_shutdown(SINGBOX_RECONNECT_INITIAL_DELAY, &mut shutdown).await {
                break;
            }
            continue;
        };

        let client = ClashWebSocketClient::new(endpoint);
        match time::timeout(
            SINGBOX_WS_CONNECT_TIMEOUT,
            client.connect(ClashWebSocketResource::Traffic),
        )
        .await
        {
            Ok(Ok(mut session)) => loop {
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
                    message = time::timeout(COALESCE_INTERVAL, session.next_event()) => {
                        match message {
                            Ok(Ok(ClashWebSocketEvent::Traffic(traffic))) => {
                                reconnect_backoff.reset();
                                let sample = ServerSpeedSample::from(traffic);
                                let _ = sample_tx.send(sample).await;
                            }
                            Ok(Ok(ClashWebSocketEvent::Connections(_))) | Err(_) => {}
                            Ok(Err(error)) => {
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

/// Rolls every stored profile over when the calendar day changes.
///
/// `add_traffic` only rolls the row it touches, so without this every profile
/// other than the active one keeps showing yesterday's "today" totals until the
/// next launch runs `initialize_data`. Returns the marker to keep using.
async fn roll_over_statistics_day(database: &Database, previous: i64, current: i64) -> i64 {
    if previous != current {
        if let Err(error) = database.server_stats().reset_rollover(current).await {
            tracing::warn!(
                ?error,
                "failed to roll server statistics over to the new day"
            );
        }
    }

    current
}

async fn singbox_process_identity(supervisor: &CoreSupervisor) -> Option<CoreProcessIdentity> {
    supervisor
        .status()
        .await
        .ok()
        .and_then(core_process_identity)
}

/// Emits while traffic flows and once more when it stops, so the speed display
/// returns to 0 B/s instead of freezing on the last non-zero rate, without
/// pushing an event every second while the core sits idle.
const fn should_emit_statistics(has_traffic: bool, emitted_traffic: bool) -> bool {
    has_traffic || emitted_traffic
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
    main_pid: u32,
    pre_pid: Option<u32>,
}

fn core_process_identity(snapshot: SupervisorSnapshot) -> Option<CoreProcessIdentity> {
    let main_pid = snapshot.main_pid?;
    Some(CoreProcessIdentity {
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

/// A zero state port means the generated config exposes no Clash API, so both
/// the statistics service and the proxy monitor skip connecting instead of
/// dialling 127.0.0.1:0.
pub(crate) fn available_state_port(port: u16) -> Option<u16> {
    (port != 0).then_some(port)
}

#[cfg(test)]
mod tests {
    use crate::supervisor::SupervisorConnectionState;
    use voya_core::{InItem, ProfileItem, ProfileProtocol, ServerEndpoint, TunModeItem};

    use super::*;

    #[test]
    fn statistics_traffic_conversion_preserves_counts_and_saturates_overflow() {
        assert_eq!(
            ServerSpeedSample::from(ClashTraffic {
                up: 1234,
                down: 5678
            }),
            ServerSpeedSample {
                proxy_up_bytes: 1234,
                proxy_down_bytes: 5678,
                direct_up_bytes: 0,
                direct_down_bytes: 0,
            }
        );
        let overflow = ServerSpeedSample::from(ClashTraffic {
            up: u64::MAX,
            down: u64::MAX,
        });
        assert_eq!(overflow.proxy_up_bytes, i64::MAX);
        assert_eq!(overflow.proxy_down_bytes, i64::MAX);
    }

    #[test]
    fn statistics_config_snapshot_does_not_carry_a_state_port() {
        // The port used to be recomputed here from `tun_mode_item.enable_tun`,
        // which disagreed with the generated config on a pre-socks topology.
        // It now travels on the supervisor snapshot, so a config change no
        // longer has to restart the statistics loop to pick it up.
        let base = AppConfig {
            inbound: vec![InItem {
                local_port: 12000,
                ..InItem::default()
            }],
            tun_mode_item: TunModeItem {
                enable_tun: true,
                ..TunModeItem::default()
            },
            ..AppConfig::default()
        };
        let mut without_tun = base.clone();
        without_tun.tun_mode_item.enable_tun = false;

        assert_eq!(
            StatisticsConfigSnapshot::from_app_config(&base),
            StatisticsConfigSnapshot::from_app_config(&without_tun)
        );
    }

    #[test]
    fn statistics_core_process_identity_tracks_pid_changes() {
        let first = SupervisorSnapshot {
            connected_duration_ms: None,
            active_tun_backend: None,
            state: SupervisorConnectionState::Connected,
            active_profile_id: Some("profile-a".to_string()),
            active_group_id: None,
            main_pid: Some(100),
            pre_pid: None,
            clash_api_port: None,
            clash_api_secret: None,
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
            core_process_identity(first),
            core_process_identity(restarted)
        );
        assert_eq!(core_process_identity(disconnected), None);
    }

    #[test]
    fn statistics_state_port_zero_is_unavailable() {
        // A generated config with no Clash API reports port 0; dialling
        // 127.0.0.1:0 would connect to an arbitrary local listener.
        assert_eq!(available_state_port(0), None);
        assert_eq!(available_state_port(1), Some(1));
    }

    #[tokio::test]
    async fn statistics_record_tick_keys_persistence_to_active_server_and_sums_display() {
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
            active_profile_id: Some("active".to_string()),
        };

        let snapshot = record_statistics_tick(
            &database,
            &config,
            &mut TrafficWriteBuffer::default(),
            ServerSpeedSample {
                proxy_up_bytes: 1000,
                proxy_down_bytes: 2000,
                direct_up_bytes: 300,
                direct_down_bytes: 400,
            },
            10,
            true,
        )
        .await
        .expect("statistics test operation should succeed");

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
    async fn statistics_record_tick_reports_idle_ticks_without_touching_the_database() {
        let database = Database::connect_in_memory()
            .await
            .expect("statistics test operation should succeed");
        database
            .profiles()
            .upsert(&sample_profile("active"))
            .await
            .expect("statistics test operation should succeed");
        let config = StatisticsConfigSnapshot {
            active_profile_id: Some("active".to_string()),
        };

        let idle = ServerSpeedSample::default();
        let snapshot = record_statistics_tick(
            &database,
            &config,
            &mut TrafficWriteBuffer::default(),
            idle,
            10,
            true,
        )
        .await
        .expect("an idle tick still reports a zero snapshot");

        assert_eq!(snapshot.upload_bytes_per_second, 0.0);
        assert_eq!(snapshot.download_bytes_per_second, 0.0);
        assert_eq!(snapshot.proxy_download_bytes_per_second, 0.0);
        assert!(snapshot.server_stat.is_none());
        assert!(
            database
                .server_stats()
                .get("active")
                .await
                .expect("statistics test operation should succeed")
                .is_none(),
            "an idle sample must not write traffic rows"
        );
    }

    #[test]
    fn statistics_emit_once_more_after_traffic_stops() {
        assert!(should_emit_statistics(true, false));
        assert!(should_emit_statistics(true, true));
        assert!(
            should_emit_statistics(false, true),
            "the first idle tick must reset the displayed speed to zero"
        );
        assert!(
            !should_emit_statistics(false, false),
            "a quiet core must not push an event every second"
        );
    }

    #[tokio::test]
    async fn statistics_record_tick_rolls_today_at_date_boundary() {
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
            active_profile_id: Some("active".to_string()),
        };

        let snapshot = record_statistics_tick(
            &database,
            &config,
            &mut TrafficWriteBuffer::default(),
            ServerSpeedSample {
                proxy_up_bytes: 5,
                proxy_down_bytes: 7,
                direct_up_bytes: 11,
                direct_down_bytes: 13,
            },
            2,
            true,
        )
        .await
        .expect("statistics test operation should succeed");
        let stat = snapshot
            .server_stat
            .expect("statistics test operation should succeed");

        assert_eq!(stat.today_up, 5);
        assert_eq!(stat.today_down, 7);
        assert_eq!(stat.total_up, 105);
        assert_eq!(stat.total_down, 207);
        assert_eq!(stat.date_now, 2);
    }

    #[tokio::test]
    async fn statistics_day_change_rolls_over_every_profile_not_only_the_active_one() {
        let database = Database::connect_in_memory()
            .await
            .expect("statistics test operation should succeed");
        for index_id in ["active", "idle"] {
            database
                .profiles()
                .upsert(&sample_profile(index_id))
                .await
                .expect("statistics test operation should succeed");
            database
                .server_stats()
                .upsert(&ServerStatItem {
                    index_id: index_id.to_string(),
                    total_up: 100,
                    total_down: 200,
                    today_up: 90,
                    today_down: 180,
                    date_now: 1,
                })
                .await
                .expect("statistics test operation should succeed");
        }

        assert_eq!(roll_over_statistics_day(&database, 1, 1).await, 1);
        assert_eq!(
            stat_row(&database, "idle").await.today_up,
            90,
            "a tick inside the same day must not touch stored totals"
        );

        assert_eq!(roll_over_statistics_day(&database, 1, 2).await, 2);

        let idle = stat_row(&database, "idle").await;
        assert_eq!(idle.today_up, 0);
        assert_eq!(idle.today_down, 0);
        assert_eq!(idle.date_now, 2);
        assert_eq!(idle.total_up, 100, "lifetime totals survive the rollover");
        assert_eq!(stat_row(&database, "active").await.today_up, 0);
    }

    /// The per-second row rewrite was the remaining half of the statistics
    /// write amplification: the panel is refreshed every second, but SQLite
    /// only has to learn the totals in batches.
    #[tokio::test]
    async fn statistics_buffers_traffic_and_writes_it_once_per_flush_window() {
        let database = Database::connect_in_memory()
            .await
            .expect("statistics test operation should succeed");
        database
            .profiles()
            .upsert(&sample_profile("active"))
            .await
            .expect("statistics test operation should succeed");
        let config = enabled_config("active");
        let mut buffer = TrafficWriteBuffer::default();
        let sample = ServerSpeedSample {
            proxy_up_bytes: 10,
            proxy_down_bytes: 20,
            ..ServerSpeedSample::default()
        };

        // Tick 1 writes through to establish a baseline the UI can project from.
        let first = tick(&database, &config, &mut buffer, sample, false).await;
        assert_eq!(stat_row(&database, "active").await.total_up, 10);

        // Ticks 2..=4 stay in memory, but the UI keeps counting.
        for expected_total in [20, 30, 40] {
            let snapshot = tick(&database, &config, &mut buffer, sample, false).await;
            assert_eq!(
                snapshot
                    .server_stat
                    .as_ref()
                    .expect("a projected row")
                    .total_up,
                expected_total
            );
            assert_eq!(
                stat_row(&database, "active").await.total_up,
                10,
                "only the first tick may have reached SQLite"
            );
        }
        assert_eq!(
            first
                .server_stat
                .as_ref()
                .expect("a projected row")
                .total_up,
            10
        );

        tick(&database, &config, &mut buffer, sample, true).await;

        assert_eq!(stat_row(&database, "active").await.total_up, 50);
        assert_eq!(stat_row(&database, "active").await.total_down, 100);
    }

    /// A flush window's worth of traffic must not evaporate because the app
    /// closed between flushes.
    #[tokio::test]
    async fn statistics_shutdown_flush_saves_what_the_buffer_still_holds() {
        let database = Database::connect_in_memory()
            .await
            .expect("statistics test operation should succeed");
        database
            .profiles()
            .upsert(&sample_profile("active"))
            .await
            .expect("statistics test operation should succeed");
        let config = enabled_config("active");
        let mut buffer = TrafficWriteBuffer::default();
        let sample = ServerSpeedSample {
            proxy_up_bytes: 7,
            proxy_down_bytes: 11,
            ..ServerSpeedSample::default()
        };

        tick(&database, &config, &mut buffer, sample, false).await;
        tick(&database, &config, &mut buffer, sample, false).await;
        assert_eq!(stat_row(&database, "active").await.total_up, 7);

        flush_traffic_buffer(&database, &mut buffer)
            .await
            .expect("statistics test operation should succeed");

        assert_eq!(stat_row(&database, "active").await.total_up, 14);
        assert_eq!(stat_row(&database, "active").await.total_down, 22);
    }

    /// Buffered bytes belong to the profile they were measured on; a switch
    /// that did not flush first would credit them to the new profile.
    #[tokio::test]
    async fn statistics_profile_switch_flushes_the_previous_profile_first() {
        let database = Database::connect_in_memory()
            .await
            .expect("statistics test operation should succeed");
        for index_id in ["first", "second"] {
            database
                .profiles()
                .upsert(&sample_profile(index_id))
                .await
                .expect("statistics test operation should succeed");
        }
        let mut buffer = TrafficWriteBuffer::default();
        let sample = ServerSpeedSample {
            proxy_up_bytes: 5,
            ..ServerSpeedSample::default()
        };

        let first = enabled_config("first");
        tick(&database, &first, &mut buffer, sample, false).await;
        tick(&database, &first, &mut buffer, sample, false).await;

        let second = enabled_config("second");
        tick(&database, &second, &mut buffer, sample, false).await;

        assert_eq!(stat_row(&database, "first").await.total_up, 10);
        assert_eq!(stat_row(&database, "second").await.total_up, 5);
    }

    #[test]
    fn statistics_flush_window_spans_ten_coalesced_ticks() {
        assert_eq!(TRAFFIC_FLUSH_TICKS, 10);
    }

    fn enabled_config(active_profile_id: &str) -> StatisticsConfigSnapshot {
        StatisticsConfigSnapshot {
            active_profile_id: Some(active_profile_id.to_string()),
        }
    }

    async fn tick(
        database: &Database,
        config: &StatisticsConfigSnapshot,
        buffer: &mut TrafficWriteBuffer,
        sample: ServerSpeedSample,
        flush_due: bool,
    ) -> StatisticsSnapshot {
        record_statistics_tick(database, config, buffer, sample, 10, flush_due)
            .await
            .expect("statistics test operation should succeed")
    }

    async fn stat_row(database: &Database, index_id: &str) -> ServerStatItem {
        database
            .server_stats()
            .get(index_id)
            .await
            .expect("statistics test operation should succeed")
            .expect("statistics test operation should succeed")
    }

    fn sample_profile(index_id: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            remarks: index_id.to_string(),
            protocol: ProfileProtocol::Vmess {
                server: ServerEndpoint {
                    address: "example.test".to_string(),
                    port: 443,
                },
                uuid: String::new(),
                cipher: None,
            },
            ..ProfileItem::default()
        }
    }
}
