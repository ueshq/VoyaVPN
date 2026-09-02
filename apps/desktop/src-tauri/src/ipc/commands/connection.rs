use super::{lifecycle::*, support::*, *};
use voya_app::connection_mode::{apply_connection_mode, derive_connection_mode};
use voya_contracts::{ConnectionMode, ConnectionModeStatus};

fn build_connection_mode_status(
    state: &AppState,
    config: &AppConfig,
) -> Result<ConnectionModeStatus, AppError> {
    let (mode, pac_enabled) = derive_connection_mode(config);
    let tun_status = tun_manager(state).status(config).map_err(tun_error)?;

    Ok(ConnectionModeStatus {
        mode,
        pac_enabled,
        pac_available: matches!(TargetOs::current(), TargetOs::Windows | TargetOs::Macos),
        vpn_available: tun_status.allow_enable_tun || tun_status.enabled,
        process_rules_effective: mode == ConnectionMode::Vpn,
    })
}

#[tauri::command]
#[specta::specta]
pub fn connection_mode_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionModeStatus, AppError> {
    let config = current_config(&state)?;
    build_connection_mode_status(&state, &config)
}

/// Switches the app between the three Hiddify-style connection modes by
/// mutating the persisted system proxy + TUN primitives in one transaction.
/// Entering VPN runs the full TUN preflight; only a TUN flag change restarts
/// a connected core, while proxy-only/system-proxy switches re-apply the OS
/// proxy live.
#[tauri::command]
#[specta::specta]
pub async fn set_connection_mode<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    mode: ConnectionMode,
    pac_enabled: Option<bool>,
) -> Result<ConnectionModeStatus, AppError> {
    let target_os = TargetOs::current();
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let (_, current_pac) = derive_connection_mode(&original);
    let pac_enabled = pac_enabled.unwrap_or(current_pac);
    if mode == ConnectionMode::SystemProxy
        && pac_enabled
        && !matches!(target_os, TargetOs::Windows | TargetOs::Macos)
    {
        return Err(sysproxy_error(SystemProxyManagerError::PacUnavailable(
            target_os,
        )));
    }

    // Entering VPN goes through the manager so elevation/provider preflight
    // runs; every other transition only flips already-validated primitives.
    let tun_status = if mode == ConnectionMode::Vpn {
        let status = tun_manager(&state)
            .set_enabled(mutation.config_mut(), true)
            .map_err(tun_error)?;
        apply_connection_mode(mutation.config_mut(), mode, pac_enabled);
        status
    } else {
        apply_connection_mode(mutation.config_mut(), mode, pac_enabled);
        tun_manager(&state)
            .status(mutation.config())
            .map_err(tun_error)?
    };

    let sysproxy_status =
        apply_system_proxy(&app, &state, mutation.config(), false).map_err(sysproxy_error)?;
    let committed = mutation.config().clone();
    let tun_flag_changed = original.tun_mode_item.enable_tun != committed.tun_mode_item.enable_tun;

    if let Err(failure) = commit_with_compensation(commit_config_mutation(mutation), || {
        apply_system_proxy(&app, &state, &original, false).map(|_| ())
    })
    .await
    {
        if let Some(compensation_error) = failure.compensation {
            report_post_commit_error(
                &app,
                "System proxy recovery failed",
                &format!(
                    "The configuration was not saved and restoring the previous system proxy mode failed: {compensation_error}"
                ),
                AppNoticeLevel::Error,
            );
        }
        return Err(failure.commit);
    }

    if let Err(error) = emit_sysproxy_changed(&app, &sysproxy_status) {
        report_post_commit_error(
            &app,
            "System proxy status refresh failed",
            &format!("{error:?}"),
            AppNoticeLevel::Warning,
        );
    }
    if let Err(error) = emit_tun_changed(&app, &tun_status) {
        report_post_commit_error(
            &app,
            "TUN status refresh failed",
            &format!("{error:?}"),
            AppNoticeLevel::Warning,
        );
    }
    if let Err(error) = crate::refresh_tray_menu(&app) {
        report_post_commit_error(
            &app,
            "Tray refresh failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
    if tun_flag_changed {
        if let Err(error) = restart_if_connected_after_config_change(
            &app,
            &state,
            &committed,
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

    build_connection_mode_status(&state, &committed)
}
