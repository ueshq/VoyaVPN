use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use flate2::read::GzDecoder;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
        check_app_from_cdn_release_index, check_core_from_cdn_manifest,
        check_package_from_releases, core_acquisition_policy, parse_github_releases, parse_version,
        release_package_for_core, supported_release_packages, updatable_core_types, AssetArch,
        AssetOs, BinaryAcquisition, CdnCoreAssetManifest, CdnReleaseIndex, CdnUpdateClient,
        PackageTarget, ReleaseCheck, ReleaseError, ReleaseFetchOptions, ReleasePackage,
        UpstreamReleaseEvidence, CDN_CORE_ASSET_MANIFEST_FILE, CDN_RELEASE_INDEX_FILE,
    },
    DownloadClient, DownloadError, DownloadRequest,
};
use voya_platform::{
    coreinfo::{
        core_type_dir_name, discover_executable, ensure_executable_permission,
        executable_name_for_current_os, get_core_info, CoreInfoError,
    },
    paths::AppPaths,
};

use crate::{
    runtime::{RuntimeError, RuntimeManager},
    supervisor::{SupervisorConnectionState, SupervisorSnapshot},
};

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
    #[error("failed to run version probe for {core_type:?}: {source}")]
    VersionProbe {
        core_type: CoreType,
        #[source]
        source: io::Error,
    },
    #[error("staged update directory {0} does not exist")]
    MissingStagedDir(PathBuf),
    #[error("safe binary swap failed at {path}: {source}")]
    SwapIo { path: PathBuf, source: io::Error },
    #[error("downloaded core update for {target_id} is missing {field}")]
    MissingDownloadedCoreField {
        target_id: String,
        field: &'static str,
    },
    #[error("invalid sha256 checksum {0:?}")]
    InvalidSha256(String),
    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("downloaded core archive path is outside the update staging directory")]
    UnsafeArchivePath,
    #[error("unsupported core archive format for {0}")]
    UnsupportedArchiveFormat(PathBuf),
    #[error("failed to read or write archive {path}: {source}")]
    ArchiveIo { path: PathBuf, source: io::Error },
    #[error("failed to extract zip archive {path}: {source}")]
    ZipArchive {
        path: PathBuf,
        source: zip::result::ZipError,
    },
    #[error("unsafe archive entry {entry:?} in {path}")]
    UnsafeArchiveEntry { path: PathBuf, entry: String },
    #[error("extracted {core_type:?} archive did not contain an executable in {staged_dir}; expected one of: {candidates}")]
    MissingExtractedExecutable {
        core_type: CoreType,
        staged_dir: PathBuf,
        candidates: String,
    },
    #[error(transparent)]
    CoreInfo(#[from] CoreInfoError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateTargetKind {
    App,
    Core,
    Geo,
    Srs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateAcquisition {
    AppPackage,
    DownloadOnFirstRun,
    OptionalDownload,
    Unsupported,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CoreUpdateApplyRequest {
    pub target_id: String,
    pub file_name: String,
    pub sha256: String,
    pub remote_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CoreUpdateApplyResult {
    pub applied_version: String,
    pub core_type: CoreType,
    pub target_dir: String,
    pub rollback_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreUpdateRuntimeApplyResult {
    pub update: CoreUpdateApplyResult,
    pub stopped_runtime: Option<SupervisorSnapshot>,
    pub restarted_runtime: Option<SupervisorSnapshot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RulesetGeoSourceSettings {
    pub geo_source_url: Option<String>,
    pub srs_source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinarySwapPlan {
    pub target_dir: PathBuf,
    pub staged_dir: PathBuf,
    pub backup_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinarySwapOutcome {
    pub target_dir: PathBuf,
    pub backup_dir: Option<PathBuf>,
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

    pub async fn apply_core_update(
        &self,
        request: &CoreUpdateApplyRequest,
    ) -> Result<CoreUpdateApplyResult> {
        let paths = self.paths.clone();
        let request = request.clone();
        spawn_update_blocking("apply core update", move || {
            apply_downloaded_core_update(&paths, &request)
        })
        .await
    }

    async fn current_package_version(&self, package: &ReleasePackage) -> Result<Option<Version>> {
        match package.target {
            PackageTarget::App => Ok(parse_version(env!("CARGO_PKG_VERSION"))),
            PackageTarget::Core(core_type) => self.installed_core_version(core_type).await,
        }
    }

    async fn installed_core_version(&self, core_type: CoreType) -> Result<Option<Version>> {
        let paths = self.paths.clone();
        spawn_update_blocking("core version probe", move || {
            installed_core_version_blocking(&paths, core_type)
        })
        .await
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
        let needs_core_manifest = selected.iter().any(|id| id.starts_with("core:"));

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

        let core_manifest = if needs_core_manifest {
            match urls.core_manifest_url {
                Some(url) => match self.cdn.fetch_core_manifest(&url, options).await {
                    Ok(manifest) => ManifestLoad::Loaded(manifest),
                    Err(error) => ManifestLoad::Failed(error.to_string()),
                },
                None => ManifestLoad::Missing(
                    ReleaseError::MissingCdnManifestUrl {
                        manifest: CDN_CORE_ASSET_MANIFEST_FILE,
                    }
                    .to_string(),
                ),
            }
        } else {
            ManifestLoad::NotNeeded
        };

        CdnUpdateMetadata {
            release_index,
            core_manifest,
        }
    }
}

async fn spawn_update_blocking<T, F>(operation: &'static str, task: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|source| UpdateManagerError::BlockingTaskJoin { operation, source })?
}

fn installed_core_version_blocking(
    paths: &AppPaths,
    core_type: CoreType,
) -> Result<Option<Version>> {
    let Some(core_info) = get_core_info(core_type) else {
        return Ok(None);
    };
    let Some(version_arg) = core_info.version_arg else {
        return Ok(None);
    };
    let Ok(executable) = discover_executable(paths, core_info) else {
        return Ok(None);
    };

    let output = Command::new(executable)
        .args(version_arg.split_whitespace())
        .output()
        .map_err(|source| UpdateManagerError::VersionProbe { core_type, source })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    Ok(parse_core_version_output(core_type, &combined))
}

pub async fn apply_downloaded_core_update_with_runtime(
    update_manager: &UpdateManager<'_>,
    runtime_manager: &RuntimeManager<'_>,
    config: &AppConfig,
    request: &CoreUpdateApplyRequest,
) -> Result<CoreUpdateRuntimeApplyResult> {
    let core_type = core_type_for_update_target_id(&request.target_id)
        .ok_or_else(|| UpdateManagerError::UnsupportedTarget(request.target_id.clone()))?;
    let snapshot = runtime_manager.status().await?;
    let should_stop = snapshot.state == SupervisorConnectionState::Connected
        && snapshot.running_core_type == Some(core_type);
    let should_restart = should_stop && should_resume_runtime_after_apply(&snapshot, config);
    let stopped_runtime = if should_stop {
        Some(runtime_manager.disconnect().await?)
    } else {
        None
    };

    match update_manager.apply_core_update(request).await {
        Ok(update) => {
            let restarted_runtime = if should_restart {
                Some(runtime_manager.restart(config).await?)
            } else {
                None
            };

            Ok(CoreUpdateRuntimeApplyResult {
                update,
                stopped_runtime,
                restarted_runtime,
            })
        }
        Err(error) => {
            if should_restart {
                let _ = runtime_manager.restart(config).await;
            }
            Err(error)
        }
    }
}

fn should_resume_runtime_after_apply(snapshot: &SupervisorSnapshot, config: &AppConfig) -> bool {
    let current_profile_id = config.index_id.trim();
    !current_profile_id.is_empty()
        && snapshot
            .active_profile_id
            .as_deref()
            .is_some_and(|active_profile_id| active_profile_id == current_profile_id)
}

#[derive(Debug, Clone)]
struct CdnUpdateMetadata {
    release_index: ManifestLoad<CdnReleaseIndex>,
    core_manifest: ManifestLoad<CdnCoreAssetManifest>,
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
    core_manifest_url: Option<String>,
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
        PackageTarget::Core(_) => check_core_from_cdn_manifest(
            package,
            current_version,
            os,
            arch,
            metadata.core_manifest.loaded()?,
        ),
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
    let core_manifest_url = normalize_optional_url(config.const_item.cdn_core_manifest_url.clone())
        .or_else(|| {
            base_url
                .as_deref()
                .and_then(|base| cdn_manifest_url_from_base(base, CDN_CORE_ASSET_MANIFEST_FILE))
        });

    ConfiguredUpdateManifestUrls {
        release_index_url,
        core_manifest_url,
    }
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
        targets.push(package_to_target(&package, selected.contains(package.id)));
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

pub fn parse_core_version_output(core_type: CoreType, output: &str) -> Option<Version> {
    match core_type {
        CoreType::Xray | CoreType::v2fly | CoreType::v2fly_v5 => output
            .lines()
            .find(|line| {
                let lower = line.to_ascii_lowercase();
                lower.contains("xray") || lower.contains("v2ray")
            })
            .and_then(parse_version)
            .or_else(|| parse_version(output)),
        CoreType::mihomo | CoreType::sing_box => parse_version(output),
        _ => parse_version(output),
    }
}

#[must_use]
pub fn selected_target_ids(config: &AppConfig, override_ids: &[String]) -> BTreeSet<String> {
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

#[must_use]
pub fn core_type_for_update_target_id(target_id: &str) -> Option<CoreType> {
    let normalized = normalize_target_id(target_id);
    supported_release_packages()
        .into_iter()
        .find(|package| package.id == normalized)
        .and_then(|package| match package.target {
            PackageTarget::Core(core_type) => Some(core_type),
            PackageTarget::App => None,
        })
}

pub fn apply_downloaded_core_update(
    paths: &AppPaths,
    request: &CoreUpdateApplyRequest,
) -> Result<CoreUpdateApplyResult> {
    let core_type = core_type_for_update_target_id(&request.target_id)
        .ok_or_else(|| UpdateManagerError::UnsupportedTarget(request.target_id.clone()))?;
    let file_name = required_update_field(&request.target_id, "fileName", &request.file_name)?;
    let applied_version =
        required_update_field(&request.target_id, "remoteVersion", &request.remote_version)?
            .to_string();
    let expected_sha256 = normalize_sha256(&request.sha256)?;
    let archive_path = canonicalize_update_archive_path(paths, Path::new(file_name))?;

    verify_sha256(&archive_path, &expected_sha256)?;

    let archive_format = CoreArchiveFormat::from_path(&archive_path)
        .ok_or_else(|| UpdateManagerError::UnsupportedArchiveFormat(archive_path.clone()))?;
    let staged_dir = paths.temp_file(format!(
        "updates/stage-{}-{}",
        core_type_dir_name(core_type),
        monotonic_millis()
    ));

    if staged_dir.exists() {
        fs::remove_dir_all(&staged_dir).map_err(|source| UpdateManagerError::ArchiveIo {
            path: staged_dir.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&staged_dir).map_err(|source| UpdateManagerError::ArchiveIo {
        path: staged_dir.clone(),
        source,
    })?;

    if let Err(error) =
        extract_and_prepare_core_archive(&archive_path, &staged_dir, archive_format, core_type)
    {
        let _ = fs::remove_dir_all(&staged_dir);
        return Err(error);
    }

    let outcome =
        match swap_staged_binary_dir(&swap_plan_for_core(paths, core_type, staged_dir.clone())) {
            Ok(outcome) => outcome,
            Err(error) => {
                let _ = fs::remove_dir_all(&staged_dir);
                return Err(error);
            }
        };

    let _ = fs::remove_file(&archive_path);

    Ok(CoreUpdateApplyResult {
        applied_version,
        core_type,
        target_dir: outcome.target_dir.to_string_lossy().into_owned(),
        rollback_path: outcome
            .backup_dir
            .map(|path| path.to_string_lossy().into_owned()),
    })
}

fn canonicalize_update_archive_path(paths: &AppPaths, archive_path: &Path) -> Result<PathBuf> {
    let updates_dir = paths.temp_file("updates");
    let updates_dir =
        fs::canonicalize(&updates_dir).map_err(|_| UpdateManagerError::UnsafeArchivePath)?;
    let archive_path =
        fs::canonicalize(archive_path).map_err(|_| UpdateManagerError::UnsafeArchivePath)?;

    if archive_path.starts_with(updates_dir) {
        Ok(archive_path)
    } else {
        Err(UpdateManagerError::UnsafeArchivePath)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoreArchiveFormat {
    Zip,
    TarGz,
    Gz,
}

impl CoreArchiveFormat {
    fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".zip") {
            Some(Self::Zip)
        } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            Some(Self::TarGz)
        } else if name.ends_with(".gz") {
            Some(Self::Gz)
        } else {
            None
        }
    }
}

fn required_update_field<'a>(
    target_id: &str,
    field: &'static str,
    value: &'a str,
) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        Err(UpdateManagerError::MissingDownloadedCoreField {
            target_id: target_id.to_string(),
            field,
        })
    } else {
        Ok(value)
    }
}

fn normalize_sha256(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(value)
    } else {
        Err(UpdateManagerError::InvalidSha256(value))
    }
}

fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    use std::io::Read as _;

    let mut file = fs::File::open(path).map_err(|source| UpdateManagerError::ArchiveIo {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| UpdateManagerError::ArchiveIo {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    let actual = sha256_hex(&digest);
    if actual == expected {
        Ok(())
    } else {
        Err(UpdateManagerError::ChecksumMismatch {
            path: path.to_path_buf(),
            expected: expected.to_string(),
            actual,
        })
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn extract_and_prepare_core_archive(
    archive_path: &Path,
    staged_dir: &Path,
    archive_format: CoreArchiveFormat,
    core_type: CoreType,
) -> Result<Vec<PathBuf>> {
    match archive_format {
        CoreArchiveFormat::Zip => extract_zip_archive(archive_path, staged_dir)?,
        CoreArchiveFormat::TarGz => extract_tar_gz_archive(archive_path, staged_dir)?,
        CoreArchiveFormat::Gz => extract_single_gz_archive(archive_path, staged_dir, core_type)?,
    }

    normalize_staged_core_dir(staged_dir, core_type)
}

fn extract_zip_archive(archive_path: &Path, staged_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive_path).map_err(|source| UpdateManagerError::ArchiveIo {
        path: archive_path.to_path_buf(),
        source,
    })?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|source| UpdateManagerError::ZipArchive {
            path: archive_path.to_path_buf(),
            source,
        })?;

    archive
        .extract(staged_dir)
        .map_err(|source| UpdateManagerError::ZipArchive {
            path: archive_path.to_path_buf(),
            source,
        })
}

fn extract_tar_gz_archive(archive_path: &Path, staged_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive_path).map_err(|source| UpdateManagerError::ArchiveIo {
        path: archive_path.to_path_buf(),
        source,
    })?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|source| UpdateManagerError::ArchiveIo {
            path: archive_path.to_path_buf(),
            source,
        })?;

    for entry in entries {
        let mut entry = entry.map_err(|source| UpdateManagerError::ArchiveIo {
            path: archive_path.to_path_buf(),
            source,
        })?;
        let entry_path = entry
            .path()
            .map_err(|source| UpdateManagerError::ArchiveIo {
                path: archive_path.to_path_buf(),
                source,
            })?
            .to_path_buf();
        let unpacked =
            entry
                .unpack_in(staged_dir)
                .map_err(|source| UpdateManagerError::ArchiveIo {
                    path: archive_path.to_path_buf(),
                    source,
                })?;
        if !unpacked {
            return Err(UpdateManagerError::UnsafeArchiveEntry {
                path: archive_path.to_path_buf(),
                entry: entry_path.to_string_lossy().into_owned(),
            });
        }
    }

    Ok(())
}

fn extract_single_gz_archive(
    archive_path: &Path,
    staged_dir: &Path,
    core_type: CoreType,
) -> Result<()> {
    let output_name = single_gz_output_name(archive_path, core_type);
    let output_path = staged_dir.join(output_name);
    let input = fs::File::open(archive_path).map_err(|source| UpdateManagerError::ArchiveIo {
        path: archive_path.to_path_buf(),
        source,
    })?;
    let mut decoder = GzDecoder::new(input);
    let mut output =
        fs::File::create(&output_path).map_err(|source| UpdateManagerError::ArchiveIo {
            path: output_path.clone(),
            source,
        })?;

    io::copy(&mut decoder, &mut output).map_err(|source| UpdateManagerError::ArchiveIo {
        path: archive_path.to_path_buf(),
        source,
    })?;

    Ok(())
}

fn single_gz_output_name(archive_path: &Path, core_type: CoreType) -> String {
    let file_name = archive_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("core.gz");
    let stem = file_name.strip_suffix(".gz").unwrap_or(file_name);
    let stem_lower = stem.to_ascii_lowercase();

    if let Some(core_info) = get_core_info(core_type) {
        for name in core_info.executable_names() {
            let candidate = executable_name_for_current_os(name);
            let candidate_lower = candidate.to_ascii_lowercase();
            if stem_lower == candidate_lower
                || stem_lower.starts_with(&format!("{candidate_lower}-"))
                || stem_lower.starts_with(&format!("{candidate_lower}_"))
            {
                return candidate;
            }
        }
    }

    stem.to_string()
}

fn normalize_staged_core_dir(staged_dir: &Path, core_type: CoreType) -> Result<Vec<PathBuf>> {
    if !staged_executable_candidates(staged_dir, core_type)?.is_empty() {
        return chmod_staged_executables(staged_dir, core_type);
    }

    flatten_single_top_level_dir(staged_dir)?;

    let chmod_paths = chmod_staged_executables(staged_dir, core_type)?;
    if chmod_paths.is_empty() {
        return Err(UpdateManagerError::MissingExtractedExecutable {
            core_type,
            staged_dir: staged_dir.to_path_buf(),
            candidates: executable_candidate_list(core_type)?,
        });
    }

    Ok(chmod_paths)
}

fn flatten_single_top_level_dir(staged_dir: &Path) -> Result<()> {
    let entries = read_dir_paths(staged_dir)?;
    if entries.len() != 1 || !entries[0].is_dir() {
        return Ok(());
    }

    let top_level_dir = entries[0].clone();
    for entry in fs::read_dir(&top_level_dir).map_err(|source| UpdateManagerError::ArchiveIo {
        path: top_level_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| UpdateManagerError::ArchiveIo {
            path: top_level_dir.clone(),
            source,
        })?;
        let source_path = entry.path();
        let target_path = staged_dir.join(entry.file_name());
        fs::rename(&source_path, &target_path).map_err(|source| UpdateManagerError::ArchiveIo {
            path: source_path,
            source,
        })?;
    }

    fs::remove_dir(&top_level_dir).map_err(|source| UpdateManagerError::ArchiveIo {
        path: top_level_dir,
        source,
    })?;

    Ok(())
}

fn read_dir_paths(path: &Path) -> Result<Vec<PathBuf>> {
    fs::read_dir(path)
        .map_err(|source| UpdateManagerError::ArchiveIo {
            path: path.to_path_buf(),
            source,
        })?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|source| UpdateManagerError::ArchiveIo {
                    path: path.to_path_buf(),
                    source,
                })
        })
        .collect()
}

fn chmod_staged_executables(staged_dir: &Path, core_type: CoreType) -> Result<Vec<PathBuf>> {
    let mut chmod_paths = Vec::new();
    for candidate in staged_executable_candidates(staged_dir, core_type)? {
        ensure_executable_permission(&candidate)?;
        chmod_paths.push(candidate);
    }

    Ok(chmod_paths)
}

fn staged_executable_candidates(staged_dir: &Path, core_type: CoreType) -> Result<Vec<PathBuf>> {
    let core_info = get_core_info(core_type).ok_or(CoreInfoError::MissingCoreInfo(core_type))?;
    let mut candidates = Vec::new();

    for name in core_info.executable_names() {
        let candidate = staged_dir.join(executable_name_for_current_os(name));
        match candidate.try_exists() {
            Ok(true) if candidate.is_file() => candidates.push(candidate),
            Ok(_) => {}
            Err(source) => {
                return Err(UpdateManagerError::ArchiveIo {
                    path: candidate,
                    source,
                });
            }
        }
    }

    Ok(candidates)
}

fn executable_candidate_list(core_type: CoreType) -> Result<String> {
    let core_info = get_core_info(core_type).ok_or(CoreInfoError::MissingCoreInfo(core_type))?;
    Ok(core_info
        .executable_names()
        .iter()
        .map(|name| executable_name_for_current_os(name))
        .collect::<Vec<_>>()
        .join(", "))
}

pub fn swap_staged_binary_dir(plan: &BinarySwapPlan) -> Result<BinarySwapOutcome> {
    if !plan.staged_dir.is_dir() {
        return Err(UpdateManagerError::MissingStagedDir(
            plan.staged_dir.clone(),
        ));
    }
    if let Some(parent) = plan.target_dir.parent() {
        fs::create_dir_all(parent).map_err(|source| UpdateManagerError::SwapIo {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let mut backup_dir = None;
    if plan.target_dir.exists() {
        if let Some(parent) = plan.backup_dir.parent() {
            fs::create_dir_all(parent).map_err(|source| UpdateManagerError::SwapIo {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        if plan.backup_dir.exists() {
            fs::remove_dir_all(&plan.backup_dir).map_err(|source| UpdateManagerError::SwapIo {
                path: plan.backup_dir.clone(),
                source,
            })?;
        }
        fs::rename(&plan.target_dir, &plan.backup_dir).map_err(|source| {
            UpdateManagerError::SwapIo {
                path: plan.target_dir.clone(),
                source,
            }
        })?;
        backup_dir = Some(plan.backup_dir.clone());
    }

    if let Err(source) = fs::rename(&plan.staged_dir, &plan.target_dir) {
        if let Some(backup) = &backup_dir {
            let _ = fs::rename(backup, &plan.target_dir);
        }
        return Err(UpdateManagerError::SwapIo {
            path: plan.staged_dir.clone(),
            source,
        });
    }

    Ok(BinarySwapOutcome {
        target_dir: plan.target_dir.clone(),
        backup_dir,
    })
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

fn package_to_target(package: &ReleasePackage, selected: bool) -> UpdateTarget {
    let (kind, core_type) = match package.target {
        PackageTarget::App => (UpdateTargetKind::App, None),
        PackageTarget::Core(core_type) => (UpdateTargetKind::Core, Some(core_type)),
    };

    UpdateTarget {
        id: package.id.to_string(),
        name: package.name.to_string(),
        kind,
        core_type,
        selected,
        update_supported: true,
        license: package.policy.license.map(ToString::to_string),
        acquisition: acquisition(package.policy.acquisition),
        redistribute_in_installer: package.policy.redistribute_in_installer,
        remarks: match package.policy.acquisition {
            BinaryAcquisition::DownloadOnFirstRun => {
                "download on first run; not bundled in installers".to_string()
            }
            BinaryAcquisition::AppPackage => "application package update".to_string(),
            BinaryAcquisition::OptionalDownload => "optional download".to_string(),
            BinaryAcquisition::Unsupported => "update check unsupported".to_string(),
        },
    }
}

fn acquisition(value: BinaryAcquisition) -> UpdateAcquisition {
    match value {
        BinaryAcquisition::AppPackage => UpdateAcquisition::AppPackage,
        BinaryAcquisition::DownloadOnFirstRun => UpdateAcquisition::DownloadOnFirstRun,
        BinaryAcquisition::OptionalDownload => UpdateAcquisition::OptionalDownload,
        BinaryAcquisition::Unsupported => UpdateAcquisition::Unsupported,
    }
}

fn default_selected_targets() -> Vec<String> {
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
        "Xray" => "core:xray".to_string(),
        "mihomo" => "core:mihomo".to_string(),
        "sing_box" => "core:sing_box".to_string(),
        _ => value.to_string(),
    }
}

#[must_use]
pub fn core_download_dir(paths: &AppPaths, core_type: CoreType) -> PathBuf {
    paths.core_bin_dir(core_type_dir_name(core_type))
}

#[must_use]
pub fn default_core_target_ids() -> Vec<String> {
    updatable_core_types()
        .iter()
        .filter_map(|core_type| release_package_for_core(*core_type))
        .map(|package| package.id.to_string())
        .collect()
}

#[must_use]
pub fn gpl_or_agpl_core_policies() -> Vec<(CoreType, UpdateAcquisition, bool)> {
    [CoreType::mihomo, CoreType::sing_box, CoreType::juicity]
        .into_iter()
        .map(|core_type| {
            let policy = core_acquisition_policy(core_type);
            (
                core_type,
                acquisition(policy.acquisition),
                policy.redistribute_in_installer,
            )
        })
        .collect()
}

#[must_use]
pub fn swap_plan_for_core(
    paths: &AppPaths,
    core_type: CoreType,
    staged_dir: PathBuf,
) -> BinarySwapPlan {
    let target_dir = core_download_dir(paths, core_type);
    let backup_dir = paths.temp_file(format!(
        "updates/backup-{}-{}",
        core_type_dir_name(core_type),
        monotonic_millis()
    ));

    BinarySwapPlan {
        target_dir,
        staged_dir,
        backup_dir,
    }
}

fn monotonic_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, env, io::Write as _, path::Path, sync::Arc};

    use flate2::{write::GzEncoder, Compression};
    use sha2::{Digest as _, Sha256};
    use tar::{Builder as TarBuilder, Header};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use zip::{write::FileOptions, ZipWriter};

    use voya_core::{ConfigType, ProfileItem, RoutingItem, RuleType, RulesItem};
    use voya_net::update::UpstreamAssetTemplates;
    use voya_platform::{
        coreinfo::{executable_name_for_current_os, TargetOs},
        elevation::SudoPasswordStore,
        paths::StorageMode,
        test_support::RecordingRunner,
    };

    use super::*;
    use crate::{
        runtime::RuntimeManager,
        supervisor::{CoreSupervisor, SupervisorConnectionState, SupervisorDeps},
    };

    #[test]
    fn update_version_parsing_matches_supported_core_outputs() {
        assert_eq!(
            parse_core_version_output(CoreType::Xray, "Xray 1.8.7 (Xray, Penetrates Everything.)"),
            Some(Version::new(1, 8, 7))
        );
        assert_eq!(
            parse_core_version_output(CoreType::mihomo, "Mihomo Meta v1.18.4 linux amd64"),
            Some(Version::new(1, 18, 4))
        );
        assert_eq!(
            parse_core_version_output(CoreType::sing_box, "sing-box version 1.10.0"),
            Some(Version::new(1, 10, 0))
        );
    }

    #[test]
    fn update_selected_target_ids_accept_legacy_v2rayn_storage_names() {
        let mut config = AppConfig::default();
        config.check_update_item.selected_core_types = Some(vec![
            "Xray".to_string(),
            "mihomo".to_string(),
            "sing_box".to_string(),
            "GeoFiles".to_string(),
        ]);

        let selected = selected_target_ids(&config, &[]);
        assert!(selected.contains("core:xray"));
        assert!(selected.contains("core:mihomo"));
        assert!(selected.contains("core:sing_box"));
        assert!(selected.contains("geo"));
    }

    #[test]
    fn update_check_fixture_selects_asset_and_marks_newer_version() {
        let package = release_package_for_core(CoreType::Xray).expect("xray package");
        let upstream = xray_upstream_release_evidence();
        let result = check_package_fixture(
            &package,
            &upstream,
            r#"[{
                "tag_name": "v1.9.0",
                "prerelease": false,
                "assets": [{
                    "name": "Xray-linux-64.zip",
                    "browser_download_url": "https://cdn.example/Xray-linux-64.zip"
                }]
            }]"#,
            Some(&Version::new(1, 8, 0)),
            AssetOs::Linux,
            AssetArch::X64,
            false,
        )
        .expect("check result");

        assert_eq!(result.status, UpdateResultStatus::UpdateAvailable);
        assert_eq!(
            result.download_url.as_deref(),
            Some("https://cdn.example/Xray-linux-64.zip")
        );
    }

    fn xray_upstream_release_evidence() -> UpstreamReleaseEvidence {
        UpstreamReleaseEvidence {
            release_api_url: "https://api.github.com/repos/XTLS/Xray-core/releases",
            release_url: "https://github.com/XTLS/Xray-core/releases",
            asset_templates: UpstreamAssetTemplates {
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
        }
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
        assert_eq!(
            urls.core_manifest_url.as_deref(),
            Some("https://cdn.voyavpn.test/stable/core-assets.json")
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
    async fn updates_check_uses_cdn_manifests_and_returns_checksums() {
        let Some(os) = AssetOs::current() else {
            return;
        };
        let Some(arch) = AssetArch::current() else {
            return;
        };
        let os_label = manifest_os_label(os);
        let arch_label = manifest_arch_label(arch);
        let app_name = format!("VoyaVPN-{os_label}-{arch_label}.zip");
        let core_name = format!("Xray-{os_label}-{arch_label}.zip");
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
        let core_manifest = format!(
            r#"{{
              "productName": "VoyaVPN",
              "manifestVersion": 1,
              "channel": "stable",
              "baseUrl": "http://127.0.0.1/stable",
              "assets": [{{
                "coreType": "Xray",
                "version": "9.9.9",
                "license": "MPL-2.0",
                "os": "{os_label}",
                "arch": "{arch_label}",
                "archiveFormat": "zip",
                "executableCandidates": ["xray"],
                "url": "http://127.0.0.1/stable/{core_name}",
                "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "bytes": 64,
                "upstreamUrl": "https://github.com/XTLS/Xray-core/releases/download/v9.9.9/{core_name}",
                "name": "{core_name}",
                "path": "{core_name}"
              }}]
            }}"#
        );
        let base = spawn_http_fixture(HashMap::from([
            ("/stable/release-index.json".to_string(), release_index),
            ("/stable/core-assets.json".to_string(), core_manifest),
        ]))
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
                    selected_target_ids: vec!["app".to_string(), "core:xray".to_string()],
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
        let xray = run
            .results
            .iter()
            .find(|result| result.target_id == "core:xray")
            .expect("xray result");

        assert_eq!(app.status, UpdateResultStatus::UpdateAvailable);
        assert_eq!(app.remote_version.as_deref(), Some("9.9.9"));
        assert_eq!(
            app.sha256.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(app.bytes, Some(42));
        assert_eq!(xray.status, UpdateResultStatus::UpdateAvailable);
        assert_eq!(
            xray.sha256.as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        for result in [app, xray] {
            assert!(!result
                .download_url
                .as_deref()
                .unwrap_or_default()
                .contains("github.com"));
        }

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
    fn update_gpl_or_agpl_cores_stay_download_on_first_run() {
        for (_core_type, acquisition, redistribute) in gpl_or_agpl_core_policies() {
            assert_eq!(acquisition, UpdateAcquisition::DownloadOnFirstRun);
            assert!(!redistribute);
        }
    }

    #[test]
    fn update_safe_binary_swap_replaces_target_and_keeps_backup() {
        let root = unique_temp_root("swap");
        let target = root.join("bin").join("xray");
        let staged = root.join("stage").join("xray");
        let backup = root.join("backup").join("xray");
        fs::create_dir_all(&target).expect("target dir");
        fs::create_dir_all(&staged).expect("stage dir");
        fs::write(target.join("old"), b"old").expect("old file");
        fs::write(staged.join("new"), b"new").expect("new file");

        let outcome = swap_staged_binary_dir(&BinarySwapPlan {
            target_dir: target.clone(),
            staged_dir: staged.clone(),
            backup_dir: backup.clone(),
        })
        .expect("swap");

        assert_eq!(outcome.backup_dir.as_ref(), Some(&backup));
        assert!(target.join("new").exists());
        assert!(backup.join("old").exists());
        assert!(!staged.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_safe_binary_swap_rolls_back_when_staged_moves_with_target() {
        let root = unique_temp_root("swap-rollback");
        let target = root.join("bin").join("xray");
        let staged = target.join("staged");
        let backup = root.join("backup").join("xray");
        fs::create_dir_all(&staged).expect("stage dir inside target");
        fs::write(target.join("old"), b"old").expect("old file");
        fs::write(staged.join("new"), b"new").expect("new file");

        let error = swap_staged_binary_dir(&BinarySwapPlan {
            target_dir: target.clone(),
            staged_dir: staged.clone(),
            backup_dir: backup.clone(),
        })
        .expect_err("swap should fail because staged path moved with target");

        assert!(matches!(error, UpdateManagerError::SwapIo { .. }));
        assert!(target.join("old").exists());
        assert!(target.join("staged").join("new").exists());
        assert!(!backup.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_apply_zip_archive_verifies_checksum_extracts_and_swaps() {
        let root = unique_temp_root("apply-zip");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        let exe_name = executable_name_for_current_os("xray");
        let target_exe = paths.core_bin_file(core_type_dir_name(CoreType::Xray), &exe_name);
        fs::create_dir_all(target_exe.parent().expect("target exe parent")).expect("target dir");
        fs::write(&target_exe, b"old-xray").expect("old xray");
        let archive = update_archive_path(&paths, "Xray-linux-64.zip");
        write_zip_archive(
            &archive,
            &[(&exe_name, b"new-xray".as_slice()), ("geoip.dat", b"geo")],
        );

        let result = apply_downloaded_core_update(
            &paths,
            &apply_request(CoreType::Xray, &archive, "1.8.24"),
        )
        .expect("apply zip");

        assert_eq!(result.applied_version, "1.8.24");
        assert_eq!(result.core_type, CoreType::Xray);
        assert_eq!(
            result.target_dir,
            paths
                .core_bin_dir(core_type_dir_name(CoreType::Xray))
                .to_string_lossy()
        );
        assert_eq!(fs::read(&target_exe).expect("read new xray"), b"new-xray");
        let rollback_path = PathBuf::from(result.rollback_path.expect("rollback path"));
        assert_eq!(
            fs::read(rollback_path.join(&exe_name)).expect("read backup xray"),
            b"old-xray"
        );
        assert!(!archive.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_apply_tar_gz_archive_flattens_top_level_dir() {
        let root = unique_temp_root("apply-tar-gz");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        let exe_name = executable_name_for_current_os("sing-box");
        let target_exe = paths.core_bin_file(core_type_dir_name(CoreType::sing_box), &exe_name);
        fs::create_dir_all(target_exe.parent().expect("target exe parent")).expect("target dir");
        fs::write(&target_exe, b"old-sing-box").expect("old sing-box");
        let archive = update_archive_path(&paths, "sing-box-1.12.0-linux-amd64.tar.gz");
        let nested_exe = format!("sing-box-1.12.0-linux-amd64/{exe_name}");
        write_tar_gz_archive(&archive, &[(&nested_exe, b"new-sing-box".as_slice())]);

        let result = apply_downloaded_core_update(
            &paths,
            &apply_request(CoreType::sing_box, &archive, "1.12.0"),
        )
        .expect("apply tar.gz");

        assert_eq!(result.core_type, CoreType::sing_box);
        assert_eq!(
            fs::read(&target_exe).expect("read new sing-box"),
            b"new-sing-box"
        );
        assert!(!target_exe
            .parent()
            .expect("target dir")
            .join("sing-box-1.12.0-linux-amd64")
            .exists());
        let rollback_path = PathBuf::from(result.rollback_path.expect("rollback path"));
        assert_eq!(
            fs::read(rollback_path.join(&exe_name)).expect("read backup sing-box"),
            b"old-sing-box"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn update_apply_single_gz_archive_uses_core_candidate_name_and_chmods() {
        use std::os::unix::fs::PermissionsExt;

        let root = unique_temp_root("apply-gz");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        let core_info = get_core_info(CoreType::mihomo).expect("mihomo core info");
        let exe_name = executable_name_for_current_os(core_info.executable_names()[0]);
        let target_exe = paths.core_bin_file(core_type_dir_name(CoreType::mihomo), &exe_name);
        fs::create_dir_all(target_exe.parent().expect("target exe parent")).expect("target dir");
        fs::write(&target_exe, b"old-mihomo").expect("old mihomo");
        let archive = update_archive_path(&paths, format!("{exe_name}-v1.19.15.gz"));
        write_gz_file(&archive, b"new-mihomo");

        apply_downloaded_core_update(
            &paths,
            &apply_request(CoreType::mihomo, &archive, "1.19.15"),
        )
        .expect("apply gz");

        assert_eq!(
            fs::read(&target_exe).expect("read new mihomo"),
            b"new-mihomo"
        );
        let mode = fs::metadata(&target_exe)
            .expect("stat mihomo")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_apply_checksum_failure_leaves_current_core_intact() {
        let root = unique_temp_root("apply-bad-checksum");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        let exe_name = executable_name_for_current_os("xray");
        let target_exe = paths.core_bin_file(core_type_dir_name(CoreType::Xray), &exe_name);
        fs::create_dir_all(target_exe.parent().expect("target exe parent")).expect("target dir");
        fs::write(&target_exe, b"old-xray").expect("old xray");
        let archive = update_archive_path(&paths, "Xray-linux-64.zip");
        write_zip_archive(&archive, &[(&exe_name, b"new-xray".as_slice())]);
        let mut request = apply_request(CoreType::Xray, &archive, "1.8.24");
        request.sha256 =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();

        let error = apply_downloaded_core_update(&paths, &request).expect_err("checksum error");

        assert!(matches!(error, UpdateManagerError::ChecksumMismatch { .. }));
        assert_eq!(fs::read(&target_exe).expect("read old xray"), b"old-xray");
        assert!(archive.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_apply_extraction_failure_leaves_current_core_intact() {
        let root = unique_temp_root("apply-bad-archive");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        let exe_name = executable_name_for_current_os("xray");
        let target_exe = paths.core_bin_file(core_type_dir_name(CoreType::Xray), &exe_name);
        fs::create_dir_all(target_exe.parent().expect("target exe parent")).expect("target dir");
        fs::write(&target_exe, b"old-xray").expect("old xray");
        let archive = update_archive_path(&paths, "Xray-linux-64.zip");
        fs::write(&archive, b"not a zip").expect("corrupt archive");

        let error = apply_downloaded_core_update(
            &paths,
            &apply_request(CoreType::Xray, &archive, "1.8.24"),
        )
        .expect_err("extraction error");

        assert!(matches!(error, UpdateManagerError::ZipArchive { .. }));
        assert_eq!(fs::read(&target_exe).expect("read old xray"), b"old-xray");
        assert!(archive.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_apply_rejects_archive_outside_updates_dir_without_removing_it() {
        let root = unique_temp_root("apply-outside-archive");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        fs::create_dir_all(paths.temp_file("updates")).expect("create updates dir");
        let archive = root.join("Xray-linux-64.zip");
        let exe_name = executable_name_for_current_os("xray");
        write_zip_archive(&archive, &[(&exe_name, b"new-xray".as_slice())]);

        let error = apply_downloaded_core_update(
            &paths,
            &apply_request(CoreType::Xray, &archive, "1.8.24"),
        )
        .expect_err("outside archive should be rejected");

        assert!(matches!(error, UpdateManagerError::UnsafeArchivePath));
        assert!(archive.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn updates_apply_with_runtime_stops_swaps_and_restarts_same_profile() {
        let database = Database::connect_in_memory()
            .await
            .expect("update manager test operation should succeed");
        let root = unique_temp_root("runtime-apply");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        paths
            .ensure_dirs()
            .expect("update manager test operation should succeed");
        let target_exe = write_fake_core_executable(&paths, CoreType::Xray, b"old-xray");
        database
            .profiles()
            .upsert(&active_xray_profile("active"))
            .await
            .expect("update manager test operation should succeed");
        let runner = RecordingRunner::default();
        let runtime = runtime_manager_for(&database, &paths, runner.clone());
        let updates = UpdateManager::new(&database, paths.clone());
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        runtime
            .connect(&config)
            .await
            .expect("update manager test operation should succeed");
        let archive = update_archive_path(&paths, "Xray-linux-64.zip");
        let exe_name = executable_name_for_current_os("xray");
        write_zip_archive(&archive, &[(&exe_name, b"new-xray".as_slice())]);

        let result = apply_downloaded_core_update_with_runtime(
            &updates,
            &runtime,
            &config,
            &apply_request(CoreType::Xray, &archive, "1.8.24"),
        )
        .await
        .expect("runtime apply");

        assert_eq!(result.update.applied_version, "1.8.24");
        assert_eq!(result.update.core_type, CoreType::Xray);
        assert_eq!(
            result
                .stopped_runtime
                .as_ref()
                .map(|snapshot| snapshot.state),
            Some(SupervisorConnectionState::Disconnected)
        );
        assert_eq!(
            result
                .restarted_runtime
                .as_ref()
                .and_then(|snapshot| snapshot.active_profile_id.as_deref()),
            Some("active")
        );
        assert_eq!(fs::read(&target_exe).expect("read new xray"), b"new-xray");
        assert_eq!(runner.spawns().len(), 2);
        assert_eq!(runner.stops().as_slice(), [10]);

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn updates_apply_with_runtime_restarts_after_apply_failure() {
        let database = Database::connect_in_memory()
            .await
            .expect("update manager test operation should succeed");
        let root = unique_temp_root("runtime-apply-failure");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        paths
            .ensure_dirs()
            .expect("update manager test operation should succeed");
        let target_exe = write_fake_core_executable(&paths, CoreType::Xray, b"old-xray");
        database
            .profiles()
            .upsert(&active_xray_profile("active"))
            .await
            .expect("update manager test operation should succeed");
        let runner = RecordingRunner::default();
        let runtime = runtime_manager_for(&database, &paths, runner.clone());
        let updates = UpdateManager::new(&database, paths.clone());
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        runtime
            .connect(&config)
            .await
            .expect("update manager test operation should succeed");
        let archive = update_archive_path(&paths, "Xray-linux-64.zip");
        let exe_name = executable_name_for_current_os("xray");
        write_zip_archive(&archive, &[(&exe_name, b"new-xray".as_slice())]);
        let mut request = apply_request(CoreType::Xray, &archive, "1.8.24");
        request.sha256 =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();

        let error =
            apply_downloaded_core_update_with_runtime(&updates, &runtime, &config, &request)
                .await
                .expect_err("checksum error");

        assert!(matches!(error, UpdateManagerError::ChecksumMismatch { .. }));
        assert_eq!(fs::read(&target_exe).expect("read old xray"), b"old-xray");
        assert!(archive.exists());
        assert_eq!(runner.spawns().len(), 2);
        assert_eq!(runner.stops().as_slice(), [10]);
        assert_eq!(
            runtime.status().await.expect("runtime status").state,
            SupervisorConnectionState::Connected
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn updates_apply_with_runtime_skips_stop_when_no_runtime_is_active() {
        let database = Database::connect_in_memory()
            .await
            .expect("update manager test operation should succeed");
        let root = unique_temp_root("runtime-apply-disconnected");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        paths
            .ensure_dirs()
            .expect("update manager test operation should succeed");
        let target_exe = write_fake_core_executable(&paths, CoreType::Xray, b"old-xray");
        let runner = RecordingRunner::default();
        let runtime = runtime_manager_for(&database, &paths, runner.clone());
        let updates = UpdateManager::new(&database, paths.clone());
        let config = AppConfig {
            index_id: "active".to_string(),
            ..AppConfig::default()
        };
        let archive = update_archive_path(&paths, "Xray-linux-64.zip");
        let exe_name = executable_name_for_current_os("xray");
        write_zip_archive(&archive, &[(&exe_name, b"new-xray".as_slice())]);

        let result = apply_downloaded_core_update_with_runtime(
            &updates,
            &runtime,
            &config,
            &apply_request(CoreType::Xray, &archive, "1.8.24"),
        )
        .await
        .expect("runtime apply");

        assert!(result.stopped_runtime.is_none());
        assert!(result.restarted_runtime.is_none());
        assert_eq!(fs::read(&target_exe).expect("read new xray"), b"new-xray");
        assert!(runner.spawns().is_empty());
        assert!(runner.stops().is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_apply_rejects_app_geo_and_srs_targets() {
        let root = unique_temp_root("apply-non-core");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);
        let archive = root.join("VoyaVPN-linux-x64.zip");
        fs::create_dir_all(archive.parent().expect("archive parent")).expect("archive dir");
        write_zip_archive(&archive, &[("VoyaVPN", b"app".as_slice())]);
        let sha256 = fixture_sha256(&archive);

        for target_id in ["app", "geo", "srs"] {
            let request = CoreUpdateApplyRequest {
                target_id: target_id.to_string(),
                file_name: archive.to_string_lossy().into_owned(),
                sha256: sha256.clone(),
                remote_version: "1.8.24".to_string(),
            };

            let error =
                apply_downloaded_core_update(&paths, &request).expect_err("unsupported target");

            assert!(matches!(error, UpdateManagerError::UnsupportedTarget(_)));
        }
        assert!(archive.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn update_status_lists_app_cores_geo_and_srs() {
        let config = AppConfig::default();
        let status = update_status(&config);
        let ids = status
            .targets
            .iter()
            .map(|target| target.id.as_str())
            .collect::<BTreeSet<_>>();

        assert!(ids.contains("app"));
        assert!(ids.contains("core:xray"));
        assert!(ids.contains("core:mihomo"));
        assert!(ids.contains("core:sing_box"));
        assert!(ids.contains("geo"));
        assert!(ids.contains("srs"));
    }

    #[test]
    fn update_swap_plan_uses_core_bin_directory() {
        let paths = AppPaths::new("/tmp/VoyaVPN", StorageMode::Portable);
        let plan = swap_plan_for_core(&paths, CoreType::Xray, PathBuf::from("/tmp/staged"));

        assert_eq!(plan.target_dir, Path::new("/tmp/VoyaVPN/bin/xray"));
        assert!(plan.backup_dir.starts_with("/tmp/VoyaVPN/guiTemps/updates"));
    }

    fn runtime_manager_for<'db>(
        database: &'db Database,
        paths: &AppPaths,
        runner: RecordingRunner,
    ) -> RuntimeManager<'db> {
        let supervisor = CoreSupervisor::spawn(SupervisorDeps::new(
            Arc::new(runner),
            Arc::new(SudoPasswordStore::new()),
        ));
        RuntimeManager::with_target_os(database, paths.clone(), supervisor, TargetOs::Linux)
    }

    fn write_fake_core_executable(
        paths: &AppPaths,
        core_type: CoreType,
        contents: &[u8],
    ) -> PathBuf {
        let core_info = get_core_info(core_type).expect("core info");
        let executable_name = executable_name_for_current_os(core_info.executable_names()[0]);
        let executable = paths.core_bin_file(core_type_dir_name(core_type), executable_name);
        fs::create_dir_all(executable.parent().expect("core dir")).expect("core dir");
        fs::write(&executable, contents).expect("fake core");
        executable
    }

    fn active_xray_profile(index_id: &str) -> ProfileItem {
        ProfileItem {
            index_id: index_id.to_string(),
            config_type: ConfigType::VLESS,
            core_type: Some(CoreType::Xray),
            remarks: "Runtime".to_string(),
            address: "example.test".to_string(),
            port: 443,
            password: "00000000-0000-0000-0000-000000000000".to_string(),
            network: "tcp".to_string(),
            ..ProfileItem::default()
        }
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
        env::temp_dir().join(format!(
            "voyavpn-update-{name}-{}-{}",
            std::process::id(),
            monotonic_millis()
        ))
    }

    fn update_archive_path(paths: &AppPaths, file_name: impl AsRef<Path>) -> PathBuf {
        let archive = paths.temp_file("updates").join(file_name);
        fs::create_dir_all(archive.parent().expect("archive parent")).expect("create updates dir");
        archive
    }

    fn apply_request(
        core_type: CoreType,
        archive: &Path,
        remote_version: &str,
    ) -> CoreUpdateApplyRequest {
        CoreUpdateApplyRequest {
            target_id: release_package_for_core(core_type)
                .expect("core package")
                .id
                .to_string(),
            file_name: archive.to_string_lossy().into_owned(),
            sha256: fixture_sha256(archive),
            remote_version: remote_version.to_string(),
        }
    }

    fn fixture_sha256(path: &Path) -> String {
        let bytes = fs::read(path).expect("read checksum fixture");
        sha256_hex(&Sha256::digest(bytes))
    }

    fn write_zip_archive(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("create zip");
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default();
        for (name, contents) in entries {
            zip.start_file(*name, options).expect("start zip file");
            zip.write_all(contents).expect("write zip file");
        }
        zip.finish().expect("finish zip");
    }

    fn write_tar_gz_archive(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("create tar.gz");
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = TarBuilder::new(encoder);
        for (name, contents) in entries {
            let mut header = Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, *name, *contents)
                .expect("append tar entry");
        }
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish tar.gz");
    }

    fn write_gz_file(path: &Path, contents: &[u8]) {
        let file = fs::File::create(path).expect("create gz");
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder.write_all(contents).expect("write gz");
        encoder.finish().expect("finish gz");
    }
}
