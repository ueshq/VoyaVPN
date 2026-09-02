//! Background scheduler that refreshes subscriptions carrying an auto-update
//! interval. Network fetches run outside the config mutation lock; only the
//! import/commit step holds it, reusing the manager's prepare-then-commit
//! flow and its compare-and-discard race protection.

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{sync::watch, task::JoinHandle, time};
use voya_core::{SubItem, SubMetadataItem, SubscriptionUpdateResult};
use voya_db::Database;
use voya_platform::coreinfo::TargetOs;

use crate::{
    config_mutation::ConfigMutationCoordinator,
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
    handle: JoinHandle<()>,
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

        Self { shutdown, handle }
    }

    pub fn close(&self) {
        let _ = self.shutdown.send(true);
    }
}

impl Drop for SubscriptionAutoUpdateScheduler {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        self.handle.abort();
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
                )
                .await;
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
        let Some(item) = subs.iter().find(|sub| sub.id == id) else {
            continue;
        };
        let entry = attempts.entry(id.clone()).or_default();
        entry.last_attempt_unix = unix_now_seconds();

        let outcome = run_single_update(database, coordinator, supervisor, target_os, item).await;
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
    let prepared = match SubscriptionManager::new(database)
        .prepare_subscription_update(
            &config_snapshot,
            Some(&item.id),
            connected,
            proxy_url.as_deref(),
        )
        .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            outcome.error = Some(error.to_string());
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

    let mut mutation = match coordinator.begin().await {
        Ok(mutation) => mutation,
        Err(error) => {
            outcome.error = Some(error.to_string());
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
                Ok(_) => outcome.result = Some(result),
                Err(error) => outcome.error = Some(error.to_string()),
            }
        }
        Err(error) => outcome.error = Some(error.to_string()),
    }

    outcome
}

fn unix_now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn backoff_schedule_doubles_and_saturates() {
        assert_eq!(failure_backoff_seconds(1), 300);
        assert_eq!(failure_backoff_seconds(2), 600);
        assert_eq!(failure_backoff_seconds(3), 1200);
        assert_eq!(failure_backoff_seconds(7), 19_200);
        assert_eq!(failure_backoff_seconds(100), 19_200);
    }
}
