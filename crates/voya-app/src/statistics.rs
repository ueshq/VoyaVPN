use std::{
    future::Future,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time,
};
use voya_contracts::StatisticsSnapshot;
use voya_core::ServerStatItem;
use voya_db::{Database, DbError};
use voya_net::clash::{ClashTraffic, ClashWebSocketEvent, ClashWebSocketResource};

use crate::{
    backoff::sleep_or_shutdown,
    clash_follow::follow_core_ws,
    proxy_runtime::ProxyRuntimeManager,
    shutdown_task::ShutdownTask,
    supervisor::{ClashApiAccess, CoreSupervisor, SupervisorSnapshot},
};

mod target;
use target::TrafficTarget;

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
const SINGBOX_INITIAL_DELAY: Duration = Duration::from_secs(5);
/// The supervisor answers from its own thread, which a core start or stop
/// occupies for seconds; a tick does not wait that long to learn the target.
const RUNNING_CORE_TIMEOUT: Duration = Duration::from_secs(1);

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
}

impl ServerSpeedSample {
    #[must_use]
    const fn has_traffic(self) -> bool {
        self.proxy_up_bytes > 0 || self.proxy_down_bytes > 0
    }

    fn add(&mut self, sample: Self) {
        self.proxy_up_bytes = self
            .proxy_up_bytes
            .saturating_add(sample.proxy_up_bytes.max(0));
        self.proxy_down_bytes = self
            .proxy_down_bytes
            .saturating_add(sample.proxy_down_bytes.max(0));
    }
}

/// The zero rates the UI shows once the core stops.
#[must_use]
pub fn zero_statistics_snapshot() -> StatisticsSnapshot {
    StatisticsSnapshot {
        active_profile_id: None,
        upload_bytes_per_second: 0.0,
        download_bytes_per_second: 0.0,
        server_stat: None,
    }
}

pub trait StatisticsEventSink: Send + Sync {
    fn emit_statistics(&self, snapshot: StatisticsSnapshot);
}

pub struct StatisticsManager {
    /// Writes the samples to SQLite; `shutdown` waits for its final flush.
    aggregator: ShutdownTask,
    /// Reads the core's traffic stream; holds nothing worth waiting for.
    collector: JoinHandle<()>,
}

impl StatisticsManager {
    pub fn spawn(
        database: Database,
        supervisor: CoreSupervisor,
        proxy_runtime: ProxyRuntimeManager,
        event_sink: Arc<dyn StatisticsEventSink>,
    ) -> Self {
        let (sample_tx, sample_rx) = mpsc::channel(STATISTICS_CHANNEL_SIZE);

        let aggregator = ShutdownTask::spawn(|shutdown| {
            run_statistics_aggregator(
                database,
                supervisor.clone(),
                proxy_runtime,
                event_sink,
                sample_rx,
                shutdown,
            )
        });
        let collector = tokio::spawn(run_singbox_statistics_service(
            supervisor,
            sample_tx,
            aggregator.subscribe(),
        ));

        Self {
            aggregator,
            collector,
        }
    }

    /// Stops both loops and returns once the aggregator has written the
    /// traffic it still buffers. Tauri ends the process with
    /// `std::process::exit`, so a flush that was only signalled would be lost.
    pub async fn shutdown(&self) {
        // It may sit in a websocket connect for seconds; there is nothing to
        // save there.
        self.collector.abort();
        self.aggregator.shutdown().await;
    }

    async fn initialize_data(database: &Database, date_now: i64) -> Result<()> {
        database.server_stats().reset_rollover(date_now).await?;

        Ok(())
    }
}

impl Drop for StatisticsManager {
    fn drop(&mut self) {
        self.collector.abort();
    }
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
    /// The immediate write that fetches `baseline` failed. Until a scheduled
    /// flush succeeds the buffer keeps to the flush cadence, so a database
    /// that refuses writes is asked every flush window and not every tick.
    baseline_write_failed: bool,
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

    let stat = database
        .server_stats()
        .add_traffic(
            &index_id,
            date_now,
            buffer.buffered.proxy_up_bytes,
            buffer.buffered.proxy_down_bytes,
        )
        .await?;
    buffer.buffered = ServerSpeedSample::default();
    // `None`: the node was deleted with bytes still buffered for it. They have
    // no row to go to, and keeping them would repeat the write every flush.
    buffer.baseline = stat;

    Ok(())
}

/// Applies one coalesced tick.
///
/// Returns the snapshot the UI renders, which is unchanged in cadence and
/// content — the only difference is that most ticks no longer touch SQLite.
async fn record_statistics_tick(
    database: &Database,
    active_profile_id: Option<&str>,
    buffer: &mut TrafficWriteBuffer,
    sample: ServerSpeedSample,
    date_now: i64,
    flush_due: bool,
) -> Result<StatisticsSnapshot> {
    let Some(index_id) = active_profile_id else {
        flush_traffic_buffer(database, buffer).await?;
        return Ok(snapshot_from_sample(None, sample, None));
    };
    if !buffer.targets(index_id, date_now) {
        // The buffer moves on to the new row whether or not the old one could
        // be written: holding on to it would pin every later tick to a profile
        // or a day that is no longer current.
        if let Err(error) = flush_traffic_buffer(database, buffer).await {
            tracing::warn!(?error, "failed to flush statistics for the previous target");
        }
        *buffer = TrafficWriteBuffer::default();
    }
    if sample.has_traffic() {
        buffer.push(index_id, date_now, sample);
    }
    // The first write after a switch goes straight through: without a baseline
    // row there is nothing to project the running totals from, and the panel
    // would show no lifetime figure for the whole first flush window.
    if flush_due || (buffer.baseline.is_none() && !buffer.baseline_write_failed) {
        let flushed = flush_traffic_buffer(database, buffer).await;
        buffer.baseline_write_failed = flushed.is_err();
        flushed?;
    }

    Ok(snapshot_from_sample(
        active_profile_id,
        sample,
        buffer.projected(),
    ))
}

impl From<ClashTraffic> for ServerSpeedSample {
    fn from(traffic: ClashTraffic) -> Self {
        Self {
            proxy_up_bytes: i64::try_from(traffic.up).unwrap_or(i64::MAX),
            proxy_down_bytes: i64::try_from(traffic.down).unwrap_or(i64::MAX),
        }
    }
}

/// The day "today's" counters belong to: the local calendar date, so they
/// reset at the user's midnight. The UTC day stands in where the platform
/// cannot say what the local date is.
#[must_use]
fn current_day_marker() -> i64 {
    let unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        });
    voya_platform::localtime::local_day_number(unix_seconds).unwrap_or(unix_seconds / 86_400)
}

async fn run_statistics_aggregator(
    database: Database,
    supervisor: CoreSupervisor,
    proxy_runtime: ProxyRuntimeManager,
    event_sink: Arc<dyn StatisticsEventSink>,
    mut sample_rx: mpsc::Receiver<ServerSpeedSample>,
    mut shutdown: watch::Receiver<bool>,
) {
    if let Err(error) = StatisticsManager::initialize_data(&database, current_day_marker()).await {
        tracing::warn!(?error, "failed to initialize server statistics");
    }

    let mut interval = time::interval(COALESCE_INTERVAL);
    // A tick that ran long is followed by the next one a full interval later.
    // The default bursts to catch up, and each catch-up tick would report the
    // 0 B/s of the instant it covers.
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    let mut pending = ServerSpeedSample::default();
    let mut emitted_traffic = false;
    let mut day_marker = current_day_marker();
    let mut buffer = TrafficWriteBuffer::default();
    let mut ticks_since_flush = 0_u64;
    let mut target = TrafficTarget::new(database.clone(), proxy_runtime);
    let mut running_core = RunningCore::new(supervisor.subscribe_clash_api());

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
                // Only measured bytes need a node to go to, so an idle core
                // costs the supervisor and the Clash API nothing.
                if sample.has_traffic() {
                    let has_target = target.current().is_some();
                    let snapshot = running_core
                        .snapshot(has_target, || read_running_core(&supervisor))
                        .await;
                    if let Some(snapshot) = snapshot {
                        target.follow(snapshot).await;
                    }
                }
                let current_day = current_day_marker();
                if current_day != day_marker {
                    roll_over_statistics_day(&database, &mut buffer, current_day).await;
                    day_marker = current_day;
                }
                ticks_since_flush = ticks_since_flush.saturating_add(1);
                let flush_due = ticks_since_flush >= TRAFFIC_FLUSH_TICKS;
                if flush_due {
                    ticks_since_flush = 0;
                }
                match record_statistics_tick(
                    &database,
                    target.current(),
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

    // `StatisticsManager::shutdown` waits for this, so on the graceful path it
    // saves up to a flush window of traffic that batching would otherwise have
    // discarded.
    if let Err(error) = flush_traffic_buffer(&database, &mut buffer).await {
        tracing::warn!(?error, "failed to flush buffered statistics on shutdown");
    }
}

/// The supervisor's last answer about the running core, asked for again only
/// when that core changed.
///
/// Asking costs a round trip through the supervisor's own thread, which a core
/// start or stop occupies for seconds, and it used to be paid on every tick
/// that carried traffic. What a snapshot says about the traffic's target moves
/// only with the core, and every start, stop and restart moves the Clash API
/// access: a start mints a new token and a stop clears it.
struct RunningCore {
    access: watch::Receiver<ClashApiAccess>,
    snapshot: Option<SupervisorSnapshot>,
}

impl RunningCore {
    const fn new(access: watch::Receiver<ClashApiAccess>) -> Self {
        Self {
            access,
            snapshot: None,
        }
    }

    /// The snapshot to follow, or `None` when the supervisor had to be asked
    /// and did not answer: the target then stays where it was, and the next
    /// tick asks again.
    ///
    /// Traffic that has no target yet always asks. The snapshot it would
    /// reuse was taken before the core finished connecting.
    async fn snapshot<Fut>(
        &mut self,
        has_target: bool,
        read: impl FnOnce() -> Fut,
    ) -> Option<&SupervisorSnapshot>
    where
        Fut: Future<Output = Option<SupervisorSnapshot>>,
    {
        // A closed channel means the supervisor is gone; asking says so.
        let moved = self.access.has_changed().unwrap_or(true);
        if moved || !has_target || self.snapshot.is_none() {
            // Marked seen before the read, so a core that changes during it
            // is asked about again.
            self.access.borrow_and_update();
            self.snapshot = read().await;
        }
        self.snapshot.as_ref()
    }
}

/// A supervisor that does not answer in time is not waited for.
async fn read_running_core(supervisor: &CoreSupervisor) -> Option<SupervisorSnapshot> {
    time::timeout(RUNNING_CORE_TIMEOUT, supervisor.status())
        .await
        .ok()?
        .ok()
}

async fn run_singbox_statistics_service(
    supervisor: CoreSupervisor,
    sample_tx: mpsc::Sender<ServerSpeedSample>,
    mut shutdown: watch::Receiver<bool>,
) {
    if sleep_or_shutdown(SINGBOX_INITIAL_DELAY, &mut shutdown).await {
        return;
    }

    // The supervisor reports the port the running main config actually listens
    // on, and the bearer token that config demands. Recomputing the port from
    // the TUN setting is wrong on a pre-socks topology, where the main process
    // keeps the base Clash API port and the pre-socks one takes that port + 1:
    // statistics would then be read from the pre-socks process, which has no
    // per-node counters at all. The token exists only in the config that launch
    // generated, so it can only come from here.
    follow_core_ws(
        supervisor.subscribe_clash_api(),
        ClashWebSocketResource::Traffic,
        shutdown,
        |event| {
            let sample_tx = sample_tx.clone();
            async move {
                if let ClashWebSocketEvent::Traffic(traffic) = event {
                    let _ = sample_tx.send(ServerSpeedSample::from(traffic)).await;
                }
            }
        },
    )
    .await;
}

/// Rolls every stored profile over to `current`, the day that has just begun.
///
/// A traffic write only rolls the row it touches, so without this every profile
/// other than the active one keeps showing yesterday's "today" totals until the
/// next launch runs `initialize_data`.
async fn roll_over_statistics_day(
    database: &Database,
    buffer: &mut TrafficWriteBuffer,
    current: i64,
) {
    // Buffered bytes were measured yesterday, and the rollover is about to
    // zero today's counters for every row.
    if let Err(error) = flush_traffic_buffer(database, buffer).await {
        tracing::warn!(?error, "failed to flush statistics before day rollover");
    }
    if let Err(error) = database.server_stats().reset_rollover(current).await {
        tracing::warn!(
            ?error,
            "failed to roll server statistics over to the new day"
        );
    }
}

/// Emits while traffic flows and once more when it stops, so the speed display
/// returns to 0 B/s instead of freezing on the last non-zero rate, without
/// pushing an event every second while the core sits idle.
const fn should_emit_statistics(has_traffic: bool, emitted_traffic: bool) -> bool {
    has_traffic || emitted_traffic
}

fn snapshot_from_sample(
    active_profile_id: Option<&str>,
    sample: ServerSpeedSample,
    server_stat: Option<ServerStatItem>,
) -> StatisticsSnapshot {
    StatisticsSnapshot {
        active_profile_id: active_profile_id.map(str::to_string),
        upload_bytes_per_second: sample.proxy_up_bytes.max(0) as f64,
        download_bytes_per_second: sample.proxy_down_bytes.max(0) as f64,
        server_stat: server_stat.map(crate::contract_map::server_stat_to_contract),
    }
}

#[cfg(test)]
mod tests {
    use voya_core::{ProfileItem, ProfileProtocol, ServerEndpoint};

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
            }
        );
        let overflow = ServerSpeedSample::from(ClashTraffic {
            up: u64::MAX,
            down: u64::MAX,
        });
        assert_eq!(overflow.proxy_up_bytes, i64::MAX);
        assert_eq!(overflow.proxy_down_bytes, i64::MAX);
    }

    #[tokio::test]
    async fn statistics_record_tick_keys_persistence_to_active_server() {
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
        let config = Some("active");

        let snapshot = record_statistics_tick(
            &database,
            config,
            &mut TrafficWriteBuffer::default(),
            ServerSpeedSample {
                proxy_up_bytes: 1000,
                proxy_down_bytes: 2000,
            },
            10,
            true,
        )
        .await
        .expect("statistics test operation should succeed");

        assert_eq!(snapshot.upload_bytes_per_second, 1000.0);
        assert_eq!(snapshot.download_bytes_per_second, 2000.0);
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
        let config = Some("active");

        let idle = ServerSpeedSample::default();
        let snapshot = record_statistics_tick(
            &database,
            config,
            &mut TrafficWriteBuffer::default(),
            idle,
            10,
            true,
        )
        .await
        .expect("an idle tick still reports a zero snapshot");

        assert_eq!(snapshot.upload_bytes_per_second, 0.0);
        assert_eq!(snapshot.download_bytes_per_second, 0.0);
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
        let config = Some("active");

        let snapshot = record_statistics_tick(
            &database,
            config,
            &mut TrafficWriteBuffer::default(),
            ServerSpeedSample {
                proxy_up_bytes: 5,
                proxy_down_bytes: 7,
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

        roll_over_statistics_day(&database, &mut TrafficWriteBuffer::default(), 2).await;

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
        let config = Some("active");
        let mut buffer = TrafficWriteBuffer::default();
        let sample = ServerSpeedSample {
            proxy_up_bytes: 10,
            proxy_down_bytes: 20,
        };

        // Tick 1 writes through to establish a baseline the UI can project from.
        let first = tick(&database, config, &mut buffer, sample, false).await;
        assert_eq!(stat_row(&database, "active").await.total_up, 10);

        // Ticks 2..=4 stay in memory, but the UI keeps counting.
        for expected_total in [20, 30, 40] {
            let snapshot = tick(&database, config, &mut buffer, sample, false).await;
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

        tick(&database, config, &mut buffer, sample, true).await;

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
        let config = Some("active");
        let mut buffer = TrafficWriteBuffer::default();
        let sample = ServerSpeedSample {
            proxy_up_bytes: 7,
            proxy_down_bytes: 11,
        };

        tick(&database, config, &mut buffer, sample, false).await;
        tick(&database, config, &mut buffer, sample, false).await;
        assert_eq!(stat_row(&database, "active").await.total_up, 7);

        flush_traffic_buffer(&database, &mut buffer)
            .await
            .expect("statistics test operation should succeed");

        assert_eq!(stat_row(&database, "active").await.total_up, 14);
        assert_eq!(stat_row(&database, "active").await.total_down, 22);
    }

    /// Deleting the connected node disconnects it with up to a flush window
    /// of its traffic still buffered. That traffic has no row left to go to;
    /// it must be dropped once, not retried — and warned about — every flush.
    #[tokio::test]
    async fn statistics_for_a_deleted_profile_are_dropped_without_an_error() {
        let database = Database::connect_in_memory()
            .await
            .expect("statistics test operation should succeed");
        database
            .profiles()
            .upsert(&sample_profile("active"))
            .await
            .expect("statistics test operation should succeed");
        let config = Some("active");
        let mut buffer = TrafficWriteBuffer::default();
        let sample = ServerSpeedSample {
            proxy_up_bytes: 7,
            proxy_down_bytes: 11,
        };
        tick(&database, config, &mut buffer, sample, false).await;
        tick(&database, config, &mut buffer, sample, false).await;
        database
            .profiles()
            .delete("active")
            .await
            .expect("statistics test operation should succeed");

        let snapshot = tick(
            &database,
            config,
            &mut buffer,
            ServerSpeedSample::default(),
            true,
        )
        .await;

        assert_eq!(snapshot.server_stat, None);
        assert!(!buffer.buffered.has_traffic());
        assert!(!buffer.baseline_write_failed);
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

        let first = Some("first");
        tick(&database, first, &mut buffer, sample, false).await;
        tick(&database, first, &mut buffer, sample, false).await;

        let second = Some("second");
        tick(&database, second, &mut buffer, sample, false).await;

        assert_eq!(stat_row(&database, "first").await.total_up, 10);
        assert_eq!(stat_row(&database, "second").await.total_up, 5);
    }

    /// The supervisor is asked once per core, not once per tick.
    #[tokio::test]
    async fn the_running_core_is_read_again_only_when_it_changes() {
        let (access_tx, access_rx) = watch::channel(ClashApiAccess::default());
        let mut running_core = RunningCore::new(access_rx);
        let reads = std::cell::Cell::new(0_u32);
        let read = || {
            reads.set(reads.get() + 1);
            async { Some(SupervisorSnapshot::disconnected()) }
        };

        assert!(running_core.snapshot(false, read).await.is_some());
        assert!(running_core.snapshot(true, read).await.is_some());
        assert!(running_core.snapshot(true, read).await.is_some());
        assert_eq!(reads.get(), 1, "an unchanged core was asked about again");

        // Traffic nobody is credited with keeps asking: the cached snapshot
        // was taken before the core finished connecting.
        assert!(running_core.snapshot(false, read).await.is_some());
        assert_eq!(reads.get(), 2);

        access_tx
            .send(ClashApiAccess::new(Some(10_814), None))
            .expect("the receiver is alive");
        assert!(running_core.snapshot(true, read).await.is_some());
        assert!(running_core.snapshot(true, read).await.is_some());
        assert_eq!(
            reads.get(),
            3,
            "a new core must be asked about exactly once"
        );

        // No answer leaves nothing to follow, and the next tick asks again.
        access_tx
            .send(ClashApiAccess::default())
            .expect("the receiver is alive");
        assert!(running_core
            .snapshot(true, || async { None })
            .await
            .is_none());
        assert!(running_core.snapshot(true, read).await.is_some());
        assert_eq!(reads.get(), 4);
    }

    #[test]
    fn statistics_flush_window_spans_ten_coalesced_ticks() {
        assert_eq!(TRAFFIC_FLUSH_TICKS, 10);
    }

    async fn tick(
        database: &Database,
        active_profile_id: Option<&str>,
        buffer: &mut TrafficWriteBuffer,
        sample: ServerSpeedSample,
        flush_due: bool,
    ) -> StatisticsSnapshot {
        record_statistics_tick(database, active_profile_id, buffer, sample, 10, flush_due)
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
