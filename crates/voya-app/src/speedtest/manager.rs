use super::core_backend::{cleanup_stale_speedtest_configs, reserve_speedtest_ports};

use super::*;

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
    pub(super) fn with_probe_and_backend(
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
        index_ids: Vec<String>,
    ) -> Result<SpeedtestRunResult> {
        self.run_with_callback(database, config, index_ids, |_| {})
            .await
    }

    pub async fn run_with_callback<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        index_ids: Vec<String>,
        on_result: F,
    ) -> Result<SpeedtestRunResult>
    where
        F: Fn(SpeedtestResult) + Send + Sync,
    {
        let cancel = self.begin_job()?;
        let result = self
            .run_inner(database, config, index_ids, Arc::clone(&cancel), on_result)
            .await;
        self.finish_job(&cancel)?;

        result
    }

    async fn run_inner<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        index_ids: Vec<String>,
        cancel: CancellationFlag,
        on_result: F,
    ) -> Result<SpeedtestRunResult>
    where
        F: Fn(SpeedtestResult) + Send + Sync,
    {
        let selected = select_test_items(database, config, &index_ids).await?;
        clear_previous_results(database, &selected, &on_result).await?;

        let mut results = self
            .run_batch_items(database, config, &selected, Arc::clone(&cancel), &on_result)
            .await?;
        let completed_count = u32::try_from(results.len()).unwrap_or(u32::MAX);

        let cancelled = is_cancelled(&cancel);
        // Every profile got a `Testing` marker before the first probe, so
        // anything the run never reached has to be written back to a terminal
        // state or it stays pending forever, including across restarts.
        let pending =
            finalize_pending_results(database, &selected, &results, cancelled, &on_result).await?;
        results.extend(pending);

        Ok(SpeedtestRunResult {
            cancelled,
            selected_count: u32::try_from(selected.len()).unwrap_or(u32::MAX),
            completed_count,
            results,
        })
    }

    /// Cancels the active run and kills every probe core synchronously.
    ///
    /// Tauri ends the process with `std::process::exit`, so managed state, the
    /// pending `run_speedtest` future and its session values are never dropped.
    /// The shell has to call this from its exit handler, otherwise probe cores
    /// outlive the app holding loopback listeners and open tunnels.
    pub fn shutdown(&self) {
        if let Err(error) = self.cancel() {
            tracing::warn!(?error, "failed to cancel speedtest during shutdown");
        }
        self.core_backend.stop_all();
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

    async fn run_batch_items<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        items: &[ServerTestItem],
        cancel: CancellationFlag,
        on_result: &F,
    ) -> Result<Vec<SpeedtestResult>>
    where
        F: Fn(SpeedtestResult) + Send + Sync,
    {
        let batch = self
            .prepare_speedtest_items(database, config, items)
            .await?;
        let mut results = record_item_failures(database, batch.failures, items, on_result).await?;
        for (core_type, group) in group_prepared_items(batch.prepared) {
            if is_cancelled(&cancel) {
                break;
            }
            let page_size = speedtest_page_size(config, group.len());
            let batch_count = group.chunks(page_size).len();
            for (batch_index, page) in group.chunks(page_size).enumerate() {
                if is_cancelled(&cancel) {
                    break;
                }
                let entries = page
                    .iter()
                    .map(|prepared| prepared.entry.clone())
                    .collect::<Vec<_>>();
                let session = match self
                    .core_backend
                    .start(core_type, entries, Arc::clone(&cancel))
                    .await
                {
                    Ok(session) => session,
                    Err(SpeedtestError::Cancelled) => break,
                    Err(error) => {
                        // A core that will not start fails this page, not the
                        // whole run: record every profile in it and continue.
                        tracing::warn!(?error, ?core_type, "speedtest core failed to start");
                        let failures = page
                            .iter()
                            .map(|prepared| {
                                SpeedtestItemFailure::from_error(
                                    prepared.item.index_id.clone(),
                                    &error,
                                )
                            })
                            .collect::<Vec<_>>();
                        results.extend(
                            record_item_failures(database, failures, items, on_result).await?,
                        );
                        continue;
                    }
                };
                for prepared in page {
                    if is_cancelled(&cancel) {
                        break;
                    }
                    let result = self
                        .run_realping(database, config, prepared.item.clone(), Arc::clone(&cancel))
                        .await?;
                    if let Some(result) = result {
                        on_result(result.clone());
                        results.push(result);
                    }
                }
                session.close().await;
                if batch_index + 1 < batch_count && !is_cancelled(&cancel) {
                    time::sleep(speedtest_delay_interval(config)).await;
                }
            }
        }

        Ok(results)
    }

    async fn prepare_speedtest_items(
        &self,
        database: &Database,
        config: &AppConfig,
        items: &[ServerTestItem],
    ) -> Result<PreparedSpeedtestBatch> {
        let env = load_runtime_core_gen_env(database, &self.paths, config, self.target_os).await?;
        let builder = CoreConfigContextBuilder::new(&env);
        let reserved = reserve_speedtest_ports(
            items
                .iter()
                .map(|item| i32::from(item.socks_port))
                .collect(),
        )
        .await?;
        let mut batch = PreparedSpeedtestBatch::default();

        for (mut item, socks_port) in items.iter().cloned().zip(reserved) {
            let socks_port = match socks_port {
                Ok(socks_port) => socks_port,
                Err(error) => {
                    batch
                        .failures
                        .push(SpeedtestItemFailure::from_error(item.index_id, &error));
                    continue;
                }
            };
            item.socks_port = socks_port;
            let build = builder.build(config, &item.profile);
            if !build.success() {
                // One unusable profile in the selection must not cancel the
                // profiles that are still testable.
                batch.failures.push(SpeedtestItemFailure::new(
                    item.index_id,
                    SpeedtestOutcome::InvalidProfile,
                ));
                continue;
            }
            batch.prepared.push(PreparedSpeedtestItem {
                entry: SpeedtestConfigEntry {
                    index_id: item.index_id.clone(),
                    port: i32::from(socks_port),
                    context: build.context,
                },
                item,
            });
        }

        Ok(batch)
    }

    async fn run_realping(
        &self,
        database: &Database,
        config: &AppConfig,
        item: ServerTestItem,
        cancel: CancellationFlag,
    ) -> Result<Option<SpeedtestResult>> {
        let index_id = item.index_id.clone();
        let result = match self
            .probe
            .realping(item.socks_port, config.speed_test_item.clone(), cancel)
            .await
        {
            Ok(realping) => SpeedtestResult {
                index_id,
                delay: Some(realping.delay),
                outcome: measured_outcome(realping.delay),
                detail: None,
                ip_info: realping.ip_info,
                country_code: realping.country_code,
            },
            Err(error) => {
                tracing::warn!(index_id = %index_id, ?error, "speedtest realping failed");
                SpeedtestResult {
                    index_id,
                    delay: Some(-1),
                    outcome: speedtest_outcome(&error),
                    detail: speedtest_detail(&error),
                    // The failure is the outcome now. This used to write
                    // "Skipped" into the IP-info column, which is neither IP
                    // information nor translatable.
                    ip_info: None,
                    country_code: None,
                }
            }
        };
        let saved = persist_speedtest_result_with_retry(database, &result, &item.profile).await?;

        Ok(saved.then_some(result))
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

/// Persists and reports a terminal result for profiles that could not be
/// tested, so a bad profile or a core that refuses to start is a per-item
/// failure instead of an aborted run.
async fn record_item_failures<F>(
    database: &Database,
    failures: Vec<SpeedtestItemFailure>,
    selected: &[ServerTestItem],
    on_result: &F,
) -> Result<Vec<SpeedtestResult>>
where
    F: Fn(SpeedtestResult) + Send + Sync,
{
    let mut results = Vec::with_capacity(failures.len());
    for failure in failures {
        let result = make_failure_result(failure.index_id, failure.outcome, failure.detail);
        let Some(item) = selected
            .iter()
            .find(|item| item.index_id == result.index_id)
        else {
            continue;
        };
        if persist_speedtest_result_with_retry(database, &result, &item.profile).await? {
            on_result(result.clone());
            results.push(result);
        }
    }

    Ok(results)
}

/// Clears the pre-run markers of every selected profile the run never reached.
/// `clear_previous_results` marks the whole selection pending up front, so a
/// cancel or an early stop would otherwise leave those rows pending forever.
async fn finalize_pending_results<F>(
    database: &Database,
    selected: &[ServerTestItem],
    results: &[SpeedtestResult],
    cancelled: bool,
    on_result: &F,
) -> Result<Vec<SpeedtestResult>>
where
    F: Fn(SpeedtestResult) + Send + Sync,
{
    let tested = results
        .iter()
        .map(|result| result.index_id.as_str())
        .collect::<HashSet<_>>();
    let outcome = if cancelled {
        SpeedtestOutcome::Cancelled
    } else {
        SpeedtestOutcome::Skipped
    };
    let untested = selected
        .iter()
        .filter(|item| !tested.contains(item.index_id.as_str()))
        .map(|item| SpeedtestItemFailure::new(item.index_id.clone(), outcome))
        .collect::<Vec<_>>();

    record_item_failures(database, untested, selected, on_result).await
}

/// Longest a contended write is retried before the result is given up on.
const PERSIST_RETRY_MAX_ATTEMPTS: u32 = 5;
/// First backoff step; doubles per attempt, so the total wait stays under a
/// second even in the worst case.
const PERSIST_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(20);

/// Persists one result, riding out a contended SQLite write.
///
/// The database runs in WAL with a busy timeout, so `SQLITE_BUSY` should be
/// rare — but it is still reachable (a checkpoint, or a write that started
/// before the busy handler could be armed). Propagating it aborted the whole
/// run and threw away every probe that had already completed, which is a far
/// worse outcome than waiting a few tens of milliseconds for one row.
async fn persist_speedtest_result_with_retry(
    database: &Database,
    result: &SpeedtestResult,
    profile: &ProfileItem,
) -> Result<bool> {
    let mut delay = PERSIST_RETRY_INITIAL_DELAY;
    for attempt in 1..PERSIST_RETRY_MAX_ATTEMPTS {
        match persist_speedtest_result(database, result, profile).await {
            Err(error) if is_database_contention(&error) => {
                tracing::debug!(
                    index_id = %result.index_id,
                    attempt,
                    "speedtest result write contended; retrying"
                );
                time::sleep(delay).await;
                delay = delay.saturating_mul(2);
            }
            outcome => return outcome,
        }
    }

    persist_speedtest_result(database, result, profile).await
}

/// Whether a persistence failure is SQLite telling us to come back later.
///
/// `voya-app` has no `sqlx` dependency (the architecture gate keeps the
/// persistence boundary inside `voya-db`), so the classification walks the
/// error source chain and matches SQLite's own wording for `SQLITE_BUSY`,
/// `SQLITE_BUSY_SNAPSHOT` and `SQLITE_LOCKED`. Misreading an unrelated error as
/// contention only costs a few retries, and the loop is bounded either way.
fn is_database_contention(error: &SpeedtestError) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = source {
        let message = current.to_string().to_ascii_lowercase();
        if message.contains("database is locked") || message.contains("database table is locked") {
            return true;
        }
        source = current.source();
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speedtest_persist_retry_recognizes_a_contended_write() {
        // SQLite reports `SQLITE_BUSY` / `SQLITE_BUSY_SNAPSHOT` as "database is
        // locked"; sqlx passes that message through unchanged.
        let busy = SpeedtestError::Database(DbError::Io {
            path: PathBuf::from("voya.db"),
            source: io::Error::other("database is locked"),
        });

        assert!(
            is_database_contention(&busy),
            "SQLITE_BUSY must be retried, not turned into a lost speedtest run"
        );
    }

    #[test]
    fn speedtest_persist_retry_ignores_unrelated_failures() {
        let cancelled = SpeedtestError::Cancelled;
        let missing = SpeedtestError::MissingCoreInfo(CoreType::sing_box);

        assert!(!is_database_contention(&cancelled));
        assert!(!is_database_contention(&missing));
    }

    #[tokio::test]
    async fn speedtest_persist_retry_writes_the_result_through() {
        let database = Database::connect_in_memory()
            .await
            .expect("speedtest test operation should succeed");
        database
            .profiles()
            .upsert(&ProfileItem {
                index_id: "active".to_string(),
                remarks: "Active".to_string(),
                ..ProfileItem::default()
            })
            .await
            .expect("speedtest test operation should succeed");

        persist_speedtest_result_with_retry(
            &database,
            &SpeedtestResult {
                index_id: "active".to_string(),
                delay: Some(42),
                outcome: SpeedtestOutcome::Completed,
                detail: None,
                ip_info: None,
                country_code: None,
            },
            &database
                .profiles()
                .get("active")
                .await
                .expect("load profile")
                .expect("profile exists"),
        )
        .await
        .expect("speedtest test operation should succeed");

        assert_eq!(
            database
                .profile_exs()
                .get("active")
                .await
                .expect("speedtest test operation should succeed")
                .expect("the probe result is persisted")
                .delay,
            42
        );
    }

    /// The backoff has to stay short enough that a contended write never looks
    /// like a hung run to the user.
    #[test]
    fn speedtest_persist_retry_backoff_is_bounded() {
        let mut total = Duration::ZERO;
        let mut delay = PERSIST_RETRY_INITIAL_DELAY;
        for _ in 1..PERSIST_RETRY_MAX_ATTEMPTS {
            total += delay;
            delay = delay.saturating_mul(2);
        }

        assert!(total < Duration::from_secs(1), "{total:?}");
    }
}
