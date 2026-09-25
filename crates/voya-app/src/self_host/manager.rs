//! The node's lifecycle: settings, credentials, the core process, restarts
//! after a crash, and the network check that feeds the links.
//!
//! The `lifecycle` lock serializes every change of the process, so a save
//! arriving while a crash restart is pending cannot start two cores. It is held
//! across the slow parts (spawning, stopping, removing router forwards); the
//! `node` lock beside it is only taken for short reads and writes, so the page
//! never waits on a starting core. A generation counter, bumped on every start
//! and stop, lets a scheduled restart notice that the user changed something in
//! the meantime.

use std::{
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, Instant},
};

use tokio::{
    sync::{mpsc, watch, Mutex, MutexGuard, Notify},
    task::{self, JoinHandle},
};
use voya_contracts::{
    AppNoticeLevel, LogCode, LogLevel, NoticeCode, SelfHostConfig, SelfHostEnvironmentReport,
    SelfHostProblem, SelfHostRecord, SelfHostRuntime, SelfHostRuntimeStatus, SelfHostState,
    SelfHostStats,
};
use voya_core::{SelfHostClashApi, LOOPBACK};
use voya_db::Database;
use voya_net::clash::{ClashApiEndpoint, ClashRestClient};
use voya_platform::process::ProcessExit;

use super::{
    environment::{check_environment, CheckInput},
    identity::{mint_credentials, pick_free_port},
    process::{
        core_executable, remove_config, start_core, start_problem, stop_core, ExitForwarder,
        RunningCore,
    },
    spec::{
        core_settings_changed, enabled_ports, normalized_config, selfhost_spec, share_links,
        validate_self_host_config,
    },
    watch::run_watch_loop,
    Result, SelfHostDeps, SelfHostError,
};
use crate::{
    backoff::{exponential_delay, sleep_or_shutdown},
    supervisor::ClashApiSecret,
};

/// A core that exits this many times in a row is left stopped.
const MAX_RESTART_ATTEMPTS: u32 = 5;
/// A core that ran this long before exiting starts a fresh restart streak.
const STABLE_RUN: Duration = Duration::from_secs(60);
/// Removing router forwards must not hold up quitting.
const UNMAP_ON_STOP_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Clone)]
pub struct SelfHostManager {
    inner: Arc<Inner>,
}

struct Inner {
    database: Database,
    deps: SelfHostDeps,
    /// Serializes starts, stops and exit handling. The functions that need it
    /// take its guard as a parameter.
    lifecycle: Mutex<()>,
    node: Mutex<NodeState>,
    /// Serializes network checks; a check takes seconds and must not hold
    /// `node`, which every command reads.
    check_lock: Mutex<()>,
    shutdown: watch::Sender<bool>,
    check_now: Notify,
    tasks: StdMutex<Vec<JoinHandle<()>>>,
}

struct NodeState {
    core: Option<RunningCore>,
    status: SelfHostRuntimeStatus,
    problem: Option<SelfHostProblem>,
    detail: Option<String>,
    problem_port: Option<u16>,
    restart_attempt: u32,
    generation: u64,
    environment: Option<SelfHostEnvironmentReport>,
    mapped_ports: Vec<u16>,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            core: None,
            status: SelfHostRuntimeStatus::Stopped,
            problem: None,
            detail: None,
            problem_port: None,
            restart_attempt: 0,
            generation: 0,
            environment: None,
            mapped_ports: Vec::new(),
        }
    }
}

impl NodeState {
    fn fail(&mut self, problem: SelfHostProblem, detail: Option<String>, port: Option<u16>) {
        self.status = SelfHostRuntimeStatus::Failed;
        self.problem = Some(problem);
        self.detail = detail;
        self.problem_port = port;
    }

    fn clear_problem(&mut self) {
        self.problem = None;
        self.detail = None;
        self.problem_port = None;
    }
}

impl SelfHostManager {
    /// Starts the exit listener and the watch loop, and brings the node up if
    /// it was enabled when the app last quit. Must run inside a Tokio runtime.
    #[must_use]
    pub fn spawn(database: Database, deps: SelfHostDeps) -> Self {
        let (exit_tx, mut exit_rx) = mpsc::unbounded_channel::<ProcessExit>();
        deps.runner
            .set_exit_handler(Some(Arc::new(ExitForwarder(exit_tx))));
        let (shutdown, shutdown_rx) = watch::channel(false);
        let manager = Self {
            inner: Arc::new(Inner {
                database,
                deps,
                lifecycle: Mutex::new(()),
                node: Mutex::new(NodeState::default()),
                check_lock: Mutex::new(()),
                shutdown,
                check_now: Notify::new(),
                tasks: StdMutex::new(Vec::new()),
            }),
        };
        // A config left by a crashed run holds the old private key.
        remove_config(&manager.inner.deps);

        let exits = manager.clone();
        let exit_task = tokio::spawn(async move {
            while let Some(exit) = exit_rx.recv().await {
                exits.handle_exit(exit).await;
            }
        });
        let watcher = manager.clone();
        let watch_task = tokio::spawn(run_watch_loop(watcher, shutdown_rx));
        let launcher = manager.clone();
        let launch_task = tokio::spawn(async move {
            if let Err(error) = launcher.start_on_launch().await {
                tracing::warn!(%error, "failed to start the self-hosted node at launch");
            }
        });
        manager.track([exit_task, watch_task, launch_task]);
        manager
    }

    fn track(&self, handles: impl IntoIterator<Item = JoinHandle<()>>) {
        if let Ok(mut tasks) = self.inner.tasks.lock() {
            tasks.retain(|task| !task.is_finished());
            tasks.extend(handles);
        }
    }

    fn deps(&self) -> &SelfHostDeps {
        &self.inner.deps
    }

    pub(super) fn shutdown_receiver(&self) -> watch::Receiver<bool> {
        self.inner.shutdown.subscribe()
    }

    pub(super) async fn wait_for_check_request(&self) {
        self.inner.check_now.notified().await;
    }

    async fn start_on_launch(&self) -> Result<()> {
        let mut record = self.inner.database.self_host().load().await?;
        if !record.config.enabled {
            return Ok(());
        }
        self.ensure_identity(&mut record).await?;
        let lifecycle = self.inner.lifecycle.lock().await;
        self.start_locked(&lifecycle, &record).await;
        drop(lifecycle);
        self.after_change();
        Ok(())
    }

    pub async fn state(&self) -> Result<SelfHostState> {
        let record = self.inner.database.self_host().load().await?;
        let node = self.inner.node.lock().await;
        Ok(self.build_state(&record, &node))
    }

    pub async fn save_config(&self, config: SelfHostConfig) -> Result<SelfHostState> {
        validate_self_host_config(&config).map_err(SelfHostError::Validation)?;
        let config = normalized_config(config);
        let mut record = self.inner.database.self_host().load().await?;
        let restart = core_settings_changed(&record.config, &config);
        let recheck = record.config.upnp_enabled != config.upnp_enabled;
        record.config = config;
        let state = self.apply(record, restart).await?;
        if recheck {
            self.inner.check_now.notify_one();
        }
        Ok(state)
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<SelfHostState> {
        let mut record = self.inner.database.self_host().load().await?;
        record.config.enabled = enabled;
        self.apply(record, false).await
    }

    /// New UUID, keys and short id: every link handed out so far stops working.
    pub async fn rotate_credentials(&self) -> Result<SelfHostState> {
        let mut record = self.inner.database.self_host().load().await?;
        record.credentials = Some(mint_credentials()?);
        self.apply(record, true).await
    }

    /// Stores `record` and brings the process in line with it.
    async fn apply(&self, mut record: SelfHostRecord, restart: bool) -> Result<SelfHostState> {
        if record.config.enabled {
            self.assign_ports(&mut record).await?;
            if record.credentials.is_none() {
                record.credentials = Some(mint_credentials()?);
            }
        }
        // Taken before the save, so racing changes are stored and applied in
        // the same order.
        let lifecycle = self.inner.lifecycle.lock().await;
        self.inner.database.self_host().save(&record).await?;

        let running = {
            let mut node = self.inner.node.lock().await;
            node.restart_attempt = 0;
            node.core.is_some()
        };
        if !record.config.enabled {
            self.stop_locked(&lifecycle).await;
        } else if restart || !running {
            self.stop_locked(&lifecycle).await;
            self.start_locked(&lifecycle, &record).await;
        }
        let state = {
            let node = self.inner.node.lock().await;
            self.build_state(&record, &node)
        };
        drop(lifecycle);
        self.after_change();
        Ok(state)
    }

    async fn ensure_identity(&self, record: &mut SelfHostRecord) -> Result<()> {
        let before = record.clone();
        self.assign_ports(record).await?;
        if record.credentials.is_none() {
            record.credentials = Some(mint_credentials()?);
        }
        if *record != before {
            self.inner.database.self_host().save(record).await?;
        }
        Ok(())
    }

    /// Replaces a `0` port with a free random one. Chosen ports are kept, so
    /// links stay valid across restarts. Each probe binds sockets, so the draw
    /// runs on the blocking pool.
    async fn assign_ports(&self, record: &mut SelfHostRecord) -> Result<()> {
        let (vless, shadowsocks) = (record.config.vless_port, record.config.shadowsocks_port);
        if vless != 0 && shadowsocks != 0 {
            return Ok(());
        }
        let network = Arc::clone(&self.deps().network);
        let available = move |port| network.port_available(port);
        let (vless, shadowsocks) = task::spawn_blocking(move || {
            let vless = match vless {
                0 => pick_free_port(&[shadowsocks], &available)?,
                port => port,
            };
            let shadowsocks = match shadowsocks {
                0 => pick_free_port(&[vless], &available)?,
                port => port,
            };
            Ok::<_, SelfHostError>((vless, shadowsocks))
        })
        .await??;
        record.config.vless_port = vless;
        record.config.shadowsocks_port = shadowsocks;
        Ok(())
    }

    /// Starts the core. `node` is only held to publish `Starting` and then the
    /// outcome, so the page can read the state while the core spawns.
    async fn start_locked(&self, _lifecycle: &MutexGuard<'_, ()>, record: &SelfHostRecord) {
        let ports = enabled_ports(&record.config);
        {
            let mut node = self.inner.node.lock().await;
            node.generation += 1;
            node.clear_problem();
            if ports.is_empty() {
                node.fail(SelfHostProblem::NoProtocol, None, None);
                return;
            }
            node.status = SelfHostRuntimeStatus::Starting;
        }
        self.after_change();

        let deps = self.deps().clone();
        let record = record.clone();
        let started = task::spawn_blocking(move || {
            if let Some(port) = ports
                .iter()
                .copied()
                .find(|port| !deps.network.port_available(*port))
            {
                return Err((SelfHostProblem::PortInUse, None, Some(port)));
            }
            let clash_port = pick_free_port(&ports, |port| deps.network.port_available(port))
                .map_err(|error| (start_problem(&error), Some(error.to_string()), None))?;
            let clash_secret = ClashApiSecret::generate();
            let Some(spec) = selfhost_spec(
                &record,
                Some(SelfHostClashApi {
                    port: i32::from(clash_port),
                    secret: clash_secret.as_str().to_string(),
                }),
                &deps.log_level,
            ) else {
                return Err((SelfHostProblem::StartFailed, None, None));
            };
            start_core(&deps, &spec)
                .map(|handle| RunningCore {
                    handle,
                    clash_port,
                    clash_secret,
                    started_at: Instant::now(),
                })
                .map_err(|error| (start_problem(&error), Some(error.to_string()), None))
        })
        .await
        .unwrap_or_else(|error| Err((SelfHostProblem::StartFailed, Some(error.to_string()), None)));

        let mut node = self.inner.node.lock().await;
        match started {
            Ok(core) => {
                node.core = Some(core);
                node.status = SelfHostRuntimeStatus::Running;
                self.log(LogLevel::Info, LogCode::SelfHostStarted, None);
                // The node is reachable at new ports or keys: renew the router
                // forward and the links right away.
                self.inner.check_now.notify_one();
            }
            Err((problem, detail, port)) => {
                self.log(
                    LogLevel::Error,
                    LogCode::SelfHostStartFailed,
                    detail.clone(),
                );
                node.fail(problem, detail, port);
            }
        }
    }

    /// Stops the core and removes the router forwards. The state reads
    /// `Stopped` at once; the stop and the unmap run without `node`.
    async fn stop_locked(&self, _lifecycle: &MutexGuard<'_, ()>) {
        let (core, mapped) = {
            let mut node = self.inner.node.lock().await;
            node.generation += 1;
            node.status = SelfHostRuntimeStatus::Stopped;
            node.clear_problem();
            (node.core.take(), std::mem::take(&mut node.mapped_ports))
        };
        if let Some(core) = core {
            let deps = self.deps().clone();
            if let Err(error) = task::spawn_blocking(move || stop_core(&deps, &core.handle)).await {
                tracing::warn!(%error, "self-hosted stop task failed");
            }
            self.log(LogLevel::Info, LogCode::SelfHostStopped, None);
        }
        if !mapped.is_empty() {
            let unmap = self.deps().port_mapper.unmap(mapped);
            if tokio::time::timeout(UNMAP_ON_STOP_TIMEOUT, unmap)
                .await
                .is_err()
            {
                tracing::debug!("timed out removing router forwards");
            }
        }
    }

    pub(super) async fn handle_exit(&self, exit: ProcessExit) {
        // Waits out a start still spawning: the core that exited may be the
        // one it has not recorded yet.
        let _lifecycle = self.inner.lifecycle.lock().await;
        let mut node = self.inner.node.lock().await;
        let Some(core) = node.core.as_ref() else {
            return;
        };
        if core.handle.id() != exit.process_id {
            return;
        }
        let ran_for = core.started_at.elapsed();
        node.core = None;
        remove_config(self.deps());
        if ran_for >= STABLE_RUN {
            node.restart_attempt = 0;
        }
        node.restart_attempt += 1;
        let detail = exit.exit_code.map(|code| format!("exit code {code}"));
        if node.restart_attempt > MAX_RESTART_ATTEMPTS {
            node.fail(SelfHostProblem::CoreExited, detail.clone(), None);
            self.log(LogLevel::Error, LogCode::SelfHostGaveUp, detail.clone());
            self.deps()
                .sink
                .notice(AppNoticeLevel::Error, NoticeCode::SelfHostGaveUp, detail);
        } else {
            let attempt = node.restart_attempt;
            let backoff = self.deps().restart_backoff;
            let delay = exponential_delay(attempt - 1, backoff.initial, backoff.max);
            node.status = SelfHostRuntimeStatus::Retrying;
            node.problem = Some(SelfHostProblem::CoreExited);
            node.detail = detail.clone();
            self.log(
                LogLevel::Warn,
                LogCode::SelfHostRetryScheduled {
                    attempt,
                    delay_ms: u32::try_from(delay.as_millis()).unwrap_or(u32::MAX),
                },
                detail,
            );
            let generation = node.generation;
            let manager = self.clone();
            let mut shutdown = self.shutdown_receiver();
            let retry = tokio::spawn(async move {
                if !sleep_or_shutdown(delay, &mut shutdown).await {
                    manager.restart_after_exit(generation).await;
                }
            });
            self.track([retry]);
        }
        drop(node);
        self.deps().sink.state_changed();
    }

    async fn restart_after_exit(&self, generation: u64) {
        let lifecycle = self.inner.lifecycle.lock().await;
        {
            let node = self.inner.node.lock().await;
            if node.generation != generation || node.core.is_some() {
                return;
            }
        }
        match self.inner.database.self_host().load().await {
            Ok(record) if record.config.enabled => self.start_locked(&lifecycle, &record).await,
            Ok(_) => return,
            Err(error) => {
                self.inner.node.lock().await.fail(
                    SelfHostProblem::StartFailed,
                    Some(error.to_string()),
                    None,
                );
            }
        }
        drop(lifecycle);
        self.deps().sink.state_changed();
    }

    /// Live traffic, or zeros while the node is down.
    pub async fn stats(&self) -> Result<SelfHostStats> {
        let access = {
            let node = self.inner.node.lock().await;
            node.core
                .as_ref()
                .map(|core| (core.clash_port, core.clash_secret.clone()))
        };
        let Some((port, secret)) = access else {
            return Ok(SelfHostStats {
                active_connections: 0,
                upload_total_bytes: 0.0,
                download_total_bytes: 0.0,
            });
        };
        let client = ClashRestClient::new(ClashApiEndpoint {
            host: LOOPBACK.to_string(),
            port,
            secret: Some(secret.into_token()),
        });
        let connections = client.get_connections().await?;
        Ok(SelfHostStats {
            active_connections: u32::try_from(connections.connections.len()).unwrap_or(u32::MAX),
            upload_total_bytes: connections.upload_total as f64,
            download_total_bytes: connections.download_total as f64,
        })
    }

    /// Runs the network check now and returns the refreshed page state. A
    /// check already under way answers the request: queuing another would
    /// repeat its router, probe and self-test round trips back to back.
    pub async fn run_environment_check(&self) -> Result<SelfHostState> {
        match self.inner.check_lock.try_lock() {
            Ok(serial) => self.check_locked(&serial, false).await?,
            Err(_) => drop(self.inner.check_lock.lock().await),
        }
        self.state().await
    }

    /// The network check the watch loop runs. Returns whether the node is
    /// hosting at all, so the loop can skip idle ticks.
    pub(super) async fn periodic_check(&self) -> Result<()> {
        self.check(true).await
    }

    async fn check(&self, periodic: bool) -> Result<()> {
        let serial = self.inner.check_lock.lock().await;
        self.check_locked(&serial, periodic).await
    }

    async fn check_locked(&self, _serial: &MutexGuard<'_, ()>, periodic: bool) -> Result<()> {
        let record = self.inner.database.self_host().load().await?;
        if periodic && !record.config.enabled {
            return Ok(());
        }
        let (input, generation, previous) = {
            let node = self.inner.node.lock().await;
            (
                CheckInput {
                    ports: enabled_ports(&record.config),
                    node_running: node.core.is_some(),
                    upnp_enabled: record.config.upnp_enabled,
                    previously_mapped: node.mapped_ports.clone(),
                    self_test: node.core.is_some().then(|| record.clone()),
                },
                node.generation,
                node.environment.clone(),
            )
        };
        let outcome = check_environment(self.deps(), input).await;

        for port in &outcome.newly_mapped {
            self.log(
                LogLevel::Info,
                LogCode::SelfHostPortMapped { port: *port },
                None,
            );
        }
        if outcome.mapping_failed {
            self.log(
                LogLevel::Warn,
                LogCode::SelfHostPortMappingFailed,
                outcome.report.port_mapping.detail.clone(),
            );
        }
        if periodic && record.config.custom_address.is_none() {
            if let Some(previous) = previous.as_ref() {
                if public_addresses_moved(previous, &outcome.report) {
                    self.deps().sink.notice(
                        AppNoticeLevel::Warning,
                        NoticeCode::SelfHostAddressChanged,
                        None,
                    );
                }
            }
        }

        let mut node = self.inner.node.lock().await;
        node.environment = Some(outcome.report);
        if node.generation == generation {
            node.mapped_ports = outcome.mapped_ports;
        } else {
            // The node restarted during the check; the loop re-maps next time.
            node.mapped_ports.extend(outcome.mapped_ports);
            node.mapped_ports.sort_unstable();
            node.mapped_ports.dedup();
        }
        drop(node);
        self.deps().sink.state_changed();
        Ok(())
    }

    /// Adds the Windows firewall rule for the core, behind a UAC prompt.
    pub async fn apply_firewall_rule(&self) -> Result<SelfHostState> {
        let deps = self.deps().clone();
        task::spawn_blocking(move || {
            let program = core_executable(&deps)?;
            deps.firewall.allow_program(&program)?;
            Ok::<_, SelfHostError>(())
        })
        .await??;
        self.check(false).await?;
        self.state().await
    }

    /// Stops the core and the loops. Awaited at exit: Tauri ends the process
    /// with `std::process::exit`, so nothing is dropped.
    pub async fn shutdown(&self) {
        // A closed channel only means the loops already left.
        let _ = self.inner.shutdown.send(true);
        {
            let lifecycle = self.inner.lifecycle.lock().await;
            self.stop_locked(&lifecycle).await;
        }
        self.deps().runner.set_exit_handler(None);
        let tasks = self
            .inner
            .tasks
            .lock()
            .map(|mut tasks| std::mem::take(&mut *tasks))
            .unwrap_or_default();
        for task in &tasks {
            task.abort();
        }
        // Do not return while a cancelled watcher still owns a DB connection
        // or can publish an event after the host has finished shutting down.
        for task in tasks {
            let _ = task.await;
        }
    }

    fn build_state(&self, record: &SelfHostRecord, node: &NodeState) -> SelfHostState {
        SelfHostState {
            config: record.config.clone(),
            defaults: SelfHostConfig::default(),
            runtime: SelfHostRuntime {
                status: node.status,
                problem: node.problem,
                detail: node.detail.clone(),
                port: node.problem_port,
            },
            share_links: share_links(record, node.environment.as_ref()),
            environment: node.environment.clone(),
            firewall_rule_supported: self.deps().firewall.manages_firewall(),
        }
    }

    fn after_change(&self) {
        self.deps().sink.state_changed();
    }

    fn log(&self, level: LogLevel, code: LogCode, detail: Option<String>) {
        self.deps().sink.log(level, code, detail);
    }
}

fn public_addresses_moved(
    previous: &SelfHostEnvironmentReport,
    current: &SelfHostEnvironmentReport,
) -> bool {
    [
        (&previous.ipv4.public_address, &current.ipv4.public_address),
        (&previous.ipv6.public_address, &current.ipv6.public_address),
    ]
    .into_iter()
    .any(|(before, after)| before.is_some() && after.is_some() && before != after)
}
