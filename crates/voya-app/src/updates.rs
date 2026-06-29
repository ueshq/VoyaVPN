use std::collections::{BTreeMap, BTreeSet};

use semver::Version;
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use voya_core::{AppConfig, CoreType, DnsItem, RoutingItem};
use voya_db::{Database, DbError};
use voya_net::{
    ruleset::{
        collect_singbox_ruleset_assets, discover_local_singbox_ruleset_paths, geo_assets,
        AcquiredRulesetGeoAsset, AssetAcquisitionOptions, RulesetGeoClient, RulesetGeoError,
        SrsAsset, DEFAULT_GEO_SOURCE_URL, DEFAULT_SINGBOX_RULESET_URL,
    },
    update::{
        app_release_artifacts_for_cdn_index, app_release_package, cdn_manifest_url_from_base,
        check_app_from_cdn_release_index, check_package_from_releases, parse_github_releases,
        parse_version, supported_release_packages, AssetArch, AssetOs, BinaryAcquisition,
        CdnReleaseIndex, CdnUpdateClient, PackageTarget, ReleaseCheck, ReleaseError,
        ReleaseFetchOptions, ReleasePackage, UpstreamReleaseEvidence, CDN_RELEASE_INDEX_FILE,
    },
    DownloadClient, DownloadError, DownloadRequest,
};
use voya_platform::paths::AppPaths;

use crate::runtime::RuntimeError;

const GEO_TARGET_ID: &str = "geo";
const SRS_TARGET_ID: &str = "srs";

pub type Result<T> = std::result::Result<T, UpdateManagerError>;

#[derive(Debug, Error)]
pub enum UpdateManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Release(#[from] ReleaseError),
    #[error(transparent)]
    RulesetGeo(#[from] RulesetGeoError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("blocking update task {operation} failed: {source}")]
    BlockingTaskJoin {
        operation: &'static str,
        #[source]
        source: tokio::task::JoinError,
    },
    #[error("unsupported update target {0}")]
    UnsupportedTarget(String),
    #[error("current OS or CPU architecture is not supported for downloads")]
    UnsupportedPlatform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateTargetKind {
    App,
    Geo,
    Srs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateAcquisition {
    AppPackage,
    OptionalDownload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateResultStatus {
    Skipped,
    UpToDate,
    UpdateAvailable,
    Downloaded,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTarget {
    pub id: String,
    pub name: String,
    pub kind: UpdateTargetKind,
    pub core_type: Option<CoreType>,
    pub selected: bool,
    pub update_supported: bool,
    pub license: Option<String>,
    pub acquisition: UpdateAcquisition,
    pub redistribute_in_installer: bool,
    pub remarks: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub pre_release: bool,
    pub targets: Vec<UpdateTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub target_id: String,
    pub status: UpdateResultStatus,
    pub message: String,
    pub current_version: Option<String>,
    pub remote_version: Option<String>,
    pub download_url: Option<String>,
    pub file_name: Option<String>,
    pub sha256: Option<String>,
    pub bytes: Option<u32>,
    pub used_proxy: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRunResult {
    pub pre_release: bool,
    pub results: Vec<UpdateCheckResult>,
    pub targets: Vec<UpdateTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualAppUpdateLinks {
    pub current_version: String,
    pub remote_version: Option<String>,
    pub has_update: bool,
    pub release_index_url: String,
    pub channel: String,
    pub target: String,
    pub arch: String,
    pub downloads: Vec<ManualAppUpdateDownload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualAppUpdateDownload {
    pub name: String,
    pub kind: String,
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub bytes: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RulesetGeoSourceSettings {
    pub geo_source_url: Option<String>,
    pub srs_source_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateRequestOptions {
    pub pre_release: bool,
    pub selected_target_ids: Vec<String>,
    pub prefer_proxy: bool,
    pub proxy_url: Option<String>,
}

impl Default for UpdateRequestOptions {
    fn default() -> Self {
        Self {
            pre_release: false,
            selected_target_ids: Vec::new(),
            prefer_proxy: true,
            proxy_url: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateManager<'db> {
    database: &'db Database,
    paths: AppPaths,
    cdn: CdnUpdateClient,
    downloads: DownloadClient,
    ruleset_geo: RulesetGeoClient,
}

impl<'db> UpdateManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database, paths: AppPaths) -> Self {
        Self {
            database,
            paths,
            cdn: CdnUpdateClient::new(),
            downloads: DownloadClient::new(),
            ruleset_geo: RulesetGeoClient::new(),
        }
    }

    #[must_use]
    pub fn status(&self, config: &AppConfig) -> UpdateStatus {
        update_status(config)
    }

    pub fn save_preferences(
        &self,
        config: &mut AppConfig,
        pre_release: bool,
        selected_target_ids: Vec<String>,
    ) {
        config.check_update_item.check_pre_release_update = pre_release;
        config.check_update_item.selected_core_types = Some(selected_target_ids);
    }

    #[must_use]
    pub fn source_settings(&self, config: &AppConfig) -> RulesetGeoSourceSettings {
        source_settings(config)
    }

    pub fn save_source_settings(
        &self,
        config: &mut AppConfig,
        settings: RulesetGeoSourceSettings,
    ) -> RulesetGeoSourceSettings {
        config.const_item.geo_source_url = normalize_optional_url(settings.geo_source_url);
        config.const_item.srs_source_url = normalize_optional_url(settings.srs_source_url);
        self.source_settings(config)
    }

    pub async fn check_updates(
        &self,
        config: &AppConfig,
        options: &UpdateRequestOptions,
    ) -> Result<UpdateRunResult> {
        let os = AssetOs::current().ok_or(UpdateManagerError::UnsupportedPlatform)?;
        let arch = AssetArch::current().ok_or(UpdateManagerError::UnsupportedPlatform)?;
        let selected = selected_target_ids(config, &options.selected_target_ids);
        let fetch_options = release_fetch_options(options);
        let metadata = self
            .load_cdn_update_metadata(config, &selected, &fetch_options)
            .await;
        let mut results = Vec::new();

        for package in supported_release_packages() {
            if !selected.contains(package.id) {
                results.push(skipped(package.id, "not selected"));
                continue;
            }

            let current = self.current_package_version(&package).await?;
            match check_package_from_cdn_metadata(&package, current.as_ref(), os, arch, &metadata) {
                Ok(check) => results.push(check_result_to_update_result(&check)),
                Err(error) => results.push(UpdateCheckResult {
                    target_id: package.id.to_string(),
                    status: UpdateResultStatus::Error,
                    message: error,
                    current_version: current.as_ref().map(ToString::to_string),
                    remote_version: None,
                    download_url: None,
                    file_name: None,
                    sha256: None,
                    bytes: None,
                    used_proxy: None,
                }),
            }
        }

        append_geo_srs_check_results(config, &selected, &mut results);

        Ok(UpdateRunResult {
            pre_release: options.pre_release,
            results,
            targets: update_targets(config),
        })
    }

    pub async fn download_updates(
        &self,
        config: &AppConfig,
        options: &UpdateRequestOptions,
    ) -> Result<UpdateRunResult> {
        let os = AssetOs::current().ok_or(UpdateManagerError::UnsupportedPlatform)?;
        let arch = AssetArch::current().ok_or(UpdateManagerError::UnsupportedPlatform)?;
        let selected = selected_target_ids(config, &options.selected_target_ids);
        let fetch_options = release_fetch_options(options);
        let metadata = self
            .load_cdn_update_metadata(config, &selected, &fetch_options)
            .await;
        let mut results = Vec::new();

        for package in supported_release_packages() {
            if !selected.contains(package.id) {
                results.push(skipped(package.id, "not selected"));
                continue;
            }

            let current = self.current_package_version(&package).await?;
            match check_package_from_cdn_metadata(&package, current.as_ref(), os, arch, &metadata) {
                Ok(check) if check.has_update => {
                    let file_name = check.asset.name.clone();
                    let target = self.paths.temp_file(format!("updates/{file_name}"));
                    let response = self
                        .downloads
                        .download_file(
                            DownloadRequest {
                                url: check.asset.download_url.clone(),
                                user_agent: Some(voya_net::USER_AGENT_PREFIX.to_string()),
                                prefer_proxy: options.prefer_proxy,
                                proxy_url: options.proxy_url.clone(),
                                response_body_limit: None,
                            },
                            &target,
                        )
                        .await?;
                    results.push(UpdateCheckResult {
                        target_id: package.id.to_string(),
                        status: UpdateResultStatus::Downloaded,
                        message: format!("downloaded {}", target.display()),
                        current_version: check.current_version.as_ref().map(ToString::to_string),
                        remote_version: Some(check.remote_version.to_string()),
                        download_url: Some(check.asset.download_url),
                        file_name: Some(target.to_string_lossy().into_owned()),
                        sha256: check.asset.sha256,
                        bytes: check.asset.bytes.and_then(ipc_bytes),
                        used_proxy: Some(response.used_proxy),
                    });
                }
                Ok(check) => results.push(check_result_to_update_result(&check)),
                Err(error) => results.push(UpdateCheckResult {
                    target_id: package.id.to_string(),
                    status: UpdateResultStatus::Error,
                    message: error,
                    current_version: current.as_ref().map(ToString::to_string),
                    remote_version: None,
                    download_url: None,
                    file_name: None,
                    sha256: None,
                    bytes: None,
                    used_proxy: None,
                }),
            }
        }

        if selected.contains(GEO_TARGET_ID) {
            results.extend(self.download_geo_files(config, options).await?);
        }
        if selected.contains(SRS_TARGET_ID) {
            results.extend(self.download_srs_files(config, options).await?);
        }

        Ok(UpdateRunResult {
            pre_release: options.pre_release,
            results,
            targets: update_targets(config),
        })
    }

    pub async fn manual_app_update_links(
        &self,
        config: &AppConfig,
        options: &UpdateRequestOptions,
    ) -> Result<ManualAppUpdateLinks> {
        let os = AssetOs::current().ok_or(UpdateManagerError::UnsupportedPlatform)?;
        let arch = AssetArch::current().ok_or(UpdateManagerError::UnsupportedPlatform)?;
        let release_index_url =
            update_manifest_urls(config)
                .release_index_url
                .ok_or(UpdateManagerError::Release(
                    ReleaseError::MissingCdnManifestUrl {
                        manifest: CDN_RELEASE_INDEX_FILE,
                    },
                ))?;
        let index = self
            .cdn
            .fetch_release_index(&release_index_url, &release_fetch_options(options))
            .await?;

        manual_app_update_links_from_index(
            &release_index_url,
            env!("CARGO_PKG_VERSION"),
            os,
            arch,
            &index,
        )
    }

    async fn current_package_version(&self, package: &ReleasePackage) -> Result<Option<Version>> {
        match package.target {
            PackageTarget::App => Ok(parse_version(env!("CARGO_PKG_VERSION"))),
            PackageTarget::Core(_) => Ok(None),
        }
    }

    async fn download_geo_files(
        &self,
        config: &AppConfig,
        options: &UpdateRequestOptions,
    ) -> Result<Vec<UpdateCheckResult>> {
        let assets = geo_assets(config.const_item.geo_source_url.as_deref());
        let acquired = self
            .ruleset_geo
            .acquire_geo_assets(
                &assets,
                self.paths.bin_dir(),
                &asset_acquisition_options(options),
            )
            .await?;

        Ok(acquired
            .into_iter()
            .map(|asset| acquired_asset_to_result(GEO_TARGET_ID, asset))
            .collect())
    }

    async fn download_srs_files(
        &self,
        config: &AppConfig,
        options: &UpdateRequestOptions,
    ) -> Result<Vec<UpdateCheckResult>> {
        let routings = self.database.routings().list().await?;
        let dns_items = self.database.dns().list().await?;
        let assets = collect_srs_assets(config, &routings, &dns_items);
        let srs_dir = self.paths.bin_dir().join("srss");
        let acquired = self
            .ruleset_geo
            .acquire_srs_assets(&assets, srs_dir, &asset_acquisition_options(options))
            .await?;

        Ok(acquired
            .into_iter()
            .map(|asset| acquired_asset_to_result(SRS_TARGET_ID, asset))
            .collect())
    }

    async fn load_cdn_update_metadata(
        &self,
        config: &AppConfig,
        selected: &BTreeSet<String>,
        options: &ReleaseFetchOptions,
    ) -> CdnUpdateMetadata {
        let urls = update_manifest_urls(config);
        let needs_release_index = selected.contains("app");
        let release_index = if needs_release_index {
            match urls.release_index_url {
                Some(url) => match self.cdn.fetch_release_index(&url, options).await {
                    Ok(index) => ManifestLoad::Loaded(index),
                    Err(error) => ManifestLoad::Failed(error.to_string()),
                },
                None => ManifestLoad::Missing(
                    ReleaseError::MissingCdnManifestUrl {
                        manifest: CDN_RELEASE_INDEX_FILE,
                    }
                    .to_string(),
                ),
            }
        } else {
            ManifestLoad::NotNeeded
        };

        CdnUpdateMetadata { release_index }
    }
}

#[derive(Debug, Clone)]
struct CdnUpdateMetadata {
    release_index: ManifestLoad<CdnReleaseIndex>,
}

#[derive(Debug, Clone)]
enum ManifestLoad<T> {
    NotNeeded,
    Missing(String),
    Failed(String),
    Loaded(T),
}

impl<T> ManifestLoad<T> {
    fn loaded(&self) -> std::result::Result<&T, String> {
        match self {
            Self::Loaded(value) => Ok(value),
            Self::Missing(error) | Self::Failed(error) => Err(error.clone()),
            Self::NotNeeded => Err("CDN manifest was not loaded".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfiguredUpdateManifestUrls {
    release_index_url: Option<String>,
}

fn release_fetch_options(options: &UpdateRequestOptions) -> ReleaseFetchOptions {
    ReleaseFetchOptions {
        include_prerelease: options.pre_release,
        prefer_proxy: options.prefer_proxy,
        proxy_url: options.proxy_url.clone(),
    }
}

fn check_package_from_cdn_metadata(
    package: &ReleasePackage,
    current_version: Option<&Version>,
    os: AssetOs,
    arch: AssetArch,
    metadata: &CdnUpdateMetadata,
) -> std::result::Result<ReleaseCheck, String> {
    match package.target {
        PackageTarget::App => check_app_from_cdn_release_index(
            package,
            current_version,
            os,
            arch,
            metadata.release_index.loaded()?,
        ),
        PackageTarget::Core(_) => {
            return Err(format!("update check unsupported for {}", package.id));
        }
    }
    .map_err(|error| error.to_string())
}

pub fn manual_app_update_links_from_index(
    release_index_url: &str,
    current_version: &str,
    os: AssetOs,
    arch: AssetArch,
    index: &CdnReleaseIndex,
) -> Result<ManualAppUpdateLinks> {
    let package = app_release_package();
    let artifacts = app_release_artifacts_for_cdn_index(&package, os, arch, index)?;
    let downloads = artifacts
        .into_iter()
        .map(|artifact| {
            let version =
                non_empty_string(&artifact.version).unwrap_or_else(|| index.version.clone());
            ManualAppUpdateDownload {
                name: asset_name_from_fields(&artifact.name, &artifact.url),
                kind: artifact.kind,
                version,
                url: artifact.url,
                sha256: non_empty_string(&artifact.sha256),
                bytes: (artifact.bytes > 0)
                    .then_some(artifact.bytes)
                    .and_then(ipc_bytes),
            }
        })
        .collect::<Vec<_>>();
    let remote_version = downloads
        .first()
        .and_then(|download| parse_version(&download.version).map(|version| version.to_string()));
    let current = parse_version(current_version);
    let has_update = remote_version
        .as_deref()
        .and_then(parse_version)
        .is_some_and(|remote| current.as_ref().is_none_or(|current| current < &remote));

    Ok(ManualAppUpdateLinks {
        current_version: current_version.to_string(),
        remote_version,
        has_update,
        release_index_url: release_index_url.to_string(),
        channel: index.channel.clone(),
        target: asset_os_label(os).to_string(),
        arch: asset_arch_label(arch).to_string(),
        downloads,
    })
}

fn update_manifest_urls(config: &AppConfig) -> ConfiguredUpdateManifestUrls {
    let base_url = normalize_optional_url(config.const_item.cdn_base_url.clone());
    let release_index_url = normalize_optional_url(config.const_item.cdn_release_index_url.clone())
        .or_else(|| {
            base_url
                .as_deref()
                .and_then(|base| cdn_manifest_url_from_base(base, CDN_RELEASE_INDEX_FILE))
        });

    ConfiguredUpdateManifestUrls { release_index_url }
}

#[must_use]
pub fn update_status(config: &AppConfig) -> UpdateStatus {
    UpdateStatus {
        pre_release: config.check_update_item.check_pre_release_update,
        targets: update_targets(config),
    }
}

#[must_use]
pub fn source_settings(config: &AppConfig) -> RulesetGeoSourceSettings {
    RulesetGeoSourceSettings {
        geo_source_url: config.const_item.geo_source_url.clone(),
        srs_source_url: config.const_item.srs_source_url.clone(),
    }
}

#[must_use]
pub fn local_singbox_ruleset_paths(paths: &AppPaths) -> BTreeMap<String, String> {
    discover_local_singbox_ruleset_paths(paths.bin_dir().join("srss"))
}

#[must_use]
pub fn update_targets(config: &AppConfig) -> Vec<UpdateTarget> {
    let selected = selected_target_ids(config, &[]);
    let mut targets = Vec::new();

    for package in supported_release_packages() {
        if let Some(target) = package_to_target(&package, selected.contains(package.id)) {
            targets.push(target);
        }
    }

    targets.push(UpdateTarget {
        id: GEO_TARGET_ID.to_string(),
        name: "Geo files".to_string(),
        kind: UpdateTargetKind::Geo,
        core_type: None,
        selected: selected.contains(GEO_TARGET_ID),
        update_supported: true,
        license: None,
        acquisition: UpdateAcquisition::OptionalDownload,
        redistribute_in_installer: false,
        remarks: "geosite.dat, geoip.dat, and companion geo databases".to_string(),
    });
    targets.push(UpdateTarget {
        id: SRS_TARGET_ID.to_string(),
        name: "sing-box rulesets".to_string(),
        kind: UpdateTargetKind::Srs,
        core_type: None,
        selected: selected.contains(SRS_TARGET_ID),
        update_supported: true,
        license: None,
        acquisition: UpdateAcquisition::OptionalDownload,
        redistribute_in_installer: false,
        remarks: "SRS assets derived from routing and DNS rule sets".to_string(),
    });

    targets
}

#[must_use]
pub fn selected_target_ids(config: &AppConfig, override_ids: &[String]) -> BTreeSet<String> {
    let supported = supported_target_ids();
    let ids = if override_ids.is_empty() {
        config
            .check_update_item
            .selected_core_types
            .as_ref()
            .filter(|ids| !ids.is_empty())
            .cloned()
            .unwrap_or_else(default_selected_targets)
    } else {
        override_ids.to_vec()
    };

    ids.into_iter()
        .map(|id| normalize_target_id(&id))
        .filter(|id| supported.contains(id))
        .filter(|id| !id.is_empty())
        .collect()
}

#[must_use]
pub fn collect_srs_assets(
    config: &AppConfig,
    routings: &[RoutingItem],
    dns_items: &[DnsItem],
) -> Vec<SrsAsset> {
    collect_singbox_ruleset_assets(
        config.const_item.srs_source_url.as_deref(),
        routings,
        dns_items,
    )
}

pub fn check_package_fixture(
    package: &ReleasePackage,
    upstream: &UpstreamReleaseEvidence,
    release_json: &str,
    current_version: Option<&Version>,
    os: AssetOs,
    arch: AssetArch,
    include_prerelease: bool,
) -> Result<UpdateCheckResult> {
    let releases = parse_github_releases(release_json).map_err(ReleaseError::from)?;
    let check = check_package_from_releases(
        package,
        upstream,
        current_version,
        os,
        arch,
        &ReleaseFetchOptions {
            include_prerelease,
            prefer_proxy: false,
            proxy_url: None,
        },
        &releases,
    )?;

    Ok(check_result_to_update_result(&check))
}

fn append_geo_srs_check_results(
    config: &AppConfig,
    selected: &BTreeSet<String>,
    results: &mut Vec<UpdateCheckResult>,
) {
    if selected.contains(GEO_TARGET_ID) {
        let geo_source = config
            .const_item
            .geo_source_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(DEFAULT_GEO_SOURCE_URL);
        results.push(UpdateCheckResult {
            target_id: GEO_TARGET_ID.to_string(),
            status: UpdateResultStatus::UpdateAvailable,
            message: "geo files can be refreshed on demand".to_string(),
            current_version: None,
            remote_version: None,
            download_url: Some(geo_source.to_string()),
            file_name: None,
            sha256: None,
            bytes: None,
            used_proxy: None,
        });
    }
    if selected.contains(SRS_TARGET_ID) {
        let srs_source = config
            .const_item
            .srs_source_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(DEFAULT_SINGBOX_RULESET_URL);
        results.push(UpdateCheckResult {
            target_id: SRS_TARGET_ID.to_string(),
            status: UpdateResultStatus::UpdateAvailable,
            message: "SRS rulesets can be refreshed on demand".to_string(),
            current_version: None,
            remote_version: None,
            download_url: Some(srs_source.to_string()),
            file_name: None,
            sha256: None,
            bytes: None,
            used_proxy: None,
        });
    }
}

fn check_result_to_update_result(check: &ReleaseCheck) -> UpdateCheckResult {
    UpdateCheckResult {
        target_id: check.package_id.clone(),
        status: if check.has_update {
            UpdateResultStatus::UpdateAvailable
        } else {
            UpdateResultStatus::UpToDate
        },
        message: if check.has_update {
            format!("{} is available", check.remote_version)
        } else {
            format!("already on {}", check.remote_version)
        },
        current_version: check.current_version.as_ref().map(ToString::to_string),
        remote_version: Some(check.remote_version.to_string()),
        download_url: Some(check.asset.download_url.clone()),
        file_name: Some(check.asset.name.clone()),
        sha256: check.asset.sha256.clone(),
        bytes: check.asset.bytes.and_then(ipc_bytes),
        used_proxy: None,
    }
}

fn acquired_asset_to_result(target_id: &str, asset: AcquiredRulesetGeoAsset) -> UpdateCheckResult {
    UpdateCheckResult {
        target_id: target_id.to_string(),
        status: UpdateResultStatus::Downloaded,
        message: format!(
            "downloaded {} ({} bytes)",
            asset.path.display(),
            asset.bytes
        ),
        current_version: None,
        remote_version: None,
        download_url: Some(asset.url),
        file_name: Some(asset.path.to_string_lossy().into_owned()),
        sha256: None,
        bytes: ipc_bytes(asset.bytes),
        used_proxy: Some(asset.used_proxy),
    }
}

fn asset_name_from_fields(name: &str, url: &str) -> String {
    non_empty_string(name).unwrap_or_else(|| {
        url.rsplit('/')
            .next()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("download")
            .to_string()
    })
}

fn asset_os_label(os: AssetOs) -> &'static str {
    match os {
        AssetOs::Windows => "windows",
        AssetOs::Linux => "linux",
        AssetOs::Macos => "macos",
    }
}

fn asset_arch_label(arch: AssetArch) -> &'static str {
    match arch {
        AssetArch::X64 => "x64",
        AssetArch::Arm64 => "arm64",
        AssetArch::Riscv64 => "riscv64",
    }
}

fn asset_acquisition_options(options: &UpdateRequestOptions) -> AssetAcquisitionOptions {
    AssetAcquisitionOptions {
        prefer_proxy: options.prefer_proxy,
        proxy_url: options.proxy_url.clone(),
    }
}

fn normalize_optional_url(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn ipc_bytes(bytes: u64) -> Option<u32> {
    u32::try_from(bytes).ok()
}

fn skipped(target_id: &str, message: &str) -> UpdateCheckResult {
    UpdateCheckResult {
        target_id: target_id.to_string(),
        status: UpdateResultStatus::Skipped,
        message: message.to_string(),
        current_version: None,
        remote_version: None,
        download_url: None,
        file_name: None,
        sha256: None,
        bytes: None,
        used_proxy: None,
    }
}

fn package_to_target(package: &ReleasePackage, selected: bool) -> Option<UpdateTarget> {
    let (kind, core_type) = match package.target {
        PackageTarget::App => (UpdateTargetKind::App, None),
        PackageTarget::Core(_) => return None,
    };

    let acquisition = acquisition(package.policy.acquisition);

    Some(UpdateTarget {
        id: package.id.to_string(),
        name: package.name.to_string(),
        kind,
        core_type,
        selected,
        update_supported: true,
        license: package.policy.license.map(ToString::to_string),
        acquisition,
        redistribute_in_installer: package.policy.redistribute_in_installer,
        remarks: match package.policy.acquisition {
            BinaryAcquisition::AppPackage => "application package update".to_string(),
            BinaryAcquisition::OptionalDownload => "optional download".to_string(),
        },
    })
}

fn acquisition(value: BinaryAcquisition) -> UpdateAcquisition {
    match value {
        BinaryAcquisition::AppPackage => UpdateAcquisition::AppPackage,
        BinaryAcquisition::OptionalDownload => UpdateAcquisition::OptionalDownload,
    }
}

fn default_selected_targets() -> Vec<String> {
    supported_release_packages()
        .into_iter()
        .map(|package| package.id.to_string())
        .chain([GEO_TARGET_ID.to_string(), SRS_TARGET_ID.to_string()])
        .collect()
}

fn supported_target_ids() -> BTreeSet<String> {
    supported_release_packages()
        .into_iter()
        .map(|package| package.id.to_string())
        .chain([GEO_TARGET_ID.to_string(), SRS_TARGET_ID.to_string()])
        .collect()
}

fn normalize_target_id(value: &str) -> String {
    let value = value.trim();
    match value {
        "GeoFiles" => GEO_TARGET_ID.to_string(),
        "v2rayN" => "app".to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        env, fs,
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use voya_core::{RoutingItem, RuleType, RulesItem};
    use voya_platform::paths::StorageMode;

    use super::*;

    #[test]
    fn update_selected_target_ids_accept_legacy_v2rayn_storage_names() {
        let mut config = AppConfig::default();
        config.check_update_item.selected_core_types =
            Some(vec!["sing_box".to_string(), "GeoFiles".to_string()]);

        let selected = selected_target_ids(&config, &[]);
        assert!(selected.contains("geo"));
        assert!(!selected.contains("core:sing_box"));
    }

    #[test]
    fn updates_manifest_urls_derive_from_cdn_base_url() {
        let mut config = AppConfig::default();
        config.const_item.cdn_base_url = Some(" https://cdn.voyavpn.test/stable/ ".to_string());

        let urls = update_manifest_urls(&config);

        assert_eq!(
            urls.release_index_url.as_deref(),
            Some("https://cdn.voyavpn.test/stable/release-index.json")
        );
    }

    #[test]
    fn manual_app_update_links_resolve_current_target_downloads() {
        let index = voya_net::update::parse_cdn_release_index(
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
                  "name": "VoyaVPN-linux-x64.deb"
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
                  "name": "VoyaVPN-linux-x64.AppImage"
                },
                {
                  "channel": "stable",
                  "version": "2.0.0",
                  "target": "windows",
                  "arch": "x64",
                  "kind": "nsis",
                  "url": "https://cdn.voyavpn.test/stable/VoyaVPN-windows-x64.exe",
                  "bytes": 30,
                  "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                  "name": "VoyaVPN-windows-x64.exe"
                }
              ]
            }"#,
        )
        .expect("release index");

        let links = manual_app_update_links_from_index(
            "https://cdn.voyavpn.test/stable/release-index.json",
            "1.0.0",
            AssetOs::Linux,
            AssetArch::X64,
            &index,
        )
        .expect("manual links");

        assert_eq!(links.remote_version.as_deref(), Some("2.0.0"));
        assert!(links.has_update);
        assert_eq!(links.target, "linux");
        assert_eq!(links.arch, "x64");
        assert_eq!(links.downloads.len(), 2);
        assert_eq!(links.downloads[0].kind, "appimage");
        assert_eq!(
            links.downloads[0].url,
            "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage"
        );
        assert!(!links
            .downloads
            .iter()
            .any(|download| download.url.contains("github.com")));
    }

    #[test]
    fn manual_app_update_links_reject_github_download_url() {
        let index = voya_net::update::parse_cdn_release_index(
            r#"{
              "productName": "VoyaVPN",
              "channel": "stable",
              "version": "2.0.0",
              "baseUrl": "https://cdn.voyavpn.test/stable",
              "artifacts": [{
                "channel": "stable",
                "version": "2.0.0",
                "target": "linux",
                "arch": "x64",
                "kind": "appimage",
                "url": "https://github.com/voyavpn/voyavpn/releases/download/v2.0.0/VoyaVPN-linux-x64.AppImage",
                "bytes": 10,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "name": "VoyaVPN-linux-x64.AppImage"
              }]
            }"#,
        )
        .expect("release index");

        let error = manual_app_update_links_from_index(
            "https://cdn.voyavpn.test/stable/release-index.json",
            "1.0.0",
            AssetOs::Linux,
            AssetArch::X64,
            &index,
        )
        .expect_err("github URL must be rejected");

        assert!(matches!(
            error,
            UpdateManagerError::Release(ReleaseError::ForbiddenProductionUrl(url))
                if url.contains("github.com")
        ));
    }

    #[tokio::test]
    async fn updates_check_uses_cdn_release_index_and_returns_app_checksum() {
        let Some(os) = AssetOs::current() else {
            return;
        };
        let Some(arch) = AssetArch::current() else {
            return;
        };
        let os_label = manifest_os_label(os);
        let arch_label = manifest_arch_label(arch);
        let app_name = format!("VoyaVPN-{os_label}-{arch_label}.zip");
        let release_index = format!(
            r#"{{
              "productName": "VoyaVPN",
              "channel": "stable",
              "version": "9.9.9",
              "baseUrl": "http://127.0.0.1/stable",
              "artifacts": [{{
                "channel": "stable",
                "version": "9.9.9",
                "target": "{os_label}",
                "arch": "{arch_label}",
                "kind": "zip",
                "url": "http://127.0.0.1/stable/{app_name}",
                "bytes": 42,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "name": "{app_name}",
                "originalName": "{app_name}"
              }}]
            }}"#
        );
        let base = spawn_http_fixture(HashMap::from([(
            "/stable/release-index.json".to_string(),
            release_index,
        )]))
        .await;

        let mut config = AppConfig::default();
        config.const_item.cdn_base_url = Some(format!("{base}/stable"));
        let database = Database::connect_in_memory()
            .await
            .expect("update manager test operation should succeed");
        let root = unique_temp_root("cdn-check");
        let manager = UpdateManager::new(&database, AppPaths::new(&root, StorageMode::Portable));

        let run = manager
            .check_updates(
                &config,
                &UpdateRequestOptions {
                    selected_target_ids: vec!["app".to_string()],
                    prefer_proxy: false,
                    ..UpdateRequestOptions::default()
                },
            )
            .await
            .expect("cdn update check");

        let app = run
            .results
            .iter()
            .find(|result| result.target_id == "app")
            .expect("app result");

        assert_eq!(app.status, UpdateResultStatus::UpdateAvailable);
        assert_eq!(app.remote_version.as_deref(), Some("9.9.9"));
        assert_eq!(
            app.sha256.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(app.bytes, Some(42));
        assert!(!app
            .download_url
            .as_deref()
            .unwrap_or_default()
            .contains("github.com"));
        assert!(!run
            .results
            .iter()
            .any(|result| result.target_id.starts_with("core:")));

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn updates_check_rejects_github_cdn_base_url() {
        let mut config = AppConfig::default();
        config.const_item.cdn_base_url =
            Some("https://github.com/voyavpn/voyavpn/releases".to_string());
        let database = Database::connect_in_memory()
            .await
            .expect("update manager test operation should succeed");
        let manager = UpdateManager::new(
            &database,
            AppPaths::new("/tmp/VoyaVPN", StorageMode::Portable),
        );

        let run = manager
            .check_updates(
                &config,
                &UpdateRequestOptions {
                    selected_target_ids: vec!["app".to_string()],
                    prefer_proxy: false,
                    ..UpdateRequestOptions::default()
                },
            )
            .await
            .expect("github base rejection returns an update result");
        let app = run
            .results
            .iter()
            .find(|result| result.target_id == "app")
            .expect("app result");

        assert_eq!(app.status, UpdateResultStatus::Error);
        assert!(app.message.contains("github.com"));
    }

    #[test]
    fn update_collects_srs_assets_from_routing_and_dns() {
        let mut config = AppConfig::default();
        config.const_item.srs_source_url =
            Some("https://rules.example/rule-set-{0}/{1}.srs".to_string());
        let routing = RoutingItem {
            rule_set: vec![RulesItem {
                ip: Some(vec!["geoip:private".to_string(), "1.1.1.1".to_string()]),
                domain: Some(vec!["geosite:cn".to_string()]),
                rule_type: Some(RuleType::Routing),
                ..RulesItem::default()
            }],
            ..RoutingItem::default()
        };
        let dns = DnsItem {
            normal_dns: Some(
                r#"{"rules":[{"rule_set":["geosite-google","geoip-cloudflare"]}]}"#.to_string(),
            ),
            ..DnsItem::default()
        };

        let assets = collect_srs_assets(&config, &[routing], &[dns]);
        let names = assets
            .iter()
            .map(|asset| asset.file_name.as_str())
            .collect::<BTreeSet<_>>();

        assert!(names.contains("geoip-private.srs"));
        assert!(names.contains("geoip-cloudflare.srs"));
        assert!(names.contains("geosite-cn.srs"));
        assert!(names.contains("geosite-google.srs"));
        assert!(names.contains("geosite-category-ads-all.srs"));
        assert!(assets
            .iter()
            .any(|asset| asset.url == "https://rules.example/rule-set-geoip/geoip-private.srs"));
    }

    #[tokio::test]
    async fn ruleset_source_settings_trim_blank_sources() {
        let mut config = AppConfig::default();
        let database = Database::connect_in_memory()
            .await
            .expect("update manager test operation should succeed");
        let manager = UpdateManager::new(
            &database,
            AppPaths::new("/tmp/VoyaVPN", StorageMode::Portable),
        );

        let saved = manager.save_source_settings(
            &mut config,
            RulesetGeoSourceSettings {
                geo_source_url: Some("  ".to_string()),
                srs_source_url: Some(" https://rules.example/{0}/{1}.srs ".to_string()),
            },
        );

        assert_eq!(saved.geo_source_url, None);
        assert_eq!(
            saved.srs_source_url.as_deref(),
            Some("https://rules.example/{0}/{1}.srs")
        );
        assert_eq!(config.const_item.geo_source_url, None);
    }

    #[test]
    fn ruleset_local_path_discovery_uses_acquired_srs_directory() {
        let root = unique_temp_root("ruleset-local");
        let paths = AppPaths::new(&root, StorageMode::Portable);
        let srs_dir = paths.bin_dir().join("srss");
        fs::create_dir_all(&srs_dir).expect("srs dir");
        fs::write(srs_dir.join("geosite-cn.srs"), b"srs").expect("srs file");

        let discovered = local_singbox_ruleset_paths(&paths);

        assert_eq!(
            discovered.get("geosite-cn"),
            Some(
                &srs_dir
                    .join("geosite-cn.srs")
                    .to_string_lossy()
                    .into_owned()
            )
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_status_lists_app_geo_and_srs() {
        let config = AppConfig::default();
        let status = update_status(&config);
        let ids = status
            .targets
            .iter()
            .map(|target| target.id.as_str())
            .collect::<BTreeSet<_>>();

        assert!(ids.contains("app"));
        assert!(ids.contains("geo"));
        assert!(ids.contains("srs"));
        assert!(!ids.iter().any(|id| id.starts_with("core:")));
    }

    fn manifest_os_label(os: AssetOs) -> &'static str {
        match os {
            AssetOs::Windows => "windows",
            AssetOs::Linux => "linux",
            AssetOs::Macos => "macos",
        }
    }

    fn manifest_arch_label(arch: AssetArch) -> &'static str {
        match arch {
            AssetArch::X64 => "x64",
            AssetArch::Arm64 => "arm64",
            AssetArch::Riscv64 => "riscv64",
        }
    }

    async fn spawn_http_fixture(routes: HashMap<String, String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("update manager test operation should succeed");
        let address = listener
            .local_addr()
            .expect("update manager test operation should succeed");
        let max_requests = routes.len();
        let routes = Arc::new(routes);

        tokio::spawn(async move {
            for _ in 0..max_requests {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let routes = Arc::clone(&routes);
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

    fn unique_temp_root(name: &str) -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        env::temp_dir().join(format!(
            "voyavpn-update-{name}-{}-{}",
            std::process::id(),
            millis
        ))
    }
}
