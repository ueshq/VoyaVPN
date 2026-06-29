use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use voya_core::CoreType;

use crate::{DownloadClient, DownloadError, DownloadRequest};

const RELEASE_METADATA_RESPONSE_LIMIT_BYTES: usize = 4 * 1024 * 1024;

// Legacy GitHub release metadata stays available for fixture tests and migration
// compatibility. Production app/core checks use CDN manifests through
// `CdnUpdateClient` and reject GitHub production download URLs.
pub const VOYA_APP_RELEASES_API_URL: &str = "https://api.github.com/repos/voyavpn/voyavpn/releases";
pub const VOYA_APP_RELEASES_URL: &str = "https://github.com/voyavpn/voyavpn/releases";
pub const CDN_RELEASE_INDEX_FILE: &str = "release-index.json";
pub const CDN_CORE_ASSET_MANIFEST_FILE: &str = "core-assets.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetOs {
    Windows,
    Linux,
    Macos,
}

impl AssetOs {
    #[must_use]
    pub const fn current() -> Option<Self> {
        if cfg!(target_os = "windows") {
            Some(Self::Windows)
        } else if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else if cfg!(target_os = "macos") {
            Some(Self::Macos)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetArch {
    X64,
    Arm64,
    Riscv64,
}

impl AssetArch {
    #[must_use]
    pub const fn current() -> Option<Self> {
        if cfg!(target_arch = "x86_64") {
            Some(Self::X64)
        } else if cfg!(target_arch = "aarch64") {
            Some(Self::Arm64)
        } else if cfg!(target_arch = "riscv64") {
            Some(Self::Riscv64)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageTarget {
    App,
    Core(CoreType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryAcquisition {
    AppPackage,
    OptionalDownload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicy {
    pub license: Option<&'static str>,
    pub acquisition: BinaryAcquisition,
    pub redistribute_in_installer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrereleasePolicy {
    UserControlled,
    StableOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpstreamAssetTemplates {
    pub windows_x64: Option<&'static str>,
    pub windows_arm64: Option<&'static str>,
    pub linux_x64: Option<&'static str>,
    pub linux_arm64: Option<&'static str>,
    pub linux_riscv64: Option<&'static str>,
    pub macos_x64: Option<&'static str>,
    pub macos_arm64: Option<&'static str>,
}

impl UpstreamAssetTemplates {
    #[must_use]
    pub const fn template_for(self, os: AssetOs, arch: AssetArch) -> Option<&'static str> {
        match (os, arch) {
            (AssetOs::Windows, AssetArch::X64) => self.windows_x64,
            (AssetOs::Windows, AssetArch::Arm64) => self.windows_arm64,
            (AssetOs::Windows, AssetArch::Riscv64) => None,
            (AssetOs::Linux, AssetArch::X64) => self.linux_x64,
            (AssetOs::Linux, AssetArch::Arm64) => self.linux_arm64,
            (AssetOs::Linux, AssetArch::Riscv64) => self.linux_riscv64,
            (AssetOs::Macos, AssetArch::X64) => self.macos_x64,
            (AssetOs::Macos, AssetArch::Arm64) => self.macos_arm64,
            (AssetOs::Macos, AssetArch::Riscv64) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleasePackage {
    pub id: &'static str,
    pub name: &'static str,
    pub target: PackageTarget,
    pub prerelease_policy: PrereleasePolicy,
    pub policy: PackagePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpstreamReleaseEvidence {
    pub release_api_url: &'static str,
    pub release_url: &'static str,
    pub asset_templates: UpstreamAssetTemplates,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct GitHubReleaseAsset {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct GitHubRelease {
    #[serde(default)]
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<GitHubReleaseAsset>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedAssetSource {
    CdnReleaseIndex,
    CdnCoreManifest,
    ReleaseAsset,
    TemplateFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAsset {
    pub name: String,
    pub download_url: String,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
    pub source: ResolvedAssetSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseFetchOptions {
    pub include_prerelease: bool,
    pub prefer_proxy: bool,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseCheck {
    pub package_id: String,
    pub target: PackageTarget,
    pub current_version: Option<Version>,
    pub remote_version: Version,
    pub has_update: bool,
    pub prerelease: bool,
    pub release_url: Option<String>,
    pub asset: ResolvedAsset,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnReleaseIndex {
    #[serde(default)]
    pub product_name: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub artifacts: Vec<CdnReleaseArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnReleaseArtifact {
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub original_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub release_target: Option<String>,
    #[serde(default)]
    pub original_relative_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnCoreAssetManifest {
    #[serde(default)]
    pub product_name: String,
    #[serde(default)]
    pub manifest_version: u32,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub assets: Vec<CdnCoreAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnCoreAsset {
    #[serde(default)]
    pub core_type: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub archive_format: String,
    #[serde(default)]
    pub executable_candidates: Vec<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub upstream_url: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("failed to parse release metadata JSON: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("no matching release found")]
    NoRelease,
    #[error("release tag {0:?} does not contain a semantic version")]
    InvalidVersion(String),
    #[error("no update asset for {package_id} on {os:?}/{arch:?}")]
    UnsupportedAssetTarget {
        package_id: String,
        os: AssetOs,
        arch: AssetArch,
    },
    #[error("CDN {manifest} URL is not configured")]
    MissingCdnManifestUrl { manifest: &'static str },
    #[error("production CDN URL is not allowed: {0}")]
    ForbiddenProductionUrl(String),
}

#[derive(Debug, Clone, Default)]
pub struct GitHubReleaseClient {
    download: DownloadClient,
}

#[derive(Debug, Clone, Default)]
pub struct CdnUpdateClient {
    download: DownloadClient,
}

impl CdnUpdateClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            download: DownloadClient::new(),
        }
    }

    pub async fn fetch_release_index(
        &self,
        release_index_url: &str,
        options: &ReleaseFetchOptions,
    ) -> Result<CdnReleaseIndex, ReleaseError> {
        ensure_production_url_allowed(release_index_url)?;
        let response = self
            .download
            .download_text(DownloadRequest {
                url: release_index_url.to_string(),
                user_agent: Some(crate::USER_AGENT_PREFIX.to_string()),
                prefer_proxy: options.prefer_proxy,
                proxy_url: options.proxy_url.clone(),
                response_body_limit: Some(RELEASE_METADATA_RESPONSE_LIMIT_BYTES),
            })
            .await?;

        Ok(parse_cdn_release_index(&response.body)?)
    }

    pub async fn fetch_core_manifest(
        &self,
        core_manifest_url: &str,
        options: &ReleaseFetchOptions,
    ) -> Result<CdnCoreAssetManifest, ReleaseError> {
        ensure_production_url_allowed(core_manifest_url)?;
        let response = self
            .download
            .download_text(DownloadRequest {
                url: core_manifest_url.to_string(),
                user_agent: Some(crate::USER_AGENT_PREFIX.to_string()),
                prefer_proxy: options.prefer_proxy,
                proxy_url: options.proxy_url.clone(),
                response_body_limit: Some(RELEASE_METADATA_RESPONSE_LIMIT_BYTES),
            })
            .await?;

        Ok(parse_cdn_core_asset_manifest(&response.body)?)
    }
}

#[must_use]
pub fn cdn_manifest_url_from_base(base_url: &str, file_name: &str) -> Option<String> {
    let base_url = base_url.trim().trim_end_matches('/');
    (!base_url.is_empty()).then(|| format!("{base_url}/{file_name}"))
}

pub fn parse_cdn_release_index(input: &str) -> Result<CdnReleaseIndex, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn parse_cdn_core_asset_manifest(
    input: &str,
) -> Result<CdnCoreAssetManifest, serde_json::Error> {
    serde_json::from_str(input)
}

pub fn check_app_from_cdn_release_index(
    package: &ReleasePackage,
    current_version: Option<&Version>,
    os: AssetOs,
    arch: AssetArch,
    index: &CdnReleaseIndex,
) -> Result<ReleaseCheck, ReleaseError> {
    let artifact = resolve_cdn_release_artifact(package, os, arch, index)?;
    ensure_production_url_allowed(&artifact.url)?;
    let version = if !artifact.version.trim().is_empty() {
        artifact.version.as_str()
    } else {
        index.version.as_str()
    };
    let remote_version = parse_version(version)
        .ok_or_else(|| ReleaseError::InvalidVersion(artifact.version.clone()))?;
    let has_update = current_version.is_none_or(|current| current < &remote_version);

    Ok(ReleaseCheck {
        package_id: package.id.to_string(),
        target: package.target,
        current_version: current_version.cloned(),
        remote_version,
        has_update,
        prerelease: !index.channel.eq_ignore_ascii_case("stable"),
        release_url: (!index.base_url.trim().is_empty()).then(|| index.base_url.clone()),
        asset: ResolvedAsset {
            name: asset_name(&artifact.name, &artifact.url),
            download_url: artifact.url.clone(),
            sha256: non_empty_string(&artifact.sha256),
            bytes: (artifact.bytes > 0).then_some(artifact.bytes),
            source: ResolvedAssetSource::CdnReleaseIndex,
        },
    })
}

pub fn app_release_artifacts_for_cdn_index(
    package: &ReleasePackage,
    os: AssetOs,
    arch: AssetArch,
    index: &CdnReleaseIndex,
) -> Result<Vec<CdnReleaseArtifact>, ReleaseError> {
    let artifacts = matching_cdn_release_artifacts(package, os, arch, index)?;

    for artifact in &artifacts {
        ensure_production_url_allowed(&artifact.url)?;
    }

    Ok(artifacts.into_iter().cloned().collect())
}

pub fn check_core_from_cdn_manifest(
    package: &ReleasePackage,
    current_version: Option<&Version>,
    os: AssetOs,
    arch: AssetArch,
    manifest: &CdnCoreAssetManifest,
) -> Result<ReleaseCheck, ReleaseError> {
    let asset = resolve_cdn_core_asset(package, os, arch, manifest)?;
    ensure_production_url_allowed(&asset.url)?;
    let remote_version = parse_version(&asset.version)
        .ok_or_else(|| ReleaseError::InvalidVersion(asset.version.clone()))?;
    let has_update = current_version.is_none_or(|current| current < &remote_version);

    Ok(ReleaseCheck {
        package_id: package.id.to_string(),
        target: package.target,
        current_version: current_version.cloned(),
        remote_version,
        has_update,
        prerelease: !manifest.channel.eq_ignore_ascii_case("stable"),
        release_url: (!manifest.base_url.trim().is_empty()).then(|| manifest.base_url.clone()),
        asset: ResolvedAsset {
            name: asset_name(&asset.name, &asset.url),
            download_url: asset.url.clone(),
            sha256: non_empty_string(&asset.sha256),
            bytes: (asset.bytes > 0).then_some(asset.bytes),
            source: ResolvedAssetSource::CdnCoreManifest,
        },
    })
}

fn resolve_cdn_release_artifact<'a>(
    package: &ReleasePackage,
    os: AssetOs,
    arch: AssetArch,
    index: &'a CdnReleaseIndex,
) -> Result<&'a CdnReleaseArtifact, ReleaseError> {
    matching_cdn_release_artifacts(package, os, arch, index).and_then(|artifacts| {
        artifacts
            .into_iter()
            .next()
            .ok_or_else(|| ReleaseError::UnsupportedAssetTarget {
                package_id: package.id.to_string(),
                os,
                arch,
            })
    })
}

fn matching_cdn_release_artifacts<'a>(
    package: &ReleasePackage,
    os: AssetOs,
    arch: AssetArch,
    index: &'a CdnReleaseIndex,
) -> Result<Vec<&'a CdnReleaseArtifact>, ReleaseError> {
    if package.target != PackageTarget::App {
        return Err(ReleaseError::UnsupportedAssetTarget {
            package_id: package.id.to_string(),
            os,
            arch,
        });
    }

    let mut artifacts = index
        .artifacts
        .iter()
        .filter(|artifact| {
            manifest_os(&artifact.target) == Some(os) && manifest_arch(&artifact.arch) == Some(arch)
        })
        .collect::<Vec<_>>();
    artifacts.sort_by_key(|artifact| app_artifact_kind_rank(os, &artifact.kind));

    if artifacts.is_empty() {
        Err(ReleaseError::UnsupportedAssetTarget {
            package_id: package.id.to_string(),
            os,
            arch,
        })
    } else {
        Ok(artifacts)
    }
}

fn resolve_cdn_core_asset<'a>(
    package: &ReleasePackage,
    os: AssetOs,
    arch: AssetArch,
    manifest: &'a CdnCoreAssetManifest,
) -> Result<&'a CdnCoreAsset, ReleaseError> {
    let PackageTarget::Core(core_type) = package.target else {
        return Err(ReleaseError::UnsupportedAssetTarget {
            package_id: package.id.to_string(),
            os,
            arch,
        });
    };

    manifest
        .assets
        .iter()
        .find(|asset| {
            manifest_core_type(&asset.core_type) == Some(core_type)
                && manifest_os(&asset.os) == Some(os)
                && manifest_arch(&asset.arch) == Some(arch)
        })
        .ok_or_else(|| ReleaseError::UnsupportedAssetTarget {
            package_id: package.id.to_string(),
            os,
            arch,
        })
}

fn app_artifact_kind_rank(os: AssetOs, kind: &str) -> u8 {
    let kind = kind.trim().to_ascii_lowercase();
    match (os, kind.as_str()) {
        (AssetOs::Windows, "nsis") => 0,
        (AssetOs::Windows, "msi") => 1,
        (AssetOs::Windows, "zip") => 2,
        (AssetOs::Macos, "dmg") => 0,
        (AssetOs::Macos, "app") => 1,
        (AssetOs::Linux, "appimage") => 0,
        (AssetOs::Linux, "deb") => 1,
        (AssetOs::Linux, "rpm") => 2,
        _ => 10,
    }
}

fn manifest_os(value: &str) -> Option<AssetOs> {
    match value.trim().to_ascii_lowercase().as_str() {
        "windows" | "win" | "win32" | "msvc" => Some(AssetOs::Windows),
        "linux" => Some(AssetOs::Linux),
        "macos" | "darwin" | "osx" | "apple" => Some(AssetOs::Macos),
        _ => None,
    }
}

fn manifest_arch(value: &str) -> Option<AssetArch> {
    match value.trim().to_ascii_lowercase().as_str() {
        "x64" | "x86_64" | "amd64" => Some(AssetArch::X64),
        "arm64" | "aarch64" => Some(AssetArch::Arm64),
        "riscv64" => Some(AssetArch::Riscv64),
        _ => None,
    }
}

fn manifest_core_type(value: &str) -> Option<CoreType> {
    let _ = value;
    None
}

fn asset_name(name: &str, url: &str) -> String {
    non_empty_string(name).unwrap_or_else(|| file_name_from_url(url))
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn ensure_production_url_allowed(url: &str) -> Result<(), ReleaseError> {
    let value = url.trim();
    if value.is_empty() {
        return Err(ReleaseError::ForbiddenProductionUrl(url.to_string()));
    }

    let lower = value.to_ascii_lowercase();
    if lower.contains("placeholder")
        || lower.contains("voyavpn.example")
        || lower.contains("github.com")
        || lower.contains("githubusercontent.com")
        || lower.contains("github.io")
    {
        return Err(ReleaseError::ForbiddenProductionUrl(url.to_string()));
    }

    Ok(())
}

impl GitHubReleaseClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            download: DownloadClient::new(),
        }
    }

    pub async fn fetch_releases(
        &self,
        release_api_url: &str,
        options: &ReleaseFetchOptions,
    ) -> Result<Vec<GitHubRelease>, ReleaseError> {
        let response = self
            .download
            .download_text(DownloadRequest {
                url: release_api_url.to_string(),
                user_agent: Some(crate::USER_AGENT_PREFIX.to_string()),
                prefer_proxy: options.prefer_proxy,
                proxy_url: options.proxy_url.clone(),
                response_body_limit: Some(RELEASE_METADATA_RESPONSE_LIMIT_BYTES),
            })
            .await?;

        Ok(parse_github_releases(&response.body)?)
    }

    pub async fn check_package(
        &self,
        package: &ReleasePackage,
        upstream: &UpstreamReleaseEvidence,
        current_version: Option<&Version>,
        os: AssetOs,
        arch: AssetArch,
        options: &ReleaseFetchOptions,
    ) -> Result<ReleaseCheck, ReleaseError> {
        let releases = self
            .fetch_releases(upstream.release_api_url, options)
            .await?;

        check_package_from_releases(
            package,
            upstream,
            current_version,
            os,
            arch,
            options,
            &releases,
        )
    }
}

pub fn parse_github_releases(input: &str) -> Result<Vec<GitHubRelease>, serde_json::Error> {
    if input.trim_start().starts_with('[') {
        serde_json::from_str(input)
    } else {
        serde_json::from_str(input).map(|release| vec![release])
    }
}

pub fn check_package_from_releases(
    package: &ReleasePackage,
    upstream: &UpstreamReleaseEvidence,
    current_version: Option<&Version>,
    os: AssetOs,
    arch: AssetArch,
    options: &ReleaseFetchOptions,
    releases: &[GitHubRelease],
) -> Result<ReleaseCheck, ReleaseError> {
    let release = select_release(releases, effective_prerelease(package, options))?;
    let remote_version = parse_version(&release.tag_name)
        .ok_or_else(|| ReleaseError::InvalidVersion(release.tag_name.clone()))?;
    let asset = resolve_asset(package, upstream, release, &remote_version, os, arch)?;
    let has_update = current_version.is_none_or(|current| current < &remote_version);

    Ok(ReleaseCheck {
        package_id: package.id.to_string(),
        target: package.target,
        current_version: current_version.cloned(),
        remote_version,
        has_update,
        prerelease: release.prerelease,
        release_url: release.html_url.clone(),
        asset,
    })
}

pub fn select_release(
    releases: &[GitHubRelease],
    include_prerelease: bool,
) -> Result<&GitHubRelease, ReleaseError> {
    releases
        .iter()
        .filter(|release| !release.draft)
        .find(|release| include_prerelease || !release.prerelease)
        .ok_or(ReleaseError::NoRelease)
}

pub fn resolve_asset(
    package: &ReleasePackage,
    upstream: &UpstreamReleaseEvidence,
    release: &GitHubRelease,
    version: &Version,
    os: AssetOs,
    arch: AssetArch,
) -> Result<ResolvedAsset, ReleaseError> {
    let template = upstream
        .asset_templates
        .template_for(os, arch)
        .ok_or_else(|| ReleaseError::UnsupportedAssetTarget {
            package_id: package.id.to_string(),
            os,
            arch,
        })?;
    let download_url = render_asset_template(template, &release.tag_name, version);
    let expected_name = file_name_from_url(&download_url);

    if let Some(asset) = release
        .assets
        .iter()
        .find(|asset| asset.name == expected_name)
        .or_else(|| {
            release.assets.iter().find(|asset| {
                !asset.browser_download_url.is_empty() && asset.browser_download_url == download_url
            })
        })
    {
        return Ok(ResolvedAsset {
            name: asset.name.clone(),
            download_url: asset.browser_download_url.clone(),
            sha256: None,
            bytes: (asset.size > 0).then_some(asset.size),
            source: ResolvedAssetSource::ReleaseAsset,
        });
    }

    Ok(ResolvedAsset {
        name: expected_name,
        download_url,
        sha256: None,
        bytes: None,
        source: ResolvedAssetSource::TemplateFallback,
    })
}

#[must_use]
pub fn render_asset_template(template: &str, tag: &str, version: &Version) -> String {
    template
        .replace("{tag}", tag)
        .replace("{version}", &version.to_string())
}

#[must_use]
pub fn parse_version(input: &str) -> Option<Version> {
    let candidate = version_candidate(input)?;
    Version::parse(&normalize_semver(&candidate)?).ok()
}

#[must_use]
pub fn supported_release_packages() -> Vec<ReleasePackage> {
    vec![app_release_package()]
}

#[must_use]
pub fn release_package_for_core(core_type: CoreType) -> Option<ReleasePackage> {
    supported_release_packages()
        .into_iter()
        .find(|package| package.target == PackageTarget::Core(core_type))
}

#[must_use]
pub fn app_release_package() -> ReleasePackage {
    ReleasePackage {
        id: "app",
        name: "VoyaVPN",
        target: PackageTarget::App,
        prerelease_policy: PrereleasePolicy::UserControlled,
        policy: PackagePolicy {
            license: Some("MIT"),
            acquisition: BinaryAcquisition::AppPackage,
            redistribute_in_installer: true,
        },
    }
}

#[must_use]
pub fn core_acquisition_policy(core_type: CoreType) -> PackagePolicy {
    match core_type {
        CoreType::sing_box => PackagePolicy {
            license: Some("GPL-3.0-or-later"),
            acquisition: BinaryAcquisition::AppPackage,
            redistribute_in_installer: true,
        },
    }
}

#[must_use]
pub fn updatable_core_types() -> &'static [CoreType] {
    &[]
}

fn effective_prerelease(package: &ReleasePackage, options: &ReleaseFetchOptions) -> bool {
    match package.prerelease_policy {
        PrereleasePolicy::UserControlled => options.include_prerelease,
        PrereleasePolicy::StableOnly => false,
    }
}

fn file_name_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url)
        .to_string()
}

fn version_candidate(input: &str) -> Option<String> {
    let input = input.trim();
    let mut start = None;
    for (index, ch) in input.char_indices() {
        if ch.is_ascii_digit()
            || (ch == 'v'
                && input[index + ch.len_utf8()..].starts_with(|c: char| c.is_ascii_digit()))
        {
            start = Some(index + usize::from(ch == 'v'));
            break;
        }
    }
    let start = start?;
    let candidate = input[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+'))
        .collect::<String>();

    (!candidate.is_empty()).then_some(candidate)
}

fn normalize_semver(candidate: &str) -> Option<String> {
    let split_at = candidate.find(['-', '+']).unwrap_or(candidate.len());
    let core = &candidate[..split_at];
    let suffix = &candidate[split_at..];
    let parts = core
        .split('.')
        .filter(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }

    let normalized_core = match parts.as_slice() {
        [major] => format!("{major}.0.0"),
        [major, minor] => format!("{major}.{minor}.0"),
        [major, minor, patch, ..] => format!("{major}.{minor}.{patch}"),
        [] => return None,
    };

    Some(format!("{normalized_core}{suffix}"))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use tokio::sync::Mutex;

    use super::*;
    use crate::test_support::spawn_http_fixture;

    #[test]
    fn update_release_selection_respects_pre_release_toggle() {
        let releases = parse_github_releases(
            r#"[
              { "tag_name": "v2.0.0-beta.1", "prerelease": true, "assets": [] },
              { "tag_name": "v1.9.0", "prerelease": false, "assets": [] }
            ]"#,
        )
        .expect("fixture releases");

        assert_eq!(
            select_release(&releases, false).expect("stable").tag_name,
            "v1.9.0"
        );
        assert_eq!(
            select_release(&releases, true).expect("pre").tag_name,
            "v2.0.0-beta.1"
        );
    }

    #[test]
    fn update_cdn_release_index_resolves_app_asset_with_checksum() {
        let index = parse_cdn_release_index(
            r#"{
              "productName": "VoyaVPN",
              "channel": "stable",
              "version": "2.0.0",
              "baseUrl": "https://cdn.voyavpn.test/stable",
              "artifacts": [
                {
                  "channel": "stable",
                  "version": "2.0.0",
                  "target": "linux",
                  "arch": "x64",
                  "kind": "deb",
                  "url": "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.deb",
                  "bytes": 20,
                  "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                  "name": "VoyaVPN-linux-x64.deb",
                  "originalName": "voya.deb"
                },
                {
                  "channel": "stable",
                  "version": "2.0.0",
                  "target": "linux",
                  "arch": "x64",
                  "kind": "appimage",
                  "url": "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage",
                  "bytes": 10,
                  "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                  "name": "VoyaVPN-linux-x64.AppImage",
                  "originalName": "voya.AppImage"
                }
              ]
            }"#,
        )
        .expect("cdn release index");

        let check = check_app_from_cdn_release_index(
            &app_release_package(),
            Some(&Version::new(1, 0, 0)),
            AssetOs::Linux,
            AssetArch::X64,
            &index,
        )
        .expect("cdn app check");

        assert!(check.has_update);
        assert_eq!(check.remote_version, Version::new(2, 0, 0));
        assert_eq!(check.asset.source, ResolvedAssetSource::CdnReleaseIndex);
        assert_eq!(check.asset.name, "VoyaVPN-linux-x64.AppImage");
        assert_eq!(
            check.asset.download_url,
            "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage"
        );
        assert_eq!(
            check.asset.sha256.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(check.asset.bytes, Some(10));
    }

    #[test]
    fn update_version_parser_accepts_tags_and_cli_output() {
        assert_eq!(
            parse_version("v1.2.3").expect("update test operation should succeed"),
            Version::new(1, 2, 3)
        );
        assert_eq!(
            parse_version("1.2").expect("update test operation should succeed"),
            Version::new(1, 2, 0)
        );
        assert_eq!(
            parse_version("v2.0.0-beta.1").expect("update test operation should succeed"),
            Version::parse("2.0.0-beta.1").expect("update test operation should succeed")
        );
    }

    #[tokio::test]
    async fn update_download_bytes_uses_proxy_to_direct_fallback() {
        let seen_user_agents = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([("/asset.zip".to_string(), "asset-bytes".to_string())]),
            1,
            Arc::clone(&seen_user_agents),
        )
        .await;
        let response = DownloadClient::new()
            .download_bytes(DownloadRequest {
                url: format!("{base}/asset.zip"),
                user_agent: Some("VoyaUpdateTest/1".to_string()),
                prefer_proxy: true,
                proxy_url: Some("http://127.0.0.1:9".to_string()),
                response_body_limit: None,
            })
            .await
            .expect("download fallback");

        assert!(!response.used_proxy);
        assert_eq!(response.attempts.len(), 2);
        assert_eq!(response.body, b"asset-bytes");
        assert_eq!(
            seen_user_agents.lock().await.as_slice(),
            ["VoyaUpdateTest/1"]
        );
    }
}
