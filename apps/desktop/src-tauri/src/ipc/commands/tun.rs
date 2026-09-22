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
    let current = tun_status_off_thread(&state, config.clone()).await?;
    if !current.requires_elevation {
        return Ok(current);
    }

    let elevation = state.elevation_manager().clone();
    run_blocking("elevation request", move || elevation.request())
        .await?
        .map_err(AppError::from)?;

    tun_status_off_thread(&state, config).await
}

#[tauri::command]
#[specta::specta]
pub async fn tun_status(state: tauri::State<'_, AppState>) -> Result<TunStatus, AppError> {
    let config = state.config_mutations().current_config();

    tun_status_off_thread(&state, config).await
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
    let planned = state
        .config_mutations()
        .mutate(async |_unit_of_work, config| -> Result<_, AppError> {
            // The preflight probe forks OS helpers, so it runs off the runtime
            // against a snapshot; only the validated flag change happens under the
            // guard.
            let status = plan_tun_enabled_off_thread(&state, config.clone(), enabled).await?;
            TunManager::apply_enabled(config, enabled);
            Ok::<_, AppError>(status)
        })
        .await?;
    let status = planned.value;
    let config = planned.config;
    if let Err(error) = emit_tun_changed(&app, &status) {
        report_post_commit_error(
            &app,
            NoticeCode::TunStatusRefreshFailed,
            &format!("{error:?}"),
            AppNoticeLevel::Warning,
        );
    }
    // `enable_tun` is committed, and the settings bundle mirrors it; without
    // this a stale bundle would rewrite the flag back on the next Save-all.
    emit_connection_mode_invalidation(&app, "tun-enabled-changed");
    restart_after_config_change(&app, &state, &config, ConfigChange::TUN).await;

    Ok(status)
}
