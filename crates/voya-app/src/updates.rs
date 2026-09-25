use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use voya_contracts::ResourceUpdateFile;
use voya_db::{Database, DbError};
use voya_net::ruleset::{
    collect_singbox_ruleset_assets, discover_local_singbox_ruleset_paths, AcquiredRuleset,
    AssetAcquisitionOptions, RulesetClient, RulesetError,
};
use voya_platform::{
    filesystem,
    paths::{AppPaths, RULE_SET_SEED_DIR_NAME},
};

pub type Result<T> = std::result::Result<T, UpdateManagerError>;

/// Where staged rule sets live under the app-data bin directory.
const SRS_DIR_NAME: &str = "srss";

#[derive(Debug, Error)]
pub enum UpdateManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Ruleset(#[from] RulesetError),
}

#[derive(Debug, Clone)]
pub struct UpdateManager<'db> {
    database: &'db Database,
    paths: AppPaths,
    ruleset: RulesetClient,
}

impl<'db> UpdateManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database, paths: AppPaths) -> Self {
        Self {
            database,
            paths,
            ruleset: RulesetClient::new(),
        }
    }

    pub async fn update_srs_assets(
        &self,
        proxy_url: Option<String>,
    ) -> Result<Vec<ResourceUpdateFile>> {
        let routings = self.database.routings().list().await?;
        let assets = collect_singbox_ruleset_assets(None, &routings);
        let acquired = self
            .ruleset
            .acquire_srs_assets(
                &assets,
                srs_dir(&self.paths),
                &asset_acquisition_options(proxy_url),
            )
            .await?;

        Ok(acquired.into_iter().map(resource_update_file).collect())
    }
}

/// Copies the packaged rule sets (see `core_seed_resources_dir`) into the
/// app-data rule-set directory, skipping any already there: a rule-library
/// update may have replaced them with newer ones. Returns the copies.
///
/// Until these exist the generated config names remote rule sets that the
/// core fetches through the proxy, so the default China and LAN rules would
/// not apply on a first start whose proxy cannot reach GitHub.
pub fn install_seed_rule_sets(
    paths: &AppPaths,
    core_seed_resources_dir: &Path,
) -> io::Result<Vec<PathBuf>> {
    filesystem::copy_missing_files(
        &core_seed_resources_dir.join(RULE_SET_SEED_DIR_NAME),
        &srs_dir(paths),
        "srs",
    )
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

fn resource_update_file(asset: AcquiredRuleset) -> ResourceUpdateFile {
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
    fn packaged_rule_sets_become_local_rule_sets() {
        let dir = tempfile::Builder::new()
            .prefix("voyavpn-seed-rule-sets-")
            .tempdir()
            .expect("temp dir");
        let seeds = dir.path().join("resources").join("core-seeds");
        let paths = AppPaths::new(dir.path().join("app"));
        filesystem::write_file_with_parent(
            &seeds.join(RULE_SET_SEED_DIR_NAME).join("geosite-cn.srs"),
            b"SRS",
        )
        .expect("packaged rule set");
        assert!(local_singbox_ruleset_paths(&paths).is_empty());

        let copied = install_seed_rule_sets(&paths, &seeds).expect("install");

        assert_eq!(copied.len(), 1);
        let local = local_singbox_ruleset_paths(&paths);
        assert_eq!(
            local.get("geosite-cn").map(PathBuf::from),
            Some(srs_dir(&paths).join("geosite-cn.srs"))
        );
    }

    #[test]
    fn resource_updates_always_prefer_the_runtime_proxy() {
        let options = asset_acquisition_options(Some("http://127.0.0.1:10808".to_string()));

        assert!(options.prefer_proxy);
        assert_eq!(options.proxy_url.as_deref(), Some("http://127.0.0.1:10808"));
    }

    #[test]
    fn resource_update_result_keeps_file_and_proxy_metadata() {
        let result = resource_update_file(AcquiredRuleset {
            name: "geoip-cn".to_string(),
            file_name: "geoip-cn.srs".to_string(),
            url: "https://example.com/geoip-cn.srs".to_string(),
            path: "bin/srss/geoip-cn.srs".into(),
            bytes: u64::MAX,
            used_proxy: true,
            attempts: Vec::new(),
        });

        assert_eq!(result.name, "geoip-cn.srs");
        assert_eq!(result.bytes, u32::MAX);
        assert!(result.used_proxy);
    }
}
