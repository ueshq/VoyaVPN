use std::collections::BTreeMap;
use std::path::PathBuf;

use thiserror::Error;
pub use voya_contracts::ResourceUpdateFile;
use voya_db::{Database, DbError};
use voya_net::ruleset::{
    collect_singbox_ruleset_assets, discover_local_singbox_ruleset_paths, geo_assets,
    AcquiredRulesetGeoAsset, AssetAcquisitionOptions, RulesetGeoClient, RulesetGeoError,
};
use voya_platform::paths::AppPaths;

pub type Result<T> = std::result::Result<T, UpdateManagerError>;

/// Where staged rule sets live under the app-data bin directory.
const SRS_DIR_NAME: &str = "srss";

#[derive(Debug, Error)]
pub enum UpdateManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    RulesetGeo(#[from] RulesetGeoError),
}

#[derive(Debug, Clone)]
pub struct UpdateManager<'db> {
    database: &'db Database,
    paths: AppPaths,
    ruleset_geo: RulesetGeoClient,
}

impl<'db> UpdateManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database, paths: AppPaths) -> Self {
        Self {
            database,
            paths,
            ruleset_geo: RulesetGeoClient::new(),
        }
    }

    pub async fn update_geo_assets(
        &self,
        proxy_url: Option<String>,
    ) -> Result<Vec<ResourceUpdateFile>> {
        let assets = geo_assets(None);
        let acquired = self
            .ruleset_geo
            .acquire_geo_assets(
                &assets,
                self.paths.bin_dir(),
                &asset_acquisition_options(proxy_url),
            )
            .await?;

        Ok(acquired.into_iter().map(resource_update_file).collect())
    }

    pub async fn update_srs_assets(
        &self,
        proxy_url: Option<String>,
    ) -> Result<Vec<ResourceUpdateFile>> {
        let routings = self.database.routings().list().await?;
        let assets = collect_singbox_ruleset_assets(None, &routings);
        let acquired = self
            .ruleset_geo
            .acquire_srs_assets(
                &assets,
                srs_dir(&self.paths),
                &asset_acquisition_options(proxy_url),
            )
            .await?;

        Ok(acquired.into_iter().map(resource_update_file).collect())
    }
}

#[must_use]
pub fn local_singbox_ruleset_paths(paths: &AppPaths) -> BTreeMap<String, String> {
    discover_local_singbox_ruleset_paths(srs_dir(paths))
}

#[must_use]
fn srs_dir(paths: &AppPaths) -> PathBuf {
    paths.bin_dir().join(SRS_DIR_NAME)
}

fn asset_acquisition_options(proxy_url: Option<String>) -> AssetAcquisitionOptions {
    AssetAcquisitionOptions {
        prefer_proxy: true,
        proxy_url,
    }
}

fn resource_update_file(asset: AcquiredRulesetGeoAsset) -> ResourceUpdateFile {
    ResourceUpdateFile {
        name: asset.file_name,
        bytes: u32::try_from(asset.bytes).unwrap_or(u32::MAX),
        used_proxy: asset.used_proxy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voya_core::RoutingItem;

    #[test]
    fn resource_assets_use_builtin_urls() {
        let geo = geo_assets(None);
        for name in ["geoip", "geosite"] {
            let asset = geo
                .iter()
                .find(|asset| asset.name == name)
                .expect("Geo asset");
            assert_eq!(asset.url, format!("https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/{name}.dat"));
        }
        let routing = RoutingItem {
            rule_set: vec![voya_core::RulesItem {
                ip: Some(vec!["geoip:cn".to_string()]),
                domain: Some(vec!["geosite:google".to_string()]),
                ..voya_core::RulesItem::default()
            }],
            ..RoutingItem::default()
        };
        let srs = collect_singbox_ruleset_assets(None, &[routing]);
        for (tag, kind) in [("geoip-cn", "geoip"), ("geosite-google", "geosite")] {
            let asset = srs
                .iter()
                .find(|asset| asset.tag == tag)
                .expect("SRS asset");
            assert_eq!(asset.url, format!("https://raw.githubusercontent.com/2dust/sing-box-rules/rule-set-{kind}/{tag}.srs"));
        }
    }

    #[test]
    fn resource_updates_always_prefer_the_runtime_proxy() {
        let options = asset_acquisition_options(Some("http://127.0.0.1:10808".to_string()));

        assert!(options.prefer_proxy);
        assert_eq!(options.proxy_url.as_deref(), Some("http://127.0.0.1:10808"));
    }

    #[test]
    fn resource_update_result_keeps_file_and_proxy_metadata() {
        let result = resource_update_file(AcquiredRulesetGeoAsset {
            kind: voya_net::ruleset::AcquiredAssetKind::Geo,
            name: "geoip".to_string(),
            file_name: "geoip.dat".to_string(),
            url: "https://example.com/geoip.dat".to_string(),
            path: "bin/geoip.dat".into(),
            bytes: u64::MAX,
            used_proxy: true,
            attempts: Vec::new(),
        });

        assert_eq!(result.name, "geoip.dat");
        assert_eq!(result.bytes, u32::MAX);
        assert!(result.used_proxy);
    }
}
