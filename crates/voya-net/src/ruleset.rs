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
/// Bytes that only ever start a text response. A captive portal, a proxy block page or a CDN
/// error page answers 200 with HTML, and a rule set never begins like this.
const TEXTUAL_BODY_PREFIXES: &[&[u8]] = &[b"<", b"\xef\xbb\xbf<", b"{", b"HTTP/"];

const DEFAULT_SRS_GEOSITE_TAGS: &[&str] = &["google", "cn", "geolocation-cn", "category-ads-all"];

#[derive(Debug, Error)]
pub enum RulesetError {
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

pub type Result<T> = std::result::Result<T, RulesetError>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetAcquisitionOptions {
    pub prefer_proxy: bool,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquiredRuleset {
    /// The rule set's tag (`geosite-cn`).
    pub name: String,
    pub file_name: String,
    pub url: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub used_proxy: bool,
    pub attempts: Vec<DownloadAttempt>,
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

#[derive(Debug, Clone, Default)]
pub struct RulesetClient {
    download: DownloadClient,
}

impl RulesetClient {
    pub fn new() -> Self {
        Self {
            download: DownloadClient::new(),
        }
    }

    /// Stages every rule set, then publishes the whole batch or none of it.
    pub async fn acquire_srs_assets(
        &self,
        assets: &[SrsAsset],
        target_dir: impl AsRef<Path>,
        options: &AssetAcquisitionOptions,
    ) -> Result<Vec<AcquiredRuleset>> {
        let target_dir = target_dir.as_ref();
        let staging = StagingDir::create(target_dir).await?;
        let mut acquired = Vec::new();

        for asset in assets {
            let staged = self
                .stage_asset(&asset.url, &asset.file_name, staging.path(), options)
                .await?;
            acquired.push(AcquiredRuleset {
                name: asset.tag.clone(),
                file_name: asset.file_name.clone(),
                url: asset.url.clone(),
                path: target_dir.join(&asset.file_name),
                bytes: staged.bytes,
                used_proxy: staged.used_proxy,
                attempts: staged.attempts,
            });
        }

        staging
            .commit(
                target_dir,
                assets.iter().map(|asset| asset.file_name.as_str()),
            )
            .await?;
        Ok(acquired)
    }

    async fn stage_asset(
        &self,
        url: &str,
        file_name: &str,
        staging_dir: &Path,
        options: &AssetAcquisitionOptions,
    ) -> Result<StagedAsset> {
        let response = self
            .download
            .download_bytes(download_request(url, options))
            .await?;
        let bytes = validate_asset(file_name, &response.body)?;
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

fn asset_io(path: &Path, source: std::io::Error) -> RulesetError {
    RulesetError::AssetIo {
        path: path.to_path_buf(),
        source,
    }
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

/// Checks that a downloaded body is a rule set before it can replace a live file.
///
/// `download_bytes` treats any non-empty 2xx body as a success, so a captive portal or a proxy
/// block page would otherwise be committed over a working `geosite-cn.srs` and leave the core
/// unable to start.
fn validate_asset(file_name: &str, body: &[u8]) -> Result<u64> {
    let path = Path::new(file_name);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(RulesetError::InvalidAsset {
            path: path.to_path_buf(),
            reason: "file name must not contain directories".to_string(),
        });
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if extension != "srs" {
        return Err(RulesetError::InvalidAsset {
            path: path.to_path_buf(),
            reason: format!("unexpected extension {extension:?}"),
        });
    }
    if body.is_empty() {
        return Err(RulesetError::InvalidAsset {
            path: path.to_path_buf(),
            reason: "empty file".to_string(),
        });
    }
    if let Err(reason) = validate_srs_content(body) {
        return Err(RulesetError::InvalidAsset {
            path: path.to_path_buf(),
            reason,
        });
    }

    Ok(u64::try_from(body.len()).unwrap_or(u64::MAX))
}

fn validate_srs_content(body: &[u8]) -> std::result::Result<(), String> {
    if let Some(prefix) = TEXTUAL_BODY_PREFIXES
        .iter()
        .copied()
        .find(|prefix| body.starts_with(prefix))
    {
        return Err(format!(
            "expected a binary rule set but the response starts with {:?}",
            String::from_utf8_lossy(prefix)
        ));
    }
    body.starts_with(SRS_MAGIC)
        .then_some(())
        .ok_or_else(|| "sing-box rule sets must start with the SRS magic bytes".to_string())
}

/// The `geoip:` name the config generator turns into `ip_is_private`.
const GEOIP_PRIVATE_NAME: &str = "private";

fn collect_srs_from_rule(
    rule: &RulesItem,
    geoip: &mut BTreeSet<String>,
    geosite: &mut BTreeSet<String>,
) {
    if let Some(items) = &rule.ip {
        for item in items {
            // `geoip:private` is generated as `ip_is_private`, never as a rule
            // set, so its `.srs` would be downloaded and never read.
            if let Some(value) = nonempty_str(item.strip_prefix("geoip:"))
                .filter(|value| *value != GEOIP_PRIVATE_NAME)
            {
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
    use std::{collections::HashMap, sync::Arc};

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

    fn srs(base: &str, name: &str) -> SrsAsset {
        SrsAsset::new(&format!("{base}/{{1}}.srs"), "geosite", name)
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

        assert!(
            !names.contains("geoip-private.srs"),
            "geoip:private is generated as ip_is_private and needs no rule set"
        );
        assert!(names.contains("geosite-cn.srs"));
        assert!(names.contains("geosite-google.srs"));
        assert!(names.contains("geosite-category-ads-all.srs"));
    }

    #[tokio::test]
    async fn ruleset_client_downloads_assets_through_proxy_to_direct_fallback() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([("/geosite-cn.srs".to_string(), b"SRS-binary".to_vec())]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let target_root = unique_temp_root("ruleset-download");
        let options = AssetAcquisitionOptions {
            prefer_proxy: true,
            proxy_url: Some("http://127.0.0.1:9".to_string()),
        };

        let srs = RulesetClient::new()
            .acquire_srs_assets(&[srs(&base, "cn")], target_root.join("srss"), &options)
            .await
            .expect("srs asset");

        assert!(!srs[0].used_proxy);
        assert_eq!(srs[0].attempts.len(), 2);
        assert_eq!(srs[0].name, "geosite-cn");
        assert_eq!(srs[0].bytes, 10);
        assert!(target_root.join("srss/geosite-cn.srs").exists());
        // The proxy attempt never reaches the fixture; the direct fallback does.
        assert_eq!(
            seen_user_agents.lock().await.as_slice(),
            [USER_AGENT_PREFIX]
        );

        let _ = fs::remove_dir_all(target_root);
    }

    #[tokio::test]
    async fn failed_asset_batch_preserves_existing_files() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([("/geosite-cn.srs".to_string(), b"SRS-new-cn".to_vec())]),
            2,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let target_root = unique_temp_root("ruleset-atomic");
        fs::create_dir_all(&target_root).expect("target directory");
        fs::write(target_root.join("geosite-cn.srs"), b"SRS-old-cn").expect("old cn");
        fs::write(target_root.join("geosite-google.srs"), b"SRS-old-google").expect("old google");

        // The fixture serves only the first, so the batch fails part way.
        let error = RulesetClient::new()
            .acquire_srs_assets(
                &[srs(&base, "cn"), srs(&base, "google")],
                &target_root,
                &AssetAcquisitionOptions::default(),
            )
            .await
            .expect_err("incomplete batch should fail");

        assert!(matches!(error, RulesetError::Download(_)));
        assert_eq!(
            fs::read(target_root.join("geosite-cn.srs")).expect("cn"),
            b"SRS-old-cn"
        );
        assert_eq!(
            fs::read(target_root.join("geosite-google.srs")).expect("google"),
            b"SRS-old-google"
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
        fs::write(target_root.join("a.srs"), b"old-a").expect("old geosite");
        fs::write(target_root.join("b.srs"), b"old-b").expect("old geoip");

        let staging = StagingDir::create(&target_root)
            .await
            .expect("staging directory");
        fs::write(staging.path().join("a.srs"), b"new-a").expect("staged geosite");
        fs::write(staging.path().join("c.srs"), b"new-c").expect("staged country");
        // "b.srs" is deliberately absent from staging, so publishing it fails after the two
        // earlier assets have already been moved onto their live paths.

        let error = staging
            .commit(&target_root, ["a.srs", "c.srs", "b.srs"].into_iter())
            .await
            .expect_err("commit of a missing staged asset should fail");

        assert!(matches!(error, RulesetError::AssetIo { .. }), "{error:?}");
        assert_eq!(
            fs::read(target_root.join("a.srs")).expect("geosite"),
            b"old-a"
        );
        assert_eq!(
            fs::read(target_root.join("b.srs")).expect("geoip"),
            b"old-b"
        );
        assert!(!target_root.join("c.srs").exists());

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
        fs::write(target_root.join("a.srs"), b"old-a").expect("old geosite");

        let staging = StagingDir::create(&target_root)
            .await
            .expect("staging directory");
        fs::write(staging.path().join("a.srs"), b"new-a").expect("staged geosite");
        fs::write(staging.path().join("b.srs"), b"new-b").expect("staged geoip");

        staging
            .commit(&target_root, ["a.srs", "b.srs"].into_iter())
            .await
            .expect("commit");

        assert_eq!(
            fs::read(target_root.join("a.srs")).expect("geosite"),
            b"new-a"
        );
        assert_eq!(
            fs::read(target_root.join("b.srs")).expect("geoip"),
            b"new-b"
        );

        let _ = fs::remove_dir_all(target_root);
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
                validate_asset("geosite-cn.srs", body).is_err(),
                "textual body should not pass as a .srs asset"
            );
        }

        assert!(
            validate_asset("geosite-cn.srs", b"\x00binary-but-not-srs").is_err(),
            "a .srs asset without the SRS magic should be rejected"
        );
        validate_asset("geosite-cn.srs", b"SRS\x03\x00rules").expect("valid rule set");
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

        let error = RulesetClient::new()
            .acquire_srs_assets(
                &[srs(&base, "cn")],
                &target_root,
                &AssetAcquisitionOptions::default(),
            )
            .await
            .expect_err("an HTML body should not be accepted as a rule set");

        assert!(
            matches!(error, RulesetError::InvalidAsset { .. }),
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
                "/geosite-cn.srs".to_string(),
                RawFixtureResponse {
                    status: "200 OK".to_string(),
                    content_length: Some(declared_length),
                    extra_headers: Vec::new(),
                    body: b"SRS".to_vec(),
                },
            )]),
            1,
        )
        .await;
        let target_root = unique_temp_root("ruleset-oversize");
        let client = RulesetClient::new();

        let error = client
            .acquire_srs_assets(
                &[srs(&base, "cn")],
                &target_root,
                &AssetAcquisitionOptions::default(),
            )
            .await
            .expect_err("oversized ruleset asset should fail");

        match error {
            RulesetError::Download(DownloadError::ResponseTooLarge {
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
        assert!(!target_root.join("geosite-cn.srs").exists());

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
        tempfile::Builder::new()
            .prefix(&format!("voyavpn-{name}-"))
            .tempdir()
            .expect("ruleset test temp dir")
            .keep()
    }
}
