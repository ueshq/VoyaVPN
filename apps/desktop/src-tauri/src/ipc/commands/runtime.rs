use super::{support::*, *};

// Every command here is a thin adapter over `voya_app::core_flow`, which owns
// the emit order and the failure/compensation policy for all of them.

#[tauri::command]
#[specta::specta]
pub async fn connect_active_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = current_config(&state)?;
    let flow = core_flow(&app, &state);

    flow.connect(&config)
        .await
        .map(runtime_status_response)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn disconnect_core<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = current_config(&state)?;
    let flow = core_flow(&app, &state);

    flow.disconnect(&config)
        .await
        .map(runtime_status_response)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn restart_core<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = current_config(&state)?;
    let flow = core_flow(&app, &state);

    flow.restart(&config)
        .await
        .map(runtime_status_response)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_status(
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    runtime_manager(&state)
        .status()
        .await
        .map(runtime_status_response)
        .map_err(AppError::from)
}
