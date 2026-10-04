use super::{support::*, *};

/// Trigger the one-time native authorization dialog and, on success, install
/// the passwordless elevation launcher. No admin password is stored.
// `async` because the dialog blocks for as long as the user takes to answer
// it; run inline on a sync command it would freeze the whole window.
#[tauri::command]
#[specta::specta]
pub async fn tun_request_elevation(
    state: tauri::State<'_, AppState>,
) -> Result<TunStatus, AppError> {
    let config = state.config_mutations().current_config();
    let tun = tun_manager(&state);
    let current = tun.status_off_thread(&config).await?;
    if !current.requires_elevation {
        return Ok(current);
    }

    let elevation = state.elevation_manager().clone();
    run_blocking("elevation request", move || elevation.request())
        .await?
        .map_err(AppError::from)?;

    Ok(tun.status_off_thread(&config).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn tun_status(state: tauri::State<'_, AppState>) -> Result<TunStatus, AppError> {
    let config = state.config_mutations().current_config();

    Ok(tun_manager(&state).status_off_thread(&config).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn tun_provider_diagnostics(
    state: tauri::State<'_, AppState>,
) -> Result<TunProviderDiagnostics, AppError> {
    let manager = tun_manager(&state);

    run_blocking("TUN provider diagnostics", move || {
        manager.provider_diagnostics()
    })
    .await?
    .map_err(AppError::from)
}
