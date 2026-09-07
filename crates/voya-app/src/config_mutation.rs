use std::sync::{Arc, RwLock};

use thiserror::Error;
use tokio::sync::{Mutex, MutexGuard};
use voya_core::AppConfig;
use voya_db::{AppStateRecord, Database, DbError, UnitOfWork};

use crate::{
    dns::DnsManager, groups::GroupManager, presets::PresetManager, profiles::ProfileManager,
    routing::RoutingManager, settings_save::settings_from_app_config,
    subscriptions::SubscriptionManager,
};

pub type SharedAppConfig = Arc<RwLock<AppConfig>>;

#[derive(Debug, Error)]
pub enum ConfigMutationError {
    #[error(transparent)]
    Database(#[from] DbError),
}

#[derive(Debug)]
pub struct CommitCompensationFailure<CommitError, CompensationError> {
    pub commit: CommitError,
    pub compensation: Option<CompensationError>,
}

pub async fn commit_with_compensation<T, CommitError, CompensationError>(
    commit: impl std::future::Future<Output = Result<T, CommitError>>,
    compensate: impl FnOnce() -> Result<(), CompensationError>,
) -> Result<T, CommitCompensationFailure<CommitError, CompensationError>> {
    match commit.await {
        Ok(value) => Ok(value),
        Err(commit) => Err(CommitCompensationFailure {
            commit,
            compensation: compensate().err(),
        }),
    }
}

#[derive(Debug)]
pub struct ConfigMutationCoordinator {
    database: Database,
    config: SharedAppConfig,
    mutation_lock: Mutex<()>,
}

impl ConfigMutationCoordinator {
    #[must_use]
    pub fn new(database: Database, config: SharedAppConfig) -> Self {
        Self {
            database,
            config,
            mutation_lock: Mutex::new(()),
        }
    }

    #[must_use]
    pub fn shared_config(&self) -> SharedAppConfig {
        Arc::clone(&self.config)
    }

    #[must_use]
    pub fn config_lock(&self) -> &RwLock<AppConfig> {
        self.config.as_ref()
    }

    #[must_use]
    pub fn current_config(&self) -> AppConfig {
        read_config(&self.config)
    }

    pub async fn begin(&self) -> Result<ConfigMutationGuard<'_>, ConfigMutationError> {
        let mutation_lock = self.mutation_lock.lock().await;
        let working_config = read_config(&self.config);
        let unit_of_work = self.database.begin().await?;

        Ok(ConfigMutationGuard {
            coordinator: self,
            _mutation_lock: mutation_lock,
            unit_of_work,
            working_config,
        })
    }
}

#[derive(Debug)]
pub struct ConfigMutationGuard<'coordinator> {
    coordinator: &'coordinator ConfigMutationCoordinator,
    _mutation_lock: MutexGuard<'coordinator, ()>,
    unit_of_work: UnitOfWork,
    working_config: AppConfig,
}

impl ConfigMutationGuard<'_> {
    #[must_use]
    pub const fn config(&self) -> &AppConfig {
        &self.working_config
    }

    #[must_use]
    pub fn config_mut(&mut self) -> &mut AppConfig {
        &mut self.working_config
    }

    #[must_use]
    pub const fn unit_of_work(&self) -> &UnitOfWork {
        &self.unit_of_work
    }

    #[must_use]
    pub fn split(&mut self) -> (&UnitOfWork, &mut AppConfig) {
        (&self.unit_of_work, &mut self.working_config)
    }

    #[must_use]
    pub fn profiles(&self) -> ProfileManager<'_> {
        ProfileManager::new_in(&self.unit_of_work)
    }

    #[must_use]
    pub fn groups(&self) -> GroupManager<'_> {
        GroupManager::new_in(&self.unit_of_work)
    }

    #[must_use]
    pub fn subscriptions(&self) -> SubscriptionManager<'_> {
        SubscriptionManager::new_in(&self.unit_of_work)
    }

    #[must_use]
    pub fn routings(&self) -> RoutingManager<'_> {
        RoutingManager::new_in(&self.unit_of_work)
    }

    #[must_use]
    pub fn dns(&self) -> DnsManager<'_> {
        DnsManager::new_in(&self.unit_of_work)
    }

    #[must_use]
    pub fn presets(&self) -> PresetManager<'_> {
        PresetManager::new_in(&self.unit_of_work)
    }

    pub async fn commit(self) -> Result<AppConfig, ConfigMutationError> {
        let settings = settings_from_app_config(&self.working_config);
        let state = AppStateRecord {
            active_profile_id: (!self.working_config.index_id.is_empty())
                .then(|| self.working_config.index_id.clone()),
            active_routing_id: (!self
                .working_config
                .routing_basic_item
                .routing_index_id
                .is_empty())
            .then(|| {
                self.working_config
                    .routing_basic_item
                    .routing_index_id
                    .clone()
            }),
        };
        self.unit_of_work
            .settings()
            .save_with_state(&settings, &state)
            .await?;
        self.unit_of_work.commit().await?;
        publish_config(&self.coordinator.config, &self.working_config);

        Ok(self.working_config)
    }
}

fn read_config(config: &RwLock<AppConfig>) -> AppConfig {
    match config.read() {
        Ok(config) => config.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

fn publish_config(config: &RwLock<AppConfig>, updated: &AppConfig) {
    match config.write() {
        Ok(mut config) => *config = updated.clone(),
        Err(poisoned) => *poisoned.into_inner() = updated.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::Barrier;

    #[tokio::test]
    async fn concurrent_mutations_read_the_latest_committed_config() {
        let database = Database::connect_in_memory()
            .await
            .expect("coordinator database should connect");
        let shared = Arc::new(RwLock::new(AppConfig::default()));
        let coordinator = Arc::new(ConfigMutationCoordinator::new(
            database,
            Arc::clone(&shared),
        ));
        let first_started = Arc::new(Barrier::new(2));
        let release_first = Arc::new(Barrier::new(2));

        let first = {
            let coordinator = Arc::clone(&coordinator);
            let first_started = Arc::clone(&first_started);
            let release_first = Arc::clone(&release_first);
            tokio::spawn(async move {
                let mut mutation = coordinator.begin().await.expect("mutation should begin");
                mutation.config_mut().ui_item.current_theme = Some("Dark".to_string());
                first_started.wait().await;
                release_first.wait().await;
                mutation.commit().await.expect("mutation should commit");
            })
        };
        first_started.wait().await;

        let second = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                let mut mutation = coordinator.begin().await.expect("mutation should begin");
                assert_eq!(
                    mutation.config().ui_item.current_theme.as_deref(),
                    Some("Dark")
                );
                mutation.config_mut().ui_item.current_language = "fr".to_string();
                mutation.commit().await.expect("mutation should commit");
            })
        };
        release_first.wait().await;
        first.await.expect("first mutation task should finish");
        second.await.expect("second mutation task should finish");

        let final_config = coordinator.current_config();
        assert_eq!(final_config.ui_item.current_theme.as_deref(), Some("Dark"));
        assert_eq!(final_config.ui_item.current_language, "fr");
    }

    #[tokio::test]
    async fn failed_commit_keeps_database_and_memory_unchanged() {
        let database = Database::connect_in_memory()
            .await
            .expect("coordinator database should connect");
        sqlx::query(
            r#"
            CREATE TRIGGER reject_settings_insert
            BEFORE INSERT ON app_settings
            BEGIN
                SELECT RAISE(ABORT, 'blocked settings insert');
            END
            "#,
        )
        .execute(database.pool())
        .await
        .expect("failure trigger should be created");
        let shared = Arc::new(RwLock::new(AppConfig::default()));
        let coordinator = ConfigMutationCoordinator::new(database.clone(), Arc::clone(&shared));
        let mut mutation = coordinator.begin().await.expect("mutation should begin");
        mutation.config_mut().ui_item.current_theme = Some("Dark".to_string());
        mutation
            .unit_of_work()
            .subscriptions()
            .upsert(&voya_core::SubItem {
                id: "subscription-a".to_string(),
                remarks: "Subscription".to_string(),
                ..voya_core::SubItem::default()
            })
            .await
            .expect("business row should be staged");

        assert!(mutation.commit().await.is_err());
        assert!(coordinator.current_config().ui_item.current_theme.is_none());
        assert!(database
            .subscriptions()
            .get("subscription-a")
            .await
            .expect("subscription lookup should succeed")
            .is_none());
    }

    #[tokio::test]
    async fn failed_commit_runs_compensation_and_preserves_both_errors() {
        let compensated = AtomicBool::new(false);
        let failure = commit_with_compensation(async { Err::<(), _>("commit failed") }, || {
            compensated.store(true, Ordering::SeqCst);
            Err("restore failed")
        })
        .await
        .expect_err("commit should fail");

        assert!(compensated.load(Ordering::SeqCst));
        assert_eq!(failure.commit, "commit failed");
        assert_eq!(failure.compensation, Some("restore failed"));
    }

    #[tokio::test]
    async fn successful_commit_does_not_run_compensation() {
        let compensated = AtomicBool::new(false);
        let result = commit_with_compensation(async { Ok::<_, &str>(42) }, || {
            compensated.store(true, Ordering::SeqCst);
            Ok::<_, &str>(())
        })
        .await
        .expect("commit should succeed");

        assert_eq!(result, 42);
        assert!(!compensated.load(Ordering::SeqCst));
    }
}
