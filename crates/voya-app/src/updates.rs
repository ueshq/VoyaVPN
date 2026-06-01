use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

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
        check_package_from_releases, core_acquisition_policy, parse_github_releases, parse_version,
        release_package_for_core, supported_release_packages, updatable_core_types, AssetArch,
        AssetOs, BinaryAcquisition, GitHubReleaseClient, PackageTarget, ReleaseError,
        ReleaseFetchOptions, ReleasePackage,
    },
    DownloadClient, DownloadError, DownloadRequest,
};
use voya_platform::{
    coreinfo::{core_type_dir_name, discover_executable, get_core_info},
    paths::AppPaths,
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
    pub used_proxy: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRunResult {
    pub pre_release: bool,
    pub results: Vec<UpdateCheckResult>,
    pub targets: Vec<UpdateTarget>,
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
    releases: GitHubReleaseClient,
    downloads: DownloadClient,
    ruleset_geo: RulesetGeoClient,
}

impl<'db> UpdateManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database, paths: AppPaths) -> Self {
        Self {
            database,
            paths,
            releases: GitHubReleaseClient::new(),
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
        let mut results = Vec::new();

        for package in supported_release_packages() {
            if !selected.contains(package.id) {
                results.push(skipped(package.id, "not selected"));
                continue;
            }

            let current = self.current_package_version(&package)?;
            let fetch_options = ReleaseFetchOptions {
                include_prerelease: options.pre_release,
                prefer_proxy: options.prefer_proxy,
                proxy_url: options.proxy_url.clone(),
            };
            match self
                .releases
                .check_package(&package, current.as_ref(), os, arch, &fetch_options)
                .await
            {
                Ok(check) => results.push(check_result_to_update_result(&check)),
                Err(error) => results.push(UpdateCheckResult {
                    target_id: package.id.to_string(),
                    status: UpdateResultStatus::Error,
                    message: error.to_string(),
                    current_version: current.as_ref().map(ToString::to_string),
                    remote_version: None,
                    download_url: None,
                    file_name: None,
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
        let mut results = Vec::new();

        for package in supported_release_packages() {
            if !selected.contains(package.id) {
                results.push(skipped(package.id, "not selected"));
                continue;
            }

            let current = self.current_package_version(&package)?;
            let fetch_options = ReleaseFetchOptions {
                include_prerelease: options.pre_release,
                prefer_proxy: options.prefer_proxy,
                proxy_url: options.proxy_url.clone(),
            };
            match self
                .releases
                .check_package(&package, current.as_ref(), os, arch, &fetch_options)
                .await
            {
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
                        used_proxy: Some(response.used_proxy),
                    });
                }
                Ok(check) => results.push(check_result_to_update_result(&check)),
                Err(error) => results.push(UpdateCheckResult {
                    target_id: package.id.to_string(),
                    status: UpdateResultStatus::Error,
                    message: error.to_string(),
                    current_version: current.as_ref().map(ToString::to_string),
                    remote_version: None,
                    download_url: None,
                    file_name: None,
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

    fn current_package_version(&self, package: &ReleasePackage) -> Result<Option<Version>> {
        match package.target {
            PackageTarget::App => Ok(parse_version(env!("CARGO_PKG_VERSION"))),
            PackageTarget::Core(core_type) => self.installed_core_version(core_type),
        }
    }

    fn installed_core_version(&self, core_type: CoreType) -> Result<Option<Version>> {
        let Some(core_info) = get_core_info(core_type) else {
            return Ok(None);
        };
        let Some(version_arg) = core_info.version_arg else {
            return Ok(None);
        };
        let Ok(executable) = discover_executable(&self.paths, core_info) else {
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

#[must_use]
pub fn check_package_fixture(
    package: &ReleasePackage,
    release_json: &str,
    current_version: Option<&Version>,
    os: AssetOs,
    arch: AssetArch,
    include_prerelease: bool,
) -> Result<UpdateCheckResult> {
    let releases = parse_github_releases(release_json).map_err(ReleaseError::from)?;
    let check = check_package_from_releases(
        package,
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
            used_proxy: None,
        });
    }
}

fn check_result_to_update_result(check: &voya_net::update::ReleaseCheck) -> UpdateCheckResult {
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
        used_proxy: Some(asset.used_proxy),
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

fn skipped(target_id: &str, message: &str) -> UpdateCheckResult {
    UpdateCheckResult {
        target_id: target_id.to_string(),
        status: UpdateResultStatus::Skipped,
        message: message.to_string(),
        current_version: None,
        remote_version: None,
        download_url: None,
        file_name: None,
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
    use std::{env, path::Path};

    use voya_core::{RoutingItem, RuleType, RulesItem};
    use voya_platform::paths::StorageMode;

    use super::*;

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
        let result = check_package_fixture(
            &package,
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
        let database = Database::connect_in_memory().await.unwrap();
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

    fn unique_temp_root(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "voyavpn-update-{name}-{}-{}",
            std::process::id(),
            monotonic_millis()
        ))
    }
}
