use super::{connection_mode::*, lifecycle::*, support::*, *};

// `async` because reading the OS proxy state shells out (`networksetup` on
// macOS, `gsettings` on Linux) once per network service.
#[tauri::command]
#[specta::specta]
pub async fn system_proxy_status(
    state: tauri::State<'_, AppState>,
) -> Result<SystemProxyStatusResponse, AppError> {
    let config = current_config(&state)?;
    let runtime_config = app_runtime_system_proxy_config(&config, false, TargetOs::current());
    let manager = state.system_proxy_manager();

    run_blocking("system proxy status", move || {
        manager
            .status_with_force_disable(&runtime_config.config, runtime_config.force_disable)
            .map(system_proxy_status_response)
    })
    .await?
    .map_err(sysproxy_error)
}

/// Changes only the system proxy flavor. Same transaction as
/// `set_connection_mode`: the mode is persisted always, the machine is only
/// touched while a core is running.
#[tauri::command]
#[specta::specta]
pub async fn set_system_proxy_mode<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    mode: ContractSysProxyType,
) -> Result<SystemProxyStatusResponse, AppError> {
    let connected = supervisor_connection_state(&state).await?;

    let status = connection_mode_manager(&app, &state)
        .set_system_proxy_mode(
            state.config_mutations(),
            voya_app::contract_map::sysproxy_type_from_contract(mode),
            connected,
        )
        .await
        .map_err(connection_mode_error)?;
    // `sys_proxy_type` is committed, and the settings bundle mirrors it.
    emit_connection_mode_invalidation(&app, "system-proxy-mode-changed");

    Ok(system_proxy_status_response(status))
}
