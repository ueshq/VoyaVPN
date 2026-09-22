use std::collections::HashMap;

use thiserror::Error;
use voya_contracts::ExportProfilesResult;
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

/// The share links of `index_ids`, one per line in selection order.
pub async fn export_profiles(
    database: &Database,
    config: &AppConfig,
    index_ids: &[String],
) -> Result<ExportProfilesResult> {
    let profiles = load_profiles(database, index_ids).await?;

    Ok(ExportProfilesResult {
        text: export_share_links(&profiles, config)?,
        count: u32::try_from(profiles.len()).unwrap_or(u32::MAX),
    })
}

async fn load_profiles(database: &Database, index_ids: &[String]) -> Result<Vec<ProfileItem>> {
    if index_ids.is_empty() {
        return Err(ExportManagerError::EmptySelection);
    }

    // One query for the whole selection rather than one per node; the
    // links still come out in selection order.
    let by_id = database
        .profiles()
        .list_by_ids(index_ids)
        .await?
        .into_iter()
        .map(|profile| (profile.index_id.clone(), profile))
        .collect::<HashMap<_, _>>();
    index_ids
        .iter()
        .map(|index_id| {
            by_id
                .get(index_id)
                .cloned()
                .ok_or_else(|| ExportManagerError::ProfileNotFound(index_id.clone()))
        })
        .collect()
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
    async fn export_profiles_exports_share_links_in_selection_order() {
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

        let result = export_profiles(
            &database,
            &AppConfig::default(),
            &["two".to_string(), "one".to_string()],
        )
        .await
        .expect("export test operation should succeed");

        let lines = result.text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("two.example.test"));
        assert!(lines[1].contains("one.example.test"));
    }

    #[tokio::test]
    async fn export_profiles_rejects_an_empty_or_unknown_selection() {
        let database = Database::connect_in_memory()
            .await
            .expect("database test operation should succeed");
        for (index_ids, expected_missing) in [
            (Vec::new(), None),
            (vec!["ghost".to_string()], Some("ghost")),
        ] {
            let error = export_profiles(&database, &AppConfig::default(), &index_ids)
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
