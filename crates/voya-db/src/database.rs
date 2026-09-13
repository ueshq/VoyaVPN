use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous},
    Row, Sqlite, SqlitePool, Transaction,
};
use tokio::sync::Mutex;

use crate::{
    AppStateRepository, DbError, PolicyGroupRepository, ProfileExRepository, ProfileRepository,
    Result, RoutingRepository, ServerStatRepository, SettingsRepository,
    SubscriptionMetadataRepository, SubscriptionRepository,
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

        // Even closing a read/write connection can checkpoint a WAL left by a
        // crashed older build. Inspect existing files read-only first so a
        // refusal preserves both the database and its uncheckpointed data.
        if path.try_exists().map_err(|source| DbError::Io {
            path: path.to_path_buf(),
            source,
        })? {
            let inspection = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options.clone().read_only(true).create_if_missing(false))
                .await?;
            let result = validate_existing_schema(&inspection, path).await;
            inspection.close().await;
            result?;
        }

        // Validate before any persistent PRAGMA or schema write. Initialize on
        // a separate connection so the application pool's prepared statements
        // only ever see the complete current schema.
        {
            let migration_pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options.clone())
                .await?;
            let result = async {
                validate_existing_schema(&migration_pool, path).await?;
                enable_write_ahead_logging(&migration_pool).await?;
                MIGRATOR.run(&migration_pool).await?;
                crate::repos::normalize_retired_settings(&migration_pool).await?;
                crate::repos::normalize_retired_profile_blobs(&migration_pool).await
            }
            .await;
            migration_pool.close().await;
            result?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

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
        crate::repos::normalize_retired_settings(&pool).await?;
        crate::repos::normalize_retired_profile_blobs(&pool).await?;

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
}

/// Declares the same repository accessor on all three session types.
///
/// Every repository needs one accessor on [`Database`] (autocommit on the pool),
/// one on [`UnitOfWork`] (joining the caller's transaction) and one on
/// [`DatabaseSession`] that dispatches between them. Written out that was three
/// bodies per repository whose only variable is the type, and adding a
/// repository meant remembering all three.
macro_rules! session_accessors {
    ($($accessor:ident => $repository:ident),+ $(,)?) => {
        impl Database {
            $(
                #[must_use]
                pub fn $accessor(&self) -> $repository<'_> {
                    $repository::new(&self.pool)
                }
            )+
        }

        impl UnitOfWork {
            $(
                #[must_use]
                pub fn $accessor(&self) -> $repository<'_> {
                    $repository::new_in_transaction(&self.transaction)
                }
            )+
        }

        impl<'database> DatabaseSession<'database> {
            $(
                #[must_use]
                pub fn $accessor(self) -> $repository<'database> {
                    match self {
                        Self::Database(database) => database.$accessor(),
                        Self::UnitOfWork(unit_of_work) => unit_of_work.$accessor(),
                    }
                }
            )+
        }
    };
}

session_accessors! {
    profiles => ProfileRepository,
    profile_exs => ProfileExRepository,
    server_stats => ServerStatRepository,
    subscriptions => SubscriptionRepository,
    subscription_metadata => SubscriptionMetadataRepository,
    routings => RoutingRepository,
    settings => SettingsRepository,
    app_state => AppStateRepository,
    policy_groups => PolicyGroupRepository,
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

/// Accepts only a fresh database or the exact current baseline. This runs before
/// WAL or the migrator can write to an unsupported database.
async fn validate_existing_schema(pool: &SqlitePool, path: &Path) -> Result<()> {
    let user_table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    let expected = latest_migration_version();
    let unsupported = |found| DbError::UnsupportedDatabaseSchema {
        path: path.to_path_buf(),
        found,
        expected,
        manual_reset_command: manual_database_reset_command(path),
    };
    let has_bookkeeping: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if has_bookkeeping == 0 {
        return if user_table_count == 0 {
            Ok(())
        } else {
            Err(unsupported(None))
        };
    }

    let columns = sqlx::query("PRAGMA table_info(_sqlx_migrations)")
        .fetch_all(pool)
        .await?;
    if [
        "version",
        "description",
        "installed_on",
        "success",
        "checksum",
        "execution_time",
    ]
    .iter()
    .any(|name| {
        !columns
            .iter()
            .any(|row| row.get::<String, _>("name") == *name)
    }) {
        return Err(unsupported(None));
    }
    let rows =
        sqlx::query("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await?;
    // sqlx may have created its bookkeeping table before an interrupted first
    // initialization. Only an empty table with no application tables is fresh.
    if rows.is_empty() && user_table_count == 0 {
        return Ok(());
    }
    let found = rows
        .last()
        .and_then(|row| row.try_get::<i64, _>("version").ok());
    if rows.len() != 1 || user_table_count == 0 {
        return Err(unsupported(found));
    }
    let row = &rows[0];
    let success: i64 = row.try_get("success").map_err(|_| unsupported(found))?;
    let checksum: Vec<u8> = row.try_get("checksum").map_err(|_| unsupported(found))?;
    if success != 1
        || !MIGRATOR.iter().any(|baseline| {
            Some(baseline.version) == found && baseline.checksum.as_ref() == checksum.as_slice()
        })
    {
        return Err(unsupported(found));
    }
    Ok(())
}

/// The current baseline's identifier, separate from the settings DTO version.
fn latest_migration_version() -> i64 {
    MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or_default()
}

/// The write-ahead log and shared-memory sidecars have to go with the database
/// file: SQLite would otherwise recover the old contents from a leftover `-wal`.
#[cfg(windows)]
pub fn manual_database_reset_command(path: &Path) -> String {
    let database = path.display();
    format!("Remove-Item -LiteralPath '{database}','{database}-wal','{database}-shm' -ErrorAction SilentlyContinue")
}

/// The write-ahead log and shared-memory sidecars have to go with the database
/// file: SQLite would otherwise recover the old contents from a leftover `-wal`.
#[cfg(not(windows))]
pub fn manual_database_reset_command(path: &Path) -> String {
    let database = path.display();
    format!("rm -f -- '{database}' '{database}-wal' '{database}-shm'")
}

#[cfg(test)]
mod tests;
