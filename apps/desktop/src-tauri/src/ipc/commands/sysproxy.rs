use super::{lifecycle::*, support::*, *};

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

#[tauri::command]
#[specta::specta]
pub async fn set_system_proxy_mode<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    mode: ContractSysProxyType,
) -> Result<SystemProxyStatusResponse, AppError> {
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let target_os = TargetOs::current();
    if mode == ContractSysProxyType::Pac
        && !matches!(target_os, TargetOs::Windows | TargetOs::Macos)
    {
        return Err(sysproxy_error(SystemProxyManagerError::PacUnavailable(
            target_os,
        )));
    }

    mutation.config_mut().system_proxy_item.sys_proxy_type =
        voya_app::contract_map::sysproxy_type_from_contract(mode);
    let status = commit_system_proxy_mutation(&app, &state, mutation, &original).await?;

    Ok(system_proxy_status_response(status))
}
