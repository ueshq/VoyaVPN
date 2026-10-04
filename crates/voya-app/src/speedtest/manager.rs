use futures_util::{stream, FutureExt, Stream, StreamExt};
use tokio::task;
use voya_core::PreparedContextBuilder;

use super::core_backend::{
    background_task_failed, reserve_speedtest_ports, wait_for_speedtest_ports,
};

use super::persist::persist_speedtest_results_with_retry;
use super::*;

/// A started probe core held for one page's measurements.
///
/// Tears down off the Tokio workers; stopping a child blocks until the reaper
/// thread has killed and waited it. `Drop` covers panics and early returns —
/// a cancelled run among them, which is an ordinary path — and it hands the
/// stop to the blocking pool too whenever a runtime is there to take it.
pub struct ProbeCoreSession {
    core: Option<Box<dyn ProbeCore>>,
}

impl ProbeCoreSession {
    pub async fn close(mut self) {
        let core = self.core.take();
        if let Some(core) = core {
            if let Err(error) = task::spawn_blocking(move || core.stop()).await {
                tracing::warn!(?error, "failed to close speedtest core session");
            }
        }
    }
}

impl Drop for ProbeCoreSession {
    fn drop(&mut self) {
        let Some(core) = self.core.take() else {
            return;
        };
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn_blocking(move || core.stop());
            }
            Err(_) => core.stop(),
        }
    }
}

/// Starts one throwaway probe core for a page of entries and waits for its
/// SOCKS ports. Shared by the speedtest run and the self-host self-test.
pub async fn start_probe_core_page(
    launcher: &Arc<dyn ProbeCoreLauncher>,
    entries: &[SpeedtestConfigEntry],
    cancel: &CancellationFlag,
) -> Result<ProbeCoreSession> {
    check_cancelled(cancel)?;
    let ports = entries.iter().map(|entry| entry.port).collect::<Vec<_>>();
    let config_json = generate_singbox_speedtest_config_json(entries)?;
    let launcher = Arc::clone(launcher);
    let core = task::spawn_blocking(move || launcher.start(config_json))
        .await
        .map_err(background_task_failed)??;
    // Bound before the wait, so a core that never becomes ready — or a run
    // cancelled while waiting, which is the ordinary way out — is torn down.
    let session = ProbeCoreSession { core: Some(core) };
    if let Err(error) = wait_for_speedtest_ports(&ports, cancel).await {
        session.close().await;
        return Err(error);
    }

    Ok(session)
}

/// A run's results as they are recorded: reported through the caller's
/// callback, and kept by the run itself so that one failing part-way still
/// knows which nodes it reached.
pub(super) struct Recorded<'a, F> {
    pub(super) on_results: &'a F,
    pub(super) results: Vec<SpeedtestResult>,
}

/// Measures nodes with a throwaway sing-box per page, probed over SOCKS.
///
/// On macOS a connected PacketTunnel core takes over instead: every connection
/// then goes through the tunnel, so a probe core would measure the node through
/// the running VPN rather than directly.
#[derive(Clone)]
pub struct SpeedtestManager {
    pub(super) probe: Arc<dyn SpeedtestProbe>,
    pub(super) launcher: Arc<dyn ProbeCoreLauncher>,
    pub(super) running_core: Option<Arc<dyn RunningCoreProbe>>,
    pub(super) paths: AppPaths,
    pub(super) target_os: TargetOs,
    pub(super) active_cancel: Arc<Mutex<Option<CancellationFlag>>>,
    /// Held for a run's whole database work. Cancelling a superseded run only
    /// sets its flag; it still writes its untested nodes back as `Cancelled`
    /// on the way out, and without this that write can land on top of the
    /// `Testing` markers and results of the run that replaced it.
    pub(super) run_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SpeedtestManager {
    /// A manager over the host's launcher: the desktop's starts each probe
    /// core as a child process, the phones run theirs inside the app process.
    #[must_use]
    pub fn with_launcher(paths: AppPaths, launcher: Arc<dyn ProbeCoreLauncher>) -> Self {
        Self::with_probe_and_launcher(paths, Arc::new(ReqwestSpeedtestProbe), launcher)
    }

    #[must_use]
    pub(super) fn with_probe_and_launcher(
        paths: AppPaths,
        probe: Arc<dyn SpeedtestProbe>,
        launcher: Arc<dyn ProbeCoreLauncher>,
    ) -> Self {
        Self {
            probe,
            launcher,
            running_core: None,
            paths,
            target_os: TargetOs::current(),
            active_cancel: Arc::new(Mutex::new(None)),
            run_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// macOS: while the PacketTunnel core is connected, measure through it.
    /// The check runs per test, so a disconnected app still uses probe cores.
    #[must_use]
    pub fn with_running_core(mut self, running_core: Arc<dyn RunningCoreProbe>) -> Self {
        self.running_core = Some(running_core);
        self
    }

    pub async fn run_with_callback<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        index_ids: Vec<String>,
        on_results: F,
    ) -> Result<SpeedtestRunResult>
    where
        F: Fn(Vec<SpeedtestResult>) + Send + Sync,
    {
        // Supersede first, so the run being replaced starts unwinding at once,
        // then wait for it to finish writing.
        let cancel = self.begin_job();
        let result = {
            let _run = self.run_lock.lock().await;
            if is_cancelled(&cancel) {
                // Superseded while queued: it never marked anything pending,
                // so there is nothing to write back.
                Ok(SpeedtestRunResult {
                    cancelled: true,
                    selected_count: 0,
                    completed_count: 0,
                    results: Vec::new(),
                })
            } else {
                self.run_inner(database, config, index_ids, Arc::clone(&cancel), on_results)
                    .await
            }
        };
        self.finish_job(&cancel);

        result
    }

    async fn run_inner<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        index_ids: Vec<String>,
        cancel: CancellationFlag,
        on_results: F,
    ) -> Result<SpeedtestRunResult>
    where
        F: Fn(Vec<SpeedtestResult>) + Send + Sync,
    {
        let selected = select_test_items(database, config, &index_ids).await?;
        clear_previous_results(database, &selected, &on_results).await?;

        // Owned here rather than by the two paths below: when one of them
        // fails part-way, what it had already recorded is still known, and
        // the rest of the selection can be taken off `Testing`.
        let mut recorded = Recorded {
            on_results: &on_results,
            results: Vec::new(),
        };
        let connected = match &self.running_core {
            Some(running_core) => running_core.connect().await,
            None => None,
        };
        let ran = if let Some(core) = connected {
            self.run_through_running_core(
                core,
                database,
                config,
                &selected,
                Arc::clone(&cancel),
                &mut recorded,
            )
            .await
        } else {
            self.run_batch_items(
                database,
                config,
                &selected,
                Arc::clone(&cancel),
                &mut recorded,
            )
            .await
        };
        let mut results = recorded.results;
        let completed_count = u32::try_from(results.len()).unwrap_or(u32::MAX);

        let cancelled = is_cancelled(&cancel);
        // Every profile got a `Testing` marker before the first probe, so
        // anything the run never reached has to be written back to a terminal
        // state or it stays pending forever, including across restarts. That
        // holds for a run that failed too, whose error is the one reported.
        let finalized =
            finalize_pending_results(database, &selected, &results, cancelled, &on_results).await;
        if let Err(error) = ran {
            if let Err(finalize_error) = finalized {
                tracing::warn!(
                    error = ?finalize_error,
                    "failed to clear the pending markers of a failed speedtest run"
                );
            }
            return Err(error);
        }
        results.extend(finalized?);

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
        self.cancel();
        self.launcher.stop_all();
    }

    /// Returns whether a run was active to cancel.
    pub fn cancel(&self) -> bool {
        match lock_ignoring_poison(&self.active_cancel).as_ref() {
            Some(cancel) => {
                cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    pub fn status(&self) -> SpeedtestStatus {
        SpeedtestStatus {
            running: lock_ignoring_poison(&self.active_cancel).is_some(),
        }
    }

    async fn run_batch_items<F>(
        &self,
        database: &Database,
        config: &AppConfig,
        items: &[ServerTestItem],
        cancel: CancellationFlag,
        recorded: &mut Recorded<'_, F>,
    ) -> Result<()>
    where
        F: Fn(Vec<SpeedtestResult>) + Send + Sync,
    {
        let env = load_runtime_core_gen_env(database, &self.paths, config, self.target_os).await?;
        let contexts = CoreConfigContextBuilder::new(&env).prepare(config);
        let capabilities = self.launcher.capabilities();
        let page_size = speedtest_page_size(config, items.len());
        let mut remaining = items.iter().peekable();
        while remaining.peek().is_some() {
            if is_cancelled(&cancel) {
                break;
            }
            // Contexts are built a page at a time: each one carries its own
            // copy of the app config, and built for the whole selection up
            // front they all stayed resident until the run ended. A profile
            // that cannot be tested is therefore reported when its page comes
            // up, and one a cancel never reaches finishes `Cancelled`.
            let (page, invalid) = prepare_page(&contexts, &capabilities, &mut remaining, page_size);
            recorded
                .results
                .extend(record_item_failures(database, invalid, items, recorded.on_results).await?);
            // Reserved per page, right before its core binds them: reserved
            // for the whole selection up front, a later page's ports sat free
            // for as long as the earlier pages ran, for anything to take.
            let (page, failures) = reserve_page_ports(page).await?;
            recorded.results.extend(
                record_item_failures(database, failures, items, recorded.on_results).await?,
            );
            if page.is_empty() {
                continue;
            }
            let entries = page
                .iter()
                .map(|prepared| prepared.entry.clone())
                .collect::<Vec<_>>();
            let session = match start_probe_core_page(&self.launcher, &entries, &cancel).await {
                Ok(session) => session,
                Err(SpeedtestError::Cancelled) => {
                    cancel.store(true, Ordering::SeqCst);
                    break;
                }
                Err(error) => {
                    // A core that will not start fails this page, not the
                    // whole run: record every profile in it and continue.
                    tracing::warn!(?error, "speedtest core failed to start");
                    let failures = page
                        .iter()
                        .map(|prepared| {
                            SpeedtestItemFailure::from_error(prepared.item.index_id.clone(), &error)
                        })
                        .collect::<Vec<_>>();
                    recorded.results.extend(
                        record_item_failures(database, failures, items, recorded.on_results)
                            .await?,
                    );
                    continue;
                }
            };
            // Every node in the page has its own inbound on this core, so the
            // probes are independent. One at a time, a page of mostly dead
            // nodes cost two full timeouts per node. Indices rather than
            // borrows keep the stream's futures `'static`, as in
            // `run_through_running_core`.
            let probes = page
                .iter()
                .enumerate()
                .map(|(index, prepared)| (index, prepared.item.socks_port))
                .collect::<Vec<_>>();
            let mut pending = stream::iter(probes.into_iter().map(|(index, socks_port)| {
                let probe = Arc::clone(&self.probe);
                let speed_test = config.speed_test.clone();
                let cancel = Arc::clone(&cancel);
                async move {
                    // Queued probes that would start after a cancel never run.
                    let probed = if is_cancelled(&cancel) {
                        None
                    } else {
                        Some(
                            ProbeTask::spawn(probe.realping(socks_port, speed_test, cancel))
                                .join()
                                .await,
                        )
                    };
                    (index, probed)
                }
            }))
            .buffer_unordered(SPEEDTEST_CONCURRENCY);
            while let Some(ready) = next_ready_batch(&mut pending, &cancel).await {
                let writes = ready
                    .into_iter()
                    .filter_map(|(index, probed)| {
                        let prepared = page.get(index)?;
                        let result = realping_result(prepared.item.index_id.clone(), probed?);
                        Some((&prepared.item.profile, result))
                    })
                    .collect();
                recorded
                    .results
                    .extend(persist_and_report(database, writes, recorded.on_results).await?);
            }
            drop(pending);
            session.close().await;
            if remaining.peek().is_some() && !is_cancelled(&cancel) {
                time::sleep(speedtest_delay_interval(config)).await;
            }
        }

        Ok(())
    }

    fn begin_job(&self) -> CancellationFlag {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut active = lock_ignoring_poison(&self.active_cancel);
        if let Some(previous) = active.replace(Arc::clone(&cancel)) {
            previous.store(true, Ordering::SeqCst);
        }

        cancel
    }

    fn finish_job(&self, cancel: &CancellationFlag) {
        let mut active = lock_ignoring_poison(&self.active_cancel);
        if active
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, cancel))
        {
            *active = None;
        }
    }
}

/// Takes the next page off `remaining`: up to `page_size` profiles a probe core
/// can carry, plus an `InvalidProfile` failure for each unusable one met on the
/// way, so one bad profile never cancels the testable ones around it.
///
/// A profile whose outbound type the core was not built for is reported as
/// `ProtocolUnsupported` instead: sing-box refuses a whole config that names
/// an unknown outbound type, so one such node would fail its entire page.
fn prepare_page<'a>(
    contexts: &PreparedContextBuilder,
    capabilities: &ProbeCoreCapabilities,
    remaining: &mut impl Iterator<Item = &'a ServerTestItem>,
    page_size: usize,
) -> (Vec<PreparedSpeedtestItem>, Vec<SpeedtestItemFailure>) {
    let mut page = Vec::with_capacity(page_size);
    let mut invalid = Vec::new();
    while page.len() < page_size {
        let Some(item) = remaining.next() else {
            break;
        };
        if let Some(tag) = capabilities.missing_build_tag(item.profile.config_type()) {
            invalid.push(
                SpeedtestItemFailure::new(
                    item.index_id.clone(),
                    SpeedtestOutcome::ProtocolUnsupported,
                )
                .with_detail(format!("the bundled sing-box is built without {tag}")),
            );
            continue;
        }
        // A probe core only carries each node's own outbound, so the entry's
        // context leaves the routing rules' outbounds out.
        let build = contexts.build_node_outbound(&item.profile);
        if !build.success() {
            invalid.push(SpeedtestItemFailure::new(
                item.index_id.clone(),
                SpeedtestOutcome::InvalidProfile,
            ));
            continue;
        }
        page.push(PreparedSpeedtestItem {
            entry: SpeedtestConfigEntry {
                index_id: item.index_id.clone(),
                // The preferred port; `reserve_page_ports` settles it.
                port: i32::from(item.socks_port),
                context: build.context,
            },
            item: item.clone(),
        });
    }
    (page, invalid)
}

/// The next finished measurement plus every other one already finished, up to
/// `SPEEDTEST_CONCURRENCY`: a burst of answers then costs one transaction and
/// one delivery rather than one of each per node. `None` once the stream ends
/// or the run is cancelled.
///
/// A probe only notices a cancel between attempts, so the run stops waiting at
/// once and `finalize_pending_results` marks whatever was still in flight.
pub(super) async fn next_ready_batch<S>(
    pending: &mut S,
    cancel: &CancellationFlag,
) -> Option<Vec<S::Item>>
where
    S: Stream + Unpin,
{
    let first = tokio::select! {
        biased;
        () = cancelled(cancel) => return None,
        next = pending.next() => next?,
    };
    let mut ready = vec![first];
    while ready.len() < SPEEDTEST_CONCURRENCY {
        match pending.next().now_or_never() {
            Some(Some(next)) => ready.push(next),
            Some(None) | None => break,
        }
    }
    // `cancelled` polls, so a probe answering its own cancel can still win the
    // race above; its answer is dropped like the ones still in flight.
    (!is_cancelled(cancel)).then_some(ready)
}

/// Moves every node in `page` onto a free loopback port, starting from the one
/// it prefers. A node with no port left becomes a failure; the rest are ready
/// for a probe core.
async fn reserve_page_ports(
    page: Vec<PreparedSpeedtestItem>,
) -> Result<(Vec<PreparedSpeedtestItem>, Vec<SpeedtestItemFailure>)> {
    let starts = page
        .iter()
        .map(|prepared| i32::from(prepared.item.socks_port))
        .collect();
    let reserved = reserve_speedtest_ports(starts).await?;
    let mut ready = Vec::with_capacity(page.len());
    let mut failures = Vec::new();
    for (mut prepared, port) in page.into_iter().zip(reserved) {
        match port {
            Ok(port) => {
                prepared.item.socks_port = port;
                prepared.entry.port = i32::from(port);
                ready.push(prepared);
            }
            Err(error) => failures.push(SpeedtestItemFailure::from_error(
                prepared.item.index_id,
                &error,
            )),
        }
    }

    Ok((ready, failures))
}

/// One probe on its own task.
///
/// A probe times its request from send to the moment its future resumes. In
/// the run's result stream that future would sit unpolled while the loop
/// writes another probe's result, and the write would be counted as latency;
/// on a task of its own it is polled as soon as the response arrives.
/// Dropping the handle (a cancelled run) aborts the probe.
struct ProbeTask(task::JoinHandle<Result<RealPingProbeResult>>);

impl ProbeTask {
    fn spawn(probe: BoxFuture<'static, Result<RealPingProbeResult>>) -> Self {
        Self(tokio::spawn(probe))
    }

    async fn join(mut self) -> Result<RealPingProbeResult> {
        (&mut self.0)
            .await
            .unwrap_or_else(|error| Err(SpeedtestError::BackgroundTask(error.to_string())))
    }
}

impl Drop for ProbeTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Maps one probe-core measurement onto the persisted outcome vocabulary.
fn realping_result(index_id: String, probed: Result<RealPingProbeResult>) -> SpeedtestResult {
    match probed {
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
    }
}

/// Persists and reports a terminal result for profiles that could not be
/// tested, so a bad profile or a core that refuses to start is a per-item
/// failure instead of an aborted run.
///
/// A cancelled run hands every untested node in here at once. They are written
/// in one transaction and reported in one delivery: per node, that was a
/// commit, a linear search of the selection and an IPC event each.
pub(super) async fn record_item_failures<F>(
    database: &Database,
    failures: Vec<SpeedtestItemFailure>,
    selected: &[ServerTestItem],
    on_results: &F,
) -> Result<Vec<SpeedtestResult>>
where
    F: Fn(Vec<SpeedtestResult>) + Send + Sync,
{
    if failures.is_empty() {
        return Ok(Vec::new());
    }
    let mut by_id = HashMap::with_capacity(selected.len());
    for item in selected {
        by_id.entry(item.index_id.as_str()).or_insert(&item.profile);
    }
    let writes = failures
        .into_iter()
        .filter_map(|failure| {
            let profile = by_id.get(failure.index_id.as_str()).copied()?;
            Some((
                profile,
                make_failure_result(failure.index_id, failure.outcome, failure.detail),
            ))
        })
        .collect::<Vec<_>>();
    persist_and_report(database, writes, on_results).await
}

/// Writes `writes` in one transaction, then reports the rows that were written
/// in one delivery.
pub(super) async fn persist_and_report<F>(
    database: &Database,
    writes: Vec<(&ProfileItem, SpeedtestResult)>,
    on_results: &F,
) -> Result<Vec<SpeedtestResult>>
where
    F: Fn(Vec<SpeedtestResult>) + Send + Sync,
{
    if writes.is_empty() {
        return Ok(Vec::new());
    }
    let results = persist_speedtest_results_with_retry(database, writes).await?;
    if !results.is_empty() {
        on_results(results.clone());
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
    on_results: &F,
) -> Result<Vec<SpeedtestResult>>
where
    F: Fn(Vec<SpeedtestResult>) + Send + Sync,
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

    record_item_failures(database, untested, selected, on_results).await
}
