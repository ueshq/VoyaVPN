use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use thiserror::Error;
use voya_contracts::Routing as RoutingContract;
use voya_core::{
    AppConfig, MoveAction, RoutingItem, RuleType, RulesItem, BLOCK_TAG, DEFAULT_DOMAIN_STRATEGY,
    DIRECT_TAG, PROXY_TAG,
};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};
use voya_net::{DownloadClient, DownloadError, DownloadRequest, DEFAULT_TEXT_RESPONSE_LIMIT_BYTES};

const DEFAULT_ROUTING_SORT_STEP: i32 = 10;
const BUILTIN_ROUTING_VERSION: &str = "V4-";

static ROUTING_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
static ROUTING_RULE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub type Result<T> = std::result::Result<T, RoutingManagerError>;

#[derive(Debug, Error)]
pub enum RoutingManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("routing profile {0} was not found")]
    RoutingNotFound(String),
    #[error("routing profile id is required")]
    MissingRoutingId,
    #[error("routing rule {rule_id} was not found in {routing_id}")]
    RuleNotFound { routing_id: String, rule_id: String },
    #[error("invalid routing template: {0}")]
    InvalidTemplate(String),
    #[error("invalid routing rules: {0}")]
    InvalidRules(String),
    #[error("cannot move routing rule {rule_id}: {reason}")]
    InvalidMove { rule_id: String, reason: String },
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedRoutingTemplate {
    version_prefix: Option<String>,
    items: Vec<RoutingItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoutingTemplateApplyResult {
    pub routing_ids: Vec<String>,
    pub active_routing_id: Option<String>,
    pub reused_existing_routing: bool,
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
    pub(crate) const fn from_session(database: DatabaseSession<'db>) -> Self {
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

    /// Download and fully validate an external configuration-template routing source.
    ///
    /// This intentionally performs no database or configuration writes. The Voya
    /// bundle is self-contained and child rules-file URLs are never followed.
    pub(crate) async fn prepare_external_config_template(
        &self,
        source_url: &str,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
    ) -> Result<PreparedRoutingTemplate> {
        let source_url = source_url.trim();
        if source_url.is_empty() {
            return Err(RoutingManagerError::InvalidTemplate(
                "routing template source URL is required".to_string(),
            ));
        }
        voya_net::validate_absolute_https_url(source_url)
            .map_err(|error| RoutingManagerError::InvalidTemplate(error.to_string()))?;

        let download = DownloadClient::new();
        let response = download
            .download_text(DownloadRequest {
                url: source_url.to_string(),
                user_agent: None,
                prefer_proxy,
                proxy_url: proxy_url.map(ToOwned::to_owned),
                response_body_limit: Some(DEFAULT_TEXT_RESPONSE_LIMIT_BYTES),
            })
            .await?;
        let template = parse_routing_template(&response.body)?;
        let version_prefix: Option<String> = None;
        let mut items = Vec::with_capacity(template.routings.len());

        for (index, mut item) in template.routings.into_iter().enumerate() {
            if !item.url.trim().is_empty() {
                return Err(RoutingManagerError::InvalidTemplate(format!(
                    "routing item {index} must be self-contained; child URLs are not supported"
                )));
            }
            if item.rule_set.is_empty() {
                return Err(RoutingManagerError::InvalidRules(format!(
                    "routing item {index} contains no rules"
                )));
            }

            item.id.clear();
            item.url.clear();
            item.enabled = true;
            normalize_routing_item(&mut item);
            items.push(item);
        }

        Ok(PreparedRoutingTemplate {
            version_prefix,
            items,
        })
    }

    #[must_use]
    pub(crate) fn prepare_builtin_config_template(&self) -> PreparedRoutingTemplate {
        PreparedRoutingTemplate {
            version_prefix: Some(BUILTIN_ROUTING_VERSION.to_string()),
            items: builtin_routing_items(),
        }
    }

    /// Persist a routing template that has already passed all network and parse checks.
    pub(crate) async fn apply_prepared_config_template(
        &self,
        config: &mut AppConfig,
        mut prepared: PreparedRoutingTemplate,
    ) -> Result<RoutingTemplateApplyResult> {
        let existing = self.database.routings().list().await?;
        if let Some(prefix) = prepared.version_prefix.as_deref() {
            if let Some(existing_item) = existing
                .iter()
                .find(|item| item.remarks.starts_with(prefix))
                .cloned()
            {
                let mut active = existing_item;
                if !active.enabled {
                    active.enabled = true;
                    self.database.routings().upsert(&active).await?;
                }
                self.database.routings().set_active(&active.id).await?;
                config
                    .routing_basic_item
                    .routing_index_id
                    .clone_from(&active.id);

                return Ok(RoutingTemplateApplyResult {
                    routing_ids: vec![active.id.clone()],
                    active_routing_id: Some(active.id),
                    reused_existing_routing: true,
                });
            }
        }

        if prepared.items.is_empty() {
            return Err(RoutingManagerError::InvalidTemplate(
                "template contains no importable routing items".to_string(),
            ));
        }

        let mut max_sort = self.database.routings().max_sort().await?;
        let mut routing_ids = Vec::with_capacity(prepared.items.len());
        let mut active_routing_id = None;
        let mut claimed_ids = BTreeSet::new();
        let mut inserted = 0_usize;
        for (index, item) in prepared.items.iter_mut().enumerate() {
            // "Import template" is a repeatable action, so re-applying the same
            // template must refresh the routings it produced last time instead
            // of appending another identical set. A self-contained bundle
            // carries no id, so `remarks` is the only stable identity it has;
            // each existing routing is claimed at most once so a template with
            // repeated remarks still yields one row per item.
            let previous = existing
                .iter()
                .find(|candidate| {
                    candidate.remarks == item.remarks && !claimed_ids.contains(&candidate.id)
                })
                .map(|candidate| (candidate.id.clone(), candidate.sort));
            if let Some((previous_id, previous_sort)) = previous {
                claimed_ids.insert(previous_id.clone());
                item.id = previous_id;
                item.sort = previous_sort;
            } else {
                inserted += 1;
                max_sort += DEFAULT_ROUTING_SORT_STEP;
                item.sort = max_sort;
            }
            normalize_routing_item(item);
            self.database.routings().upsert(item).await?;
            if index == 0 {
                active_routing_id = Some(item.id.clone());
            }
            routing_ids.push(item.id.clone());
        }

        if let Some(active_id) = active_routing_id.as_deref() {
            self.database.routings().set_active(active_id).await?;
            config.routing_basic_item.routing_index_id = active_id.to_string();
        }

        Ok(RoutingTemplateApplyResult {
            routing_ids,
            active_routing_id,
            reused_existing_routing: inserted == 0,
        })
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

struct RoutingTemplate {
    routings: Vec<RoutingItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VoyaRoutingBundleV1 {
    schema_version: u32,
    routings: Vec<RoutingContract>,
}

fn parse_routing_template(value: &str) -> Result<RoutingTemplate> {
    let raw = serde_json::from_str::<serde_json::Value>(value)
        .map_err(|error| RoutingManagerError::InvalidTemplate(error.to_string()))?;
    if raw
        .get("routings")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|routings| {
            routings.iter().any(|routing| {
                routing.get("isActive").is_some() || routing.get("ruleNum").is_some()
            })
        })
    {
        return Err(RoutingManagerError::InvalidTemplate(
            "routing bundle contains persisted or derived state".to_string(),
        ));
    }
    let bundle = serde_json::from_str::<VoyaRoutingBundleV1>(value)
        .map_err(|error| RoutingManagerError::InvalidTemplate(error.to_string()))?;
    if bundle.schema_version != 1 {
        return Err(RoutingManagerError::InvalidTemplate(format!(
            "unsupported schemaVersion {}; expected 1",
            bundle.schema_version
        )));
    }
    if bundle.routings.is_empty() {
        return Err(RoutingManagerError::InvalidTemplate(
            "template contains no routing items".to_string(),
        ));
    }
    for (index, routing) in bundle.routings.iter().enumerate() {
        if !routing.source_url.trim().is_empty() {
            return Err(RoutingManagerError::InvalidTemplate(format!(
                "routing item {index} must be self-contained; child URLs are not supported"
            )));
        }
        if routing.rules.is_empty() {
            return Err(RoutingManagerError::InvalidTemplate(format!(
                "routing item {index} contains no rules"
            )));
        }
    }

    Ok(RoutingTemplate {
        routings: bundle
            .routings
            .into_iter()
            .map(crate::contract_map::routing_from_contract)
            .collect(),
    })
}

fn normalize_routing_item(item: &mut RoutingItem) {
    if item.id.trim().is_empty() {
        item.id = generate_routing_id();
    }
    if item.remarks.trim().is_empty() {
        item.remarks = "Routing".to_string();
    }
    if item.domain_strategy.trim().is_empty() {
        item.domain_strategy = DEFAULT_DOMAIN_STRATEGY.to_string();
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

fn builtin_routing_items() -> Vec<RoutingItem> {
    vec![
        RoutingItem {
            remarks: format!("{BUILTIN_ROUTING_VERSION}Bypass mainland (Whitelist)"),
            rule_set: vec![
                rule(
                    "Block udp/443",
                    BLOCK_TAG,
                    None,
                    None,
                    Some("443"),
                    Some("udp"),
                ),
                rule(
                    "Proxy Google",
                    PROXY_TAG,
                    Some(vec!["geosite:google"]),
                    None,
                    None,
                    None,
                ),
                rule(
                    "Bypass private domains",
                    DIRECT_TAG,
                    Some(vec!["geosite:private"]),
                    None,
                    None,
                    None,
                ),
                rule(
                    "Bypass private IPs",
                    DIRECT_TAG,
                    None,
                    Some(vec!["geoip:private"]),
                    None,
                    None,
                ),
                rule(
                    "Bypass CN domains",
                    DIRECT_TAG,
                    Some(vec!["geosite:cn"]),
                    None,
                    None,
                    None,
                ),
                rule(
                    "Bypass CN IPs",
                    DIRECT_TAG,
                    None,
                    Some(vec!["geoip:cn"]),
                    None,
                    None,
                ),
            ],
            ..RoutingItem::default()
        },
        RoutingItem {
            remarks: format!("{BUILTIN_ROUTING_VERSION}Blacklist"),
            rule_set: vec![
                rule("Bypass bittorrent", DIRECT_TAG, None, None, None, None)
                    .with_protocol(vec!["bittorrent"]),
                rule(
                    "Block udp/443",
                    BLOCK_TAG,
                    None,
                    None,
                    Some("443"),
                    Some("udp"),
                ),
                rule(
                    "Proxy GFW",
                    PROXY_TAG,
                    Some(vec!["geosite:gfw", "geosite:greatfire"]),
                    None,
                    None,
                    None,
                ),
                rule(
                    "Final direct",
                    DIRECT_TAG,
                    None,
                    None,
                    Some("0-65535"),
                    None,
                ),
            ],
            ..RoutingItem::default()
        },
        RoutingItem {
            remarks: format!("{BUILTIN_ROUTING_VERSION}Global"),
            rule_set: vec![
                rule(
                    "Block udp/443",
                    BLOCK_TAG,
                    None,
                    None,
                    Some("443"),
                    Some("udp"),
                ),
                rule(
                    "Bypass private IPs",
                    DIRECT_TAG,
                    None,
                    Some(vec!["geoip:private"]),
                    None,
                    None,
                ),
                rule("Final proxy", PROXY_TAG, None, None, Some("0-65535"), None),
            ],
            ..RoutingItem::default()
        },
    ]
}

trait RuleBuilder {
    fn with_protocol(self, protocol: Vec<&str>) -> Self;
}

impl RuleBuilder for RulesItem {
    fn with_protocol(mut self, protocol: Vec<&str>) -> Self {
        self.protocol = Some(protocol.into_iter().map(ToOwned::to_owned).collect());
        self
    }
}

fn rule(
    remarks: &str,
    outbound_tag: &str,
    domain: Option<Vec<&str>>,
    ip: Option<Vec<&str>>,
    port: Option<&str>,
    network: Option<&str>,
) -> RulesItem {
    RulesItem {
        remarks: Some(remarks.to_string()),
        outbound_tag: Some(outbound_tag.to_string()),
        domain: domain.map(strings),
        ip: ip.map(strings),
        port: port.map(ToOwned::to_owned),
        network: network.map(ToOwned::to_owned),
        rule_type: Some(RuleType::Routing),
        ..RulesItem::default()
    }
}

fn strings(values: Vec<&str>) -> Vec<String> {
    values.into_iter().map(ToOwned::to_owned).collect()
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
    use voya_core::{AppConfig, RuleType};
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
    async fn routing_manager_imports_builtin_templates_once_and_sets_active() {
        let database = Database::connect_in_memory()
            .await
            .expect("routing manager test operation should succeed");
        let manager = RoutingManager::new(&database);
        let mut config = AppConfig::default();

        let imported = manager
            .apply_prepared_config_template(&mut config, manager.prepare_builtin_config_template())
            .await
            .expect("routing manager test operation should succeed");
        let reused = manager
            .apply_prepared_config_template(&mut config, manager.prepare_builtin_config_template())
            .await
            .expect("routing manager test operation should succeed");

        assert_eq!(imported.routing_ids.len(), 3);
        assert!(reused.reused_existing_routing);
        assert!(config
            .routing_basic_item
            .routing_index_id
            .starts_with("routing-"));
        assert_eq!(
            database
                .routings()
                .active()
                .await
                .expect("routing manager test operation should succeed")
                .expect("routing manager test operation should succeed")
                .remarks,
            "V4-Bypass mainland (Whitelist)"
        );
    }

    /// Re-importing an external template is a repeatable button, and the
    /// external path has no `V4-` version prefix to reuse, so without
    /// remarks-keyed replacement every click appended another full copy.
    #[tokio::test]
    async fn routing_manager_replaces_external_template_routings_on_reimport() {
        let database = Database::connect_in_memory()
            .await
            .expect("routing manager test operation should succeed");
        let manager = RoutingManager::new(&database);
        let mut config = AppConfig::default();

        let first = manager
            .apply_prepared_config_template(&mut config, external_template("direct"))
            .await
            .expect("routing manager test operation should succeed");
        let second = manager
            .apply_prepared_config_template(&mut config, external_template("proxy"))
            .await
            .expect("routing manager test operation should succeed");

        assert!(!first.reused_existing_routing);
        assert!(second.reused_existing_routing);
        assert_eq!(second.routing_ids, first.routing_ids);
        let routings = database
            .routings()
            .list()
            .await
            .expect("routing manager test operation should succeed");
        assert_eq!(routings.len(), 2, "{routings:?}");
        assert_eq!(
            routings
                .iter()
                .map(|item| item.remarks.as_str())
                .collect::<Vec<_>>(),
            vec!["External A", "External B"]
        );
        assert!(routings
            .iter()
            .all(|item| item.rule_set[0].outbound_tag.as_deref() == Some("proxy")));
        assert_eq!(
            config.routing_basic_item.routing_index_id,
            first.routing_ids[0]
        );
    }

    fn external_template(outbound_tag: &str) -> PreparedRoutingTemplate {
        PreparedRoutingTemplate {
            version_prefix: None,
            items: ["External A", "External B"]
                .into_iter()
                .map(|remarks| RoutingItem {
                    remarks: remarks.to_string(),
                    rule_set: vec![RulesItem {
                        remarks: Some(remarks.to_string()),
                        outbound_tag: Some(outbound_tag.to_string()),
                        domain: Some(vec!["full:external.example.com".to_string()]),
                        rule_type: Some(RuleType::Routing),
                        ..RulesItem::default()
                    }],
                    ..RoutingItem::default()
                })
                .collect(),
        }
    }

    #[test]
    fn routing_manager_parses_strict_self_contained_voya_bundle() {
        let parsed = parse_routing_template(
            r#"{
              "schemaVersion": 1,
              "routings": [
                {
                  "id": "",
                  "remarks": "good",
                  "sourceUrl": "",
                  "rules": [{"id":"","kind":null,"port":null,"network":null,"inboundTags":null,"outbound":"direct","ip":null,"domain":["full:good.example.com"],"protocol":null,"process":null,"enabled":true,"remarks":"direct","scope":"routing"}],
                  "enabled": true,
                  "locked": false,
                  "icon": "",
                  "singboxRulesetPath": "",
                  "domainStrategy": "AsIs",
                  "singboxDomainStrategy": "",
                  "sort": 0
                },
                {
                  "id": "",
                  "remarks": "also-good",
                  "sourceUrl": "",
                  "rules": [{"id":"","kind":null,"port":null,"network":null,"inboundTags":null,"outbound":"proxy","ip":null,"domain":["full:proxy.example.com"],"protocol":null,"process":null,"enabled":true,"remarks":"proxy","scope":"routing"}],
                  "enabled": true,
                  "locked": false,
                  "icon": "",
                  "singboxRulesetPath": "",
                  "domainStrategy": "AsIs",
                  "singboxDomainStrategy": "",
                  "sort": 0
                }
              ]
            }"#,
        )
        .expect("strict Voya routing bundle should parse");

        assert_eq!(
            parsed
                .routings
                .iter()
                .map(|item| item.remarks.as_str())
                .collect::<Vec<_>>(),
            vec!["good", "also-good"]
        );
        assert!(parsed.routings.iter().all(|item| item.url.is_empty()));
        assert!(parsed.routings.iter().all(|item| item.rule_set.len() == 1));
    }

    #[test]
    fn routing_bundle_rejects_pascal_case_string_rules_child_urls_and_wrong_versions() {
        for invalid in [
            r#"{"SchemaVersion":1,"Routings":[]}"#,
            r#"{"schemaVersion":2,"routings":[{}]}"#,
            r#"{"schemaVersion":1,"routings":[{"rules":"[]"}]}"#,
            r#"{"schemaVersion":1,"routings":[{"sourceUrl":"https://example.test/rules.json","rules":[]}]}"#,
        ] {
            assert!(parse_routing_template(invalid).is_err());
        }
    }
}
