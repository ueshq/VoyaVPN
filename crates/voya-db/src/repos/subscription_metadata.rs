use sqlx::{sqlite::SqliteRow, Row};
use tokio::sync::Mutex;
use voya_core::SubMetadataItem;

use crate::{
    executor::{run_query, RepositoryExecutor},
    Result,
};

#[derive(Debug, Clone, Copy)]
pub struct SubscriptionMetadataRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

impl<'executor> SubscriptionMetadataRepository<'executor> {
    #[must_use]
    pub(crate) const fn new(pool: &'executor sqlx::SqlitePool) -> Self {
        Self {
            executor: RepositoryExecutor::Pool(pool),
        }
    }

    #[must_use]
    pub(crate) const fn new_in_transaction(
        transaction: &'executor Mutex<sqlx::Transaction<'static, sqlx::Sqlite>>,
    ) -> Self {
        Self {
            executor: RepositoryExecutor::Transaction(transaction),
        }
    }

    pub async fn upsert(&self, item: &SubMetadataItem) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO subscription_metadata (
                subscription_id, upload_bytes, download_bytes, total_bytes,
                expire_at, last_update_at, profile_title
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(subscription_id) DO UPDATE SET
                upload_bytes = excluded.upload_bytes,
                download_bytes = excluded.download_bytes,
                total_bytes = excluded.total_bytes,
                expire_at = excluded.expire_at,
                last_update_at = excluded.last_update_at,
                profile_title = excluded.profile_title
            "#,
            )
            .bind(&item.subscription_id)
            .bind(item.upload_bytes)
            .bind(item.download_bytes)
            .bind(item.total_bytes)
            .bind(item.expire_at)
            .bind(item.last_update_at)
            .bind(&item.profile_title),
            execute
        )?;

        Ok(())
    }

    pub async fn get(&self, subscription_id: &str) -> Result<Option<SubMetadataItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM subscription_metadata WHERE subscription_id = ?")
                .bind(subscription_id),
            fetch_optional
        )?;

        row.map(row_to_metadata).transpose()
    }

    pub async fn list(&self) -> Result<Vec<SubMetadataItem>> {
        let rows = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM subscription_metadata ORDER BY subscription_id"),
            fetch_all
        )?;

        rows.into_iter().map(row_to_metadata).collect()
    }
}

fn row_to_metadata(row: SqliteRow) -> Result<SubMetadataItem> {
    Ok(SubMetadataItem {
        subscription_id: row.try_get("subscription_id")?,
        upload_bytes: row.try_get("upload_bytes")?,
        download_bytes: row.try_get("download_bytes")?,
        total_bytes: row.try_get("total_bytes")?,
        expire_at: row.try_get("expire_at")?,
        last_update_at: row.try_get("last_update_at")?,
        profile_title: row.try_get("profile_title")?,
    })
}
