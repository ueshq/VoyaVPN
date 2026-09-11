use voya_contracts::{NodeGroup, NodeGroupMembership, NodeGroupsSnapshot};

use crate::{
    executor::{repository_constructors, run_query, RepositoryExecutor},
    Result,
};

#[derive(Debug, Clone, Copy)]
pub struct NodeGroupRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(NodeGroupRepository);

impl NodeGroupRepository<'_> {
    pub async fn snapshot(&self) -> Result<NodeGroupsSnapshot> {
        let groups = run_query!(
            self.executor,
            sqlx::query_as::<_, (String, String, i32)>(
                "SELECT id, name, sort FROM node_groups ORDER BY sort, id"
            ),
            fetch_all
        )?;
        let memberships = run_query!(
            self.executor,
            sqlx::query_as::<_, (String, String)>(
                "SELECT profile_id, group_id FROM node_group_memberships ORDER BY profile_id"
            ),
            fetch_all
        )?;
        Ok(NodeGroupsSnapshot {
            groups: groups
                .into_iter()
                .map(|(id, name, sort)| NodeGroup { id, name, sort })
                .collect(),
            memberships: memberships
                .into_iter()
                .map(|(profile_id, group_id)| NodeGroupMembership {
                    profile_id,
                    group_id,
                })
                .collect(),
        })
    }

    pub async fn save(&self, group: &NodeGroup) -> Result<()> {
        run_query!(self.executor, sqlx::query("INSERT INTO node_groups (id, name, sort) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, sort = excluded.sort")
            .bind(&group.id).bind(&group.name).bind(group.sort), execute)?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        Ok(run_query!(
            self.executor,
            sqlx::query("DELETE FROM node_groups WHERE id = ?").bind(id),
            execute
        )?
        .rows_affected()
            > 0)
    }

    pub async fn group_for_profile(&self, id: &str) -> Result<Option<String>> {
        Ok(run_query!(
            self.executor,
            sqlx::query_scalar("SELECT group_id FROM node_group_memberships WHERE profile_id = ?")
                .bind(id),
            fetch_optional
        )?)
    }

    /// Batch callers join one UnitOfWork so every assignment commits together.
    pub async fn assign(&self, profile_id: &str, group_id: Option<&str>) -> Result<()> {
        if let Some(group_id) = group_id {
            run_query!(self.executor, sqlx::query("INSERT INTO node_group_memberships (profile_id, group_id) VALUES (?, ?) ON CONFLICT(profile_id) DO UPDATE SET group_id = excluded.group_id")
                .bind(profile_id).bind(group_id), execute)?;
        } else {
            run_query!(
                self.executor,
                sqlx::query("DELETE FROM node_group_memberships WHERE profile_id = ?")
                    .bind(profile_id),
                execute
            )?;
        }
        Ok(())
    }
}
