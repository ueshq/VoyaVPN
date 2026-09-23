use super::{post_commit::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn run_speedtest<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    request: voya_contracts::SpeedtestRequest,
) -> Result<SpeedtestRunResult, AppError> {
    let voya_contracts::SpeedtestTarget::Profiles {
        profile_ids: index_ids,
    } = request.target;
    map_ipc_input(
        input_safety::validate_text_list(&index_ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "node id",
        AppErrorSubsystem::Speedtest,
    )?;
    let config = state.config_mutations().current_config();
    let manager = state.speedtest_manager();
    let emit_app = app.clone();
    let result = state
        .services()
        .run_speedtest(&manager, &config, index_ids, move |results| {
            if let Err(error) = emit_speedtest_results(&emit_app, results) {
                tracing::warn!(?error, "failed to emit speedtest results");
            }
        })
        .await
        .map_err(AppError::from)?;

    emit_invalidation(
        &app,
        "speedtest-updated",
        invalidation::profile_scopes(false),
    );

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_speedtest<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<SpeedtestStatus, AppError> {
    if state.speedtest_manager().cancel() {
        emit_app_log(
            &app,
            LogLevel::Info,
            LogCode::SpeedtestCancellationRequested,
            None,
        );
    }

    Ok(state.speedtest_manager().status())
}

#[tauri::command]
#[specta::specta]
pub fn speedtest_status(state: tauri::State<'_, AppState>) -> Result<SpeedtestStatus, AppError> {
    Ok(state.speedtest_manager().status())
}

/// Looks up the exit address of the running connection through its local proxy.
#[tauri::command]
#[specta::specta]
pub async fn check_connection_ip(
    state: tauri::State<'_, AppState>,
) -> Result<voya_contracts::ConnectionIpResult, AppError> {
    let config = state.config_mutations().current_config();
    let snapshot = state.supervisor().status().await.map_err(AppError::from)?;
    let exit = voya_app::connection_ip::check_connection_ip(&config, &snapshot)
        .await
        .map_err(AppError::from)?;

    Ok(exit)
}
