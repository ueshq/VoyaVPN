use sqlx::{sqlite::SqliteRow, Row};
use voya_core::ServerStatItem;

use crate::{
    executor::{repository_constructors, run_query, RepositoryExecutor},
    Result,
};

#[derive(Debug, Clone, Copy)]
pub struct ServerStatRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(ServerStatRepository);

impl<'executor> ServerStatRepository<'executor> {
    pub async fn upsert(&self, item: &ServerStatItem) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO server_stat_items (
                index_id, total_up, total_down, today_up, today_down, day_number
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(index_id) DO UPDATE SET
                total_up = excluded.total_up,
                total_down = excluded.total_down,
                today_up = excluded.today_up,
                today_down = excluded.today_down,
                day_number = excluded.day_number
            "#,
            )
            .bind(&item.index_id)
            .bind(item.total_up)
            .bind(item.total_down)
            .bind(item.today_up)
            .bind(item.today_down)
            .bind(item.day_number),
            execute
        )?;

        Ok(())
    }

    pub async fn get(&self, index_id: &str) -> Result<Option<ServerStatItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM server_stat_items WHERE index_id = ?").bind(index_id),
            fetch_optional
        )?;

        row.map(row_to_server_stat).transpose()
    }

    pub async fn reset_rollover(&self, day_number: i64) -> Result<u64> {
        let result = run_query!(
            self.executor,
            sqlx::query(
                r#"
            UPDATE server_stat_items
            SET today_up = 0, today_down = 0, day_number = ?
            WHERE day_number <> ?
            "#,
            )
            .bind(day_number)
            .bind(day_number),
            execute
        )?;

        Ok(result.rows_affected())
    }

    /// Adds one sample to a profile's counters and returns the stored row.
    ///
    /// The statistics aggregator calls this once per flush for as long as a core
    /// is connected. It is one statement rather than the read-modify-write it
    /// used to be (a SELECT, a day-rollover upsert, then a second upsert), each
    /// its own autocommit transaction on the pool. Doing the arithmetic in SQL
    /// also makes the update atomic against any other writer.
    ///
    /// Unqualified column names in `DO UPDATE SET` are the row's values from
    /// before this insert, so the `CASE` compares the stored day against the
    /// sample's and starts the daily counters over when they differ — the day
    /// rollover that used to take an extra statement.
    ///
    /// `None` when the profile is gone. Its row went with it (the foreign key
    /// cascades), and traffic measured just before a node was deleted has
    /// nowhere to be counted: that is an answer, not a constraint failure.
    pub async fn add_traffic(
        &self,
        index_id: &str,
        day_number: i64,
        proxy_up: i64,
        proxy_down: i64,
    ) -> Result<Option<ServerStatItem>> {
        let up = proxy_up.max(0);
        let down = proxy_down.max(0);
        let row = run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO server_stat_items (
                index_id, total_up, total_down, today_up, today_down, day_number
            )
            SELECT ?, ?, ?, ?, ?, ?
            WHERE EXISTS (SELECT 1 FROM profile_items WHERE index_id = ?)
            ON CONFLICT(index_id) DO UPDATE SET
                total_up = total_up + excluded.total_up,
                total_down = total_down + excluded.total_down,
                today_up = CASE
                    WHEN day_number = excluded.day_number THEN today_up + excluded.today_up
                    ELSE excluded.today_up
                END,
                today_down = CASE
                    WHEN day_number = excluded.day_number THEN today_down + excluded.today_down
                    ELSE excluded.today_down
                END,
                day_number = excluded.day_number
            RETURNING *
            "#,
            )
            .bind(index_id)
            .bind(up)
            .bind(down)
            .bind(up)
            .bind(down)
            .bind(day_number)
            .bind(index_id),
            fetch_optional
        )?;

        row.map(row_to_server_stat).transpose()
    }
}

fn row_to_server_stat(row: SqliteRow) -> Result<ServerStatItem> {
    Ok(ServerStatItem {
        index_id: row.try_get("index_id")?,
        total_up: row.try_get("total_up")?,
        total_down: row.try_get("total_down")?,
        today_up: row.try_get("today_up")?,
        today_down: row.try_get("today_down")?,
        day_number: row.try_get("day_number")?,
    })
}
