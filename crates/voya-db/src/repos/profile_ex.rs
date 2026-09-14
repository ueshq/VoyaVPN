use sqlx::{sqlite::SqliteRow, Row, SqliteConnection};
use voya_contracts::{SpeedtestOutcome, SpeedtestResult};
use voya_core::{ProfileExItem, ProfileItem};

use crate::{
    blob,
    executor::{repository_constructors, run_query, with_connection, RepositoryExecutor},
    Result,
};

/// Writes one profile's sort position without touching its other columns.
///
/// Shared by [`ProfileExRepository::set_sort`] and its batched sibling so the
/// single-row and the whole-list reorder can never drift apart on which columns
/// a reorder is allowed to overwrite.
const SET_SORT_STATEMENT: &str = r#"
    INSERT INTO profile_ex_items (index_id, sort) VALUES (?, ?)
    ON CONFLICT(index_id) DO UPDATE SET sort = excluded.sort
"#;

#[derive(Debug, Clone, Copy)]
pub struct ProfileExRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(ProfileExRepository);

impl<'executor> ProfileExRepository<'executor> {
    #[must_use]
    pub(crate) const fn from_executor(executor: RepositoryExecutor<'executor>) -> Self {
        Self { executor }
    }

    pub async fn upsert(&self, item: &ProfileExItem) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO profile_ex_items (
                index_id, delay, sort, message, ip_info, country_code
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(index_id) DO UPDATE SET
                delay = excluded.delay,
                sort = excluded.sort,
                message = excluded.message,
                ip_info = excluded.ip_info,
                country_code = excluded.country_code
            "#,
            )
            .bind(&item.index_id)
            .bind(item.delay)
            .bind(item.sort)
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
            sqlx::query("SELECT * FROM profile_ex_items ORDER BY sort, index_id"),
            fetch_all
        )?;

        rows.into_iter().map(row_to_profile_ex).collect()
    }

    pub async fn max_sort(&self) -> Result<i32> {
        let max_sort: Option<i32> = run_query!(
            self.executor,
            sqlx::query_scalar("SELECT MAX(sort) FROM profile_ex_items"),
            fetch_one
        )?;

        Ok(max_sort.unwrap_or(0))
    }

    /// Assigns one profile's sort position in a single statement.
    ///
    /// Reordering calls this once per profile, so the read-modify-write it used
    /// to do (`ensure`'s SELECT, its insert for a missing row, then an upsert)
    /// cost two to three separately committed statements per row — hundreds of
    /// fsynced commits for a large subscription. The upsert leaves every other
    /// column of an existing row untouched, which is exactly what the previous
    /// read-then-write did.
    pub async fn set_sort(&self, index_id: &str, sort: i32) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query(SET_SORT_STATEMENT).bind(index_id).bind(sort),
            execute
        )?;

        Ok(())
    }

    /// Assigns a whole ordering, all-or-nothing, in one transaction.
    ///
    /// A reorder rewrites every row that moved, and [`Self::set_sort`] on the
    /// pool autocommits — and therefore fsyncs — once per row, so reordering a
    /// large subscription paid hundreds of commits for one user gesture.
    ///
    /// The batch runs through `executor::with_connection`, so inside a
    /// [`crate::UnitOfWork`] it joins the caller's transaction: the caller still
    /// decides when to commit and a mid-batch failure leaves the whole unit to
    /// roll back.
    ///
    /// Entries are applied in the order given, so a caller that lists the same
    /// profile twice gets the last position it asked for, exactly as repeated
    /// [`Self::set_sort`] calls would.
    pub async fn set_sort_many(&self, entries: &[(&str, i32)]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        with_connection(self.executor, entries, |connection, entries| {
            Box::pin(set_sort_on(connection, entries))
        })
        .await
    }

    pub async fn delete_orphans(&self) -> Result<u64> {
        let result = run_query!(
            self.executor,
            sqlx::query(
                r#"
            DELETE FROM profile_ex_items
            WHERE index_id NOT IN (SELECT index_id FROM profile_items)
            "#,
            ),
            execute
        )?;

        Ok(result.rows_affected())
    }
}

async fn set_sort_on(connection: &mut SqliteConnection, entries: &[(&str, i32)]) -> Result<()> {
    for (index_id, sort) in entries {
        sqlx::query(SET_SORT_STATEMENT)
            .bind(*index_id)
            .bind(*sort)
            .execute(&mut *connection)
            .await?;
    }

    Ok(())
}

fn row_to_profile_ex(row: SqliteRow) -> Result<ProfileExItem> {
    Ok(ProfileExItem {
        index_id: row.try_get("index_id")?,
        delay: row.try_get("delay")?,
        sort: row.try_get("sort")?,
        message: row.try_get("message")?,
        ip_info: row.try_get("ip_info")?,
        country_code: row.try_get("country_code")?,
    })
}
