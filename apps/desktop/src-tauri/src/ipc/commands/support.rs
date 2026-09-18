use std::sync::{
    atomic::{AtomicBool, Ordering},
    LazyLock,
};

use voya_app::{
    log_batch::{LogBatcher, LOG_BATCH_CAPACITY, LOG_BATCH_WINDOW},
    supervisor::ClashApiAccess,
};

use super::*;

/// Every Logs-panel line, whoever wrote it, so batching keeps their order.
static LOG_LINES: LazyLock<LogBatcher<LogLineEvent>> =
    LazyLock::new(|| LogBatcher::new(LOG_BATCH_CAPACITY));
/// Set by whichever line starts the flusher. A flag rather than a `OnceLock`:
/// the `tracing` layer queues lines too, so starting the flusher may re-enter
/// here, which `get_or_init` would deadlock on.
static LOG_FLUSHER_STARTED: AtomicBool = AtomicBool::new(false);

pub(super) fn current_config(state: &AppState) -> AppConfig {
    state.config_mutations().current_config()
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

pub(super) fn tun_manager(state: &AppState) -> TunManager {
    // A fresh manager per command is fine — it is a handle, not a resource —
    // but the PlugInKit registration memo has to outlive it, or every status
    // read forks `pluginkit` again.
    TunManager::new(state.elevation_manager().state())
        .with_provider_registration_cache(state.provider_registration_cache())
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

pub(super) fn runtime_proxy_url(
    prefer_proxy: bool,
    proxy_url: Option<String>,
    config: &AppConfig,
) -> Option<String> {
    app_runtime_proxy_url(prefer_proxy, proxy_url, config, TargetOs::current())
}

/// Emits one typed event, failing the way a command reports its own errors.
pub(crate) fn emit_event<R, E>(app: &tauri::AppHandle<R>, event: E) -> Result<(), AppError>
where
    R: tauri::Runtime,
    E: Event + serde::Serialize + Clone,
{
    event
        .emit(app)
        .map_err(|error| AppError::internal(AppErrorSubsystem::App, error.to_string()))
}

/// Emits one typed event whose loss must not fail the work it reports.
pub(crate) fn emit_or_warn<R, E>(app: &tauri::AppHandle<R>, event: E, what: &'static str)
where
    R: tauri::Runtime,
    E: Event + serde::Serialize + Clone,
{
    if let Err(error) = event.emit(app) {
        tracing::warn!(?error, "failed to emit {what}");
    }
}

/// A log line the app itself wrote.
///
/// `code` is resolved against the locale files by the Logs panel; `detail` is
/// the untranslated diagnostic printed after it. Core process output does not
/// come through here — it keeps its own words via [`emit_core_log`].
pub(crate) fn emit_app_log<R>(
    app: &tauri::AppHandle<R>,
    level: LogLevel,
    code: LogCode,
    detail: Option<&str>,
) where
    R: tauri::Runtime,
{
    queue_log_line(
        app,
        level,
        LogLineBody::App {
            code,
            detail: detail.map(ToString::to_string),
        },
    )
}

/// One line of the core process's own output, passed through verbatim.
pub(crate) fn emit_core_log<R>(app: &tauri::AppHandle<R>, level: LogLevel, line: String)
where
    R: tauri::Runtime,
{
    queue_log_line(app, level, LogLineBody::Core { line });
}

/// Queues one Logs-panel line; the flusher delivers queued lines as a single
/// `LogLines` event per [`LOG_BATCH_WINDOW`]. Never blocks and never fails,
/// so the core's pipe reader and the `tracing` layer can call it directly.
pub(crate) fn queue_log_line<R>(app: &tauri::AppHandle<R>, level: LogLevel, body: LogLineBody)
where
    R: tauri::Runtime,
{
    if !LOG_FLUSHER_STARTED.swap(true, Ordering::AcqRel) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            LOG_LINES
                .run(LOG_BATCH_WINDOW, |lines| {
                    // Not traced on failure: the `tracing` layer would queue
                    // that warning here again, once per window.
                    let _ = TransientStreamEvent::LogLines(lines).emit(&app);
                })
                .await;
        });
    }
    LOG_LINES.push(LogLineEvent {
        id: next_log_line_id(),
        level,
        body,
    });
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
    emit_event(
        app,
        TransientStreamEvent::CoreState(runtime_status_event(state, active_profile_id, snapshot)),
    )
}

pub(crate) fn emit_statistics_zero<R>(app: &tauri::AppHandle<R>) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    emit_event(
        app,
        TransientStreamEvent::Statistics(statistics_snapshot_to_contract(
            voya_app::statistics::StatisticsSnapshot::zero(),
        )),
    )
}

pub(super) fn emit_speedtest_result<R>(
    app: &tauri::AppHandle<R>,
    result: &SpeedtestResult,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    emit_event(app, TransientStreamEvent::SpeedtestResult(result.clone()))
}

/// A change that was already committed, whose follow-up work failed.
///
/// `code` names the message; `detail` is the untranslated error behind it, so
/// it goes to the log line and the toast's second line but never into the
/// title the user reads.
pub(super) fn report_post_commit_error<R>(
    app: &tauri::AppHandle<R>,
    code: NoticeCode,
    detail: &str,
    level: AppNoticeLevel,
) where
    R: tauri::Runtime,
{
    match level {
        AppNoticeLevel::Info => tracing::info!(?code, detail, "post-commit operation failed"),
        AppNoticeLevel::Warning => {
            tracing::warn!(?code, detail, "post-commit operation failed");
        }
        AppNoticeLevel::Error => tracing::error!(?code, detail, "post-commit operation failed"),
    }

    let log_level = match level {
        AppNoticeLevel::Info => LogLevel::Info,
        AppNoticeLevel::Warning => LogLevel::Warn,
        AppNoticeLevel::Error => LogLevel::Error,
    };
    emit_app_log(app, log_level, LogCode::PostCommitFailed, Some(detail));
    emit_or_warn(
        app,
        AppEvent::Notice(AppNotice {
            level,
            code,
            detail: Some(detail.to_string()),
        }),
        "post-commit notice",
    );
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
