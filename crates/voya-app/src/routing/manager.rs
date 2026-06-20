use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use thiserror::Error;
use voya_core::{
    AppConfig, MoveAction, RoutingItem, RuleType, RulesItem, BLOCK_TAG, DEFAULT_DOMAIN_STRATEGY,
    DIRECT_TAG, PROXY_TAG,
};
use voya_db::{Database, DbError};
use voya_net::{DownloadClient, DownloadError, DownloadRequest, DEFAULT_TEXT_RESPONSE_LIMIT_BYTES};

const DEFAULT_ROUTING_SORT_STEP: i32 = 10;
const BUILTIN_ROUTING_VERSION: &str = "V4-";
const ROUTING_TEMPLATE_CHILD_HOSTS: &[&str] = &["raw.githubusercontent.com", "cdn.jsdelivr.net"];

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

#[derive(Debug, Clone, Copy)]
pub struct RoutingManager<'db> {
    database: &'db Database,
}

impl<'db> RoutingManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self { database }
    }

    pub async fn list_routings(&self) -> Result<Vec<RoutingItem>> {
        Ok(self.database.routings().list().await?)
    }

    pub async fn get_routing(&self, id: &str) -> Result<Option<RoutingItem>> {
        Ok(self.database.routings().get(id).await?)
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
        let should_activate =
            item.is_active || active_id.as_deref() == Some(item.id.as_str()) || active_id.is_none();

        normalize_routing_item(&mut item);
        if is_new && item.sort <= 0 {
            item.sort = self.database.routings().max_sort().await? + DEFAULT_ROUTING_SORT_STEP;
        }
        item.is_active = should_activate;

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

    pub async fn import_routing_templates(
        &self,
        config: &mut AppConfig,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
        import_advanced_rules: bool,
    ) -> Result<Vec<RoutingItem>> {
        let source_url = config
            .const_item
            .route_rules_template_source_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let imported = if let Some(source_url) = source_url {
            match self
                .import_external_template(
                    config,
                    &source_url,
                    prefer_proxy,
                    proxy_url,
                    import_advanced_rules,
                )
                .await
            {
                Ok(imported) => imported,
                Err(error) => {
                    tracing::warn!(?error, "falling back to built-in routing templates");
                    self.import_builtin_templates(config, import_advanced_rules)
                        .await?
                }
            }
        } else {
            self.import_builtin_templates(config, import_advanced_rules)
                .await?
        };

        self.ensure_active_routing(config).await?;

        Ok(imported)
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

    async fn import_external_template(
        &self,
        config: &mut AppConfig,
        source_url: &str,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
        import_advanced_rules: bool,
    ) -> Result<Vec<RoutingItem>> {
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

        self.apply_template(
            config,
            template,
            Some(source_url),
            Some(&download),
            prefer_proxy,
            proxy_url,
            import_advanced_rules,
        )
        .await
    }

    async fn apply_template(
        &self,
        config: &mut AppConfig,
        template: RoutingTemplate,
        source_url: Option<&str>,
        download: Option<&DownloadClient>,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
        import_advanced_rules: bool,
    ) -> Result<Vec<RoutingItem>> {
        let child_url_policy = source_url
            .map(TemplateChildUrlPolicy::from_source_url)
            .transpose()
            .map_err(RoutingManagerError::InvalidTemplate)?;
        let existing = self.database.routings().list().await?;
        if !import_advanced_rules
            && !template.version.trim().is_empty()
            && existing
                .iter()
                .any(|item| item.remarks.starts_with(template.version.trim()))
        {
            return Ok(Vec::new());
        }

        let mut imported = Vec::new();
        let mut max_sort = self.database.routings().max_sort().await?;
        for (index, template_item) in template.routing_items.into_iter().enumerate() {
            let mut item = match template_item.into_routing_item() {
                Ok(item) => item,
                Err(error) => {
                    tracing::warn!(index, ?error, "skipping invalid routing template item");
                    continue;
                }
            };
            let rules = if item.rule_set.is_empty() {
                let rule_url = item.url.trim().to_string();
                if rule_url.is_empty() {
                    continue;
                }
                let Some(download) = download else {
                    continue;
                };
                if let Some(policy) = child_url_policy.as_ref() {
                    if let Err(reason) = policy.validate_child_url(&rule_url) {
                        tracing::warn!(
                            index,
                            routing = %item.remarks,
                            reason,
                            "skipping routing template item with disallowed rules URL"
                        );
                        continue;
                    }
                }
                let response = match download
                    .download_text(DownloadRequest {
                        url: rule_url,
                        user_agent: None,
                        prefer_proxy,
                        proxy_url: proxy_url.map(ToOwned::to_owned),
                        response_body_limit: Some(DEFAULT_TEXT_RESPONSE_LIMIT_BYTES),
                    })
                    .await
                {
                    Ok(response) => response,
                    Err(error) => {
                        tracing::warn!(
                            index,
                            routing = %item.remarks,
                            ?error,
                            "skipping routing template item after rules download failed"
                        );
                        continue;
                    }
                };
                match parse_rules(&response.body) {
                    Ok(rules) => rules,
                    Err(error) => {
                        tracing::warn!(
                            index,
                            routing = %item.remarks,
                            ?error,
                            "skipping routing template item after rules parse failed"
                        );
                        continue;
                    }
                }
            } else {
                std::mem::take(&mut item.rule_set)
            };

            if rules.is_empty() {
                continue;
            }

            item.rule_set = rules;
            if !template.version.trim().is_empty() {
                item.remarks = format!("{}-{}", template.version.trim(), item.remarks);
            }
            max_sort += DEFAULT_ROUTING_SORT_STEP;
            item.sort = max_sort;
            item.url.clear();
            item.enabled = true;
            item.is_active = !import_advanced_rules && imported.is_empty();
            normalize_routing_item(&mut item);
            self.database.routings().upsert(&item).await?;
            if item.is_active {
                self.database.routings().set_active(&item.id).await?;
                config
                    .routing_basic_item
                    .routing_index_id
                    .clone_from(&item.id);
            }
            imported.push(item);
        }

        Ok(imported)
    }

    async fn import_builtin_templates(
        &self,
        config: &mut AppConfig,
        import_advanced_rules: bool,
    ) -> Result<Vec<RoutingItem>> {
        let existing = self.database.routings().list().await?;
        if !import_advanced_rules
            && existing
                .iter()
                .any(|item| item.remarks.starts_with(BUILTIN_ROUTING_VERSION))
        {
            return Ok(Vec::new());
        }

        let mut imported = Vec::new();
        let mut max_sort = self.database.routings().max_sort().await?;
        for (index, mut item) in builtin_routing_items().into_iter().enumerate() {
            max_sort += DEFAULT_ROUTING_SORT_STEP;
            item.sort = max_sort;
            item.is_active = !import_advanced_rules && index == 0;
            normalize_routing_item(&mut item);
            self.database.routings().upsert(&item).await?;
            if item.is_active {
                self.database.routings().set_active(&item.id).await?;
                config
                    .routing_basic_item
                    .routing_index_id
                    .clone_from(&item.id);
            }
            imported.push(item);
        }

        Ok(imported)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct RoutingTemplate {
    #[serde(alias = "version")]
    version: String,
    #[serde(alias = "routingItems")]
    routing_items: Vec<TemplateRoutingItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct TemplateRoutingItem {
    #[serde(alias = "id")]
    id: String,
    #[serde(alias = "remarks")]
    remarks: String,
    #[serde(alias = "url")]
    url: String,
    #[serde(alias = "ruleSet")]
    rule_set: TemplateRuleSet,
    #[serde(alias = "enabled")]
    enabled: bool,
    #[serde(alias = "locked")]
    locked: bool,
    #[serde(alias = "customIcon")]
    custom_icon: String,
    #[serde(alias = "customRulesetPath4Singbox")]
    custom_ruleset_path4_singbox: String,
    #[serde(alias = "domainStrategy")]
    domain_strategy: String,
    #[serde(alias = "domainStrategy4Singbox")]
    domain_strategy4_singbox: String,
}

impl Default for TemplateRoutingItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            url: String::new(),
            rule_set: TemplateRuleSet::default(),
            enabled: true,
            locked: false,
            custom_icon: String::new(),
            custom_ruleset_path4_singbox: String::new(),
            domain_strategy: String::new(),
            domain_strategy4_singbox: String::new(),
        }
    }
}

impl TemplateRoutingItem {
    fn into_routing_item(self) -> Result<RoutingItem> {
        Ok(RoutingItem {
            id: self.id,
            remarks: self.remarks,
            url: self.url,
            rule_set: self.rule_set.into_rules()?,
            enabled: self.enabled,
            locked: self.locked,
            custom_icon: self.custom_icon,
            custom_ruleset_path4_singbox: self.custom_ruleset_path4_singbox,
            domain_strategy: self.domain_strategy,
            domain_strategy4_singbox: self.domain_strategy4_singbox,
            ..RoutingItem::default()
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum TemplateRuleSet {
    Rules(Vec<RulesItem>),
    Text(String),
}

impl Default for TemplateRuleSet {
    fn default() -> Self {
        Self::Rules(Vec::new())
    }
}

impl TemplateRuleSet {
    fn into_rules(self) -> Result<Vec<RulesItem>> {
        match self {
            Self::Rules(mut rules) => {
                for rule in &mut rules {
                    normalize_rule(rule);
                }
                Ok(rules)
            }
            Self::Text(text) if text.trim().is_empty() => Ok(Vec::new()),
            Self::Text(text) => parse_rules(&text),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TemplateChildUrlPolicy {
    source_origin: UrlOrigin,
}

impl TemplateChildUrlPolicy {
    fn from_source_url(source_url: &str) -> std::result::Result<Self, String> {
        let parsed = reqwest::Url::parse(source_url.trim())
            .map_err(|error| format!("invalid source URL: {error}"))?;
        let source_origin = UrlOrigin::from_url(&parsed)?;

        Ok(Self { source_origin })
    }

    fn validate_child_url(&self, child_url: &str) -> std::result::Result<(), String> {
        let parsed = reqwest::Url::parse(child_url.trim())
            .map_err(|error| format!("invalid rules URL: {error}"))?;
        let child_origin = UrlOrigin::from_url(&parsed)?;
        if child_origin != self.source_origin && !self.is_allowed_known_child_origin(&child_origin)
        {
            return Err("rules URL origin is not allowed for template source".to_string());
        }

        Ok(())
    }

    fn is_allowed_known_child_origin(&self, child_origin: &UrlOrigin) -> bool {
        self.source_origin.scheme == "https"
            && child_origin.scheme == "https"
            && child_origin.port == Some(443)
            && ROUTING_TEMPLATE_CHILD_HOSTS.contains(&self.source_origin.host.as_str())
            && ROUTING_TEMPLATE_CHILD_HOSTS.contains(&child_origin.host.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UrlOrigin {
    scheme: String,
    host: String,
    port: Option<u16>,
}

impl UrlOrigin {
    fn from_url(url: &reqwest::Url) -> std::result::Result<Self, String> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err("scheme must be http or https".to_string());
        }
        let host = url
            .host_str()
            .map(normalize_url_host)
            .filter(|host| !host.is_empty())
            .ok_or_else(|| "host is required".to_string())?;

        Ok(Self {
            scheme: url.scheme().to_string(),
            host,
            port: url.port_or_known_default(),
        })
    }
}

fn normalize_url_host(host: &str) -> String {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

fn parse_routing_template(value: &str) -> Result<RoutingTemplate> {
    let template = serde_json::from_str::<RoutingTemplate>(value)
        .map_err(|error| RoutingManagerError::InvalidTemplate(error.to_string()))?;
    if template.routing_items.is_empty() {
        return Err(RoutingManagerError::InvalidTemplate(
            "template contains no routing items".to_string(),
        ));
    }

    Ok(template)
}

fn parse_rules(value: &str) -> Result<Vec<RulesItem>> {
    let mut rules = serde_json::from_str::<Vec<RulesItem>>(value)
        .map_err(|error| RoutingManagerError::InvalidRules(error.to_string()))?;
    for rule in &mut rules {
        normalize_rule(rule);
    }

    Ok(rules)
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
    item.rule_num = i32::try_from(item.rule_set.len()).unwrap_or(i32::MAX);
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
    use std::{collections::HashMap, sync::Arc};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
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
        assert!(
            manager
                .set_active_routing(&mut config, &second.id)
                .await
                .expect("routing manager test operation should succeed")
                .is_active
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
        assert_eq!(added.rule_num, 2);
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
            .import_routing_templates(&mut config, false, None, false)
            .await
            .expect("routing manager test operation should succeed");
        let skipped = manager
            .import_routing_templates(&mut config, false, None, false)
            .await
            .expect("routing manager test operation should succeed");

        assert_eq!(imported.len(), 3);
        assert!(skipped.is_empty());
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

    #[tokio::test]
    async fn routing_manager_external_template_skips_bad_secondary_items() {
        let routes = HashMap::from([
            (
                "/template.json".to_string(),
                r#"{
                  "version": "R1",
                  "routingItems": [
                    { "remarks": "missing", "url": "__BASE__/missing.json" },
                    { "remarks": "bad-json", "url": "__BASE__/bad.json" },
                    { "remarks": "blocked", "url": "http://127.0.0.1:9/blocked.json" },
                    { "remarks": "good", "url": "__BASE__/good.json" },
                    { "remarks": "also-good", "url": "__BASE__/also-good.json" }
                  ]
                }"#
                .to_string(),
            ),
            (
                "/good.json".to_string(),
                r#"[{"remarks":"direct","outboundTag":"direct","domain":["full:good.example.com"]}]"#
                    .to_string(),
            ),
            ("/bad.json".to_string(), r#"{"not":"an array"}"#.to_string()),
            (
                "/also-good.json".to_string(),
                r#"[{"remarks":"proxy","outboundTag":"proxy","domain":["full:proxy.example.com"]}]"#
                    .to_string(),
            ),
        ]);
        let base = spawn_http_fixture(routes, 5).await;
        let database = Database::connect_in_memory()
            .await
            .expect("routing manager test operation should succeed");
        let manager = RoutingManager::new(&database);
        let mut config = AppConfig::default();
        config.const_item.route_rules_template_source_url = Some(format!("{base}/template.json"));

        let imported = manager
            .import_routing_templates(&mut config, false, None, false)
            .await
            .expect("routing manager test operation should succeed");

        assert_eq!(
            imported
                .iter()
                .map(|item| item.remarks.as_str())
                .collect::<Vec<_>>(),
            vec!["R1-good", "R1-also-good"]
        );
        assert!(imported[0].is_active);
        assert!(!imported[1].is_active);

        let stored = database
            .routings()
            .list()
            .await
            .expect("routing manager test operation should succeed");
        assert_eq!(stored.len(), 2);
        assert!(stored.iter().all(|item| item.url.is_empty()));
        assert!(stored.iter().all(|item| item.rule_num == 1));
        assert!(!stored.iter().any(|item| item.remarks == "R1-missing"
            || item.remarks == "R1-bad-json"
            || item.remarks == "R1-blocked"));

        let active = database
            .routings()
            .active()
            .await
            .expect("routing manager test operation should succeed")
            .expect("routing manager test operation should succeed");
        assert_eq!(active.remarks, "R1-good");
        assert_eq!(config.routing_basic_item.routing_index_id, active.id);
    }

    #[test]
    fn routing_template_child_url_policy_requires_same_origin() {
        let policy =
            TemplateChildUrlPolicy::from_source_url("https://cdn.example.test/templates/main.json")
                .expect("routing manager test operation should succeed");

        policy
            .validate_child_url("https://cdn.example.test/rules/direct.json")
            .expect("same-origin child URL should be allowed");
        policy
            .validate_child_url("https://cdn.example.test:443/rules/direct.json")
            .expect("default port should match same-origin child URL");

        let preset_policy = TemplateChildUrlPolicy::from_source_url(
            "https://raw.githubusercontent.com/Chocolate4U/Iran-v2ray-rules/main/v2rayN/template.json",
        )
        .expect("routing manager test operation should succeed");
        preset_policy
            .validate_child_url(
                "https://cdn.jsdelivr.net/gh/Chocolate4U/Iran-v2ray-rules@main/v2rayN/all.json",
            )
            .expect("known HTTPS routing template CDN mirror should be allowed");
        preset_policy
            .validate_child_url(
                "http://cdn.jsdelivr.net/gh/Chocolate4U/Iran-v2ray-rules@main/v2rayN/all.json",
            )
            .expect_err("known CDN mirror must still use HTTPS");

        for denied in [
            "http://cdn.example.test/rules/direct.json",
            "https://cdn.example.test:8443/rules/direct.json",
            "https://evil.example.test/rules/direct.json",
            "file:///tmp/rules.json",
            "https://127.0.0.1/rules/direct.json",
        ] {
            policy
                .validate_child_url(denied)
                .expect_err("cross-origin or non-HTTP child URL should be rejected");
        }
    }

    #[test]
    fn routing_template_parser_accepts_v2rayn_camel_case_rule_json() {
        let template = parse_routing_template(
            r#"{
              "version": "R1",
              "routingItems": [
                {
                  "remarks": "split",
                  "ruleSet": "[{\"remarks\":\"direct\",\"outboundTag\":\"direct\",\"domain\":[\"full:direct.example.com\"]}]"
                }
              ]
            }"#,
        )
        .expect("routing manager test operation should succeed");
        let item = template
            .routing_items
            .into_iter()
            .next()
            .expect("routing manager test operation should succeed")
            .into_routing_item()
            .expect("routing manager test operation should succeed");

        assert_eq!(item.remarks, "split");
        assert_eq!(item.rule_set[0].outbound_tag.as_deref(), Some(DIRECT_TAG));
        assert_eq!(
            item.rule_set[0]
                .domain
                .as_ref()
                .expect("routing manager test operation should succeed"),
            &vec!["full:direct.example.com".to_string()]
        );
    }

    async fn spawn_http_fixture(routes: HashMap<String, String>, max_requests: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("routing manager test operation should succeed");
        let address = listener
            .local_addr()
            .expect("routing manager test operation should succeed");
        let base = format!("http://{address}");
        let server_base = base.clone();
        let routes = Arc::new(routes);

        tokio::spawn(async move {
            for _ in 0..max_requests {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let routes = Arc::clone(&routes);
                let server_base = server_base.clone();
                tokio::spawn(async move {
                    let mut buffer = vec![0; 8192];
                    let bytes_read = socket.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .and_then(|target| target.split('?').next())
                        .unwrap_or("/");
                    let body = routes
                        .get(path)
                        .map(|body| body.replace("__BASE__", &server_base))
                        .unwrap_or_default();
                    let status = if routes.contains_key(path) {
                        "200 OK"
                    } else {
                        "404 Not Found"
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        base
    }
}
