use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn run_speedtest<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    request: voya_contracts::SpeedTestRequest,
) -> Result<SpeedtestRunResult, AppError> {
    let index_ids = match request.target {
        voya_contracts::SpeedTestTarget::All => Vec::new(),
        voya_contracts::SpeedTestTarget::Profiles { profile_ids } => profile_ids,
    };
    validate_ipc_text_list(
        &index_ids,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Speedtest,
    )?;
    let config = current_config(&state)?;
    let manager = speedtest_manager(&state);
    let emit_app = app.clone();
    let result = state
        .services()
        .run_speedtest(&manager, &config, request.kind, index_ids, move |result| {
            if let Err(error) = emit_speedtest_result(&emit_app, &result) {
                tracing::warn!(?error, "failed to emit speedtest result");
            }
        })
        .await
        .map_err(AppError::from)?;

    emit_profile_invalidation(&app, "speedtest-updated", false);

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_speedtest<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<SpeedtestStatus, AppError> {
    let cancelled = speedtest_manager(&state).cancel().map_err(AppError::from)?;
    if cancelled {
        // The cancellation already happened; a failed log emit must not turn a
        // successful command into an error.
        if let Err(error) = emit_app_log(
            &app,
            LogLevel::Info,
            LogCode::SpeedtestCancellationRequested,
            None,
        ) {
            tracing::warn!(?error, "failed to emit speedtest cancellation log");
        }
    }

    speedtest_manager(&state).status().map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub fn speedtest_status(state: tauri::State<'_, AppState>) -> Result<SpeedtestStatus, AppError> {
    speedtest_manager(&state).status().map_err(AppError::from)
}
