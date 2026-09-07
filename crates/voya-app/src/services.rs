//! Application-owned facade over persistence-backed use cases.
//!
//! Desktop shells depend on this facade instead of constructing repositories or
//! passing the database handle through command adapters.

use std::{path::Path, sync::Arc};

use thiserror::Error;
use voya_contracts::{AppSettingsV1, SpeedTestKind};
pub use voya_core::{AppConfig, CoreType, SysProxyType, TrafficMode, DEFAULT_LOCAL_PORT};
use voya_db::{AppStateRecord, Database, DbError};
use voya_platform::{paths::AppPaths, process::ProcessRunner};

use crate::{
    config_mutation::{ConfigMutationCoordinator, SharedAppConfig},
    dns::DnsManager,
    exports::ExportManager,
    groups::GroupManager,
    presets::PresetManager,
    profiles::{ProfileExManager, ProfileManager},
    routing::RoutingManager,
    runtime::RuntimeManager,
    settings_save::{app_config_from_settings, settings_from_app_config, SettingsContractError},
    speedtest::{SpeedTestResult, SpeedtestManager, SpeedtestRunResult},
    statistics::{StatisticsConfigSource, StatisticsEventSink, StatisticsManager},
    subscriptions::SubscriptionManager,
    supervisor::CoreSupervisor,
    updates::UpdateManager,
};

#[derive(Debug, Error)]
pub enum AppServicesError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Settings(#[from] SettingsContractError),
}

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

    pub async fn load_config(&self) -> Result<AppConfig, AppServicesError> {
        let settings = self.database.settings().load().await?;
        let state = self.database.app_state().load().await?;
        self.database
            .settings()
            .save_with_state(&settings, &state)
            .await?;
        Ok(app_config_from_settings(&settings, &state)?)
    }

    #[must_use]
    pub fn config_mutations(&self, config: SharedAppConfig) -> ConfigMutationCoordinator {
        ConfigMutationCoordinator::new(self.database.clone(), config)
    }

    #[must_use]
    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn config_from_settings(
        &self,
        settings: &AppSettingsV1,
        current: &AppConfig,
    ) -> Result<AppConfig, SettingsContractError> {
        let state = AppStateRecord {
            active_profile_id: (!current.index_id.is_empty()).then(|| current.index_id.clone()),
            active_routing_id: (!current.routing_basic_item.routing_index_id.is_empty())
                .then(|| current.routing_basic_item.routing_index_id.clone()),
        };
        app_config_from_settings(settings, &state)
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

    pub async fn run_speedtest<F>(
        &self,
        manager: &SpeedtestManager,
        config: &AppConfig,
        kind: SpeedTestKind,
        profile_ids: Vec<String>,
        on_result: F,
    ) -> crate::speedtest::Result<SpeedtestRunResult>
    where
        F: Fn(SpeedTestResult) + Send + Sync,
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
            SysProxyType::ForcedClear
        );
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
            SysProxyType::ForcedClear
        );
        reopened.database.close().await;

        std::fs::remove_dir_all(&app_dir).expect("test database directory should be removable");
    }
}
