use super::{post_commit::*, support::*, *};

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

#[tauri::command]
#[specta::specta]
pub async fn set_tun_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<TunStatus, AppError> {
    let planned =
        set_tun_enabled_use_case(state.config_mutations(), &tun_manager(&state), enabled).await?;
    let status = planned.value;
    let config = planned.config;
    if let Err(error) = emit_tun_changed(&app, &status) {
        report_post_commit_error(
            &app,
            NoticeCode::TunStatusRefreshFailed,
            &error.message,
            AppNoticeLevel::Warning,
        );
    }
    // `tun.enabled` is committed, and the settings bundle mirrors it; without
    // this a stale bundle would rewrite the flag back on the next Save-all.
    emit_invalidation(
        &app,
        "tun-enabled-changed",
        invalidation::connection_mode_scopes(),
    );
    restart_after_config_change(&app, &state, &config, ConfigChange::TUN).await;

    Ok(status)
}
