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

        // Validate, switch to WAL and migrate on a throwaway single-connection
        // pool, then drop it before opening the pool the application keeps.
        //
        // sqlx caches prepared statements per connection, and that cache holds
        // the column metadata each statement was prepared against. A migration
        // that runs `ALTER TABLE … ADD COLUMN` after a connection has already
        // served a query leaves that connection able to hand back rows shaped
        // like the *old* schema, which decodes as an out-of-bounds column read
        // for every query issued afterwards on it. Migrating in isolation means
        // no connection in the long-lived pool can predate the schema it serves.
        {
            let migration_pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options.clone())
                .await?;
            let result = async {
                validate_existing_schema(&migration_pool, path).await?;
                enable_write_ahead_logging(&migration_pool).await?;
                MIGRATOR.run(&migration_pool).await.map_err(DbError::from)
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
