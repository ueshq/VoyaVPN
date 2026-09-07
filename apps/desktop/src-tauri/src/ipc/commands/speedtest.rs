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
        AppError::Speedtest,
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
        // The cancellation already happened; a failed log emit must not turn a
        // successful command into an error.
        if let Err(error) =
            emit_runtime_log(&app, LogLevel::Info, "Speedtest cancellation requested")
        {
            tracing::warn!(?error, "failed to emit speedtest cancellation log");
        }
    }

    speedtest_manager(&state).status().map_err(speedtest_error)
}

#[tauri::command]
#[specta::specta]
pub fn speedtest_status(state: tauri::State<'_, AppState>) -> Result<SpeedtestStatus, AppError> {
    speedtest_manager(&state).status().map_err(speedtest_error)
}
