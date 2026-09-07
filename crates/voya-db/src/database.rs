use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous},
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

/// How long a file-backed connection waits on SQLite's busy handler before it
/// gives up. It has to outlast a statistics commit plus the mutation that is
/// holding the write lock, so it is deliberately longer than SQLite's 5 s default.
const BUSY_TIMEOUT: Duration = Duration::from_secs(15);

/// Statement that opens a [`UnitOfWork`].
///
/// Every mutation reads before it writes, and SQLite refuses to invoke the busy
/// handler when a deferred transaction upgrades its read lock to a write lock, so
/// an overlapping autocommit writer (the statistics aggregator, a speedtest) would
/// make the whole mutation fail instantly with `database is locked`. Taking the
/// write lock up front makes writers queue on the busy handler instead.
const BEGIN_IMMEDIATE: &str = "BEGIN IMMEDIATE";

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
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        validate_existing_schema(&pool, path).await?;
        enable_write_ahead_logging(&pool).await?;
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
            transaction: Mutex::new(self.pool.begin_with(BEGIN_IMMEDIATE).await?),
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

/// Switches the database file to write-ahead logging.
///
/// Runs after [`validate_existing_schema`] on purpose: the pragma rewrites the
/// file header, and a database that is not ours must be left byte-for-byte
/// untouched. WAL is a persistent property of the file, so connections the pool
/// opens later inherit it without repeating the pragma, and it must be set before
/// any concurrent task starts because switching journal mode needs an exclusive
/// lock that the busy handler cannot wait on.
async fn enable_write_ahead_logging(pool: &SqlitePool) -> Result<()> {
    sqlx::raw_sql("PRAGMA journal_mode = WAL")
        .execute(pool)
        .await?;
    Ok(())
}

/// Rejects databases this build cannot migrate, before sqlx reports the same
/// condition as an opaque `VersionMissing` without the file path or a reset hint.
async fn validate_existing_schema(pool: &SqlitePool, path: &Path) -> Result<()> {
    let user_table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    if user_table_count == 0 {
        return Ok(());
    }

    let expected = latest_migration_version();
    let found = applied_migration_version(pool).await?;
    match found {
        Some(applied) if applied <= expected => Ok(()),
        _ => Err(DbError::UnsupportedDatabaseSchema {
            path: path.to_path_buf(),
            found,
            expected,
            manual_reset_command: manual_database_reset_command(path),
        }),
    }
}

/// Highest migration version this build knows how to apply.
fn latest_migration_version() -> i64 {
    MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or_default()
}

/// Highest migration version recorded in the sqlx bookkeeping table, or `None`
/// when the file has user tables that this application never created.
async fn applied_migration_version(pool: &SqlitePool) -> Result<Option<i64>> {
    let has_bookkeeping: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if has_bookkeeping == 0 {
        return Ok(None);
    }

    let applied = sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await?;
    Ok(applied)
}

/// The write-ahead log and shared-memory sidecars have to go with the database
/// file: SQLite would otherwise recover the old contents from a leftover `-wal`.
#[cfg(windows)]
fn manual_database_reset_command(path: &Path) -> String {
    let database = path.display();
    format!("Remove-Item -LiteralPath '{database}','{database}-wal','{database}-shm' -ErrorAction SilentlyContinue")
}

/// The write-ahead log and shared-memory sidecars have to go with the database
/// file: SQLite would otherwise recover the old contents from a leftover `-wal`.
#[cfg(not(windows))]
fn manual_database_reset_command(path: &Path) -> String {
    let database = path.display();
    format!("rm -f -- '{database}' '{database}-wal' '{database}-shm'")
}

#[cfg(test)]
mod tests;
