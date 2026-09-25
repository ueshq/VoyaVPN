use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock,
    },
    time::{SystemTime, UNIX_EPOCH},
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

/// Maps one `input_safety` check onto the shared `input_text_error` issue.
///
/// Rejected IPC text is a *validation* failure addressed to the argument that
/// carried it; this is the one remap, so the shell never picks an error variant
/// per call site.
pub(super) fn map_ipc_input<T>(
    result: input_safety::Result<T>,
    field: &str,
    subsystem: AppErrorSubsystem,
) -> Result<T, AppError> {
    result.map_err(|error| input_text_error(&error, field, subsystem))
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
    state.services().runtime(
        state.supervisor(),
        state.core_seed_resource_dir().map(Path::to_path_buf),
    )
}

pub(super) fn tun_manager(state: &AppState) -> TunManager {
    // A fresh manager per command is fine — it is a handle, not a resource —
    // but the PlugInKit registration memo has to outlive it, or every status
    // read forks `pluginkit` again.
    state.services().tun_manager(
        state.elevation_manager().state(),
        Some(state.provider_registration_cache()),
    )
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
    E: Emit,
{
    event
        .emit(app)
        .map_err(|error| AppError::internal(AppErrorSubsystem::App, error.to_string()))
}

/// Emits one typed event whose loss must not fail the work it reports.
pub(crate) fn emit_or_warn<R, E>(app: &tauri::AppHandle<R>, event: E, what: &'static str)
where
    R: tauri::Runtime,
    E: Emit,
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

/// Starts or pauses delivering Logs-panel lines to the webview; see
/// [`voya_app::log_batch`]. The rotating file log is unaffected.
pub(crate) fn stream_log_lines(streaming: bool) {
    LOG_LINES.set_streaming(streaming);
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
        logged_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0),
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
        TransientStreamEvent::Statistics(voya_app::statistics::zero_statistics_snapshot()),
    )
}

pub(super) fn emit_speedtest_results<R>(
    app: &tauri::AppHandle<R>,
    results: Vec<SpeedtestResult>,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    emit_event(app, TransientStreamEvent::SpeedtestResults(results))
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

#[cfg(not(feature = "mac-app-store"))]
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
