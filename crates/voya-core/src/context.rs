use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use regex::Regex;
use serde_json::Value;
use thiserror::Error;

use crate::{
    singbox::support::singbox_supports_config_type, AppConfig, ConfigType, CoreType,
    InboundProtocol, ProfileItem, ProfileProtocol, ProfileTransport, RoutingItem, RulesItem,
    ServerEndpoint, SimpleDnsItem, SubItem, TlsMode,
};

pub const PROXY_TAG: &str = "proxy";
pub const DIRECT_TAG: &str = "direct";
pub const BLOCK_TAG: &str = "block";
pub const STREAM_SECURITY_TLS: &str = "tls";
pub const LOOPBACK: &str = "127.0.0.1";
pub const DEFAULT_NETWORK: &str = "raw";

const XHTTP: &str = "xhttp";
const KCP: &str = "kcp";
const WS: &str = "ws";
const SHADOWSOCKS_RAW: &str = "raw";

const SINGBOX_UNSUPPORTED_TRANSPORTS: &[&str] = &[KCP, XHTTP];
const SINGBOX_SHADOWSOCKS_ALLOWED_TRANSPORTS: &[&str] = &[SHADOWSOCKS_RAW, WS];
const FLOWS: &[&str] = &["", "xtls-rprx-vision", "xtls-rprx-vision-udp443"];
/// Single source of truth for the Shadowsocks ciphers sing-box accepts.
///
/// `crate::singbox` reads the same list when it emits the `method` field so
/// validation and generation cannot drift apart.
pub(crate) const SS_SECURITIES_IN_SINGBOX: &[&str] = &[
    "aes-256-gcm",
    "aes-192-gcm",
    "aes-128-gcm",
    "chacha20-ietf-poly1305",
    "xchacha20-ietf-poly1305",
    "none",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
    "aes-128-ctr",
    "aes-192-ctr",
    "aes-256-ctr",
    "aes-128-cfb",
    "aes-192-cfb",
    "aes-256-cfb",
    "rc4-md5",
    "chacha20-ietf",
    "xchacha20",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreGenPlatform {
    Windows,
    MacOS,
    Linux,
}

impl CoreGenPlatform {
    #[must_use]
    pub const fn is_windows(self) -> bool {
        matches!(self, Self::Windows)
    }

    #[must_use]
    pub const fn is_macos(self) -> bool {
        matches!(self, Self::MacOS)
    }

    #[must_use]
    pub const fn is_non_windows(self) -> bool {
        !self.is_windows()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeValidatorResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl NodeValidatorResult {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn success(&self) -> bool {
        self.errors.is_empty()
    }

    fn push_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    fn push_error(&mut self, error: impl Into<String>) {
        self.errors.push(error.into());
    }

    fn extend_prefixed_warnings(&mut self, prefix: &str, result: &Self) {
        self.warnings.extend(
            result
                .warnings
                .iter()
                .map(|warning| format!("{prefix}: {warning}")),
        );
    }

    fn extend_prefixed_errors(&mut self, prefix: &str, result: &Self) {
        self.errors.extend(
            result
                .errors
                .iter()
                .map(|error| format!("{prefix}: {error}")),
        );
    }

    #[must_use]
    pub fn combined(left: &Self, right: Option<&Self>) -> Self {
        let mut combined = Self {
            errors: left.errors.clone(),
            warnings: left.warnings.clone(),
        };
        if let Some(right) = right {
            combined.errors.extend(right.errors.clone());
            combined.warnings.extend(right.warnings.clone());
        }
        combined
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreConfigContext {
    pub node: ProfileItem,
    pub run_core_type: CoreType,
    pub routing_item: Option<RoutingItem>,
    pub simple_dns_item: SimpleDnsItem,
    pub all_proxies_map: BTreeMap<String, ProfileItem>,
    pub app_config: AppConfig,
    pub server_test_item_map: BTreeMap<String, String>,
    pub is_tun_enabled: bool,
    pub protect_domain_list: Vec<String>,
    pub platform: CoreGenPlatform,
    pub singbox_ruleset_paths: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeedtestConfigEntry {
    pub index_id: String,
    pub port: i32,
    pub context: CoreConfigContext,
}

impl Default for CoreConfigContext {
    fn default() -> Self {
        Self {
            node: ProfileItem::default(),
            run_core_type: CoreType::sing_box,
            routing_item: None,
            simple_dns_item: SimpleDnsItem::default(),
            all_proxies_map: BTreeMap::new(),
            app_config: AppConfig::default(),
            server_test_item_map: BTreeMap::new(),
            is_tun_enabled: false,
            protect_domain_list: Vec::new(),
            platform: CoreGenPlatform::Linux,
            singbox_ruleset_paths: BTreeMap::new(),
        }
    }
}

impl CoreConfigContext {
    #[must_use]
    pub fn is_windows(&self) -> bool {
        self.platform.is_windows()
    }

    #[must_use]
    pub fn is_macos(&self) -> bool {
        self.platform.is_macos()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreConfigContextBuilderResult {
    pub context: CoreConfigContext,
    pub validator_result: NodeValidatorResult,
}

impl CoreConfigContextBuilderResult {
    #[must_use]
    pub fn success(&self) -> bool {
        self.validator_result.success()
    }
}

impl Default for CoreConfigContextBuilderResult {
    fn default() -> Self {
        Self {
            context: CoreConfigContext::default(),
            validator_result: NodeValidatorResult::empty(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoreConfigContextBuilderAllResult {
    pub main_result: CoreConfigContextBuilderResult,
    pub pre_socks_result: Option<CoreConfigContextBuilderResult>,
}

impl CoreConfigContextBuilderAllResult {
    #[must_use]
    pub fn success(&self) -> bool {
        self.main_result.success()
            && self
                .pre_socks_result
                .as_ref()
                .is_none_or(CoreConfigContextBuilderResult::success)
    }

    #[must_use]
    pub fn combined_validator_result(&self) -> NodeValidatorResult {
        NodeValidatorResult::combined(
            &self.main_result.validator_result,
            self.pre_socks_result
                .as_ref()
                .map(|result| &result.validator_result),
        )
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextBuildError {
    #[error("active profile id is empty")]
    MissingActiveProfileId,
    #[error("active profile {0} was not found")]
    ActiveProfileNotFound(String),
}

pub trait CoreGenEnv {
    fn platform(&self) -> CoreGenPlatform;

    fn get_active_profile(&self, config: &AppConfig) -> Option<ProfileItem> {
        let active_id = config.index_id.trim();
        if active_id.is_empty() {
            return None;
        }
        self.get_profile_by_index_id(active_id)
    }

    fn get_profile_by_index_id(&self, index_id: &str) -> Option<ProfileItem>;

    fn get_profile_by_remarks(&self, remarks: &str) -> Option<ProfileItem>;

    fn get_profile_items_ordered_by_index_ids(&self, index_ids: &[String]) -> Vec<ProfileItem>;

    fn get_profile_items_by_subscription_id(&self, subscription_id: &str) -> Vec<ProfileItem>;

    fn get_subscription(&self, subscription_id: &str) -> Option<SubItem>;

    fn get_default_routing(&self, config: &AppConfig) -> Option<RoutingItem>;

    fn get_local_port(&self, protocol: InboundProtocol) -> i32;

    fn get_singbox_ruleset_paths(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CoreConfigContextBuilder<'env, E> {
    env: &'env E,
}

impl<'env, E> CoreConfigContextBuilder<'env, E>
where
    E: CoreGenEnv,
{
    #[must_use]
    pub fn new(env: &'env E) -> Self {
        Self { env }
    }

    pub fn build_active(
        &self,
        config: &AppConfig,
    ) -> Result<CoreConfigContextBuilderResult, ContextBuildError> {
        let active_id = config.index_id.trim();
        if active_id.is_empty() {
            return Err(ContextBuildError::MissingActiveProfileId);
        }
        let node = self
            .env
            .get_active_profile(config)
            .ok_or_else(|| ContextBuildError::ActiveProfileNotFound(active_id.to_string()))?;
        Ok(self.build(config, &node))
    }

    #[must_use]
    pub fn build(&self, config: &AppConfig, node: &ProfileItem) -> CoreConfigContextBuilderResult {
        let run_core_type = CoreType::sing_box;
        let mut context = CoreConfigContext {
            node: node.clone(),
            run_core_type,
            routing_item: self.env.get_default_routing(config),
            simple_dns_item: config.simple_dns_item.clone(),
            all_proxies_map: BTreeMap::new(),
            app_config: config.clone(),
            server_test_item_map: BTreeMap::new(),
            is_tun_enabled: config.tun_mode_item.enable_tun,
            protect_domain_list: Vec::new(),
            platform: self.env.platform(),
            singbox_ruleset_paths: self.env.get_singbox_ruleset_paths(),
        };

        let (active_node, node_result) = self.resolve_node(&mut context, node);
        if !node_result.success() {
            return CoreConfigContextBuilderResult {
                context,
                validator_result: node_result,
            };
        }
        context.node = active_node;

        let mut validator_result = NodeValidatorResult::empty();
        validator_result.warnings.extend(node_result.warnings);
        self.resolve_rule_outbounds(&mut context, &mut validator_result);

        CoreConfigContextBuilderResult {
            context,
            validator_result,
        }
    }

    #[must_use]
    pub fn build_all(
        &self,
        config: &AppConfig,
        node: &ProfileItem,
    ) -> CoreConfigContextBuilderAllResult {
        let main_result = self.build(config, node);
        if !main_result.success() {
            return CoreConfigContextBuilderAllResult {
                main_result,
                pre_socks_result: None,
            };
        }

        let Some(pre_socks_result) = self.build_pre_socks_if_needed(&main_result.context) else {
            return CoreConfigContextBuilderAllResult {
                main_result,
                pre_socks_result: None,
            };
        };

        let mut resolved_main_result = main_result;
        resolved_main_result.context.is_tun_enabled = false;
        merge_protect_domains(
            &mut resolved_main_result.context.protect_domain_list,
            &pre_socks_result.context.protect_domain_list,
        );

        CoreConfigContextBuilderAllResult {
            main_result: resolved_main_result,
            pre_socks_result: Some(pre_socks_result),
        }
    }

    fn build_pre_socks_if_needed(
        &self,
        node_context: &CoreConfigContext,
    ) -> Option<CoreConfigContextBuilderResult> {
        let config = &node_context.app_config;
        let node = &node_context.node;
        let pre_socks_item = pre_socks_item(config, node, self.env)?;
        let mut pre_socks_result = self.build(config, &pre_socks_item);
        let pre_socks_domains = pre_socks_result.context.protect_domain_list.clone();
        pre_socks_result.context.protect_domain_list = node_context.protect_domain_list.clone();
        merge_protect_domains(
            &mut pre_socks_result.context.protect_domain_list,
            &pre_socks_domains,
        );
        Some(pre_socks_result)
    }

    fn resolve_node(
        &self,
        context: &mut CoreConfigContext,
        node: &ProfileItem,
    ) -> (ProfileItem, NodeValidatorResult) {
        if node.index_id.trim().is_empty() {
            return (node.clone(), NodeValidatorResult::empty());
        }

        let register_result = self.register_node(context, node);
        (node.clone(), register_result)
    }

    fn register_node(
        &self,
        context: &mut CoreConfigContext,
        node: &ProfileItem,
    ) -> NodeValidatorResult {
        if node.config_type().is_group_type() {
            return self.register_group_node(context, node);
        }

        register_single_node(context, node)
    }

    fn register_group_node(
        &self,
        context: &mut CoreConfigContext,
        node: &ProfileItem,
    ) -> NodeValidatorResult {
        if !node.config_type().is_group_type() {
            return NodeValidatorResult::empty();
        }

        let mut ancestors = BTreeSet::new();
        ancestors.insert(node.index_id.clone());
        let mut global_visited = BTreeSet::new();
        global_visited.insert(node.index_id.clone());
        self.traverse_group_node(context, node, &mut global_visited, &ancestors)
    }

    fn traverse_group_node(
        &self,
        context: &mut CoreConfigContext,
        node: &ProfileItem,
        global_visited: &mut BTreeSet<String>,
        ancestors: &BTreeSet<String>,
    ) -> NodeValidatorResult {
        let mut child_index_ids = Vec::new();
        let mut child_index_seen = BTreeSet::new();
        let mut child_result = NodeValidatorResult::empty();
        let group_child_list = self.group_child_profile_items(&node.protocol, &mut child_result);

        for child_node in group_child_list {
            if ancestors.contains(&child_node.index_id) {
                child_result.push_error(format!(
                    "group cycle dependency: {} -> {}",
                    node.remarks, child_node.remarks
                ));
                continue;
            }

            if global_visited.contains(&child_node.index_id) {
                push_unique_child_index(
                    &mut child_index_ids,
                    &mut child_index_seen,
                    &child_node.index_id,
                );
                continue;
            }

            if !child_node.config_type().is_group_type() {
                let child_node_result = register_single_node(context, &child_node);
                child_result.extend_prefixed_warnings(
                    &format!("group child {} / {}", node.remarks, child_node.remarks),
                    &child_node_result,
                );
                child_result.extend_prefixed_errors(
                    &format!("group child {} / {}", node.remarks, child_node.remarks),
                    &child_node_result,
                );
                if !child_node_result.success() {
                    continue;
                }

                global_visited.insert(child_node.index_id.clone());
                push_unique_child_index(
                    &mut child_index_ids,
                    &mut child_index_seen,
                    &child_node.index_id,
                );
                continue;
            }

            let mut new_ancestors = ancestors.clone();
            new_ancestors.insert(child_node.index_id.clone());
            let child_group_result =
                self.traverse_group_node(context, &child_node, global_visited, &new_ancestors);
            child_result.extend_prefixed_warnings(
                &format!(
                    "group child group {} / {}",
                    node.remarks, child_node.remarks
                ),
                &child_group_result,
            );
            child_result.extend_prefixed_errors(
                &format!(
                    "group child group {} / {}",
                    node.remarks, child_node.remarks
                ),
                &child_group_result,
            );
            if !child_group_result.success() {
                continue;
            }

            global_visited.insert(child_node.index_id.clone());
            push_unique_child_index(
                &mut child_index_ids,
                &mut child_index_seen,
                &child_node.index_id,
            );
        }

        if child_index_ids.is_empty() {
            child_result.push_error(format!("group has no valid child node: {}", node.remarks));
            return child_result;
        }

        child_result.warnings.extend(child_result.errors.clone());
        child_result.errors.clear();

        let mut resolved_node = node.clone();
        resolved_node
            .protocol
            .replace_child_profile_ids(child_index_ids);
        context
            .all_proxies_map
            .insert(resolved_node.index_id.clone(), resolved_node);
        child_result
    }

    fn group_child_profile_items(
        &self,
        protocol: &ProfileProtocol,
        result: &mut NodeValidatorResult,
    ) -> Vec<ProfileItem> {
        let mut items = Vec::new();
        items.extend(self.sub_child_profile_items(protocol, result));
        items.extend(self.selected_child_profile_items(protocol));
        items
    }

    fn selected_child_profile_items(&self, protocol: &ProfileProtocol) -> Vec<ProfileItem> {
        let child_ids = protocol.child_profile_ids();
        if child_ids.is_empty() {
            return Vec::new();
        }
        self.env.get_profile_items_ordered_by_index_ids(child_ids)
    }

    fn sub_child_profile_items(
        &self,
        protocol: &ProfileProtocol,
        result: &mut NodeValidatorResult,
    ) -> Vec<ProfileItem> {
        let ProfileProtocol::PolicyGroup {
            source_subscription_id: Some(subscription_id),
            filter,
            ..
        } = protocol
        else {
            return Vec::new();
        };
        // An unparsable filter must select nothing: falling back to "no filter"
        // would silently turn a filtered group into an all-nodes group.
        let filter = match filter.as_deref().and_then(nonempty) {
            Some(pattern) => match Regex::new(pattern) {
                Ok(filter) => Some(filter),
                Err(_) => {
                    result.push_error(format!("invalid subscription filter regex: {pattern}"));
                    return Vec::new();
                }
            },
            None => None,
        };

        self.env
            .get_profile_items_by_subscription_id(subscription_id)
            .into_iter()
            .filter(|profile| {
                !profile.config_type().is_complex_type()
                    && profile_is_valid(profile)
                    && filter
                        .as_ref()
                        .is_none_or(|filter| filter.is_match(&profile.remarks))
            })
            .collect()
    }

    fn resolve_rule_outbounds(
        &self,
        context: &mut CoreConfigContext,
        validator_result: &mut NodeValidatorResult,
    ) {
        let Some(routing_item) = context.routing_item.clone() else {
            return;
        };

        for rule_item in routing_item
            .rule_set
            .iter()
            .filter(|rule| rule.enabled && !is_builtin_outbound(rule.outbound_tag.as_deref()))
        {
            self.resolve_rule_outbound(context, validator_result, rule_item);
        }
    }

    fn resolve_rule_outbound(
        &self,
        context: &mut CoreConfigContext,
        validator_result: &mut NodeValidatorResult,
        rule_item: &RulesItem,
    ) {
        let rule_name = rule_item.remarks.as_deref().unwrap_or_default();
        let Some(outbound_tag) = rule_item.outbound_tag.as_deref().and_then(nonempty) else {
            validator_result
                .push_warning(format!("routing rule {rule_name} has empty outbound tag"));
            return;
        };

        // A rule that names an outbound node must not fall back to the active
        // node: routing.rs would silently retarget it to the main proxy.
        let Some(rule_outbound_node) = self.env.get_profile_by_remarks(outbound_tag) else {
            validator_result.push_error(format!(
                "routing rule {rule_name} outbound node not found: {outbound_tag}"
            ));
            return;
        };

        let (active_rule_node, rule_result) = self.resolve_node(context, &rule_outbound_node);
        validator_result
            .warnings
            .extend(rule_result.warnings.iter().map(|warning| {
                format!("routing rule {rule_name} outbound {outbound_tag} warning: {warning}")
            }));

        if !rule_result.success() {
            validator_result
                .errors
                .extend(rule_result.errors.iter().map(|error| {
                    format!("routing rule {rule_name} outbound {outbound_tag} error: {error}")
                }));
            return;
        }

        context
            .all_proxies_map
            .insert(format!("remark:{outbound_tag}"), active_rule_node);
    }
}

mod node_registration;
mod validation;
use node_registration::{pre_socks_item, register_single_node};
use validation::*;
pub use validation::{is_domain, validate_node};
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoreBasicItem, RoutingBasicItem, RulesItem, TunModeItem};

    #[derive(Debug, Clone)]
    struct MemoryEnv {
        platform: CoreGenPlatform,
        profiles: Vec<ProfileItem>,
        subs: Vec<SubItem>,
        routings: Vec<RoutingItem>,
        local_socks_port: i32,
    }

    impl Default for MemoryEnv {
        fn default() -> Self {
            Self {
                platform: CoreGenPlatform::Linux,
                profiles: Vec::new(),
                subs: Vec::new(),
                routings: Vec::new(),
                local_socks_port: 10808,
            }
        }
    }

    impl CoreGenEnv for MemoryEnv {
        fn platform(&self) -> CoreGenPlatform {
            self.platform
        }

        fn get_profile_by_index_id(&self, index_id: &str) -> Option<ProfileItem> {
            self.profiles
                .iter()
                .find(|profile| profile.index_id == index_id)
                .cloned()
        }

        fn get_profile_by_remarks(&self, remarks: &str) -> Option<ProfileItem> {
            self.profiles
                .iter()
                .find(|profile| profile.remarks == remarks)
                .cloned()
        }

        fn get_profile_items_ordered_by_index_ids(&self, index_ids: &[String]) -> Vec<ProfileItem> {
            index_ids
                .iter()
                .filter_map(|index_id| self.get_profile_by_index_id(index_id))
                .collect()
        }

        fn get_profile_items_by_subscription_id(&self, subscription_id: &str) -> Vec<ProfileItem> {
            self.profiles
                .iter()
                .filter(|profile| profile.subscription_id.as_deref() == Some(subscription_id))
                .cloned()
                .collect()
        }

        fn get_subscription(&self, subscription_id: &str) -> Option<SubItem> {
            self.subs
                .iter()
                .find(|sub| sub.id == subscription_id)
                .cloned()
        }

        fn get_default_routing(&self, config: &AppConfig) -> Option<RoutingItem> {
            self.routings
                .iter()
                .find(|routing| routing.id == config.routing_basic_item.routing_index_id)
                .or_else(|| self.routings.first())
                .cloned()
        }

        fn get_local_port(&self, protocol: InboundProtocol) -> i32 {
            match protocol {
                InboundProtocol::socks => self.local_socks_port,
                _ => self.local_socks_port + protocol.port_offset(),
            }
        }
    }

    #[test]
    fn context_build_active_resolves_structured_dns_routing_and_active_node() {
        let active = vless_profile("active", "Active", "active.example.com");
        let config = app_config("active");
        let env = MemoryEnv {
            profiles: vec![active],
            routings: vec![RoutingItem {
                id: "routing".to_string(),
                ..RoutingItem::default()
            }],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env)
            .build_active(&config)
            .expect("active context");

        assert!(result.success());
        assert_eq!(result.context.node.index_id, "active");
        assert_eq!(result.context.run_core_type, CoreType::sing_box);
        assert_eq!(
            result.context.routing_item.as_ref().map(|item| &item.id),
            Some(&"routing".to_string())
        );
        assert_eq!(result.context.simple_dns_item, config.simple_dns_item);
    }

    #[test]
    fn context_registers_rule_outbounds_by_remark() {
        let active = vless_profile("active", "Active", "active.example.com");
        let rule_node = vless_profile("rule", "RuleNode", "rule.example.com");
        let env = MemoryEnv {
            profiles: vec![active.clone(), rule_node],
            routings: vec![RoutingItem {
                id: "routing".to_string(),
                rule_set: vec![RulesItem {
                    id: "rule-1".to_string(),
                    outbound_tag: Some("RuleNode".to_string()),
                    remarks: Some("route through node".to_string()),
                    ..RulesItem::default()
                }],
                ..RoutingItem::default()
            }],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&app_config("active"), &active);

        assert!(result.success());
        assert_eq!(
            result
                .context
                .all_proxies_map
                .get("remark:RuleNode")
                .map(|profile| profile.index_id.as_str()),
            Some("rule")
        );
    }

    #[test]
    fn context_group_resolution_detects_cycles_and_dedupes_children() {
        let leaf = vless_profile("leaf", "Leaf", "leaf.example.com");
        let mut root = group_profile("root", "Root", "leaf,leaf,nested");
        let nested = group_profile("nested", "Nested", "root,leaf");
        if let ProfileProtocol::PolicyGroup {
            source_subscription_id,
            filter,
            ..
        } = &mut root.protocol
        {
            *source_subscription_id = Some("sub".to_string());
            *filter = Some("^Sub".to_string());
        }
        let sub_leaf = ProfileItem {
            subscription_id: Some("sub".to_string()),
            ..vless_profile("sub-leaf", "Sub Leaf", "sub.example.com")
        };
        let ignored_sub_leaf = ProfileItem {
            subscription_id: Some("sub".to_string()),
            ..vless_profile("ignored-sub-leaf", "Ignored", "ignored.example.com")
        };
        let env = MemoryEnv {
            profiles: vec![leaf, root.clone(), nested, sub_leaf, ignored_sub_leaf],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&app_config("root"), &root);

        assert!(result.success());
        assert_eq!(
            result
                .context
                .all_proxies_map
                .get("root")
                .map(|profile| profile.protocol.child_profile_ids()),
            Some(
                [
                    "sub-leaf".to_string(),
                    "leaf".to_string(),
                    "nested".to_string()
                ]
                .as_slice()
            )
        );
        assert!(result
            .validator_result
            .warnings
            .iter()
            .any(|warning| warning.contains("cycle dependency")));
    }

    #[test]
    fn context_invalid_subscription_filter_matches_nothing_and_reports_error() {
        let sub_leaf = ProfileItem {
            subscription_id: Some("sub".to_string()),
            ..vless_profile("sub-leaf", "Sub Leaf", "sub.example.com")
        };
        let group = ProfileItem {
            index_id: "group".to_string(),
            remarks: "Group".to_string(),
            protocol: ProfileProtocol::PolicyGroup {
                child_profile_ids: Vec::new(),
                source_subscription_id: Some("sub".to_string()),
                filter: Some("^(HK|SG".to_string()),
                strategy: crate::MultipleLoad::LeastPing,
            },
            ..ProfileItem::default()
        };
        let env = MemoryEnv {
            profiles: vec![sub_leaf, group.clone()],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&app_config("group"), &group);

        assert!(!result.success());
        assert!(result
            .validator_result
            .errors
            .iter()
            .any(|error| error.contains("invalid subscription filter regex")));
        assert!(!result.context.all_proxies_map.contains_key("sub-leaf"));
    }

    #[test]
    fn context_unresolvable_rule_outbound_is_a_validation_error() {
        let active = vless_profile("active", "Active", "active.example.com");
        let env = MemoryEnv {
            profiles: vec![active.clone()],
            routings: vec![routing_with_outbound_tag("RenamedNode")],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&app_config("active"), &active);

        assert!(!result.success());
        assert!(result
            .validator_result
            .errors
            .iter()
            .any(|error| error.contains("outbound node not found: RenamedNode")));
        assert!(!result
            .context
            .all_proxies_map
            .contains_key("remark:RenamedNode"));
    }

    #[test]
    fn context_invalid_rule_outbound_node_is_a_validation_error() {
        let active = vless_profile("active", "Active", "active.example.com");
        let mut broken = vless_profile("broken", "BrokenNode", "broken.example.com");
        if let ProfileProtocol::Vless { flow, .. } = &mut broken.protocol {
            *flow = Some("bogus-flow".to_string());
        }
        let env = MemoryEnv {
            profiles: vec![active.clone(), broken],
            routings: vec![routing_with_outbound_tag("BrokenNode")],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&app_config("active"), &active);

        assert!(!result.success());
        assert!(result
            .validator_result
            .errors
            .iter()
            .any(|error| error.contains("outbound BrokenNode error")));
        assert!(!result
            .context
            .all_proxies_map
            .contains_key("remark:BrokenNode"));
    }

    #[test]
    fn context_protect_domains_include_address_and_ech_sni() {
        let mut active = vless_profile("active", "Active", "node.example.com");
        active.tls = Some(crate::TlsSettings {
            mode: TlsMode::Tls,
            server_name: Some("fallback.example.com".to_string()),
            alpn: Vec::new(),
            reality_public_key: None,
            reality_short_id: None,
            reality_spider_x: None,
            mldsa65_verify: None,
            certificate_pem: None,
            certificate_sha256: Vec::new(),
            ech_config: vec![
                "ech-query.example.com".to_string(),
                "https://dns.example/dns-query".to_string(),
            ],
            final_mask: None,
        });
        let env = MemoryEnv {
            profiles: vec![active.clone()],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&app_config("active"), &active);

        assert!(result.success());
        assert_eq!(
            result.context.protect_domain_list,
            vec![
                "node.example.com".to_string(),
                "ech-query.example.com".to_string()
            ]
        );
    }

    #[test]
    fn context_build_all_creates_pre_socks_and_disables_main_tun() {
        let active = vless_profile("active", "Active", "active.example.com");
        let mut config = app_config("active");
        config.tun_mode_item.enable_tun = true;
        let env = MemoryEnv {
            platform: CoreGenPlatform::Linux,
            profiles: vec![active.clone()],
            local_socks_port: 20808,
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build_all(&config, &active);

        assert!(result.success());
        assert!(!result.main_result.context.is_tun_enabled);
        let pre_context = &result
            .pre_socks_result
            .as_ref()
            .expect("pre socks context")
            .context;
        assert_eq!(pre_context.node.config_type(), ConfigType::SOCKS);
        assert_eq!(pre_context.node.address(), LOOPBACK);
        assert_eq!(pre_context.node.port(), 20808);
    }

    #[test]
    fn context_build_all_keeps_tun_direct_on_windows() {
        let active = vless_profile("active", "Active", "active.example.com");
        let mut config = app_config("active");
        config.tun_mode_item.enable_tun = true;
        let env = MemoryEnv {
            platform: CoreGenPlatform::Windows,
            profiles: vec![active.clone()],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build_all(&config, &active);

        assert!(result.success());
        assert!(result.main_result.context.is_tun_enabled);
        assert!(result.pre_socks_result.is_none());
    }

    #[test]
    fn context_custom_pre_socks_uses_configured_port_without_tun() {
        let active = ProfileItem {
            index_id: "custom".to_string(),
            subscription_id: Some("custom-sub".to_string()),
            remarks: "Custom".to_string(),
            protocol: ProfileProtocol::Custom {
                source: String::new(),
                filter: None,
            },
            ..ProfileItem::default()
        };
        let env = MemoryEnv {
            profiles: vec![active.clone()],
            subs: vec![SubItem {
                id: "custom-sub".to_string(),
                pre_socks_port: Some(18888),
                ..SubItem::default()
            }],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build_all(&app_config("custom"), &active);

        let pre_context = &result
            .pre_socks_result
            .as_ref()
            .expect("custom pre socks context")
            .context;
        assert_eq!(pre_context.node.config_type(), ConfigType::SOCKS);
        assert_eq!(pre_context.node.port(), 18888);
    }

    fn routing_with_outbound_tag(outbound_tag: &str) -> RoutingItem {
        RoutingItem {
            id: "routing".to_string(),
            rule_set: vec![RulesItem {
                id: "rule-1".to_string(),
                outbound_tag: Some(outbound_tag.to_string()),
                remarks: Some("route through node".to_string()),
                ..RulesItem::default()
            }],
            ..RoutingItem::default()
        }
    }

    fn app_config(active_id: &str) -> AppConfig {
        AppConfig {
            index_id: active_id.to_string(),
            core_basic_item: CoreBasicItem::default(),
            routing_basic_item: RoutingBasicItem::default(),
            tun_mode_item: TunModeItem::default(),
            ..AppConfig::default()
        }
    }

    fn vless_profile(index_id: &str, remarks: &str, address: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            remarks: remarks.to_string(),
            protocol: ProfileProtocol::Vless {
                server: ServerEndpoint {
                    address: address.to_string(),
                    port: 443,
                },
                uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                flow: Some(String::new()),
                encryption: Some("none".to_string()),
            },
            tls: Some(crate::TlsSettings {
                mode: TlsMode::Tls,
                server_name: None,
                alpn: Vec::new(),
                reality_public_key: None,
                reality_short_id: None,
                reality_spider_x: None,
                mldsa65_verify: None,
                certificate_pem: None,
                certificate_sha256: Vec::new(),
                ech_config: Vec::new(),
                final_mask: None,
            }),
            ..ProfileItem::default()
        }
    }

    fn group_profile(index_id: &str, remarks: &str, child_items: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            remarks: remarks.to_string(),
            protocol: ProfileProtocol::PolicyGroup {
                child_profile_ids: child_items.split(',').map(str::to_string).collect(),
                source_subscription_id: None,
                filter: None,
                strategy: crate::MultipleLoad::LeastPing,
            },
            ..ProfileItem::default()
        }
    }
}
