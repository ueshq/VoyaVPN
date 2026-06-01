use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use voya_core::CoreType;

use crate::{DownloadClient, DownloadError, DownloadRequest};

pub const VOYA_APP_RELEASES_API_URL: &str = "https://api.github.com/repos/voyavpn/voyavpn/releases";
pub const VOYA_APP_RELEASES_URL: &str = "https://github.com/voyavpn/voyavpn/releases";

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
    DownloadOnFirstRun,
    OptionalDownload,
    Unsupported,
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
pub struct AssetTemplates {
    pub windows_x64: Option<&'static str>,
    pub windows_arm64: Option<&'static str>,
    pub linux_x64: Option<&'static str>,
    pub linux_arm64: Option<&'static str>,
    pub linux_riscv64: Option<&'static str>,
    pub macos_x64: Option<&'static str>,
    pub macos_arm64: Option<&'static str>,
}

impl AssetTemplates {
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
    pub release_api_url: &'static str,
    pub release_url: &'static str,
    pub templates: AssetTemplates,
    pub prerelease_policy: PrereleasePolicy,
    pub policy: PackagePolicy,
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
    ReleaseAsset,
    TemplateFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAsset {
    pub name: String,
    pub download_url: String,
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

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("failed to parse GitHub release JSON: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("no matching release found")]
    NoRelease,
    #[error("release tag {0:?} does not contain a semantic version")]
    InvalidVersion(String),
    #[error("no asset template for {package_id} on {os:?}/{arch:?}")]
    UnsupportedAssetTarget {
        package_id: String,
        os: AssetOs,
        arch: AssetArch,
    },
}

#[derive(Debug, Clone, Default)]
pub struct GitHubReleaseClient {
    download: DownloadClient,
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
            })
            .await?;

        Ok(parse_github_releases(&response.body)?)
    }

    pub async fn check_package(
        &self,
        package: &ReleasePackage,
        current_version: Option<&Version>,
        os: AssetOs,
        arch: AssetArch,
        options: &ReleaseFetchOptions,
    ) -> Result<ReleaseCheck, ReleaseError> {
        let releases = self
            .fetch_releases(package.release_api_url, options)
            .await?;

        check_package_from_releases(package, current_version, os, arch, options, &releases)
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
    current_version: Option<&Version>,
    os: AssetOs,
    arch: AssetArch,
    options: &ReleaseFetchOptions,
    releases: &[GitHubRelease],
) -> Result<ReleaseCheck, ReleaseError> {
    let release = select_release(releases, effective_prerelease(package, options))?;
    let remote_version = parse_version(&release.tag_name)
        .ok_or_else(|| ReleaseError::InvalidVersion(release.tag_name.clone()))?;
    let asset = resolve_asset(package, release, &remote_version, os, arch)?;
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
    release: &GitHubRelease,
    version: &Version,
    os: AssetOs,
    arch: AssetArch,
) -> Result<ResolvedAsset, ReleaseError> {
    let template = package.templates.template_for(os, arch).ok_or_else(|| {
        ReleaseError::UnsupportedAssetTarget {
            package_id: package.id.to_string(),
            os,
            arch,
        }
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
            source: ResolvedAssetSource::ReleaseAsset,
        });
    }

    Ok(ResolvedAsset {
        name: expected_name,
        download_url,
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
    vec![
        app_release_package(),
        xray_release_package(),
        mihomo_release_package(),
        sing_box_release_package(),
    ]
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
        release_api_url: VOYA_APP_RELEASES_API_URL,
        release_url: VOYA_APP_RELEASES_URL,
        templates: AssetTemplates {
            windows_x64: Some(
                "https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-windows-x64.zip",
            ),
            windows_arm64: Some(
                "https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-windows-arm64.zip",
            ),
            linux_x64: Some(
                "https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-linux-x64.AppImage",
            ),
            linux_arm64: Some(
                "https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-linux-arm64.AppImage",
            ),
            linux_riscv64: Some(
                "https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-linux-riscv64.AppImage",
            ),
            macos_x64: Some(
                "https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-macos-x64.dmg",
            ),
            macos_arm64: Some(
                "https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-macos-arm64.dmg",
            ),
        },
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
        CoreType::mihomo | CoreType::sing_box => PackagePolicy {
            license: Some("GPL-3.0"),
            acquisition: BinaryAcquisition::DownloadOnFirstRun,
            redistribute_in_installer: false,
        },
        CoreType::juicity => PackagePolicy {
            license: Some("AGPL-3.0"),
            acquisition: BinaryAcquisition::DownloadOnFirstRun,
            redistribute_in_installer: false,
        },
        CoreType::Xray => PackagePolicy {
            license: Some("MPL-2.0"),
            acquisition: BinaryAcquisition::DownloadOnFirstRun,
            redistribute_in_installer: false,
        },
        CoreType::v2rayN => app_release_package().policy,
        _ => PackagePolicy {
            license: None,
            acquisition: BinaryAcquisition::DownloadOnFirstRun,
            redistribute_in_installer: false,
        },
    }
}

#[must_use]
pub fn updatable_core_types() -> &'static [CoreType] {
    &[CoreType::Xray, CoreType::mihomo, CoreType::sing_box]
}

fn xray_release_package() -> ReleasePackage {
    let base = "https://github.com/XTLS/Xray-core/releases";

    ReleasePackage {
        id: "core:xray",
        name: "Xray",
        target: PackageTarget::Core(CoreType::Xray),
        release_api_url: "https://api.github.com/repos/XTLS/Xray-core/releases",
        release_url: base,
        templates: AssetTemplates {
            windows_x64: Some(
                "https://github.com/XTLS/Xray-core/releases/download/{tag}/Xray-windows-64.zip",
            ),
            windows_arm64: Some(
                "https://github.com/XTLS/Xray-core/releases/download/{tag}/Xray-windows-arm64-v8a.zip",
            ),
            linux_x64: Some(
                "https://github.com/XTLS/Xray-core/releases/download/{tag}/Xray-linux-64.zip",
            ),
            linux_arm64: Some(
                "https://github.com/XTLS/Xray-core/releases/download/{tag}/Xray-linux-arm64-v8a.zip",
            ),
            linux_riscv64: Some(
                "https://github.com/XTLS/Xray-core/releases/download/{tag}/Xray-linux-riscv64.zip",
            ),
            macos_x64: Some(
                "https://github.com/XTLS/Xray-core/releases/download/{tag}/Xray-macos-64.zip",
            ),
            macos_arm64: Some(
                "https://github.com/XTLS/Xray-core/releases/download/{tag}/Xray-macos-arm64-v8a.zip",
            ),
        },
        prerelease_policy: PrereleasePolicy::UserControlled,
        policy: core_acquisition_policy(CoreType::Xray),
    }
}

fn mihomo_release_package() -> ReleasePackage {
    let base = "https://github.com/MetaCubeX/mihomo/releases";

    ReleasePackage {
        id: "core:mihomo",
        name: "mihomo",
        target: PackageTarget::Core(CoreType::mihomo),
        release_api_url: "https://api.github.com/repos/MetaCubeX/mihomo/releases",
        release_url: base,
        templates: AssetTemplates {
            windows_x64: Some(
                "https://github.com/MetaCubeX/mihomo/releases/download/{tag}/mihomo-windows-amd64-v1-{tag}.zip",
            ),
            windows_arm64: Some(
                "https://github.com/MetaCubeX/mihomo/releases/download/{tag}/mihomo-windows-arm64-{tag}.zip",
            ),
            linux_x64: Some(
                "https://github.com/MetaCubeX/mihomo/releases/download/{tag}/mihomo-linux-amd64-v1-{tag}.gz",
            ),
            linux_arm64: Some(
                "https://github.com/MetaCubeX/mihomo/releases/download/{tag}/mihomo-linux-arm64-{tag}.gz",
            ),
            linux_riscv64: Some(
                "https://github.com/MetaCubeX/mihomo/releases/download/{tag}/mihomo-linux-riscv64-{tag}.gz",
            ),
            macos_x64: Some(
                "https://github.com/MetaCubeX/mihomo/releases/download/{tag}/mihomo-darwin-amd64-v1-{tag}.gz",
            ),
            macos_arm64: Some(
                "https://github.com/MetaCubeX/mihomo/releases/download/{tag}/mihomo-darwin-arm64-{tag}.gz",
            ),
        },
        prerelease_policy: PrereleasePolicy::StableOnly,
        policy: core_acquisition_policy(CoreType::mihomo),
    }
}

fn sing_box_release_package() -> ReleasePackage {
    let base = "https://github.com/SagerNet/sing-box/releases";

    ReleasePackage {
        id: "core:sing_box",
        name: "sing-box",
        target: PackageTarget::Core(CoreType::sing_box),
        release_api_url: "https://api.github.com/repos/SagerNet/sing-box/releases",
        release_url: base,
        templates: AssetTemplates {
            windows_x64: Some(
                "https://github.com/SagerNet/sing-box/releases/download/{tag}/sing-box-{version}-windows-amd64.zip",
            ),
            windows_arm64: Some(
                "https://github.com/SagerNet/sing-box/releases/download/{tag}/sing-box-{version}-windows-arm64.zip",
            ),
            linux_x64: Some(
                "https://github.com/SagerNet/sing-box/releases/download/{tag}/sing-box-{version}-linux-amd64.tar.gz",
            ),
            linux_arm64: Some(
                "https://github.com/SagerNet/sing-box/releases/download/{tag}/sing-box-{version}-linux-arm64.tar.gz",
            ),
            linux_riscv64: Some(
                "https://github.com/SagerNet/sing-box/releases/download/{tag}/sing-box-{version}-linux-riscv64.tar.gz",
            ),
            macos_x64: Some(
                "https://github.com/SagerNet/sing-box/releases/download/{tag}/sing-box-{version}-darwin-amd64.tar.gz",
            ),
            macos_arm64: Some(
                "https://github.com/SagerNet/sing-box/releases/download/{tag}/sing-box-{version}-darwin-arm64.tar.gz",
            ),
        },
        prerelease_policy: PrereleasePolicy::StableOnly,
        policy: core_acquisition_policy(CoreType::sing_box),
    }
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
    let split_at = candidate
        .find(|ch| matches!(ch, '-' | '+'))
        .unwrap_or(candidate.len());
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

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };

    use super::*;

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
    fn update_asset_templates_cover_supported_os_arch_matrix() {
        let release = GitHubRelease {
            tag_name: "v1.2.3".to_string(),
            ..fixture_release()
        };
        let version = parse_version("v1.2.3").expect("version");

        let xray = release_package_for_core(CoreType::Xray).expect("xray package");
        let xray_asset = resolve_asset(
            &xray,
            &release,
            &version,
            AssetOs::Windows,
            AssetArch::Arm64,
        )
        .expect("xray asset");
        assert_eq!(xray_asset.name, "Xray-windows-arm64-v8a.zip");

        let mihomo = release_package_for_core(CoreType::mihomo).expect("mihomo package");
        let mihomo_asset =
            resolve_asset(&mihomo, &release, &version, AssetOs::Linux, AssetArch::X64)
                .expect("mihomo asset");
        assert_eq!(mihomo_asset.name, "mihomo-linux-amd64-v1-v1.2.3.gz");

        let sing_box = release_package_for_core(CoreType::sing_box).expect("sing-box package");
        let sing_asset = resolve_asset(
            &sing_box,
            &release,
            &version,
            AssetOs::Linux,
            AssetArch::Riscv64,
        )
        .expect("sing-box asset");
        assert_eq!(sing_asset.name, "sing-box-1.2.3-linux-riscv64.tar.gz");
    }

    #[test]
    fn update_asset_selection_prefers_release_asset_and_falls_back_to_template() {
        let package = release_package_for_core(CoreType::Xray).expect("xray package");
        let version = parse_version("v1.8.7").expect("version");
        let release = GitHubRelease {
            tag_name: "v1.8.7".to_string(),
            assets: vec![GitHubReleaseAsset {
                name: "Xray-linux-64.zip".to_string(),
                browser_download_url: "https://cdn.example/Xray-linux-64.zip".to_string(),
                size: 10,
                content_type: None,
            }],
            ..fixture_release()
        };

        let exact = resolve_asset(&package, &release, &version, AssetOs::Linux, AssetArch::X64)
            .expect("exact asset");
        assert_eq!(exact.source, ResolvedAssetSource::ReleaseAsset);
        assert_eq!(exact.download_url, "https://cdn.example/Xray-linux-64.zip");

        let fallback = resolve_asset(&package, &release, &version, AssetOs::Macos, AssetArch::X64)
            .expect("fallback asset");
        assert_eq!(fallback.source, ResolvedAssetSource::TemplateFallback);
        assert_eq!(fallback.name, "Xray-macos-64.zip");
    }

    #[test]
    fn update_version_parser_accepts_tags_and_cli_output() {
        assert_eq!(parse_version("v1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(parse_version("1.2").unwrap(), Version::new(1, 2, 0));
        assert_eq!(
            parse_version("Xray 1.8.7 (Xray, Penetrates Everything.)").unwrap(),
            Version::new(1, 8, 7)
        );
        assert_eq!(
            parse_version("mihomo v1.18.4 linux amd64").unwrap(),
            Version::new(1, 18, 4)
        );
        assert_eq!(
            parse_version("v2.0.0-beta.1").unwrap(),
            Version::parse("2.0.0-beta.1").unwrap()
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

    fn fixture_release() -> GitHubRelease {
        GitHubRelease {
            tag_name: String::new(),
            name: None,
            html_url: None,
            draft: false,
            prerelease: false,
            assets: Vec::new(),
            body: None,
        }
    }

    async fn spawn_http_fixture(
        routes: HashMap<String, String>,
        max_requests: usize,
        seen_user_agents: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let routes = Arc::new(routes);

        tokio::spawn(async move {
            for _ in 0..max_requests {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let routes = Arc::clone(&routes);
                let seen_user_agents = Arc::clone(&seen_user_agents);
                tokio::spawn(async move {
                    let mut buffer = vec![0; 4096];
                    let bytes_read = socket.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .and_then(|target| target.split('?').next())
                        .unwrap_or("/");
                    let user_agent = request
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("user-agent")
                                .then(|| value.trim().to_string())
                        })
                        .unwrap_or_default();
                    seen_user_agents.lock().await.push(user_agent);
                    let body = routes.get(path).cloned().unwrap_or_default();
                    let status = if routes.contains_key(path) {
                        "200 OK"
                    } else {
                        "404 Not Found"
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        format!("http://{address}")
    }
}
