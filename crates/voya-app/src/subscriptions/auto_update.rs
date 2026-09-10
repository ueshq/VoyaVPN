//! Background scheduler that refreshes subscriptions carrying an auto-update
//! interval. Network fetches run outside the config mutation lock; only the
//! import/commit step holds it, reusing the manager's prepare-then-commit
//! flow and its compare-and-discard race protection.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{sync::watch, task::JoinHandle, time};
use voya_core::{SubItem, SubMetadataItem, SubscriptionUpdateResult};
use voya_db::Database;
use voya_platform::coreinfo::TargetOs;

use crate::{
    config_mutation::ConfigMutationCoordinator,
    redaction::redact_urls,
    subscriptions::SubscriptionManager,
    supervisor::{CoreSupervisor, SupervisorConnectionState},
    sysproxy::runtime_default_proxy_url,
};

const TICK_INTERVAL: Duration = Duration::from_secs(60);
/// Base delay before retrying a subscription whose last automatic update
/// failed; doubles per consecutive failure, capped by the interval itself.
const FAILURE_BACKOFF_BASE_SECONDS: i64 = 300;
const FAILURE_BACKOFF_MAX_DOUBLINGS: u32 = 6;
/// Never re-attempt the same subscription faster than this, regardless of
/// how short its configured interval is.
const MIN_ATTEMPT_SPACING_SECONDS: i64 = 60;
/// Reported when `close()` interrupts an attempt. It is not a source failure,
/// but it is not a success either: the schedule must retry it next launch.
const SHUTDOWN_MESSAGE: &str = "subscription auto-update stopped for shutdown";

/// Result of one automatic update attempt, delivered to the host sink.
#[derive(Debug, Clone)]
pub struct AutoUpdateOutcome {
    pub subscription_id: String,
    pub remarks: String,
    /// `None` when the fetch failed before anything could be imported.
    pub result: Option<SubscriptionUpdateResult>,
    pub config_changed: bool,
    pub error: Option<String>,
    /// Consecutive failed attempts including this one; `0` on success. Lets
    /// the host throttle user-facing notices to the first failure.
    pub consecutive_failures: u32,
}

pub trait SubscriptionAutoUpdateSink: Send + Sync {
    fn update_completed(&self, outcome: AutoUpdateOutcome);
}

/// Per-subscription in-memory attempt bookkeeping for failure backoff.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttemptState {
    pub consecutive_failures: u32,
    pub last_attempt_unix: i64,
}

/// Pure due-set computation: a subscription is due when it is enabled, has a
/// positive interval, its last successful update is older than the interval
/// (or it never updated), and any failure backoff window has elapsed.
#[must_use]
pub fn due_subscription_ids(
    now_unix: i64,
    subs: &[SubItem],
    metadata: &[SubMetadataItem],
    attempts: &BTreeMap<String, AttemptState>,
) -> Vec<String> {
    let last_updates: BTreeMap<&str, i64> = metadata
        .iter()
        .filter_map(|item| {
            item.last_update_at
                .map(|at| (item.subscription_id.as_str(), at))
        })
        .collect();

    subs.iter()
        .filter(|sub| {
            let Some(interval_minutes) = sub.auto_update_interval_minutes else {
                return false;
            };
            if interval_minutes <= 0 || !sub.enabled || sub.url.trim().is_empty() {
                return false;
            }
            let interval_seconds = i64::from(interval_minutes).saturating_mul(60);
            let due_by_age = last_updates
                .get(sub.id.as_str())
                .is_none_or(|last| now_unix.saturating_sub(*last) >= interval_seconds);
            if !due_by_age {
                return false;
            }

            match attempts.get(&sub.id) {
                Some(state) => {
                    let spacing = if state.consecutive_failures > 0 {
                        failure_backoff_seconds(state.consecutive_failures).min(interval_seconds)
                    } else {
                        MIN_ATTEMPT_SPACING_SECONDS
                    };
                    now_unix.saturating_sub(state.last_attempt_unix)
                        >= spacing.max(MIN_ATTEMPT_SPACING_SECONDS)
                }
                None => true,
            }
        })
        .map(|sub| sub.id.clone())
        .collect()
}

fn failure_backoff_seconds(consecutive_failures: u32) -> i64 {
    let doublings = consecutive_failures
        .saturating_sub(1)
        .min(FAILURE_BACKOFF_MAX_DOUBLINGS);
    FAILURE_BACKOFF_BASE_SECONDS.saturating_mul(1_i64 << doublings)
}

pub struct SubscriptionAutoUpdateScheduler {
    shutdown: watch::Sender<bool>,
    /// Taken by [`Self::shutdown`], so the `Drop` below does not then abort a
    /// task that already stopped on its own. Behind a lock because the shell
    /// holds the scheduler by shared reference.
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl SubscriptionAutoUpdateScheduler {
    #[must_use]
    pub fn spawn(
        database: Database,
        coordinator: Arc<ConfigMutationCoordinator>,
        supervisor: CoreSupervisor,
        target_os: TargetOs,
        sink: Arc<dyn SubscriptionAutoUpdateSink>,
    ) -> Self {
        let (shutdown, shutdown_rx) = watch::channel(false);
        let handle = tokio::spawn(run_scheduler(
            database,
            coordinator,
            supervisor,
            target_os,
            sink,
            shutdown_rx,
        ));

        Self {
            shutdown,
            handle: Mutex::new(Some(handle)),
        }
    }

    /// Requests shutdown without waiting for the loop to stop.
    ///
    /// An in-flight run abandons its download, skips the subscriptions it has
    /// not started, and discards a fetch that finished after the request rather
    /// than committing a configuration into a runtime that is being torn down.
    /// A commit already in progress still runs to completion — SQLite keeps it
    /// atomic — and dropping the scheduler aborts the task outright.
    ///
    /// Prefer [`Self::shutdown`] on the exit path: Tauri ends the process with
    /// `std::process::exit`, so nothing here is ever dropped, and a commit that
    /// is mid-flight when the process goes loses the update it just downloaded.
    pub fn close(&self) {
        let _ = self.shutdown.send(true);
    }

    /// Requests shutdown and waits for the loop to actually stop.
    ///
    /// Unlike [`Self::close`] this is safe to call immediately before the
    /// process exits: it returns once the scheduler has left its loop, so an
    /// update it had already committed is not raced by the exit.
    pub async fn shutdown(&self) {
        let _ = self.shutdown.send(true);
        // The guard is dropped before the await: holding a std lock across one
        // would be a deadlock waiting to happen.
        let handle = self.handle.lock().ok().and_then(|mut slot| slot.take());
        if let Some(handle) = handle {
            // A `JoinError` means the loop panicked or was aborted; either way
            // it is no longer running, which is all this call promises.
            let _ = handle.await;
        }
    }
}

impl Drop for SubscriptionAutoUpdateScheduler {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Ok(slot) = self.handle.lock() {
            if let Some(handle) = slot.as_ref() {
                handle.abort();
            }
        }
    }
}

async fn run_scheduler(
    database: Database,
    coordinator: Arc<ConfigMutationCoordinator>,
    supervisor: CoreSupervisor,
    target_os: TargetOs,
    sink: Arc<dyn SubscriptionAutoUpdateSink>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut interval = time::interval(TICK_INTERVAL);
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    // The first tick fires immediately; consume it so updates start one full
    // interval after launch instead of racing app startup.
    interval.tick().await;
    let mut attempts: BTreeMap<String, AttemptState> = BTreeMap::new();
    // A second handle on the same signal: the `select!` below borrows
    // `shutdown` for the whole statement, so the run itself needs its own.
    let mut run_shutdown = shutdown.clone();

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = interval.tick() => {
                run_due_updates(
                    &database,
                    &coordinator,
                    &supervisor,
                    target_os,
                    sink.as_ref(),
                    &mut attempts,
                    &mut run_shutdown,
                )
                .await;
                // A run interrupted by shutdown consumes the notification the
                // arm above waits on, so leave from here instead.
                if *run_shutdown.borrow() {
                    break;
                }
            }
        }
    }
}

async fn run_due_updates(
    database: &Database,
    coordinator: &ConfigMutationCoordinator,
    supervisor: &CoreSupervisor,
    target_os: TargetOs,
    sink: &dyn SubscriptionAutoUpdateSink,
    attempts: &mut BTreeMap<String, AttemptState>,
    shutdown: &mut watch::Receiver<bool>,
) {
    let now = unix_now_seconds();
    let (subs, metadata) = match (
        database.subscriptions().list().await,
        database.subscription_metadata().list().await,
    ) {
        (Ok(subs), Ok(metadata)) => (subs, metadata),
        (Err(error), _) | (_, Err(error)) => {
            tracing::warn!(?error, "subscription auto-update failed to read state");
            return;
        }
    };
    attempts.retain(|id, _| subs.iter().any(|sub| &sub.id == id));

    for id in due_subscription_ids(now, &subs, &metadata, attempts) {
        // `close()` is called during app shutdown, immediately before the
        // runtime is torn down; starting another fetch/commit cycle here would
        // publish a configuration into a runtime that is already going away.
        if *shutdown.borrow() {
            break;
        }
        let Some(item) = subs.iter().find(|sub| sub.id == id) else {
            continue;
        };
        let entry = attempts.entry(id.clone()).or_default();
        entry.last_attempt_unix = unix_now_seconds();

        let outcome =
            run_single_update(database, coordinator, supervisor, target_os, item, shutdown).await;
        let entry = attempts.entry(id).or_default();
        if outcome.error.is_none() {
            entry.consecutive_failures = 0;
        } else {
            entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
        }
        sink.update_completed(AutoUpdateOutcome {
            consecutive_failures: entry.consecutive_failures,
            ..outcome
        });
    }
}

async fn run_single_update(
    database: &Database,
    coordinator: &ConfigMutationCoordinator,
    supervisor: &CoreSupervisor,
    target_os: TargetOs,
    item: &SubItem,
    shutdown: &mut watch::Receiver<bool>,
) -> AutoUpdateOutcome {
    let mut outcome = AutoUpdateOutcome {
        subscription_id: item.id.clone(),
        remarks: item.remarks.clone(),
        result: None,
        config_changed: false,
        error: None,
        consecutive_failures: 0,
    };

    // Snapshot + fetch happen outside the mutation lock (network I/O).
    let config_snapshot = coordinator.current_config();
    let connected = matches!(
        supervisor.status().await.map(|snapshot| snapshot.state),
        Ok(SupervisorConnectionState::Connected)
    );
    let proxy_url = if connected {
        runtime_default_proxy_url(&config_snapshot, target_os)
    } else {
        None
    };
    // A download runs for as long as its timeout allows, so shutdown has to be
    // able to abandon it rather than only being noticed between subscriptions.
    let manager = SubscriptionManager::new(database);
    let fetched = tokio::select! {
        biased;
        _ = shutdown.changed() => {
            outcome.error = Some(SHUTDOWN_MESSAGE.to_string());
            return outcome;
        }
        fetched = manager.prepare_subscription_update(
            &config_snapshot,
            Some(&item.id),
            connected,
            proxy_url.as_deref(),
        ) => fetched,
    };
    let prepared = match fetched {
        Ok(prepared) => prepared,
        Err(error) => {
            outcome.error = Some(redact_urls(&error.to_string()));
            return outcome;
        }
    };
    if !prepared.has_imports() {
        let result = prepared.into_result();
        outcome.error =
            Some(
                result.messages.last().cloned().unwrap_or_else(|| {
                    "subscription fetch returned nothing importable".to_string()
                }),
            );
        outcome.result = Some(result);
        return outcome;
    }

    // Committing here would race the runtime teardown that follows `close()`,
    // so a fetch that finished after the request is discarded instead.
    if *shutdown.borrow() {
        outcome.error = Some(SHUTDOWN_MESSAGE.to_string());
        return outcome;
    }
    let mut mutation = match coordinator.begin().await {
        Ok(mutation) => mutation,
        Err(error) => {
            outcome.error = Some(redact_urls(&error.to_string()));
            return outcome;
        }
    };
    let before = mutation.config().clone();
    let applied = {
        let (unit_of_work, config) = mutation.split();
        SubscriptionManager::new_in(unit_of_work)
            .apply_prepared_subscription_update(config, prepared)
            .await
    };
    match applied {
        Ok(result) => {
            outcome.config_changed = before != *mutation.config();
            match mutation.commit().await {
                Ok(_) => {
                    outcome.error = unusable_update_message(&result);
                    outcome.result = Some(result);
                }
                Err(error) => outcome.error = Some(redact_urls(&error.to_string())),
            }
        }
        Err(error) => outcome.error = Some(redact_urls(&error.to_string())),
    }

    outcome
}

/// A committed update that imported nothing is not a success: the source
/// answered with something unusable (an expired-plan page, a login redirect, a
/// filter that matches no node). Reporting it as a failure feeds the backoff
/// and lets the host warn the user instead of silently resetting the schedule.
fn unusable_update_message(result: &SubscriptionUpdateResult) -> Option<String> {
    if result.updated > 0 || result.skipped == 0 {
        return None;
    }

    Some(
        result
            .messages
            .last()
            .cloned()
            .unwrap_or_else(|| "subscription update imported nothing".to_string()),
    )
}

fn unix_now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, RwLock};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use voya_core::AppConfig;
    use voya_platform::{privilege::ElevationState, test_support::RecordingRunner};

    use crate::supervisor::SupervisorDeps;

    use super::*;

    #[derive(Default)]
    struct RecordingSink {
        outcomes: Mutex<Vec<AutoUpdateOutcome>>,
    }

    impl RecordingSink {
        fn outcomes(&self) -> Vec<AutoUpdateOutcome> {
            match self.outcomes.lock() {
                Ok(outcomes) => outcomes.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        }
    }

    impl SubscriptionAutoUpdateSink for RecordingSink {
        fn update_completed(&self, outcome: AutoUpdateOutcome) {
            match self.outcomes.lock() {
                Ok(mut outcomes) => outcomes.push(outcome),
                Err(poisoned) => poisoned.into_inner().push(outcome),
            }
        }
    }

    /// Serves `body` once on `/sub` and then stops accepting.
    async fn spawn_subscription_fixture(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("auto-update test operation should succeed");
        let address = listener
            .local_addr()
            .expect("auto-update test operation should succeed");

        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = vec![0; 4096];
            let _ = socket.read(&mut buffer).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });

        format!("http://{address}/sub")
    }

    fn scheduler_supervisor() -> CoreSupervisor {
        CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(RecordingRunner::default()),
            Arc::new(ElevationState::new()),
        ))
    }

    async fn scheduler_database(url: String) -> Database {
        let database = Database::connect_in_memory()
            .await
            .expect("auto-update test operation should succeed");
        database
            .subscriptions()
            .upsert(&SubItem {
                id: "junk".to_string(),
                remarks: "Junk".to_string(),
                url,
                enabled: true,
                auto_update_interval_minutes: Some(60),
                ..SubItem::default()
            })
            .await
            .expect("auto-update test operation should succeed");

        database
    }

    /// An expired plan or a login redirect answers 200 with something that
    /// parses to no profile. Counting that as success would reset the backoff
    /// and move `last_update_at` forward, hiding a dead source forever.
    #[tokio::test]
    async fn a_response_with_nothing_importable_counts_as_a_failed_attempt() {
        let url = spawn_subscription_fixture("<html>your plan has expired</html>").await;
        let database = scheduler_database(url).await;
        let coordinator = ConfigMutationCoordinator::new(
            database.clone(),
            Arc::new(RwLock::new(AppConfig::default())),
        );
        let sink = RecordingSink::default();
        let mut attempts = BTreeMap::new();
        let (_shutdown, mut shutdown_rx) = watch::channel(false);

        run_due_updates(
            &database,
            &coordinator,
            &scheduler_supervisor(),
            TargetOs::Linux,
            &sink,
            &mut attempts,
            &mut shutdown_rx,
        )
        .await;

        let outcomes = sink.outcomes();
        assert_eq!(outcomes.len(), 1, "{outcomes:?}");
        assert!(outcomes[0].error.is_some(), "{:?}", outcomes[0]);
        assert_eq!(outcomes[0].consecutive_failures, 1);
        assert_eq!(
            attempts.get("junk").map(|state| state.consecutive_failures),
            Some(1)
        );
        assert_eq!(
            database
                .subscription_metadata()
                .get("junk")
                .await
                .expect("auto-update test operation should succeed")
                .and_then(|metadata| metadata.last_update_at),
            None,
            "a junk response must not mark the subscription current"
        );
    }

    /// `close()` runs immediately before the runtime is disconnected, so a
    /// requested shutdown must stop the scheduler from starting more work.
    #[tokio::test]
    async fn a_requested_shutdown_stops_the_run_before_it_fetches_anything() {
        let url = spawn_subscription_fixture("vless://uuid@example.test:443#Node").await;
        let database = scheduler_database(url).await;
        let coordinator = ConfigMutationCoordinator::new(
            database.clone(),
            Arc::new(RwLock::new(AppConfig::default())),
        );
        let sink = RecordingSink::default();
        let mut attempts = BTreeMap::new();
        let (shutdown, mut shutdown_rx) = watch::channel(false);
        shutdown
            .send(true)
            .expect("auto-update test operation should succeed");

        run_due_updates(
            &database,
            &coordinator,
            &scheduler_supervisor(),
            TargetOs::Linux,
            &sink,
            &mut attempts,
            &mut shutdown_rx,
        )
        .await;

        assert!(sink.outcomes().is_empty());
        assert!(attempts.is_empty());
        assert!(database
            .profiles()
            .list()
            .await
            .expect("auto-update test operation should succeed")
            .is_empty());
    }

    fn sub(id: &str, interval_minutes: Option<i32>, enabled: bool) -> SubItem {
        SubItem {
            id: id.to_string(),
            remarks: id.to_string(),
            url: "https://example.test/sub".to_string(),
            enabled,
            auto_update_interval_minutes: interval_minutes,
            ..SubItem::default()
        }
    }

    fn meta(id: &str, last_update_at: Option<i64>) -> SubMetadataItem {
        SubMetadataItem {
            subscription_id: id.to_string(),
            last_update_at,
            ..SubMetadataItem::default()
        }
    }

    /// Every `AutoUpdateOutcome::error` reaches the user as a toast and a Logs
    /// panel line, so the subscription URL (which carries the account token)
    /// must never survive into it.
    #[test]
    fn outcome_errors_never_carry_the_subscription_url() {
        let error = crate::subscriptions::SubscriptionManagerError::Download(
            voya_net::DownloadError::AttemptsFailed {
                url: "https://sub.example.test/link?token=abc123".to_string(),
                attempts: vec![voya_net::DownloadAttempt {
                    url: "https://sub.example.test/link?token=abc123".to_string(),
                    via_proxy: false,
                    bytes: 0,
                    error: Some("connection refused".to_string()),
                }],
            },
        );

        let reported = redact_urls(&error.to_string());

        assert!(!reported.contains("token=abc123"));
        assert!(!reported.contains("sub.example.test"));
        assert!(reported.contains("all download attempts failed for [redacted URL]"));
    }

    #[test]
    fn due_requires_enabled_positive_interval_and_elapsed_age() {
        let now = 100_000;
        let subs = vec![
            sub("due-never-updated", Some(60), true),
            sub("due-stale", Some(60), true),
            sub("fresh", Some(60), true),
            sub("disabled", Some(60), false),
            sub("no-interval", None, true),
            sub("zero-interval", Some(0), true),
        ];
        let metadata = vec![
            meta("due-stale", Some(now - 3600)),
            meta("fresh", Some(now - 3599)),
        ];

        assert_eq!(
            due_subscription_ids(now, &subs, &metadata, &BTreeMap::new()),
            vec!["due-never-updated".to_string(), "due-stale".to_string()]
        );
    }

    #[test]
    fn failure_backoff_delays_retries_exponentially_but_caps_at_interval() {
        let now = 100_000;
        let subs = vec![sub("failing", Some(60), true)];
        let metadata = Vec::new();

        let mut attempts = BTreeMap::new();
        attempts.insert(
            "failing".to_string(),
            AttemptState {
                consecutive_failures: 1,
                last_attempt_unix: now - 299,
            },
        );
        assert!(
            due_subscription_ids(now, &subs, &metadata, &attempts).is_empty(),
            "first failure should back off for 300s"
        );

        attempts.insert(
            "failing".to_string(),
            AttemptState {
                consecutive_failures: 1,
                last_attempt_unix: now - 300,
            },
        );
        assert_eq!(
            due_subscription_ids(now, &subs, &metadata, &attempts).len(),
            1
        );

        // Many failures: backoff is capped at the configured interval (3600s).
        attempts.insert(
            "failing".to_string(),
            AttemptState {
                consecutive_failures: 10,
                last_attempt_unix: now - 3600,
            },
        );
        assert_eq!(
            due_subscription_ids(now, &subs, &metadata, &attempts).len(),
            1
        );
    }

    #[test]
    fn successful_attempts_still_respect_minimum_spacing() {
        let now = 100_000;
        let subs = vec![sub("rapid", Some(1), true)];
        let mut attempts = BTreeMap::new();
        attempts.insert(
            "rapid".to_string(),
            AttemptState {
                consecutive_failures: 0,
                last_attempt_unix: now - 30,
            },
        );

        assert!(due_subscription_ids(now, &subs, &Vec::new(), &attempts).is_empty());
    }

    #[test]
    fn committed_updates_that_imported_nothing_count_as_failures() {
        let imported = SubscriptionUpdateResult {
            updated: 1,
            skipped: 1,
            messages: vec!["Other->no nodes were imported".to_string()],
            ..SubscriptionUpdateResult::default()
        };
        assert_eq!(unusable_update_message(&imported), None);

        let nothing_usable = SubscriptionUpdateResult {
            updated: 0,
            skipped: 1,
            messages: vec!["Plan->no importable nodes were found".to_string()],
            ..SubscriptionUpdateResult::default()
        };
        assert_eq!(
            unusable_update_message(&nothing_usable).as_deref(),
            Some("Plan->no importable nodes were found")
        );

        let nothing_attempted = SubscriptionUpdateResult::default();
        assert_eq!(unusable_update_message(&nothing_attempted), None);
    }

    #[test]
    fn backoff_schedule_doubles_and_saturates() {
        assert_eq!(failure_backoff_seconds(1), 300);
        assert_eq!(failure_backoff_seconds(2), 600);
        assert_eq!(failure_backoff_seconds(3), 1200);
        assert_eq!(failure_backoff_seconds(7), 19_200);
        assert_eq!(failure_backoff_seconds(100), 19_200);
    }

    /// The exit path calls `shutdown()` immediately before Tauri ends the
    /// process, so it has to actually wait. `close()` only signals, which is
    /// why the two are separate.
    #[tokio::test]
    async fn shutdown_waits_for_the_loop_to_leave_while_close_only_signals() {
        let url = spawn_subscription_fixture("vless://uuid@example.test:443#Node").await;
        let database = scheduler_database(url).await;
        let coordinator = ConfigMutationCoordinator::new(
            database.clone(),
            Arc::new(RwLock::new(AppConfig::default())),
        );
        let scheduler = SubscriptionAutoUpdateScheduler::spawn(
            database,
            Arc::new(coordinator),
            scheduler_supervisor(),
            TargetOs::Linux,
            Arc::new(RecordingSink::default()),
        );

        // `close()` returns before the task has necessarily stopped, so the
        // handle is still there to be awaited.
        scheduler.close();
        assert!(
            scheduler
                .handle
                .lock()
                .expect("auto-update test operation should succeed")
                .is_some(),
            "close() must not consume the join handle"
        );

        scheduler.shutdown().await;

        assert!(
            scheduler
                .handle
                .lock()
                .expect("auto-update test operation should succeed")
                .is_none(),
            "shutdown() must have joined the loop"
        );
        // Idempotent: the exit path is latched, but a double call must not hang.
        scheduler.shutdown().await;
    }
}
