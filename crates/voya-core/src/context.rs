use std::{collections::BTreeMap, net::IpAddr};

use crate::{
    singbox::support::{singbox_supports_config_type, state_port2},
    validation::{ValidationCode, ValidationMessage, ValidationScope},
    AppConfig, ConfigType, CoreType, InboundProtocol, ProfileItem, ProfileProtocol, RoutingItem,
    RulesItem, ServerEndpoint, SimpleDnsItem, TlsMode,
};

pub const PROXY_TAG: &str = "proxy";
pub const DIRECT_TAG: &str = "direct";
pub const BLOCK_TAG: &str = "block";
pub const STREAM_SECURITY_TLS: &str = "tls";
pub const LOOPBACK: &str = "127.0.0.1";
pub const DEFAULT_NETWORK: &str = "raw";

const WS: &str = "ws";
const SHADOWSOCKS_RAW: &str = "raw";

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
}

/// How TUN is delivered on the host, injected as a platform fact.
///
/// ADR 0005 moved macOS to a single in-process NetworkExtension config, so the
/// topology can no longer be inferred from [`CoreGenPlatform`] alone: Windows
/// (service backend) and macOS (PacketTunnel provider) both carry TUN inside
/// the one generated config, while Linux still splits into a privileged TUN
/// process and an unprivileged SOCKS process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunTopology {
    /// One sing-box process owns both the TUN inbound and the remote outbounds.
    SingleProcess,
    /// A privileged TUN process forwards into an unprivileged SOCKS process.
    PreSocks,
}

impl TunTopology {
    /// Default topology for a platform when the caller injects nothing better.
    #[must_use]
    pub const fn for_platform(platform: CoreGenPlatform) -> Self {
        match platform {
            CoreGenPlatform::Windows | CoreGenPlatform::MacOS => Self::SingleProcess,
            CoreGenPlatform::Linux => Self::PreSocks,
        }
    }

    #[must_use]
    pub const fn is_pre_socks(self) -> bool {
        matches!(self, Self::PreSocks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeValidatorResult {
    pub errors: Vec<ValidationMessage>,
    pub warnings: Vec<ValidationMessage>,
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

    fn push_warning(&mut self, warning: impl Into<ValidationMessage>) {
        self.warnings.push(warning.into());
    }

    fn push_error(&mut self, error: impl Into<ValidationMessage>) {
        self.errors.push(error.into());
    }

    /// Adopt a child's findings, recording the hop that reached them.
    ///
    /// This replaced two `format!("{prefix}: {message}")` helpers: gluing a
    /// prefix onto a message is what made the composed text untranslatable,
    /// because neither half could be looked up any more.
    fn extend_scoped(&mut self, scope: &ValidationScope, result: &Self) {
        self.warnings.extend(
            result
                .warnings
                .iter()
                .map(|warning| warning.clone().within(scope.clone())),
        );
        self.errors.extend(
            result
                .errors
                .iter()
                .map(|error| error.clone().within(scope.clone())),
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
    /// Bearer token the generated `experimental.clash_api` requires.
    ///
    /// Injected by the orchestration layer (a per-launch random value) so
    /// `voya-core` stays deterministic; `None` emits no `secret`, which is what
    /// golden fixtures and previews use.
    pub clash_api_secret: Option<String>,
    /// The active policy group with its usable members in order; `node` is
    /// then the first member. `None` while a single node is active.
    pub policy_group: Option<ContextPolicyGroup>,
    /// Policy groups that routing rules send traffic to, each with its usable
    /// members in order. A rule naming the active group uses `proxy` instead.
    pub rule_policy_groups: Vec<ContextPolicyGroup>,
}

/// How a routing rule names a policy group as its outbound: this prefix and
/// the group id. Nodes are named by their remarks.
pub const GROUP_OUTBOUND_PREFIX: &str = "group:";

/// A policy group as the generator sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextPolicyGroup {
    pub group: crate::PolicyGroupItem,
    pub members: Vec<ProfileItem>,
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
            clash_api_secret: None,
            policy_group: None,
            rule_policy_groups: Vec::new(),
        }
    }
}

impl CoreConfigContext {
    /// Every node the main proxy can send traffic through: the group's members
    /// while a group is active, otherwise the single active node.
    #[must_use]
    pub fn active_outbound_nodes(&self) -> Vec<&ProfileItem> {
        self.policy_group
            .as_ref()
            .map_or_else(|| vec![&self.node], |group| group.members.iter().collect())
    }

    #[must_use]
    pub fn is_windows(&self) -> bool {
        self.platform.is_windows()
    }

    #[must_use]
    pub fn is_macos(&self) -> bool {
        self.platform.is_macos()
    }

    /// Port this context's `experimental.clash_api.external_controller` binds.
    ///
    /// The pre-socks split gives the TUN process `api2 + 1` and leaves the main
    /// process on `api2`, so callers must read the port back from the context
    /// they generated instead of re-deriving it from
    /// `tun_mode_item.enable_tun`, which disagrees on the Linux TUN path.
    #[must_use]
    pub fn clash_api_port(&self) -> i32 {
        state_port2(&self.app_config, self.is_tun_enabled)
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

pub trait CoreGenEnv {
    fn platform(&self) -> CoreGenPlatform;

    /// TUN process topology for this host.
    ///
    /// Overriding this is how a caller tells `voya-core` that TUN runs inside
    /// the single generated config (macOS PacketTunnel, Windows service)
    /// instead of the Linux pre-socks split; see [`TunTopology`].
    fn tun_topology(&self) -> TunTopology {
        TunTopology::for_platform(self.platform())
    }

    /// Per-launch bearer token for the generated Clash API, if the caller has
    /// one. Injected so config generation stays deterministic.
    fn get_clash_api_secret(&self) -> Option<String> {
        None
    }

    fn get_profile_by_remarks(&self, remarks: &str) -> Option<ProfileItem>;
    /// A policy group a routing rule names, with its members resolved against
    /// the current node list. `None` when no such group exists.
    fn get_policy_group(&self, _id: &str) -> Option<ContextPolicyGroup> {
        None
    }
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
            clash_api_secret: self.env.get_clash_api_secret(),
            policy_group: None,
            rule_policy_groups: Vec::new(),
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
        self.with_pre_socks(self.build(config, node))
    }

    /// [`Self::build_for_group`] plus the platform pre-socks config, added
    /// exactly as [`Self::build_all`] adds it for a single node.
    #[must_use]
    pub fn build_all_for_group(
        &self,
        config: &AppConfig,
        group: &crate::PolicyGroupItem,
        members: &[ProfileItem],
    ) -> CoreConfigContextBuilderAllResult {
        self.with_pre_socks(self.build_for_group(config, group, members))
    }

    /// Builds the context for an active policy group from its resolved members.
    ///
    /// A member that fails validation is left out and reported as a warning
    /// scoped to that member, so one broken node cannot keep the rest of the
    /// group offline. A group with no usable member is an error: sing-box
    /// rejects an empty group, and quietly using some other node would send
    /// traffic somewhere the user did not choose.
    #[must_use]
    pub fn build_for_group(
        &self,
        config: &AppConfig,
        group: &crate::PolicyGroupItem,
        members: &[ProfileItem],
    ) -> CoreConfigContextBuilderResult {
        let mut result = self.build(config, &ProfileItem::default());
        if !result.success() {
            return result;
        }
        let mut usable = Vec::new();
        for member in members {
            let scope = ValidationScope::PolicyGroupMember {
                group: group.name.clone(),
                member: member.remarks.clone(),
            };
            let member_result = register_single_node(&mut result.context, member);
            if member_result.success() {
                usable.push(member.clone());
                result
                    .validator_result
                    .extend_scoped(&scope, &member_result);
            } else {
                result.validator_result.warnings.extend(
                    member_result
                        .errors
                        .iter()
                        .chain(&member_result.warnings)
                        .map(|finding| finding.clone().within(scope.clone())),
                );
            }
        }
        let Some(first) = usable.first().cloned() else {
            result
                .validator_result
                .push_error(ValidationCode::PolicyGroupWithoutValidMembers {
                    group: group.name.clone(),
                });
            return result;
        };
        result.context.node = first;
        result.context.policy_group = Some(ContextPolicyGroup {
            group: group.clone(),
            members: usable,
        });
        result
    }

    fn with_pre_socks(
        &self,
        main_result: CoreConfigContextBuilderResult,
    ) -> CoreConfigContextBuilderAllResult {
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
        let pre_socks_item = pre_socks_item(config, self.env)?;
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

        let register_result = register_single_node(context, node);
        (node.clone(), register_result)
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
            validator_result.push_warning(ValidationCode::RoutingRuleWithoutOutbound {
                rule: rule_name.to_string(),
            });
            return;
        };

        if let Some(group_id) = outbound_tag.strip_prefix(GROUP_OUTBOUND_PREFIX) {
            self.resolve_rule_group(context, validator_result, rule_name, outbound_tag, group_id);
            return;
        }

        // A rule that names an outbound node must not fall back to the active
        // node: routing.rs would silently retarget it to the main proxy.
        let Some(rule_outbound_node) = self.env.get_profile_by_remarks(outbound_tag) else {
            validator_result.push_error(ValidationCode::RoutingRuleOutboundNotFound {
                rule: rule_name.to_string(),
                outbound: outbound_tag.to_string(),
            });
            return;
        };

        let (active_rule_node, rule_result) = self.resolve_node(context, &rule_outbound_node);
        let scope = ValidationScope::RoutingRuleOutbound {
            rule: rule_name.to_string(),
            outbound: outbound_tag.to_string(),
        };
        validator_result.extend_scoped(&scope, &rule_result);
        if !rule_result.success() {
            return;
        }

        context
            .all_proxies_map
            .insert(format!("remark:{outbound_tag}"), active_rule_node);
    }

    /// A rule that sends traffic through a policy group. Members the generator
    /// cannot use are left out with a member-scoped warning, exactly as for the
    /// active group; a group that is gone or has no usable member is an error,
    /// because falling back to the main proxy would route somewhere the user
    /// did not choose.
    fn resolve_rule_group(
        &self,
        context: &mut CoreConfigContext,
        validator_result: &mut NodeValidatorResult,
        rule_name: &str,
        outbound_tag: &str,
        group_id: &str,
    ) {
        if context
            .rule_policy_groups
            .iter()
            .any(|group| group.group.id == group_id)
        {
            return;
        }
        let Some(group) = self.env.get_policy_group(group_id) else {
            validator_result.push_error(ValidationCode::RoutingRuleOutboundNotFound {
                rule: rule_name.to_string(),
                outbound: outbound_tag.to_string(),
            });
            return;
        };
        let mut usable = Vec::new();
        for member in &group.members {
            let scope = ValidationScope::PolicyGroupMember {
                group: group.group.name.clone(),
                member: member.remarks.clone(),
            };
            let member_result = register_single_node(context, member);
            if member_result.success() {
                usable.push(member.clone());
                validator_result.extend_scoped(&scope, &member_result);
            } else {
                validator_result.warnings.extend(
                    member_result
                        .errors
                        .iter()
                        .chain(&member_result.warnings)
                        .map(|finding| finding.clone().within(scope.clone())),
                );
            }
        }
        if usable.is_empty() {
            validator_result.push_error(ValidationCode::PolicyGroupWithoutValidMembers {
                group: group.group.name.clone(),
            });
            return;
        }
        context.rule_policy_groups.push(ContextPolicyGroup {
            group: group.group,
            members: usable,
        });
    }
}

mod validation;
use validation::*;
pub use validation::{is_domain, validate_node};

fn pre_socks_item<E: CoreGenEnv>(config: &AppConfig, env: &E) -> Option<ProfileItem> {
    // The topology is an injected platform fact, not something derived from
    // `CoreGenPlatform`: macOS runs TUN inside the single NetworkExtension
    // config (ADR 0005), so `build_all` must not synthesize a pre-socks context
    // there even though macOS is "non-Windows".
    if config.tun_mode_item.enable_tun && env.tun_topology().is_pre_socks() {
        return Some(socks_profile(env.get_local_port(InboundProtocol::socks)));
    }

    None
}

fn register_single_node(
    context: &mut CoreConfigContext,
    node: &ProfileItem,
) -> NodeValidatorResult {
    let result = validate_node(node, context.run_core_type);
    if !result.success() {
        return result;
    }

    context
        .all_proxies_map
        .insert(node.index_id.clone(), node.clone());

    push_domain_if_needed(&mut context.protect_domain_list, node.address());

    if let Some(tls) = &node.tls {
        if !tls.ech_config.is_empty() {
            let server_name = tls.server_name.as_deref().unwrap_or_default();
            let ech_query_sni = if tls.mode == TlsMode::Tls
                && tls.ech_config.iter().any(|value| value.contains("://"))
            {
                tls.ech_config
                    .iter()
                    .find(|value| !value.contains("://"))
                    .map(|value| value.split_once('+').map_or(value.as_str(), |(sni, _)| sni))
                    .unwrap_or(server_name)
            } else {
                server_name
            };
            push_domain_if_needed(&mut context.protect_domain_list, ech_query_sni);
        }
    }

    result
}

fn socks_profile(port: i32) -> ProfileItem {
    ProfileItem {
        protocol: ProfileProtocol::Socks {
            server: ServerEndpoint {
                address: LOOPBACK.to_string(),
                port,
            },
            username: String::new(),
            password: String::new(),
        },
        ..ProfileItem::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoreBasicItem, RoutingBasicItem, RulesItem, TunModeItem};

    #[derive(Debug, Clone)]
    struct MemoryEnv {
        platform: CoreGenPlatform,
        profiles: Vec<ProfileItem>,
        routings: Vec<RoutingItem>,
        local_socks_port: i32,
    }

    impl Default for MemoryEnv {
        fn default() -> Self {
            Self {
                platform: CoreGenPlatform::Linux,
                profiles: Vec::new(),
                routings: Vec::new(),
                local_socks_port: 10808,
            }
        }
    }

    impl CoreGenEnv for MemoryEnv {
        fn platform(&self) -> CoreGenPlatform {
            self.platform
        }

        fn get_profile_by_remarks(&self, remarks: &str) -> Option<ProfileItem> {
            self.profiles
                .iter()
                .find(|profile| profile.remarks == remarks)
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
    fn context_build_resolves_structured_dns_routing_for_the_supplied_node() {
        let active = vless_profile("active", "Active", "active.example.com");
        let config = app_config("active");
        let env = MemoryEnv {
            profiles: vec![active.clone()],
            routings: vec![RoutingItem {
                id: "routing".to_string(),
                ..RoutingItem::default()
            }],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&config, &active);

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
    fn context_unresolvable_rule_outbound_is_a_validation_error() {
        let active = vless_profile("active", "Active", "active.example.com");
        let env = MemoryEnv {
            profiles: vec![active.clone()],
            routings: vec![routing_with_outbound_tag("RenamedNode")],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build(&app_config("active"), &active);

        assert!(!result.success());
        assert!(result.validator_result.errors.iter().any(|error| error.code
            == ValidationCode::RoutingRuleOutboundNotFound {
                rule: "route through node".to_string(),
                outbound: "RenamedNode".to_string(),
            }));
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
            .any(|error| error.scope
                == vec![ValidationScope::RoutingRuleOutbound {
                    rule: "route through node".to_string(),
                    outbound: "BrokenNode".to_string(),
                }]));
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
            certificate_pem: None,
            ech_config: vec![
                "ech-query.example.com".to_string(),
                "https://dns.example/dns-query".to_string(),
            ],
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
    fn context_build_all_keeps_single_process_tun_on_macos() {
        // ADR 0005 runs macOS TUN inside the NetworkExtension config, so
        // `build_all` must not synthesize a pre-socks context there. Before the
        // topology became an injected fact this only worked because voya-app
        // bypassed `build_all` entirely.
        let active = vless_profile("active", "Active", "active.example.com");
        let mut config = app_config("active");
        config.tun_mode_item.enable_tun = true;
        let env = MemoryEnv {
            platform: CoreGenPlatform::MacOS,
            profiles: vec![active.clone()],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build_all(&config, &active);

        assert!(result.success());
        assert!(result.pre_socks_result.is_none());
        assert!(result.main_result.context.is_tun_enabled);
    }

    #[test]
    fn context_clash_api_port_splits_between_main_and_pre_socks_on_linux() {
        let active = vless_profile("active", "Active", "active.example.com");
        let mut config = app_config("active");
        config.tun_mode_item.enable_tun = true;
        let env = MemoryEnv {
            platform: CoreGenPlatform::Linux,
            profiles: vec![active.clone()],
            ..MemoryEnv::default()
        };

        let result = CoreConfigContextBuilder::new(&env).build_all(&config, &active);

        let main_port = result.main_result.context.clash_api_port();
        let pre_port = result
            .pre_socks_result
            .as_ref()
            .expect("pre socks context")
            .context
            .clash_api_port();
        assert_eq!(pre_port, main_port + 1);
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
                certificate_pem: None,
                ech_config: Vec::new(),
            }),
            ..ProfileItem::default()
        }
    }
}
