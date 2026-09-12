use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;
use voya_core::{AppConfig, MoveAction, RoutingItem, RulesItem};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};

const DEFAULT_ROUTING_SORT_STEP: i32 = 10;

static ROUTING_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
static ROUTING_RULE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub type Result<T> = std::result::Result<T, RoutingManagerError>;

#[derive(Debug, Error)]
pub enum RoutingManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("routing profile {0} was not found")]
    RoutingNotFound(String),
    #[error("routing profile id is required")]
    MissingRoutingId,
    #[error("routing rule {rule_id} was not found in {routing_id}")]
    RuleNotFound { routing_id: String, rule_id: String },
    #[error("cannot move routing rule {rule_id}: {reason}")]
    InvalidMove { rule_id: String, reason: String },
}

#[derive(Debug, Clone, Copy)]
pub struct RoutingManager<'db> {
    database: DatabaseSession<'db>,
}

impl<'db> RoutingManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self::from_session(DatabaseSession::from_database(database))
    }

    #[must_use]
    pub fn new_in(unit_of_work: &'db UnitOfWork) -> Self {
        Self::from_session(DatabaseSession::from_unit_of_work(unit_of_work))
    }

    #[must_use]
    const fn from_session(database: DatabaseSession<'db>) -> Self {
        Self { database }
    }

    pub async fn list_routings(&self) -> Result<Vec<RoutingItem>> {
        Ok(self.database.routings().list().await?)
    }

    pub async fn save_routing(
        &self,
        config: &mut AppConfig,
        mut item: RoutingItem,
    ) -> Result<RoutingItem> {
        let active_id = self.database.routings().active().await?.map(|item| item.id);
        let is_new = if item.id.trim().is_empty() {
            item.id = generate_routing_id();
            true
        } else {
            !self.database.routings().exists(&item.id).await?
        };
        let should_activate = active_id.as_deref() == Some(item.id.as_str()) || active_id.is_none();

        normalize_routing_item(&mut item);
        if is_new && item.sort <= 0 {
            item.sort = self.database.routings().max_sort().await? + DEFAULT_ROUTING_SORT_STEP;
        }
        self.database.routings().upsert(&item).await?;
        if should_activate {
            self.database.routings().set_active(&item.id).await?;
            config
                .routing_basic_item
                .routing_index_id
                .clone_from(&item.id);
        }
        self.ensure_active_routing(config).await?;

        self.database
            .routings()
            .get(&item.id)
            .await?
            .ok_or_else(|| RoutingManagerError::RoutingNotFound(item.id))
    }

    pub async fn delete_routings(&self, config: &mut AppConfig, ids: &[String]) -> Result<u32> {
        let deleted = self.database.routings().delete_many(ids).await?;
        self.ensure_active_routing(config).await?;

        Ok(u32::try_from(deleted).unwrap_or(u32::MAX))
    }

    pub async fn set_active_routing(
        &self,
        config: &mut AppConfig,
        id: &str,
    ) -> Result<RoutingItem> {
        if id.trim().is_empty() {
            return Err(RoutingManagerError::MissingRoutingId);
        }
        if !self.database.routings().set_active(id).await? {
            return Err(RoutingManagerError::RoutingNotFound(id.to_string()));
        }
        config.routing_basic_item.routing_index_id = id.to_string();

        self.database
            .routings()
            .get(id)
            .await?
            .ok_or_else(|| RoutingManagerError::RoutingNotFound(id.to_string()))
    }

    pub async fn save_rule(&self, routing_id: &str, mut rule: RulesItem) -> Result<RoutingItem> {
        let mut routing = self.load_routing(routing_id).await?;
        normalize_rule(&mut rule);

        if let Some(existing) = routing
            .rule_set
            .iter_mut()
            .find(|candidate| candidate.id == rule.id)
        {
            *existing = rule;
        } else {
            routing.rule_set.push(rule);
        }

        normalize_routing_item(&mut routing);
        self.database.routings().upsert(&routing).await?;

        Ok(routing)
    }

    pub async fn delete_rules(&self, routing_id: &str, rule_ids: &[String]) -> Result<RoutingItem> {
        let mut routing = self.load_routing(routing_id).await?;
        let before = routing.rule_set.len();
        routing
            .rule_set
            .retain(|rule| !rule_ids.iter().any(|id| id == &rule.id));
        if before == routing.rule_set.len() && !rule_ids.is_empty() {
            return Err(RoutingManagerError::RuleNotFound {
                routing_id: routing_id.to_string(),
                rule_id: rule_ids[0].clone(),
            });
        }

        normalize_routing_item(&mut routing);
        self.database.routings().upsert(&routing).await?;

        Ok(routing)
    }

    pub async fn move_rule(
        &self,
        routing_id: &str,
        rule_id: &str,
        action: MoveAction,
        position: Option<i32>,
    ) -> Result<RoutingItem> {
        let mut routing = self.load_routing(routing_id).await?;
        let Some(index) = routing.rule_set.iter().position(|rule| rule.id == rule_id) else {
            return Err(RoutingManagerError::RuleNotFound {
                routing_id: routing_id.to_string(),
                rule_id: rule_id.to_string(),
            });
        };

        let next_index =
            moved_index(index, routing.rule_set.len(), action, position).map_err(|reason| {
                RoutingManagerError::InvalidMove {
                    rule_id: rule_id.to_string(),
                    reason,
                }
            })?;
        if next_index != index {
            let rule = routing.rule_set.remove(index);
            let adjusted = if next_index > index {
                next_index.saturating_sub(1)
            } else {
                next_index
            };
            routing.rule_set.insert(adjusted, rule);
        }

        normalize_routing_item(&mut routing);
        self.database.routings().upsert(&routing).await?;

        Ok(routing)
    }

    pub async fn ensure_active_routing(
        &self,
        config: &mut AppConfig,
    ) -> Result<Option<RoutingItem>> {
        if let Some(active) = self.database.routings().active().await? {
            config
                .routing_basic_item
                .routing_index_id
                .clone_from(&active.id);
            return Ok(Some(active));
        }

        let configured = config.routing_basic_item.routing_index_id.trim();
        if !configured.is_empty() {
            if let Some(item) = self.database.routings().get(configured).await? {
                self.database.routings().set_active(&item.id).await?;
                config
                    .routing_basic_item
                    .routing_index_id
                    .clone_from(&item.id);
                return Ok(Some(item));
            }
        }

        if let Some(first) = self.database.routings().first().await? {
            self.database.routings().set_active(&first.id).await?;
            config
                .routing_basic_item
                .routing_index_id
                .clone_from(&first.id);
            return Ok(Some(first));
        }

        config.routing_basic_item.routing_index_id.clear();
        Ok(None)
    }

    async fn load_routing(&self, routing_id: &str) -> Result<RoutingItem> {
        if routing_id.trim().is_empty() {
            return Err(RoutingManagerError::MissingRoutingId);
        }

        self.database
            .routings()
            .get(routing_id)
            .await?
            .ok_or_else(|| RoutingManagerError::RoutingNotFound(routing_id.to_string()))
    }
}

fn normalize_routing_item(item: &mut RoutingItem) {
    if item.id.trim().is_empty() {
        item.id = generate_routing_id();
    }
    if item.remarks.trim().is_empty() {
        item.remarks = "Routing".to_string();
    }
    for rule in &mut item.rule_set {
        normalize_rule(rule);
    }
}

fn normalize_rule(rule: &mut RulesItem) {
    if rule.id.trim().is_empty() {
        rule.id = generate_rule_id();
    }
}

fn moved_index(
    index: usize,
    count: usize,
    action: MoveAction,
    position: Option<i32>,
) -> std::result::Result<usize, String> {
    match action {
        MoveAction::Top => Ok(0),
        MoveAction::Up => Ok(index.saturating_sub(1)),
        MoveAction::Down => Ok((index + 2).min(count)),
        MoveAction::Bottom => Ok(count),
        MoveAction::Position => {
            let position = position.unwrap_or(0);
            if position < 0 {
                return Err("position must be non-negative".to_string());
            }
            Ok(usize::try_from(position).unwrap_or(usize::MAX).min(count))
        }
    }
}

fn generate_routing_id() -> String {
    generate_id("routing", &ROUTING_ID_COUNTER)
}

fn generate_rule_id() -> String {
    generate_id("rule", &ROUTING_RULE_ID_COUNTER)
}

fn generate_id(prefix: &str, counter: &AtomicU64) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        });
    let sequence = counter.fetch_add(1, Ordering::Relaxed);

    format!("{prefix}-{millis}-{sequence}")
}

#[cfg(test)]
mod tests {
    use voya_core::{AppConfig, RuleType, BLOCK_TAG, DIRECT_TAG, PROXY_TAG};
    use voya_db::Database;

    use super::*;

    #[tokio::test]
    async fn routing_manager_selects_active_and_moves_rules() {
        let database = Database::connect_in_memory()
            .await
            .expect("routing manager test operation should succeed");
        let manager = RoutingManager::new(&database);
        let mut config = AppConfig::default();

        let first = manager
            .save_routing(
                &mut config,
                RoutingItem {
                    remarks: "First".to_string(),
                    rule_set: vec![RulesItem {
                        remarks: Some("A".to_string()),
                        outbound_tag: Some(DIRECT_TAG.to_string()),
                        domain: Some(vec!["full:a.example.com".to_string()]),
                        rule_type: Some(RuleType::Routing),
                        ..RulesItem::default()
                    }],
                    ..RoutingItem::default()
                },
            )
            .await
            .expect("routing manager test operation should succeed");
        let second = manager
            .save_routing(
                &mut config,
                RoutingItem {
                    remarks: "Second".to_string(),
                    rule_set: vec![RulesItem {
                        remarks: Some("B".to_string()),
                        outbound_tag: Some(PROXY_TAG.to_string()),
                        domain: Some(vec!["full:b.example.com".to_string()]),
                        rule_type: Some(RuleType::Routing),
                        ..RulesItem::default()
                    }],
                    ..RoutingItem::default()
                },
            )
            .await
            .expect("routing manager test operation should succeed");

        assert_eq!(config.routing_basic_item.routing_index_id, first.id);
        assert_eq!(
            manager
                .set_active_routing(&mut config, &second.id)
                .await
                .expect("routing manager test operation should succeed")
                .id,
            second.id
        );
        assert_eq!(config.routing_basic_item.routing_index_id, second.id);

        let added = manager
            .save_rule(
                &second.id,
                RulesItem {
                    remarks: Some("C".to_string()),
                    outbound_tag: Some(BLOCK_TAG.to_string()),
                    domain: Some(vec!["full:c.example.com".to_string()]),
                    ..RulesItem::default()
                },
            )
            .await
            .expect("routing manager test operation should succeed");
        assert_eq!(added.rule_set.len(), 2);
        let moved = manager
            .move_rule(&second.id, &added.rule_set[1].id, MoveAction::Top, None)
            .await
            .expect("routing manager test operation should succeed");
        assert_eq!(moved.rule_set[0].remarks.as_deref(), Some("C"));
    }
}
