//! Application-owned facade over persistence-backed use cases.
//!
//! Desktop shells depend on this facade instead of constructing repositories or
//! passing the database handle through command adapters.

use std::{path::Path, sync::Arc};

pub use voya_core::{AppConfig, SysProxyType, TrafficMode, DEFAULT_LOCAL_PORT};
use voya_db::{Database, DbError};
use voya_platform::{coreinfo::TargetOs, paths::AppPaths, process::ProcessRunner};

use crate::{
    config_mutation::{ConfigMutationCoordinator, SharedAppConfig},
    connection_mode::{enforce_platform_connection_mode, seed_platform_connection_defaults},
    exports::ExportManager,
    profiles::ProfileManager,
    routing::RoutingManager,
    runtime::RuntimeManager,
    self_host::{SelfHostDeps, SelfHostManager},
    settings::save::app_config_from_settings,
    speedtest::{SpeedtestManager, SpeedtestResult, SpeedtestRunResult},
    statistics::{StatisticsEventSink, StatisticsManager},
    subscriptions::{
        SubscriptionAutoUpdateScheduler, SubscriptionAutoUpdateSink, SubscriptionManager,
    },
    supervisor::CoreSupervisor,
    updates::UpdateManager,
};

#[derive(Debug, Clone)]
pub struct AppServices {
    database: Database,
    runtime_paths: AppPaths,
    /// Shared by every `RuntimeManager` this facade hands out, so concurrent
    /// commands cannot interleave the runtime config write/delete around the
    /// supervisor's Start and Stop.
    runtime_lock: Arc<tokio::sync::Mutex<()>>,
    settings_application: crate::settings::apply::SettingsApplication,
}

impl AppServices {
    pub async fn connect(database_path: &Path, runtime_paths: AppPaths) -> Result<Self, DbError> {
        Ok(Self {
            database: Database::connect(database_path).await?,
            runtime_paths,
            runtime_lock: Arc::new(tokio::sync::Mutex::new(())),
            settings_application: crate::settings::apply::SettingsApplication::default(),
        })
    }

    /// Load the persisted settings projected onto an `AppConfig` for the
    /// platform the app runs on: a fresh install
    /// starts in the native VPN mode where the platform has one and in the
    /// shipped language closest to `system_locale`, and macOS always loads in
    /// VPN mode.
    pub async fn load_config_for(
        &self,
        target_os: TargetOs,
        system_locale: Option<&str>,
    ) -> Result<AppConfig, DbError> {
        let stored = self.database.settings().load_stored().await?;
        let fresh = stored.is_none();
        let state = self.database.app_state().load().await?;
        let mut config = app_config_from_settings(&stored.unwrap_or_default(), &state);
        if fresh {
            seed_platform_connection_defaults(&mut config, target_os);
            if let Some(locale) = system_locale {
                config.ui_item.current_language =
                    crate::language::ui_language_for_locale(locale).to_string();
            }
        }
        enforce_platform_connection_mode(&mut config, target_os);
        Ok(config)
    }

    #[must_use]
    pub fn config_mutations(&self, config: SharedAppConfig) -> ConfigMutationCoordinator {
        ConfigMutationCoordinator::new(self.database.clone(), config)
    }

    pub async fn initialize_profile_metrics(&self) -> crate::profiles::Result<u64> {
        Ok(self.database.profile_exs().delete_orphans().await?)
    }

    #[must_use]
    pub fn profiles(&self) -> ProfileManager<'_> {
        ProfileManager::new(&self.database)
    }

    #[must_use]
    pub fn policy_groups(&self) -> crate::policy_groups::PolicyGroupManager<'_> {
        crate::policy_groups::PolicyGroupManager::new(&self.database)
    }

    #[must_use]
    pub fn subscriptions(&self) -> SubscriptionManager<'_> {
        SubscriptionManager::new(&self.database)
    }

    #[must_use]
    pub fn routings(&self) -> RoutingManager<'_> {
        RoutingManager::new(&self.database)
    }

    /// Seeds the default routing profile on a database that has none. Returns
    /// whether a profile was created.
    pub async fn ensure_default_routing(
        &self,
        coordinator: &ConfigMutationCoordinator,
    ) -> Result<bool, voya_contracts::AppError> {
        let committed = coordinator
            .mutate(async |unit_of_work, config| {
                let language = config.ui_item.current_language.clone();
                let seeded = RoutingManager::new_in(unit_of_work)
                    .ensure_default_routing(config, &language)
                    .await?;
                Ok::<bool, voya_contracts::AppError>(seeded.is_some())
            })
            .await?;
        Ok(committed.value)
    }

    #[must_use]
    pub fn exports(&self) -> ExportManager<'_> {
        ExportManager::new(&self.database)
    }

    #[must_use]
    pub fn updates(&self) -> UpdateManager<'_> {
        UpdateManager::new(&self.database, self.runtime_paths.clone())
    }

    #[must_use]
    pub fn runtime(&self, supervisor: CoreSupervisor) -> RuntimeManager<'_> {
        RuntimeManager::new(&self.database, self.runtime_paths.clone(), supervisor)
            .with_operation_lock(Arc::clone(&self.runtime_lock))
            .with_settings_application(self.settings_application.clone())
    }

    /// An explicit live mode switch acknowledges that field alone. Other
    /// settings saved since the last connection remain pending.
    pub fn acknowledge_traffic_mode(
        &self,
        snapshot: &crate::supervisor::SupervisorSnapshot,
        outcome: &crate::proxy_runtime::TrafficModeChangeOutcome,
    ) {
        if snapshot.state == crate::supervisor::SupervisorConnectionState::Connected
            && !matches!(
                outcome.runtime_result,
                Err(crate::proxy_runtime::TrafficModeChangeError::Apply(_))
            )
        {
            self.settings_application.traffic_mode_applied(outcome.mode);
        }
    }

    #[must_use]
    pub fn spawn_statistics(
        &self,
        supervisor: CoreSupervisor,
        config: SharedAppConfig,
        event_sink: Arc<dyn StatisticsEventSink>,
    ) -> StatisticsManager {
        StatisticsManager::spawn(self.database.clone(), supervisor, config, event_sink)
    }

    /// Starts the background subscription auto-update loop.
    ///
    /// Exists so the shell does not have to. `AppServices::database()` was the
    /// one accessor that handed a `voya_db::Database` out of this facade, and
    /// `setup()` used it for exactly this call — the header of this file
    /// forbids that, and `check:architecture` cannot see it because the handle
    /// arrives through a voya-app method rather than a `voya_db::` path.
    #[must_use]
    pub fn spawn_subscription_auto_update(
        &self,
        coordinator: Arc<ConfigMutationCoordinator>,
        supervisor: CoreSupervisor,
        target_os: TargetOs,
        sink: Arc<dyn SubscriptionAutoUpdateSink>,
    ) -> SubscriptionAutoUpdateScheduler {
        SubscriptionAutoUpdateScheduler::spawn(
            self.database.clone(),
            coordinator,
            supervisor,
            target_os,
            sink,
        )
    }

    /// Starts the self-hosted node's manager: its exit listener, its watch
    /// loop, and the node itself when it was left enabled.
    #[must_use]
    pub fn spawn_self_host(&self, deps: SelfHostDeps) -> SelfHostManager {
        SelfHostManager::spawn(self.database.clone(), deps)
    }

    pub async fn run_speedtest<F>(
        &self,
        manager: &SpeedtestManager,
        config: &AppConfig,
        profile_ids: Vec<String>,
        on_results: F,
    ) -> crate::speedtest::Result<SpeedtestRunResult>
    where
        F: Fn(Vec<SpeedtestResult>) + Send + Sync,
    {
        // The manager reads an empty list as every stored node. The app only
        // tests explicit selections, so an empty one is rejected here.
        if profile_ids.is_empty() {
            return Err(crate::speedtest::SpeedtestError::EmptySelection);
        }
        manager
            .run_with_callback(&self.database, config, profile_ids, on_results)
            .await
    }

    #[must_use]
    pub fn runtime_paths(&self) -> &AppPaths {
        &self.runtime_paths
    }

    /// Nodes are measured with throwaway probe cores from the packaged seed.
    #[must_use]
    pub fn speedtest_manager(
        &self,
        core_seed_resource_dir: Option<std::path::PathBuf>,
        runner: Arc<dyn ProcessRunner>,
        supervisor: CoreSupervisor,
    ) -> SpeedtestManager {
        self.speedtest_manager_with_launcher(
            Arc::new(crate::speedtest::ProcessProbeCoreLauncher::new(
                self.runtime_paths.clone(),
                core_seed_resource_dir,
                runner,
            )),
            supervisor,
        )
    }

    /// The same manager over a host-supplied launcher, for a platform that may
    /// not spawn a child process.
    ///
    /// Where the core runs inside a tunnel provider its traffic would carry a
    /// probe core's connections too, so while connected the test goes through
    /// the running core instead — on macOS and on both phones alike.
    #[must_use]
    pub fn speedtest_manager_with_launcher(
        &self,
        launcher: Arc<dyn crate::speedtest::ProbeCoreLauncher>,
        supervisor: CoreSupervisor,
    ) -> SpeedtestManager {
        let manager = SpeedtestManager::with_launcher(self.runtime_paths.clone(), launcher);
        if TargetOs::current().runs_core_in_tunnel_provider() {
            manager.with_running_core(Arc::new(crate::speedtest::SupervisorRunningCoreProbe::new(
                supervisor,
            )))
        } else {
            manager
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::save::{app_config_from_settings, settings_from_app_config};
    use voya_contracts::{AppSettingsV1, SystemProxyType};

    /// The plain persisted-settings projection, without the platform seeding
    /// and enforcement that [`AppServices::load_config_for`] layers on top.
    async fn load_config(services: &AppServices) -> Result<AppConfig, DbError> {
        let settings = services.database.settings().load().await?;
        let state = services.database.app_state().load().await?;
        Ok(app_config_from_settings(&settings, &state))
    }

    #[tokio::test]
    async fn fresh_database_loads_default_settings_and_reopens() {
        let app_dir = std::env::temp_dir().join(format!(
            "voyavpn-fresh-settings-test-{}",
            uuid::Uuid::new_v4()
        ));
        let database_path = app_dir.join(voya_db::DATABASE_NAME);
        let runtime_paths = AppPaths::new(&app_dir);

        let services = AppServices::connect(&database_path, runtime_paths.clone())
            .await
            .expect("fresh database should connect");
        services.database.close().await;
        let before = std::fs::read(&database_path).expect("fresh database bytes");
        let services = AppServices::connect(&database_path, runtime_paths.clone())
            .await
            .expect("current database should connect");
        let initial = load_config(&services)
            .await
            .expect("fresh default settings should load");
        assert_eq!(
            initial.system_proxy_item.sys_proxy_type,
            SysProxyType::ForcedChange
        );
        assert!(!initial.tun_mode_item.enable_tun);
        services.database.close().await;
        assert_eq!(
            std::fs::read(&database_path).expect("database bytes"),
            before
        );

        let reopened = AppServices::connect(&database_path, runtime_paths)
            .await
            .expect("initialized database should reconnect");
        let persisted = load_config(&reopened)
            .await
            .expect("persisted default settings should reload");
        assert_eq!(
            persisted.system_proxy_item.sys_proxy_type,
            SysProxyType::ForcedChange
        );
        reopened.database.close().await;

        std::fs::remove_dir_all(&app_dir).expect("test database directory should be removable");
    }

    #[tokio::test]
    async fn platform_loading_seeds_fresh_installs_and_keeps_macos_in_vpn_mode() {
        for (target_os, fresh_tun) in [
            (TargetOs::Windows, true),
            (TargetOs::Macos, true),
            (TargetOs::Linux, false),
        ] {
            let app_dir = std::env::temp_dir().join(format!(
                "voyavpn-platform-load-test-{}",
                uuid::Uuid::new_v4()
            ));
            let services = AppServices::connect(
                &app_dir.join(voya_db::DATABASE_NAME),
                AppPaths::new(&app_dir),
            )
            .await
            .expect("test database");

            let fresh = services
                .load_config_for(target_os, None)
                .await
                .expect("fresh");
            assert_eq!(fresh.tun_mode_item.enable_tun, fresh_tun, "{target_os:?}");

            let mut stored = AppSettingsV1::default();
            stored.network.tun.enabled = false;
            stored.network.system_proxy.mode = SystemProxyType::ForcedChange;
            services
                .database
                .settings()
                .save(&stored)
                .await
                .expect("settings");
            let loaded = services
                .load_config_for(target_os, None)
                .await
                .expect("stored");
            assert_eq!(
                loaded.tun_mode_item.enable_tun,
                target_os == TargetOs::Macos,
                "a saved choice wins except where the platform has no system proxy: {target_os:?}"
            );

            services.database.close().await;
            std::fs::remove_dir_all(app_dir).expect("remove test database");
        }
    }

    #[tokio::test]
    async fn a_fresh_install_speaks_the_system_language_until_one_is_saved() {
        let app_dir = std::env::temp_dir().join(format!(
            "voyavpn-language-load-test-{}",
            uuid::Uuid::new_v4()
        ));
        let services = AppServices::connect(
            &app_dir.join(voya_db::DATABASE_NAME),
            AppPaths::new(&app_dir),
        )
        .await
        .expect("test database");

        let fresh = services
            .load_config_for(TargetOs::Linux, Some("zh-Hant-TW"))
            .await
            .expect("fresh");
        assert_eq!(fresh.ui_item.current_language, "zh-Hant");
        let unknown = services
            .load_config_for(TargetOs::Linux, None)
            .await
            .expect("no locale");
        assert_eq!(unknown.ui_item.current_language, "en");

        let mut stored = AppSettingsV1::default();
        stored.appearance.language = "en".to_string();
        services
            .database
            .settings()
            .save(&stored)
            .await
            .expect("settings");
        let chosen = services
            .load_config_for(TargetOs::Linux, Some("zh-CN"))
            .await
            .expect("stored");
        assert_eq!(chosen.ui_item.current_language, "en", "a saved choice wins");

        services.database.close().await;
        std::fs::remove_dir_all(app_dir).expect("remove test database");
    }

    #[tokio::test]
    async fn loading_preserves_all_current_proxy_preferences_without_rewriting() {
        for mode in [
            SystemProxyType::ForcedClear,
            SystemProxyType::Unchanged,
            SystemProxyType::ForcedChange,
        ] {
            for tun_enabled in [false, true] {
                let app_dir = std::env::temp_dir()
                    .join(format!("voyavpn-mode-read-test-{}", uuid::Uuid::new_v4()));
                let database_path = app_dir.join(voya_db::DATABASE_NAME);
                let paths = AppPaths::new(&app_dir);
                let services = AppServices::connect(&database_path, paths.clone())
                    .await
                    .expect("test database");
                let mut expected = AppSettingsV1::default();
                expected.network.system_proxy.mode = mode;
                expected.network.tun.enabled = tun_enabled;
                expected.network.system_proxy.exceptions = "localhost,example.test".to_string();
                services
                    .database
                    .settings()
                    .save(&expected)
                    .await
                    .expect("settings");

                let loaded = load_config(&services).await.expect("settings read");
                assert_eq!(settings_from_app_config(&loaded), expected);
                assert_eq!(
                    services
                        .database
                        .settings()
                        .load()
                        .await
                        .expect("stored settings"),
                    expected
                );
                services.database.close().await;

                let before = std::fs::read(&database_path).expect("database bytes");
                let reopened = AppServices::connect(&database_path, paths)
                    .await
                    .expect("reopen");
                let reloaded = load_config(&reopened).await.expect("reload settings");
                assert_eq!(settings_from_app_config(&reloaded), expected);
                reopened.database.close().await;
                assert_eq!(
                    std::fs::read(&database_path).expect("database bytes"),
                    before
                );
                std::fs::remove_dir_all(app_dir).expect("remove test database");
            }
        }
    }
}
