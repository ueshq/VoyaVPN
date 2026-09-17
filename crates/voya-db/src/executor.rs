use std::{future::Future, pin::Pin};

use sqlx::{Sqlite, SqliteConnection, SqlitePool, Transaction};
use tokio::sync::Mutex;

use crate::Result;

#[derive(Debug, Clone, Copy)]
pub(crate) enum RepositoryExecutor<'executor> {
    Pool(&'executor SqlitePool),
    Transaction(&'executor Mutex<Transaction<'static, Sqlite>>),
}

macro_rules! run_query {
    ($executor:expr, $query:expr, $method:ident) => {{
        let query = $query;
        match $executor {
            $crate::executor::RepositoryExecutor::Pool(pool) => query.$method(pool).await,
            $crate::executor::RepositoryExecutor::Transaction(transaction) => {
                let mut transaction = transaction.lock().await;
                query.$method(&mut **transaction).await
            }
        }
    }};
}

/// Declares the two executor constructors every repository needs.
///
/// The repository struct stays hand-written so its derives and field stay
/// greppable; only the boilerplate that was byte-identical across all eight
/// repositories is generated here.
macro_rules! repository_constructors {
    ($repository:ident) => {
        impl<'executor> $repository<'executor> {
            #[must_use]
            pub(crate) const fn new(pool: &'executor sqlx::SqlitePool) -> Self {
                Self {
                    executor: $crate::executor::RepositoryExecutor::Pool(pool),
                }
            }

            #[must_use]
            pub(crate) const fn new_in_transaction(
                transaction: &'executor tokio::sync::Mutex<
                    sqlx::Transaction<'static, sqlx::Sqlite>,
                >,
            ) -> Self {
                Self {
                    executor: $crate::executor::RepositoryExecutor::Transaction(transaction),
                }
            }
        }
    };
}

/// A batch of statements that borrows one connection for its whole run.
pub(crate) type ConnectionFuture<'connection, T> =
    Pin<Box<dyn Future<Output = Result<T>> + Send + 'connection>>;

/// Runs a multi-statement batch on one connection, all-or-nothing.
///
/// `run_query!` cannot express this: every statement has to see the same
/// connection, and the repository may only own — and therefore commit — a
/// transaction when it is driving the pool directly. Inside a
/// [`crate::UnitOfWork`] the batch joins the caller's transaction so the caller
/// still decides when to commit, and a mid-batch failure leaves the whole unit
/// to roll back.
///
/// Whatever the batch borrows travels in `input`: the connection handed to
/// `work` lives shorter than the caller's data, so the closure cannot capture
/// that data by reference itself.
pub(crate) async fn with_connection<Input, T>(
    executor: RepositoryExecutor<'_>,
    input: &Input,
    work: impl for<'connection> FnOnce(
        &'connection mut SqliteConnection,
        &'connection Input,
    ) -> ConnectionFuture<'connection, T>,
) -> Result<T>
where
    Input: ?Sized + Sync,
{
    match executor {
        RepositoryExecutor::Pool(pool) => {
            // A deferred transaction is enough because every batch run here
            // only writes: SQLite refuses the busy handler when a transaction
            // upgrades a read lock to a write lock, never when it takes the
            // write lock with its first statement.
            let mut transaction = pool.begin().await?;
            let value = work(&mut transaction, input).await?;
            transaction.commit().await?;
            Ok(value)
        }
        RepositoryExecutor::Transaction(transaction) => {
            let mut transaction = transaction.lock().await;
            work(&mut transaction, input).await
        }
    }
}

/// Runs one delete statement per id as a single [`with_connection`] batch and
/// returns how many rows it removed.
pub(crate) async fn delete_each(
    executor: RepositoryExecutor<'_>,
    statement: &'static str,
    ids: &[String],
) -> Result<u64> {
    with_connection(executor, ids, |connection, ids| {
        Box::pin(delete_each_on(connection, statement, ids))
    })
    .await
}

/// `SELECT MAX(sort) FROM <table>`: the highest sort value, or 0 for an empty
/// table.
pub(crate) async fn max_sort(
    executor: RepositoryExecutor<'_>,
    statement: &'static str,
) -> Result<i32> {
    let max: Option<i32> = run_query!(executor, sqlx::query_scalar(statement), fetch_one)?;

    Ok(max.unwrap_or(0))
}

/// `SELECT EXISTS(SELECT 1 FROM <table> WHERE <id> = ?)` with the id bound.
pub(crate) async fn row_exists(
    executor: RepositoryExecutor<'_>,
    statement: &'static str,
    id: &str,
) -> Result<bool> {
    let found: i64 = run_query!(executor, sqlx::query_scalar(statement).bind(id), fetch_one)?;

    Ok(found != 0)
}

async fn delete_each_on(
    connection: &mut SqliteConnection,
    statement: &'static str,
    ids: &[String],
) -> Result<u64> {
    let mut deleted = 0;
    for id in ids {
        let result = sqlx::query(statement)
            .bind(id)
            .execute(&mut *connection)
            .await?;
        deleted += result.rows_affected();
    }

    Ok(deleted)
}

pub(crate) use repository_constructors;
pub(crate) use run_query;
