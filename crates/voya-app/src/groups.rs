use thiserror::Error;
use voya_core::{
    generate_singbox_config_value, generate_xray_config_value, group_preview_from_values,
    list_group_child_candidates, validate_group_profile, AppConfig, CoreConfigContextBuilder,
    CoreGenPlatform, CoreType, DnsItem, GroupChildCandidate, GroupPreview, GroupValidationResult,
    ProfileItem, ProfileListItem, RoutingItem, SingboxConfigError, SubItem,
};
use voya_db::{Database, DbError};

use crate::coregen::{CoreTypeFallback, SnapshotCoreGenData, SnapshotCoreGenEnv};
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
    #[error(transparent)]
    SingboxConfig(#[from] SingboxConfigError),
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
        let source = GroupPreviewSource {
            profiles: &profiles,
            routings: &routings,
            dns_items: &dns_items,
            subs: &subs,
        };
        let xray_value = self.preview_value(config, profile, &source, CoreType::Xray)?;
        let singbox_value = self.preview_value(config, profile, &source, CoreType::sing_box)?;

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
        source: &GroupPreviewSource<'_>,
        core_type: CoreType,
    ) -> Result<PreviewValue> {
        let mut node = profile.clone();
        node.core_type = Some(core_type);
        let env = SnapshotCoreGenEnv::new(
            config,
            CoreGenPlatform::Linux,
            CoreTypeFallback::Fixed(core_type),
            SnapshotCoreGenData {
                profiles: source
                    .profiles
                    .iter()
                    .map(|candidate| {
                        if candidate.index_id == node.index_id {
                            node.clone()
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect(),
                routings: source.routings.to_vec(),
                dns_items: source.dns_items.to_vec(),
                full_config_templates: Vec::new(),
                subs: source.subs.to_vec(),
            },
        );
        let mut preview_config = config.clone();
        preview_config.index_id.clone_from(&node.index_id);
        preview_config.tun_mode_item.enable_tun = false;
        let result = CoreConfigContextBuilder::new(&env).build(&preview_config, &node);
        let value = match core_type {
            CoreType::sing_box => generate_singbox_config_value(&result.context)?,
            _ => generate_xray_config_value(&result.context),
        };

        Ok(PreviewValue {
            value,
            builder_warnings: result.validator_result.warnings,
        })
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

#[derive(Debug, Clone, Copy)]
struct GroupPreviewSource<'a> {
    profiles: &'a [ProfileItem],
    routings: &'a [RoutingItem],
    dns_items: &'a [DnsItem],
    subs: &'a [SubItem],
}

#[cfg(test)]
mod tests {
    use voya_core::{ConfigType, ProtocolExtraItem};
    use voya_db::Database;

    use super::*;

    #[tokio::test]
    async fn group_manager_blocks_cycle_before_save() {
        let database = Database::connect_in_memory()
            .await
            .expect("group manager test operation should succeed");
        let manager = GroupManager::new(&database);
        let mut config = AppConfig::default();
        let leaf = sample_leaf("leaf", "Leaf");
        let root = sample_group("root", "Root", "leaf,nested");
        let nested = sample_group("nested", "Nested", "root");

        database
            .profiles()
            .upsert(&leaf)
            .await
            .expect("group manager test operation should succeed");
        database
            .profiles()
            .upsert(&root)
            .await
            .expect("group manager test operation should succeed");
        database
            .profiles()
            .upsert(&nested)
            .await
            .expect("group manager test operation should succeed");

        let error = manager
            .save_group_profile(&mut config, root)
            .await
            .expect_err("cycle should fail");

        assert!(matches!(error, GroupManagerError::Validation(_)));
    }

    #[tokio::test]
    async fn group_manager_preview_exposes_xray_and_singbox_routes() {
        let database = Database::connect_in_memory()
            .await
            .expect("group manager test operation should succeed");
        let manager = GroupManager::new(&database);
        let leaf_a = sample_leaf("leaf-a", "Leaf A");
        let leaf_b = sample_leaf("leaf-b", "Leaf B");
        let group = sample_group("group", "Group", "leaf-a,leaf-b");

        database
            .profiles()
            .upsert(&leaf_a)
            .await
            .expect("group manager test operation should succeed");
        database
            .profiles()
            .upsert(&leaf_b)
            .await
            .expect("group manager test operation should succeed");
        database
            .profiles()
            .upsert(&group)
            .await
            .expect("group manager test operation should succeed");

        let preview = manager
            .preview_group_profile(&AppConfig::default(), &group)
            .await
            .expect("group manager test operation should succeed");

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
