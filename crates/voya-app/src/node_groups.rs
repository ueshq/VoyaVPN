//! User-owned folders. Membership never changes runtime configuration.
use std::collections::HashSet;

use thiserror::Error;
use voya_contracts::{MoveAction, NodeGroup, NodeGroupAssignment, NodeGroupsSnapshot};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};

#[derive(Debug, Error)]
pub enum NodeGroupError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Profile(#[from] crate::profiles::ProfileManagerError),
    #[error("group name must not be empty")]
    EmptyName,
    #[error("a group with this name already exists")]
    DuplicateName,
    #[error("node group {0} was not found")]
    GroupNotFound(String),
    #[error("node {0} was not found")]
    ProfileNotFound(String),
    #[error("a node can only be assigned once per operation")]
    DuplicateAssignment,
    #[error("unsupported group move")]
    InvalidMove,
}

type Result<T> = std::result::Result<T, NodeGroupError>;

#[derive(Debug, Clone, Copy)]
pub struct NodeGroupManager<'db> {
    database: DatabaseSession<'db>,
}

impl<'db> NodeGroupManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self {
            database: DatabaseSession::from_database(database),
        }
    }

    #[must_use]
    pub fn new_in(unit: &'db UnitOfWork) -> Self {
        Self {
            database: DatabaseSession::from_unit_of_work(unit),
        }
    }

    pub async fn list(&self) -> Result<NodeGroupsSnapshot> {
        Ok(self.database.node_groups().snapshot().await?)
    }

    pub async fn save(&self, id: Option<&str>, name: &str) -> Result<NodeGroup> {
        let name = name.trim();
        if name.is_empty() {
            return Err(NodeGroupError::EmptyName);
        }
        let snapshot = self.list().await?;
        if snapshot
            .groups
            .iter()
            .any(|g| g.name == name && Some(g.id.as_str()) != id)
        {
            return Err(NodeGroupError::DuplicateName);
        }
        let mut group = if let Some(id) = id {
            snapshot
                .groups
                .into_iter()
                .find(|g| g.id == id)
                .ok_or_else(|| NodeGroupError::GroupNotFound(id.to_string()))?
        } else {
            NodeGroup {
                id: uuid::Uuid::new_v4().to_string(),
                name: String::new(),
                sort: snapshot
                    .groups
                    .iter()
                    .map(|g| g.sort)
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1),
            }
        };
        group.name = name.to_string();
        self.database.node_groups().save(&group).await?;
        Ok(group)
    }

    /// The caller owns one UnitOfWork so name and membership commit together.
    pub async fn update(
        &self,
        id: &str,
        name: &str,
        assignments: &[NodeGroupAssignment],
    ) -> Result<NodeGroup> {
        let group = self.save(Some(id), name).await?;
        self.assign(assignments).await?;
        Ok(group)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        if !self.database.node_groups().delete(id).await? {
            return Err(NodeGroupError::GroupNotFound(id.to_string()));
        }
        Ok(())
    }

    pub async fn move_group(&self, id: &str, action: MoveAction) -> Result<()> {
        let mut groups = self.list().await?.groups;
        let from = groups
            .iter()
            .position(|g| g.id == id)
            .ok_or_else(|| NodeGroupError::GroupNotFound(id.to_string()))?;
        let to = match action {
            MoveAction::Top => 0,
            MoveAction::Up => from.saturating_sub(1),
            MoveAction::Down => (from + 1).min(groups.len() - 1),
            MoveAction::Bottom => groups.len() - 1,
            MoveAction::Position => return Err(NodeGroupError::InvalidMove),
        };
        let moved = groups.remove(from);
        groups.insert(to, moved);
        for (sort, mut group) in groups.into_iter().enumerate() {
            group.sort = i32::try_from(sort).unwrap_or(i32::MAX);
            self.database.node_groups().save(&group).await?;
        }
        Ok(())
    }

    /// Invoked inside one UnitOfWork. Validate the whole delta before writing;
    /// unrelated membership changes are never replaced by a stale dialog.
    pub async fn assign(&self, assignments: &[NodeGroupAssignment]) -> Result<()> {
        crate::profiles::ProfileManager::from_session(self.database)
            .require_manual(
                &assignments
                    .iter()
                    .map(|item| item.profile_id.clone())
                    .collect::<Vec<_>>(),
            )
            .await?;
        let groups = self.list().await?.groups;
        let mut seen = HashSet::new();
        for assignment in assignments {
            if !seen.insert(&assignment.profile_id) {
                return Err(NodeGroupError::DuplicateAssignment);
            }
            if !self
                .database
                .profiles()
                .exists(&assignment.profile_id)
                .await?
            {
                return Err(NodeGroupError::ProfileNotFound(
                    assignment.profile_id.clone(),
                ));
            }
            if let Some(id) = &assignment.group_id {
                if !groups.iter().any(|g| &g.id == id) {
                    return Err(NodeGroupError::GroupNotFound(id.clone()));
                }
            }
        }
        for assignment in assignments {
            self.database
                .node_groups()
                .assign(&assignment.profile_id, assignment.group_id.as_deref())
                .await?;
        }
        Ok(())
    }
}
