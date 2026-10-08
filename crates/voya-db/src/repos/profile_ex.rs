use sqlx::{sqlite::SqliteRow, Row};
use voya_contracts::{SpeedtestOutcome, SpeedtestResult};
use voya_core::{ProfileExItem, ProfileItem};

use crate::{
    blob,
    executor::{repository_constructors, run_query, RepositoryExecutor},
    Result,
};

/// What was last measured through each node. A node with no row has not been
/// measured.
#[derive(Debug, Clone, Copy)]
pub struct ProfileExRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(ProfileExRepository);

impl<'executor> ProfileExRepository<'executor> {
    pub async fn upsert(&self, item: &ProfileExItem) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO profile_ex_items (
                index_id, delay, message, ip_info, country_code
            ) VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(index_id) DO UPDATE SET
                delay = excluded.delay,
                message = excluded.message,
                ip_info = excluded.ip_info,
                country_code = excluded.country_code
            "#,
            )
            .bind(&item.index_id)
            .bind(item.delay)
            .bind(&item.message)
            .bind(&item.ip_info)
            .bind(&item.country_code),
            execute
        )?;

        Ok(())
    }

    /// Commit only results for the exact connection that was tested. The
    /// comparison and write are one SQLite statement, so an edit or deletion
    /// cannot race a read-then-write check and restore an obsolete country.
    pub async fn set_probe_result(
        &self,
        profile: &ProfileItem,
        result: &SpeedtestResult,
    ) -> Result<bool> {
        let (protocol, transport, tls) = blob::profile_blobs(profile)?;
        let pending = matches!(
            result.outcome,
            SpeedtestOutcome::Waiting | SpeedtestOutcome::Testing
        );
        let updated = run_query!(self.executor, sqlx::query(r#"
            INSERT INTO profile_ex_items (index_id, delay, message, ip_info, country_code)
            SELECT index_id, COALESCE(?, 0), ?, ?, ? FROM profile_items
            WHERE index_id = ? AND protocol = ? AND transport IS ? AND tls IS ?
            ON CONFLICT(index_id) DO UPDATE SET
                delay = COALESCE(?, profile_ex_items.delay),
                message = excluded.message,
                ip_info = COALESCE(excluded.ip_info, profile_ex_items.ip_info),
                country_code = CASE WHEN ? THEN profile_ex_items.country_code ELSE excluded.country_code END
        "#)
            .bind(result.delay)
            .bind(result.outcome.as_stored())
            .bind(&result.ip_info)
            .bind(&result.country_code)
            .bind(&profile.index_id)
            .bind(protocol).bind(transport).bind(tls)
            .bind(result.delay).bind(pending), execute)?;
        Ok(updated.rows_affected() != 0)
    }

    pub async fn get(&self, index_id: &str) -> Result<Option<ProfileExItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM profile_ex_items WHERE index_id = ?").bind(index_id),
            fetch_optional
        )?;

        row.map(row_to_profile_ex).transpose()
    }

    pub async fn ensure(&self, index_id: &str) -> Result<ProfileExItem> {
        if let Some(item) = self.get(index_id).await? {
            return Ok(item);
        }

        let item = ProfileExItem {
            index_id: index_id.to_string(),
            ..ProfileExItem::default()
        };
        self.upsert(&item).await?;

        Ok(item)
    }

    #[cfg(test)]
    pub async fn list(&self) -> Result<Vec<ProfileExItem>> {
        let rows = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM profile_ex_items ORDER BY index_id"),
            fetch_all
        )?;

        rows.into_iter().map(row_to_profile_ex).collect()
    }
}

fn row_to_profile_ex(row: SqliteRow) -> Result<ProfileExItem> {
    Ok(ProfileExItem {
        index_id: row.try_get("index_id")?,
        delay: row.try_get("delay")?,
        message: row.try_get("message")?,
        ip_info: row.try_get("ip_info")?,
        country_code: row.try_get("country_code")?,
    })
}
