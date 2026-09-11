use sqlx::{sqlite::SqliteRow, Row};
use voya_core::SubItem;

use crate::{
    executor::{repository_constructors, run_query, RepositoryExecutor},
    Result,
};

#[derive(Debug, Clone, Copy)]
pub struct SubscriptionRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(SubscriptionRepository);

impl<'executor> SubscriptionRepository<'executor> {
    pub async fn upsert(&self, item: &SubItem) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO subscriptions (
                id, remarks, url, more_url, enabled, user_agent, sort, filter,
                convert_target, auto_update_interval_minutes
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                remarks = excluded.remarks,
                url = excluded.url,
                more_url = excluded.more_url,
                enabled = excluded.enabled,
                user_agent = excluded.user_agent,
                sort = excluded.sort,
                filter = excluded.filter,
                convert_target = excluded.convert_target,
                auto_update_interval_minutes = excluded.auto_update_interval_minutes
            "#,
            )
            .bind(&item.id)
            .bind(&item.remarks)
            .bind(&item.url)
            .bind(&item.more_url)
            .bind(item.enabled)
            .bind(&item.user_agent)
            .bind(item.sort)
            .bind(&item.filter)
            .bind(&item.convert_target)
            .bind(item.auto_update_interval_minutes),
            execute
        )?;

        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<SubItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM subscriptions WHERE id = ?").bind(id),
            fetch_optional
        )?;

        row.map(row_to_subscription).transpose()
    }

    pub async fn get_by_url(&self, url: &str) -> Result<Option<SubItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM subscriptions WHERE url = ?").bind(url),
            fetch_optional
        )?;

        row.map(row_to_subscription).transpose()
    }

    pub async fn list(&self) -> Result<Vec<SubItem>> {
        let rows = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM subscriptions ORDER BY sort, id"),
            fetch_all
        )?;

        rows.into_iter().map(row_to_subscription).collect()
    }

    pub async fn max_sort(&self) -> Result<i32> {
        let max_sort: Option<i32> = run_query!(
            self.executor,
            sqlx::query_scalar("SELECT MAX(sort) FROM subscriptions"),
            fetch_one
        )?;

        Ok(max_sort.unwrap_or(0))
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = run_query!(
            self.executor,
            sqlx::query("DELETE FROM subscriptions WHERE id = ?").bind(id),
            execute
        )?;

        Ok(result.rows_affected() > 0)
    }
}

fn row_to_subscription(row: SqliteRow) -> Result<SubItem> {
    Ok(SubItem {
        id: row.try_get("id")?,
        remarks: row.try_get("remarks")?,
        url: row.try_get("url")?,
        more_url: row.try_get("more_url")?,
        enabled: row.try_get("enabled")?,
        user_agent: row.try_get("user_agent")?,
        sort: row.try_get("sort")?,
        filter: row.try_get("filter")?,
        convert_target: row.try_get("convert_target")?,
        auto_update_interval_minutes: row.try_get("auto_update_interval_minutes")?,
    })
}
