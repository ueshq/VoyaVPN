use std::{
    fs,
    path::{Path, PathBuf},
};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Sqlite, SqlitePool, Transaction,
};
use tokio::sync::Mutex;

use crate::{
    AppStateRepository, DbError, ProfileExRepository, ProfileRepository, Result, RoutingRepository,
    ServerStatRepository, SettingsRepository, SubscriptionMetadataRepository,
    SubscriptionRepository,
};

pub const DATABASE_NAME: &str = "voyavpn.sqlite";

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
    path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct UnitOfWork {
    transaction: Mutex<Transaction<'static, Sqlite>>,
}

#[derive(Debug, Clone, Copy)]
pub enum DatabaseSession<'database> {
    Database(&'database Database),
    UnitOfWork(&'database UnitOfWork),
}

impl Database {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| DbError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        validate_existing_schema(&pool, path).await?;
        MIGRATOR.run(&pool).await?;

        Ok(Self {
            pool,
            path: Some(path.to_path_buf()),
        })
    }

    pub async fn connect_in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(":memory:")
                    .foreign_keys(true),
            )
            .await?;
        MIGRATOR.run(&pool).await?;

        Ok(Self { pool, path: None })
    }

    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    #[must_use]
    pub fn profiles(&self) -> ProfileRepository<'_> {
        ProfileRepository::new(&self.pool)
    }

    #[must_use]
    pub fn profile_exs(&self) -> ProfileExRepository<'_> {
        ProfileExRepository::new(&self.pool)
    }

    #[must_use]
    pub fn server_stats(&self) -> ServerStatRepository<'_> {
        ServerStatRepository::new(&self.pool)
    }

    #[must_use]
    pub fn subscriptions(&self) -> SubscriptionRepository<'_> {
        SubscriptionRepository::new(&self.pool)
    }

    #[must_use]
    pub fn subscription_metadata(&self) -> SubscriptionMetadataRepository<'_> {
        SubscriptionMetadataRepository::new(&self.pool)
    }

    #[must_use]
    pub fn routings(&self) -> RoutingRepository<'_> {
        RoutingRepository::new(&self.pool)
    }

    #[must_use]
    pub fn settings(&self) -> SettingsRepository<'_> {
        SettingsRepository::new(&self.pool)
    }

    #[must_use]
    pub fn app_state(&self) -> AppStateRepository<'_> {
        AppStateRepository::new(&self.pool)
    }

    pub async fn begin(&self) -> Result<UnitOfWork> {
        Ok(UnitOfWork {
            transaction: Mutex::new(self.pool.begin().await?),
        })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

impl UnitOfWork {
    #[must_use]
    pub fn profiles(&self) -> ProfileRepository<'_> {
        ProfileRepository::new_in_transaction(&self.transaction)
    }

    #[must_use]
    pub fn profile_exs(&self) -> ProfileExRepository<'_> {
        ProfileExRepository::new_in_transaction(&self.transaction)
    }

    #[must_use]
    pub fn server_stats(&self) -> ServerStatRepository<'_> {
        ServerStatRepository::new_in_transaction(&self.transaction)
    }

    #[must_use]
    pub fn subscriptions(&self) -> SubscriptionRepository<'_> {
        SubscriptionRepository::new_in_transaction(&self.transaction)
    }

    #[must_use]
    pub fn subscription_metadata(&self) -> SubscriptionMetadataRepository<'_> {
        SubscriptionMetadataRepository::new_in_transaction(&self.transaction)
    }

    #[must_use]
    pub fn routings(&self) -> RoutingRepository<'_> {
        RoutingRepository::new_in_transaction(&self.transaction)
    }

    #[must_use]
    pub fn settings(&self) -> SettingsRepository<'_> {
        SettingsRepository::new_in_transaction(&self.transaction)
    }

    #[must_use]
    pub fn app_state(&self) -> AppStateRepository<'_> {
        AppStateRepository::new_in_transaction(&self.transaction)
    }

    pub async fn commit(self) -> Result<()> {
        self.transaction.into_inner().commit().await?;
        Ok(())
    }
}

impl<'database> DatabaseSession<'database> {
    #[must_use]
    pub const fn from_database(database: &'database Database) -> Self {
        Self::Database(database)
    }

    #[must_use]
    pub const fn from_unit_of_work(unit_of_work: &'database UnitOfWork) -> Self {
        Self::UnitOfWork(unit_of_work)
    }

    #[must_use]
    pub fn profiles(self) -> ProfileRepository<'database> {
        match self {
            Self::Database(database) => database.profiles(),
            Self::UnitOfWork(unit_of_work) => unit_of_work.profiles(),
        }
    }

    #[must_use]
    pub fn profile_exs(self) -> ProfileExRepository<'database> {
        match self {
            Self::Database(database) => database.profile_exs(),
            Self::UnitOfWork(unit_of_work) => unit_of_work.profile_exs(),
        }
    }

    #[must_use]
    pub fn server_stats(self) -> ServerStatRepository<'database> {
        match self {
            Self::Database(database) => database.server_stats(),
            Self::UnitOfWork(unit_of_work) => unit_of_work.server_stats(),
        }
    }

    #[must_use]
    pub fn subscriptions(self) -> SubscriptionRepository<'database> {
        match self {
            Self::Database(database) => database.subscriptions(),
            Self::UnitOfWork(unit_of_work) => unit_of_work.subscriptions(),
        }
    }

    #[must_use]
    pub fn subscription_metadata(self) -> SubscriptionMetadataRepository<'database> {
        match self {
            Self::Database(database) => database.subscription_metadata(),
            Self::UnitOfWork(unit_of_work) => unit_of_work.subscription_metadata(),
        }
    }

    #[must_use]
    pub fn routings(self) -> RoutingRepository<'database> {
        match self {
            Self::Database(database) => database.routings(),
            Self::UnitOfWork(unit_of_work) => unit_of_work.routings(),
        }
    }

    #[must_use]
    pub fn settings(self) -> SettingsRepository<'database> {
        match self {
            Self::Database(database) => database.settings(),
            Self::UnitOfWork(unit_of_work) => unit_of_work.settings(),
        }
    }

    #[must_use]
    pub fn app_state(self) -> AppStateRepository<'database> {
        match self {
            Self::Database(database) => database.app_state(),
            Self::UnitOfWork(unit_of_work) => unit_of_work.app_state(),
        }
    }
}

async fn validate_existing_schema(pool: &SqlitePool, path: &Path) -> Result<()> {
    let user_table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_one(pool)
    .await?;
    if user_table_count == 0 {
        return Ok(());
    }

    let has_metadata: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_metadata')",
    )
    .fetch_one(pool)
    .await?;
    let found = if has_metadata == 0 {
        None
    } else {
        sqlx::query_scalar::<_, i64>("SELECT version FROM schema_metadata WHERE id = 1")
            .fetch_optional(pool)
            .await?
    };
    if found != Some(1) {
        return Err(DbError::UnsupportedDatabaseSchema {
            path: path.to_path_buf(),
            found,
            expected: 1,
            manual_reset_command: manual_database_reset_command(path),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn manual_database_reset_command(path: &Path) -> String {
    format!("Remove-Item -LiteralPath '{}'", path.display())
}

#[cfg(not(windows))]
fn manual_database_reset_command(path: &Path) -> String {
    format!("rm -- '{}'", path.display())
}

#[cfg(test)]
mod tests;
