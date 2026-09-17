use sqlx::{sqlite::SqliteRow, Row};
use voya_core::{GroupStrategy, PolicyGroupItem};

use crate::{
    executor::{
        delete_each, max_sort, repository_constructors, row_exists, run_query, RepositoryExecutor,
    },
    Result,
};

/// Policy groups and their ordered explicit members.
///
/// A group row and its member rows are separate statements, so writes belong
/// inside a unit of work where they commit together.
#[derive(Debug, Clone, Copy)]
pub struct PolicyGroupRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(PolicyGroupRepository);

impl<'executor> PolicyGroupRepository<'executor> {
    /// Every group in display order, each with its members in order.
    pub async fn list(&self) -> Result<Vec<PolicyGroupItem>> {
        let rows = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM policy_groups ORDER BY sort, id"),
            fetch_all
        )?;
        let members = run_query!(
            self.executor,
            sqlx::query_as::<_, (String, String)>(
                "SELECT group_id, profile_id FROM policy_group_members ORDER BY group_id, position",
            ),
            fetch_all
        )?;
        let mut groups = rows.iter().map(row_to_group).collect::<Result<Vec<_>>>()?;
        for (group_id, profile_id) in members {
            if let Some(group) = groups.iter_mut().find(|group| group.id == group_id) {
                group.member_ids.push(profile_id);
            }
        }

        Ok(groups)
    }

    pub async fn get(&self, id: &str) -> Result<Option<PolicyGroupItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM policy_groups WHERE id = ?").bind(id),
            fetch_optional
        )?;
        let Some(row) = row else {
            return Ok(None);
        };
        let mut group = row_to_group(&row)?;
        group.member_ids = run_query!(
            self.executor,
            sqlx::query_scalar::<_, String>(
                "SELECT profile_id FROM policy_group_members WHERE group_id = ? ORDER BY position",
            )
            .bind(id),
            fetch_all
        )?;

        Ok(Some(group))
    }

    /// Writes the group row and replaces its member list.
    pub async fn upsert(&self, group: &PolicyGroupItem) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO policy_groups (
                id, name, strategy, source_subscription_id, auto_created,
                selected_profile_id, test_url, interval_seconds, tolerance_ms, sort
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                strategy = excluded.strategy,
                source_subscription_id = excluded.source_subscription_id,
                auto_created = excluded.auto_created,
                selected_profile_id = excluded.selected_profile_id,
                test_url = excluded.test_url,
                interval_seconds = excluded.interval_seconds,
                tolerance_ms = excluded.tolerance_ms,
                sort = excluded.sort
            "#,
            )
            .bind(&group.id)
            .bind(&group.name)
            .bind(group.strategy.as_db_str())
            .bind(&group.source_subscription_id)
            .bind(group.auto_created)
            .bind(&group.selected_profile_id)
            .bind(&group.test_url)
            .bind(group.interval_seconds)
            .bind(group.tolerance_ms)
            .bind(group.sort),
            execute
        )?;
        run_query!(
            self.executor,
            sqlx::query("DELETE FROM policy_group_members WHERE group_id = ?").bind(&group.id),
            execute
        )?;
        for (position, profile_id) in group.member_ids.iter().enumerate() {
            run_query!(
                self.executor,
                sqlx::query(
                    "INSERT INTO policy_group_members (group_id, profile_id, position) VALUES (?, ?, ?)",
                )
                .bind(&group.id)
                .bind(profile_id)
                .bind(i64::try_from(position).unwrap_or(i64::MAX)),
                execute
            )?;
        }

        Ok(())
    }

    pub async fn exists(&self, id: &str) -> Result<bool> {
        row_exists(
            self.executor,
            "SELECT EXISTS(SELECT 1 FROM policy_groups WHERE id = ?)",
            id,
        )
        .await
    }

    pub async fn delete_many(&self, ids: &[String]) -> Result<u64> {
        delete_each(self.executor, "DELETE FROM policy_groups WHERE id = ?", ids).await
    }

    /// Deletes the groups imports created for these subscriptions. Groups the
    /// user built outlive the subscription and only lose their binding.
    pub async fn delete_auto_created_for_subscriptions(
        &self,
        subscription_ids: &[String],
    ) -> Result<u64> {
        delete_each(
            self.executor,
            "DELETE FROM policy_groups WHERE auto_created = 1 AND source_subscription_id = ?",
            subscription_ids,
        )
        .await
    }

    /// The group an import created for this subscription, if one still exists.
    pub async fn auto_created_for_subscription(
        &self,
        subscription_id: &str,
    ) -> Result<Option<String>> {
        Ok(run_query!(
            self.executor,
            sqlx::query_scalar::<_, String>(
                "SELECT id FROM policy_groups WHERE auto_created = 1 AND source_subscription_id = ? ORDER BY sort, id LIMIT 1",
            )
            .bind(subscription_id),
            fetch_optional
        )?)
    }

    pub async fn set_selected_member(&self, id: &str, profile_id: Option<&str>) -> Result<bool> {
        let result = run_query!(
            self.executor,
            sqlx::query("UPDATE policy_groups SET selected_profile_id = ? WHERE id = ?")
                .bind(profile_id)
                .bind(id),
            execute
        )?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn max_sort(&self) -> Result<i32> {
        max_sort(self.executor, "SELECT MAX(sort) FROM policy_groups").await
    }
}

fn row_to_group(row: &SqliteRow) -> Result<PolicyGroupItem> {
    let strategy: String = row.try_get("strategy")?;
    Ok(PolicyGroupItem {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        // The column CHECK admits only the spellings `from_db_str` reads.
        strategy: GroupStrategy::from_db_str(&strategy).unwrap_or_default(),
        source_subscription_id: row.try_get("source_subscription_id")?,
        auto_created: row.try_get("auto_created")?,
        selected_profile_id: row.try_get("selected_profile_id")?,
        test_url: row.try_get("test_url")?,
        interval_seconds: row.try_get("interval_seconds")?,
        tolerance_ms: row.try_get("tolerance_ms")?,
        sort: row.try_get("sort")?,
        member_ids: Vec::new(),
    })
}
