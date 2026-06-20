use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;
use voya_core::{
    generate_singbox_config_json, generate_xray_config_json, AppConfig, ContextBuildError,
    CoreConfigContext, CoreConfigContextBuilder, CoreGenPlatform, CoreType, SingboxConfigError,
};
use voya_db::{Database, DbError};
use voya_platform::{
    coreinfo::{
        all_core_infos, copy_seed_core_asset, discover_executable, get_core_info, CoreInfo,
        CoreInfoError, CoreLaunch, CoreSeedCopyOutcome, TargetOs,
    },
    paths::{AppPaths, PathError},
};

use crate::coregen::{CoreTypeFallback, SnapshotCoreGenEnv};
use crate::supervisor::{
    CoreProcessSpec, CoreSupervisor, SupervisorError, SupervisorSnapshot, SupervisorStartRequest,
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

    pub async fn connect(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
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
        let contexts = CoreConfigContextBuilder::new(&env).build_all(config, &active_profile);
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
                let _ = fs::remove_file(main_config_path);
                let _ = cleanup_config_file(&self.paths, PRE_CONFIG_FILE_NAME);
                Err(error.into())
            }
        }
    }

    pub async fn restart(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
        self.connect(config).await
    }

    pub async fn disconnect(&self) -> Result<SupervisorSnapshot, RuntimeError> {
        let snapshot = self.supervisor.stop().await?;
        cleanup_runtime_state(&self.paths)?;

        Ok(snapshot)
    }

    pub async fn status(&self) -> Result<SupervisorSnapshot, RuntimeError> {
        self.supervisor.status().await.map_err(Into::into)
    }

    fn process_spec(
        &self,
        context: &CoreConfigContext,
        config_file_name: &str,
    ) -> Result<CoreProcessSpec, RuntimeError> {
        let core_type = context.run_core_type;
        let core_info = get_core_info(core_type).ok_or(RuntimeError::MissingCoreInfo(core_type))?;
        self.copy_seed_core_asset(core_type)?;
        let executable = discover_executable(&self.paths, core_info)?;
        let launch = core_launch_plan(core_type, executable, &self.paths, config_file_name)
            .ok_or(RuntimeError::MissingCoreInfo(core_type))?;

        Ok(CoreProcessSpec::new(core_type, launch)
            .with_display_log(context.node.display_log)
            .with_may_need_sudo(true))
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

fn write_runtime_config(
    paths: &AppPaths,
    file_name: &str,
    context: &CoreConfigContext,
) -> Result<PathBuf, RuntimeError> {
    let json = if context.run_core_type == CoreType::sing_box {
        generate_singbox_config_json(context)?
    } else {
        generate_xray_config_json(context)
    };
    let path = paths.bin_config_file(file_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RuntimeError::CreateConfigDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&path, json).map_err(|source| RuntimeError::WriteConfig {
        path: path.clone(),
        source,
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
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RuntimeError::RemoveConfig { path, source }),
    }
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

async fn load_runtime_core_gen_env(
    database: &Database,
    paths: &AppPaths,
    config: &AppConfig,
    target_os: TargetOs,
) -> Result<SnapshotCoreGenEnv, DbError> {
    Ok(SnapshotCoreGenEnv::new(
        config,
        core_gen_platform(target_os),
        CoreTypeFallback::ConfigDefaults,
        database.profiles().list().await?,
        database.routings().list().await?,
        database.dns().list().await?,
        database.subscriptions().list().await?,
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
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc, Mutex,
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use voya_core::{
        ConfigType, CoreGenEnv, CoreType, DnsItem, ProfileItem, RoutingItem, RuleType, RulesItem,
    };
    use voya_db::Database;
    use voya_platform::{
        coreinfo::{
            core_type_dir_name, executable_name_for_current_os, MIERU_CONFIG_ENV,
            XRAY_LOCAL_ASSET_ENV, XRAY_LOCAL_CERT_ENV,
        },
        paths::{core_seed_resources_dir, AppPaths, StorageMode},
        process::{
            ProcessError, ProcessHandle, ProcessOutput, ProcessRole, ProcessRunner, ProcessSpawn,
        },
    };

    use super::*;
    use crate::supervisor::SupervisorDeps;

    static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn coreinfo_app_layer_exposes_full_platform_table() {
        let infos = supported_core_infos();

        assert_eq!(infos.len(), 15);
        assert!(infos.iter().any(|info| info.core_type == CoreType::Xray));
        assert!(infos
            .iter()
            .any(|info| info.core_type == CoreType::sing_box));
        assert!(infos.iter().any(|info| info.core_type == CoreType::v2rayN));
    }

    #[test]
    fn coreinfo_app_layer_resolves_launch_command_and_env() {
        let paths = AppPaths::new("/tmp/VoyaVPN", StorageMode::Portable);
        let launch = core_launch_plan(
            CoreType::Xray,
            "/tmp/VoyaVPN/bin/xray/xray",
            &paths,
            "config.json",
        )
        .expect("xray launch plan");

        assert_eq!(launch.arguments, "run -c config.json");
        assert_eq!(launch.working_dir, paths.bin_config_dir());
        assert_eq!(
            launch.environment.get(XRAY_LOCAL_ASSET_ENV),
            Some(&"/tmp/VoyaVPN/bin".to_string())
        );
        assert_eq!(
            launch.environment.get(XRAY_LOCAL_CERT_ENV),
            Some(&"/tmp/VoyaVPN/bin".to_string())
        );

        let mieru = core_launch_plan(
            CoreType::mieru,
            "/tmp/VoyaVPN/bin/mieru/mieru",
            &paths,
            "config.json",
        )
        .expect("mieru launch plan");
        assert_eq!(mieru.arguments, "run");
        assert_eq!(
            mieru.environment.get(MIERU_CONFIG_ENV),
            Some(&"config.json".to_string())
        );
    }

    #[tokio::test]
    async fn runtime_dns_context_env_loads_persisted_dns_items() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let item = DnsItem {
            id: "dns-xray".to_string(),
            remarks: "Xray".to_string(),
            enabled: true,
            core_type: CoreType::Xray,
            normal_dns: Some(r#"{"servers":["1.1.1.1"]}"#.to_string()),
            ..DnsItem::default()
        };
        database
            .dns()
            .upsert(&item)
            .await
            .expect("runtime test operation should succeed");

        let paths = temp_paths();
        let env =
            load_runtime_core_gen_env(&database, &paths, &AppConfig::default(), TargetOs::Linux)
                .await
                .expect("runtime test operation should succeed");

        assert_eq!(env.get_dns_item(CoreType::Xray), Some(item));
    }

    #[derive(Clone, Default)]
    struct RecordingRunner {
        spawns: Arc<Mutex<Vec<ProcessSpawn>>>,
        stops: Arc<Mutex<Vec<u32>>>,
    }

    impl ProcessRunner for RecordingRunner {
        fn spawn(&self, request: ProcessSpawn) -> Result<ProcessHandle, ProcessError> {
            self.spawns.lock().expect("spawns").push(request);
            Ok(ProcessHandle::new(10, ProcessRole::Main))
        }

        fn run_oneshot(&self, _request: ProcessSpawn) -> Result<ProcessOutput, ProcessError> {
            Ok(ProcessOutput {
                status_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn stop(&self, handle: &ProcessHandle) -> Result<(), ProcessError> {
            self.stops.lock().expect("stops").push(handle.id());
            Ok(())
        }
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
        write_fake_core_executable(&paths, CoreType::Xray);
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(runner.clone()),
            Arc::new(voya_platform::elevation::SudoPasswordStore::new()),
        ));
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, TargetOs::Linux);
        let mut config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        let profile = ProfileItem {
            index_id: "active".to_string(),
            config_type: ConfigType::VLESS,
            core_type: Some(CoreType::Xray),
            remarks: "Runtime".to_string(),
            address: "example.test".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            network: "tcp".to_string(),
            ..ProfileItem::default()
        };
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
        assert_eq!(runner.spawns.lock().expect("spawns").len(), 1);
        assert_eq!(
            runner.spawns.lock().expect("spawns")[0].arguments,
            ["run", "-c", MAIN_CONFIG_FILE_NAME]
        );

        let disconnected = manager
            .disconnect()
            .await
            .expect("runtime test operation should succeed");

        assert_eq!(
            disconnected.state,
            crate::supervisor::SupervisorConnectionState::Disconnected
        );
        assert!(!paths.bin_config_file(MAIN_CONFIG_FILE_NAME).exists());
        assert_eq!(runner.stops.lock().expect("stops").as_slice(), [10]);
        config.index_id.clear();
    }

    #[tokio::test]
    async fn coreinfo_runtime_connect_copies_seed_core_before_discovery() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
        let seed_exe = write_seed_core_executable(&seed_root, CoreType::Xray, b"seed-xray");
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(runner.clone()),
            Arc::new(voya_platform::elevation::SudoPasswordStore::new()),
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
            .upsert(&active_xray_profile("active"))
            .await
            .expect("runtime test operation should succeed");

        manager
            .connect(&config)
            .await
            .expect("runtime test operation should succeed");

        let app_data_exe = paths.core_bin_file(
            core_type_dir_name(CoreType::Xray),
            executable_name_for_current_os("xray"),
        );
        let spawns = runner.spawns.lock().expect("spawns");
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].executable, app_data_exe);
        assert_ne!(spawns[0].executable, seed_exe);
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
            Arc::new(voya_platform::elevation::SudoPasswordStore::new()),
        ));
        let manager = RuntimeManager::with_target_os(&database, paths, supervisor, TargetOs::Linux)
            .with_core_seed_resource_dir(seed_root);
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        database
            .profiles()
            .upsert(&active_xray_profile("active"))
            .await
            .expect("runtime test operation should succeed");

        let error = manager.connect(&config).await.expect_err("missing core");

        match error {
            RuntimeError::CoreInfo(CoreInfoError::ExecutableNotFound { core_type, .. }) => {
                assert_eq!(core_type, CoreType::Xray);
            }
            other => panic!("expected typed missing core error, got {other:?}"),
        }
        assert!(runner.spawns.lock().expect("spawns").is_empty());
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
        write_fake_core_executable(&paths, CoreType::Xray);
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(runner),
            Arc::new(voya_platform::elevation::SudoPasswordStore::new()),
        ));
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, TargetOs::Linux);
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        let profile = ProfileItem {
            index_id: "active".to_string(),
            config_type: ConfigType::VLESS,
            core_type: Some(CoreType::Xray),
            remarks: "Runtime".to_string(),
            address: "example.test".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            network: "tcp".to_string(),
            ..ProfileItem::default()
        };
        let routing = RoutingItem {
            id: "routing-active".to_string(),
            remarks: "Active routing".to_string(),
            is_active: true,
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
        let rules = json["routing"]["rules"]
            .as_array()
            .expect("runtime test operation should succeed");
        assert!(rules.iter().any(|rule| {
            rule["outboundTag"] == "direct"
                && rule["domain"].as_array().is_some_and(|domains| {
                    domains
                        .iter()
                        .any(|domain| domain == "full:direct.example.com")
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
            StorageMode::Portable,
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

    fn active_xray_profile(index_id: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VLESS,
            core_type: Some(CoreType::Xray),
            remarks: "Runtime".to_string(),
            address: "example.test".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            network: "tcp".to_string(),
            ..ProfileItem::default()
        }
    }
}
