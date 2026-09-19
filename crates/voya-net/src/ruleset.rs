use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use thiserror::Error;
use voya_core::{text::nonempty_str, RoutingItem, RulesItem, DEFAULT_SINGBOX_RULESET_URL};

use crate::{DownloadAttempt, DownloadClient, DownloadError, DownloadRequest};

const RULESET_ASSET_RESPONSE_LIMIT_BYTES: usize = 256 * 1024 * 1024;
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// First bytes of a sing-box binary rule set (`common/srs`).
const SRS_MAGIC: &[u8] = b"SRS";
/// Metadata marker every MaxMind database carries near its end.
const MMDB_METADATA_MARKER: &[u8] = b"\xab\xcd\xefMaxMind.com";
/// Bytes that only ever start a text response. A captive portal, a proxy block page or a CDN
/// error page answers 200 with HTML, and none of the accepted asset formats begin like this.
const TEXTUAL_BODY_PREFIXES: &[&[u8]] = &[b"<", b"\xef\xbb\xbf<", b"{", b"HTTP/"];

const DEFAULT_GEO_SOURCE_URL: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/{0}.dat";
const OTHER_GEO_URLS: &[&str] = &[
    "https://raw.githubusercontent.com/Loyalsoldier/geoip/release/geoip-only-cn-private.dat",
    "https://raw.githubusercontent.com/Loyalsoldier/geoip/release/Country.mmdb",
];
const DEFAULT_SRS_GEOSITE_TAGS: &[&str] = &["google", "cn", "geolocation-cn", "category-ads-all"];

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

/// What acquiring an asset needs to know about its kind.
trait AssetSpec {
    const KIND: AcquiredAssetKind;
    /// Extensions a downloaded body of this kind may carry.
    const EXTENSIONS: &'static [&'static str];

    /// The name the acquired asset is reported under.
    fn acquired_name(&self) -> &str;
    fn file_name(&self) -> &str;
    fn url(&self) -> &str;
}

impl AssetSpec for GeoAsset {
    const KIND: AcquiredAssetKind = AcquiredAssetKind::Geo;
    const EXTENSIONS: &'static [&'static str] = &["dat", "mmdb", "metadb"];

    fn acquired_name(&self) -> &str {
        &self.name
    }

    fn file_name(&self) -> &str {
        &self.file_name
    }

    fn url(&self) -> &str {
        &self.url
    }
}

impl AssetSpec for SrsAsset {
    const KIND: AcquiredAssetKind = AcquiredAssetKind::Ruleset;
    const EXTENSIONS: &'static [&'static str] = &["srs"];

    /// A rule set is reported by its tag (`geosite-cn`), not its bare name.
    fn acquired_name(&self) -> &str {
        &self.tag
    }

    fn file_name(&self) -> &str {
        &self.file_name
    }

    fn url(&self) -> &str {
        &self.url
    }
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
        self.acquire_assets(assets, target_dir.as_ref(), options)
            .await
    }

    pub async fn acquire_srs_assets(
        &self,
        assets: &[SrsAsset],
        target_dir: impl AsRef<Path>,
        options: &AssetAcquisitionOptions,
    ) -> Result<Vec<AcquiredRulesetGeoAsset>> {
        self.acquire_assets(assets, target_dir.as_ref(), options)
            .await
    }

    /// Stages every asset, then publishes the whole batch or none of it.
    async fn acquire_assets<A: AssetSpec>(
        &self,
        assets: &[A],
        target_dir: &Path,
        options: &AssetAcquisitionOptions,
    ) -> Result<Vec<AcquiredRulesetGeoAsset>> {
        let staging = StagingDir::create(target_dir).await?;
        let mut acquired = Vec::new();

        for asset in assets {
            let staged = self
                .stage_asset(
                    asset.url(),
                    asset.file_name(),
                    staging.path(),
                    A::EXTENSIONS,
                    options,
                )
                .await?;
            acquired.push(AcquiredRulesetGeoAsset {
                kind: A::KIND,
                name: asset.acquired_name().to_string(),
                file_name: asset.file_name().to_string(),
                url: asset.url().to_string(),
                path: target_dir.join(asset.file_name()),
                bytes: staged.bytes,
                used_proxy: staged.used_proxy,
                attempts: staged.attempts,
            });
        }

        staging
            .commit(target_dir, assets.iter().map(A::file_name))
            .await?;
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
        // Asset bodies run to hundreds of megabytes, so the write goes through tokio's blocking
        // pool instead of stalling the worker that also serves IPC and the statistics stream.
        if let Some(parent) = staged_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|source| asset_io(parent, source))?;
        }
        tokio::fs::write(&staged_path, response.body)
            .await
            .map_err(|source| asset_io(&staged_path, source))?;

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
    async fn create(target_dir: &Path) -> Result<Self> {
        tokio::fs::create_dir_all(target_dir)
            .await
            .map_err(|source| asset_io(target_dir, source))?;
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = target_dir.join(format!(".voya-stage-{}-{sequence}", std::process::id()));
        tokio::fs::create_dir(&path)
            .await
            .map_err(|source| asset_io(&path, source))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    /// Moves every staged asset onto its live path.
    ///
    /// Staging lives inside `target_dir`, so each publish is a same-directory `rename`: the live
    /// file is never observed truncated. Any live file is first renamed aside into the staging
    /// directory, which keeps the batch all-or-nothing — if one asset fails to publish, the
    /// already published ones are rolled back to the files they replaced.
    async fn commit<'a>(
        &self,
        target_dir: &Path,
        file_names: impl Iterator<Item = &'a str>,
    ) -> Result<()> {
        let mut published: Vec<PublishedAsset> = Vec::new();

        for file_name in file_names {
            let staged = self.path.join(file_name);
            let target = target_dir.join(file_name);
            let replaced = match self.move_aside(file_name, &target).await {
                Ok(replaced) => replaced,
                Err(error) => {
                    rollback(&published).await;
                    return Err(error);
                }
            };
            if let Err(source) = tokio::fs::rename(&staged, &target).await {
                let error = asset_io(&target, source);
                if let Some(replaced) = replaced {
                    restore(&replaced, &target).await;
                }
                rollback(&published).await;
                return Err(error);
            }
            published.push(PublishedAsset { target, replaced });
        }

        Ok(())
    }

    /// Renames an existing live asset into the staging directory so it can be restored.
    ///
    /// Returns `None` when there was no live file to preserve.
    async fn move_aside(&self, file_name: &str, target: &Path) -> Result<Option<PathBuf>> {
        let replaced = self.path.join(format!("{file_name}.voya-replaced"));
        match tokio::fs::rename(target, &replaced).await {
            Ok(()) => Ok(Some(replaced)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(asset_io(target, source)),
        }
    }
}

struct PublishedAsset {
    target: PathBuf,
    replaced: Option<PathBuf>,
}

/// Undoes a partly published batch. A step that fails leaves that asset as the
/// new file or missing; the caller still gets the original error, so the
/// failure is only logged.
async fn rollback(published: &[PublishedAsset]) {
    for asset in published.iter().rev() {
        match &asset.replaced {
            Some(replaced) => restore(replaced, &asset.target).await,
            None => {
                if let Err(error) = tokio::fs::remove_file(&asset.target).await {
                    tracing::warn!(
                        path = %asset.target.display(),
                        %error,
                        "failed to remove a published asset while rolling back"
                    );
                }
            }
        }
    }
}

/// Moves the file an asset replaced back onto its live path.
async fn restore(replaced: &Path, target: &Path) {
    if let Err(error) = tokio::fs::rename(replaced, target).await {
        tracing::warn!(
            path = %target.display(),
            %error,
            "failed to restore a replaced asset while rolling back"
        );
    }
}

impl Drop for StagingDir {
    /// Best-effort cleanup of the leftover staging directory. It only ever holds the backups of
    /// files already renamed onto their live paths, so this is a handful of syscalls rather than
    /// the multi-megabyte writes that `stage_asset` keeps off the runtime thread.
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
    let source_url = nonempty_str(source_url);
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

    let source_url = nonempty_str(source_url).unwrap_or(DEFAULT_SINGBOX_RULESET_URL);
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

fn format_geo_url(source_url: &str, name: &str) -> String {
    source_url.replace("{0}", name).replace("{name}", name)
}

fn format_srs_url(source_url: &str, kind: &str, name: &str) -> String {
    let tag = format!("{kind}-{name}");
    source_url
        .replace("{0}", kind)
        .replace("{1}", &tag)
        .replace("{2}", name)
}

fn download_request(url: &str, options: &AssetAcquisitionOptions) -> DownloadRequest {
    DownloadRequest {
        url: url.to_string(),
        user_agent: None,
        prefer_proxy: options.prefer_proxy,
        proxy_url: options.proxy_url.clone(),
        response_body_limit: Some(RULESET_ASSET_RESPONSE_LIMIT_BYTES),
    }
}

/// Checks that a downloaded body is the asset it claims to be before it can replace a live file.
///
/// `download_bytes` treats any non-empty 2xx body as a success, so a captive portal or a proxy
/// block page would otherwise be committed over a working `geosite-cn.srs` and leave the core
/// unable to start. Each accepted extension therefore carries a cheap structural check.
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
    if let Err(reason) = validate_asset_content(extension, body) {
        return Err(RulesetGeoError::InvalidAsset {
            path: path.to_path_buf(),
            reason,
        });
    }

    Ok(u64::try_from(body.len()).unwrap_or(u64::MAX))
}

fn validate_asset_content(extension: &str, body: &[u8]) -> std::result::Result<(), String> {
    let textual_prefix = TEXTUAL_BODY_PREFIXES
        .iter()
        .copied()
        .find(|prefix| body.starts_with(prefix));
    if let Some(prefix) = textual_prefix {
        return Err(format!(
            "expected binary {extension} data but the response starts with {:?}",
            String::from_utf8_lossy(prefix)
        ));
    }

    match extension {
        "srs" => body
            .starts_with(SRS_MAGIC)
            .then_some(())
            .ok_or_else(|| "sing-box rule sets must start with the SRS magic bytes".to_string()),
        "mmdb" | "metadb" => contains(body, MMDB_METADATA_MARKER)
            .then_some(())
            .ok_or_else(|| "MaxMind databases must carry the metadata marker".to_string()),
        // v2ray `.dat` files are protobuf, which has no fixed magic; rejecting textual bodies
        // above is the strongest check available without parsing the schema.
        _ => Ok(()),
    }
}

/// Reports whether `haystack` contains `needle`. The MaxMind marker sits in the trailing
/// metadata section, so only the tail of a large database is scanned.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    const MMDB_METADATA_TAIL_BYTES: usize = 128 * 1024;
    let tail = &haystack[haystack.len().saturating_sub(MMDB_METADATA_TAIL_BYTES)..];

    tail.windows(needle.len()).any(|window| window == needle)
}

fn collect_srs_from_rule(
    rule: &RulesItem,
    geoip: &mut BTreeSet<String>,
    geosite: &mut BTreeSet<String>,
) {
    if let Some(items) = &rule.ip {
        for item in items {
            if let Some(value) = nonempty_str(item.strip_prefix("geoip:")) {
                geoip.insert(value.to_string());
            }
        }
    }
    if let Some(items) = &rule.domain {
        for item in items {
            if let Some(value) = nonempty_str(item.strip_prefix("geosite:")) {
                geosite.insert(value.to_string());
            }
        }
    }
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

    use crate::download::{
        test_support::{
            spawn_http_bytes_fixture as spawn_http_fixture, spawn_raw_http_fixture,
            RawFixtureResponse,
        },
        USER_AGENT_PREFIX,
    };

    use super::*;

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
                ("/geosite-cn.srs".to_string(), b"SRS-binary".to_vec()),
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

    #[tokio::test]
    async fn failed_commit_restores_every_replaced_asset() {
        let target_root = unique_temp_root("ruleset-commit-rollback");
        fs::create_dir_all(&target_root).expect("target directory");
        fs::write(target_root.join("geosite.dat"), b"old-geosite").expect("old geosite");
        fs::write(target_root.join("geoip.dat"), b"old-geoip").expect("old geoip");

        let staging = StagingDir::create(&target_root)
            .await
            .expect("staging directory");
        fs::write(staging.path().join("geosite.dat"), b"new-geosite").expect("staged geosite");
        fs::write(staging.path().join("Country.mmdb"), b"new-country").expect("staged country");
        // "geoip.dat" is deliberately absent from staging, so publishing it fails after the two
        // earlier assets have already been moved onto their live paths.

        let error = staging
            .commit(
                &target_root,
                ["geosite.dat", "Country.mmdb", "geoip.dat"].into_iter(),
            )
            .await
            .expect_err("commit of a missing staged asset should fail");

        assert!(
            matches!(error, RulesetGeoError::AssetIo { .. }),
            "{error:?}"
        );
        assert_eq!(
            fs::read(target_root.join("geosite.dat")).expect("geosite"),
            b"old-geosite"
        );
        assert_eq!(
            fs::read(target_root.join("geoip.dat")).expect("geoip"),
            b"old-geoip"
        );
        assert!(!target_root.join("Country.mmdb").exists());

        drop(staging);
        assert!(fs::read_dir(&target_root)
            .expect("target directory")
            .all(|entry| !entry
                .expect("target entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".voya-stage-")));

        let _ = fs::remove_dir_all(target_root);
    }

    #[tokio::test]
    async fn commit_publishes_every_staged_asset() {
        let target_root = unique_temp_root("ruleset-commit");
        fs::create_dir_all(&target_root).expect("target directory");
        fs::write(target_root.join("geosite.dat"), b"old-geosite").expect("old geosite");

        let staging = StagingDir::create(&target_root)
            .await
            .expect("staging directory");
        fs::write(staging.path().join("geosite.dat"), b"new-geosite").expect("staged geosite");
        fs::write(staging.path().join("geoip.dat"), b"new-geoip").expect("staged geoip");

        staging
            .commit(&target_root, ["geosite.dat", "geoip.dat"].into_iter())
            .await
            .expect("commit");

        assert_eq!(
            fs::read(target_root.join("geosite.dat")).expect("geosite"),
            b"new-geosite"
        );
        assert_eq!(
            fs::read(target_root.join("geoip.dat")).expect("geoip"),
            b"new-geoip"
        );

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

    /// Captive portals and proxy block pages answer 200 with HTML, which `download_bytes` reports
    /// as a successful non-empty body, so the shape of the payload has to be checked here.
    #[test]
    fn asset_validation_rejects_block_pages_and_wrong_formats() {
        for body in [
            b"<!DOCTYPE html><html><body>blocked</body></html>".as_slice(),
            b"\xef\xbb\xbf<html>blocked</html>".as_slice(),
            b"{\"message\":\"not found\"}".as_slice(),
        ] {
            assert!(
                validate_asset("geosite.dat", body, &["dat"]).is_err(),
                "textual body should not pass as a .dat asset"
            );
            assert!(
                validate_asset("geosite-cn.srs", body, &["srs"]).is_err(),
                "textual body should not pass as a .srs asset"
            );
        }

        assert!(
            validate_asset("geosite-cn.srs", b"\x00binary-but-not-srs", &["srs"]).is_err(),
            "a .srs asset without the SRS magic should be rejected"
        );
        assert!(
            validate_asset("Country.mmdb", b"\x00binary-but-not-maxmind", &["mmdb"]).is_err(),
            "an .mmdb asset without the metadata marker should be rejected"
        );

        validate_asset("geosite-cn.srs", b"SRS\x03\x00rules", &["srs"]).expect("valid rule set");
        validate_asset("geosite.dat", b"\x0a\x05china", &["dat"]).expect("valid protobuf dat");
        validate_asset(
            "Country.mmdb",
            b"\x00records\xab\xcd\xefMaxMind.com\x00",
            &["mmdb"],
        )
        .expect("valid MaxMind database");
    }

    #[tokio::test]
    async fn html_block_page_never_replaces_a_working_ruleset() {
        let base = spawn_http_fixture(
            HashMap::from([(
                "/geosite-cn.srs".to_string(),
                b"<html><body>Access denied by network policy</body></html>".to_vec(),
            )]),
            1,
            Arc::new(Mutex::new(Vec::new())),
        )
        .await;
        let target_root = unique_temp_root("ruleset-block-page");
        fs::create_dir_all(&target_root).expect("target directory");
        fs::write(target_root.join("geosite-cn.srs"), b"SRS\x03good").expect("live rule set");

        let error = RulesetGeoClient::new()
            .acquire_srs_assets(
                &[SrsAsset {
                    kind: "geosite".to_string(),
                    name: "cn".to_string(),
                    tag: "geosite-cn".to_string(),
                    file_name: "geosite-cn.srs".to_string(),
                    url: format!("{base}/geosite-cn.srs"),
                }],
                &target_root,
                &AssetAcquisitionOptions::default(),
            )
            .await
            .expect_err("an HTML body should not be accepted as a rule set");

        assert!(
            matches!(error, RulesetGeoError::InvalidAsset { .. }),
            "{error:?}"
        );
        assert_eq!(
            fs::read(target_root.join("geosite-cn.srs")).expect("rule set"),
            b"SRS\x03good"
        );

        let _ = fs::remove_dir_all(target_root);
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
