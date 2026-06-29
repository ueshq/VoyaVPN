use std::collections::BTreeMap;

use voya_core::{
    AppConfig, ConfigType, CoreGenEnv, CoreGenPlatform, CoreType, DnsItem, FullConfigTemplateItem,
    InboundProtocol, ProfileItem, RoutingItem, SubItem,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum CoreTypeFallback {
    ConfigDefaults,
    Fixed(CoreType),
}

impl CoreTypeFallback {
    const fn resolve(self, config_type: ConfigType) -> CoreType {
        match self {
            Self::ConfigDefaults => default_core_type(config_type),
            Self::Fixed(core_type) => core_type,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SnapshotCoreGenEnv {
    core_type_items: Vec<(ConfigType, CoreType)>,
    local_socks_port: i32,
    platform: CoreGenPlatform,
    profiles: Vec<ProfileItem>,
    routings: Vec<RoutingItem>,
    dns_items: Vec<DnsItem>,
    full_config_templates: Vec<FullConfigTemplateItem>,
    subs: Vec<SubItem>,
    singbox_ruleset_paths: BTreeMap<String, String>,
    fallback: CoreTypeFallback,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SnapshotCoreGenData {
    pub(crate) profiles: Vec<ProfileItem>,
    pub(crate) routings: Vec<RoutingItem>,
    pub(crate) dns_items: Vec<DnsItem>,
    pub(crate) full_config_templates: Vec<FullConfigTemplateItem>,
    pub(crate) subs: Vec<SubItem>,
}

impl SnapshotCoreGenEnv {
    pub(crate) fn new(
        config: &AppConfig,
        platform: CoreGenPlatform,
        fallback: CoreTypeFallback,
        data: SnapshotCoreGenData,
    ) -> Self {
        Self {
            core_type_items: config
                .core_type_item
                .iter()
                .map(|item| (item.config_type, item.core_type))
                .collect(),
            local_socks_port: config
                .inbound
                .first()
                .map_or(voya_core::DEFAULT_LOCAL_PORT, |inbound| inbound.local_port),
            platform,
            profiles: data.profiles,
            routings: data.routings,
            dns_items: data.dns_items,
            full_config_templates: data.full_config_templates,
            subs: data.subs,
            singbox_ruleset_paths: BTreeMap::new(),
            fallback,
        }
    }

    pub(crate) fn with_singbox_ruleset_paths(
        mut self,
        singbox_ruleset_paths: BTreeMap<String, String>,
    ) -> Self {
        self.singbox_ruleset_paths = singbox_ruleset_paths;
        self
    }
}

impl CoreGenEnv for SnapshotCoreGenEnv {
    fn platform(&self) -> CoreGenPlatform {
        self.platform
    }

    fn get_core_type(&self, profile: &ProfileItem, config_type: ConfigType) -> CoreType {
        profile
            .core_type
            .or_else(|| {
                self.core_type_items
                    .iter()
                    .find_map(|(candidate, core_type)| {
                        (*candidate == config_type).then_some(*core_type)
                    })
            })
            .unwrap_or_else(|| self.fallback.resolve(config_type))
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

    fn get_full_config_template_item(&self, core_type: CoreType) -> Option<FullConfigTemplateItem> {
        self.full_config_templates
            .iter()
            .find(|item| item.core_type == core_type)
            .cloned()
    }

    fn get_dns_item(&self, core_type: CoreType) -> Option<DnsItem> {
        self.dns_items
            .iter()
            .find(|item| item.core_type == core_type)
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

    fn get_singbox_ruleset_paths(&self) -> BTreeMap<String, String> {
        self.singbox_ruleset_paths.clone()
    }

    fn next_virtual_chain_id(&self, node: &ProfileItem, child_index_ids: &[String]) -> String {
        format!("inner-{}-{}", node.index_id, child_index_ids.join("-"))
    }
}

const fn default_core_type(_config_type: ConfigType) -> CoreType {
    CoreType::sing_box
}
