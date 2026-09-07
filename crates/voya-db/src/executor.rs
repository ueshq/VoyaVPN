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

/// Runs one delete statement per id, all-or-nothing.
///
/// `run_query!` cannot express this: every statement has to see the same
/// connection, and the repository may only own — and therefore commit — a
/// transaction when it is driving the pool directly. Inside a
/// [`crate::UnitOfWork`] the batch joins the caller's transaction so the caller
/// still decides when to commit, and a mid-batch failure leaves the whole unit
/// to roll back.
pub(crate) async fn delete_each(
    executor: RepositoryExecutor<'_>,
    statement: &'static str,
    ids: &[String],
) -> Result<u64> {
    match executor {
        RepositoryExecutor::Pool(pool) => {
            let mut transaction = pool.begin().await?;
            let deleted = delete_each_on(&mut transaction, statement, ids).await?;
            transaction.commit().await?;
            Ok(deleted)
        }
        RepositoryExecutor::Transaction(transaction) => {
            let mut transaction = transaction.lock().await;
            let connection: &mut SqliteConnection = &mut transaction;
            delete_each_on(connection, statement, ids).await
        }
    }
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
