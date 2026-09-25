use super::{connection_mode::*, post_commit::*, support::*, *};
use voya_contracts::{ConnectionMode, ConnectionModeStatus};

// `async` because the TUN status probe forks OS helpers; the per-app proxy
// dialog polls this, so it must never run on the webview's main thread.
#[tauri::command]
#[specta::specta]
pub async fn connection_mode_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionModeStatus, AppError> {
    let config = state.config_mutations().current_config();
    let tun_status = tun_status_off_thread(&state, config.clone()).await?;

    Ok(voya_app::connection_mode::connection_mode_status(
        &config,
        &tun_status,
    ))
}

/// Switches the app between system proxy and TUN mode.
///
/// The transaction itself is `voya_app::connection_mode`: the mode is always
/// persisted, but the machine's proxy settings are only rewritten while the
/// supervisor reports `Connected` — a mode pointed at a port nothing is
/// listening on would black-hole every request. Only a TUN flag change needs a
/// running core to be restarted.
#[tauri::command]
#[specta::specta]
pub async fn set_connection_mode<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    mode: ConnectionMode,
) -> Result<ConnectionModeStatus, AppError> {
    // System proxy and TUN follow the connected core, never the persisted
    // mode: applying a proxy nobody is listening behind black-holes traffic.
    let connected = runtime_manager(&state)
        .status()
        .await
        .map_err(AppError::from)?
        .state;
    let outcome = connection_mode_manager(&app, &state)
        .set_connection_mode(state.config_mutations(), mode, connected)
        .await
        .map_err(AppError::from)?;

    // The mode is persisted in the same fields the settings bundle mirrors
    // (`network.tun.enabled`, `network.systemProxy.mode`), so both caches move.
    emit_invalidation(
        &app,
        "connection-mode-changed",
        invalidation::connection_mode_scopes(),
    );

    if outcome.tun_flag_changed {
        restart_after_config_change(&app, &state, &outcome.config, ConfigChange::CONNECTION_MODE)
            .await;
    }

    Ok(outcome.status)
}
