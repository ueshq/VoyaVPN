use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    validate_xhttp_extra, AppConfig, ConfigType, CoreType, DnsItem, FullConfigTemplateItem,
    InboundProtocol, ProfileItem, ProtocolExtraItem, RoutingItem, RulesItem, SimpleDnsItem,
    SubItem,
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
const SS_SECURITIES_IN_SINGBOX: &[&str] = &[
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CoreConfigContext {
    pub node: ProfileItem,
    pub run_core_type: CoreType,
    pub routing_item: Option<RoutingItem>,
    pub raw_dns_item: Option<DnsItem>,
    pub simple_dns_item: SimpleDnsItem,
    pub all_proxies_map: BTreeMap<String, ProfileItem>,
    pub app_config: AppConfig,
    pub full_config_template: Option<FullConfigTemplateItem>,
    pub server_test_item_map: BTreeMap<String, String>,
    pub is_tun_enabled: bool,
    pub protect_domain_list: Vec<String>,
    pub platform: CoreGenPlatform,
    pub singbox_ruleset_paths: BTreeMap<String, String>,
}

impl Default for CoreConfigContext {
    fn default() -> Self {
        Self {
            node: ProfileItem::default(),
            run_core_type: CoreType::Xray,
            routing_item: None,
            raw_dns_item: None,
            simple_dns_item: SimpleDnsItem::default(),
            all_proxies_map: BTreeMap::new(),
            app_config: AppConfig::default(),
            full_config_template: None,
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
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

    fn get_core_type(&self, profile: &ProfileItem, config_type: ConfigType) -> CoreType;

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

    fn get_profile_items_by_subid(&self, subid: &str) -> Vec<ProfileItem>;

    fn get_sub_item(&self, subid: &str) -> Option<SubItem>;

    fn get_full_config_template_item(&self, core_type: CoreType) -> Option<FullConfigTemplateItem>;

    fn get_dns_item(&self, core_type: CoreType) -> Option<DnsItem>;

    fn get_default_routing(&self, config: &AppConfig) -> Option<RoutingItem>;

    fn get_local_port(&self, protocol: InboundProtocol) -> i32;

    fn get_singbox_ruleset_paths(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn next_virtual_chain_id(&self, node: &ProfileItem, child_index_ids: &[String]) -> String;
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
        let run_core_type = self.env.get_core_type(node, node.config_type);
        let core_type = generator_core_type(run_core_type);
        let mut context = CoreConfigContext {
            node: node.clone(),
            run_core_type,
            routing_item: self.env.get_default_routing(config),
            raw_dns_item: self.env.get_dns_item(core_type),
            simple_dns_item: config.simple_dns_item.clone(),
            all_proxies_map: BTreeMap::new(),
            app_config: config.clone(),
            full_config_template: self.env.get_full_config_template_item(core_type),
            server_test_item_map: BTreeMap::new(),
            is_tun_enabled: config.tun_mode_item.enable_tun,
            protect_domain_list: Vec::new(),
            platform: self.env.platform(),
            singbox_ruleset_paths: self.env.get_singbox_ruleset_paths(),
        };

        let (active_node, node_result) = self.resolve_node(&mut context, node, true);
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
        let core_type = self.env.get_core_type(node, node.config_type);
        let pre_socks_item = pre_socks_item(config, node, core_type, self.env)?;
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
        include_sub_chain: bool,
    ) -> (ProfileItem, NodeValidatorResult) {
        if node.index_id.trim().is_empty() {
            return (node.clone(), NodeValidatorResult::empty());
        }

        if include_sub_chain {
            let (virtual_chain_node, chain_result) = self.build_subscription_chain_node(node);
            if let Some(virtual_chain_node) = virtual_chain_node {
                context.all_proxies_map.insert(
                    virtual_chain_node.index_id.clone(),
                    virtual_chain_node.clone(),
                );
                let (resolved_node, mut resolved_result) =
                    self.resolve_node(context, &virtual_chain_node, false);
                prepend_warnings(&mut resolved_result, &chain_result.warnings);
                return (resolved_node, resolved_result);
            }

            if !chain_result.warnings.is_empty() {
                let mut fill_result = self.register_node(context, node);
                prepend_warnings(&mut fill_result, &chain_result.warnings);
                return (node.clone(), fill_result);
            }
        }

        let register_result = self.register_node(context, node);
        (node.clone(), register_result)
    }

    fn build_subscription_chain_node(
        &self,
        node: &ProfileItem,
    ) -> (Option<ProfileItem>, NodeValidatorResult) {
        let mut result = NodeValidatorResult::empty();
        if node.subid.trim().is_empty() || node.config_type == ConfigType::Custom {
            return (None, result);
        }

        let Some(sub_item) = self.env.get_sub_item(&node.subid) else {
            return (None, result);
        };

        let prev_node = sub_item
            .prev_profile
            .as_deref()
            .and_then(nonempty)
            .and_then(|remark| match self.env.get_profile_by_remarks(remark) {
                Some(profile) => Some(profile),
                None => {
                    result.push_warning(format!("subscription prev profile not found: {remark}"));
                    None
                }
            });
        let next_node = sub_item
            .next_profile
            .as_deref()
            .and_then(nonempty)
            .and_then(|remark| match self.env.get_profile_by_remarks(remark) {
                Some(profile) => Some(profile),
                None => {
                    result.push_warning(format!("subscription next profile not found: {remark}"));
                    None
                }
            });

        if prev_node.is_none() && next_node.is_none() {
            return (None, result);
        }

        let child_items = [prev_node.as_ref(), Some(node), next_node.as_ref()]
            .into_iter()
            .flatten()
            .map(|profile| profile.index_id.clone())
            .filter(|index_id| !index_id.trim().is_empty())
            .collect::<Vec<_>>();

        let chain_node = ProfileItem {
            index_id: self.env.next_virtual_chain_id(node, &child_items),
            config_type: ConfigType::ProxyChain,
            core_type: Some(self.env.get_core_type(node, node.config_type)),
            remarks: node.remarks.clone(),
            protocol_extra: ProtocolExtraItem {
                group_type: Some("ProxyChain".to_string()),
                child_items: Some(list_to_string(&child_items)),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        };

        (Some(chain_node), result)
    }

    fn register_node(
        &self,
        context: &mut CoreConfigContext,
        node: &ProfileItem,
    ) -> NodeValidatorResult {
        if node.config_type.is_group_type() {
            return self.register_group_node(context, node);
        }

        register_single_node(context, node)
    }

    fn register_group_node(
        &self,
        context: &mut CoreConfigContext,
        node: &ProfileItem,
    ) -> NodeValidatorResult {
        if !node.config_type.is_group_type() {
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
        let group_child_list = self.group_child_profile_items(&node.protocol_extra);
        let mut child_index_ids = Vec::new();
        let mut child_index_seen = BTreeSet::new();
        let mut child_result = NodeValidatorResult::empty();

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

            if !child_node.config_type.is_group_type() {
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
        resolved_node.protocol_extra.child_items = Some(list_to_string(&child_index_ids));
        context
            .all_proxies_map
            .insert(resolved_node.index_id.clone(), resolved_node);
        child_result
    }

    fn group_child_profile_items(&self, protocol_extra: &ProtocolExtraItem) -> Vec<ProfileItem> {
        let mut items = Vec::new();
        items.extend(self.sub_child_profile_items(protocol_extra));
        items.extend(self.selected_child_profile_items(protocol_extra));
        items
    }

    fn selected_child_profile_items(&self, protocol_extra: &ProtocolExtraItem) -> Vec<ProfileItem> {
        let child_ids = string_to_list(protocol_extra.child_items.as_deref());
        if child_ids.is_empty() {
            return Vec::new();
        }
        self.env.get_profile_items_ordered_by_index_ids(&child_ids)
    }

    fn sub_child_profile_items(&self, protocol_extra: &ProtocolExtraItem) -> Vec<ProfileItem> {
        let Some(subid) = protocol_extra.sub_child_items.as_deref().and_then(nonempty) else {
            return Vec::new();
        };
        let filter = protocol_extra
            .filter
            .as_deref()
            .and_then(nonempty)
            .and_then(|value| Regex::new(value).ok());

        self.env
            .get_profile_items_by_subid(subid)
            .into_iter()
            .filter(|profile| {
                !profile.config_type.is_complex_type()
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

        let Some(rule_outbound_node) = self.env.get_profile_by_remarks(outbound_tag) else {
            validator_result.push_warning(format!(
                "routing rule {rule_name} outbound node not found: {outbound_tag}"
            ));
            return;
        };

        let (active_rule_node, rule_result) =
            self.resolve_node(context, &rule_outbound_node, false);
        validator_result
            .warnings
            .extend(rule_result.warnings.iter().map(|warning| {
                format!("routing rule {rule_name} outbound {outbound_tag} warning: {warning}")
            }));

        if !rule_result.success() {
            validator_result
                .warnings
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

fn generator_core_type(run_core_type: CoreType) -> CoreType {
    if run_core_type == CoreType::sing_box {
        CoreType::sing_box
    } else {
        CoreType::Xray
    }
}

fn pre_socks_item<E: CoreGenEnv>(
    config: &AppConfig,
    node: &ProfileItem,
    core_type: CoreType,
    env: &E,
) -> Option<ProfileItem> {
    let enable_legacy_protect =
        config.tun_mode_item.enable_legacy_protect || env.platform().is_non_windows();

    if node.config_type != ConfigType::Custom
        && core_type != CoreType::sing_box
        && config.tun_mode_item.enable_tun
        && enable_legacy_protect
    {
        return Some(ProfileItem {
            core_type: Some(CoreType::sing_box),
            config_type: ConfigType::SOCKS,
            address: LOOPBACK.to_string(),
            port: env.get_local_port(InboundProtocol::socks),
            ..ProfileItem::default()
        });
    }

    if node.config_type == ConfigType::Custom && matches!(node.pre_socks_port, Some(1..=65535)) {
        return Some(ProfileItem {
            core_type: Some(if config.tun_mode_item.enable_tun {
                CoreType::sing_box
            } else {
                CoreType::Xray
            }),
            config_type: ConfigType::SOCKS,
            address: LOOPBACK.to_string(),
            port: node.pre_socks_port.unwrap_or_default(),
            ..ProfileItem::default()
        });
    }

    None
}

fn register_single_node(
    context: &mut CoreConfigContext,
    node: &ProfileItem,
) -> NodeValidatorResult {
    if node.config_type.is_group_type() {
        return NodeValidatorResult::empty();
    }

    let result = validate_node(node, context.run_core_type);
    if !result.success() {
        return result;
    }

    context
        .all_proxies_map
        .insert(node.index_id.clone(), node.clone());

    push_domain_if_needed(&mut context.protect_domain_list, &node.address);

    if !node.ech_config_list.trim().is_empty() {
        let ech_query_sni = if node.stream_security == STREAM_SECURITY_TLS
            && node.ech_config_list.contains("://")
        {
            node.ech_config_list
                .split_once('+')
                .map_or(node.sni.as_str(), |(sni, _)| sni)
        } else {
            node.sni.as_str()
        };
        push_domain_if_needed(&mut context.protect_domain_list, ech_query_sni);
    }

    if let Some(download_address) = xhttp_download_settings_address(node) {
        push_domain_if_needed(&mut context.protect_domain_list, &download_address);
    }

    result
}

#[must_use]
pub fn validate_node(item: &ProfileItem, core_type: CoreType) -> NodeValidatorResult {
    let mut result = NodeValidatorResult::empty();

    if item.config_type == ConfigType::Custom || item.config_type.is_group_type() {
        return result;
    }

    if item.address.trim().is_empty() {
        result.push_error("invalid Address");
    }
    if !(1..=65535).contains(&item.port) {
        result.push_error("invalid Port");
    }

    let network = get_network(item);
    if core_type == CoreType::sing_box {
        if SINGBOX_UNSUPPORTED_TRANSPORTS.contains(&network.as_str()) {
            result.push_error(format!("sing_box does not support network {network}"));
        }
        if !singbox_supports_config_type(item.config_type) {
            result.push_error(format!(
                "sing_box does not support protocol {:?}",
                item.config_type
            ));
        }
        if !singbox_transport_supported_protocol(item.config_type) && network != DEFAULT_NETWORK {
            result.push_error(format!(
                "sing_box does not support protocol {:?} with network {network}",
                item.config_type
            ));
        }
        if item.config_type == ConfigType::Shadowsocks
            && !SINGBOX_SHADOWSOCKS_ALLOWED_TRANSPORTS.contains(&network.as_str())
        {
            result.push_error(format!(
                "sing_box does not support Shadowsocks with network {network}"
            ));
        }
    } else if core_type == CoreType::Xray && !xray_supports_config_type(item.config_type) {
        result.push_error(format!(
            "Xray does not support protocol {:?}",
            item.config_type
        ));
    }

    match item.config_type {
        ConfigType::VMess => {
            if item.password.trim().is_empty() || !is_guid_like(&item.password) {
                result.push_error("invalid Password");
            }
        }
        ConfigType::VLESS => {
            if item.password.trim().is_empty()
                || (!is_guid_like(&item.password) && item.password.chars().count() > 30)
            {
                result.push_error("invalid Password");
            }
            if !FLOWS.contains(
                &item
                    .protocol_extra
                    .flow
                    .as_deref()
                    .unwrap_or_default()
                    .trim(),
            ) {
                result.push_error("invalid Flow");
            }
        }
        ConfigType::Shadowsocks => {
            if item.password.trim().is_empty() {
                result.push_error("invalid Password");
            }
            if !SS_SECURITIES_IN_SINGBOX.contains(
                &item
                    .protocol_extra
                    .ss_method
                    .as_deref()
                    .unwrap_or_default()
                    .trim(),
            ) {
                result.push_error("invalid SsMethod");
            }
        }
        _ => {}
    }

    if item.stream_security == "reality" && item.public_key.trim().is_empty() {
        result.push_error("invalid PublicKey");
    }

    if item.network == XHTTP
        && item
            .transport_extra
            .xhttp_extra
            .as_deref()
            .and_then(nonempty)
            .is_some_and(|extra| validate_xhttp_extra(extra).is_err())
    {
        result.push_error("invalid XHTTP Extra");
    }

    if !item.finalmask.trim().is_empty()
        && serde_json::from_str::<Value>(&item.finalmask).map_or(true, |value| !value.is_object())
    {
        result.push_error("invalid Finalmask");
    }

    result
}

fn profile_is_valid(item: &ProfileItem) -> bool {
    validate_node(item, CoreType::Xray).success()
        || validate_node(item, CoreType::sing_box).success()
}

fn get_network(item: &ProfileItem) -> String {
    let network = item.network.trim();
    if network.is_empty() {
        DEFAULT_NETWORK.to_string()
    } else {
        network.to_string()
    }
}

fn xray_supports_config_type(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::VMess
            | ConfigType::VLESS
            | ConfigType::Shadowsocks
            | ConfigType::Trojan
            | ConfigType::Hysteria2
            | ConfigType::WireGuard
            | ConfigType::SOCKS
            | ConfigType::HTTP
    )
}

fn singbox_supports_config_type(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::VMess
            | ConfigType::VLESS
            | ConfigType::Shadowsocks
            | ConfigType::Trojan
            | ConfigType::Hysteria2
            | ConfigType::TUIC
            | ConfigType::Anytls
            | ConfigType::Naive
            | ConfigType::WireGuard
            | ConfigType::SOCKS
            | ConfigType::HTTP
    )
}

fn singbox_transport_supported_protocol(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::VMess | ConfigType::VLESS | ConfigType::Trojan | ConfigType::Shadowsocks
    )
}

fn is_guid_like(value: &str) -> bool {
    let value = value
        .trim()
        .trim_start_matches(['{', '('])
        .trim_end_matches(['}', ')']);
    if value.len() == 32 {
        return value.chars().all(|ch| ch.is_ascii_hexdigit());
    }
    let expected = [8, 4, 4, 4, 12];
    let chunks = value.split('-').collect::<Vec<_>>();
    chunks.len() == expected.len()
        && chunks.iter().zip(expected).all(|(chunk, len)| {
            chunk.len() == len && chunk.chars().all(|ch| ch.is_ascii_hexdigit())
        })
}

fn is_builtin_outbound(outbound_tag: Option<&str>) -> bool {
    outbound_tag.is_some_and(|tag| matches!(tag, PROXY_TAG | DIRECT_TAG | BLOCK_TAG))
}

fn xhttp_download_settings_address(node: &ProfileItem) -> Option<String> {
    let extra = node.transport_extra.xhttp_extra.as_deref()?.trim();
    if extra.is_empty() {
        return None;
    }
    let value = serde_json::from_str::<Value>(extra).ok()?;
    value
        .get("downloadSettings")
        .and_then(|settings| settings.get("address"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|address| !address.is_empty())
        .map(str::to_string)
}

fn push_domain_if_needed(protect_domain_list: &mut Vec<String>, candidate: &str) {
    let candidate = candidate.trim();
    if is_domain(candidate) && !protect_domain_list.iter().any(|domain| domain == candidate) {
        protect_domain_list.push(candidate.to_string());
    }
}

fn merge_protect_domains(target: &mut Vec<String>, source: &[String]) {
    for domain in source {
        push_domain_if_needed(target, domain);
    }
}

#[must_use]
pub fn is_domain(candidate: &str) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty()
        || candidate.contains("://")
        || candidate.contains('/')
        || candidate.contains('\\')
        || candidate.parse::<IpAddr>().is_ok()
    {
        return false;
    }

    let blocked_ext = [
        "json", "txt", "xml", "cfg", "ini", "log", "yaml", "yml", "toml",
    ];
    if candidate
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .is_some_and(|extension| blocked_ext.contains(&extension.to_ascii_lowercase().as_str()))
    {
        return false;
    }

    candidate.split('.').all(|label| {
        !label.is_empty()
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    }) && candidate.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn push_unique_child_index(
    child_index_ids: &mut Vec<String>,
    child_index_seen: &mut BTreeSet<String>,
    child_index_id: &str,
) {
    if child_index_seen.insert(child_index_id.to_string()) {
        child_index_ids.push(child_index_id.to_string());
    }
}

fn string_to_list(value: Option<&str>) -> Vec<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace(['\r', '\n'], ""))
        .map(|value| {
            value
                .split(',')
                .filter_map(nonempty)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn list_to_string(values: &[String]) -> String {
    values.join(",")
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn prepend_warnings(result: &mut NodeValidatorResult, warnings: &[String]) {
    if warnings.is_empty() {
        return;
    }
    let mut merged = warnings.to_vec();
    merged.append(&mut result.warnings);
    result.warnings = merged;
}

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
        dns_items: Vec<DnsItem>,
        templates: Vec<FullConfigTemplateItem>,
        local_socks_port: i32,
    }

    impl Default for MemoryEnv {
        fn default() -> Self {
            Self {
                platform: CoreGenPlatform::Linux,
                profiles: Vec::new(),
                subs: Vec::new(),
                routings: Vec::new(),
                dns_items: Vec::new(),
                templates: Vec::new(),
                local_socks_port: 10808,
            }
        }
    }

    impl CoreGenEnv for MemoryEnv {
        fn platform(&self) -> CoreGenPlatform {
            self.platform
        }

        fn get_core_type(&self, profile: &ProfileItem, config_type: ConfigType) -> CoreType {
            profile.core_type.unwrap_or(match config_type {
                ConfigType::TUIC | ConfigType::Anytls | ConfigType::Naive => CoreType::sing_box,
                _ => CoreType::Xray,
            })
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

        fn get_profile_items_by_subid(&self, subid: &str) -> Vec<ProfileItem> {
            self.profiles
                .iter()
                .filter(|profile| profile.subid == subid)
                .cloned()
                .collect()
        }

        fn get_sub_item(&self, subid: &str) -> Option<SubItem> {
            self.subs.iter().find(|sub| sub.id == subid).cloned()
        }

        fn get_full_config_template_item(
            &self,
            core_type: CoreType,
        ) -> Option<FullConfigTemplateItem> {
            self.templates
                .iter()
                .find(|template| template.core_type == core_type)
                .cloned()
        }

        fn get_dns_item(&self, core_type: CoreType) -> Option<DnsItem> {
            self.dns_items
                .iter()
                .find(|dns| dns.core_type == core_type)
                .cloned()
        }

        fn get_default_routing(&self, config: &AppConfig) -> Option<RoutingItem> {
            self.routings
                .iter()
                .find(|routing| {
                    routing.is_active || routing.id == config.routing_basic_item.routing_index_id
                })
                .or_else(|| self.routings.first())
                .cloned()
        }

        fn get_local_port(&self, protocol: InboundProtocol) -> i32 {
            match protocol {
                InboundProtocol::socks => self.local_socks_port,
                _ => self.local_socks_port + protocol.as_i32(),
            }
        }

        fn next_virtual_chain_id(&self, node: &ProfileItem, child_index_ids: &[String]) -> String {
            format!("inner-{}-{}", node.index_id, child_index_ids.join("-"))
        }
    }

    #[test]
    fn context_build_active_resolves_template_dns_routing_and_active_node() {
        let active = ProfileItem {
            core_type: Some(CoreType::sing_box),
            ..vless_profile("active", "Active", "active.example.com")
        };
        let config = app_config("active");
        let env = MemoryEnv {
            profiles: vec![active],
            routings: vec![RoutingItem {
                id: "routing".to_string(),
                is_active: true,
                ..RoutingItem::default()
            }],
            dns_items: vec![DnsItem {
                id: "dns".to_string(),
                core_type: CoreType::sing_box,
                ..DnsItem::default()
            }],
            templates: vec![FullConfigTemplateItem {
                id: "template".to_string(),
                core_type: CoreType::sing_box,
                enabled: true,
                ..FullConfigTemplateItem::default()
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
        assert_eq!(
            result.context.raw_dns_item.as_ref().map(|item| &item.id),
            Some(&"dns".to_string())
        );
        assert_eq!(
            result
                .context
                .full_config_template
                .as_ref()
                .map(|item| &item.id),
            Some(&"template".to_string())
        );
    }

    #[test]
    fn context_builds_subscription_virtual_proxy_chain_deterministically() {
        let prev = vless_profile("prev", "Prev", "prev.example.com");
        let mut active = vless_profile("active", "Active", "active.example.com");
        active.subid = "sub".to_string();
        let next = vless_profile("next", "Next", "next.example.com");
        let env = MemoryEnv {
            profiles: vec![prev, active.clone(), next],
            subs: vec![SubItem {
                id: "sub".to_string(),
                prev_profile: Some("Prev".to_string()),
                next_profile: Some("Next".to_string()),
                ..SubItem::default()
            }],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&app_config("active"), &active);

        assert!(result.success());
        assert_eq!(result.context.node.config_type, ConfigType::ProxyChain);
        assert_eq!(
            result.context.node.index_id,
            "inner-active-prev-active-next"
        );
        assert_eq!(
            result.context.node.protocol_extra.child_items.as_deref(),
            Some("prev,active,next")
        );
        assert!(result
            .context
            .all_proxies_map
            .contains_key("inner-active-prev-active-next"));
    }

    #[test]
    fn context_registers_rule_outbounds_by_remark() {
        let active = vless_profile("active", "Active", "active.example.com");
        let rule_node = vless_profile("rule", "RuleNode", "rule.example.com");
        let env = MemoryEnv {
            profiles: vec![active.clone(), rule_node],
            routings: vec![RoutingItem {
                id: "routing".to_string(),
                is_active: true,
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
        root.protocol_extra.sub_child_items = Some("sub".to_string());
        root.protocol_extra.filter = Some("^Sub".to_string());
        let sub_leaf = ProfileItem {
            subid: "sub".to_string(),
            ..vless_profile("sub-leaf", "Sub Leaf", "sub.example.com")
        };
        let ignored_sub_leaf = ProfileItem {
            subid: "sub".to_string(),
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
                .and_then(|profile| profile.protocol_extra.child_items.as_deref()),
            Some("sub-leaf,leaf,nested")
        );
        assert!(result
            .validator_result
            .warnings
            .iter()
            .any(|warning| warning.contains("cycle dependency")));
    }

    #[test]
    fn context_protect_domains_include_address_ech_sni_and_xhttp_download_address() {
        let mut active = vless_profile("active", "Active", "node.example.com");
        active.stream_security = STREAM_SECURITY_TLS.to_string();
        active.sni = "fallback.example.com".to_string();
        active.ech_config_list = "ech-query.example.com+https://dns.example/dns-query".to_string();
        active.network = XHTTP.to_string();
        active.transport_extra.xhttp_extra =
            Some(r#"{"downloadSettings":{"address":"download.example.com"}}"#.to_string());
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
                "ech-query.example.com".to_string(),
                "download.example.com".to_string()
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
        assert_eq!(pre_context.node.config_type, ConfigType::SOCKS);
        assert_eq!(pre_context.node.core_type, Some(CoreType::sing_box));
        assert_eq!(pre_context.node.address, LOOPBACK);
        assert_eq!(pre_context.node.port, 20808);
    }

    #[test]
    fn context_custom_pre_socks_uses_configured_port_without_tun() {
        let active = ProfileItem {
            index_id: "custom".to_string(),
            config_type: ConfigType::Custom,
            pre_socks_port: Some(18888),
            remarks: "Custom".to_string(),
            ..ProfileItem::default()
        };
        let env = MemoryEnv {
            profiles: vec![active.clone()],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build_all(&app_config("custom"), &active);

        let pre_context = &result
            .pre_socks_result
            .as_ref()
            .expect("custom pre socks context")
            .context;
        assert_eq!(pre_context.node.config_type, ConfigType::SOCKS);
        assert_eq!(pre_context.node.core_type, Some(CoreType::Xray));
        assert_eq!(pre_context.node.port, 18888);
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
            config_type: ConfigType::VLESS,
            core_type: Some(CoreType::Xray),
            remarks: remarks.to_string(),
            address: address.to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            stream_security: STREAM_SECURITY_TLS.to_string(),
            protocol_extra: ProtocolExtraItem {
                flow: Some(String::new()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        }
    }

    fn group_profile(index_id: &str, remarks: &str, child_items: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::PolicyGroup,
            remarks: remarks.to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some(child_items.to_string()),
                group_type: Some("PolicyGroup".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        }
    }
}
