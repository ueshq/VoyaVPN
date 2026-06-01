use thiserror::Error;
use voya_core::{
    generate_singbox_config_value, generate_xray_config_value, group_preview_from_values,
    list_group_child_candidates, validate_group_profile, AppConfig, ConfigType,
    CoreConfigContextBuilder, CoreGenEnv, CoreGenPlatform, CoreType, DnsItem,
    FullConfigTemplateItem, GroupChildCandidate, GroupPreview, GroupValidationResult,
    InboundProtocol, ProfileItem, ProfileListItem, RoutingItem, SubItem,
};
use voya_db::{Database, DbError};

use crate::profiles::{ProfileManager, ProfileManagerError};

pub type Result<T> = std::result::Result<T, GroupManagerError>;

#[derive(Debug, Error)]
pub enum GroupManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Profile(#[from] ProfileManagerError),
    #[error("profile is not a policy group or proxy chain")]
    NotGroupProfile,
    #[error("group validation failed: {0:?}")]
    Validation(Vec<String>),
}

#[derive(Debug, Clone, Copy)]
pub struct GroupManager<'db> {
    database: &'db Database,
}

impl<'db> GroupManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self { database }
    }

    pub async fn list_child_candidates(
        &self,
        current_index_id: Option<&str>,
        filter: Option<&str>,
    ) -> Result<Vec<GroupChildCandidate>> {
        let profiles = self.database.profiles().list().await?;

        Ok(list_group_child_candidates(
            &profiles,
            current_index_id,
            filter,
        ))
    }

    pub async fn validate_group_profile(
        &self,
        profile: &ProfileItem,
    ) -> Result<GroupValidationResult> {
        let profiles = self.profiles_with_draft(profile).await?;

        Ok(validate_group_profile(profile, &profiles))
    }

    pub async fn preview_group_profile(
        &self,
        config: &AppConfig,
        profile: &ProfileItem,
    ) -> Result<GroupPreview> {
        ensure_group_profile(profile)?;

        let profiles = self.profiles_with_draft(profile).await?;
        let mut validation = validate_group_profile(profile, &profiles);
        if !validation.valid {
            return Ok(group_preview_from_values(validation, None, None));
        }

        let routings = self.database.routings().list().await?;
        let dns_items = self.database.dns().list().await?;
        let subs = self.database.subscriptions().list().await?;
        let xray_value = self.preview_value(
            config,
            profile,
            &profiles,
            &routings,
            &dns_items,
            &subs,
            CoreType::Xray,
        );
        let singbox_value = self.preview_value(
            config,
            profile,
            &profiles,
            &routings,
            &dns_items,
            &subs,
            CoreType::sing_box,
        );

        validation.warnings.extend(
            xray_value
                .builder_warnings
                .iter()
                .map(|warning| format!("xray: {warning}")),
        );
        validation.warnings.extend(
            singbox_value
                .builder_warnings
                .iter()
                .map(|warning| format!("sing-box: {warning}")),
        );

        Ok(group_preview_from_values(
            validation,
            Some(&xray_value.value),
            Some(&singbox_value.value),
        ))
    }

    pub async fn save_group_profile(
        &self,
        config: &mut AppConfig,
        profile: ProfileItem,
    ) -> Result<ProfileListItem> {
        ensure_group_profile(&profile)?;

        let validation = self.validate_group_profile(&profile).await?;
        if !validation.valid {
            return Err(GroupManagerError::Validation(validation.errors));
        }

        ProfileManager::new(self.database)
            .save_profile(config, profile)
            .await
            .map_err(Into::into)
    }

    async fn profiles_with_draft(&self, profile: &ProfileItem) -> Result<Vec<ProfileItem>> {
        let mut profiles = self.database.profiles().list().await?;

        if !profile.index_id.trim().is_empty() {
            if let Some(existing) = profiles
                .iter_mut()
                .find(|candidate| candidate.index_id == profile.index_id)
            {
                *existing = profile.clone();
                return Ok(profiles);
            }
        }

        profiles.push(profile.clone());
        Ok(profiles)
    }

    fn preview_value(
        &self,
        config: &AppConfig,
        profile: &ProfileItem,
        profiles: &[ProfileItem],
        routings: &[RoutingItem],
        dns_items: &[DnsItem],
        subs: &[SubItem],
        core_type: CoreType,
    ) -> PreviewValue {
        let mut node = profile.clone();
        node.core_type = Some(core_type);
        let env = GroupCoreGenEnv {
            core_type_items: config
                .core_type_item
                .iter()
                .map(|item| (item.config_type, item.core_type))
                .collect(),
            local_socks_port: config
                .inbound
                .first()
                .map_or(voya_core::DEFAULT_LOCAL_PORT, |inbound| inbound.local_port),
            profiles: profiles
                .iter()
                .map(|candidate| {
                    if candidate.index_id == node.index_id {
                        node.clone()
                    } else {
                        candidate.clone()
                    }
                })
                .collect(),
            routings: routings.to_vec(),
            dns_items: dns_items.to_vec(),
            subs: subs.to_vec(),
            core_type,
        };
        let mut preview_config = config.clone();
        preview_config.index_id.clone_from(&node.index_id);
        preview_config.tun_mode_item.enable_tun = false;
        let result = CoreConfigContextBuilder::new(&env).build(&preview_config, &node);
        let value = match core_type {
            CoreType::sing_box => generate_singbox_config_value(&result.context),
            _ => generate_xray_config_value(&result.context),
        };

        PreviewValue {
            value,
            builder_warnings: result.validator_result.warnings,
        }
    }
}

fn ensure_group_profile(profile: &ProfileItem) -> Result<()> {
    if profile.config_type.is_group_type() {
        Ok(())
    } else {
        Err(GroupManagerError::NotGroupProfile)
    }
}

#[derive(Debug, Clone)]
struct PreviewValue {
    value: serde_json::Value,
    builder_warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct GroupCoreGenEnv {
    core_type_items: Vec<(ConfigType, CoreType)>,
    local_socks_port: i32,
    profiles: Vec<ProfileItem>,
    routings: Vec<RoutingItem>,
    dns_items: Vec<DnsItem>,
    subs: Vec<SubItem>,
    core_type: CoreType,
}

impl CoreGenEnv for GroupCoreGenEnv {
    fn platform(&self) -> CoreGenPlatform {
        CoreGenPlatform::Linux
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
            .unwrap_or(self.core_type)
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
        _core_type: CoreType,
    ) -> Option<FullConfigTemplateItem> {
        None
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

    fn next_virtual_chain_id(&self, node: &ProfileItem, child_index_ids: &[String]) -> String {
        format!("inner-{}-{}", node.index_id, child_index_ids.join("-"))
    }
}

#[cfg(test)]
mod tests {
    use voya_core::{ConfigType, ProtocolExtraItem};
    use voya_db::Database;

    use super::*;

    #[tokio::test]
    async fn group_manager_blocks_cycle_before_save() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = GroupManager::new(&database);
        let mut config = AppConfig::default();
        let leaf = sample_leaf("leaf", "Leaf");
        let root = sample_group("root", "Root", "leaf,nested");
        let nested = sample_group("nested", "Nested", "root");

        database.profiles().upsert(&leaf).await.unwrap();
        database.profiles().upsert(&root).await.unwrap();
        database.profiles().upsert(&nested).await.unwrap();

        let error = manager
            .save_group_profile(&mut config, root)
            .await
            .expect_err("cycle should fail");

        assert!(matches!(error, GroupManagerError::Validation(_)));
    }

    #[tokio::test]
    async fn group_manager_preview_exposes_xray_and_singbox_routes() {
        let database = Database::connect_in_memory().await.unwrap();
        let manager = GroupManager::new(&database);
        let leaf_a = sample_leaf("leaf-a", "Leaf A");
        let leaf_b = sample_leaf("leaf-b", "Leaf B");
        let group = sample_group("group", "Group", "leaf-a,leaf-b");

        database.profiles().upsert(&leaf_a).await.unwrap();
        database.profiles().upsert(&leaf_b).await.unwrap();
        database.profiles().upsert(&group).await.unwrap();

        let preview = manager
            .preview_group_profile(&AppConfig::default(), &group)
            .await
            .unwrap();

        assert!(preview.validation.valid);
        assert!(preview
            .xray_balancers
            .iter()
            .any(|balancer| balancer.tag == "proxy-balancer"));
        assert!(preview
            .singbox_routes
            .iter()
            .any(|route| route.kind == "selector"));
        assert!(preview
            .singbox_routes
            .iter()
            .any(|route| route.kind == "urltest"));
    }

    fn sample_leaf(index_id: &str, remarks: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VLESS,
            remarks: remarks.to_string(),
            address: format!("{index_id}.example.test"),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            network: "tcp".to_string(),
            ..ProfileItem::default()
        }
    }

    fn sample_group(index_id: &str, remarks: &str, child_items: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::PolicyGroup,
            remarks: remarks.to_string(),
            address: "group".to_string(),
            protocol_extra: ProtocolExtraItem {
                child_items: Some(child_items.to_string()),
                group_type: Some("PolicyGroup".to_string()),
                ..ProtocolExtraItem::default()
            },
            ..ProfileItem::default()
        }
    }
}
