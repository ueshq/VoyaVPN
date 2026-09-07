use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn connect_active_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = current_config(&state)?;
    emit_runtime_log(&app, LogLevel::Info, "Connecting active profile")?;
    emit_core_state(
        &app,
        CoreState::Connecting,
        Some(config.index_id.clone()).filter(|value| !value.is_empty()),
        None,
    )?;

    match runtime_manager(&state).connect(&config).await {
        Ok(snapshot) => {
            emit_runtime_log(&app, LogLevel::Info, "Core supervisor started")?;
            emit_core_state(&app, CoreState::Connected, None, Some(&snapshot))?;
            match apply_system_proxy(&app, &state, &config, false) {
                Ok(status) => emit_sysproxy_changed(&app, &status)?,
                Err(error) => emit_runtime_log(
                    &app,
                    LogLevel::Warn,
                    &format!("System proxy apply failed: {error}"),
                )?,
            }
            emit_current_tun_status(&app, &state).await?;
            Ok(runtime_status_response(snapshot))
        }
        Err(error) => {
            let message = error.to_string();
            emit_runtime_log(&app, LogLevel::Error, &message)?;
            emit_core_state(&app, CoreState::Disconnected, None, None)?;
            restore_system_proxy_after_native_tun_failure(
                &app,
                &state,
                &config,
                "connect failure",
            )?;
            Err(runtime_error(error))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn disconnect_core<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    emit_runtime_log(&app, LogLevel::Info, "Disconnecting core supervisor")?;
    emit_core_state(&app, CoreState::Disconnecting, None, None)?;
    let runtime = runtime_manager(&state);

    match runtime.disconnect().await {
        Ok(snapshot) => {
            match restore_system_proxy(&app, &state) {
                Ok(status) => emit_sysproxy_changed(&app, &status)?,
                Err(error) => emit_runtime_log(
                    &app,
                    LogLevel::Warn,
                    &format!("System proxy restore failed: {error:?}"),
                )?,
            }
            emit_runtime_log(&app, LogLevel::Info, "Core supervisor stopped")?;
            emit_core_state(&app, CoreState::Disconnected, None, Some(&snapshot))?;
            emit_current_tun_status(&app, &state).await?;
            emit_statistics_zero(&app)?;
            Ok(runtime_status_response(snapshot))
        }
        Err(error) => {
            let message = error.to_string();
            emit_runtime_log(&app, LogLevel::Error, &message)?;
            match runtime.status().await {
                Ok(snapshot) => {
                    emit_core_state(
                        &app,
                        core_state_from_snapshot(&snapshot),
                        None,
                        Some(&snapshot),
                    )?;
                }
                Err(status_error) => {
                    emit_runtime_log(
                        &app,
                        LogLevel::Warn,
                        &format!("Runtime status refresh after disconnect failure failed: {status_error}"),
                    )?;
                    emit_core_state(&app, CoreState::Connected, None, None)?;
                }
            }
            Err(runtime_error(error))
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn restart_core<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = current_config(&state)?;
    emit_runtime_log(&app, LogLevel::Info, "Restarting active profile")?;
    emit_core_state(
        &app,
        CoreState::Connecting,
        Some(config.index_id.clone()).filter(|value| !value.is_empty()),
        None,
    )?;

    match runtime_manager(&state).restart(&config).await {
        Ok(snapshot) => {
            emit_runtime_log(&app, LogLevel::Info, "Core supervisor restarted")?;
            emit_core_state(&app, CoreState::Connected, None, Some(&snapshot))?;
            match apply_system_proxy(&app, &state, &config, false) {
                Ok(status) => emit_sysproxy_changed(&app, &status)?,
                Err(error) => emit_runtime_log(
                    &app,
                    LogLevel::Warn,
                    &format!("System proxy apply failed: {error}"),
                )?,
            }
            emit_current_tun_status(&app, &state).await?;
            Ok(runtime_status_response(snapshot))
        }
        Err(error) => {
            let message = error.to_string();
            emit_runtime_log(&app, LogLevel::Error, &message)?;
            emit_core_state(&app, CoreState::Disconnected, None, None)?;
            restore_system_proxy_after_native_tun_failure(
                &app,
                &state,
                &config,
                "restart failure",
            )?;
            Err(runtime_error(error))
        }
    }
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
        .map_err(runtime_error)
}
