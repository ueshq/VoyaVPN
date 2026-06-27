use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use voya_core::{
    export_inner_share_links, export_share_link, generate_singbox_config_json,
    generate_xray_config_json, AppConfig, CoreConfigContextBuilder, CoreType, ProfileItem,
    ShareError, SingboxConfigError,
};
use voya_db::{Database, DbError};
use voya_platform::{coreinfo::TargetOs, paths::AppPaths};

use crate::runtime::load_runtime_core_gen_env;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExportProfilesFormat {
    ShareLinks,
    ShareLinksBase64,
    InnerLinks,
    ClientConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfilesRequest {
    pub index_ids: Vec<String>,
    pub format: ExportProfilesFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfilesResult {
    pub text: String,
    pub count: u32,
    pub format: ExportProfilesFormat,
}

#[derive(Debug, Error)]
pub enum ExportManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Share(#[from] ShareError),
    #[error(transparent)]
    Singbox(#[from] SingboxConfigError),
    #[error("select at least one profile to export")]
    EmptySelection,
    #[error("profile not found: {0}")]
    ProfileNotFound(String),
}

pub type Result<T> = std::result::Result<T, ExportManagerError>;

pub struct ExportManager<'db> {
    database: &'db Database,
}

impl<'db> ExportManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self { database }
    }

    pub async fn export_profiles(
        &self,
        paths: &AppPaths,
        config: &AppConfig,
        target_os: TargetOs,
        request: ExportProfilesRequest,
    ) -> Result<ExportProfilesResult> {
        let profiles = self.load_profiles(&request.index_ids).await?;
        let text = match request.format {
            ExportProfilesFormat::ShareLinks => export_share_links(&profiles)?,
            ExportProfilesFormat::ShareLinksBase64 => {
                BASE64_STANDARD.encode(export_share_links(&profiles)?)
            }
            ExportProfilesFormat::InnerLinks => export_inner_share_links(&profiles)?,
            ExportProfilesFormat::ClientConfig => {
                self.export_client_config(paths, config, target_os, &profiles[0])
                    .await?
            }
        };

        Ok(ExportProfilesResult {
            text,
            count: u32::try_from(profiles.len()).unwrap_or(u32::MAX),
            format: request.format,
        })
    }

    async fn load_profiles(&self, index_ids: &[String]) -> Result<Vec<ProfileItem>> {
        if index_ids.is_empty() {
            return Err(ExportManagerError::EmptySelection);
        }

        let mut profiles = Vec::with_capacity(index_ids.len());
        for index_id in index_ids {
            let profile = self
                .database
                .profiles()
                .get(index_id)
                .await?
                .ok_or_else(|| ExportManagerError::ProfileNotFound(index_id.clone()))?;
            profiles.push(profile);
        }

        Ok(profiles)
    }

    async fn export_client_config(
        &self,
        paths: &AppPaths,
        config: &AppConfig,
        target_os: TargetOs,
        profile: &ProfileItem,
    ) -> Result<String> {
        let mut export_config = config.clone();
        export_config.index_id.clone_from(&profile.index_id);
        let env =
            load_runtime_core_gen_env(self.database, paths, &export_config, target_os).await?;
        let result = CoreConfigContextBuilder::new(&env).build(&export_config, profile);

        if result.context.run_core_type == CoreType::sing_box {
            generate_singbox_config_json(&result.context).map_err(ExportManagerError::from)
        } else {
            Ok(generate_xray_config_json(&result.context))
        }
    }
}

fn export_share_links(profiles: &[ProfileItem]) -> Result<String> {
    profiles
        .iter()
        .map(export_share_link)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map(|links| links.join("\n"))
        .map_err(ExportManagerError::from)
}

#[cfg(test)]
mod tests {
    use voya_core::{ConfigType, ProfileItem};
    use voya_platform::paths::StorageMode;

    use super::*;

    #[tokio::test]
    async fn export_manager_exports_share_links_in_selection_order() {
        let database = Database::connect_in_memory()
            .await
            .expect("database test operation should succeed");
        for id in ["one", "two"] {
            database
                .profiles()
                .upsert(&ProfileItem {
                    index_id: id.to_string(),
                    config_type: ConfigType::VLESS,
                    remarks: id.to_string(),
                    address: format!("{id}.example.test"),
                    port: 443,
                    password: "00000000-0000-0000-0000-000000000000".to_string(),
                    ..ProfileItem::default()
                })
                .await
                .expect("database test operation should succeed");
        }

        let manager = ExportManager::new(&database);
        let paths = AppPaths::new(
            std::env::temp_dir().join("voyavpn-export-test"),
            StorageMode::Portable,
        );
        let result = manager
            .export_profiles(
                &paths,
                &AppConfig::default(),
                TargetOs::Linux,
                ExportProfilesRequest {
                    index_ids: vec!["two".to_string(), "one".to_string()],
                    format: ExportProfilesFormat::ShareLinks,
                },
            )
            .await
            .expect("export test operation should succeed");

        let lines = result.text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("two.example.test"));
        assert!(lines[1].contains("one.example.test"));
    }
}
