use std::sync::{Arc, RwLock};

use thiserror::Error;
use tokio::sync::{Mutex, MutexGuard};
/// The in-memory configuration every mutation works on.
///
/// Re-exported because a host takes one by reference — to restart a connected
/// core for a committed change, say — and both hosts are barred from depending
/// on `voya-core` directly (ADR 0012, and `pnpm run check:architecture`).
pub use voya_core::AppConfig;
/// Re-exported so shells can name the handle [`ConfigMutationGuard::split`]
/// already hands them without reaching into `voya-db` themselves — the facade
/// is the only persistence boundary a shell is allowed to see.
pub use voya_db::UnitOfWork;
use voya_db::{Database, DbError};
use voya_platform::coreinfo::TargetOs;

use crate::{
    connection_mode::enforce_platform_connection_mode,
    settings::save::{settings_from_app_config, state_from_app_config},
};

pub type SharedAppConfig = Arc<RwLock<AppConfig>>;

#[derive(Debug, Error)]
pub enum ConfigMutationError {
    #[error(transparent)]
    Database(#[from] DbError),
}

#[derive(Debug)]
pub struct ConfigMutationCoordinator {
    database: Database,
    config: SharedAppConfig,
    mutation_lock: Mutex<()>,
    /// The platform every commit is kept within; unset in unit tests.
    platform: Option<TargetOs>,
}

impl ConfigMutationCoordinator {
    #[must_use]
    pub fn new(database: Database, config: SharedAppConfig) -> Self {
        Self {
            database,
            config,
            mutation_lock: Mutex::new(()),
            platform: None,
        }
    }

    /// Keep every committed configuration within what `target_os` offers, so
    /// no command can persist a capture mode the platform does not have.
    #[must_use]
    pub fn with_target_os(mut self, target_os: TargetOs) -> Self {
        self.platform = Some(target_os);
        self
    }

    #[must_use]
    pub fn current_config(&self) -> AppConfig {
        read_config(&self.config)
    }

    /// Runs one operation inside a configuration mutation and commits it.
    ///
    /// Thirteen commands spelled this sequence out by hand — begin, clone the
    /// configuration to compare against, `split()`, call one manager, derive
    /// `config_changed`, commit — and only the manager call and the cache
    /// invalidation that follows the commit ever differed. The invalidation
    /// needs an `AppHandle` and stays in the shell; everything above it is here,
    /// where the ordering (compare *before* commit, commit *after* the
    /// operation) can be asserted.
    ///
    /// `config_changed` deliberately over-approximates in the safe direction:
    /// it compares the whole `AppConfig`, so a command that rewrote it is never
    /// missed, at the cost of an occasional extra refetch.
    pub async fn mutate<T, E, F>(&self, operation: F) -> Result<CommittedMutation<T>, E>
    where
        F: AsyncFnOnce(&UnitOfWork, &mut AppConfig) -> Result<T, E>,
        E: From<ConfigMutationError>,
    {
        let mut mutation = self.begin().await?;
        let original = mutation.config().clone();
        let value = {
            let (unit_of_work, config) = mutation.split();
            operation(unit_of_work, config).await?
        };
        let config_changed = original != *mutation.config();
        let config = mutation.commit().await?;

        Ok(CommittedMutation {
            value,
            config,
            config_changed,
        })
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

/// What one committed configuration mutation produced.
#[derive(Debug)]
pub struct CommittedMutation<T> {
    /// Whatever the operation returned.
    pub value: T,
    /// The configuration as committed.
    pub config: AppConfig,
    /// Whether the commit rewrote the persisted configuration, and therefore
    /// whether the settings bundle projected from it is stale. See
    /// `crate::invalidation`.
    pub config_changed: bool,
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

    pub async fn commit(mut self) -> Result<AppConfig, ConfigMutationError> {
        if let Some(target_os) = self.coordinator.platform {
            enforce_platform_connection_mode(&mut self.working_config, target_os);
        }
        let settings = settings_from_app_config(&self.working_config);
        let state = state_from_app_config(&self.working_config);
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

    #[tokio::test]
    async fn a_macos_coordinator_never_commits_a_system_proxy_mode() {
        let database = Database::connect_in_memory().await.expect("database");
        let shared = Arc::new(RwLock::new(AppConfig::default()));
        let coordinator = ConfigMutationCoordinator::new(database, Arc::clone(&shared))
            .with_target_os(TargetOs::Macos);

        let mut mutation = coordinator.begin().await.expect("begin");
        mutation.config_mut().tun_mode_item.enable_tun = false;
        let committed = mutation.commit().await.expect("commit");

        assert!(committed.tun_mode_item.enable_tun);
        assert!(read_config(&shared).tun_mode_item.enable_tun);
    }
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
                mutation.config_mut().ui_item.current_language = "zh-Hant".to_string();
                mutation.commit().await.expect("mutation should commit");
            })
        };
        release_first.wait().await;
        first.await.expect("first mutation task should finish");
        second.await.expect("second mutation task should finish");

        let final_config = coordinator.current_config();
        assert_eq!(final_config.ui_item.current_theme.as_deref(), Some("Dark"));
        assert_eq!(final_config.ui_item.current_language, "zh-Hant");
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
    async fn mutate_commits_and_reports_that_the_config_changed() {
        let database = Database::connect_in_memory()
            .await
            .expect("coordinator database should connect");
        let shared = Arc::new(RwLock::new(AppConfig::default()));
        let coordinator = ConfigMutationCoordinator::new(database, Arc::clone(&shared));

        let committed = coordinator
            .mutate(async |_unit_of_work, config| {
                config.ui_item.current_language = "zh-Hant".to_string();
                Ok::<_, ConfigMutationError>("saved")
            })
            .await
            .expect("mutation should commit");

        assert_eq!(committed.value, "saved");
        assert!(committed.config_changed);
        assert_eq!(committed.config.ui_item.current_language, "zh-Hant");
        // Committed means published: the next reader sees it without a refetch.
        assert_eq!(
            coordinator.current_config().ui_item.current_language,
            "zh-Hant"
        );
    }

    /// `config_changed` drives the settings-bundle invalidation, so a mutation
    /// that only wrote business rows must report `false` — otherwise every
    /// profile save refetches a bundle nothing touched.
    #[tokio::test]
    async fn mutate_reports_no_config_change_when_only_rows_were_written() {
        let database = Database::connect_in_memory()
            .await
            .expect("coordinator database should connect");
        let shared = Arc::new(RwLock::new(AppConfig::default()));
        let coordinator = ConfigMutationCoordinator::new(database.clone(), Arc::clone(&shared));

        let committed = coordinator
            .mutate(async |unit_of_work, _config| {
                unit_of_work
                    .subscriptions()
                    .upsert(&voya_core::SubItem {
                        id: "subscription-a".to_string(),
                        remarks: "Subscription".to_string(),
                        ..voya_core::SubItem::default()
                    })
                    .await?;
                Ok::<_, ConfigMutationError>(())
            })
            .await
            .expect("mutation should commit");

        assert!(!committed.config_changed);
        assert!(database
            .subscriptions()
            .get("subscription-a")
            .await
            .expect("subscription lookup should succeed")
            .is_some());
    }

    /// A failed operation must not reach `commit()`: the guard is dropped, so
    /// the transaction rolls back and the in-memory config is left alone.
    #[tokio::test]
    async fn mutate_rolls_back_when_the_operation_fails() {
        let database = Database::connect_in_memory()
            .await
            .expect("coordinator database should connect");
        let shared = Arc::new(RwLock::new(AppConfig::default()));
        let coordinator = ConfigMutationCoordinator::new(database.clone(), Arc::clone(&shared));

        let failure = coordinator
            .mutate(async |unit_of_work, config| {
                config.ui_item.current_language = "zh-Hant".to_string();
                unit_of_work
                    .subscriptions()
                    .upsert(&voya_core::SubItem {
                        id: "subscription-a".to_string(),
                        remarks: "Subscription".to_string(),
                        ..voya_core::SubItem::default()
                    })
                    .await?;
                Err::<(), _>(ConfigMutationError::Database(DbError::InvalidEnum {
                    enum_name: "ProfileProtocol",
                    value: "unknown".to_string(),
                }))
            })
            .await;

        assert!(failure.is_err());
        assert_eq!(coordinator.current_config().ui_item.current_language, "en");
        assert!(database
            .subscriptions()
            .get("subscription-a")
            .await
            .expect("subscription lookup should succeed")
            .is_none());
    }
}
