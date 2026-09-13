use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn proxy_list_connections(
    state: tauri::State<'_, AppState>,
) -> Result<ProxyConnectionsSnapshot, AppError> {
    let clash_api = current_clash_api_access(&state).await;

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
    validate_present_ipc_text(
        connection_id.as_deref(),
        "proxy connection id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::ProxyRuntime,
    )?;
    let clash_api = current_clash_api_access(&state).await;
    let snapshot = state
        .proxy_runtime()
        .close_connection(&clash_api, connection_id.as_deref())
        .await
        .map_err(AppError::from)?;

    emit_proxy_runtime_invalidation(&app, "proxy-connection-closed", false);

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
    let mode = traffic_mode_from_contract(mode);
    let snapshot = state.supervisor().status().await.map_err(AppError::from)?;
    let outcome = state
        .proxy_runtime()
        .change_traffic_mode(state.config_mutations(), &snapshot, mode)
        .await
        .map_err(AppError::from)?;
    // The preference is already committed, including when a live step fails.
    state
        .services()
        .acknowledge_traffic_mode(&snapshot, &outcome);
    emit_proxy_runtime_invalidation(app, "proxy-traffic-mode-changed", outcome.config_changed);
    outcome.runtime_result.map_err(AppError::from)?;

    Ok(voya_contracts::TrafficModeResponse {
        mode: traffic_mode_to_contract(outcome.mode),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn proxy_start_monitor(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ProxyMonitorStatus, AppError> {
    let clash_api = current_clash_api_access(&state).await;

    match state.proxy_monitor_controller().start(
        &clash_api,
        std::sync::Arc::new(crate::TauriProxyRuntimeEventSink { app: app.clone() }),
    ) {
        Ok(status) => {
            emit_proxy_monitor_status(&app, &status);
            Ok(status)
        }
        Err(error) => {
            let message = error.to_string();
            emit_proxy_monitor_status(&app, &ProxyMonitorStatus::failed(message));
            Err(AppError::from(error))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn proxy_stop_monitor(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ProxyMonitorStatus, AppError> {
    match state.proxy_monitor_controller().stop() {
        Ok(status) => {
            emit_proxy_monitor_status(&app, &status);
            Ok(status)
        }
        Err(error) => {
            let message = error.to_string();
            emit_proxy_monitor_status(&app, &ProxyMonitorStatus::failed(message));
            Err(AppError::from(error))
        }
    }
}
