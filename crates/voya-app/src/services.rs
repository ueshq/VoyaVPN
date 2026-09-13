//! Application-owned facade over persistence-backed use cases.
//!
//! Desktop shells depend on this facade instead of constructing repositories or
//! passing the database handle through command adapters.

use std::{path::Path, sync::Arc};

pub use voya_core::{AppConfig, CoreType, SysProxyType, TrafficMode, DEFAULT_LOCAL_PORT};
use voya_db::{Database, DbError};
use voya_platform::{coreinfo::TargetOs, paths::AppPaths, process::ProcessRunner};

use crate::{
    config_mutation::{ConfigMutationCoordinator, SharedAppConfig},
    dns::DnsManager,
    exports::ExportManager,
    profiles::ProfileManager,
    routing::RoutingManager,
    runtime::RuntimeManager,
    settings_save::app_config_from_settings,
    speedtest::{SpeedtestManager, SpeedtestResult, SpeedtestRunResult},
    statistics::{StatisticsConfigSource, StatisticsEventSink, StatisticsManager},
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
    settings_application: crate::settings_apply::SettingsApplication,
}

impl AppServices {
    pub async fn connect(database_path: &Path, runtime_paths: AppPaths) -> Result<Self, DbError> {
        Ok(Self {
            database: Database::connect(database_path).await?,
            runtime_paths,
            runtime_lock: Arc::new(tokio::sync::Mutex::new(())),
            settings_application: crate::settings_apply::SettingsApplication::default(),
        })
    }

    /// Read the persisted settings and project them onto an `AppConfig`.
    ///
    /// Only the database can fail here: every settings field is typed, so the
    /// projection itself is total.
    pub async fn load_config(&self) -> Result<AppConfig, DbError> {
        let settings = self.database.settings().load().await?;
        let state = self.database.app_state().load().await?;
        Ok(app_config_from_settings(&settings, &state))
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
    pub fn dns(&self) -> DnsManager<'_> {
        DnsManager::new(&self.database)
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
        config_source: Arc<dyn StatisticsConfigSource>,
        event_sink: Arc<dyn StatisticsEventSink>,
    ) -> StatisticsManager {
        StatisticsManager::spawn(self.database.clone(), supervisor, config_source, event_sink)
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

    pub async fn run_speedtest<F>(
        &self,
        manager: &SpeedtestManager,
        config: &AppConfig,
        profile_ids: Vec<String>,
        on_result: F,
    ) -> crate::speedtest::Result<SpeedtestRunResult>
    where
        F: Fn(SpeedtestResult) + Send + Sync,
    {
        // The manager reads an empty list as every stored node. The app only
        // tests explicit selections, so an empty one is rejected here.
        if profile_ids.is_empty() {
            return Err(crate::speedtest::SpeedtestError::EmptySelection);
        }
        manager
            .run_with_callback(&self.database, config, profile_ids, on_result)
            .await
    }

    #[must_use]
    pub fn runtime_paths(&self) -> &AppPaths {
        &self.runtime_paths
    }

    #[must_use]
    pub fn speedtest_manager(
        &self,
        core_seed_resource_dir: Option<std::path::PathBuf>,
        runner: Arc<dyn ProcessRunner>,
    ) -> SpeedtestManager {
        SpeedtestManager::new(self.runtime_paths.clone(), core_seed_resource_dir, runner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_save::settings_from_app_config;
    use voya_contracts::{AppSettingsV1, SystemProxyType};

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
        let initial = services
            .load_config()
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
        let persisted = reopened
            .load_config()
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

                let loaded = services.load_config().await.expect("settings read");
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
                let reloaded = reopened.load_config().await.expect("reload settings");
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
