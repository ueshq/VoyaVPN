use std::{
    collections::BTreeSet,
    fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;
use voya_app::autostart::{AutostartManager, AutostartManagerError, AutostartStatus};
use voya_app::backup::{
    BackupManager, BackupManagerError, BackupOperationResult, BackupRemoteResult,
    BackupRestoreResult, BackupStatus,
};
use voya_app::certificates::{
    calculate_certificate_sha256 as calculate_certificate_sha256_impl,
    fetch_certificate as fetch_certificate_impl, CertificateError, CertificateFetchRequest,
    CertificateFetchResult,
};
use voya_app::clash::{
    ClashConnectionsSnapshot, ClashDelayTestResult, ClashManager, ClashManagerError,
    ClashMonitorStatus, ClashProxiesSnapshot,
};
use voya_app::diagnostics::{
    diagnostics_settings, prepare_diagnostics_settings, DiagnosticsClient, DiagnosticsErrorClass,
    DiagnosticsEvent, DiagnosticsRecordStatus, DiagnosticsReleaseChannel, DiagnosticsResult,
    DiagnosticsSettings,
};
use voya_app::dns::{DnsManager, DnsManagerError, DnsSettings, DnsValidationIssue};
use voya_app::elevation::ElevationError;
use voya_app::exports::{
    ExportManager, ExportManagerError, ExportProfilesFormat, ExportProfilesRequest,
    ExportProfilesResult,
};
use voya_app::groups::{GroupManager, GroupManagerError};
use voya_app::hotkeys::{
    GlobalHotkeyBinding, HotkeyManager, HotkeyManagerError, HotkeyRegistrar, HotkeyStatus,
};
use voya_app::input_safety::{self, InputSafetyError};
use voya_app::presets::{PresetApplyOptions, PresetApplyResult, PresetManager, PresetManagerError};
use voya_app::profiles::{ProfileManager, ProfileManagerError};
use voya_app::qr::{QrCodeError, QrCodeImage, QrCodeManager, QrScanResult};
use voya_app::routing::{RoutingManager, RoutingManagerError};
use voya_app::runtime::{RuntimeError, RuntimeManager};
use voya_app::speedtest::{
    SpeedTestResult, SpeedtestError, SpeedtestManager, SpeedtestRunResult, SpeedtestStatus,
};
use voya_app::subscriptions::{SubscriptionManager, SubscriptionManagerError};
use voya_app::supervisor::{SupervisorConnectionState, SupervisorError, SupervisorSnapshot};
use voya_app::sysproxy::SystemProxyManagerError;
use voya_app::templates::{FullConfigTemplateManager, FullConfigTemplateManagerError};
use voya_app::tun::{TunManager, TunManagerError, TunStatus};
use voya_app::updates::{
    ManualAppUpdateLinks, RulesetGeoSourceSettings, UpdateManager, UpdateManagerError,
    UpdateRequestOptions, UpdateResultStatus, UpdateRunResult, UpdateStatus,
};
use voya_core::{
    AppConfig, CoreType, FullConfigTemplateItem, GlobalHotkey, GroupChildCandidate, GroupPreview,
    GroupValidationResult, ImportProfilesResult, KeyEventItem, MoveAction, PresetType,
    ProfileDedupeResult, ProfileItem, ProfileListItem, ProfileSortKey, RoutingItem, RuleMode,
    RulesItem, SubItem, SubscriptionUpdateResult, SysProxyType, WebDavItem,
};
use voya_platform::{
    coreinfo::{
        copy_seed_core_asset, CoreInfoError, CoreSeedCopyOutcome, CoreSeedCopyStatus, TargetOs,
    },
    sysproxy::SystemProxyStatus,
};

use super::events::{
    next_log_line_id, CoreState, CoreStateEvent, InvalidateEvent, LogLevel, LogLineEvent,
    QueryInvalidation, TransientStreamEvent,
};
#[cfg(debug_assertions)]
use super::events::{AppEvent, AppNotice, AppNoticeLevel, DemoRequest, DemoResponse};
use crate::AppState;

const PROFILE_IMPORT_DIR_NAME: &str = "imports";
const IPC_ID_MAX_CHARS: usize = 128;
const IPC_NAME_MAX_CHARS: usize = 256;
const IPC_FILTER_MAX_CHARS: usize = 256;
const IPC_PATH_MAX_CHARS: usize = 4096;
const IPC_PROXY_URL_MAX_CHARS: usize = 2048;
const IPC_QR_CONTENT_MAX_CHARS: usize = 4096;
const IPC_LIST_MAX_ITEMS: usize = 1024;
const MISSING_CORE_SEARCH_DIR_LABEL: &str = "application core directory";

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum AppError {
    EventEmit(String),
    Autostart(String),
    ConfigLoad(String),
    ConfigSave(String),
    Backup(String),
    Certificate(String),
    Clash(String),
    Database(String),
    Dns(DnsCommandError),
    Group(String),
    Hotkey(String),
    Preset(String),
    Profile(String),
    Qr(String),
    Export(String),
    MissingCore(MissingCoreError),
    Runtime(String),
    Routing(String),
    Speedtest(String),
    Sudo(String),
    Subscription(String),
    SysProxy(String),
    State(String),
    Template(String),
    Tun(String),
    Update(String),
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DnsCommandError {
    pub message: String,
    pub issues: Vec<DnsValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MissingCoreError {
    pub message: String,
    pub core_type: CoreType,
    pub search_dir: String,
    pub candidates: Vec<String>,
    pub download_url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CoreSeedInstallStatus {
    Installed,
    AlreadyInstalled,
    SeedMissing,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CoreSeedInstallResult {
    pub core_type: CoreType,
    pub status: CoreSeedInstallStatus,
    pub installed_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeConnectionState {
    Disconnected,
    Connected,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusResponse {
    pub state: RuntimeConnectionState,
    pub active_profile_id: Option<String>,
    pub main_pid: Option<u32>,
    pub pre_pid: Option<u32>,
    pub running_core_type: Option<CoreType>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppUpdaterState {
    Ready,
    Unconfigured,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdaterStatus {
    pub current_version: String,
    pub state: AppUpdaterState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppUpdateDiagnosticAction {
    Check,
    Install,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppUpdateDiagnosticResult {
    Success,
    Failure,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsStatus {
    pub enabled: bool,
    pub delivery_configured: bool,
    pub queued_events: u32,
    pub queued_bytes: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxyStatusResponse {
    pub requested_mode: SysProxyType,
    pub effective_mode: SysProxyType,
    pub pac_available: bool,
    pub proxy: Option<String>,
    pub exceptions: String,
    pub pac_url: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn app_health() -> Result<String, AppError> {
    Ok("ok".to_string())
}

#[tauri::command]
#[specta::specta]
pub fn load_app_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, AppError> {
    let config = state
        .config_store()
        .load()
        .map_err(|error| AppError::ConfigLoad(error.to_string()))?;
    let mut guard = state
        .config()
        .write()
        .map_err(|_| AppError::State("app config lock is poisoned".to_string()))?;

    *guard = config.clone();

    Ok(config)
}

#[tauri::command]
#[specta::specta]
pub fn save_app_config(
    state: tauri::State<'_, AppState>,
    config: AppConfig,
) -> Result<AppConfig, AppError> {
    state
        .config_store()
        .save(&config)
        .map_err(|error| AppError::ConfigSave(error.to_string()))?;
    let mut guard = state
        .config()
        .write()
        .map_err(|_| AppError::State("app config lock is poisoned".to_string()))?;

    *guard = config.clone();

    Ok(config)
}

#[tauri::command]
#[specta::specta]
pub async fn diagnostics_status(
    state: tauri::State<'_, AppState>,
) -> Result<DiagnosticsStatus, AppError> {
    let settings = current_diagnostics_settings(&state)?;
    let client = state.diagnostics_client();
    let client = client.lock().await;

    Ok(diagnostics_status_response(&settings, &client))
}

#[tauri::command]
#[specta::specta]
pub async fn set_diagnostics_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<DiagnosticsStatus, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    config.diagnostics_item.enabled = enabled;
    let settings = diagnostics_settings_for_config(&mut config);
    persist_config_if_changed(&state, &original, &config)?;

    let client = state.diagnostics_client();
    let mut client = client.lock().await;
    if !enabled {
        client.clear();
    }

    Ok(diagnostics_status_response(&settings, &client))
}

#[tauri::command]
#[specta::specta]
pub fn autostart_status(state: tauri::State<'_, AppState>) -> Result<AutostartStatus, AppError> {
    let config = current_config(&state)?;

    AutostartManager::new()
        .status(&config)
        .map_err(autostart_error)
}

#[tauri::command]
#[specta::specta]
pub fn set_autostart_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<AutostartStatus, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let status = AutostartManager::new()
        .set_enabled(&mut config, enabled)
        .map_err(autostart_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_app_config_invalidation(&app, "autostart-updated")?;

    Ok(status)
}

#[tauri::command]
#[specta::specta]
pub fn global_hotkey_status(state: tauri::State<'_, AppState>) -> Result<HotkeyStatus, AppError> {
    let config = current_config(&state)?;

    HotkeyManager::new(std::sync::Arc::new(voya_app::hotkeys::NoopHotkeyRegistrar))
        .status(&config)
        .map_err(hotkey_error)
}

#[tauri::command]
#[specta::specta]
pub fn save_global_hotkeys<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: Vec<KeyEventItem>,
) -> Result<HotkeyStatus, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let registrar = std::sync::Arc::new(TauriHotkeyRegistrar { app: app.clone() });
    let status = HotkeyManager::new(registrar)
        .save_settings(&mut config, settings)
        .map_err(hotkey_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_app_config_invalidation(&app, "global-hotkeys-updated")?;

    Ok(status)
}

#[tauri::command]
#[specta::specta]
pub fn generate_qr_code(content: String) -> Result<QrCodeImage, AppError> {
    validate_ipc_text(
        &content,
        "QR content",
        IPC_QR_CONTENT_MAX_CHARS,
        AppError::Qr,
    )?;

    QrCodeManager.generate_svg(&content).map_err(qr_error)
}

#[tauri::command]
#[specta::specta]
pub fn scan_screen_qr() -> Result<QrScanResult, AppError> {
    Ok(QrCodeManager.scan_screen())
}

#[tauri::command]
#[specta::specta]
pub async fn fetch_certificate(
    request: CertificateFetchRequest,
) -> Result<CertificateFetchResult, AppError> {
    validate_required_ipc_text(
        &request.address,
        "certificate address",
        IPC_NAME_MAX_CHARS,
        AppError::Certificate,
    )?;
    if let Some(server_name) = request.server_name.as_deref() {
        validate_ipc_text(
            server_name,
            "certificate server name",
            IPC_NAME_MAX_CHARS,
            AppError::Certificate,
        )?;
    }

    fetch_certificate_impl(request)
        .await
        .map_err(certificate_error)
}

#[tauri::command]
#[specta::specta]
pub fn calculate_certificate_sha256(pem: String) -> Result<Vec<String>, AppError> {
    validate_required_ipc_text(
        &pem,
        "certificate PEM",
        IPC_QR_CONTENT_MAX_CHARS * 8,
        AppError::Certificate,
    )?;

    calculate_certificate_sha256_impl(&pem).map_err(certificate_error)
}

pub(crate) fn register_global_hotkeys_for_config<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: &AppConfig,
) -> Result<HotkeyStatus, AppError> {
    let registrar = std::sync::Arc::new(TauriHotkeyRegistrar { app: app.clone() });

    HotkeyManager::new(registrar)
        .register_from_config(config)
        .map_err(hotkey_error)
}

/// Trigger the one-time native authorization dialog and, on success, install
/// the passwordless elevation launcher. No admin password is stored.
#[tauri::command]
#[specta::specta]
pub fn tun_request_elevation(state: tauri::State<'_, AppState>) -> Result<TunStatus, AppError> {
    state
        .elevation_manager()
        .request()
        .map_err(elevation_error)?;
    let config = current_config(&state)?;
    tun_manager(&state).status(&config).map_err(tun_error)
}

/// Remove the passwordless elevation launcher + sudoers drop-in and clear the
/// session grant.
#[tauri::command]
#[specta::specta]
pub fn tun_revoke_elevation(state: tauri::State<'_, AppState>) -> Result<TunStatus, AppError> {
    state.elevation_manager().revoke();
    let config = current_config(&state)?;
    tun_manager(&state).status(&config).map_err(tun_error)
}

#[tauri::command]
#[specta::specta]
pub async fn connect_active_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = current_config(&state)?;
    emit_runtime_log(&app, LogLevel::Info, "Connecting active profile")?;
    emit_core_state(
        &app,
        CoreState::Connecting,
        Some(config.index_id.clone()).filter(|value| !value.is_empty()),
        None,
    )?;

    match runtime_manager(&state).connect(&config).await {
        Ok(snapshot) => {
            emit_runtime_log(&app, LogLevel::Info, "Core supervisor started")?;
            emit_core_state(&app, CoreState::Connected, None, Some(&snapshot))?;
            match apply_system_proxy(&app, &state, &config, false) {
                Ok(status) => emit_sysproxy_changed(&app, &status)?,
                Err(error) => emit_runtime_log(
                    &app,
                    LogLevel::Warn,
                    &format!("System proxy apply failed: {error}"),
                )?,
            }
            Ok(runtime_status_response(snapshot))
        }
        Err(error) => {
            let message = error.to_string();
            emit_runtime_log(&app, LogLevel::Error, &message)?;
            emit_core_state(&app, CoreState::Disconnected, None, None)?;
            record_runtime_start_failure_diagnostics(&state, &config, &error);
            Err(runtime_error(error))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn disconnect_core<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    emit_runtime_log(&app, LogLevel::Info, "Disconnecting core supervisor")?;
    emit_core_state(&app, CoreState::Disconnecting, None, None)?;

    match runtime_manager(&state).disconnect().await {
        Ok(snapshot) => {
            match restore_system_proxy(&app, &state) {
                Ok(status) => emit_sysproxy_changed(&app, &status)?,
                Err(error) => emit_runtime_log(
                    &app,
                    LogLevel::Warn,
                    &format!("System proxy restore failed: {error:?}"),
                )?,
            }
            emit_runtime_log(&app, LogLevel::Info, "Core supervisor stopped")?;
            emit_core_state(&app, CoreState::Disconnected, None, Some(&snapshot))?;
            emit_statistics_zero(&app)?;
            Ok(runtime_status_response(snapshot))
        }
        Err(error) => {
            let message = error.to_string();
            emit_runtime_log(&app, LogLevel::Error, &message)?;
            Err(runtime_error(error))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn restart_core<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = current_config(&state)?;
    emit_runtime_log(&app, LogLevel::Info, "Restarting active profile")?;
    emit_core_state(
        &app,
        CoreState::Connecting,
        Some(config.index_id.clone()).filter(|value| !value.is_empty()),
        None,
    )?;

    match runtime_manager(&state).restart(&config).await {
        Ok(snapshot) => {
            emit_runtime_log(&app, LogLevel::Info, "Core supervisor restarted")?;
            emit_core_state(&app, CoreState::Connected, None, Some(&snapshot))?;
            match apply_system_proxy(&app, &state, &config, false) {
                Ok(status) => emit_sysproxy_changed(&app, &status)?,
                Err(error) => emit_runtime_log(
                    &app,
                    LogLevel::Warn,
                    &format!("System proxy apply failed: {error}"),
                )?,
            }
            Ok(runtime_status_response(snapshot))
        }
        Err(error) => {
            let message = error.to_string();
            emit_runtime_log(&app, LogLevel::Error, &message)?;
            emit_core_state(&app, CoreState::Disconnected, None, None)?;
            record_runtime_start_failure_diagnostics(&state, &config, &error);
            Err(runtime_error(error))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_status(
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    runtime_manager(&state)
        .status()
        .await
        .map(runtime_status_response)
        .map_err(runtime_error)
}

#[tauri::command]
#[specta::specta]
pub fn system_proxy_status(
    state: tauri::State<'_, AppState>,
) -> Result<SystemProxyStatusResponse, AppError> {
    let config = current_config(&state)?;

    state
        .system_proxy_manager()
        .status(&config)
        .map(system_proxy_status_response)
        .map_err(sysproxy_error)
}

#[tauri::command]
#[specta::specta]
pub fn set_system_proxy_mode<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    mode: SysProxyType,
) -> Result<SystemProxyStatusResponse, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let status = state
        .system_proxy_manager()
        .set_mode(&mut config, mode)
        .map_err(sysproxy_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_sysproxy_changed(&app, &status)?;
    crate::refresh_tray_menu(&app).map_err(|error| AppError::State(error.to_string()))?;

    Ok(system_proxy_status_response(status))
}

#[tauri::command]
#[specta::specta]
pub fn tun_status(state: tauri::State<'_, AppState>) -> Result<TunStatus, AppError> {
    let config = current_config(&state)?;

    tun_manager(&state).status(&config).map_err(tun_error)
}

#[tauri::command]
#[specta::specta]
pub async fn set_tun_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<TunStatus, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let status = tun_manager(&state)
        .set_enabled(&mut config, enabled)
        .map_err(tun_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_tun_changed(&app, status.enabled)?;
    restart_if_connected_after_config_change(&app, &state, &config, "TUN changed").await?;

    Ok(status)
}

#[tauri::command]
#[specta::specta]
pub async fn load_dns_settings(state: tauri::State<'_, AppState>) -> Result<DnsSettings, AppError> {
    let config = current_config(&state)?;

    DnsManager::new(state.database())
        .load_settings(&config.simple_dns_item)
        .await
        .map_err(dns_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_dns_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: DnsSettings,
) -> Result<DnsSettings, AppError> {
    let original = current_config(&state)?;
    let saved = DnsManager::new(state.database())
        .save_settings(settings)
        .await
        .map_err(dns_error)?;
    let mut config = original.clone();
    config.simple_dns_item = saved.simple_dns_item.clone();

    persist_config_if_changed(&state, &original, &config)?;
    emit_dns_invalidation(&app, "dns-settings-saved")?;
    restart_if_connected_after_config_change(&app, &state, &config, "DNS changed").await?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn load_full_config_templates(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FullConfigTemplateItem>, AppError> {
    FullConfigTemplateManager::new(state.database())
        .load_templates()
        .await
        .map_err(template_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_full_config_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    template: FullConfigTemplateItem,
) -> Result<FullConfigTemplateItem, AppError> {
    let saved = FullConfigTemplateManager::new(state.database())
        .save_template(template)
        .await
        .map_err(template_error)?;

    emit_full_config_template_invalidation(&app, "full-config-template-saved")?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn list_profiles(
    state: tauri::State<'_, AppState>,
    subid: Option<String>,
    filter: Option<String>,
) -> Result<Vec<ProfileListItem>, AppError> {
    validate_present_ipc_text(
        subid.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    validate_optional_ipc_text(
        filter.as_deref(),
        "profile filter",
        IPC_FILTER_MAX_CHARS,
        AppError::Profile,
    )?;
    let config = current_config(&state)?;

    ProfileManager::new(state.database())
        .list_profiles(&config, subid.as_deref(), filter.as_deref())
        .await
        .map_err(profile_error)
}

#[tauri::command]
#[specta::specta]
pub async fn get_profile(
    state: tauri::State<'_, AppState>,
    index_id: String,
) -> Result<Option<ProfileListItem>, AppError> {
    validate_required_ipc_text(
        &index_id,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let config = current_config(&state)?;

    ProfileManager::new(state.database())
        .get_profile(&config, &index_id)
        .await
        .map_err(profile_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile: ProfileItem,
) -> Result<ProfileListItem, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let result = ProfileManager::new(state.database())
        .save_profile(&mut config, profile)
        .await
        .map_err(profile_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_profile_invalidation(
        &app,
        "profile-saved",
        [result.profile.index_id.clone()],
        original.index_id != config.index_id,
    )?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<u32, AppError> {
    validate_ipc_text_list(
        &index_ids,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let deleted = ProfileManager::new(state.database())
        .delete_profiles(&mut config, &index_ids)
        .await
        .map_err(profile_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_profile_invalidation(
        &app,
        "profiles-deleted",
        index_ids,
        original.index_id != config.index_id,
    )?;

    Ok(u32::try_from(deleted).unwrap_or(u32::MAX))
}

#[tauri::command]
#[specta::specta]
pub async fn copy_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<Vec<ProfileListItem>, AppError> {
    validate_ipc_text_list(
        &index_ids,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let copied = ProfileManager::new(state.database())
        .copy_profiles(&mut config, &index_ids)
        .await
        .map_err(profile_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_profile_invalidation(
        &app,
        "profiles-copied",
        copied
            .iter()
            .map(|item| item.profile.index_id.clone())
            .collect::<Vec<_>>(),
        original.index_id != config.index_id,
    )?;

    Ok(copied)
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_share_links(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    export_profiles_result(&state, index_ids, ExportProfilesFormat::ShareLinks).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_share_links_base64(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    export_profiles_result(&state, index_ids, ExportProfilesFormat::ShareLinksBase64).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_inner_links(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    export_profiles_result(&state, index_ids, ExportProfilesFormat::InnerLinks).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_client_config(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    export_profiles_result(&state, index_ids, ExportProfilesFormat::ClientConfig).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_id: String,
) -> Result<ProfileListItem, AppError> {
    validate_required_ipc_text(
        &index_id,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let active = ProfileManager::new(state.database())
        .set_active_profile(&mut config, &index_id)
        .await
        .map_err(profile_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_profile_invalidation(&app, "active-profile-changed", [index_id], true)?;

    Ok(active)
}

#[tauri::command]
#[specta::specta]
pub async fn move_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    subid: Option<String>,
    index_id: String,
    action: MoveAction,
    position: Option<i32>,
) -> Result<Vec<ProfileListItem>, AppError> {
    validate_present_ipc_text(
        subid.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    validate_required_ipc_text(
        &index_id,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let config = current_config(&state)?;
    let profiles = ProfileManager::new(state.database())
        .move_profile(&config, subid.as_deref(), &index_id, action, position)
        .await
        .map_err(profile_error)?;

    emit_profile_invalidation(&app, "profile-moved", [index_id], false)?;

    Ok(profiles)
}

#[tauri::command]
#[specta::specta]
pub async fn sort_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    subid: Option<String>,
    sort_key: ProfileSortKey,
    ascending: bool,
) -> Result<Vec<ProfileListItem>, AppError> {
    validate_present_ipc_text(
        subid.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let config = current_config(&state)?;
    let profiles = ProfileManager::new(state.database())
        .sort_profiles(&config, subid.as_deref(), sort_key, ascending)
        .await
        .map_err(profile_error)?;

    emit_profile_invalidation(
        &app,
        "profiles-sorted",
        profiles
            .iter()
            .map(|item| item.profile.index_id.clone())
            .collect::<Vec<_>>(),
        false,
    )?;

    Ok(profiles)
}

#[tauri::command]
#[specta::specta]
pub async fn move_profiles_to_group<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
    subid: String,
) -> Result<u32, AppError> {
    validate_ipc_text_list(
        &index_ids,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    validate_required_ipc_text(
        &subid,
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let updated = ProfileManager::new(state.database())
        .move_profiles_to_group(&index_ids, &subid)
        .await
        .map_err(profile_error)?;

    emit_profile_invalidation(&app, "profiles-moved-to-group", index_ids, false)?;

    Ok(u32::try_from(updated).unwrap_or(u32::MAX))
}

#[tauri::command]
#[specta::specta]
pub async fn dedupe_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    subid: Option<String>,
    keep_older: Option<bool>,
) -> Result<ProfileDedupeResult, AppError> {
    validate_present_ipc_text(
        subid.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let result = ProfileManager::new(state.database())
        .dedupe_profiles(&mut config, subid.as_deref(), keep_older.unwrap_or(false))
        .await
        .map_err(profile_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_profile_invalidation(
        &app,
        "profiles-deduped",
        result.removed_index_ids.clone(),
        original.index_id != config.index_id,
    )?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn list_group_child_candidates(
    state: tauri::State<'_, AppState>,
    current_index_id: Option<String>,
    filter: Option<String>,
) -> Result<Vec<GroupChildCandidate>, AppError> {
    validate_present_ipc_text(
        current_index_id.as_deref(),
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Group,
    )?;
    validate_optional_ipc_text(
        filter.as_deref(),
        "group candidate filter",
        IPC_FILTER_MAX_CHARS,
        AppError::Group,
    )?;
    GroupManager::new(state.database())
        .list_child_candidates(current_index_id.as_deref(), filter.as_deref())
        .await
        .map_err(group_error)
}

#[tauri::command]
#[specta::specta]
pub async fn validate_group_profile(
    state: tauri::State<'_, AppState>,
    profile: ProfileItem,
) -> Result<GroupValidationResult, AppError> {
    GroupManager::new(state.database())
        .validate_group_profile(&profile)
        .await
        .map_err(group_error)
}

#[tauri::command]
#[specta::specta]
pub async fn preview_group_profile(
    state: tauri::State<'_, AppState>,
    profile: ProfileItem,
) -> Result<GroupPreview, AppError> {
    let config = current_config(&state)?;

    GroupManager::new(state.database())
        .preview_group_profile(&config, &profile)
        .await
        .map_err(group_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_group_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile: ProfileItem,
) -> Result<ProfileListItem, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let result = GroupManager::new(state.database())
        .save_group_profile(&mut config, profile)
        .await
        .map_err(group_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_profile_invalidation(
        &app,
        "group-profile-saved",
        [result.profile.index_id.clone()],
        original.index_id != config.index_id,
    )?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn list_subscriptions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SubItem>, AppError> {
    SubscriptionManager::new(state.database())
        .list_subscriptions()
        .await
        .map_err(subscription_error)
}

#[tauri::command]
#[specta::specta]
pub async fn get_subscription(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Option<SubItem>, AppError> {
    validate_required_ipc_text(
        &id,
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Subscription,
    )?;
    SubscriptionManager::new(state.database())
        .get_subscription(&id)
        .await
        .map_err(subscription_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_subscription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    item: SubItem,
) -> Result<SubItem, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let saved = SubscriptionManager::new(state.database())
        .save_subscription(&mut config, item)
        .await
        .map_err(subscription_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_subscription_invalidation(&app, "subscription-saved", false, original != config)?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_subscriptions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    validate_ipc_text_list(
        &ids,
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Subscription,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let deleted = SubscriptionManager::new(state.database())
        .delete_subscriptions(&mut config, &ids)
        .await
        .map_err(subscription_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_subscription_invalidation(&app, "subscriptions-deleted", true, original != config)?;

    Ok(deleted)
}

#[tauri::command]
#[specta::specta]
pub async fn import_profiles_from_text<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    text: String,
    subid: Option<String>,
    is_sub: bool,
) -> Result<ImportProfilesResult, AppError> {
    validate_present_ipc_text(
        subid.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Subscription,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let result = SubscriptionManager::new(state.database())
        .import_profiles_from_text(&mut config, &text, subid.as_deref(), is_sub)
        .await
        .map_err(subscription_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_subscription_invalidation(&app, "profiles-imported", true, original != config)?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn import_profiles_from_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    path: String,
    subid: Option<String>,
    is_sub: bool,
) -> Result<ImportProfilesResult, AppError> {
    validate_present_ipc_text(
        subid.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Subscription,
    )?;
    let path = resolve_scoped_ipc_file(
        &path,
        &state.runtime_paths().temp_file(PROFILE_IMPORT_DIR_NAME),
        IpcFileScope::ProfileImport,
    )?;
    let text = fs::read_to_string(&path)
        .map_err(|error| AppError::Subscription(format!("failed to read import file: {error}")))?;

    import_profiles_from_text(app, state, text, subid, is_sub).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_subscriptions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    subid: Option<String>,
    prefer_proxy: bool,
    proxy_url: Option<String>,
) -> Result<SubscriptionUpdateResult, AppError> {
    validate_present_ipc_text(
        subid.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Subscription,
    )?;
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Subscription,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let result = SubscriptionManager::new(state.database())
        .update_subscriptions(
            &mut config,
            subid.as_deref(),
            prefer_proxy,
            proxy_url.as_deref(),
            current_unix_time(),
        )
        .await
        .map_err(subscription_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_subscription_invalidation(&app, "subscriptions-updated", true, original != config)?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn run_due_subscription_updates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    prefer_proxy: bool,
    proxy_url: Option<String>,
) -> Result<SubscriptionUpdateResult, AppError> {
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Subscription,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let result = SubscriptionManager::new(state.database())
        .run_due_updates(
            &mut config,
            current_unix_time(),
            prefer_proxy,
            proxy_url.as_deref(),
        )
        .await
        .map_err(subscription_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_subscription_invalidation(&app, "due-subscriptions-updated", true, original != config)?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn list_routings(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RoutingItem>, AppError> {
    RoutingManager::new(state.database())
        .list_routings()
        .await
        .map_err(routing_error)
}

#[tauri::command]
#[specta::specta]
pub async fn get_routing(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Option<RoutingItem>, AppError> {
    validate_required_ipc_text(&id, "routing id", IPC_ID_MAX_CHARS, AppError::Routing)?;
    RoutingManager::new(state.database())
        .get_routing(&id)
        .await
        .map_err(routing_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_routing<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    item: RoutingItem,
) -> Result<RoutingItem, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let saved = RoutingManager::new(state.database())
        .save_routing(&mut config, item)
        .await
        .map_err(routing_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_routing_invalidation(
        &app,
        "routing-saved",
        [saved.id.clone()],
        original != config,
    )?;
    restart_if_connected_after_routing_change(&app, &state, &config).await?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_routings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    validate_ipc_text_list(&ids, "routing id", IPC_ID_MAX_CHARS, AppError::Routing)?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let deleted = RoutingManager::new(state.database())
        .delete_routings(&mut config, &ids)
        .await
        .map_err(routing_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_routing_invalidation(&app, "routings-deleted", ids, original != config)?;
    restart_if_connected_after_routing_change(&app, &state, &config).await?;

    Ok(deleted)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_routing<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<RoutingItem, AppError> {
    validate_required_ipc_text(&id, "routing id", IPC_ID_MAX_CHARS, AppError::Routing)?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let active = RoutingManager::new(state.database())
        .set_active_routing(&mut config, &id)
        .await
        .map_err(routing_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_routing_invalidation(&app, "active-routing-changed", [id], true)?;
    restart_if_connected_after_routing_change(&app, &state, &config).await?;

    Ok(active)
}

#[tauri::command]
#[specta::specta]
pub async fn save_routing_rule<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
    rule: RulesItem,
) -> Result<RoutingItem, AppError> {
    validate_required_ipc_text(
        &routing_id,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    let config = current_config(&state)?;
    let saved = RoutingManager::new(state.database())
        .save_rule(&routing_id, rule)
        .await
        .map_err(routing_error)?;

    emit_routing_invalidation(&app, "routing-rule-saved", [routing_id], false)?;
    restart_if_connected_after_routing_change(&app, &state, &config).await?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_routing_rules<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
    rule_ids: Vec<String>,
) -> Result<RoutingItem, AppError> {
    validate_required_ipc_text(
        &routing_id,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    validate_ipc_text_list(
        &rule_ids,
        "routing rule id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    let config = current_config(&state)?;
    let saved = RoutingManager::new(state.database())
        .delete_rules(&routing_id, &rule_ids)
        .await
        .map_err(routing_error)?;

    emit_routing_invalidation(&app, "routing-rules-deleted", [routing_id], false)?;
    restart_if_connected_after_routing_change(&app, &state, &config).await?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn move_routing_rule<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
    rule_id: String,
    action: MoveAction,
    position: Option<i32>,
) -> Result<RoutingItem, AppError> {
    validate_required_ipc_text(
        &routing_id,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    validate_required_ipc_text(
        &rule_id,
        "routing rule id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    let config = current_config(&state)?;
    let saved = RoutingManager::new(state.database())
        .move_rule(&routing_id, &rule_id, action, position)
        .await
        .map_err(routing_error)?;

    emit_routing_invalidation(&app, "routing-rule-moved", [routing_id], false)?;
    restart_if_connected_after_routing_change(&app, &state, &config).await?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn import_routing_templates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    prefer_proxy: bool,
    proxy_url: Option<String>,
    import_advanced_rules: bool,
) -> Result<Vec<RoutingItem>, AppError> {
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Routing,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let imported = RoutingManager::new(state.database())
        .import_routing_templates(
            &mut config,
            prefer_proxy,
            proxy_url.as_deref(),
            import_advanced_rules,
        )
        .await
        .map_err(routing_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_routing_invalidation(
        &app,
        "routing-templates-imported",
        imported
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        original != config,
    )?;
    restart_if_connected_after_routing_change(&app, &state, &config).await?;

    Ok(imported)
}

#[tauri::command]
#[specta::specta]
pub async fn apply_regional_preset<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    preset_type: PresetType,
    prefer_proxy: bool,
    proxy_url: Option<String>,
) -> Result<PresetApplyResult, AppError> {
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Preset,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let result = PresetManager::new(state.database())
        .apply(
            &mut config,
            preset_type,
            PresetApplyOptions {
                prefer_proxy,
                proxy_url,
            },
        )
        .await
        .map_err(preset_error)?;

    persist_config_if_changed(&state, &original, &config)?;
    emit_preset_invalidation(&app, "regional-preset-applied")?;
    restart_if_connected_after_config_change(&app, &state, &config, "Regional preset changed")
        .await?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn clash_list_proxies(
    state: tauri::State<'_, AppState>,
) -> Result<ClashProxiesSnapshot, AppError> {
    let config = current_config(&state)?;

    ClashManager::new()
        .proxies(&config)
        .await
        .map_err(clash_error)
}

#[tauri::command]
#[specta::specta]
pub async fn clash_test_delay(
    state: tauri::State<'_, AppState>,
    proxy_names: Vec<String>,
) -> Result<Vec<ClashDelayTestResult>, AppError> {
    validate_ipc_text_list(
        &proxy_names,
        "Clash proxy name",
        IPC_NAME_MAX_CHARS,
        AppError::Clash,
    )?;
    let config = current_config(&state)?;

    ClashManager::new()
        .test_delay(&config, proxy_names)
        .await
        .map_err(clash_error)
}

#[tauri::command]
#[specta::specta]
pub async fn clash_select_proxy<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    group_name: String,
    proxy_name: String,
) -> Result<ClashProxiesSnapshot, AppError> {
    validate_required_ipc_text(
        &group_name,
        "Clash group name",
        IPC_NAME_MAX_CHARS,
        AppError::Clash,
    )?;
    validate_required_ipc_text(
        &proxy_name,
        "Clash proxy name",
        IPC_NAME_MAX_CHARS,
        AppError::Clash,
    )?;
    let config = current_config(&state)?;
    let snapshot = ClashManager::new()
        .select_proxy(&config, &group_name, &proxy_name)
        .await
        .map_err(clash_error)?;

    emit_clash_invalidation(&app, "clash-proxy-selected")?;

    Ok(snapshot)
}

#[tauri::command]
#[specta::specta]
pub async fn clash_list_connections(
    state: tauri::State<'_, AppState>,
) -> Result<ClashConnectionsSnapshot, AppError> {
    let config = current_config(&state)?;

    ClashManager::new()
        .connections(&config)
        .await
        .map_err(clash_error)
}

#[tauri::command]
#[specta::specta]
pub async fn clash_close_connection<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    connection_id: Option<String>,
) -> Result<ClashConnectionsSnapshot, AppError> {
    validate_present_ipc_text(
        connection_id.as_deref(),
        "Clash connection id",
        IPC_ID_MAX_CHARS,
        AppError::Clash,
    )?;
    let config = current_config(&state)?;
    let snapshot = ClashManager::new()
        .close_connection(&config, connection_id.as_deref())
        .await
        .map_err(clash_error)?;

    emit_clash_invalidation(&app, "clash-connection-closed")?;

    Ok(snapshot)
}

#[tauri::command]
#[specta::specta]
pub async fn clash_set_rule_mode<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    mode: RuleMode,
) -> Result<AppConfig, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    if config.clash_ui_item.rule_mode != mode {
        if mode != RuleMode::Unchanged {
            ClashManager::new()
                .set_rule_mode(&config, mode)
                .await
                .map_err(clash_error)?;
        }
        config.clash_ui_item.rule_mode = mode;
        persist_config_if_changed(&state, &original, &config)?;
    }

    emit_clash_invalidation(&app, "clash-rule-mode-changed")?;

    Ok(config)
}

#[tauri::command]
#[specta::specta]
pub async fn clash_reload_config<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), AppError> {
    validate_optional_ipc_text(
        path.as_deref(),
        "Clash config path",
        IPC_PATH_MAX_CHARS,
        AppError::Clash,
    )?;
    let config = current_config(&state)?;

    ClashManager::new()
        .reload_config(&config, path.as_deref())
        .await
        .map_err(clash_error)?;
    emit_clash_invalidation(&app, "clash-config-reloaded")?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn clash_start_monitor(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ClashMonitorStatus, AppError> {
    let config = match current_config(&state) {
        Ok(config) => config,
        Err(error) => {
            emit_clash_monitor_status(
                &app,
                &ClashMonitorStatus::failed("Clash monitor failed to read current config"),
            );
            return Err(error);
        }
    };

    match state.clash_monitor_controller().start(
        &config,
        std::sync::Arc::new(crate::TauriClashEventSink { app: app.clone() }),
    ) {
        Ok(status) => {
            emit_clash_monitor_status(&app, &status);
            Ok(status)
        }
        Err(error) => {
            let message = error.to_string();
            emit_clash_monitor_status(&app, &ClashMonitorStatus::failed(message));
            Err(clash_error(error))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn clash_stop_monitor(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ClashMonitorStatus, AppError> {
    match state.clash_monitor_controller().stop() {
        Ok(status) => {
            emit_clash_monitor_status(&app, &status);
            Ok(status)
        }
        Err(error) => {
            let message = error.to_string();
            emit_clash_monitor_status(&app, &ClashMonitorStatus::failed(message));
            Err(clash_error(error))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn run_speedtest<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    action: voya_core::SpeedActionType,
    index_ids: Vec<String>,
) -> Result<SpeedtestRunResult, AppError> {
    validate_ipc_text_list(
        &index_ids,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Speedtest,
    )?;
    let config = current_config(&state)?;
    let manager = speedtest_manager(&state);
    let emit_app = app.clone();
    let result = manager
        .run_with_callback(
            state.database(),
            &config,
            action,
            index_ids,
            move |result| {
                if let Err(error) = emit_speedtest_result(&emit_app, &result) {
                    tracing::warn!(?error, "failed to emit speedtest result");
                }
            },
        )
        .await
        .map_err(speedtest_error)?;

    let changed_ids = result
        .results
        .iter()
        .map(|item| item.index_id.clone())
        .collect::<Vec<_>>();
    emit_profile_invalidation(&app, "speedtest-updated", changed_ids, false)?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_speedtest<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<SpeedtestStatus, AppError> {
    let cancelled = speedtest_manager(&state)
        .cancel()
        .map_err(speedtest_error)?;
    if cancelled {
        emit_runtime_log(&app, LogLevel::Info, "Speedtest cancellation requested")?;
    }

    speedtest_manager(&state).status().map_err(speedtest_error)
}

#[tauri::command]
#[specta::specta]
pub fn speedtest_status(state: tauri::State<'_, AppState>) -> Result<SpeedtestStatus, AppError> {
    speedtest_manager(&state).status().map_err(speedtest_error)
}

#[tauri::command]
#[specta::specta]
pub fn app_update_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<AppUpdaterStatus, AppError> {
    let current_version = app.package_info().version.to_string();

    Ok(match app.updater() {
        Ok(_) => AppUpdaterStatus {
            current_version,
            state: AppUpdaterState::Ready,
            message: None,
        },
        Err(error) => AppUpdaterStatus {
            current_version,
            state: app_updater_state_for_error(&error),
            message: Some(error.to_string()),
        },
    })
}

#[tauri::command]
#[specta::specta]
pub fn record_app_update_diagnostic(
    state: tauri::State<'_, AppState>,
    action: AppUpdateDiagnosticAction,
    result: AppUpdateDiagnosticResult,
    message: Option<String>,
) -> Result<(), AppError> {
    validate_optional_ipc_text(
        message.as_deref(),
        "app update diagnostic message",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Update,
    )?;
    let config = current_config(&state)?;
    let diagnostics_result = diagnostics_result_for_app_update_diagnostic(result);
    let error_class = match result {
        AppUpdateDiagnosticResult::Failure => Some(diagnostics_error_class_for_app_update_message(
            action,
            message.as_deref(),
        )),
        AppUpdateDiagnosticResult::Success | AppUpdateDiagnosticResult::Skipped => None,
    };
    let event = match action {
        AppUpdateDiagnosticAction::Check => {
            DiagnosticsEvent::update_check(diagnostics_result, error_class)
        }
        AppUpdateDiagnosticAction::Install => {
            DiagnosticsEvent::app_update_install(diagnostics_result, error_class)
        }
    };
    record_diagnostics_event(&state, &config, event);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_status(state: tauri::State<'_, AppState>) -> Result<UpdateStatus, AppError> {
    let config = current_config(&state)?;

    Ok(update_manager(&state).status(&config))
}

#[tauri::command]
#[specta::specta]
pub fn save_update_preferences(
    state: tauri::State<'_, AppState>,
    pre_release: bool,
    selected_target_ids: Vec<String>,
) -> Result<UpdateStatus, AppError> {
    validate_ipc_text_list(
        &selected_target_ids,
        "update target id",
        IPC_ID_MAX_CHARS,
        AppError::Update,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let manager = update_manager(&state);
    manager.save_preferences(&mut config, pre_release, selected_target_ids);
    persist_config_if_changed(&state, &original, &config)?;

    Ok(manager.status(&config))
}

#[tauri::command]
#[specta::specta]
pub fn load_ruleset_geo_sources(
    state: tauri::State<'_, AppState>,
) -> Result<RulesetGeoSourceSettings, AppError> {
    let config = current_config(&state)?;

    Ok(update_manager(&state).source_settings(&config))
}

#[tauri::command]
#[specta::specta]
pub fn save_ruleset_geo_sources(
    state: tauri::State<'_, AppState>,
    settings: RulesetGeoSourceSettings,
) -> Result<RulesetGeoSourceSettings, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let manager = update_manager(&state);
    let saved = manager.save_source_settings(&mut config, settings);
    persist_config_if_changed(&state, &original, &config)?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn check_updates(
    state: tauri::State<'_, AppState>,
    pre_release: bool,
    selected_target_ids: Vec<String>,
    prefer_proxy: bool,
    proxy_url: Option<String>,
) -> Result<UpdateRunResult, AppError> {
    validate_ipc_text_list(
        &selected_target_ids,
        "update target id",
        IPC_ID_MAX_CHARS,
        AppError::Update,
    )?;
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Update,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let manager = update_manager(&state);
    manager.save_preferences(&mut config, pre_release, selected_target_ids.clone());
    persist_config_if_changed(&state, &original, &config)?;

    let result = manager
        .check_updates(
            &config,
            &UpdateRequestOptions {
                pre_release,
                selected_target_ids,
                prefer_proxy,
                proxy_url,
            },
        )
        .await;

    match &result {
        Ok(run) => {
            let (diagnostics_result, error_class) = diagnostics_result_for_update_run(run);
            record_diagnostics_event(
                &state,
                &config,
                DiagnosticsEvent::update_check(diagnostics_result, error_class),
            );
        }
        Err(error) => record_diagnostics_event(
            &state,
            &config,
            DiagnosticsEvent::update_check(
                DiagnosticsResult::Failure,
                Some(diagnostics_error_class_for_update_error(error)),
            ),
        ),
    }

    result.map_err(update_error)
}

#[tauri::command]
#[specta::specta]
pub async fn download_updates(
    state: tauri::State<'_, AppState>,
    pre_release: bool,
    selected_target_ids: Vec<String>,
    prefer_proxy: bool,
    proxy_url: Option<String>,
) -> Result<UpdateRunResult, AppError> {
    validate_ipc_text_list(
        &selected_target_ids,
        "update target id",
        IPC_ID_MAX_CHARS,
        AppError::Update,
    )?;
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Update,
    )?;
    let original = current_config(&state)?;
    let mut config = original.clone();
    let manager = update_manager(&state);
    manager.save_preferences(&mut config, pre_release, selected_target_ids.clone());
    persist_config_if_changed(&state, &original, &config)?;

    let result = manager
        .download_updates(
            &config,
            &UpdateRequestOptions {
                pre_release,
                selected_target_ids,
                prefer_proxy,
                proxy_url,
            },
        )
        .await;

    match &result {
        Ok(run) => {
            let (diagnostics_result, error_class) = diagnostics_result_for_update_run(run);
            record_diagnostics_event(
                &state,
                &config,
                DiagnosticsEvent::update_download(diagnostics_result, error_class),
            );
        }
        Err(error) => {
            let error_class = diagnostics_error_class_for_update_error(error);
            record_diagnostics_event(
                &state,
                &config,
                DiagnosticsEvent::update_download(DiagnosticsResult::Failure, Some(error_class)),
            );
        }
    }

    result.map_err(update_error)
}

#[tauri::command]
#[specta::specta]
pub async fn manual_app_update_links(
    state: tauri::State<'_, AppState>,
    pre_release: bool,
    prefer_proxy: bool,
    proxy_url: Option<String>,
) -> Result<ManualAppUpdateLinks, AppError> {
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Update,
    )?;
    let config = current_config(&state)?;

    update_manager(&state)
        .manual_app_update_links(
            &config,
            &UpdateRequestOptions {
                pre_release,
                selected_target_ids: vec!["app".to_string()],
                prefer_proxy,
                proxy_url,
            },
        )
        .await
        .map_err(update_error)
}

/// Re-install a core binary from the packaged seed (`{resource_dir}/core-seeds/<core>/`)
/// into `bin/<core>/`. This is the recovery action behind the missing-core prompt: the
/// startup seed copy already runs automatically, but this lets the UI re-run it on demand
/// when the binary is absent (e.g. cleared bin dir, antivirus removal, or a skipped first run).
#[tauri::command]
#[specta::specta]
pub fn install_core_seed(
    state: tauri::State<'_, AppState>,
    core_type: CoreType,
) -> Result<CoreSeedInstallResult, AppError> {
    let Some(seed_dir) = state.core_seed_resource_dir() else {
        return Ok(CoreSeedInstallResult {
            core_type,
            status: CoreSeedInstallStatus::SeedMissing,
            installed_files: Vec::new(),
        });
    };

    let outcome = copy_seed_core_asset(state.runtime_paths(), seed_dir, core_type)
        .map_err(core_seed_install_error)?;

    Ok(core_seed_install_result(outcome))
}

#[tauri::command]
#[specta::specta]
pub fn backup_status(state: tauri::State<'_, AppState>) -> Result<BackupStatus, AppError> {
    let config = current_config(&state)?;

    Ok(backup_manager(&state).status(&config))
}

#[tauri::command]
#[specta::specta]
pub fn backup_save_webdav_settings(
    state: tauri::State<'_, AppState>,
    settings: WebDavItem,
) -> Result<WebDavItem, AppError> {
    let original = current_config(&state)?;
    let mut config = original.clone();
    let saved = backup_manager(&state).save_webdav_settings(&mut config, settings);
    persist_config_if_changed(&state, &original, &config)?;

    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn backup_create_local(
    state: tauri::State<'_, AppState>,
    output_path: Option<String>,
) -> Result<BackupOperationResult, AppError> {
    validate_optional_ipc_text(
        output_path.as_deref(),
        "backup output path",
        IPC_PATH_MAX_CHARS,
        AppError::Backup,
    )?;
    let config = current_config(&state)?;
    let output_path = output_path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from);

    backup_manager(&state)
        .create_local_backup(&config, output_path.as_deref())
        .await
        .map_err(backup_error)
}

#[tauri::command]
#[specta::specta]
pub async fn backup_restore_local<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    input_path: String,
) -> Result<BackupRestoreResult, AppError> {
    validate_required_ipc_text(
        &input_path,
        "backup restore path",
        IPC_PATH_MAX_CHARS,
        AppError::Backup,
    )?;
    let input_path = resolve_scoped_ipc_file(
        &input_path,
        state.runtime_paths().backup_dir(),
        IpcFileScope::BackupRestore,
    )?;
    let result = backup_manager(&state)
        .restore_local_backup(&input_path)
        .await
        .map_err(backup_restore_error)?;
    replace_current_config(&state, &result.restored_config)?;
    emit_backup_invalidation(&app, "backup-restored")?;

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn backup_webdav_check(
    state: tauri::State<'_, AppState>,
    settings: WebDavItem,
) -> Result<BackupOperationResult, AppError> {
    let config = save_webdav_settings_for_operation(&state, settings)?;

    backup_manager(&state)
        .webdav_check(&config.web_dav_item)
        .await
        .map_err(backup_error)
}

#[tauri::command]
#[specta::specta]
pub async fn backup_webdav_push(
    state: tauri::State<'_, AppState>,
    settings: WebDavItem,
) -> Result<BackupRemoteResult, AppError> {
    let config = save_webdav_settings_for_operation(&state, settings)?;

    backup_manager(&state)
        .webdav_push(&config, &config.web_dav_item)
        .await
        .map_err(backup_error)
}

#[tauri::command]
#[specta::specta]
pub async fn backup_webdav_pull<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: WebDavItem,
) -> Result<BackupRestoreResult, AppError> {
    let config = save_webdav_settings_for_operation(&state, settings)?;
    let result = backup_manager(&state)
        .webdav_pull(&config.web_dav_item)
        .await
        .map_err(backup_error)?;
    replace_current_config(&state, &result.restored_config)?;
    emit_backup_invalidation(&app, "backup-webdav-restored")?;

    Ok(result)
}

#[cfg(debug_assertions)]
#[tauri::command]
#[specta::specta]
pub fn ipc_demo_round_trip<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: DemoRequest,
) -> Result<DemoResponse, AppError> {
    let response = DemoResponse {
        echoed_message: request.message.clone(),
        message_length: u32::try_from(request.message.chars().count()).unwrap_or(u32::MAX),
    };

    InvalidateEvent {
        keys: vec![QueryInvalidation {
            query_key: vec!["ipc-demo".to_string()],
            reason: "demo-round-trip".to_string(),
        }],
    }
    .emit(&app)
    .map_err(|error| AppError::EventEmit(error.to_string()))?;

    TransientStreamEvent::LogLine(LogLineEvent {
        id: next_log_line_id(),
        level: LogLevel::Info,
        line: format!("IPC demo echoed {} characters", response.message_length),
    })
    .emit(&app)
    .map_err(|error| AppError::EventEmit(error.to_string()))?;

    TransientStreamEvent::CoreState(CoreStateEvent {
        state: CoreState::Disconnected,
        active_profile_id: None,
        main_pid: None,
        pre_pid: None,
        running_core_type: None,
    })
    .emit(&app)
    .map_err(|error| AppError::EventEmit(error.to_string()))?;

    AppEvent::Notice(AppNotice {
        level: AppNoticeLevel::Info,
        title: "IPC demo".to_string(),
        message: Some("Typed command and event bridge are connected.".to_string()),
    })
    .emit(&app)
    .map_err(|error| AppError::EventEmit(error.to_string()))?;

    Ok(response)
}

fn current_config(state: &AppState) -> Result<AppConfig, AppError> {
    state
        .config()
        .read()
        .map_err(|_| AppError::State("app config lock is poisoned".to_string()))
        .map(|guard| guard.clone())
}

async fn export_profiles_result(
    state: &AppState,
    index_ids: Vec<String>,
    format: ExportProfilesFormat,
) -> Result<ExportProfilesResult, AppError> {
    validate_ipc_text_list(
        &index_ids,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Export,
    )?;
    let config = current_config(state)?;

    ExportManager::new(state.database())
        .export_profiles(
            state.runtime_paths(),
            &config,
            TargetOs::current(),
            ExportProfilesRequest { index_ids, format },
        )
        .await
        .map_err(export_error)
}

fn validate_present_ipc_text(
    value: Option<&str>,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_present_text(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

fn validate_optional_ipc_text(
    value: Option<&str>,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_optional_text(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

fn validate_ipc_text_list(
    values: &[String],
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_text_list(values, max_chars, IPC_LIST_MAX_ITEMS)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

fn validate_required_ipc_text(
    value: &str,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_required_text(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

fn validate_ipc_text(
    value: &str,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_text(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

fn ipc_text_error(
    error: InputSafetyError,
    field: &str,
    make_error: fn(String) -> AppError,
) -> AppError {
    let reason = match error {
        InputSafetyError::EmptyValue => "value is required".to_string(),
        InputSafetyError::TooLong => "value is too long".to_string(),
        InputSafetyError::ControlCharacters => "control characters are not allowed".to_string(),
        InputSafetyError::TooManyItems => "too many items".to_string(),
        error => error.to_string(),
    };

    make_error(format!("invalid {field}: {reason}"))
}

pub(crate) fn diagnostics_release_channel() -> DiagnosticsReleaseChannel {
    if cfg!(debug_assertions) {
        DiagnosticsReleaseChannel::Debug
    } else {
        DiagnosticsReleaseChannel::Stable
    }
}

pub(crate) fn record_app_start_diagnostics<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let state = app.state::<AppState>();
    match current_config(&state) {
        Ok(config) => record_diagnostics_event(
            &state,
            &config,
            DiagnosticsEvent::app_start(DiagnosticsResult::Success),
        ),
        Err(error) => tracing::debug!(?error, "failed to load config for app start diagnostics"),
    }
}

fn current_diagnostics_settings(state: &AppState) -> Result<DiagnosticsSettings, AppError> {
    let original = current_config(state)?;
    let mut config = original.clone();
    let settings = diagnostics_settings_for_config(&mut config);
    persist_config_if_changed(state, &original, &config)?;

    Ok(settings)
}

fn diagnostics_settings_for_config(config: &mut AppConfig) -> DiagnosticsSettings {
    if config.diagnostics_item.enabled {
        prepare_diagnostics_settings(
            config,
            env!("CARGO_PKG_VERSION"),
            diagnostics_release_channel(),
        )
    } else {
        diagnostics_settings(
            config,
            env!("CARGO_PKG_VERSION"),
            diagnostics_release_channel(),
        )
    }
}

fn diagnostics_status_response(
    settings: &DiagnosticsSettings,
    client: &DiagnosticsClient,
) -> DiagnosticsStatus {
    DiagnosticsStatus {
        enabled: settings.enabled(),
        delivery_configured: settings.endpoint_url().is_some(),
        queued_events: u32::try_from(client.queued_events()).unwrap_or(u32::MAX),
        queued_bytes: u32::try_from(client.queued_bytes()).unwrap_or(u32::MAX),
    }
}

fn record_diagnostics_event(state: &AppState, config: &AppConfig, event: DiagnosticsEvent) {
    let settings = diagnostics_settings(
        config,
        env!("CARGO_PKG_VERSION"),
        diagnostics_release_channel(),
    );
    let client = state.diagnostics_client();

    tauri::async_runtime::spawn(async move {
        let mut client = client.lock().await;
        let outcome = client.record(&settings, event);
        if outcome.status != DiagnosticsRecordStatus::Queued {
            return;
        }

        let flush = client.flush(&settings).await;
        tracing::debug!(
            status = ?flush.status,
            attempted_events = flush.attempted_events,
            queued_events = flush.queued_events,
            "diagnostics flush completed"
        );
    });
}

fn diagnostics_result_for_update_run(
    run: &UpdateRunResult,
) -> (DiagnosticsResult, Option<DiagnosticsErrorClass>) {
    if run
        .results
        .iter()
        .any(|result| result.status == UpdateResultStatus::Error)
    {
        return (
            DiagnosticsResult::Failure,
            Some(DiagnosticsErrorClass::Unknown),
        );
    }

    if run
        .results
        .iter()
        .all(|result| result.status == UpdateResultStatus::Skipped)
    {
        return (DiagnosticsResult::Skipped, None);
    }

    (DiagnosticsResult::Success, None)
}

fn diagnostics_result_for_app_update_diagnostic(
    result: AppUpdateDiagnosticResult,
) -> DiagnosticsResult {
    match result {
        AppUpdateDiagnosticResult::Success => DiagnosticsResult::Success,
        AppUpdateDiagnosticResult::Failure => DiagnosticsResult::Failure,
        AppUpdateDiagnosticResult::Skipped => DiagnosticsResult::Skipped,
    }
}

fn record_runtime_start_failure_diagnostics(
    state: &AppState,
    config: &AppConfig,
    error: &RuntimeError,
) {
    record_diagnostics_event(
        state,
        config,
        DiagnosticsEvent::runtime_start_failure(diagnostics_error_class_for_runtime_error(error)),
    );

    if let Some(core_type) = runtime_missing_core_type(error) {
        record_diagnostics_event(state, config, DiagnosticsEvent::core_missing(core_type));
    }
}

fn runtime_missing_core_type(error: &RuntimeError) -> Option<CoreType> {
    match error {
        RuntimeError::MissingCoreInfo(core_type)
        | RuntimeError::CoreInfo(CoreInfoError::MissingCoreInfo(core_type))
        | RuntimeError::CoreInfo(CoreInfoError::ExecutableNotFound { core_type, .. }) => {
            Some(*core_type)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
enum IpcFileScope {
    ProfileImport,
    BackupRestore,
}

impl IpcFileScope {
    fn invalid_path_error(self) -> AppError {
        match self {
            Self::ProfileImport => AppError::Subscription(
                "invalid import file path: provide a relative file name inside the import directory"
                    .to_string(),
            ),
            Self::BackupRestore => AppError::Backup(
                "invalid backup restore path: provide a relative file name inside the backup directory"
                    .to_string(),
            ),
        }
    }

    fn unavailable_error(self) -> AppError {
        match self {
            Self::ProfileImport => AppError::Subscription(
                "import file is not available in the import directory".to_string(),
            ),
            Self::BackupRestore => {
                AppError::Backup("backup file is not available in the backup directory".to_string())
            }
        }
    }

    fn prepare_error(self, source: io::Error) -> AppError {
        match self {
            Self::ProfileImport => {
                AppError::Subscription(format!("failed to prepare import directory: {source}"))
            }
            Self::BackupRestore => {
                AppError::Backup(format!("failed to prepare backup directory: {source}"))
            }
        }
    }

    fn scoped_file_error(self, error: InputSafetyError) -> AppError {
        match error {
            InputSafetyError::InvalidPath
            | InputSafetyError::EmptyValue
            | InputSafetyError::TooLong
            | InputSafetyError::ControlCharacters
            | InputSafetyError::TooManyItems => self.invalid_path_error(),
            InputSafetyError::PathUnavailable => self.unavailable_error(),
            InputSafetyError::PrepareDirectory(source) => self.prepare_error(source),
        }
    }
}

fn resolve_scoped_ipc_file(
    input: &str,
    base_dir: &std::path::Path,
    scope: IpcFileScope,
) -> Result<PathBuf, AppError> {
    input_safety::resolve_scoped_file(input, base_dir, IPC_PATH_MAX_CHARS)
        .map_err(|error| scope.scoped_file_error(error))
}

fn diagnostics_error_class_for_runtime_error(error: &RuntimeError) -> DiagnosticsErrorClass {
    match error {
        RuntimeError::MissingCoreInfo(_)
        | RuntimeError::CoreInfo(CoreInfoError::MissingCoreInfo(_))
        | RuntimeError::CoreInfo(CoreInfoError::ExecutableNotFound { .. }) => {
            DiagnosticsErrorClass::CoreMissing
        }
        RuntimeError::CoreInfo(error) => diagnostics_error_class_for_core_info_error(error),
        RuntimeError::Supervisor(SupervisorError::ElevationNotGranted(_)) => {
            DiagnosticsErrorClass::PermissionDenied
        }
        RuntimeError::Supervisor(_) => DiagnosticsErrorClass::RuntimeStartFailed,
        _ => DiagnosticsErrorClass::RuntimeStartFailed,
    }
}

fn diagnostics_error_class_for_core_info_error(error: &CoreInfoError) -> DiagnosticsErrorClass {
    match error {
        CoreInfoError::MissingCoreInfo(_) | CoreInfoError::ExecutableNotFound { .. } => {
            DiagnosticsErrorClass::CoreMissing
        }
        CoreInfoError::CreateCoreBinDir { source, .. }
        | CoreInfoError::InspectExecutable { source, .. }
        | CoreInfoError::InspectCoreSeed { source, .. }
        | CoreInfoError::ReadCoreSeedDir { source, .. }
        | CoreInfoError::CopyCoreSeedAsset { source, .. }
        | CoreInfoError::ChmodExecutable { source, .. }
            if source.kind() == io::ErrorKind::PermissionDenied =>
        {
            DiagnosticsErrorClass::PermissionDenied
        }
        _ => DiagnosticsErrorClass::Unknown,
    }
}

fn diagnostics_error_class_for_update_error(error: &UpdateManagerError) -> DiagnosticsErrorClass {
    match error {
        UpdateManagerError::Download(_) => DiagnosticsErrorClass::NetworkUnavailable,
        UpdateManagerError::Release(_) | UpdateManagerError::RulesetGeo(_) => {
            DiagnosticsErrorClass::EndpointUnavailable
        }
        UpdateManagerError::Runtime(error) => diagnostics_error_class_for_runtime_error(error),
        _ => DiagnosticsErrorClass::Unknown,
    }
}

fn diagnostics_error_class_for_app_update_message(
    action: AppUpdateDiagnosticAction,
    message: Option<&str>,
) -> DiagnosticsErrorClass {
    let message = message.unwrap_or_default().to_ascii_lowercase();

    if message.contains("unsupported") {
        return DiagnosticsErrorClass::Unknown;
    }

    if message.contains("emptyendpoint")
        || message.contains("empty endpoint")
        || message.contains("endpoint")
        || message.contains("fetch")
        || message.contains("network")
        || message.contains("request")
        || message.contains("timeout")
        || message.contains("http")
    {
        return DiagnosticsErrorClass::EndpointUnavailable;
    }

    match action {
        AppUpdateDiagnosticAction::Check => DiagnosticsErrorClass::EndpointUnavailable,
        AppUpdateDiagnosticAction::Install => DiagnosticsErrorClass::UpdaterInstallFailed,
    }
}

fn runtime_manager(state: &AppState) -> RuntimeManager<'_> {
    let manager = RuntimeManager::new(
        state.database(),
        state.runtime_paths().clone(),
        state.supervisor(),
    );

    if let Some(seed_dir) = state.core_seed_resource_dir() {
        manager.with_core_seed_resource_dir(seed_dir.to_path_buf())
    } else {
        manager
    }
}

fn speedtest_manager(state: &AppState) -> SpeedtestManager {
    state.speedtest_manager()
}

fn tun_manager(state: &AppState) -> TunManager {
    TunManager::new(state.elevation_manager().state())
}

fn update_manager(state: &AppState) -> UpdateManager<'_> {
    UpdateManager::new(state.database(), state.runtime_paths().clone())
}

fn backup_manager(state: &AppState) -> BackupManager<'_> {
    BackupManager::new(
        state.database(),
        state.config_store(),
        state.runtime_paths().clone(),
    )
}

struct TauriHotkeyRegistrar<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R> HotkeyRegistrar for TauriHotkeyRegistrar<R>
where
    R: tauri::Runtime + 'static,
{
    fn unregister_all(&self) -> Result<(), HotkeyManagerError> {
        self.app
            .global_shortcut()
            .unregister_all()
            .map_err(|error| HotkeyManagerError::Register(error.to_string()))
    }

    fn register(&self, bindings: &[GlobalHotkeyBinding]) -> Result<(), HotkeyManagerError> {
        for binding in bindings {
            voya_platform::hotkeys::validate_hotkey_accelerator(
                binding.action,
                &binding.accelerator,
            )?;
            let action = binding.action;
            self.app
                .global_shortcut()
                .on_shortcut(
                    binding.accelerator.as_str(),
                    move |app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            handle_global_hotkey(app, action);
                        }
                    },
                )
                .map_err(|error| HotkeyManagerError::Register(error.to_string()))?;
        }

        Ok(())
    }
}

fn handle_global_hotkey<R: tauri::Runtime>(app: &tauri::AppHandle<R>, action: GlobalHotkey) {
    match action {
        GlobalHotkey::ShowForm => toggle_main_window(app),
        GlobalHotkey::SystemProxyClear => {
            spawn_global_hotkey_proxy_mode(app, SysProxyType::ForcedClear);
        }
        GlobalHotkey::SystemProxySet => {
            spawn_global_hotkey_proxy_mode(app, SysProxyType::ForcedChange);
        }
        GlobalHotkey::SystemProxyUnchanged => {
            spawn_global_hotkey_proxy_mode(app, SysProxyType::Unchanged);
        }
        GlobalHotkey::SystemProxyPac => {
            spawn_global_hotkey_proxy_mode(app, SysProxyType::Pac);
        }
    }
}

fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    match window.is_visible() {
        Ok(true) => {
            if let Err(error) = window.hide() {
                tracing::warn!(?error, "failed to hide main window from global hotkey");
            }
        }
        Ok(false) | Err(_) => {
            if let Err(error) = window.show() {
                tracing::warn!(?error, "failed to show main window from global hotkey");
            }
            if let Err(error) = window.set_focus() {
                tracing::warn!(?error, "failed to focus main window from global hotkey");
            }
        }
    }
}

fn spawn_global_hotkey_proxy_mode<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    mode: SysProxyType,
) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = set_system_proxy_mode(app.clone(), state, mode) {
            tracing::warn!(?error, "global hotkey system proxy mode switch failed");
        }
    });
}

fn apply_system_proxy<R>(
    _app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
    force_disable: bool,
) -> Result<SystemProxyStatus, SystemProxyManagerError>
where
    R: tauri::Runtime,
{
    state
        .system_proxy_manager()
        .apply_config(config, force_disable)
}

pub(crate) fn restore_system_proxy<R>(
    _app: &tauri::AppHandle<R>,
    state: &AppState,
) -> Result<SystemProxyStatus, AppError>
where
    R: tauri::Runtime,
{
    let config = current_config(state)?;

    state
        .system_proxy_manager()
        .restore(&config)
        .map_err(sysproxy_error)
}

fn runtime_status_response(snapshot: SupervisorSnapshot) -> RuntimeStatusResponse {
    RuntimeStatusResponse {
        state: match snapshot.state {
            SupervisorConnectionState::Disconnected => RuntimeConnectionState::Disconnected,
            SupervisorConnectionState::Connected => RuntimeConnectionState::Connected,
        },
        active_profile_id: snapshot.active_profile_id,
        main_pid: snapshot.main_pid,
        pre_pid: snapshot.pre_pid,
        running_core_type: snapshot.running_core_type,
    }
}

fn system_proxy_status_response(status: SystemProxyStatus) -> SystemProxyStatusResponse {
    SystemProxyStatusResponse {
        requested_mode: status.requested_type,
        effective_mode: status.effective_type,
        pac_available: status.pac_available,
        proxy: status.proxy,
        exceptions: status.exceptions,
        pac_url: status.pac_url,
    }
}

fn emit_runtime_log<R>(
    app: &tauri::AppHandle<R>,
    level: LogLevel,
    line: &str,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::LogLine(LogLineEvent {
        id: next_log_line_id(),
        level,
        line: line.to_string(),
    })
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_core_state<R>(
    app: &tauri::AppHandle<R>,
    state: CoreState,
    active_profile_id: Option<String>,
    snapshot: Option<&SupervisorSnapshot>,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::CoreState(core_state_event(state, active_profile_id, snapshot))
        .emit(app)
        .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_statistics_zero<R>(app: &tauri::AppHandle<R>) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::Statistics(super::events::StatisticsSnapshot {
        active_profile_id: None,
        proxy_upload_bytes_per_second: 0.0,
        proxy_download_bytes_per_second: 0.0,
        direct_upload_bytes_per_second: 0.0,
        direct_download_bytes_per_second: 0.0,
        upload_bytes_per_second: 0.0,
        download_bytes_per_second: 0.0,
        server_stat: None,
    })
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_speedtest_result<R>(
    app: &tauri::AppHandle<R>,
    result: &SpeedTestResult,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::SpeedtestResult(result.clone())
        .emit(app)
        .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn core_state_event(
    state: CoreState,
    active_profile_id: Option<String>,
    snapshot: Option<&SupervisorSnapshot>,
) -> CoreStateEvent {
    CoreStateEvent {
        state,
        active_profile_id: snapshot
            .and_then(|snapshot| snapshot.active_profile_id.clone())
            .or(active_profile_id),
        main_pid: snapshot.and_then(|snapshot| snapshot.main_pid),
        pre_pid: snapshot.and_then(|snapshot| snapshot.pre_pid),
        running_core_type: snapshot.and_then(|snapshot| snapshot.running_core_type),
    }
}

fn persist_config_if_changed(
    state: &AppState,
    original: &AppConfig,
    updated: &AppConfig,
) -> Result<(), AppError> {
    if original == updated {
        return Ok(());
    }

    state
        .config_store()
        .save(updated)
        .map_err(|error| AppError::ConfigSave(error.to_string()))?;
    let mut guard = state
        .config()
        .write()
        .map_err(|_| AppError::State("app config lock is poisoned".to_string()))?;
    *guard = updated.clone();

    Ok(())
}

fn replace_current_config(state: &AppState, config: &AppConfig) -> Result<(), AppError> {
    let mut guard = state
        .config()
        .write()
        .map_err(|_| AppError::State("app config lock is poisoned".to_string()))?;
    *guard = config.clone();

    Ok(())
}

fn save_webdav_settings_for_operation(
    state: &AppState,
    settings: WebDavItem,
) -> Result<AppConfig, AppError> {
    let original = current_config(state)?;
    let mut config = original.clone();
    backup_manager(state).save_webdav_settings(&mut config, settings);
    persist_config_if_changed(state, &original, &config)?;

    Ok(config)
}

fn profile_error(error: ProfileManagerError) -> AppError {
    match error {
        ProfileManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Profile(error.to_string()),
    }
}

fn runtime_error(error: RuntimeError) -> AppError {
    let message = error.to_string();
    match error {
        RuntimeError::CoreInfo(CoreInfoError::ExecutableNotFound {
            core_type,
            search_dir: _,
            candidates,
            url,
        }) => AppError::MissingCore(MissingCoreError {
            message: missing_core_error_message(core_type),
            core_type,
            search_dir: missing_core_search_dir_label(),
            candidates: missing_core_candidates(&candidates),
            download_url: url.to_string(),
        }),
        _ => AppError::Runtime(message),
    }
}

fn missing_core_error_message(core_type: CoreType) -> String {
    format!(
        "core {core_type:?} executable is missing; install or update the core package and try again"
    )
}

fn missing_core_search_dir_label() -> String {
    MISSING_CORE_SEARCH_DIR_LABEL.to_string()
}

fn missing_core_candidates(candidates: &str) -> Vec<String> {
    candidates
        .split(',')
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn core_seed_install_result(outcome: CoreSeedCopyOutcome) -> CoreSeedInstallResult {
    let status = match outcome.status {
        CoreSeedCopyStatus::Copied => CoreSeedInstallStatus::Installed,
        CoreSeedCopyStatus::AlreadyInstalled => CoreSeedInstallStatus::AlreadyInstalled,
        CoreSeedCopyStatus::SeedMissing => CoreSeedInstallStatus::SeedMissing,
    };
    let installed_files = outcome
        .copied_files
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();

    CoreSeedInstallResult {
        core_type: outcome.core_type,
        status,
        installed_files,
    }
}

fn core_seed_install_error(error: CoreInfoError) -> AppError {
    AppError::Runtime(error.to_string())
}

fn group_error(error: GroupManagerError) -> AppError {
    match error {
        GroupManagerError::Database(error) => AppError::Database(error.to_string()),
        GroupManagerError::Profile(error) => profile_error(error),
        error => AppError::Group(error.to_string()),
    }
}

fn subscription_error(error: SubscriptionManagerError) -> AppError {
    match error {
        SubscriptionManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Subscription(error.to_string()),
    }
}

fn routing_error(error: RoutingManagerError) -> AppError {
    match error {
        RoutingManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Routing(error.to_string()),
    }
}

fn speedtest_error(error: SpeedtestError) -> AppError {
    AppError::Speedtest(error.to_string())
}

fn preset_error(error: PresetManagerError) -> AppError {
    match error {
        PresetManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Preset(error.to_string()),
    }
}

fn qr_error(error: QrCodeError) -> AppError {
    match error {
        QrCodeError::EmptyContent => AppError::Qr("QR content is empty".to_string()),
        QrCodeError::Generate(_) => AppError::Qr("failed to generate QR code".to_string()),
    }
}

fn certificate_error(error: CertificateError) -> AppError {
    AppError::Certificate(error.to_string())
}

fn template_error(error: FullConfigTemplateManagerError) -> AppError {
    match error {
        FullConfigTemplateManagerError::Db(error) => AppError::Database(error.to_string()),
        error => AppError::Template(error.to_string()),
    }
}

fn export_error(error: ExportManagerError) -> AppError {
    match error {
        ExportManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Export(error.to_string()),
    }
}

fn clash_error(error: ClashManagerError) -> AppError {
    AppError::Clash(error.to_string())
}

fn dns_error(error: DnsManagerError) -> AppError {
    match error {
        DnsManagerError::Database(error) => AppError::Database(error.to_string()),
        DnsManagerError::Validation(issues) => AppError::Dns(DnsCommandError {
            message: "DNS settings validation failed".to_string(),
            issues,
        }),
    }
}

fn autostart_error(error: AutostartManagerError) -> AppError {
    AppError::Autostart(error.to_string())
}

fn hotkey_error(error: HotkeyManagerError) -> AppError {
    AppError::Hotkey(error.to_string())
}

fn sysproxy_error(error: SystemProxyManagerError) -> AppError {
    AppError::SysProxy(error.to_string())
}

fn tun_error(error: TunManagerError) -> AppError {
    match error {
        TunManagerError::ElevationRequired => AppError::Sudo(error.to_string()),
        TunManagerError::UnsupportedPlatform => AppError::Tun(error.to_string()),
    }
}

fn app_updater_state_for_error(error: &tauri_plugin_updater::Error) -> AppUpdaterState {
    match error {
        tauri_plugin_updater::Error::EmptyEndpoints => AppUpdaterState::Unconfigured,
        tauri_plugin_updater::Error::UnsupportedArch
        | tauri_plugin_updater::Error::UnsupportedOs => AppUpdaterState::Unsupported,
        _ => AppUpdaterState::Error,
    }
}

fn update_error(error: UpdateManagerError) -> AppError {
    match error {
        UpdateManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Update(error.to_string()),
    }
}

fn backup_error(error: BackupManagerError) -> AppError {
    match error {
        BackupManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Backup(error.to_string()),
    }
}

fn backup_restore_error(error: BackupManagerError) -> AppError {
    match error {
        BackupManagerError::Database(error) => AppError::Database(error.to_string()),
        BackupManagerError::Io { source, .. } => {
            AppError::Backup(format!("backup restore filesystem error: {source}"))
        }
        BackupManagerError::Zip { source, .. } => {
            AppError::Backup(format!("backup restore zip error: {source}"))
        }
        BackupManagerError::InvalidArchive(_) => {
            AppError::Backup("invalid backup archive".to_string())
        }
        BackupManagerError::ConfigSerialize(error) => {
            AppError::Backup(format!("failed to serialize backup data: {error}"))
        }
        BackupManagerError::ConfigDeserialize(error) => {
            AppError::Backup(format!("failed to deserialize backup data: {error}"))
        }
        BackupManagerError::RestoreRollback { restore, rollback } => AppError::Backup(format!(
            "backup restore failed and rollback failed: restore error: {}; rollback error: {}",
            backup_restore_error_message(*restore),
            backup_restore_error_message(*rollback)
        )),
        BackupManagerError::WebDav(error) => AppError::Backup(error.to_string()),
    }
}

fn backup_restore_error_message(error: BackupManagerError) -> String {
    match backup_restore_error(error) {
        AppError::Backup(message) | AppError::Database(message) => message,
        _ => "backup restore failed".to_string(),
    }
}

fn elevation_error(error: ElevationError) -> AppError {
    AppError::Sudo(error.to_string())
}

fn current_unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

fn emit_profile_invalidation<R, I>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    affected_index_ids: I,
    active_changed: bool,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
    I: IntoIterator<Item = String>,
{
    let mut keys = BTreeSet::new();
    keys.insert(vec!["profiles".to_string()]);
    keys.insert(vec!["profile-ex".to_string()]);
    if active_changed {
        keys.insert(vec!["active-profile".to_string()]);
    }
    for index_id in affected_index_ids {
        if !index_id.is_empty() {
            keys.insert(vec!["profile".to_string(), index_id]);
        }
    }

    InvalidateEvent {
        keys: keys
            .into_iter()
            .map(|query_key| QueryInvalidation {
                query_key,
                reason: reason.to_string(),
            })
            .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_subscription_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    profiles_changed: bool,
    config_changed: bool,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    let mut keys = BTreeSet::new();
    keys.insert(vec!["subscriptions".to_string()]);
    if profiles_changed {
        keys.insert(vec!["profiles".to_string()]);
        keys.insert(vec!["profile-ex".to_string()]);
    }
    if config_changed {
        keys.insert(vec!["active-profile".to_string()]);
    }

    InvalidateEvent {
        keys: keys
            .into_iter()
            .map(|query_key| QueryInvalidation {
                query_key,
                reason: reason.to_string(),
            })
            .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_routing_invalidation<R, I>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    affected_ids: I,
    active_changed: bool,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
    I: IntoIterator<Item = String>,
{
    let mut keys = BTreeSet::new();
    keys.insert(vec!["routings".to_string()]);
    if active_changed {
        keys.insert(vec!["active-routing".to_string()]);
    }
    for id in affected_ids {
        if !id.is_empty() {
            keys.insert(vec!["routing".to_string(), id]);
        }
    }

    InvalidateEvent {
        keys: keys
            .into_iter()
            .map(|query_key| QueryInvalidation {
                query_key,
                reason: reason.to_string(),
            })
            .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_dns_invalidation<R>(app: &tauri::AppHandle<R>, reason: &str) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    InvalidateEvent {
        keys: [
            vec!["dns".to_string()],
            vec!["app-config".to_string()],
            vec!["active-dns".to_string()],
        ]
        .into_iter()
        .map(|query_key| QueryInvalidation {
            query_key,
            reason: reason.to_string(),
        })
        .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_full_config_template_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    InvalidateEvent {
        keys: [
            vec!["full-config-templates".to_string()],
            vec!["app-config".to_string()],
        ]
        .into_iter()
        .map(|query_key| QueryInvalidation {
            query_key,
            reason: reason.to_string(),
        })
        .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_preset_invalidation<R>(app: &tauri::AppHandle<R>, reason: &str) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    InvalidateEvent {
        keys: [
            vec!["dns".to_string()],
            vec!["app-config".to_string()],
            vec!["active-dns".to_string()],
            vec!["routings".to_string()],
            vec!["active-routing".to_string()],
        ]
        .into_iter()
        .map(|query_key| QueryInvalidation {
            query_key,
            reason: reason.to_string(),
        })
        .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_clash_invalidation<R>(app: &tauri::AppHandle<R>, reason: &str) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    InvalidateEvent {
        keys: [
            vec!["clash".to_string()],
            vec!["clash-proxies".to_string()],
            vec!["clash-connections".to_string()],
            vec!["app-config".to_string()],
        ]
        .into_iter()
        .map(|query_key| QueryInvalidation {
            query_key,
            reason: reason.to_string(),
        })
        .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_clash_monitor_status<R>(app: &tauri::AppHandle<R>, status: &ClashMonitorStatus)
where
    R: tauri::Runtime,
{
    if let Err(error) = TransientStreamEvent::ClashMonitorStatus(status.clone()).emit(app) {
        tracing::warn!(?error, ?status, "failed to emit Clash monitor status event");
    }
}

fn emit_backup_invalidation<R>(app: &tauri::AppHandle<R>, reason: &str) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    InvalidateEvent {
        keys: [
            vec!["app-config".to_string()],
            vec!["backup".to_string()],
            vec!["profiles".to_string()],
            vec!["profile-ex".to_string()],
            vec!["subscriptions".to_string()],
            vec!["routings".to_string()],
            vec!["dns".to_string()],
        ]
        .into_iter()
        .map(|query_key| QueryInvalidation {
            query_key,
            reason: reason.to_string(),
        })
        .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_app_config_invalidation<R>(app: &tauri::AppHandle<R>, reason: &str) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    InvalidateEvent {
        keys: [vec!["app-config".to_string()]]
            .into_iter()
            .map(|query_key| QueryInvalidation {
                query_key,
                reason: reason.to_string(),
            })
            .collect(),
    }
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn emit_tun_changed<R>(app: &tauri::AppHandle<R>, enabled: bool) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::TunChanged(super::events::TunChanged { enabled })
        .emit(app)
        .map_err(|error| AppError::EventEmit(error.to_string()))
}

async fn restart_if_connected_after_routing_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    restart_if_connected_after_config_change(app, state, config, "Routing changed").await
}

async fn restart_if_connected_after_config_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
    reason: &str,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    let status = runtime_manager(state)
        .status()
        .await
        .map_err(runtime_error)?;
    if status.state != SupervisorConnectionState::Connected {
        return Ok(());
    }

    emit_runtime_log(app, LogLevel::Info, &format!("{reason}; restarting core"))?;
    emit_core_state(
        app,
        CoreState::Connecting,
        Some(config.index_id.clone()).filter(|value| !value.is_empty()),
        None,
    )?;

    match runtime_manager(state).restart(config).await {
        Ok(snapshot) => {
            emit_runtime_log(
                app,
                LogLevel::Info,
                &format!("Core supervisor restarted after {reason}"),
            )?;
            emit_core_state(app, CoreState::Connected, None, Some(&snapshot))?;
            match apply_system_proxy(app, state, config, false) {
                Ok(status) => emit_sysproxy_changed(app, &status)?,
                Err(error) => emit_runtime_log(
                    app,
                    LogLevel::Warn,
                    &format!("System proxy apply failed: {error}"),
                )?,
            }
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            emit_runtime_log(app, LogLevel::Error, &message)?;
            emit_core_state(app, CoreState::Disconnected, None, None)?;
            record_runtime_start_failure_diagnostics(state, config, &error);
            Err(runtime_error(error))
        }
    }
}

fn emit_sysproxy_changed<R>(
    app: &tauri::AppHandle<R>,
    status: &SystemProxyStatus,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::SysProxyChanged(super::events::SysProxyChanged {
        requested_mode: sysproxy_mode(status.requested_type),
        effective_mode: sysproxy_mode(status.effective_type),
        pac_available: status.pac_available,
        proxy: status.proxy.clone(),
    })
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

fn sysproxy_mode(mode: SysProxyType) -> super::events::SysProxyMode {
    match mode {
        SysProxyType::ForcedClear => super::events::SysProxyMode::ForcedClear,
        SysProxyType::ForcedChange => super::events::SysProxyMode::ForcedChange,
        SysProxyType::Unchanged => super::events::SysProxyMode::Unchanged,
        SysProxyType::Pac => super::events::SysProxyMode::Pac,
    }
}
