use serde_json::Value;
use thiserror::Error;
use voya_core::{CoreType, FullConfigTemplateItem};
use voya_db::{Database, DbError};

#[derive(Debug, Error)]
pub enum FullConfigTemplateManagerError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("full config template only supports Xray and sing-box")]
    UnsupportedCoreType,
    #[error("{field} must be a JSON object")]
    InvalidJsonObject { field: &'static str },
}

pub type Result<T> = std::result::Result<T, FullConfigTemplateManagerError>;

pub struct FullConfigTemplateManager<'db> {
    database: &'db Database,
}

impl<'db> FullConfigTemplateManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self { database }
    }

    pub async fn load_templates(&self) -> Result<Vec<FullConfigTemplateItem>> {
        let xray = self.ensure_template(CoreType::Xray).await?;
        let sing_box = self.ensure_template(CoreType::sing_box).await?;

        Ok(vec![xray, sing_box])
    }

    pub async fn save_template(
        &self,
        mut item: FullConfigTemplateItem,
    ) -> Result<FullConfigTemplateItem> {
        normalize_template(&mut item)?;
        validate_json_object(item.config.as_deref(), "Config")?;
        validate_json_object(item.tun_config.as_deref(), "TunConfig")?;

        self.database.full_config_templates().upsert(&item).await?;

        Ok(item)
    }

    async fn ensure_template(&self, core_type: CoreType) -> Result<FullConfigTemplateItem> {
        if let Some(item) = self
            .database
            .full_config_templates()
            .get_by_core_type(core_type)
            .await?
        {
            return Ok(item);
        }

        let item = default_template(core_type)?;
        self.database.full_config_templates().upsert(&item).await?;

        Ok(item)
    }
}

fn normalize_template(item: &mut FullConfigTemplateItem) -> Result<()> {
    let defaults = default_template(item.core_type)?;

    item.id = item.id.trim().to_string();
    if item.id.is_empty() {
        item.id = defaults.id;
    }

    item.remarks = item.remarks.trim().to_string();
    if item.remarks.is_empty() {
        item.remarks = defaults.remarks;
    }

    item.config = normalize_optional_text(item.config.take());
    item.tun_config = normalize_optional_text(item.tun_config.take());
    item.proxy_detour = normalize_optional_text(item.proxy_detour.take());
    item.add_proxy_only = Some(item.add_proxy_only.unwrap_or(false));

    Ok(())
}

fn default_template(core_type: CoreType) -> Result<FullConfigTemplateItem> {
    match core_type {
        CoreType::Xray => Ok(FullConfigTemplateItem {
            id: "full-template-xray".to_string(),
            remarks: "Xray".to_string(),
            core_type,
            ..FullConfigTemplateItem::default()
        }),
        CoreType::sing_box => Ok(FullConfigTemplateItem {
            id: "full-template-sing-box".to_string(),
            remarks: "sing-box".to_string(),
            core_type,
            ..FullConfigTemplateItem::default()
        }),
        _ => Err(FullConfigTemplateManagerError::UnsupportedCoreType),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_json_object(value: Option<&str>, field: &'static str) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };

    let parsed: Value = serde_json::from_str(value)
        .map_err(|_| FullConfigTemplateManagerError::InvalidJsonObject { field })?;
    if parsed.is_object() {
        Ok(())
    } else {
        Err(FullConfigTemplateManagerError::InvalidJsonObject { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn template_manager_materializes_xray_and_sing_box_defaults() {
        let database = Database::connect_in_memory()
            .await
            .expect("database test operation should succeed");
        let manager = FullConfigTemplateManager::new(&database);

        let templates = manager
            .load_templates()
            .await
            .expect("template test operation should succeed");

        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].core_type, CoreType::Xray);
        assert_eq!(templates[1].core_type, CoreType::sing_box);
        assert_eq!(
            database
                .full_config_templates()
                .list()
                .await
                .expect("database test operation should succeed")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn template_manager_rejects_non_object_json() {
        let database = Database::connect_in_memory()
            .await
            .expect("database test operation should succeed");
        let manager = FullConfigTemplateManager::new(&database);

        let error = manager
            .save_template(FullConfigTemplateItem {
                core_type: CoreType::Xray,
                config: Some("[]".to_string()),
                ..FullConfigTemplateItem::default()
            })
            .await
            .expect_err("array config should be rejected");

        assert!(matches!(
            error,
            FullConfigTemplateManagerError::InvalidJsonObject { field: "Config" }
        ));
    }
}
