use super::{lifecycle::*, support::*, *};
use voya_app::connection_mode::{apply_connection_mode, derive_connection_mode};
use voya_contracts::{ConnectionMode, ConnectionModeStatus};

fn connection_mode_status_from(config: &AppConfig, tun_status: &TunStatus) -> ConnectionModeStatus {
    let (mode, pac_enabled) = derive_connection_mode(config);

    ConnectionModeStatus {
        mode,
        pac_enabled,
        pac_available: matches!(TargetOs::current(), TargetOs::Windows | TargetOs::Macos),
        vpn_available: tun_status.allow_enable_tun || tun_status.enabled,
        process_rules_effective: mode == ConnectionMode::Vpn,
    }
}

async fn build_connection_mode_status(
    state: &AppState,
    config: &AppConfig,
) -> Result<ConnectionModeStatus, AppError> {
    let tun_status = tun_status_off_thread(state, config.clone()).await?;

    Ok(connection_mode_status_from(config, &tun_status))
}

// `async` because the TUN status probe forks OS helpers; the per-app proxy
// dialog polls this, so it must never run on the webview's main thread.
#[tauri::command]
#[specta::specta]
pub async fn connection_mode_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionModeStatus, AppError> {
    let config = current_config(&state)?;

    build_connection_mode_status(&state, &config).await
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
    // Either way the blocking probe runs once, off the runtime, against a
    // snapshot rather than under the mutation guard's `&mut AppConfig`.
    let tun_status = if mode == ConnectionMode::Vpn {
        let status = plan_tun_enabled_off_thread(&state, original.clone(), true).await?;
        TunManager::apply_enabled(mutation.config_mut(), true);
        apply_connection_mode(mutation.config_mut(), mode, pac_enabled);
        status
    } else {
        apply_connection_mode(mutation.config_mut(), mode, pac_enabled);
        tun_status_off_thread(&state, mutation.config().clone()).await?
    };

    let committed = mutation.config().clone();
    let tun_flag_changed = original.tun_mode_item.enable_tun != committed.tun_mode_item.enable_tun;
    commit_system_proxy_mutation(&app, &state, mutation, &original).await?;

    if let Err(error) = emit_tun_changed(&app, &tun_status) {
        report_post_commit_error(
            &app,
            "TUN status refresh failed",
            &format!("{error:?}"),
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

    // The freshly probed status is reused instead of forking the OS helpers a
    // second time for the same click.
    Ok(connection_mode_status_from(&committed, &tun_status))
}
