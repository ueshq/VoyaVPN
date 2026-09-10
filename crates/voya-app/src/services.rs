//! Application-owned facade over persistence-backed use cases.
//!
//! Desktop shells depend on this facade instead of constructing repositories or
//! passing the database handle through command adapters.

use std::{path::Path, sync::Arc};

use voya_contracts::{AppSettingsV1, SpeedtestKind, SystemProxyType};
pub use voya_core::{AppConfig, CoreType, SysProxyType, TrafficMode, DEFAULT_LOCAL_PORT};
use voya_db::{Database, DbError};
use voya_platform::{coreinfo::TargetOs, paths::AppPaths, process::ProcessRunner};

use crate::{
    config_mutation::{ConfigMutationCoordinator, SharedAppConfig},
    dns::DnsManager,
    exports::ExportManager,
    groups::GroupManager,
    presets::PresetManager,
    profiles::{ProfileExManager, ProfileManager},
    routing::RoutingManager,
    runtime::RuntimeManager,
    settings_save::{app_config_from_settings, config_from_settings, settings_from_app_config},
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
}

impl AppServices {
    pub async fn connect(database_path: &Path, runtime_paths: AppPaths) -> Result<Self, DbError> {
        Ok(Self {
            database: Database::connect(database_path).await?,
            runtime_paths,
            runtime_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    /// Read the persisted settings and project them onto an `AppConfig`.
    ///
    /// Only the database can fail here: every settings field is typed, so the
    /// projection itself is total.
    pub async fn load_config(&self) -> Result<AppConfig, DbError> {
        let mut settings = self.database.settings().load().await?;
        // Retire the former local-only choice without changing an existing TUN
        // or PAC selection. Only the saved preference changes here; automatic
        // system proxy management waits until the core connects.
        if !settings.network.tun.enabled
            && matches!(
                settings.network.system_proxy.mode,
                SystemProxyType::ForcedClear | SystemProxyType::Unchanged
            )
        {
            settings.network.system_proxy.mode = SystemProxyType::ForcedChange;
        }
        let state = self.database.app_state().load().await?;
        self.database
            .settings()
            .save_with_state(&settings, &state)
            .await?;
        Ok(app_config_from_settings(&settings, &state))
    }

    #[must_use]
    pub fn config_mutations(&self, config: SharedAppConfig) -> ConfigMutationCoordinator {
        ConfigMutationCoordinator::new(self.database.clone(), config)
    }

    #[must_use]
    pub fn config_from_settings(&self, settings: &AppSettingsV1, current: &AppConfig) -> AppConfig {
        config_from_settings(settings, current)
    }

    pub async fn initialize_profile_metrics(&self) -> crate::profiles::Result<u64> {
        ProfileExManager::new(&self.database).init().await
    }

    #[must_use]
    pub fn profiles(&self) -> ProfileManager<'_> {
        ProfileManager::new(&self.database)
    }

    #[must_use]
    pub fn groups(&self) -> GroupManager<'_> {
        GroupManager::new(&self.database)
    }

    #[must_use]
    pub fn subscriptions(&self) -> SubscriptionManager<'_> {
        SubscriptionManager::new(&self.database)
    }

    #[must_use]
    pub fn routings(&self) -> RoutingManager<'_> {
        RoutingManager::new(&self.database)
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
    pub fn presets(&self) -> PresetManager<'_> {
        PresetManager::new(&self.database)
    }

    #[must_use]
    pub fn updates(&self) -> UpdateManager<'_> {
        UpdateManager::new(&self.database, self.runtime_paths.clone())
    }

    #[must_use]
    pub fn runtime(&self, supervisor: CoreSupervisor) -> RuntimeManager<'_> {
        RuntimeManager::new(&self.database, self.runtime_paths.clone(), supervisor)
            .with_operation_lock(Arc::clone(&self.runtime_lock))
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
        kind: SpeedtestKind,
        profile_ids: Vec<String>,
        on_result: F,
    ) -> crate::speedtest::Result<SpeedtestRunResult>
    where
        F: Fn(SpeedtestResult) + Send + Sync,
    {
        manager
            .run_with_callback(&self.database, config, kind, profile_ids, on_result)
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

    #[must_use]
    pub fn settings_snapshot(config: &AppConfig) -> AppSettingsV1 {
        settings_from_app_config(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn loading_retires_non_tun_local_only_preferences_and_preserves_other_settings() {
        for mode in [
            SystemProxyType::ForcedClear,
            SystemProxyType::Unchanged,
            SystemProxyType::ForcedChange,
            SystemProxyType::Pac,
        ] {
            for tun_enabled in [false, true] {
                let app_dir = std::env::temp_dir().join(format!(
                    "voyavpn-mode-upgrade-test-{}",
                    uuid::Uuid::new_v4()
                ));
                let database_path = app_dir.join(voya_db::DATABASE_NAME);
                let paths = AppPaths::new(&app_dir);
                let services = AppServices::connect(&database_path, paths.clone())
                    .await
                    .expect("test database");
                let mut expected = AppSettingsV1::default();
                expected.behavior.auto_create_subscription_group = Some(false);
                expected.network.system_proxy.mode = mode;
                expected.network.tun.enabled = tun_enabled;
                expected.network.system_proxy.exceptions = "localhost,example.test".to_string();
                services
                    .database
                    .settings()
                    .save(&expected)
                    .await
                    .expect("old settings");
                if !tun_enabled
                    && matches!(
                        mode,
                        SystemProxyType::ForcedClear | SystemProxyType::Unchanged
                    )
                {
                    expected.network.system_proxy.mode = SystemProxyType::ForcedChange;
                }

                let loaded = services.load_config().await.expect("upgraded settings");
                assert_eq!(AppServices::settings_snapshot(&loaded), expected);
                assert_eq!(
                    services
                        .database
                        .settings()
                        .load()
                        .await
                        .expect("persisted upgrade"),
                    expected
                );
                services.database.close().await;

                let reopened = AppServices::connect(&database_path, paths)
                    .await
                    .expect("reopen");
                let reloaded = reopened
                    .load_config()
                    .await
                    .expect("reload upgraded settings");
                assert_eq!(AppServices::settings_snapshot(&reloaded), expected);
                reopened.database.close().await;
                std::fs::remove_dir_all(app_dir).expect("remove test database");
            }
        }
    }
}
