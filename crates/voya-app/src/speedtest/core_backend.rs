//! Temporary sing-box processes that back profile speed tests.
//!
//! Starting a probe core writes a config file, may copy the packaged seed
//! binary and spawns a child process — all blocking syscalls — so the sequence
//! runs on `spawn_blocking` instead of parking one Tokio worker per concurrent
//! probe. Every live child is also registered with the backend: Tauri ends the
//! process through `std::process::exit`, so neither `Drop` nor the pending
//! `run_speedtest` future ever reaps a probe core, and `SpeedtestManager`
//! has to kill them synchronously on shutdown.

use std::{path::Path, sync::MutexGuard};

use tokio::task;

use super::*;
use crate::runtime::{resolve_core_executable, write_core_config};

/// Base readiness budget for a freshly started probe core.
const SPEEDTEST_READY_TIMEOUT: Duration = Duration::from_secs(3);
/// Extra readiness budget per inbound: a batch core opens one SOCKS listener
/// per profile, so a large page legitimately needs longer than a core with
/// one listener does.
const SPEEDTEST_READY_PER_ENTRY: Duration = Duration::from_millis(5);
const SPEEDTEST_READY_TIMEOUT_MAX: Duration = Duration::from_secs(15);
const SPEEDTEST_READY_INTERVAL: Duration = Duration::from_millis(50);
const SPEEDTEST_CONFIG_PREFIX: &str = "configTest";

#[derive(Clone)]
pub struct ProcessSpeedtestCoreBackend {
    paths: AppPaths,
    core_seed_resource_dir: Option<PathBuf>,
    runner: Arc<dyn ProcessRunner>,
    target_os: TargetOs,
    live: LiveProbeCores,
}

impl ProcessSpeedtestCoreBackend {
    #[must_use]
    pub fn new(
        paths: AppPaths,
        core_seed_resource_dir: Option<PathBuf>,
        runner: Arc<dyn ProcessRunner>,
    ) -> Self {
        Self {
            paths,
            core_seed_resource_dir,
            runner,
            target_os: TargetOs::current(),
            live: LiveProbeCores::default(),
        }
    }

    #[must_use]
    pub fn with_target_os(mut self, target_os: TargetOs) -> Self {
        self.target_os = target_os;
        self
    }
}

impl SpeedtestCoreBackend for ProcessSpeedtestCoreBackend {
    fn start(
        &self,
        core_type: CoreType,
        entries: Vec<SpeedtestConfigEntry>,
        cancel: CancellationFlag,
    ) -> BoxFuture<'static, Result<Box<dyn SpeedtestCoreSession>>> {
        let paths = self.paths.clone();
        let core_seed_resource_dir = self.core_seed_resource_dir.clone();
        let runner = Arc::clone(&self.runner);
        let target_os = self.target_os;
        let live = self.live.clone();
        Box::pin(async move {
            check_cancelled(&cancel)?;
            let config_file_name =
                format!("{SPEEDTEST_CONFIG_PREFIX}-{}.json", uuid::Uuid::new_v4());
            let ports = entries.iter().map(|entry| entry.port).collect::<Vec<_>>();
            let blocking_runner = Arc::clone(&runner);
            let started = task::spawn_blocking(move || {
                start_probe_core(StartProbeCoreRequest {
                    paths: &paths,
                    config_file_name: &config_file_name,
                    core_type,
                    entries: &entries,
                    core_seed_resource_dir: core_seed_resource_dir.as_ref(),
                    target_os,
                    runner: blocking_runner.as_ref(),
                })
            })
            .await
            .map_err(background_task_failed)??;

            live.register(LiveProbeCore {
                handle: started.handle.clone(),
                config_path: started.config_path.clone(),
            });
            // Bind the session before waiting so an unready core is still torn
            // down by the `?` below.
            let session = ProcessSpeedtestCoreSession {
                config_path: Some(started.config_path),
                handle: Some(started.handle),
                runner,
                live,
            };
            wait_for_speedtest_ports(&ports, &cancel).await?;

            Ok(Box::new(session) as Box<dyn SpeedtestCoreSession>)
        })
    }

    fn stop_all(&self) {
        for core in self.live.take_all() {
            if let Err(error) = self.runner.stop(&core.handle) {
                tracing::warn!(?error, "failed to stop speedtest core process on shutdown");
            }
            remove_speedtest_config(&core.config_path);
        }
    }
}

/// Probe cores that are currently running. Sessions register on start and
/// deregister when they are closed or dropped, so whatever is left belongs to a
/// run that never finished.
#[derive(Clone, Default)]
struct LiveProbeCores {
    entries: Arc<Mutex<Vec<LiveProbeCore>>>,
}

struct LiveProbeCore {
    handle: ProcessHandle,
    config_path: PathBuf,
}

impl LiveProbeCores {
    fn register(&self, core: LiveProbeCore) {
        lock_ignoring_poison(&self.entries).push(core);
    }

    fn deregister(&self, handle: &ProcessHandle) {
        lock_ignoring_poison(&self.entries).retain(|core| &core.handle != handle);
    }

    fn take_all(&self) -> Vec<LiveProbeCore> {
        std::mem::take(&mut *lock_ignoring_poison(&self.entries))
    }
}

/// A poisoned registry still holds valid child handles, and losing them would
/// leak the very processes this registry exists to reap.
fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

struct ProcessSpeedtestCoreSession {
    config_path: Option<PathBuf>,
    handle: Option<ProcessHandle>,
    runner: Arc<dyn ProcessRunner>,
    live: LiveProbeCores,
}

impl SpeedtestCoreSession for ProcessSpeedtestCoreSession {
    fn close(mut self: Box<Self>) -> BoxFuture<'static, ()> {
        let handle = self.handle.take();
        let config_path = self.config_path.take();
        let runner = Arc::clone(&self.runner);
        let live = self.live.clone();
        Box::pin(async move {
            // `ProcessRunner::stop` blocks until the reaper thread has killed
            // and waited the child, so it must not run on a Tokio worker.
            if let Err(error) = task::spawn_blocking(move || {
                stop_probe_core(&live, runner.as_ref(), handle, config_path)
            })
            .await
            {
                tracing::warn!(?error, "failed to close speedtest core session");
            }
        })
    }
}

impl Drop for ProcessSpeedtestCoreSession {
    fn drop(&mut self) {
        // Best-effort fallback for panics and `?` returns; the happy path goes
        // through `close`, which does the same work off the Tokio workers.
        let handle = self.handle.take();
        let config_path = self.config_path.take();
        stop_probe_core(&self.live, self.runner.as_ref(), handle, config_path);
    }
}

fn stop_probe_core(
    live: &LiveProbeCores,
    runner: &dyn ProcessRunner,
    handle: Option<ProcessHandle>,
    config_path: Option<PathBuf>,
) {
    if let Some(handle) = handle {
        live.deregister(&handle);
        if let Err(error) = runner.stop(&handle) {
            tracing::warn!(?error, "failed to stop speedtest core process");
        }
    }
    if let Some(config_path) = config_path {
        remove_speedtest_config(&config_path);
    }
}

struct StartProbeCoreRequest<'request> {
    paths: &'request AppPaths,
    config_file_name: &'request str,
    core_type: CoreType,
    entries: &'request [SpeedtestConfigEntry],
    core_seed_resource_dir: Option<&'request PathBuf>,
    target_os: TargetOs,
    runner: &'request dyn ProcessRunner,
}

struct StartedProbeCore {
    handle: ProcessHandle,
    config_path: PathBuf,
}

fn start_probe_core(request: StartProbeCoreRequest<'_>) -> Result<StartedProbeCore> {
    let config_path = write_speedtest_config(
        request.paths,
        request.config_file_name,
        request.core_type,
        request.entries,
    )?;
    match spawn_probe_core(&request) {
        Ok(handle) => Ok(StartedProbeCore {
            handle,
            config_path,
        }),
        Err(error) => {
            // Nothing owns the config once the spawn failed, so drop it here
            // instead of leaving it for the next launch to sweep.
            remove_speedtest_config(&config_path);
            Err(error)
        }
    }
}

fn spawn_probe_core(request: &StartProbeCoreRequest<'_>) -> Result<ProcessHandle> {
    let core_info = get_core_info(request.core_type)
        .ok_or(SpeedtestError::MissingCoreInfo(request.core_type))?;
    // A probe core must be the same binary the runtime would launch, so the
    // resolution (packaged macOS seed, else stage-then-discover) is shared with
    // `runtime` rather than copied here.
    let executable = resolve_core_executable(
        request.paths,
        request.core_seed_resource_dir.map(PathBuf::as_path),
        core_info,
        request.core_type,
        request.target_os,
    )?;
    let launch = core_launch_plan(
        request.core_type,
        executable,
        request.paths,
        request.config_file_name,
    )
    .ok_or(SpeedtestError::MissingCoreInfo(request.core_type))?;
    let spawn = ProcessSpawn::from_core_launch(ProcessRole::Probe, &launch, true)?;

    request.runner.spawn(spawn).map_err(Into::into)
}

fn write_speedtest_config(
    paths: &AppPaths,
    file_name: &str,
    core_type: CoreType,
    entries: &[SpeedtestConfigEntry],
) -> Result<PathBuf> {
    let _ = core_type;
    let json = generate_singbox_speedtest_config_json(entries)?;
    // Speedtest configs carry the same outbound credentials as the runtime
    // config, so they go through the same 0600 + O_NOFOLLOW writer.
    write_core_config(paths, file_name, &json).map_err(|error| SpeedtestError::WriteConfig {
        path: error.path,
        source: error.source,
    })
}

fn remove_speedtest_config(path: &Path) {
    if let Err(error) = filesystem::remove_file_if_exists(path) {
        tracing::warn!(
            path = %path.display(),
            ?error,
            "failed to remove speedtest config"
        );
    }
}

pub(super) fn cleanup_stale_speedtest_configs(paths: &AppPaths) {
    if let Err(error) =
        filesystem::remove_matching_files(paths.bin_config_dir(), SPEEDTEST_CONFIG_PREFIX, ".json")
    {
        tracing::warn!(
            path = %paths.bin_config_dir().display(),
            ?error,
            "failed to remove stale speedtest configs"
        );
    }
}

/// Reserves one free loopback port per requested start port. Binding probes are
/// blocking syscalls, so the whole selection is reserved on a blocking thread
/// rather than once per item on a Tokio worker. Per-port failures are returned
/// individually so one unusable profile cannot abort the run.
pub(super) async fn reserve_speedtest_ports(starts: Vec<i32>) -> Result<Vec<Result<u16>>> {
    task::spawn_blocking(move || {
        let mut used_ports = HashSet::new();
        starts
            .into_iter()
            .map(|start| find_free_speedtest_port(start, &mut used_ports))
            .collect::<Vec<_>>()
    })
    .await
    .map_err(background_task_failed)
}

fn find_free_speedtest_port(start: i32, used_ports: &mut HashSet<u16>) -> Result<u16> {
    let mut port = u16::try_from(start).map_err(|_| SpeedtestError::InvalidSocksPort(start))?;
    loop {
        if !used_ports.contains(&port) && local_port_available(port) {
            used_ports.insert(port);
            return Ok(port);
        }
        if port == u16::MAX {
            return Err(SpeedtestError::NoAvailablePort(start));
        }
        port = port.saturating_add(1);
    }
}

fn local_port_available(port: u16) -> bool {
    TcpListener::bind((LOOPBACK_ADDR, port)).is_ok()
}

async fn wait_for_speedtest_ports(ports: &[i32], cancel: &CancellationFlag) -> Result<()> {
    let mut socks_ports = Vec::with_capacity(ports.len());
    for port in ports {
        socks_ports
            .push(u16::try_from(*port).map_err(|_| SpeedtestError::InvalidSocksPort(*port))?);
    }
    let budget = speedtest_ready_timeout(socks_ports.len());
    let started = Instant::now();
    // sing-box opens its inbounds in order, so a confirmed prefix never has to
    // be probed again — a 1000-inbound page would otherwise re-probe every
    // ready port on each poll.
    let mut ready = 0_usize;
    loop {
        check_cancelled(cancel)?;
        while let Some(port) = socks_ports.get(ready) {
            if !tcp_port_is_open(LOOPBACK_ADDR, *port).await {
                break;
            }
            ready += 1;
        }
        if ready == socks_ports.len() {
            return Ok(());
        }
        if started.elapsed() >= budget {
            return Err(SpeedtestError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "temporary speedtest core opened {ready} of {} local SOCKS ports within {budget:?}",
                    socks_ports.len()
                ),
            )));
        }
        time::sleep(SPEEDTEST_READY_INTERVAL).await;
    }
}

fn speedtest_ready_timeout(entry_count: usize) -> Duration {
    let scaled =
        SPEEDTEST_READY_PER_ENTRY.saturating_mul(u32::try_from(entry_count).unwrap_or(u32::MAX));
    SPEEDTEST_READY_TIMEOUT
        .saturating_add(scaled)
        .min(SPEEDTEST_READY_TIMEOUT_MAX)
}

fn background_task_failed(error: task::JoinError) -> SpeedtestError {
    SpeedtestError::BackgroundTask(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{fs, net::TcpListener as StdTcpListener};

    use voya_platform::{
        coreinfo::{core_type_dir_name, executable_name_for_current_os},
        paths::core_seed_resources_dir,
        process::{ProcessError, ProcessOutput},
        test_support::RecordingRunner,
    };

    use super::*;

    fn test_paths() -> AppPaths {
        AppPaths::new(
            std::env::temp_dir().join(format!("voyavpn-speedtest-core-{}", uuid::Uuid::new_v4())),
        )
    }

    /// Writes a fake packaged sing-box next to the app so both the macOS
    /// "run the seed directly" path and the copy-then-discover path resolve.
    fn seed_core_binary(paths: &AppPaths) -> (PathBuf, PathBuf) {
        let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
        let seed_exe = seed_root
            .join(core_type_dir_name(CoreType::sing_box))
            .join(executable_name_for_current_os("sing-box"));
        fs::create_dir_all(seed_exe.parent().expect("seed core dir"))
            .expect("speedtest test operation should succeed");
        fs::write(&seed_exe, b"seed-sing-box").expect("speedtest test operation should succeed");

        (seed_root, seed_exe)
    }

    fn speedtest_config_count(paths: &AppPaths) -> usize {
        fs::read_dir(paths.bin_config_dir())
            .map(|entries| {
                entries
                    .filter_map(std::result::Result::ok)
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(SPEEDTEST_CONFIG_PREFIX)
                    })
                    .count()
            })
            .unwrap_or_default()
    }

    /// A runner whose `spawn` always fails, so the config-cleanup path after a
    /// failed launch can be asserted.
    struct FailingRunner;

    impl ProcessRunner for FailingRunner {
        fn spawn(&self, request: ProcessSpawn) -> std::result::Result<ProcessHandle, ProcessError> {
            Err(spawn_failure(request))
        }

        fn run_oneshot(
            &self,
            request: ProcessSpawn,
        ) -> std::result::Result<ProcessOutput, ProcessError> {
            Err(spawn_failure(request))
        }

        fn stop(&self, _handle: &ProcessHandle) -> std::result::Result<(), ProcessError> {
            Ok(())
        }
    }

    fn spawn_failure(request: ProcessSpawn) -> ProcessError {
        ProcessError::Spawn {
            executable: request.executable,
            source: io::Error::new(io::ErrorKind::NotFound, "no probe core"),
        }
    }

    #[tokio::test]
    async fn process_speedtest_core_backend_uses_packaged_seed_directly_on_macos() {
        let paths = test_paths();
        let (seed_root, seed_exe) = seed_core_binary(&paths);
        let runner = RecordingRunner::default();
        let backend = ProcessSpeedtestCoreBackend::new(
            paths.clone(),
            Some(seed_root),
            Arc::new(runner.clone()),
        )
        .with_target_os(TargetOs::Macos);

        backend
            .start(
                CoreType::sing_box,
                Vec::new(),
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .expect("speedtest test operation should succeed");

        let app_data_exe = paths.core_bin_file(
            core_type_dir_name(CoreType::sing_box),
            executable_name_for_current_os("sing-box"),
        );
        let spawns = runner.spawns();
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].executable, seed_exe);
        assert!(!app_data_exe.exists());
    }

    #[tokio::test]
    async fn process_speedtest_core_backend_stop_all_reaps_live_probe_cores() {
        let paths = test_paths();
        let (seed_root, _) = seed_core_binary(&paths);
        let runner = RecordingRunner::default();
        let backend = ProcessSpeedtestCoreBackend::new(
            paths.clone(),
            Some(seed_root),
            Arc::new(runner.clone()),
        );

        let session = backend
            .start(
                CoreType::sing_box,
                Vec::new(),
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .expect("speedtest test operation should succeed");
        assert_eq!(runner.spawns().len(), 1, "one probe core was spawned");
        assert!(runner.stops().is_empty(), "a live session is not stopped");
        assert_eq!(speedtest_config_count(&paths), 1);

        backend.stop_all();

        assert_eq!(runner.stops().len(), 1, "every live probe core is reaped");
        assert_eq!(
            speedtest_config_count(&paths),
            0,
            "shutdown removes the probe config too"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn process_speedtest_core_backend_close_deregisters_the_session() {
        let paths = test_paths();
        let (seed_root, _) = seed_core_binary(&paths);
        let runner = RecordingRunner::default();
        let backend = ProcessSpeedtestCoreBackend::new(
            paths.clone(),
            Some(seed_root),
            Arc::new(runner.clone()),
        );

        backend
            .start(
                CoreType::sing_box,
                Vec::new(),
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .expect("speedtest test operation should succeed")
            .close()
            .await;

        assert_eq!(runner.stops().len(), 1, "close stops the probe core");
        assert_eq!(speedtest_config_count(&paths), 0);

        backend.stop_all();

        assert_eq!(
            runner.stops().len(),
            1,
            "a closed session must not be reaped twice"
        );
    }

    #[tokio::test]
    async fn process_speedtest_core_backend_removes_config_when_spawn_fails() {
        let paths = test_paths();
        let (seed_root, _) = seed_core_binary(&paths);
        let backend = ProcessSpeedtestCoreBackend::new(
            paths.clone(),
            Some(seed_root),
            Arc::new(FailingRunner),
        );

        // `Box<dyn SpeedtestCoreSession>` is not `Debug`, so unwrap the error by
        // hand rather than through `expect_err`.
        let error = match backend
            .start(
                CoreType::sing_box,
                Vec::new(),
                Arc::new(AtomicBool::new(false)),
            )
            .await
        {
            Ok(_) => panic!("a failing spawn must surface as an error"),
            Err(error) => error,
        };

        assert!(matches!(error, SpeedtestError::Process(_)));
        assert_eq!(
            speedtest_config_count(&paths),
            0,
            "a failed spawn must not leave a config behind"
        );
    }

    #[test]
    fn cleanup_stale_speedtest_configs_removes_only_speedtest_json_files() {
        let paths = test_paths();
        fs::create_dir_all(paths.bin_config_dir())
            .expect("speedtest test operation should succeed");
        let stale = paths.bin_config_file("configTest-old.json");
        let current_style_stale = paths.bin_config_file("configTest-123.json");
        let runtime_config = paths.bin_config_file("config.json");
        let similar_name = paths.bin_config_file("configTest-not-json.txt");
        fs::write(&stale, "{}").expect("speedtest test operation should succeed");
        fs::write(&current_style_stale, "{}").expect("speedtest test operation should succeed");
        fs::write(&runtime_config, "{}").expect("speedtest test operation should succeed");
        fs::write(&similar_name, "{}").expect("speedtest test operation should succeed");

        cleanup_stale_speedtest_configs(&paths);

        assert!(!stale.exists());
        assert!(!current_style_stale.exists());
        assert!(runtime_config.exists());
        assert!(similar_name.exists());
    }

    #[test]
    fn speedtest_ready_timeout_scales_with_the_batch_and_stays_bounded() {
        assert_eq!(speedtest_ready_timeout(0), SPEEDTEST_READY_TIMEOUT);
        assert_eq!(
            speedtest_ready_timeout(200),
            SPEEDTEST_READY_TIMEOUT + Duration::from_secs(1)
        );
        assert_eq!(
            speedtest_ready_timeout(100_000),
            SPEEDTEST_READY_TIMEOUT_MAX
        );
    }

    #[tokio::test]
    async fn wait_for_speedtest_ports_returns_once_every_port_answers() {
        let listener = StdTcpListener::bind((LOOPBACK_ADDR, 0))
            .expect("speedtest test operation should succeed");
        let port = i32::from(
            listener
                .local_addr()
                .expect("speedtest test operation should succeed")
                .port(),
        );

        wait_for_speedtest_ports(&[port], &Arc::new(AtomicBool::new(false)))
            .await
            .expect("an open port must be reported ready");
    }

    #[tokio::test]
    async fn wait_for_speedtest_ports_rejects_a_cancelled_run_before_probing() {
        let error = wait_for_speedtest_ports(&[1], &Arc::new(AtomicBool::new(true)))
            .await
            .expect_err("a cancelled run must not wait for ports");

        assert!(matches!(error, SpeedtestError::Cancelled));
    }

    #[tokio::test]
    async fn reserve_speedtest_ports_reports_failures_per_item() {
        let reserved = reserve_speedtest_ports(vec![0, -1])
            .await
            .expect("speedtest test operation should succeed");

        assert_eq!(reserved.len(), 2);
        assert!(reserved[0].is_ok());
        assert!(matches!(
            &reserved[1],
            Err(SpeedtestError::InvalidSocksPort(-1))
        ));
    }
}
