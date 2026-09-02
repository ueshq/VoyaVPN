use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use voya_core::{RoutingItem, RulesItem};

use crate::{DownloadAttempt, DownloadClient, DownloadError, DownloadRequest, USER_AGENT_PREFIX};

const RULESET_ASSET_RESPONSE_LIMIT_BYTES: usize = 256 * 1024 * 1024;
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub const DEFAULT_GEO_SOURCE_URL: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/{0}.dat";
pub const DEFAULT_SINGBOX_RULESET_URL: &str =
    "https://raw.githubusercontent.com/2dust/sing-box-rules/rule-set-{0}/{1}.srs";
pub const OTHER_GEO_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/Loyalsoldier/geoip/release/geoip-only-cn-private.dat",
    "https://raw.githubusercontent.com/Loyalsoldier/geoip/release/Country.mmdb",
];
pub const DEFAULT_SRS_GEOSITE_TAGS: &[&str] =
    &["google", "cn", "geolocation-cn", "category-ads-all"];

#[derive(Debug, Error)]
pub enum RulesetGeoError {
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("asset file operation failed for {path}: {source}")]
    AssetIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid acquired asset {path}: {reason}")]
    InvalidAsset { path: PathBuf, reason: String },
    #[error("failed to parse ruleset manifest: {0}")]
    Manifest(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, RulesetGeoError>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetAcquisitionOptions {
    pub prefer_proxy: bool,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquiredAssetKind {
    Geo,
    Ruleset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquiredRulesetGeoAsset {
    pub kind: AcquiredAssetKind,
    pub name: String,
    pub file_name: String,
    pub url: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub used_proxy: bool,
    pub attempts: Vec<DownloadAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoAsset {
    pub name: String,
    pub file_name: String,
    pub url: String,
}

impl GeoAsset {
    pub fn new(
        name: impl Into<String>,
        file_name: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            file_name: file_name.into(),
            url: url.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrsAsset {
    pub kind: String,
    pub name: String,
    pub tag: String,
    pub file_name: String,
    pub url: String,
}

impl SrsAsset {
    pub fn new(source_url: &str, kind: &str, name: &str) -> Self {
        let tag = format!("{kind}-{name}");
        Self {
            kind: kind.to_string(),
            name: name.to_string(),
            file_name: format!("{tag}.srs"),
            url: format_srs_url(source_url, kind, name),
            tag,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RulesetGeoManifest {
    pub geo: Vec<GeoManifestEntry>,
    pub srs: Vec<SrsManifestEntry>,
}

impl RulesetGeoManifest {
    pub fn from_json(input: &str) -> Result<Self> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn geo_assets(&self, source_url: Option<&str>) -> Vec<GeoAsset> {
        let source_url = nonempty(source_url).unwrap_or(DEFAULT_GEO_SOURCE_URL);
        self.geo
            .iter()
            .filter_map(|entry| {
                let name = nonempty(Some(entry.name.as_str()))?;
                let file_name = entry
                    .file_name
                    .as_deref()
                    .and_then(|value| nonempty(Some(value)))
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("{name}.dat"));
                let url = entry
                    .url
                    .as_deref()
                    .and_then(|value| nonempty(Some(value)))
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format_geo_url(source_url, name));
                Some(GeoAsset::new(name, file_name, url))
            })
            .collect()
    }

    pub fn srs_assets(&self, source_url: Option<&str>) -> Vec<SrsAsset> {
        let source_url = nonempty(source_url).unwrap_or(DEFAULT_SINGBOX_RULESET_URL);
        self.srs
            .iter()
            .filter_map(|entry| {
                let kind = nonempty(Some(entry.kind.as_str()))?;
                let name = nonempty(Some(entry.name.as_str()))?;
                let mut asset = SrsAsset::new(source_url, kind, name);
                if let Some(tag) = entry.tag.as_deref().and_then(|value| nonempty(Some(value))) {
                    asset.tag = tag.to_string();
                }
                if let Some(file_name) = entry
                    .file_name
                    .as_deref()
                    .and_then(|value| nonempty(Some(value)))
                {
                    asset.file_name = file_name.to_string();
                }
                if let Some(url) = entry.url.as_deref().and_then(|value| nonempty(Some(value))) {
                    asset.url = url.to_string();
                }
                Some(asset)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GeoManifestEntry {
    pub name: String,
    pub file_name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SrsManifestEntry {
    pub kind: String,
    pub name: String,
    pub tag: Option<String>,
    pub file_name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RulesetGeoClient {
    download: DownloadClient,
}

impl RulesetGeoClient {
    pub fn new() -> Self {
        Self {
            download: DownloadClient::new(),
        }
    }

    pub async fn acquire_geo_assets(
        &self,
        assets: &[GeoAsset],
        target_dir: impl AsRef<Path>,
        options: &AssetAcquisitionOptions,
    ) -> Result<Vec<AcquiredRulesetGeoAsset>> {
        let target_dir = target_dir.as_ref();
        let staging = StagingDir::create(target_dir)?;
        let mut acquired = Vec::new();

        for asset in assets {
            let target = target_dir.join(&asset.file_name);
            let staged = self
                .stage_asset(
                    &asset.url,
                    &asset.file_name,
                    staging.path(),
                    &["dat", "mmdb", "metadb"],
                    options,
                )
                .await?;
            acquired.push(AcquiredRulesetGeoAsset {
                kind: AcquiredAssetKind::Geo,
                name: asset.name.clone(),
                file_name: asset.file_name.clone(),
                url: asset.url.clone(),
                path: target,
                bytes: staged.bytes,
                used_proxy: staged.used_proxy,
                attempts: staged.attempts,
            });
        }

        staging.commit(
            target_dir,
            assets.iter().map(|asset| asset.file_name.as_str()),
        )?;
        Ok(acquired)
    }

    pub async fn acquire_srs_assets(
        &self,
        assets: &[SrsAsset],
        target_dir: impl AsRef<Path>,
        options: &AssetAcquisitionOptions,
    ) -> Result<Vec<AcquiredRulesetGeoAsset>> {
        let target_dir = target_dir.as_ref();
        let staging = StagingDir::create(target_dir)?;
        let mut acquired = Vec::new();

        for asset in assets {
            let target = target_dir.join(&asset.file_name);
            let staged = self
                .stage_asset(
                    &asset.url,
                    &asset.file_name,
                    staging.path(),
                    &["srs"],
                    options,
                )
                .await?;
            acquired.push(AcquiredRulesetGeoAsset {
                kind: AcquiredAssetKind::Ruleset,
                name: asset.tag.clone(),
                file_name: asset.file_name.clone(),
                url: asset.url.clone(),
                path: target,
                bytes: staged.bytes,
                used_proxy: staged.used_proxy,
                attempts: staged.attempts,
            });
        }

        staging.commit(
            target_dir,
            assets.iter().map(|asset| asset.file_name.as_str()),
        )?;
        Ok(acquired)
    }

    async fn stage_asset(
        &self,
        url: &str,
        file_name: &str,
        staging_dir: &Path,
        allowed_extensions: &[&str],
        options: &AssetAcquisitionOptions,
    ) -> Result<StagedAsset> {
        let response = self
            .download
            .download_bytes(download_request(url, options))
            .await?;
        let bytes = validate_asset(file_name, &response.body, allowed_extensions)?;
        let staged_path = staging_dir.join(file_name);
        if let Some(parent) = staged_path.parent() {
            fs::create_dir_all(parent).map_err(|source| asset_io(parent, source))?;
        }
        fs::write(&staged_path, response.body).map_err(|source| asset_io(&staged_path, source))?;

        Ok(StagedAsset {
            attempts: response.attempts,
            bytes,
            used_proxy: response.used_proxy,
        })
    }
}

struct StagedAsset {
    attempts: Vec<DownloadAttempt>,
    bytes: u64,
    used_proxy: bool,
}

struct StagingDir {
    path: PathBuf,
}

impl StagingDir {
    fn create(target_dir: &Path) -> Result<Self> {
        fs::create_dir_all(target_dir).map_err(|source| asset_io(target_dir, source))?;
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = target_dir.join(format!(".voya-stage-{}-{sequence}", std::process::id()));
        fs::create_dir(&path).map_err(|source| asset_io(&path, source))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn commit<'a>(
        &self,
        target_dir: &Path,
        file_names: impl Iterator<Item = &'a str>,
    ) -> Result<()> {
        for file_name in file_names {
            let staged = self.path.join(file_name);
            let target = target_dir.join(file_name);
            fs::copy(&staged, &target).map_err(|source| asset_io(&target, source))?;
        }
        Ok(())
    }
}

impl Drop for StagingDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn asset_io(path: &Path, source: std::io::Error) -> RulesetGeoError {
    RulesetGeoError::AssetIo {
        path: path.to_path_buf(),
        source,
    }
}

pub fn geo_assets(source_url: Option<&str>) -> Vec<GeoAsset> {
    let source_url = nonempty(source_url);
    let uses_default_source = source_url.is_none();
    let source_url = source_url.unwrap_or(DEFAULT_GEO_SOURCE_URL);
    let mut assets = ["geosite", "geoip"]
        .into_iter()
        .map(|name| {
            GeoAsset::new(
                name,
                format!("{name}.dat"),
                format_geo_url(source_url, name),
            )
        })
        .collect::<Vec<_>>();

    if uses_default_source || source_url == DEFAULT_GEO_SOURCE_URL {
        assets.extend(OTHER_GEO_URLS.iter().map(|url| {
            let file_name = url.rsplit('/').next().unwrap_or("geo.dat");
            GeoAsset::new(file_name.trim_end_matches(".dat"), file_name, *url)
        }));
    }

    assets
}

pub fn collect_singbox_ruleset_assets(
    source_url: Option<&str>,
    routings: &[RoutingItem],
) -> Vec<SrsAsset> {
    let mut geoip = BTreeSet::new();
    let mut geosite = BTreeSet::new();

    for routing in routings {
        for rule in &routing.rule_set {
            collect_srs_from_rule(rule, &mut geoip, &mut geosite);
        }
    }

    geosite.extend(DEFAULT_SRS_GEOSITE_TAGS.iter().map(ToString::to_string));

    let source_url = nonempty(source_url).unwrap_or(DEFAULT_SINGBOX_RULESET_URL);
    geoip
        .into_iter()
        .map(|name| SrsAsset::new(source_url, "geoip", &name))
        .chain(
            geosite
                .into_iter()
                .map(|name| SrsAsset::new(source_url, "geosite", &name)),
        )
        .collect()
}

pub fn discover_local_singbox_ruleset_paths(srs_dir: impl AsRef<Path>) -> BTreeMap<String, String> {
    let srs_dir = srs_dir.as_ref();
    let Ok(entries) = fs::read_dir(srs_dir) else {
        return BTreeMap::new();
    };

    entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("srs") {
                return None;
            }
            let tag = path.file_stem()?.to_str()?.trim();
            (!tag.is_empty()).then(|| (tag.to_string(), path.to_string_lossy().into_owned()))
        })
        .collect()
}

pub fn format_geo_url(source_url: &str, name: &str) -> String {
    source_url.replace("{0}", name).replace("{name}", name)
}

pub fn format_srs_url(source_url: &str, kind: &str, name: &str) -> String {
    let tag = format!("{kind}-{name}");
    source_url
        .replace("{0}", kind)
        .replace("{1}", &tag)
        .replace("{2}", name)
}

fn download_request(url: &str, options: &AssetAcquisitionOptions) -> DownloadRequest {
    DownloadRequest {
        url: url.to_string(),
        user_agent: Some(USER_AGENT_PREFIX.to_string()),
        prefer_proxy: options.prefer_proxy,
        proxy_url: options.proxy_url.clone(),
        response_body_limit: Some(RULESET_ASSET_RESPONSE_LIMIT_BYTES),
    }
}

fn validate_asset(file_name: &str, body: &[u8], allowed_extensions: &[&str]) -> Result<u64> {
    let path = Path::new(file_name);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(RulesetGeoError::InvalidAsset {
            path: path.to_path_buf(),
            reason: "file name must not contain directories".to_string(),
        });
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !allowed_extensions.contains(&extension) {
        return Err(RulesetGeoError::InvalidAsset {
            path: path.to_path_buf(),
            reason: format!("unexpected extension {extension:?}"),
        });
    }
    if body.is_empty() {
        return Err(RulesetGeoError::InvalidAsset {
            path: path.to_path_buf(),
            reason: "empty file".to_string(),
        });
    }

    Ok(u64::try_from(body.len()).unwrap_or(u64::MAX))
}

fn collect_srs_from_rule(
    rule: &RulesItem,
    geoip: &mut BTreeSet<String>,
    geosite: &mut BTreeSet<String>,
) {
    if let Some(items) = &rule.ip {
        for item in items {
            if let Some(value) = item.strip_prefix("geoip:").and_then(nonempty_value) {
                geoip.insert(value.to_string());
            }
        }
    }
    if let Some(items) = &rule.domain {
        for item in items {
            if let Some(value) = item.strip_prefix("geosite:").and_then(nonempty_value) {
                geosite.insert(value.to_string());
            }
        }
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn nonempty_value(value: &str) -> Option<&str> {
    nonempty(Some(value))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use tokio::sync::Mutex;
    use voya_core::{RoutingItem, RuleType, RulesItem};

    use crate::download::test_support::{
        spawn_http_bytes_fixture as spawn_http_fixture, spawn_raw_http_fixture, RawFixtureResponse,
    };

    use super::*;

    #[test]
    fn ruleset_manifest_expands_geo_and_srs_assets() {
        let manifest = RulesetGeoManifest::from_json(
            r#"{
                "geo": [
                    { "name": "geosite" },
                    { "name": "country", "fileName": "Country.mmdb", "url": "https://cdn.example/Country.mmdb" }
                ],
                "srs": [
                    { "kind": "geosite", "name": "cn" },
                    { "kind": "geoip", "name": "private", "tag": "geoip-private-custom", "fileName": "geoip-private-custom.srs" }
                ]
            }"#,
        )
        .expect("manifest");

        let geo = manifest.geo_assets(Some("https://rules.example/{0}.dat"));
        assert_eq!(geo[0].url, "https://rules.example/geosite.dat");
        assert_eq!(geo[1].file_name, "Country.mmdb");

        let srs = manifest.srs_assets(Some("https://rules.example/{0}/{1}.srs"));
        assert_eq!(srs[0].url, "https://rules.example/geosite/geosite-cn.srs");
        assert_eq!(srs[1].tag, "geoip-private-custom");
        assert_eq!(srs[1].file_name, "geoip-private-custom.srs");
    }

    #[test]
    fn ruleset_collection_reads_routing_references() {
        let routing = RoutingItem {
            rule_set: vec![RulesItem {
                ip: Some(vec!["geoip:private".to_string(), "1.1.1.1".to_string()]),
                domain: Some(vec!["geosite:cn".to_string()]),
                rule_type: Some(RuleType::Routing),
                ..RulesItem::default()
            }],
            ..RoutingItem::default()
        };
        let assets =
            collect_singbox_ruleset_assets(Some("https://rules.example/{0}/{1}.srs"), &[routing]);
        let names = assets
            .iter()
            .map(|asset| asset.file_name.as_str())
            .collect::<BTreeSet<_>>();

        assert!(names.contains("geoip-private.srs"));
        assert!(names.contains("geosite-cn.srs"));
        assert!(names.contains("geosite-google.srs"));
        assert!(names.contains("geosite-category-ads-all.srs"));
    }

    #[tokio::test]
    async fn ruleset_client_downloads_assets_through_proxy_to_direct_fallback() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([
                ("/geosite.dat".to_string(), b"geosite-dat".to_vec()),
                ("/geosite-cn.srs".to_string(), b"srs-binary".to_vec()),
            ]),
            2,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let target_root = unique_temp_root("ruleset-download");
        let options = AssetAcquisitionOptions {
            prefer_proxy: true,
            proxy_url: Some("http://127.0.0.1:9".to_string()),
        };
        let client = RulesetGeoClient::new();

        let geo = client
            .acquire_geo_assets(
                &[GeoAsset::new(
                    "geosite",
                    "geosite.dat",
                    format!("{base}/geosite.dat"),
                )],
                &target_root,
                &options,
            )
            .await
            .expect("geo asset");
        let srs = client
            .acquire_srs_assets(
                &[SrsAsset {
                    kind: "geosite".to_string(),
                    name: "cn".to_string(),
                    tag: "geosite-cn".to_string(),
                    file_name: "geosite-cn.srs".to_string(),
                    url: format!("{base}/geosite-cn.srs"),
                }],
                target_root.join("srss"),
                &options,
            )
            .await
            .expect("srs asset");

        assert!(!geo[0].used_proxy);
        assert_eq!(geo[0].attempts.len(), 2);
        assert_eq!(srs[0].bytes, 10);
        assert!(target_root.join("geosite.dat").exists());
        assert!(target_root.join("srss/geosite-cn.srs").exists());
        assert_eq!(
            seen_user_agents.lock().await.as_slice(),
            [USER_AGENT_PREFIX, USER_AGENT_PREFIX]
        );

        let _ = fs::remove_dir_all(target_root);
    }

    #[tokio::test]
    async fn failed_asset_batch_preserves_existing_files() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([("/geosite.dat".to_string(), b"new-geosite".to_vec())]),
            2,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let target_root = unique_temp_root("ruleset-atomic");
        fs::create_dir_all(&target_root).expect("target directory");
        fs::write(target_root.join("geosite.dat"), b"old-geosite").expect("old geosite");
        fs::write(target_root.join("geoip.dat"), b"old-geoip").expect("old geoip");

        let error = RulesetGeoClient::new()
            .acquire_geo_assets(
                &[
                    GeoAsset::new("geosite", "geosite.dat", format!("{base}/geosite.dat")),
                    GeoAsset::new("geoip", "geoip.dat", format!("{base}/missing.dat")),
                ],
                &target_root,
                &AssetAcquisitionOptions::default(),
            )
            .await
            .expect_err("incomplete batch should fail");

        assert!(matches!(error, RulesetGeoError::Download(_)));
        assert_eq!(
            fs::read(target_root.join("geosite.dat")).expect("geosite"),
            b"old-geosite"
        );
        assert_eq!(
            fs::read(target_root.join("geoip.dat")).expect("geoip"),
            b"old-geoip"
        );
        assert!(fs::read_dir(&target_root)
            .expect("target directory")
            .all(|entry| !entry
                .expect("target entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".voya-stage-")));

        let _ = fs::remove_dir_all(target_root);
    }

    #[test]
    fn asset_file_names_cannot_escape_the_target_directory() {
        assert!(validate_asset("../geoip.dat", b"data", &["dat"]).is_err());
        assert!(validate_asset("nested/geoip.dat", b"data", &["dat"]).is_err());
        assert_eq!(
            validate_asset("geoip.dat", b"data", &["dat"]).expect("valid asset"),
            4
        );
    }

    #[tokio::test]
    async fn ruleset_client_rejects_asset_response_above_limit() {
        let declared_length = RULESET_ASSET_RESPONSE_LIMIT_BYTES + 1;
        let base = spawn_raw_http_fixture(
            HashMap::from([(
                "/geosite.dat".to_string(),
                RawFixtureResponse {
                    status: "200 OK".to_string(),
                    content_length: Some(declared_length),
                    extra_headers: Vec::new(),
                    body: b"dat".to_vec(),
                },
            )]),
            1,
        )
        .await;
        let target_root = unique_temp_root("ruleset-oversize");
        let client = RulesetGeoClient::new();

        let error = client
            .acquire_geo_assets(
                &[GeoAsset::new(
                    "geosite",
                    "geosite.dat",
                    format!("{base}/geosite.dat"),
                )],
                &target_root,
                &AssetAcquisitionOptions::default(),
            )
            .await
            .expect_err("oversized ruleset asset should fail");

        match error {
            RulesetGeoError::Download(DownloadError::ResponseTooLarge {
                limit,
                content_length,
                received,
                ..
            }) => {
                assert_eq!(limit, RULESET_ASSET_RESPONSE_LIMIT_BYTES);
                assert_eq!(
                    content_length,
                    Some(u64::try_from(declared_length).expect("declared length"))
                );
                assert_eq!(received, 0);
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(!target_root.join("geosite.dat").exists());

        let _ = fs::remove_dir_all(target_root);
    }

    #[test]
    fn ruleset_local_discovery_maps_srs_stems_to_paths() {
        let root = unique_temp_root("ruleset-local");
        let srs_dir = root.join("srss");
        fs::create_dir_all(&srs_dir).expect("srs dir");
        fs::write(srs_dir.join("geosite-cn.srs"), b"srs").expect("srs");
        fs::write(srs_dir.join("ignored.txt"), b"txt").expect("txt");

        let paths = discover_local_singbox_ruleset_paths(&srs_dir);

        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths.get("geosite-cn"),
            Some(
                &srs_dir
                    .join("geosite-cn.srs")
                    .to_string_lossy()
                    .into_owned()
            )
        );

        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("voyavpn-{name}-{}-{nanos}", std::process::id()))
    }
}
