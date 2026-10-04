use super::{post_commit::*, *};

#[tauri::command]
#[specta::specta]
pub async fn proxy_list_connections(
    state: tauri::State<'_, AppState>,
) -> Result<ProxyConnectionsSnapshot, AppError> {
    let clash_api = state.supervisor().clash_api_access().await;

    state
        .proxy_runtime()
        .connections(&clash_api)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn proxy_close_connection<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    connection_id: Option<String>,
) -> Result<ProxyConnectionsSnapshot, AppError> {
    let snapshot = voya_app::proxy_runtime::close_connection_use_case(
        state.proxy_runtime(),
        &state.supervisor(),
        connection_id,
    )
    .await?;

    emit_invalidation(
        &app,
        "proxy-connection-closed",
        invalidation::proxy_runtime_scopes(false),
    );

    Ok(snapshot)
}

#[tauri::command]
#[specta::specta]
pub async fn proxy_set_traffic_mode<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    mode: voya_contracts::TrafficMode,
) -> Result<voya_contracts::TrafficModeResponse, AppError> {
    apply_traffic_mode(&app, &state, mode).await
}

/// The traffic-mode change behind both the command and the tray.
pub(crate) async fn apply_traffic_mode<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    mode: voya_contracts::TrafficMode,
) -> Result<voya_contracts::TrafficModeResponse, AppError> {
    let change = state
        .services()
        .change_traffic_mode(
            state.config_mutations(),
            &state.supervisor(),
            state.proxy_runtime(),
            mode,
        )
        .await?;
    // The preference is already committed, including when a live step fails.
    emit_invalidation(
        app,
        "proxy-traffic-mode-changed",
        invalidation::proxy_runtime_scopes(change.config_changed),
    );

    change.applied
}

#[tauri::command]
#[specta::specta]
pub async fn proxy_start_monitor(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ProxyMonitorStatus, AppError> {
    let result = voya_app::proxy_runtime::start_monitor_use_case(
        &state.proxy_monitor_controller(),
        &state.supervisor(),
        std::sync::Arc::new(crate::TauriSinks { app: app.clone() }),
    )
    .await;

    voya_app::proxy_runtime::report_monitor_result(result, |status| {
        emit_proxy_monitor_status(&app, status);
    })
}

#[tauri::command]
#[specta::specta]
pub fn proxy_stop_monitor(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ProxyMonitorStatus, AppError> {
    voya_app::proxy_runtime::report_monitor_result(
        state.proxy_monitor_controller().stop(),
        |status| {
            emit_proxy_monitor_status(&app, status);
        },
    )
}
