use crate::{
    executor::{repository_constructors, run_query, RepositoryExecutor},
    Result,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppStateRecord {
    pub active_profile_id: Option<String>,
    pub active_routing_id: Option<String>,
    /// Never set together with `active_profile_id`; the table enforces it.
    pub active_group_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct AppStateRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(AppStateRepository);

impl<'executor> AppStateRepository<'executor> {
    pub async fn load(&self) -> Result<AppStateRecord> {
        let (active_profile_id, active_routing_id, active_group_id) = run_query!(
            self.executor,
            sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
                "SELECT active_profile_id, active_routing_id, active_group_id FROM app_state WHERE id = 1",
            ),
            fetch_one
        )?;
        Ok(AppStateRecord {
            active_profile_id,
            active_routing_id,
            active_group_id,
        })
    }

    pub async fn set_active_profile(&self, profile_id: Option<&str>) -> Result<()> {
        run_query!(
            self.executor,
            // Activating a node deactivates the group; clearing it leaves the
            // group alone.
            sqlx::query(
                "UPDATE app_state SET active_profile_id = ?1, active_group_id = CASE WHEN ?1 IS NULL THEN active_group_id ELSE NULL END WHERE id = 1",
            )
            .bind(profile_id),
            execute
        )?;
        Ok(())
    }

    pub async fn set_active_group(&self, group_id: Option<&str>) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query(
                "UPDATE app_state SET active_group_id = ?1, active_profile_id = CASE WHEN ?1 IS NULL THEN active_profile_id ELSE NULL END WHERE id = 1",
            )
            .bind(group_id),
            execute
        )?;
        Ok(())
    }

    pub async fn set_active_routing(&self, routing_id: Option<&str>) -> Result<()> {
        run_query!(
            self.executor,
            sqlx::query("UPDATE app_state SET active_routing_id = ? WHERE id = 1").bind(routing_id),
            execute
        )?;
        Ok(())
    }
}
