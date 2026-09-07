use voya_app::supervisor::ClashApiAccess;

use super::*;

pub(super) fn current_config(state: &AppState) -> Result<AppConfig, AppError> {
    Ok(state.config_mutations().current_config())
}

/// How to reach the Clash API of the core that is actually running, if any.
///
/// The generated main config decides both the port and the bearer token it
/// demands, so the supervisor snapshot is the only authority for either. An
/// empty value means no core is connected and there is nothing to dial.
///
/// The token is never logged or returned over IPC: it stays inside this
/// process, is redacted in `Debug`, and only reaches the Clash clients.
pub(super) async fn current_clash_api_access(state: &AppState) -> ClashApiAccess {
    state
        .supervisor()
        .status()
        .await
        .ok()
        .as_ref()
        .map(SupervisorSnapshot::clash_api_access)
        .unwrap_or_default()
}

/// Runs one manager call inside a configuration mutation and commits it.
///
/// The begin / clone / `split()` / compare / commit scaffold lives in
/// `voya_app::config_mutation` where it is unit-tested; commands keep only the
/// manager call and the cache invalidation that follows the commit, which is
/// the one thing that needs an `AppHandle`.
pub(super) async fn mutate_config<T, F>(
    state: &AppState,
    operation: F,
) -> Result<CommittedMutation<T>, AppError>
where
    F: AsyncFnOnce(&UnitOfWork, &mut AppConfig) -> Result<T, AppError>,
{
    state.config_mutations().mutate(operation).await
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
        AppErrorSubsystem::Export,
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
        .map_err(AppError::from)
}

// Argument guards. Rejected IPC text is a *validation* failure addressed to the
// argument that carried it, so each guard names its subsystem and the shared
// `input_text_error` mapper builds the issue — the shell no longer picks an
// error variant per call site.

pub(super) fn validate_present_ipc_text(
    value: Option<&str>,
    field: &str,
    max_chars: usize,
    subsystem: AppErrorSubsystem,
) -> Result<(), AppError> {
    input_safety::validate_present_text(value, max_chars)
        .map_err(|error| input_text_error(&error, field, subsystem))
}

pub(super) fn validate_optional_ipc_text(
    value: Option<&str>,
    field: &str,
    max_chars: usize,
    subsystem: AppErrorSubsystem,
) -> Result<(), AppError> {
    input_safety::validate_optional_text(value, max_chars)
        .map_err(|error| input_text_error(&error, field, subsystem))
}

pub(super) fn validate_ipc_text_list(
    values: &[String],
    field: &str,
    max_chars: usize,
    subsystem: AppErrorSubsystem,
) -> Result<(), AppError> {
    input_safety::validate_text_list(values, max_chars, IPC_LIST_MAX_ITEMS)
        .map_err(|error| input_text_error(&error, field, subsystem))
}

pub(super) fn validate_required_ipc_text(
    value: &str,
    field: &str,
    max_chars: usize,
    subsystem: AppErrorSubsystem,
) -> Result<(), AppError> {
    input_safety::validate_required_text(value, max_chars)
        .map_err(|error| input_text_error(&error, field, subsystem))
}

pub(super) fn validate_ipc_text(
    value: &str,
    field: &str,
    max_chars: usize,
    subsystem: AppErrorSubsystem,
) -> Result<(), AppError> {
    input_safety::validate_text(value, max_chars)
        .map_err(|error| input_text_error(&error, field, subsystem))
}

pub(super) fn validate_ipc_qr_content(
    value: &str,
    field: &str,
    max_chars: usize,
    subsystem: AppErrorSubsystem,
) -> Result<(), AppError> {
    input_safety::validate_qr_content(value, max_chars)
        .map_err(|error| input_text_error(&error, field, subsystem))
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
        .map_err(AppError::from)
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
    // A fresh manager per command is fine — it is a handle, not a resource —
    // but the PlugInKit registration memo has to outlive it, or every status
    // read forks `pluginkit` again.
    TunManager::new(state.elevation_manager().state())
        .with_provider_registration_cache(state.provider_registration_cache())
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
        .map_err(|error| {
            AppError::internal(
                AppErrorSubsystem::App,
                format!("{label} task failed: {error}"),
            )
        })
}

/// Reads the current TUN status without blocking the caller's thread.
pub(super) async fn tun_status_off_thread(
    state: &AppState,
    config: AppConfig,
) -> Result<TunStatus, AppError> {
    let manager = tun_manager(state);

    run_blocking("TUN status", move || manager.status(&config))
        .await?
        .map_err(AppError::from)
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
    .map_err(AppError::from)
}

pub(super) fn apply_system_proxy<R>(
    _app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
    force_disable: bool,
) -> Result<SystemProxyStatus, AppError>
where
    R: tauri::Runtime,
{
    let runtime_config =
        app_runtime_system_proxy_config(config, force_disable, TargetOs::current());
    state
        .system_proxy_manager()
        .apply_config(&runtime_config.config, runtime_config.force_disable)
        .map_err(AppError::from)
}

pub(super) fn runtime_proxy_url(
    prefer_proxy: bool,
    proxy_url: Option<String>,
    config: &AppConfig,
) -> Option<String> {
    app_runtime_proxy_url(prefer_proxy, proxy_url, config, TargetOs::current())
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
    .map_err(|error| AppError::internal(AppErrorSubsystem::App, error.to_string()))
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
        .map_err(|error| AppError::internal(AppErrorSubsystem::App, error.to_string()))
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
    .map_err(|error| AppError::internal(AppErrorSubsystem::App, error.to_string()))
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
        .map_err(|error| AppError::internal(AppErrorSubsystem::App, error.to_string()))
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

pub(super) fn app_updater_state_for_error(error: &tauri_plugin_updater::Error) -> AppUpdaterState {
    match error {
        tauri_plugin_updater::Error::EmptyEndpoints => AppUpdaterState::Unconfigured,
        tauri_plugin_updater::Error::UnsupportedArch
        | tauri_plugin_updater::Error::UnsupportedOs => AppUpdaterState::Unsupported,
        _ => AppUpdaterState::Error,
    }
}

/// Installing a packaged core seed reports its own failures.
///
/// Not a `From` impl: `CoreInfoError` belongs to voya-platform, so the
/// conversion is a function in `voya_app::contract_map::errors` that the
/// subsystem is passed into.
pub(super) fn core_seed_install_error(error: CoreInfoError) -> AppError {
    core_info_error(&error, AppErrorSubsystem::Runtime)
}
