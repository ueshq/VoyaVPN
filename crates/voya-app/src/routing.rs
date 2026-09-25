use thiserror::Error;
use voya_contracts::{AppError, MoveAction as ContractMoveAction, Routing, RoutingRule};
use voya_core::{AppConfig, MoveAction, RoutingItem, RulesItem};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};

use crate::config_mutation::{CommittedMutation, ConfigMutationCoordinator};
use crate::contract_map::{
    move_action_from_contract, routing_from_contract, routing_to_contract, rule_from_contract,
};

const DEFAULT_ROUTING_SORT_STEP: i32 = 10;

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
        Self::from_session(DatabaseSession::Database(database))
    }

    #[must_use]
    pub fn new_in(unit_of_work: &'db UnitOfWork) -> Self {
        Self::from_session(DatabaseSession::UnitOfWork(unit_of_work))
    }

    #[must_use]
    const fn from_session(database: DatabaseSession<'db>) -> Self {
        Self { database }
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

    /// Seeds the default routing profile when the database has none, and
    /// activates it. Returns `None` when a profile already existed.
    pub async fn ensure_default_routing(
        &self,
        config: &mut AppConfig,
        language: &str,
    ) -> Result<Option<RoutingItem>> {
        if self.database.routings().first().await?.is_some() {
            return Ok(None);
        }
        let seed = voya_core::default_routing_item(voya_core::seed_routing_remarks(language));
        self.save_routing(config, seed).await.map(Some)
    }

    /// Replaces a profile's rules with the default set. The per-app proxy rule
    /// is user data rather than part of the default, so it stays first.
    pub async fn reset_rules_to_default(&self, routing_id: &str) -> Result<RoutingItem> {
        let mut routing = self.load_routing(routing_id).await?;
        let per_app = routing
            .rule_set
            .iter()
            .find(|rule| rule.remarks.as_deref() == Some(voya_core::SENTINEL_PER_APP_PROXY))
            .cloned();
        routing.rule_set = per_app
            .into_iter()
            .chain(voya_core::default_rule_set())
            .collect();
        normalize_routing_item(&mut routing);
        self.database.routings().upsert(&routing).await?;

        Ok(routing)
    }

    /// Unions each managed rule's matchers with the current seed at startup.
    ///
    /// Profile structure, switches, outbounds and deletions stay as the user
    /// left them; only the seed's `domain`/`ip` entries missing from an
    /// existing managed rule are appended. Returns how many profiles changed.
    pub async fn refresh_managed_rules(&self) -> Result<u32> {
        let mut updated = 0;
        for mut routing in self.database.routings().list().await? {
            let mut changed = false;
            for rule in &mut routing.rule_set {
                changed |= voya_core::refresh_managed_rule(rule);
            }
            if !changed {
                continue;
            }
            normalize_routing_item(&mut routing);
            self.database.routings().upsert(&routing).await?;
            updated += 1;
        }
        Ok(updated)
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

    async fn ensure_active_routing(&self, config: &mut AppConfig) -> Result<Option<RoutingItem>> {
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
    format!("routing-{}", uuid::Uuid::new_v4().simple())
}

fn generate_rule_id() -> String {
    format!("rule-{}", uuid::Uuid::new_v4().simple())
}

// The contract-typed routing use cases both hosts share.
//
// Each mutation is one `mutate` → `RoutingManager::new_in` → contract map
// body returning the committed result, so a host keeps only its own input
// validation and post-commit tail.

/// Every routing profile in list order, as the public DTO.
pub async fn list_routings_use_case(
    services: &crate::services::AppServices,
) -> std::result::Result<Vec<Routing>, AppError> {
    Ok(services
        .list_routings()
        .await?
        .into_iter()
        .map(routing_to_contract)
        .collect())
}

/// Saves a routing profile; an empty id creates one.
pub async fn save_routing_use_case(
    mutations: &ConfigMutationCoordinator,
    item: Routing,
) -> std::result::Result<CommittedMutation<Routing>, AppError> {
    mutations
        .mutate(
            async |unit_of_work, config| -> std::result::Result<Routing, AppError> {
                Ok(routing_to_contract(
                    RoutingManager::new_in(unit_of_work)
                        .save_routing(config, routing_from_contract(item))
                        .await?,
                ))
            },
        )
        .await
}

/// Deletes routing profiles by id and keeps the active one valid.
pub async fn delete_routings_use_case(
    mutations: &ConfigMutationCoordinator,
    ids: Vec<String>,
) -> std::result::Result<CommittedMutation<u32>, AppError> {
    mutations
        .mutate(
            async |unit_of_work, config| -> std::result::Result<u32, AppError> {
                Ok(RoutingManager::new_in(unit_of_work)
                    .delete_routings(config, &ids)
                    .await?)
            },
        )
        .await
}

/// Makes a routing profile the active one.
pub async fn set_active_routing_use_case(
    mutations: &ConfigMutationCoordinator,
    id: String,
) -> std::result::Result<CommittedMutation<Routing>, AppError> {
    mutations
        .mutate(
            async |unit_of_work, config| -> std::result::Result<Routing, AppError> {
                Ok(routing_to_contract(
                    RoutingManager::new_in(unit_of_work)
                        .set_active_routing(config, &id)
                        .await?,
                ))
            },
        )
        .await
}

/// Saves one rule inside a routing profile.
pub async fn save_routing_rule_use_case(
    mutations: &ConfigMutationCoordinator,
    routing_id: String,
    rule: RoutingRule,
) -> std::result::Result<CommittedMutation<Routing>, AppError> {
    mutations
        .mutate(
            async |unit_of_work, _config| -> std::result::Result<Routing, AppError> {
                Ok(routing_to_contract(
                    RoutingManager::new_in(unit_of_work)
                        .save_rule(&routing_id, rule_from_contract(rule))
                        .await?,
                ))
            },
        )
        .await
}

/// Deletes rules from a routing profile.
pub async fn delete_routing_rules_use_case(
    mutations: &ConfigMutationCoordinator,
    routing_id: String,
    rule_ids: Vec<String>,
) -> std::result::Result<CommittedMutation<Routing>, AppError> {
    mutations
        .mutate(
            async |unit_of_work, _config| -> std::result::Result<Routing, AppError> {
                Ok(routing_to_contract(
                    RoutingManager::new_in(unit_of_work)
                        .delete_rules(&routing_id, &rule_ids)
                        .await?,
                ))
            },
        )
        .await
}

/// Moves one rule inside a routing profile.
pub async fn move_routing_rule_use_case(
    mutations: &ConfigMutationCoordinator,
    routing_id: String,
    rule_id: String,
    action: ContractMoveAction,
    position: Option<i32>,
) -> std::result::Result<CommittedMutation<Routing>, AppError> {
    mutations
        .mutate(
            async |unit_of_work, _config| -> std::result::Result<Routing, AppError> {
                Ok(routing_to_contract(
                    RoutingManager::new_in(unit_of_work)
                        .move_rule(
                            &routing_id,
                            &rule_id,
                            move_action_from_contract(action),
                            position,
                        )
                        .await?,
                ))
            },
        )
        .await
}

/// Replaces a routing profile's rules with the default set, keeping its
/// per-app proxy rule.
pub async fn reset_routing_rules_use_case(
    mutations: &ConfigMutationCoordinator,
    routing_id: String,
) -> std::result::Result<CommittedMutation<Routing>, AppError> {
    mutations
        .mutate(
            async |unit_of_work, _config| -> std::result::Result<Routing, AppError> {
                Ok(routing_to_contract(
                    RoutingManager::new_in(unit_of_work)
                        .reset_rules_to_default(&routing_id)
                        .await?,
                ))
            },
        )
        .await
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

    #[tokio::test]
    async fn a_fresh_database_is_seeded_once_with_an_active_default_routing() {
        let database = Database::connect_in_memory()
            .await
            .expect("routing manager test operation should succeed");
        let manager = RoutingManager::new(&database);
        let mut config = AppConfig::default();

        let seeded = manager
            .ensure_default_routing(&mut config, "zh-Hans")
            .await
            .expect("seed")
            .expect("a fresh database is seeded");
        assert_eq!(seeded.remarks, "智能分流");
        assert!(seeded.is_active);
        assert_eq!(config.routing_basic_item.routing_index_id, seeded.id);
        assert_eq!(seeded.rule_set.len(), voya_core::default_rule_set().len());
        assert!(seeded.rule_set.iter().all(|rule| !rule.id.is_empty()));

        assert!(manager
            .ensure_default_routing(&mut config, "en")
            .await
            .expect("second call")
            .is_none());
        assert_eq!(database.routings().list().await.expect("routings").len(), 1);
    }

    #[tokio::test]
    async fn an_existing_routing_profile_is_never_replaced_by_the_seed() {
        let database = Database::connect_in_memory()
            .await
            .expect("routing manager test operation should succeed");
        let manager = RoutingManager::new(&database);
        let mut config = AppConfig::default();
        manager
            .save_routing(
                &mut config,
                RoutingItem {
                    remarks: "Mine".to_string(),
                    ..RoutingItem::default()
                },
            )
            .await
            .expect("custom routing");

        assert!(manager
            .ensure_default_routing(&mut config, "en")
            .await
            .expect("seed check")
            .is_none());
        let routings = database.routings().list().await.expect("routings");
        assert_eq!(routings.len(), 1);
        assert_eq!(routings[0].remarks, "Mine");
    }

    #[tokio::test]
    async fn startup_refresh_appends_new_ai_domains_without_touching_the_rest() {
        let database = Database::connect_in_memory()
            .await
            .expect("routing manager test operation should succeed");
        let manager = RoutingManager::new(&database);
        let mut config = AppConfig::default();
        // An old AI list without claude.com, plus an unrelated user rule and a
        // disabled managed rule — order, switches and user matchers must hold.
        let old_ai = RulesItem {
            id: "rule-old-ai".to_string(),
            remarks: Some(voya_core::SENTINEL_AI_SERVICES.to_string()),
            outbound_tag: Some(PROXY_TAG.to_string()),
            rule_type: Some(RuleType::ALL),
            enabled: true,
            domain: Some(vec![
                "domain:anthropic.com".to_string(),
                "domain:my-extra.example".to_string(),
            ]),
            ..RulesItem::default()
        };
        let custom = RulesItem {
            id: "rule-custom".to_string(),
            remarks: Some("Mine".to_string()),
            outbound_tag: Some(DIRECT_TAG.to_string()),
            domain: Some(vec!["full:custom.example".to_string()]),
            ..RulesItem::default()
        };
        let disabled_ads = RulesItem {
            id: "rule-ads".to_string(),
            remarks: Some(voya_core::SENTINEL_BLOCK_ADS.to_string()),
            outbound_tag: Some(BLOCK_TAG.to_string()),
            rule_type: Some(RuleType::ALL),
            enabled: false,
            domain: Some(vec!["geosite:category-ads-all".to_string()]),
            ..RulesItem::default()
        };
        let saved = manager
            .save_routing(
                &mut config,
                RoutingItem {
                    remarks: "Mine".to_string(),
                    rule_set: vec![old_ai, custom, disabled_ads],
                    ..RoutingItem::default()
                },
            )
            .await
            .expect("custom routing");

        let updated = manager
            .refresh_managed_rules()
            .await
            .expect("refresh managed rules");
        assert_eq!(updated, 1);

        let routing = database
            .routings()
            .get(&saved.id)
            .await
            .expect("load routing")
            .expect("routing still exists");
        assert_eq!(routing.rule_set.len(), 3);
        assert_eq!(routing.rule_set[0].id, "rule-old-ai");
        assert_eq!(routing.rule_set[1].id, "rule-custom");
        assert_eq!(routing.rule_set[2].id, "rule-ads");
        assert!(!routing.rule_set[2].enabled);

        let ai_domains = routing.rule_set[0].domain.as_ref().expect("ai domains");
        assert!(ai_domains.contains(&"domain:claude.com".to_string()));
        assert!(ai_domains.contains(&"domain:my-extra.example".to_string()));
        assert_eq!(ai_domains[0], "domain:anthropic.com");
        assert_eq!(
            routing.rule_set[1].domain.as_ref(),
            Some(&vec!["full:custom.example".to_string()])
        );
        assert_eq!(
            routing.rule_set[2].domain.as_ref(),
            Some(&vec!["geosite:category-ads-all".to_string()])
        );

        assert_eq!(
            manager
                .refresh_managed_rules()
                .await
                .expect("second refresh"),
            0
        );
    }

    #[tokio::test]
    async fn resetting_rules_restores_the_default_set_and_keeps_the_per_app_rule_first() {
        let database = Database::connect_in_memory()
            .await
            .expect("routing manager test operation should succeed");
        let manager = RoutingManager::new(&database);
        let mut config = AppConfig::default();
        let seeded = manager
            .ensure_default_routing(&mut config, "en")
            .await
            .expect("seed")
            .expect("seeded");
        manager
            .save_rule(
                &seeded.id,
                RulesItem {
                    remarks: Some("Custom".to_string()),
                    outbound_tag: Some(BLOCK_TAG.to_string()),
                    domain: Some(vec!["full:custom.example".to_string()]),
                    ..RulesItem::default()
                },
            )
            .await
            .expect("custom rule");
        let with_per_app = manager
            .save_rule(
                &seeded.id,
                RulesItem {
                    remarks: Some(voya_core::SENTINEL_PER_APP_PROXY.to_string()),
                    outbound_tag: Some(PROXY_TAG.to_string()),
                    process: Some(vec!["curl".to_string()]),
                    ..RulesItem::default()
                },
            )
            .await
            .expect("per-app rule");
        let bypass_lan = with_per_app
            .rule_set
            .iter()
            .find(|rule| rule.remarks.as_deref() == Some(voya_core::SENTINEL_BYPASS_LAN))
            .expect("seeded LAN rule")
            .id
            .clone();
        manager
            .delete_rules(&seeded.id, &[bypass_lan])
            .await
            .expect("delete LAN rule");

        let reset = manager
            .reset_rules_to_default(&seeded.id)
            .await
            .expect("reset");
        let remarks = reset
            .rule_set
            .iter()
            .map(|rule| rule.remarks.clone().unwrap_or_default())
            .collect::<Vec<_>>();
        let mut expected = vec![voya_core::SENTINEL_PER_APP_PROXY.to_string()];
        expected.extend(
            voya_core::default_rule_set()
                .into_iter()
                .map(|rule| rule.remarks.unwrap_or_default()),
        );
        assert_eq!(remarks, expected);
        assert_eq!(reset.rule_set[0].process, Some(vec!["curl".to_string()]));
    }
}
