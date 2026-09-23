//! The throwaway cores that back profile speed tests while disconnected.
//!
//! [`ProcessProbeCoreLauncher`] is the desktop's launcher: a sing-box child
//! process per probe run. A phone may not spawn a child at all, so its host
//! runs Libbox in the app process and supplies its own [`ProbeCoreLauncher`].
//! Everything around starting a core — generating the config, waiting for the
//! SOCKS ports, tearing the core down — is portable and lives in
//! `SpeedtestManager`.
//!
//! Starting a probe core writes a config file, may copy the packaged seed
//! binary and spawns a child process — all blocking syscalls — so the sequence
//! runs on `spawn_blocking` instead of parking one Tokio worker per concurrent
//! probe. Every live child is also registered with the launcher: Tauri ends the
//! process through `std::process::exit`, so neither `Drop` nor the pending
//! `run_speedtest` future ever reaps a probe core, and `SpeedtestManager`
//! has to kill them synchronously on shutdown.

use std::path::Path;

use tokio::task;
use voya_platform::coreinfo::core_launch;

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

/// The desktop's launcher: a sing-box child process per probe run.
#[derive(Clone)]
pub struct ProcessProbeCoreLauncher {
    paths: AppPaths,
    core_seed_resource_dir: Option<PathBuf>,
    runner: Arc<dyn ProcessRunner>,
    target_os: TargetOs,
    live: LiveProbeCores,
}

impl ProcessProbeCoreLauncher {
    #[must_use]
    pub fn new(
        paths: AppPaths,
        core_seed_resource_dir: Option<PathBuf>,
        runner: Arc<dyn ProcessRunner>,
    ) -> Self {
        // A run that died with the process leaves its config behind, and only
        // a launcher that writes them knows to sweep them.
        cleanup_stale_speedtest_configs(&paths);
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

impl ProbeCoreLauncher for ProcessProbeCoreLauncher {
    fn start(&self, config_json: String) -> Result<Box<dyn ProbeCore>> {
        let config_file_name = format!("{SPEEDTEST_CONFIG_PREFIX}-{}.json", uuid::Uuid::new_v4());
        let started = start_probe_core(StartProbeCoreRequest {
            paths: &self.paths,
            config_file_name: &config_file_name,
            config_json: &config_json,
            core_seed_resource_dir: self.core_seed_resource_dir.as_ref(),
            target_os: self.target_os,
            runner: self.runner.as_ref(),
        })?;

        self.live.register(LiveProbeCore {
            handle: started.handle.clone(),
            config_path: started.config_path.clone(),
        });

        Ok(Box::new(ProcessProbeCore {
            config_path: Some(started.config_path),
            handle: Some(started.handle),
            runner: Arc::clone(&self.runner),
            live: self.live.clone(),
        }))
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

struct ProcessProbeCore {
    config_path: Option<PathBuf>,
    handle: Option<ProcessHandle>,
    runner: Arc<dyn ProcessRunner>,
    live: LiveProbeCores,
}

impl ProbeCore for ProcessProbeCore {
    fn stop(mut self: Box<Self>) {
        stop_probe_core(
            &self.live,
            self.runner.as_ref(),
            self.handle.take(),
            self.config_path.take(),
        );
    }
}

impl Drop for ProcessProbeCore {
    fn drop(&mut self) {
        // `stop` empties both slots, so this only fires for a core nobody
        // stopped — a panic, or a `?` between start and session.
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
    config_json: &'request str,
    core_seed_resource_dir: Option<&'request PathBuf>,
    target_os: TargetOs,
    runner: &'request dyn ProcessRunner,
}

struct StartedProbeCore {
    handle: ProcessHandle,
    config_path: PathBuf,
}

fn start_probe_core(request: StartProbeCoreRequest<'_>) -> Result<StartedProbeCore> {
    let config_path =
        write_speedtest_config(request.paths, request.config_file_name, request.config_json)?;
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
    // A probe core must be the same binary the runtime would launch, so the
    // resolution (packaged macOS seed, else stage-then-discover) is shared with
    // `runtime` rather than copied here.
    let executable = resolve_core_executable(
        request.paths,
        request.core_seed_resource_dir.map(PathBuf::as_path),
        request.target_os,
    )?;
    let launch = core_launch(executable, request.paths, request.config_file_name);
    let spawn = ProcessSpawn::from_core_launch(ProcessRole::Probe, &launch, true)?;

    request.runner.spawn(spawn).map_err(Into::into)
}

fn write_speedtest_config(paths: &AppPaths, file_name: &str, json: &str) -> Result<PathBuf> {
    // Speedtest configs carry the same outbound credentials as the runtime
    // config, so they go through the same 0600 + O_NOFOLLOW writer.
    write_core_config(paths, file_name, json).map_err(|error| SpeedtestError::WriteConfig {
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

fn cleanup_stale_speedtest_configs(paths: &AppPaths) {
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

pub(super) async fn wait_for_speedtest_ports(
    ports: &[i32],
    cancel: &CancellationFlag,
) -> Result<()> {
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

pub(super) fn background_task_failed(error: task::JoinError) -> SpeedtestError {
    SpeedtestError::BackgroundTask(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{fs, net::TcpListener as StdTcpListener};

    use voya_core::CoreConfigContext;
    use voya_platform::{
        coreinfo::{executable_name_for_current_os, CORE_DIR_NAME},
        paths::core_seed_resources_dir,
        process::{ProcessError, ProcessOutput},
        test_support::RecordingRunner,
    };

    use super::*;
    use crate::speedtest::start_probe_core_page;

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
            .join(CORE_DIR_NAME)
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
    async fn probe_core_launcher_launches_the_staged_seed() {
        let paths = test_paths();
        let (seed_root, _) = seed_core_binary(&paths);
        let runner = RecordingRunner::default();
        let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(
            ProcessProbeCoreLauncher::new(paths.clone(), Some(seed_root), Arc::new(runner.clone()))
                .with_target_os(TargetOs::Linux),
        );

        start_probe_core_page(&launcher, &[], &Arc::new(AtomicBool::new(false)))
            .await
            .expect("speedtest test operation should succeed");

        let app_data_exe =
            paths.core_bin_file(CORE_DIR_NAME, executable_name_for_current_os("sing-box"));
        let spawns = runner.spawns();
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].executable, app_data_exe);
        assert!(app_data_exe.exists());
    }

    /// macOS launches the seed inside the signed bundle instead of a copy.
    #[tokio::test]
    async fn probe_core_launcher_uses_packaged_seed_directly_on_macos() {
        let paths = test_paths();
        let (seed_root, seed_exe) = seed_core_binary(&paths);
        let runner = RecordingRunner::default();
        let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(
            ProcessProbeCoreLauncher::new(paths.clone(), Some(seed_root), Arc::new(runner.clone()))
                .with_target_os(TargetOs::Macos),
        );

        start_probe_core_page(&launcher, &[], &Arc::new(AtomicBool::new(false)))
            .await
            .expect("speedtest test operation should succeed");

        let app_data_exe =
            paths.core_bin_file(CORE_DIR_NAME, executable_name_for_current_os("sing-box"));
        let spawns = runner.spawns();
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].executable, seed_exe);
        assert!(!app_data_exe.exists());
    }

    #[tokio::test]
    async fn probe_core_launcher_stop_all_reaps_live_probe_cores() {
        let paths = test_paths();
        let (seed_root, _) = seed_core_binary(&paths);
        let runner = RecordingRunner::default();
        let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(ProcessProbeCoreLauncher::new(
            paths.clone(),
            Some(seed_root),
            Arc::new(runner.clone()),
        ));

        let session = start_probe_core_page(&launcher, &[], &Arc::new(AtomicBool::new(false)))
            .await
            .expect("speedtest test operation should succeed");
        assert_eq!(runner.spawns().len(), 1, "one probe core was spawned");
        assert!(runner.stops().is_empty(), "a live session is not stopped");
        assert_eq!(speedtest_config_count(&paths), 1);

        launcher.stop_all();

        assert_eq!(runner.stops().len(), 1, "every live probe core is reaped");
        assert_eq!(
            speedtest_config_count(&paths),
            0,
            "shutdown removes the probe config too"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn probe_core_launcher_close_deregisters_the_session() {
        let paths = test_paths();
        let (seed_root, _) = seed_core_binary(&paths);
        let runner = RecordingRunner::default();
        let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(ProcessProbeCoreLauncher::new(
            paths.clone(),
            Some(seed_root),
            Arc::new(runner.clone()),
        ));

        start_probe_core_page(&launcher, &[], &Arc::new(AtomicBool::new(false)))
            .await
            .expect("speedtest test operation should succeed")
            .close()
            .await;

        assert_eq!(runner.stops().len(), 1, "close stops the probe core");
        assert_eq!(speedtest_config_count(&paths), 0);

        launcher.stop_all();

        assert_eq!(
            runner.stops().len(),
            1,
            "a closed session must not be reaped twice"
        );
    }

    #[tokio::test]
    async fn probe_core_launcher_removes_config_when_spawn_fails() {
        let paths = test_paths();
        let (seed_root, _) = seed_core_binary(&paths);
        let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(ProcessProbeCoreLauncher::new(
            paths.clone(),
            Some(seed_root),
            Arc::new(FailingRunner),
        ));

        // `ProbeCoreSession` is not `Debug`, so unwrap the error by
        // hand rather than through `expect_err`.
        let error =
            match start_probe_core_page(&launcher, &[], &Arc::new(AtomicBool::new(false))).await {
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

    /// A launcher that spawns nothing, the shape the mobile host has.
    #[derive(Clone, Default)]
    struct RecordingLauncher {
        started: Arc<Mutex<Vec<String>>>,
        stopped: Arc<Mutex<usize>>,
        fail: bool,
    }

    struct RecordingCore {
        stopped: Arc<Mutex<usize>>,
    }

    impl ProbeCore for RecordingCore {
        fn stop(self: Box<Self>) {
            *lock_ignoring_poison(&self.stopped) += 1;
        }
    }

    impl ProbeCoreLauncher for RecordingLauncher {
        fn start(&self, config_json: String) -> Result<Box<dyn ProbeCore>> {
            if self.fail {
                return Err(SpeedtestError::EmptySelection);
            }
            lock_ignoring_poison(&self.started).push(config_json);

            Ok(Box::new(RecordingCore {
                stopped: Arc::clone(&self.stopped),
            }))
        }
    }

    #[tokio::test]
    async fn launcher_core_backend_hands_the_generated_config_to_the_launcher() {
        let handle = RecordingLauncher::default();
        let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(handle.clone());

        let session = start_probe_core_page(&launcher, &[], &Arc::new(AtomicBool::new(false)))
            .await
            .expect("speedtest test operation should succeed");

        {
            let started = lock_ignoring_poison(&handle.started);
            assert_eq!(started.len(), 1, "one core per run");
            // The launcher receives sing-box JSON, not the entries: a host that
            // runs Libbox in-process has nowhere to put a file.
            assert!(
                started[0].starts_with('{'),
                "a generated config: {}",
                started[0]
            );
        }

        session.close().await;
        assert_eq!(*lock_ignoring_poison(&handle.stopped), 1);
    }

    #[tokio::test]
    async fn launcher_core_backend_stops_a_core_the_run_abandoned() {
        let handle = RecordingLauncher::default();
        let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(handle.clone());

        // A port no SOCKS listener can ever answer on. The session is bound
        // before the readiness wait, so the core that did start is still torn
        // down on the way out rather than left running.
        let entries = vec![SpeedtestConfigEntry {
            index_id: "unreachable".to_owned(),
            port: -1,
            context: CoreConfigContext::default(),
        }];
        let error =
            match start_probe_core_page(&launcher, &entries, &Arc::new(AtomicBool::new(false)))
                .await
            {
                Ok(_) => panic!("an unusable port must not produce a session"),
                Err(error) => error,
            };

        assert!(matches!(error, SpeedtestError::InvalidSocksPort(-1)));
        assert_eq!(*lock_ignoring_poison(&handle.stopped), 1);
    }

    #[tokio::test]
    async fn launcher_core_backend_surfaces_a_launcher_failure() {
        let launcher: Arc<dyn ProbeCoreLauncher> = Arc::new(RecordingLauncher {
            fail: true,
            ..RecordingLauncher::default()
        });

        let error =
            match start_probe_core_page(&launcher, &[], &Arc::new(AtomicBool::new(false))).await {
                Ok(_) => panic!("a failing launcher must surface as an error"),
                Err(error) => error,
            };

        assert!(matches!(error, SpeedtestError::EmptySelection));
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
