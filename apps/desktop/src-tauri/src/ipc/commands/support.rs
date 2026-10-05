use std::sync::{
    atomic::{AtomicBool, Ordering},
    LazyLock,
};

use voya_app::log_batch::{LogBatcher, LOG_BATCH_CAPACITY, LOG_BATCH_WINDOW};

use super::*;

/// Every Logs-panel line, whoever wrote it, so batching keeps their order.
static LOG_LINES: LazyLock<LogBatcher<LogLineEvent>> =
    LazyLock::new(|| LogBatcher::new(LOG_BATCH_CAPACITY));
/// Set by whichever line starts the flusher. A flag rather than a `OnceLock`:
/// the `tracing` layer queues lines too, so starting the flusher may re-enter
/// here, which `get_or_init` would deadlock on.
static LOG_FLUSHER_STARTED: AtomicBool = AtomicBool::new(false);

pub(crate) fn runtime_manager(state: &AppState) -> RuntimeManager<'_> {
    state.services().runtime(
        state.supervisor(),
        Some(state.core_seed_resource_dir().to_path_buf()),
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
/// here.
pub(super) async fn run_blocking<T>(
    label: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, AppError>
where
    T: Send + 'static,
{
    Ok(voya_app::blocking::run_blocking(label, work).await?)
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
    LOG_LINES.push(voya_app::logging::new_log_line(level, body));
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

/// A notice raised after a change was committed — usually follow-up work that
/// failed. `voya_app::post_commit::notice_effects` decides what is logged and
/// what the toast carries; this only puts both on their channels.
pub(super) fn report_post_commit_error<R>(
    app: &tauri::AppHandle<R>,
    code: NoticeCode,
    detail: &str,
    level: AppNoticeLevel,
) where
    R: tauri::Runtime,
{
    let effects = voya_app::post_commit::notice_effects(level, code, detail);
    // The app-authored line below is what the Logs panel shows; these records
    // are the file log's copy of the same failure.
    match effects.log_level {
        Some(LogLevel::Error) => {
            tracing::error!(target: voya_app::logging::FILE_ONLY_TARGET, code = ?effects.notice.code, detail, "post-commit operation failed");
        }
        Some(LogLevel::Warn) => {
            tracing::warn!(target: voya_app::logging::FILE_ONLY_TARGET, code = ?effects.notice.code, detail, "post-commit operation failed");
        }
        Some(_) => {
            tracing::info!(code = ?effects.notice.code, detail, "post-commit operation failed");
        }
        None => tracing::info!(code = ?effects.notice.code, "post-commit notice"),
    }
    if let Some(log_level) = effects.log_level {
        emit_app_log(
            app,
            log_level,
            voya_app::post_commit::POST_COMMIT_LOG_CODE,
            Some(detail),
        );
    }
    emit_or_warn(app, AppEvent::Notice(effects.notice), "post-commit notice");
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
