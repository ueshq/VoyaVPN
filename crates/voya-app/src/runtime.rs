use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use thiserror::Error;
use tokio::sync::Mutex;
use voya_core::{
    generate_singbox_config_json, AppConfig, ContextBuildError, CoreConfigContext,
    CoreConfigContextBuilder, CoreConfigContextBuilderAllResult, CoreGenPlatform, CoreType,
    SingboxConfigError,
};
use voya_db::{Database, DbError};
use voya_platform::{
    coreinfo::{
        all_core_infos, copy_seed_core_asset, discover_executable,
        discover_packaged_seed_executable, get_core_info, CoreInfo, CoreInfoError, CoreLaunch,
        CoreSeedCopyOutcome, TargetOs,
    },
    filesystem,
    paths::{AppPaths, PathError},
    tun::{tun_backend, TunBackend},
};

use crate::coregen::{SnapshotCoreGenData, SnapshotCoreGenEnv};
use crate::supervisor::{
    CoreProcessSpec, CoreSupervisor, SupervisorConnectionState, SupervisorError,
    SupervisorSnapshot, SupervisorStartRequest,
};
use crate::updates::local_singbox_ruleset_paths;

pub const MAIN_CONFIG_FILE_NAME: &str = "config.json";
pub const PRE_CONFIG_FILE_NAME: &str = "configPre.json";
const SUDO_SCRIPT_DIR_NAME: &str = "sudo";

#[must_use]
pub fn supported_core_infos() -> &'static [CoreInfo] {
    all_core_infos()
}

#[must_use]
pub fn core_launch_plan(
    core_type: CoreType,
    executable: impl Into<PathBuf>,
    paths: &AppPaths,
    config_file: impl AsRef<Path>,
) -> Option<CoreLaunch> {
    get_core_info(core_type)
        .map(|core_info| core_info.resolve_launch(executable, paths, config_file))
}

#[derive(Clone)]
pub struct RuntimeManager<'runtime> {
    database: &'runtime Database,
    paths: AppPaths,
    core_seed_resource_dir: Option<PathBuf>,
    supervisor: CoreSupervisor,
    /// Serializes connect/restart/disconnect.
    ///
    /// The supervisor actor already serializes process lifecycle, but the
    /// runtime config files live outside it: `connect` writes `config.json`
    /// before sending Start, and `disconnect` deletes it after Stop returns.
    /// Without this lock a disconnect could delete the config a queued connect
    /// is about to launch the core against.
    operation_lock: Arc<Mutex<()>>,
    target_os: TargetOs,
}

impl<'runtime> RuntimeManager<'runtime> {
    #[must_use]
    pub fn new(database: &'runtime Database, paths: AppPaths, supervisor: CoreSupervisor) -> Self {
        Self::with_target_os(database, paths, supervisor, TargetOs::current())
    }

    #[must_use]
    pub fn with_target_os(
        database: &'runtime Database,
        paths: AppPaths,
        supervisor: CoreSupervisor,
        target_os: TargetOs,
    ) -> Self {
        Self {
            database,
            paths,
            core_seed_resource_dir: None,
            supervisor,
            operation_lock: Arc::new(Mutex::new(())),
            target_os,
        }
    }

    #[must_use]
    pub fn with_core_seed_resource_dir(
        mut self,
        core_seed_resource_dir: impl Into<PathBuf>,
    ) -> Self {
        self.core_seed_resource_dir = Some(core_seed_resource_dir.into());
        self
    }

    /// Share one runtime-operation lock across every manager built from the
    /// same services handle. Managers are cheap and constructed per command, so
    /// the lock has to be handed in rather than owned per instance.
    #[must_use]
    pub fn with_operation_lock(mut self, operation_lock: Arc<Mutex<()>>) -> Self {
        self.operation_lock = operation_lock;
        self
    }

    pub async fn connect(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
        let _guard = self.operation_lock.lock().await;
        self.start_core(config).await
    }

    pub async fn restart(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
        let _guard = self.operation_lock.lock().await;
        self.start_core(config).await
    }

    /// Restart only while the supervisor is connected.
    ///
    /// The status check runs under the runtime lock, so a disconnect that
    /// lands between a caller's own check and this call cannot be silently
    /// undone by the restart. `None` means the core was already down.
    pub async fn restart_if_connected(
        &self,
        config: &AppConfig,
    ) -> Result<Option<SupervisorSnapshot>, RuntimeError> {
        let _guard = self.operation_lock.lock().await;
        let status = self.supervisor.status().await?;
        if status.state != SupervisorConnectionState::Connected {
            return Ok(None);
        }

        self.start_core(config).await.map(Some)
    }

    pub async fn disconnect(&self) -> Result<SupervisorSnapshot, RuntimeError> {
        let _guard = self.operation_lock.lock().await;
        let snapshot = self.supervisor.stop().await?;
        cleanup_runtime_state(&self.paths)?;

        Ok(snapshot)
    }

    pub async fn status(&self) -> Result<SupervisorSnapshot, RuntimeError> {
        // Deliberately unlocked: status is polled while a connect holds the
        // lock, and the failure-recovery paths query it to find out what the
        // supervisor really did.
        self.supervisor.status().await.map_err(Into::into)
    }

    async fn start_core(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
        self.paths.ensure_dirs()?;

        let active_profile_id = config.index_id.trim();
        if active_profile_id.is_empty() {
            return Err(RuntimeError::MissingActiveProfileId);
        }

        let active_profile = self
            .database
            .profiles()
            .get(active_profile_id)
            .await?
            .ok_or_else(|| RuntimeError::ActiveProfileNotFound(active_profile_id.to_string()))?;

        let env =
            load_runtime_core_gen_env(self.database, &self.paths, config, self.target_os).await?;
        let contexts = runtime_config_contexts(&env, config, &active_profile, self.target_os);
        if !contexts.success() {
            let validation = contexts.combined_validator_result();
            return Err(RuntimeError::Validation {
                errors: validation.errors,
                warnings: validation.warnings,
            });
        }

        let main_config_path = write_runtime_config(
            &self.paths,
            MAIN_CONFIG_FILE_NAME,
            &contexts.main_result.context,
        )?;
        let main_spec = self.process_spec(&contexts.main_result.context, MAIN_CONFIG_FILE_NAME)?;

        let pre = if let Some(pre_result) = &contexts.pre_socks_result {
            write_runtime_config(&self.paths, PRE_CONFIG_FILE_NAME, &pre_result.context)?;
            Some(self.process_spec(&pre_result.context, PRE_CONFIG_FILE_NAME)?)
        } else {
            cleanup_config_file(&self.paths, PRE_CONFIG_FILE_NAME)?;
            None
        };

        let request = SupervisorStartRequest {
            active_profile_id: Some(active_profile.index_id.clone()),
            main: main_spec,
            pre,
            tun_enabled: config.tun_mode_item.enable_tun,
            sudo_script_dir: self.paths.temp_dir().join(SUDO_SCRIPT_DIR_NAME),
            restart_on_crash: true,
        };

        match self.supervisor.start(request).await {
            Ok(snapshot) => Ok(snapshot),
            Err(error) => {
                let _ = filesystem::remove_file_if_exists(&main_config_path);
                let _ = cleanup_config_file(&self.paths, PRE_CONFIG_FILE_NAME);
                Err(error.into())
            }
        }
    }

    fn process_spec(
        &self,
        context: &CoreConfigContext,
        config_file_name: &str,
    ) -> Result<CoreProcessSpec, RuntimeError> {
        let core_type = context.run_core_type;
        let core_info = get_core_info(core_type).ok_or(RuntimeError::MissingCoreInfo(core_type))?;
        let executable = self.core_executable(core_type, core_info)?;
        let launch = core_launch_plan(core_type, executable, &self.paths, config_file_name)
            .ok_or(RuntimeError::MissingCoreInfo(core_type))?;

        // Only the process whose config carries the tun inbound needs root:
        // with a pre-socks split the main core does all remote I/O and must
        // stay unprivileged.
        Ok(CoreProcessSpec::new(core_type, launch)
            .with_config_path(self.paths.bin_config_file(config_file_name))
            .with_display_log(context.node.display_log)
            .with_may_need_sudo(context.is_tun_enabled))
    }

    fn core_executable(
        &self,
        core_type: CoreType,
        core_info: &CoreInfo,
    ) -> Result<PathBuf, RuntimeError> {
        if let Some(executable) = self.packaged_seed_executable(core_info)? {
            return Ok(executable);
        }

        self.copy_seed_core_asset(core_type)?;
        discover_executable(&self.paths, core_info).map_err(Into::into)
    }

    fn packaged_seed_executable(
        &self,
        core_info: &CoreInfo,
    ) -> Result<Option<PathBuf>, RuntimeError> {
        if self.target_os != TargetOs::Macos {
            return Ok(None);
        }

        let Some(seed_resource_dir) = &self.core_seed_resource_dir else {
            return Ok(None);
        };

        discover_packaged_seed_executable(seed_resource_dir, core_info, self.target_os)
            .map_err(Into::into)
    }

    fn copy_seed_core_asset(
        &self,
        core_type: CoreType,
    ) -> Result<Option<CoreSeedCopyOutcome>, RuntimeError> {
        let Some(seed_resource_dir) = &self.core_seed_resource_dir else {
            return Ok(None);
        };

        copy_seed_core_asset(&self.paths, seed_resource_dir, core_type)
            .map(Some)
            .map_err(Into::into)
    }
}

fn runtime_config_contexts(
    env: &SnapshotCoreGenEnv,
    config: &AppConfig,
    active_profile: &voya_core::ProfileItem,
    target_os: TargetOs,
) -> CoreConfigContextBuilderAllResult {
    let builder = CoreConfigContextBuilder::new(env);
    if should_use_single_native_tun_config(config, target_os) {
        return CoreConfigContextBuilderAllResult {
            main_result: builder.build(config, active_profile),
            pre_socks_result: None,
        };
    }

    builder.build_all(config, active_profile)
}

fn should_use_single_native_tun_config(config: &AppConfig, target_os: TargetOs) -> bool {
    config.tun_mode_item.enable_tun
        && matches!(
            tun_backend(target_os),
            TunBackend::MacosPacketTunnel | TunBackend::WindowsService
        )
}

fn write_runtime_config(
    paths: &AppPaths,
    file_name: &str,
    context: &CoreConfigContext,
) -> Result<PathBuf, RuntimeError> {
    let json = generate_singbox_config_json(context)?;
    let path = paths.bin_config_file(file_name);
    // The generated config embeds outbound credentials (passwords, UUIDs,
    // Reality private keys), so it is written 0600 and never through a symlink.
    filesystem::write_private_file_with_parent(&path, json).map_err(|source| {
        RuntimeError::WriteConfig {
            path: path.clone(),
            source,
        }
    })?;

    Ok(path)
}

fn cleanup_runtime_state(paths: &AppPaths) -> Result<(), RuntimeError> {
    cleanup_config_file(paths, MAIN_CONFIG_FILE_NAME)?;
    cleanup_config_file(paths, PRE_CONFIG_FILE_NAME)?;

    Ok(())
}

fn cleanup_config_file(paths: &AppPaths, file_name: &str) -> Result<(), RuntimeError> {
    let path = paths.bin_config_file(file_name);
    filesystem::remove_file_if_exists(&path)
        .map_err(|source| RuntimeError::RemoveConfig { path, source })
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("active profile id is empty")]
    MissingActiveProfileId,
    #[error("active profile {0} was not found")]
    ActiveProfileNotFound(String),
    #[error("runtime validation failed: {errors:?}; warnings: {warnings:?}")]
    Validation {
        errors: Vec<String>,
        warnings: Vec<String>,
    },
    #[error("no core info entry for {0:?}")]
    MissingCoreInfo(CoreType),
    #[error("failed to create runtime config directory {path}: {source}")]
    CreateConfigDir { path: PathBuf, source: io::Error },
    #[error("failed to write runtime config {path}: {source}")]
    WriteConfig { path: PathBuf, source: io::Error },
    #[error("failed to remove runtime config {path}: {source}")]
    RemoveConfig { path: PathBuf, source: io::Error },
    #[error(transparent)]
    ContextBuild(#[from] ContextBuildError),
    #[error(transparent)]
    SingboxConfig(#[from] SingboxConfigError),
    #[error(transparent)]
    CoreInfo(#[from] CoreInfoError),
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
}

pub(crate) async fn load_runtime_core_gen_env(
    database: &Database,
    paths: &AppPaths,
    config: &AppConfig,
    target_os: TargetOs,
) -> Result<SnapshotCoreGenEnv, DbError> {
    Ok(SnapshotCoreGenEnv::new(
        config,
        core_gen_platform(target_os),
        SnapshotCoreGenData {
            profiles: database.profiles().list().await?,
            routings: database.routings().list().await?,
            subs: database.subscriptions().list().await?,
        },
    )
    .with_singbox_ruleset_paths(local_singbox_ruleset_paths(paths)))
}

const fn core_gen_platform(target_os: TargetOs) -> CoreGenPlatform {
    match target_os {
        TargetOs::Windows => CoreGenPlatform::Windows,
        TargetOs::Macos => CoreGenPlatform::MacOS,
        TargetOs::Linux | TargetOs::Other => CoreGenPlatform::Linux,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use voya_core::{
        CoreType, ProfileItem, ProfileProtocol, ProfileTransport, RoutingItem, RuleType, RulesItem,
        ServerEndpoint,
    };
    use voya_db::Database;
    use voya_platform::{
        coreinfo::{core_type_dir_name, executable_name_for_current_os},
        paths::{core_seed_resources_dir, AppPaths},
        test_support::RecordingRunner,
        tun::{
            NativeTunController, NativeTunError, NativeTunProviderState, NativeTunStartRequest,
            NativeTunStatus, TunBackend,
        },
    };

    use super::*;
    use crate::supervisor::SupervisorDeps;

    static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn coreinfo_app_layer_exposes_singbox_only() {
        let infos = supported_core_infos();

        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].core_type, CoreType::sing_box);
    }

    #[test]
    fn coreinfo_app_layer_resolves_launch_command_and_env() {
        let paths = AppPaths::new("/tmp/VoyaVPN");
        let launch = core_launch_plan(
            CoreType::sing_box,
            "/tmp/VoyaVPN/bin/sing_box/sing-box",
            &paths,
            "config.json",
        )
        .expect("sing-box launch plan");

        assert_eq!(launch.arguments, "run -c config.json --disable-color");
        assert_eq!(launch.working_dir, paths.bin_config_dir());
        assert!(launch.environment.is_empty());
    }

    #[tokio::test]
    async fn runtime_connect_writes_generated_config_and_starts_supervisor_path() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        paths
            .ensure_dirs()
            .expect("runtime test operation should succeed");
        write_fake_core_executable(&paths, CoreType::sing_box);
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(runner.clone()),
            Arc::new(voya_platform::privilege::ElevationState::new()),
        ));
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, TargetOs::Linux);
        let mut config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        let profile = active_singbox_profile("active");
        database
            .profiles()
            .upsert(&profile)
            .await
            .expect("runtime test operation should succeed");

        let connected = manager
            .connect(&config)
            .await
            .expect("runtime test operation should succeed");

        assert_eq!(connected.active_profile_id.as_deref(), Some("active"));
        assert!(paths.bin_config_file(MAIN_CONFIG_FILE_NAME).exists());
        let spawns = runner.spawns();
        assert_eq!(spawns.len(), 1);
        assert_eq!(
            spawns[0].arguments,
            ["run", "-c", MAIN_CONFIG_FILE_NAME, "--disable-color"]
        );

        let disconnected = manager
            .disconnect()
            .await
            .expect("runtime test operation should succeed");

        assert_eq!(disconnected.state, SupervisorConnectionState::Disconnected);
        assert!(!paths.bin_config_file(MAIN_CONFIG_FILE_NAME).exists());
        assert_eq!(runner.stops().as_slice(), [10]);
        config.index_id.clear();
    }

    #[tokio::test]
    async fn runtime_elevates_only_the_process_that_owns_the_tun_inbound() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        paths
            .ensure_dirs()
            .expect("runtime test operation should succeed");
        write_fake_core_executable(&paths, CoreType::sing_box);
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(RecordingRunner::default()),
            Arc::new(voya_platform::privilege::ElevationState::new()),
        ));
        let manager = RuntimeManager::with_target_os(&database, paths, supervisor, TargetOs::Linux);
        let profile = active_singbox_profile("active");
        let config = AppConfig {
            index_id: "active".to_string(),
            tun_mode_item: voya_core::TunModeItem {
                enable_tun: true,
                ..voya_core::TunModeItem::default()
            },
            ..AppConfig::default()
        };
        let env = SnapshotCoreGenEnv::new(
            &config,
            CoreGenPlatform::Linux,
            SnapshotCoreGenData {
                profiles: vec![profile.clone()],
                ..SnapshotCoreGenData::default()
            },
        );

        let contexts = runtime_config_contexts(&env, &config, &profile, TargetOs::Linux);
        let pre_context = &contexts
            .pre_socks_result
            .as_ref()
            .expect("Linux TUN builds a pre-socks context")
            .context;
        assert!(!contexts.main_result.context.is_tun_enabled);
        assert!(pre_context.is_tun_enabled);

        let main_spec = manager
            .process_spec(&contexts.main_result.context, MAIN_CONFIG_FILE_NAME)
            .expect("runtime test operation should succeed");
        let pre_spec = manager
            .process_spec(pre_context, PRE_CONFIG_FILE_NAME)
            .expect("runtime test operation should succeed");

        assert!(
            !main_spec.may_need_sudo,
            "the non-TUN main core must not be launched as root"
        );
        assert!(pre_spec.may_need_sudo);
    }

    #[tokio::test]
    async fn runtime_macos_native_tun_writes_single_tun_config() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        paths
            .ensure_dirs()
            .expect("runtime test operation should succeed");
        write_fake_core_executable(&paths, CoreType::sing_box);
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(
            SupervisorDeps::new(
                Arc::new(runner.clone()),
                Arc::new(voya_platform::privilege::ElevationState::new()),
            )
            .with_target_os(TargetOs::Macos)
            .with_native_tun_controller(Arc::new(TestNativeTunController)),
        );
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, TargetOs::Macos);
        let config = AppConfig {
            index_id: "active".to_string(),
            tun_mode_item: voya_core::TunModeItem {
                enable_tun: true,
                ..voya_core::TunModeItem::default()
            },
            ..AppConfig::default()
        };
        database
            .profiles()
            .upsert(&active_singbox_profile("active"))
            .await
            .expect("runtime test operation should succeed");

        let connected = manager
            .connect(&config)
            .await
            .expect("runtime test operation should succeed");

        assert_eq!(connected.state, SupervisorConnectionState::Connected);
        assert_eq!(connected.main_pid, None);
        assert!(runner.spawns().is_empty());
        assert!(!paths.bin_config_file(PRE_CONFIG_FILE_NAME).exists());

        let generated = fs::read_to_string(paths.bin_config_file(MAIN_CONFIG_FILE_NAME))
            .expect("runtime test operation should succeed");
        let json: serde_json::Value =
            serde_json::from_str(&generated).expect("runtime test operation should succeed");
        assert!(
            json["inbounds"]
                .as_array()
                .expect("inbounds")
                .iter()
                .any(|inbound| inbound["type"] == "tun"),
            "native macOS TUN must pass a tun inbound to PacketTunnel"
        );
    }

    #[tokio::test]
    async fn coreinfo_runtime_connect_copies_seed_core_before_discovery() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
        let seed_exe = write_seed_core_executable(&seed_root, CoreType::sing_box, b"seed-sing-box");
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(runner.clone()),
            Arc::new(voya_platform::privilege::ElevationState::new()),
        ));
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, TargetOs::Linux)
                .with_core_seed_resource_dir(seed_root);
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        database
            .profiles()
            .upsert(&active_singbox_profile("active"))
            .await
            .expect("runtime test operation should succeed");

        manager
            .connect(&config)
            .await
            .expect("runtime test operation should succeed");

        let app_data_exe = paths.core_bin_file(
            core_type_dir_name(CoreType::sing_box),
            executable_name_for_current_os("sing-box"),
        );
        let spawns = runner.spawns();
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].executable, app_data_exe);
        assert_ne!(spawns[0].executable, seed_exe);
    }

    #[tokio::test]
    async fn coreinfo_runtime_connect_uses_packaged_seed_directly_on_macos() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
        let seed_exe = write_seed_core_executable(&seed_root, CoreType::sing_box, b"seed-sing-box");
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(
            SupervisorDeps::new(
                Arc::new(runner.clone()),
                Arc::new(voya_platform::privilege::ElevationState::new()),
            )
            .with_target_os(TargetOs::Macos),
        );
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, TargetOs::Macos)
                .with_core_seed_resource_dir(seed_root);
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        database
            .profiles()
            .upsert(&active_singbox_profile("active"))
            .await
            .expect("runtime test operation should succeed");

        manager
            .connect(&config)
            .await
            .expect("runtime test operation should succeed");

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
    async fn coreinfo_runtime_connect_missing_seed_surfaces_typed_missing_core() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(runner.clone()),
            Arc::new(voya_platform::privilege::ElevationState::new()),
        ));
        let manager = RuntimeManager::with_target_os(&database, paths, supervisor, TargetOs::Linux)
            .with_core_seed_resource_dir(seed_root);
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        database
            .profiles()
            .upsert(&active_singbox_profile("active"))
            .await
            .expect("runtime test operation should succeed");

        let error = manager.connect(&config).await.expect_err("missing core");

        match error {
            RuntimeError::CoreInfo(CoreInfoError::ExecutableNotFound { core_type, .. }) => {
                assert_eq!(core_type, CoreType::sing_box);
            }
            other => panic!("expected typed missing core error, got {other:?}"),
        }
        assert!(runner.spawns().is_empty());
    }

    #[tokio::test]
    async fn runtime_connect_uses_active_routing_rules_from_database() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        paths
            .ensure_dirs()
            .expect("runtime test operation should succeed");
        write_fake_core_executable(&paths, CoreType::sing_box);
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(runner),
            Arc::new(voya_platform::privilege::ElevationState::new()),
        ));
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, TargetOs::Linux);
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        let profile = active_singbox_profile("active");
        let routing = RoutingItem {
            id: "routing-active".to_string(),
            remarks: "Active routing".to_string(),
            rule_set: vec![RulesItem {
                id: "rule-direct".to_string(),
                outbound_tag: Some(voya_core::DIRECT_TAG.to_string()),
                domain: Some(vec!["full:direct.example.com".to_string()]),
                rule_type: Some(RuleType::Routing),
                ..RulesItem::default()
            }],
            ..RoutingItem::default()
        };
        database
            .profiles()
            .upsert(&profile)
            .await
            .expect("runtime test operation should succeed");
        database
            .routings()
            .upsert(&routing)
            .await
            .expect("runtime test operation should succeed");

        manager
            .connect(&config)
            .await
            .expect("runtime test operation should succeed");

        let generated = fs::read_to_string(paths.bin_config_file(MAIN_CONFIG_FILE_NAME))
            .expect("runtime test operation should succeed");
        let json: serde_json::Value =
            serde_json::from_str(&generated).expect("runtime test operation should succeed");
        let rules = json["route"]["rules"]
            .as_array()
            .expect("runtime test operation should succeed");
        assert!(rules.iter().any(|rule| {
            rule["outbound"] == "direct"
                && rule["domain"].as_array().is_some_and(|domains| {
                    domains.iter().any(|domain| domain == "direct.example.com")
                })
        }));
    }

    fn temp_paths() -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("runtime test operation should succeed")
            .as_nanos();
        let counter = TEMP_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
        AppPaths::new(
            std::env::temp_dir()
                .join("voyavpn-runtime-tests")
                .join(format!("{}-{nanos}-{counter}", std::process::id())),
        )
    }

    fn write_fake_core_executable(paths: &AppPaths, core_type: CoreType) {
        let core_info = get_core_info(core_type).expect("core info");
        let executable_name = executable_name_for_current_os(core_info.executable_names()[0]);
        let executable = paths.core_bin_file(core_type_dir_name(core_type), executable_name);
        fs::create_dir_all(executable.parent().expect("core dir"))
            .expect("runtime test operation should succeed");
        fs::write(executable, b"fake").expect("runtime test operation should succeed");
    }

    fn write_seed_core_executable(
        seed_root: &Path,
        core_type: CoreType,
        contents: &[u8],
    ) -> PathBuf {
        let core_info = get_core_info(core_type).expect("core info");
        let executable_name = executable_name_for_current_os(core_info.executable_names()[0]);
        let executable = seed_root
            .join(core_type_dir_name(core_type))
            .join(executable_name);
        fs::create_dir_all(executable.parent().expect("seed core dir"))
            .expect("runtime test operation should succeed");
        fs::write(&executable, contents).expect("runtime test operation should succeed");
        executable
    }

    fn active_singbox_profile(index_id: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            remarks: "Runtime".to_string(),
            protocol: ProfileProtocol::Vless {
                server: ServerEndpoint {
                    address: "example.test".to_string(),
                    port: 443,
                },
                uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                flow: None,
                encryption: Some("none".to_string()),
            },
            transport: Some(ProfileTransport::Tcp {
                header: None,
                host: None,
                path: None,
            }),
            ..ProfileItem::default()
        }
    }

    struct TestNativeTunController;

    impl NativeTunController for TestNativeTunController {
        fn status(&self, backend: TunBackend) -> NativeTunStatus {
            NativeTunStatus {
                backend,
                provider_state: NativeTunProviderState::Stopped,
                component_ready: true,
                message: None,
            }
        }

        fn start(&self, _request: NativeTunStartRequest) -> Result<(), NativeTunError> {
            Ok(())
        }

        fn stop(&self, _backend: TunBackend) -> Result<(), NativeTunError> {
            Ok(())
        }
    }
}
