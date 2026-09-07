use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn proxy_list_groups(
    state: tauri::State<'_, AppState>,
) -> Result<ProxyGroupsSnapshot, AppError> {
    let config = current_config(&state)?;
    let clash_api = current_clash_api_access(&state).await;

    state
        .proxy_runtime()
        .groups(&config, &clash_api)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn proxy_test_delay(
    state: tauri::State<'_, AppState>,
    node_names: Vec<String>,
) -> Result<Vec<ProxyDelayTestResult>, AppError> {
    validate_ipc_text_list(
        &node_names,
        "proxy node name",
        IPC_NAME_MAX_CHARS,
        AppErrorSubsystem::ProxyRuntime,
    )?;
    let config = current_config(&state)?;
    let clash_api = current_clash_api_access(&state).await;

    state
        .proxy_runtime()
        .test_delay(&config, &clash_api, node_names)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn proxy_select_node<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    group_name: String,
    node_name: String,
) -> Result<ProxyGroupsSnapshot, AppError> {
    validate_required_ipc_text(
        &group_name,
        "proxy group name",
        IPC_NAME_MAX_CHARS,
        AppErrorSubsystem::ProxyRuntime,
    )?;
    validate_required_ipc_text(
        &node_name,
        "proxy node name",
        IPC_NAME_MAX_CHARS,
        AppErrorSubsystem::ProxyRuntime,
    )?;
    let config = current_config(&state)?;
    let clash_api = current_clash_api_access(&state).await;
    let snapshot = state
        .proxy_runtime()
        .select_node(&config, &clash_api, &group_name, &node_name)
        .await
        .map_err(AppError::from)?;

    emit_proxy_runtime_invalidation(&app, "proxy-node-selected", false);

    Ok(snapshot)
}

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
    let mode = match mode {
        voya_contracts::TrafficMode::Rule => TrafficMode::Rule,
        voya_contracts::TrafficMode::Global => TrafficMode::Global,
        voya_contracts::TrafficMode::Direct => TrafficMode::Direct,
        voya_contracts::TrafficMode::Unchanged => TrafficMode::Unchanged,
    };
    let committed = mutate_config(&state, async |_unit_of_work, config| {
        let changed = config.proxy_ui_item.traffic_mode != mode;
        if changed {
            config.proxy_ui_item.traffic_mode = mode;
        }
        Ok::<_, AppError>(changed)
    })
    .await?;
    let changed = committed.value;
    let config = committed.config;
    if changed && mode != TrafficMode::Unchanged {
        let clash_api = current_clash_api_access(&state).await;
        if let Err(error) = state
            .proxy_runtime()
            .set_traffic_mode(&clash_api, mode)
            .await
        {
            report_post_commit_error(
                &app,
                "Proxy mode saved; runtime update failed",
                &error.to_string(),
                AppNoticeLevel::Warning,
            );
        }
    }

    emit_proxy_runtime_invalidation(&app, "proxy-traffic-mode-changed", changed);

    Ok(voya_contracts::TrafficModeResponse {
        mode: match config.proxy_ui_item.traffic_mode {
            TrafficMode::Rule => voya_contracts::TrafficMode::Rule,
            TrafficMode::Global => voya_contracts::TrafficMode::Global,
            TrafficMode::Direct => voya_contracts::TrafficMode::Direct,
            TrafficMode::Unchanged => voya_contracts::TrafficMode::Unchanged,
        },
    })
}

#[tauri::command]
#[specta::specta]
pub async fn proxy_reload_config<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), AppError> {
    validate_optional_ipc_text(
        path.as_deref(),
        "proxy runtime config path",
        IPC_PATH_MAX_CHARS,
        AppErrorSubsystem::ProxyRuntime,
    )?;
    let clash_api = current_clash_api_access(&state).await;

    state
        .proxy_runtime()
        .reload_config(&clash_api, path.as_deref())
        .await
        .map_err(AppError::from)?;
    emit_proxy_runtime_invalidation(&app, "proxy-config-reloaded", false);

    Ok(())
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
