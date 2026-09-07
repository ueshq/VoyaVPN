use super::*;

pub(super) fn current_config(state: &AppState) -> Result<AppConfig, AppError> {
    Ok(state.config_mutations().current_config())
}

pub(super) async fn begin_config_mutation(
    state: &AppState,
) -> Result<ConfigMutationGuard<'_>, AppError> {
    state
        .config_mutations()
        .begin()
        .await
        .map_err(config_mutation_error)
}

pub(super) async fn commit_config_mutation(
    mutation: ConfigMutationGuard<'_>,
) -> Result<AppConfig, AppError> {
    mutation.commit().await.map_err(config_mutation_error)
}

pub(super) async fn export_profiles_result(
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

    state
        .services()
        .exports()
        .export_profiles(
            state.runtime_paths(),
            &config,
            TargetOs::current(),
            ExportProfilesRequest { index_ids, format },
        )
        .await
        .map_err(export_error)
}

pub(super) fn validate_present_ipc_text(
    value: Option<&str>,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_present_text(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

pub(super) fn validate_optional_ipc_text(
    value: Option<&str>,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_optional_text(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

pub(super) fn validate_ipc_text_list(
    values: &[String],
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_text_list(values, max_chars, IPC_LIST_MAX_ITEMS)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

pub(super) fn validate_required_ipc_text(
    value: &str,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_required_text(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

pub(super) fn validate_ipc_text(
    value: &str,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_text(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

pub(super) fn validate_ipc_qr_content(
    value: &str,
    field: &str,
    max_chars: usize,
    make_error: fn(String) -> AppError,
) -> Result<(), AppError> {
    input_safety::validate_qr_content(value, max_chars)
        .map_err(|error| ipc_text_error(error, field, make_error))
}

pub(super) fn ipc_text_error(
    error: InputSafetyError,
    field: &str,
    make_error: fn(String) -> AppError,
) -> AppError {
    let reason = match error {
        InputSafetyError::EmptyValue => "value is required".to_string(),
        InputSafetyError::TooLong => "value is too long".to_string(),
        InputSafetyError::ControlCharacters => "control characters are not allowed".to_string(),
        InputSafetyError::TooManyItems => "too many items".to_string(),
    };

    make_error(format!("invalid {field}: {reason}"))
}

/// The supervisor state the OS-facing transactions gate on.
///
/// System proxy and TUN follow the *connected core*, never the persisted mode:
/// applying a proxy nobody is listening behind black-holes every request.
pub(super) async fn supervisor_connection_state(
    state: &AppState,
) -> Result<SupervisorConnectionState, AppError> {
    runtime_manager(state)
        .status()
        .await
        .map(|snapshot| snapshot.state)
        .map_err(runtime_error)
}

pub(super) fn runtime_manager(state: &AppState) -> RuntimeManager<'_> {
    let manager = state.services().runtime(state.supervisor());

    if let Some(seed_dir) = state.core_seed_resource_dir() {
        manager.with_core_seed_resource_dir(seed_dir.to_path_buf())
    } else {
        manager
    }
}

pub(super) fn speedtest_manager(state: &AppState) -> SpeedtestManager {
    state.speedtest_manager()
}

pub(super) fn tun_manager(state: &AppState) -> TunManager {
    TunManager::new(state.elevation_manager().state())
}

pub(super) fn update_manager(state: &AppState) -> UpdateManager<'_> {
    state.services().updates()
}

pub(super) struct TauriHotkeyRegistrar<R: tauri::Runtime> {
    pub(super) app: tauri::AppHandle<R>,
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

    fn register(&self, bindings: &[ShowWindowShortcutBinding]) -> Result<(), HotkeyManagerError> {
        for binding in bindings {
            voya_platform::hotkeys::validate_hotkey_accelerator(&binding.accelerator)?;
            self.app
                .global_shortcut()
                .on_shortcut(
                    binding.accelerator.as_str(),
                    move |app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            toggle_main_window(app);
                        }
                    },
                )
                .map_err(|error| HotkeyManagerError::Register(error.to_string()))?;
        }

        Ok(())
    }
}

pub(super) fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
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

/// Runs blocking OS work off the caller's thread.
///
/// A non-`async` `#[tauri::command]` is dispatched inline on the webview's main
/// thread, and even an `async` one would stall a tokio worker while it forks
/// `pluginkit`/`systemextensionsctl`/`sc.exe`/`networksetup` or waits on an
/// authorization dialog. Every command that reaches such work funnels through
/// here, matching the pattern already used by `list_process_candidates`.
pub(super) async fn run_blocking<T>(
    label: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, AppError>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::State(format!("{label} task failed: {error}")))
}

/// Reads the current TUN status without blocking the caller's thread.
pub(super) async fn tun_status_off_thread(
    state: &AppState,
    config: AppConfig,
) -> Result<TunStatus, AppError> {
    let manager = tun_manager(state);

    run_blocking("TUN status", move || manager.status(&config))
        .await?
        .map_err(tun_error)
}

/// Validates a TUN enable/disable request without blocking the caller's thread.
///
/// The caller applies the decision with `TunManager::apply_enabled` once it
/// holds the config mutation guard, so the blocking probe never runs while the
/// guard's `&mut AppConfig` is borrowed.
pub(super) async fn plan_tun_enabled_off_thread(
    state: &AppState,
    config: AppConfig,
    enabled: bool,
) -> Result<TunStatus, AppError> {
    let manager = tun_manager(state);

    run_blocking("TUN preflight", move || {
        manager.plan_set_enabled(&config, enabled)
    })
    .await?
    .map_err(tun_error)
}

pub(super) fn apply_system_proxy<R>(
    _app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
    force_disable: bool,
) -> Result<SystemProxyStatus, SystemProxyManagerError>
where
    R: tauri::Runtime,
{
    let runtime_config =
        app_runtime_system_proxy_config(config, force_disable, TargetOs::current());
    state
        .system_proxy_manager()
        .apply_config(&runtime_config.config, runtime_config.force_disable)
}

pub(super) fn runtime_proxy_url(
    prefer_proxy: bool,
    proxy_url: Option<String>,
    config: &AppConfig,
) -> Option<String> {
    app_runtime_proxy_url(prefer_proxy, proxy_url, config, TargetOs::current())
}

pub(super) fn runtime_status_response(snapshot: SupervisorSnapshot) -> RuntimeStatusResponse {
    RuntimeStatusResponse {
        state: match snapshot.state {
            SupervisorConnectionState::Disconnected => RuntimeConnectionState::Disconnected,
            SupervisorConnectionState::Connected => RuntimeConnectionState::Connected,
        },
        active_profile_id: snapshot.active_profile_id,
        main_pid: snapshot.main_pid,
        pre_pid: snapshot.pre_pid,
        running_core_type: snapshot
            .running_core_type
            .map(voya_app::contract_map::core_type_to_contract),
    }
}

pub(super) fn system_proxy_status_response(status: SystemProxyStatus) -> SystemProxyStatusResponse {
    SystemProxyStatusResponse {
        requested_mode: voya_app::contract_map::sysproxy_type_to_contract(status.requested_type),
        effective_mode: voya_app::contract_map::sysproxy_type_to_contract(status.effective_type),
        pac_available: status.pac_available,
        proxy: status.proxy,
        exceptions: status.exceptions,
        pac_url: status.pac_url,
    }
}

pub(crate) fn emit_runtime_log<R>(
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

pub(crate) fn emit_core_state<R>(
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

pub(crate) fn emit_statistics_zero<R>(app: &tauri::AppHandle<R>) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::Statistics(crate::ipc::events::StatisticsSnapshot {
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

pub(super) fn emit_speedtest_result<R>(
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

pub(super) fn core_state_event(
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
        running_core_type: snapshot
            .and_then(|snapshot| snapshot.running_core_type)
            .map(voya_app::contract_map::core_type_to_contract),
    }
}

pub(super) fn config_mutation_error(error: ConfigMutationError) -> AppError {
    match error {
        ConfigMutationError::Database(error) => AppError::ConfigSave(error.to_string()),
    }
}

pub(super) fn report_post_commit_error<R>(
    app: &tauri::AppHandle<R>,
    title: &str,
    message: &str,
    level: AppNoticeLevel,
) where
    R: tauri::Runtime,
{
    match level {
        AppNoticeLevel::Info => tracing::info!(title, message, "post-commit operation failed"),
        AppNoticeLevel::Warning => {
            tracing::warn!(title, message, "post-commit operation failed");
        }
        AppNoticeLevel::Error => tracing::error!(title, message, "post-commit operation failed"),
    }

    let log_level = match level {
        AppNoticeLevel::Info => LogLevel::Info,
        AppNoticeLevel::Warning => LogLevel::Warn,
        AppNoticeLevel::Error => LogLevel::Error,
    };
    if let Err(error) = emit_runtime_log(app, log_level, message) {
        tracing::warn!(?error, "failed to emit post-commit runtime log");
    }
    if let Err(error) = AppEvent::Notice(AppNotice {
        level,
        title: title.to_string(),
        message: Some(message.to_string()),
    })
    .emit(app)
    {
        tracing::warn!(?error, "failed to emit post-commit notice");
    }
}

pub(super) fn profile_error(error: ProfileManagerError) -> AppError {
    match error {
        ProfileManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Profile(error.to_string()),
    }
}

pub(super) fn runtime_error(error: RuntimeError) -> AppError {
    let message = error.to_string();
    match error {
        RuntimeError::CoreInfo(CoreInfoError::ExecutableNotFound {
            core_type,
            search_dir: _,
            candidates,
            url,
        }) => AppError::MissingCore(MissingCoreError {
            message: missing_core_error_message(core_type),
            core_type: voya_app::contract_map::core_type_to_contract(core_type),
            search_dir: missing_core_search_dir_label(),
            candidates: missing_core_candidates(&candidates),
            download_url: url.to_string(),
        }),
        _ => AppError::Runtime(message),
    }
}

pub(super) fn missing_core_error_message(core_type: CoreType) -> String {
    format!(
        "core {core_type:?} executable is missing; install or update the core package and try again"
    )
}

pub(super) fn missing_core_search_dir_label() -> String {
    MISSING_CORE_SEARCH_DIR_LABEL.to_string()
}

pub(super) fn missing_core_candidates(candidates: &str) -> Vec<String> {
    candidates
        .split(',')
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(super) fn core_seed_install_result(outcome: CoreSeedCopyOutcome) -> CoreSeedInstallResult {
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
        core_type: voya_app::contract_map::core_type_to_contract(outcome.core_type),
        status,
        installed_files,
    }
}

pub(super) fn core_seed_install_error(error: CoreInfoError) -> AppError {
    AppError::Runtime(error.to_string())
}

pub(super) fn group_error(error: GroupManagerError) -> AppError {
    match error {
        GroupManagerError::Database(error) => AppError::Database(error.to_string()),
        GroupManagerError::Profile(error) => profile_error(error),
        error => AppError::Group(error.to_string()),
    }
}

pub(super) fn subscription_error(error: SubscriptionManagerError) -> AppError {
    match error {
        SubscriptionManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Subscription(error.to_string()),
    }
}

pub(super) fn routing_error(error: RoutingManagerError) -> AppError {
    match error {
        RoutingManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Routing(error.to_string()),
    }
}

pub(super) fn speedtest_error(error: SpeedtestError) -> AppError {
    AppError::Speedtest(error.to_string())
}

pub(super) fn preset_error(error: PresetManagerError) -> AppError {
    match error {
        PresetManagerError::Database(error) => AppError::Database(error.to_string()),
        PresetManagerError::Routing(RoutingManagerError::Database(error)) => {
            AppError::Database(error.to_string())
        }
        error => AppError::Preset(error.to_string()),
    }
}

pub(super) fn qr_error(error: QrCodeError) -> AppError {
    match error {
        QrCodeError::EmptyContent => AppError::Qr("QR content is empty".to_string()),
        QrCodeError::Generate(_) => AppError::Qr("failed to generate QR code".to_string()),
    }
}

pub(super) fn certificate_error(error: CertificateError) -> AppError {
    AppError::Certificate(error.to_string())
}

pub(super) fn export_error(error: ExportManagerError) -> AppError {
    match error {
        ExportManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Export(error.to_string()),
    }
}

pub(super) fn proxy_runtime_error(error: ProxyRuntimeError) -> AppError {
    AppError::ProxyRuntime(error.to_string())
}

pub(super) fn dns_error(error: DnsManagerError) -> AppError {
    match error {
        DnsManagerError::Validation(issues) => AppError::Dns(DnsCommandError {
            message: "DNS settings validation failed".to_string(),
            issues,
        }),
    }
}

pub(super) fn autostart_error(error: AutostartManagerError) -> AppError {
    AppError::Autostart(error.to_string())
}

pub(super) fn hotkey_error(error: HotkeyManagerError) -> AppError {
    AppError::Hotkey(error.to_string())
}

pub(super) fn sysproxy_error(error: SystemProxyManagerError) -> AppError {
    AppError::SysProxy(error.to_string())
}

pub(super) fn tun_error(error: TunManagerError) -> AppError {
    match error {
        TunManagerError::ElevationRequired => AppError::Sudo(error.to_string()),
        TunManagerError::UnsupportedPlatform | TunManagerError::ProviderPathMismatch { .. } => {
            AppError::Tun(error.to_string())
        }
    }
}

pub(super) fn app_updater_state_for_error(error: &tauri_plugin_updater::Error) -> AppUpdaterState {
    match error {
        tauri_plugin_updater::Error::EmptyEndpoints => AppUpdaterState::Unconfigured,
        tauri_plugin_updater::Error::UnsupportedArch
        | tauri_plugin_updater::Error::UnsupportedOs => AppUpdaterState::Unsupported,
        _ => AppUpdaterState::Error,
    }
}

pub(super) fn update_error(error: UpdateManagerError) -> AppError {
    match error {
        UpdateManagerError::Database(error) => AppError::Database(error.to_string()),
        error => AppError::Update(error.to_string()),
    }
}

pub(super) fn elevation_error(error: ElevationError) -> AppError {
    AppError::Sudo(error.to_string())
}
