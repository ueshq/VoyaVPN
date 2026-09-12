use thiserror::Error;
pub use voya_contracts::{ExportProfilesFormat, ExportProfilesRequest, ExportProfilesResult};
use voya_core::{
    export_share_link_with_options, AppConfig, ProfileItem, ShareError, ShareLinkOptions,
};
use voya_db::{Database, DbError};

#[derive(Debug, Error)]
pub enum ExportManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Share(#[from] ShareError),
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
        config: &AppConfig,
        request: ExportProfilesRequest,
    ) -> Result<ExportProfilesResult> {
        let profiles = self.load_profiles(&request.index_ids).await?;
        let text = match request.format {
            ExportProfilesFormat::ShareLinks => export_share_links(&profiles, config)?,
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
        let result = manager
            .export_profiles(
                &AppConfig::default(),
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
    async fn export_manager_rejects_an_empty_or_unknown_selection() {
        let database = Database::connect_in_memory()
            .await
            .expect("database test operation should succeed");
        let manager = ExportManager::new(&database);

        for (index_ids, expected_missing) in [
            (Vec::new(), None),
            (vec!["ghost".to_string()], Some("ghost")),
        ] {
            let error = manager
                .export_profiles(
                    &AppConfig::default(),
                    ExportProfilesRequest {
                        index_ids,
                        format: ExportProfilesFormat::ShareLinks,
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
