use super::{connection_mode::*, lifecycle::*, support::*, *};
use voya_contracts::{ConnectionMode, ConnectionModeStatus};

// `async` because the TUN status probe forks OS helpers; the per-app proxy
// dialog polls this, so it must never run on the webview's main thread.
#[tauri::command]
#[specta::specta]
pub async fn connection_mode_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionModeStatus, AppError> {
    let config = current_config(&state)?;
    let tun_status = tun_status_off_thread(&state, config.clone()).await?;

    Ok(voya_app::connection_mode::connection_mode_status(
        &config,
        &tun_status,
        TargetOs::current(),
    ))
}

/// Switches the app between the three Hiddify-style connection modes.
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
    pac_enabled: Option<bool>,
) -> Result<ConnectionModeStatus, AppError> {
    let connected = supervisor_connection_state(&state).await?;
    let outcome = connection_mode_manager(&app, &state)
        .set_connection_mode(state.config_mutations(), mode, pac_enabled, connected)
        .await
        .map_err(connection_mode_error)?;

    if outcome.tun_flag_changed {
        if let Err(error) = restart_if_connected_after_config_change(
            &app,
            &state,
            &outcome.config,
            "Connection mode changed",
        )
        .await
        {
            report_post_commit_error(
                &app,
                "Connection mode saved; core restart failed",
                &format!("{error:?}"),
                AppNoticeLevel::Warning,
            );
        }
    }

    Ok(outcome.status)
}
