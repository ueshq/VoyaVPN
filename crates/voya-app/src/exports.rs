use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use thiserror::Error;
pub use voya_contracts::{ExportProfilesFormat, ExportProfilesRequest, ExportProfilesResult};
use voya_core::{
    export_share_link_with_options, export_voya_profile_bundle, generate_singbox_config_json,
    AppConfig, CoreConfigContextBuilder, ProfileItem, ShareError, ShareLinkOptions,
    SingboxConfigError,
};
use voya_db::{Database, DbError};
use voya_platform::{coreinfo::TargetOs, paths::AppPaths};

use crate::runtime::load_runtime_core_gen_env;

#[derive(Debug, Error)]
pub enum ExportManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Share(#[from] ShareError),
    #[error(transparent)]
    Singbox(#[from] SingboxConfigError),
    #[error("select at least one node to export")]
    EmptySelection,
    #[error("node not found: {0}")]
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
            ExportProfilesFormat::ShareLinks => export_share_links(&profiles, config)?,
            ExportProfilesFormat::ShareLinksBase64 => {
                BASE64_STANDARD.encode(export_share_links(&profiles, config)?)
            }
            ExportProfilesFormat::VoyaBundle => export_voya_profile_bundle(&profiles)?,
            // A client configuration describes one running core, so a
            // multi-row selection exports the first profile only; the other
            // formats are lists and export every selected row.
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

        generate_singbox_config_json(&result.context).map_err(ExportManagerError::from)
    }
}

fn export_share_links(profiles: &[ProfileItem], config: &AppConfig) -> Result<String> {
    let options = ShareLinkOptions {
        allow_insecure: config.core_basic_item.def_allow_insecure,
        fingerprint: config.core_basic_item.def_fingerprint.clone(),
        hysteria_up_mbps: config.hysteria_item.up_mbps,
        hysteria_down_mbps: config.hysteria_item.down_mbps,
        hysteria_hop_interval: config.hysteria_item.hop_interval,
    };
    profiles
        .iter()
        .map(|profile| export_share_link_with_options(profile, &options))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map(|links| links.join("\n"))
        .map_err(ExportManagerError::from)
}

#[cfg(test)]
mod tests {
    use voya_core::{ProfileItem, ProfileProtocol, ServerEndpoint};

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
                    remarks: id.to_string(),
                    protocol: ProfileProtocol::Vless {
                        server: ServerEndpoint {
                            address: format!("{id}.example.test"),
                            port: 443,
                        },
                        uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                        flow: None,
                        encryption: Some("none".to_string()),
                    },
                    ..ProfileItem::default()
                })
                .await
                .expect("database test operation should succeed");
        }

        let manager = ExportManager::new(&database);
        let paths = AppPaths::new(std::env::temp_dir().join("voyavpn-export-test"));
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

    #[tokio::test]
    async fn export_manager_builds_a_client_config_from_the_first_selected_profile() {
        let database = Database::connect_in_memory()
            .await
            .expect("database test operation should succeed");
        for id in ["one", "two"] {
            database
                .profiles()
                .upsert(&ProfileItem {
                    index_id: id.to_string(),
                    remarks: id.to_string(),
                    protocol: ProfileProtocol::Vless {
                        server: ServerEndpoint {
                            address: format!("{id}.example.test"),
                            port: 443,
                        },
                        uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                        flow: None,
                        encryption: Some("none".to_string()),
                    },
                    ..ProfileItem::default()
                })
                .await
                .expect("database test operation should succeed");
        }

        let manager = ExportManager::new(&database);
        let paths = AppPaths::new(std::env::temp_dir().join("voyavpn-export-test"));
        let result = manager
            .export_profiles(
                &paths,
                &AppConfig::default(),
                TargetOs::Linux,
                ExportProfilesRequest {
                    index_ids: vec!["two".to_string(), "one".to_string()],
                    format: ExportProfilesFormat::ClientConfig,
                },
            )
            .await
            .expect("export test operation should succeed");

        assert!(
            result.text.contains("two.example.test"),
            "a client configuration is generated for the first selected profile"
        );
        assert!(!result.text.contains("one.example.test"));
        assert_eq!(result.format, ExportProfilesFormat::ClientConfig);
    }

    #[tokio::test]
    async fn export_manager_rejects_an_empty_or_unknown_selection() {
        let database = Database::connect_in_memory()
            .await
            .expect("database test operation should succeed");
        let manager = ExportManager::new(&database);
        let paths = AppPaths::new(std::env::temp_dir().join("voyavpn-export-test"));

        for (index_ids, expected_missing) in [
            (Vec::new(), None),
            (vec!["ghost".to_string()], Some("ghost")),
        ] {
            let error = manager
                .export_profiles(
                    &paths,
                    &AppConfig::default(),
                    TargetOs::Linux,
                    ExportProfilesRequest {
                        index_ids,
                        format: ExportProfilesFormat::ClientConfig,
                    },
                )
                .await
                .expect_err("an unusable selection must not reach config generation");

            match (error, expected_missing) {
                (ExportManagerError::EmptySelection, None) => {}
                (ExportManagerError::ProfileNotFound(id), Some(expected)) => {
                    assert_eq!(id, expected);
                }
                (error, expected) => panic!("unexpected {error:?} for {expected:?}"),
            }
        }
    }
}
