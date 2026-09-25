use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use thiserror::Error;
use tokio::sync::Mutex;
use voya_core::{
    generate_singbox_config_json, validation::ValidationMessage, AppConfig, CoreConfigContext,
    CoreConfigContextBuilder, CoreGenPlatform, SingboxConfigError,
};
use voya_db::{Database, DbError};
use voya_platform::{
    coreinfo::{
        copy_seed_core_asset, core_launch, discover_executable, discover_packaged_seed_executable,
        CoreInfoError, TargetOs,
    },
    filesystem,
    paths::{AppPaths, PathError},
};

use crate::coregen::SnapshotCoreGenEnv;
use crate::supervisor::{
    ClashApiSecret, CoreProcessSpec, CoreSupervisor, SupervisorConnectionState, SupervisorError,
    SupervisorSnapshot, SupervisorStartRequest,
};
use crate::updates::local_singbox_ruleset_paths;

pub const MAIN_CONFIG_FILE_NAME: &str = "config.json";
pub const PRE_CONFIG_FILE_NAME: &str = "configPre.json";
const SUDO_SCRIPT_DIR_NAME: &str = "sudo";

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
    settings_application: crate::settings::apply::SettingsApplication,
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
            settings_application: crate::settings::apply::SettingsApplication::default(),
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

    #[must_use]
    pub fn with_settings_application(
        mut self,
        application: crate::settings::apply::SettingsApplication,
    ) -> Self {
        self.settings_application = application;
        self
    }

    #[must_use]
    pub(crate) const fn database(&self) -> &'runtime Database {
        self.database
    }

    #[must_use]
    pub(crate) const fn paths(&self) -> &AppPaths {
        &self.paths
    }

    #[must_use]
    pub fn settings_application(&self) -> &crate::settings::apply::SettingsApplication {
        &self.settings_application
    }

    pub async fn connect(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
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

    pub async fn disconnect_removed_profile(
        &self,
    ) -> Result<Option<SupervisorSnapshot>, RuntimeError> {
        let _guard = self.operation_lock.lock().await;
        let status = self.supervisor.status().await?;
        // The running node or group was deleted out from under the core.
        let still_exists = match (
            status.active_profile_id.as_deref(),
            status.active_group_id.as_deref(),
        ) {
            (Some(id), _) => self.database.profiles().exists(id).await?,
            (None, Some(id)) => self.database.policy_groups().exists(id).await?,
            (None, None) => return Ok(None),
        };
        if still_exists {
            return Ok(None);
        }
        let snapshot = self.supervisor.stop().await?;
        cleanup_runtime_state(&self.paths)?;
        Ok(Some(snapshot))
    }

    pub async fn status(&self) -> Result<SupervisorSnapshot, RuntimeError> {
        // Deliberately unlocked: status is polled while a connect holds the
        // lock, and the failure-recovery paths query it to find out what the
        // supervisor really did.
        self.supervisor.status().await.map_err(Into::into)
    }

    async fn start_core(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
        self.paths.ensure_dirs()?;

        enum LaunchTarget {
            Node(Box<voya_core::ProfileItem>),
            Group(voya_core::PolicyGroupItem),
        }
        // A node and a group are never active together; which one this launch
        // uses decides the generated outbounds and what the snapshot reports.
        let target = match config.active_target() {
            voya_core::ActiveTarget::None => return Err(RuntimeError::MissingActiveProfileId),
            voya_core::ActiveTarget::Node(id) => LaunchTarget::Node(Box::new(
                self.database
                    .profiles()
                    .get(id)
                    .await?
                    .ok_or_else(|| RuntimeError::ActiveProfileNotFound(id.to_string()))?,
            )),
            voya_core::ActiveTarget::Group(id) => LaunchTarget::Group(
                self.database
                    .policy_groups()
                    .get(id)
                    .await?
                    .ok_or_else(|| RuntimeError::ActivePolicyGroupNotFound(id.to_string()))?,
            ),
        };

        // Minted per launch and never persisted: the core it authenticates dies
        // with this request, so a leaked token from an earlier run is useless.
        let clash_api_secret = ClashApiSecret::generate();
        let env = load_runtime_core_gen_env(self.database, &self.paths, config, self.target_os)
            .await?
            .with_clash_api_secret(clash_api_secret.clone());
        let mut contexts = match &target {
            LaunchTarget::Node(profile) => {
                CoreConfigContextBuilder::new(&env).build_all(config, profile)
            }
            LaunchTarget::Group(group) => {
                let members: Vec<voya_core::ProfileItem> =
                    voya_core::resolve_group_members(group, env.profiles())
                        .into_iter()
                        .cloned()
                        .collect();
                CoreConfigContextBuilder::new(&env).build_all_for_group(config, group, &members)
            }
        };
        let (active_profile_id, active_group_id) = match &target {
            LaunchTarget::Node(profile) => (Some(profile.index_id.clone()), None),
            LaunchTarget::Group(group) => (None, Some(group.id.clone())),
        };
        let validation = contexts.combined_validator_result();
        if !contexts.success() {
            return Err(RuntimeError::Validation {
                errors: validation.errors,
                warnings: validation.warnings,
            });
        }
        // A config that generates but is suspicious used to say nothing at all:
        // the warnings only travelled on the error variant. `warn` is the level
        // the desktop log layer forwards to the Logs panel, so these reach the
        // user rather than only the file log.
        for warning in &validation.warnings {
            tracing::warn!(
                profile = ?active_profile_id,
                group = ?active_group_id,
                "core config generation warning: {warning:?}"
            );
        }
        if self.target_os.runs_core_in_tunnel_provider() {
            // This core is the only one — macOS's PacketTunnel, a phone's
            // provider — so the speedtest measures nodes through it
            // (`speedtest::running_core`) and every node that can carry a
            // probe outbound gets one.
            contexts.main_result.context.latency_probe_nodes = env.profiles().to_vec();
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
            active_profile_id,
            active_group_id,
            main: main_spec,
            pre,
            tun_enabled: config.tun_mode_item.enable_tun,
            kill_switch: config.tun_mode_item.strict_route,
            sudo_script_dir: self.paths.temp_dir().join(SUDO_SCRIPT_DIR_NAME),
            restart_on_crash: true,
            // Taken from the generated main context, not from the TUN setting:
            // those disagree on a pre-socks topology.
            clash_api_port: contexts.main_result.context.clash_api_port(),
            clash_api_secret: Some(clash_api_secret),
        };

        match self.supervisor.start(request).await {
            Ok(snapshot) => {
                self.settings_application.core_applied(config);
                Ok(snapshot)
            }
            Err(error) => {
                // The start error is what the caller needs; a failed removal
                // only leaves a credential-bearing config behind, so say so.
                if let Err(cleanup) = filesystem::remove_file_if_exists(&main_config_path) {
                    tracing::warn!(
                        path = %main_config_path.display(),
                        error = ?cleanup,
                        "failed to remove core config after a failed start"
                    );
                }
                if let Err(cleanup) = cleanup_config_file(&self.paths, PRE_CONFIG_FILE_NAME) {
                    tracing::warn!(error = ?cleanup, "failed to remove pre-core config after a failed start");
                }
                Err(error.into())
            }
        }
    }

    fn process_spec(
        &self,
        context: &CoreConfigContext,
        config_file_name: &str,
    ) -> Result<CoreProcessSpec, RuntimeError> {
        let spec = if matches!(self.target_os, TargetOs::Macos | TargetOs::Ios) {
            // macOS and iOS run sing-box inside the PacketTunnel provider;
            // connecting never requires a standalone executable on disk.
            CoreProcessSpec::native_tun()
        } else {
            let executable = resolve_core_executable(
                &self.paths,
                self.core_seed_resource_dir.as_deref(),
                self.target_os,
            )?;
            CoreProcessSpec::new(core_launch(executable, &self.paths, config_file_name))
        };

        // Only the process whose config carries the tun inbound needs root:
        // with a pre-socks split the main core does all remote I/O and must
        // stay unprivileged.
        Ok(spec
            .with_config_path(self.paths.bin_config_file(config_file_name))
            .with_display_log(context.node.display_log)
            .with_may_need_sudo(context.is_tun_enabled))
    }
}

/// Locates the sing-box executable, staging the packaged seed if needed.
///
/// Shared with the speedtest backend: a probe core has to resolve exactly the
/// binary the runtime would launch.
pub(crate) fn resolve_core_executable(
    paths: &AppPaths,
    core_seed_resource_dir: Option<&Path>,
    target_os: TargetOs,
) -> Result<PathBuf, CoreInfoError> {
    if let Some(executable) = packaged_seed_executable(core_seed_resource_dir, target_os)? {
        return Ok(executable);
    }

    if let Some(seed_resource_dir) = core_seed_resource_dir {
        copy_seed_core_asset(paths, seed_resource_dir)?;
    }

    discover_executable(paths)
}

/// On macOS the seed inside the app bundle is launched where it lies.
///
/// Copying it into app data would strip the bundle's code signature context and
/// break the notarized launch, so the copy-then-discover path is Windows/Linux
/// only.
fn packaged_seed_executable(
    core_seed_resource_dir: Option<&Path>,
    target_os: TargetOs,
) -> Result<Option<PathBuf>, CoreInfoError> {
    if target_os != TargetOs::Macos {
        return Ok(None);
    }

    let Some(seed_resource_dir) = core_seed_resource_dir else {
        return Ok(None);
    };

    discover_packaged_seed_executable(seed_resource_dir, target_os)
}

/// Failure to write a generated core config, with the path that failed.
///
/// The runtime and the speedtest backend each have their own `WriteConfig`
/// error variant, so the shared writer reports both halves and lets the caller
/// wrap them.
#[derive(Debug)]
pub(crate) struct CoreConfigWriteError {
    pub(crate) path: PathBuf,
    pub(crate) source: io::Error,
}

/// Writes generated core JSON into the runtime config directory.
///
/// Every generated config embeds outbound credentials (passwords, UUIDs,
/// Reality private keys) and now the Clash API bearer token as well, so it is
/// written 0600 and never through a symlink.
pub(crate) fn write_core_config(
    paths: &AppPaths,
    file_name: &str,
    json: &str,
) -> Result<PathBuf, CoreConfigWriteError> {
    let path = paths.bin_config_file(file_name);
    filesystem::write_private_file_with_parent(&path, json).map_err(|source| {
        CoreConfigWriteError {
            path: path.clone(),
            source,
        }
    })?;

    Ok(path)
}

fn write_runtime_config(
    paths: &AppPaths,
    file_name: &str,
    context: &CoreConfigContext,
) -> Result<PathBuf, RuntimeError> {
    let json = generate_singbox_config_json(context)?;
    write_core_config(paths, file_name, &json).map_err(|error| RuntimeError::WriteConfig {
        path: error.path,
        source: error.source,
    })
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
    #[error("settings could not be applied: {0}")]
    SettingsApply(String),
    #[error("active node id is empty")]
    MissingActiveProfileId,
    #[error("active node {0} was not found")]
    ActiveProfileNotFound(String),
    #[error("active policy group {0} was not found")]
    ActivePolicyGroupNotFound(String),
    #[error("runtime validation failed: {errors:?}; warnings: {warnings:?}")]
    Validation {
        errors: Vec<ValidationMessage>,
        warnings: Vec<ValidationMessage>,
    },
    #[error("failed to create runtime config directory {path}: {source}")]
    CreateConfigDir { path: PathBuf, source: io::Error },
    #[error("failed to write runtime config {path}: {source}")]
    WriteConfig { path: PathBuf, source: io::Error },
    #[error("failed to remove runtime config {path}: {source}")]
    RemoveConfig { path: PathBuf, source: io::Error },
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
    let profiles = database.profiles().list().await?;
    let ipv6_unsupported_nodes =
        crate::ipv6_egress::Ipv6EgressStore::new(paths).unsupported_nodes(&profiles);
    Ok(SnapshotCoreGenEnv::new(
        config,
        core_gen_platform(target_os),
        profiles,
        database.routings().list().await?,
    )
    .with_policy_groups(database.policy_groups().list().await?)
    .with_singbox_ruleset_paths(local_singbox_ruleset_paths(paths))
    .with_ipv6_unsupported_nodes(ipv6_unsupported_nodes))
}

pub(crate) const fn core_gen_platform(target_os: TargetOs) -> CoreGenPlatform {
    match target_os {
        TargetOs::Windows => CoreGenPlatform::Windows,
        // iOS runs the same Darwin core as macOS; Android is Linux. The
        // generator only uses this to pick platform defaults in the config, so
        // each phone takes the kernel it actually has.
        TargetOs::Macos | TargetOs::Ios => CoreGenPlatform::MacOS,
        TargetOs::Linux | TargetOs::Android | TargetOs::Other => CoreGenPlatform::Linux,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc};

    use voya_core::{
        ProfileItem, ProfileProtocol, ProfileTransport, RoutingItem, RuleType, RulesItem,
        ServerEndpoint,
    };
    use voya_db::Database;
    use voya_platform::{
        coreinfo::{executable_name_for_current_os, CORE_DIR_NAME, SING_BOX_EXECUTABLES},
        paths::{core_seed_resources_dir, AppPaths},
        test_support::RecordingRunner,
        tun::{
            NativeTunController, NativeTunError, NativeTunProviderState, NativeTunStartRequest,
            NativeTunStatus, TunBackend,
        },
    };

    use super::*;
    use crate::proxy_runtime::proxy_runtime_endpoint;
    use crate::supervisor::{ClashApiAccess, SupervisorDeps};
    use voya_net::clash::ClashApiEndpoint;

    #[tokio::test]
    async fn runtime_connect_writes_generated_config_and_starts_supervisor_path() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        paths
            .ensure_dirs()
            .expect("runtime test operation should succeed");
        write_fake_core_executable(&paths);
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
        let generated = fs::read_to_string(paths.bin_config_file(MAIN_CONFIG_FILE_NAME))
            .expect("runtime test operation should succeed");
        assert!(
            !generated.contains("\"probe:"),
            "only the macOS core measures nodes through latency probes"
        );
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
        write_fake_core_executable(&paths);
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
            vec![profile.clone()],
            Vec::new(),
        );

        let contexts = CoreConfigContextBuilder::new(&env).build_all(&config, &profile);
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

    #[test]
    fn runtime_clash_api_port_matches_the_generated_main_config() {
        // The Clash API port used to be recomputed downstream from
        // `tun_mode_item.enable_tun`. On the Linux TUN topology the builder
        // clears `is_tun_enabled` on the *main* context and gives the TUN
        // inbound to the pre-socks one, so that formula addressed the wrong
        // process. The supervisor now carries the port the main config
        // actually wrote, and this pins the two together.
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
            vec![profile.clone()],
            Vec::new(),
        );

        let contexts = CoreConfigContextBuilder::new(&env).build_all(&config, &profile);
        let main_context = &contexts.main_result.context;
        assert!(
            !main_context.is_tun_enabled,
            "Linux TUN puts the TUN inbound on the pre-socks process"
        );

        let generated: serde_json::Value = serde_json::from_str(
            &generate_singbox_config_json(main_context)
                .expect("runtime test operation should succeed"),
        )
        .expect("runtime test operation should succeed");
        let external_controller = generated["experimental"]["clash_api"]["external_controller"]
            .as_str()
            .expect("the generated main config exposes a Clash API");
        let generated_port: u16 = external_controller
            .rsplit_once(':')
            .expect("external_controller is host:port")
            .1
            .parse()
            .expect("external_controller port is numeric");

        assert_eq!(
            u16::try_from(main_context.clash_api_port())
                .expect("runtime test operation should succeed"),
            generated_port,
            "the port handed to the supervisor must be the one the core opens"
        );
        assert_eq!(
            proxy_runtime_endpoint(&ClashApiAccess::unauthenticated(generated_port)),
            Some(ClashApiEndpoint::loopback(generated_port)),
            "the proxy runtime must dial the running core's Clash API"
        );
    }

    /// The Clash API listens on loopback, which is not a trust boundary: any
    /// local process — or any page a browser can be pointed at — could read the
    /// live connection list and re-route every flow. The generated config must
    /// therefore demand a token, and the supervisor must hand callers the same
    /// one so the app can still talk to its own core.
    #[tokio::test]
    async fn runtime_connect_locks_the_clash_api_behind_a_per_launch_secret() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        paths
            .ensure_dirs()
            .expect("runtime test operation should succeed");
        write_fake_core_executable(&paths);
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(RecordingRunner::default()),
            Arc::new(voya_platform::privilege::ElevationState::new()),
        ));
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, TargetOs::Linux);
        let config = AppConfig {
            index_id: "active".to_string(),
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

        let generated: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(paths.bin_config_file(MAIN_CONFIG_FILE_NAME))
                .expect("runtime test operation should succeed"),
        )
        .expect("runtime test operation should succeed");
        let written_secret = generated["experimental"]["clash_api"]["secret"]
            .as_str()
            .expect("the generated main config must require a bearer token");
        let access = connected.clash_api_access();

        assert_eq!(
            access.secret.as_ref().map(ClashApiSecret::as_str),
            Some(written_secret),
            "the supervisor must report the token the core will actually check"
        );
        assert_eq!(
            proxy_runtime_endpoint(&access)
                .expect("a connected core exposes an endpoint")
                .secret
                .as_deref(),
            Some(written_secret),
            "every Clash API client must present the token"
        );
    }

    /// Two launches must never share a token: a value that leaked from an
    /// earlier run would otherwise keep working against the current core.
    #[test]
    fn runtime_clash_api_secret_is_fresh_per_launch() {
        assert_ne!(
            ClashApiSecret::generate().as_str(),
            ClashApiSecret::generate().as_str()
        );
    }

    /// A profile whose generation only warns still connects, so the warning has
    /// to travel somewhere other than the error variant. This pins the warning
    /// surviving generation rather than the log line itself, which `tracing`
    /// owns.
    #[test]
    fn runtime_successful_generation_still_reports_its_warnings() {
        let profile = active_singbox_profile("active");
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        // A rule with a blank outbound tag generates fine — it just silently
        // does nothing, which is exactly the case a warning has to reach.
        let routing = RoutingItem {
            id: "routing-active".to_string(),
            remarks: "Active routing".to_string(),
            rule_set: vec![RulesItem {
                id: "rule-blank-outbound".to_string(),
                outbound_tag: Some("   ".to_string()),
                domain: Some(vec!["full:example.com".to_string()]),
                rule_type: Some(RuleType::Routing),
                ..RulesItem::default()
            }],
            ..RoutingItem::default()
        };
        let env = SnapshotCoreGenEnv::new(
            &config,
            CoreGenPlatform::Linux,
            vec![profile.clone()],
            vec![routing],
        );

        let contexts = CoreConfigContextBuilder::new(&env).build_all(&config, &profile);

        assert!(contexts.success(), "the config still generates");
        assert!(
            !contexts.combined_validator_result().warnings.is_empty(),
            "a rule with no outbound tag must warn"
        );
    }

    /// `build_all` reads the injected `tun_topology()` fact, so macOS keeps the
    /// TUN inbound in the single config the PacketTunnel provider is handed.
    #[test]
    fn runtime_macos_tun_builds_one_context_that_owns_the_tun_inbound() {
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
            CoreGenPlatform::MacOS,
            vec![profile.clone()],
            Vec::new(),
        );

        let contexts = CoreConfigContextBuilder::new(&env).build_all(&config, &profile);
        assert!(contexts.pre_socks_result.is_none());
        assert!(contexts.main_result.context.is_tun_enabled);
    }

    /// The native provider runs the connection: connecting must not look
    /// for a sing-box executable, and the config it hands the provider
    /// carries a probe outbound for every node so the speedtest can measure
    /// through it while connected.
    #[tokio::test]
    async fn runtime_macos_native_tun_writes_single_tun_config() {
        assert_native_tun_writes_single_tun_config(TargetOs::Macos).await;
    }

    #[tokio::test]
    async fn runtime_ios_native_tun_writes_single_tun_config() {
        assert_native_tun_writes_single_tun_config(TargetOs::Ios).await;
    }

    async fn assert_native_tun_writes_single_tun_config(target_os: TargetOs) {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        paths
            .ensure_dirs()
            .expect("runtime test operation should succeed");
        let runner = RecordingRunner::default();
        let supervisor = CoreSupervisor::spawn(
            SupervisorDeps::new(
                Arc::new(runner.clone()),
                Arc::new(voya_platform::privilege::ElevationState::new()),
            )
            .with_target_os(target_os)
            .with_native_tun_controller(Arc::new(TestNativeTunController)),
        );
        let manager =
            RuntimeManager::with_target_os(&database, paths.clone(), supervisor, target_os);
        let config = AppConfig {
            index_id: "active".to_string(),
            tun_mode_item: voya_core::TunModeItem {
                enable_tun: true,
                ..voya_core::TunModeItem::default()
            },
            ..AppConfig::default()
        };
        for id in ["active", "other"] {
            database
                .profiles()
                .upsert(&active_singbox_profile(id))
                .await
                .expect("runtime test operation should succeed");
        }

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
            "native TUN must pass a tun inbound to the provider on {target_os:?}"
        );
        let probes = json["outbounds"]
            .as_array()
            .expect("outbounds")
            .iter()
            .filter_map(|outbound| outbound["tag"].as_str())
            .filter(|tag| tag.starts_with("probe:"))
            .collect::<Vec<_>>();
        assert_eq!(probes.len(), 2, "{probes:?}");
    }

    #[tokio::test]
    async fn coreinfo_runtime_connect_copies_seed_core_before_discovery() {
        let database = Database::connect_in_memory()
            .await
            .expect("runtime test operation should succeed");
        let paths = temp_paths();
        let seed_root = core_seed_resources_dir(paths.app_dir().join("resources"));
        let seed_exe = write_seed_core_executable(&seed_root, b"seed-sing-box");
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

        let app_data_exe =
            paths.core_bin_file(CORE_DIR_NAME, executable_name_for_current_os("sing-box"));
        let spawns = runner.spawns();
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
            RuntimeError::CoreInfo(CoreInfoError::ExecutableNotFound { .. }) => {}
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
        write_fake_core_executable(&paths);
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
        let dir = tempfile::Builder::new()
            .prefix("voyavpn-runtime-tests-")
            .tempdir()
            .expect("temp dir");
        AppPaths::new(dir.keep())
    }

    fn write_fake_core_executable(paths: &AppPaths) {
        let executable_name = executable_name_for_current_os(SING_BOX_EXECUTABLES[0]);
        let executable = paths.core_bin_file(CORE_DIR_NAME, executable_name);
        fs::create_dir_all(executable.parent().expect("core dir"))
            .expect("runtime test operation should succeed");
        fs::write(executable, b"fake").expect("runtime test operation should succeed");
    }

    fn write_seed_core_executable(seed_root: &Path, contents: &[u8]) -> PathBuf {
        let executable_name = executable_name_for_current_os(SING_BOX_EXECUTABLES[0]);
        let executable = seed_root.join(CORE_DIR_NAME).join(executable_name);
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
