use sqlx::{sqlite::SqliteRow, Row};
use voya_core::RoutingItem;

use super::decode_rows;
use crate::{
    blob,
    executor::{delete_each, repository_constructors, run_query, RepositoryExecutor},
    Result,
};

/// Every routing column plus `is_active`, followed by `$tail`.
macro_rules! select_routing {
    ($tail:literal) => {
        concat!(
            "SELECT r.*, COALESCE(s.active_routing_id = r.id, 0) AS is_active FROM routing_items r CROSS JOIN app_state s ",
            $tail
        )
    };
}

/// `active_routing_id` is nullable — a fresh database has none, and deleting the
/// active set clears it through `ON DELETE SET NULL` — and SQLite evaluates
/// `NULL = r.id` to NULL rather than false. Decoding that into `is_active: bool`
/// fails, so every listing wraps the comparison in `COALESCE(..., 0)`.
#[derive(Debug, Clone, Copy)]
pub struct RoutingRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(RoutingRepository);

impl<'executor> RoutingRepository<'executor> {
    pub async fn upsert(&self, item: &RoutingItem) -> Result<()> {
        let rule_set = blob::rules_to_text(&item.rule_set)?;
        run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO routing_items (
                id, remarks, rule_set, enabled, locked,
                custom_icon, custom_ruleset_path4_singbox,
                domain_strategy4_singbox, sort
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                remarks = excluded.remarks,
                rule_set = excluded.rule_set,
                enabled = excluded.enabled,
                locked = excluded.locked,
                custom_icon = excluded.custom_icon,
                custom_ruleset_path4_singbox = excluded.custom_ruleset_path4_singbox,
                domain_strategy4_singbox = excluded.domain_strategy4_singbox,
                sort = excluded.sort
            "#,
            )
            .bind(&item.id)
            .bind(&item.remarks)
            .bind(rule_set)
            .bind(item.enabled)
            .bind(item.locked)
            .bind(&item.custom_icon)
            .bind(&item.custom_ruleset_path4_singbox)
            .bind(&item.domain_strategy4_singbox)
            .bind(item.sort),
            execute
        )?;

        Ok(())
    }

    /// Single-row lookup, deliberately strict; only [`Self::list`] skips rows it
    /// cannot decode.
    pub async fn get(&self, id: &str) -> Result<Option<RoutingItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query(select_routing!("WHERE r.id = ?")).bind(id),
            fetch_optional
        )?;

        row.map(|row| row_to_routing(&row)).transpose()
    }

    /// The rule sets in order, skipping any whose stored rules this build
    /// cannot decode: `rule_set` is an unversioned blob, and one bad set must
    /// not leave the routing screen empty.
    pub async fn list(&self) -> Result<Vec<RoutingItem>> {
        let rows = run_query!(
            self.executor,
            sqlx::query(select_routing!("ORDER BY r.sort, r.id")),
            fetch_all
        )?;

        let (items, _) = decode_rows(
            &rows,
            "id",
            "skipping a stored routing set this build cannot decode",
            "some routing sets were hidden because their stored rules could not be decoded",
            row_to_routing,
        )?;
        Ok(items)
    }

    pub async fn active(&self) -> Result<Option<RoutingItem>> {
        let row = run_query!(self.executor, sqlx::query(
            "SELECT r.*, 1 AS is_active FROM routing_items r JOIN app_state s ON s.active_routing_id = r.id ORDER BY r.sort, r.id LIMIT 1",
        ), fetch_optional)?;

        row.map(|row| row_to_routing(&row)).transpose()
    }

    pub async fn first(&self) -> Result<Option<RoutingItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query(select_routing!("ORDER BY r.sort, r.id LIMIT 1")),
            fetch_optional
        )?;

        row.map(|row| row_to_routing(&row)).transpose()
    }

    pub async fn exists(&self, id: &str) -> Result<bool> {
        let exists: i64 = run_query!(
            self.executor,
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM routing_items WHERE id = ?)").bind(id),
            fetch_one
        )?;

        Ok(exists != 0)
    }

    pub async fn max_sort(&self) -> Result<i32> {
        let max_sort: Option<i32> = run_query!(
            self.executor,
            sqlx::query_scalar("SELECT MAX(sort) FROM routing_items"),
            fetch_one
        )?;

        Ok(max_sort.unwrap_or(0))
    }

    pub async fn set_active(&self, id: &str) -> Result<bool> {
        if !self.exists(id).await? {
            return Ok(false);
        }

        let result = run_query!(
            self.executor,
            sqlx::query("UPDATE app_state SET active_routing_id = ? WHERE id = 1").bind(id),
            execute
        )?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = run_query!(
            self.executor,
            sqlx::query("DELETE FROM routing_items WHERE id = ?").bind(id),
            execute
        )?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_many(&self, ids: &[String]) -> Result<u64> {
        delete_each(self.executor, "DELETE FROM routing_items WHERE id = ?", ids).await
    }
}

fn row_to_routing(row: &SqliteRow) -> Result<RoutingItem> {
    let rule_set = row.try_get::<String, _>("rule_set")?;
    let rules = blob::rules_from_text(&rule_set)?;

    Ok(RoutingItem {
        id: row.try_get("id")?,
        remarks: row.try_get("remarks")?,
        rule_set: rules,
        enabled: row.try_get("enabled")?,
        locked: row.try_get("locked")?,
        custom_icon: row.try_get("custom_icon")?,
        custom_ruleset_path4_singbox: row.try_get("custom_ruleset_path4_singbox")?,
        domain_strategy4_singbox: row.try_get("domain_strategy4_singbox")?,
        sort: row.try_get("sort")?,
        is_active: row.try_get("is_active")?,
    })
}
