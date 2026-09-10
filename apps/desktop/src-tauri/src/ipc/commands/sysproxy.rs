use super::{connection_mode::*, lifecycle::*, support::*, *};

// Keep SystemConfiguration reads off the async worker; macOS never runs scripts.
#[tauri::command]
#[specta::specta]
pub async fn system_proxy_status(
    state: tauri::State<'_, AppState>,
) -> Result<SystemProxyStatusResponse, AppError> {
    let config = current_config(&state);
    let manager = state.system_proxy_manager();

    run_blocking("system proxy status", move || {
        manager
            .runtime_status(&config)
            .map(system_proxy_status_response)
    })
    .await?
    .map_err(AppError::from)
}

/// Changes only the system proxy flavor. Same transaction as
/// `set_connection_mode`: the mode is persisted always, the machine is only
/// touched while a core is running.
#[tauri::command]
#[specta::specta]
pub async fn set_system_proxy_mode<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    mode: SystemProxyType,
) -> Result<SystemProxyStatusResponse, AppError> {
    let connected = supervisor_connection_state(&state).await?;

    let status = connection_mode_manager(&app, &state)
        .set_system_proxy_mode(
            state.config_mutations(),
            voya_app::contract_map::sysproxy_type_from_contract(mode),
            connected,
        )
        .await
        .map_err(AppError::from)?;
    // `sys_proxy_type` is committed, and the settings bundle mirrors it.
    emit_connection_mode_invalidation(&app, "system-proxy-mode-changed");

    Ok(system_proxy_status_response(status))
}

/// Explicit recheck may retire a legacy dirty marker once local proxies are gone.
#[tauri::command]
#[specta::specta]
pub async fn recheck_system_proxy<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<SystemProxyStatusResponse, AppError> {
    let config = current_config(&state);
    let manager = state.system_proxy_manager();
    let status = run_blocking("recheck system proxy", move || {
        manager.recheck_manual_proxy(&config)
    })
    .await?
    .map_err(AppError::from)?;
    emit_sysproxy_changed(&app, &status)?;
    Ok(system_proxy_status_response(status))
}

/// Fixed destination; renderer input cannot turn this into an arbitrary opener.
#[tauri::command]
#[specta::specta]
pub async fn open_network_settings(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    let manager = state.system_proxy_manager();
    run_blocking("open network settings", move || {
        manager.open_network_settings()
    })
    .await?
    .map_err(AppError::from)
}
