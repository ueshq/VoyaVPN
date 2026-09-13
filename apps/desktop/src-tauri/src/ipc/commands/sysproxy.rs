use super::{support::*, *};

// The status is a plan computed from settings; keep it off the async worker
// all the same, because the path preparation touches the filesystem.
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
