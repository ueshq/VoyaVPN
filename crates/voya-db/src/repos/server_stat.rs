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
                index_id, total_up, total_down, today_up, today_down, date_now
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(index_id) DO UPDATE SET
                total_up = excluded.total_up,
                total_down = excluded.total_down,
                today_up = excluded.today_up,
                today_down = excluded.today_down,
                date_now = excluded.date_now
            "#,
            )
            .bind(&item.index_id)
            .bind(item.total_up)
            .bind(item.total_down)
            .bind(item.today_up)
            .bind(item.today_down)
            .bind(item.date_now),
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

    pub async fn ensure(&self, index_id: &str, date_now: i64) -> Result<ServerStatItem> {
        if let Some(mut item) = self.get(index_id).await? {
            if item.date_now != date_now {
                item.today_up = 0;
                item.today_down = 0;
                item.date_now = date_now;
                self.upsert(&item).await?;
            }

            return Ok(item);
        }

        let item = ServerStatItem {
            index_id: index_id.to_string(),
            date_now,
            ..ServerStatItem::default()
        };
        self.upsert(&item).await?;

        Ok(item)
    }

    pub async fn list(&self) -> Result<Vec<ServerStatItem>> {
        let rows = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM server_stat_items ORDER BY index_id"),
            fetch_all
        )?;

        rows.into_iter().map(row_to_server_stat).collect()
    }

    pub async fn delete_orphans(&self) -> Result<u64> {
        let result = run_query!(
            self.executor,
            sqlx::query(
                r#"
            DELETE FROM server_stat_items
            WHERE index_id NOT IN (SELECT index_id FROM profile_items)
            "#,
            ),
            execute
        )?;

        Ok(result.rows_affected())
    }

    pub async fn reset_rollover(&self, date_now: i64) -> Result<u64> {
        let result = run_query!(
            self.executor,
            sqlx::query(
                r#"
            UPDATE server_stat_items
            SET today_up = 0, today_down = 0, date_now = ?
            WHERE date_now <> ?
            "#,
            )
            .bind(date_now)
            .bind(date_now),
            execute
        )?;

        Ok(result.rows_affected())
    }

    /// Adds one sample to a profile's counters and returns the stored row.
    ///
    /// The statistics aggregator calls this every second for as long as a core
    /// is connected, so it is one statement rather than the read-modify-write it
    /// used to be (`ensure`'s SELECT, its rollover upsert, then a second
    /// upsert). Each of those was its own autocommit transaction on the pool,
    /// which meant two to three journalled commits per second just to bump four
    /// integers. Doing the arithmetic in SQL also makes the update atomic
    /// against any other writer.
    ///
    /// Unqualified column names in `DO UPDATE SET` are the row's values from
    /// before this insert, so the `CASE` compares the stored day against the
    /// sample's and starts the daily counters over when they differ — the day
    /// rollover `ensure` performed with an extra statement.
    pub async fn add_traffic(
        &self,
        index_id: &str,
        date_now: i64,
        proxy_up: i64,
        proxy_down: i64,
    ) -> Result<ServerStatItem> {
        let up = proxy_up.max(0);
        let down = proxy_down.max(0);
        let row = run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO server_stat_items (
                index_id, total_up, total_down, today_up, today_down, date_now
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(index_id) DO UPDATE SET
                total_up = total_up + excluded.total_up,
                total_down = total_down + excluded.total_down,
                today_up = CASE
                    WHEN date_now = excluded.date_now THEN today_up + excluded.today_up
                    ELSE excluded.today_up
                END,
                today_down = CASE
                    WHEN date_now = excluded.date_now THEN today_down + excluded.today_down
                    ELSE excluded.today_down
                END,
                date_now = excluded.date_now
            RETURNING *
            "#,
            )
            .bind(index_id)
            .bind(up)
            .bind(down)
            .bind(up)
            .bind(down)
            .bind(date_now),
            fetch_one
        )?;

        row_to_server_stat(row)
    }
}

fn row_to_server_stat(row: SqliteRow) -> Result<ServerStatItem> {
    Ok(ServerStatItem {
        index_id: row.try_get("index_id")?,
        total_up: row.try_get("total_up")?,
        total_down: row.try_get("total_down")?,
        today_up: row.try_get("today_up")?,
        today_down: row.try_get("today_down")?,
        date_now: row.try_get("date_now")?,
    })
}
