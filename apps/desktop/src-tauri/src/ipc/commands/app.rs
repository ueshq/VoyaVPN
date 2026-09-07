use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub fn load_ui_preferences(
    state: tauri::State<'_, AppState>,
) -> Result<AppearanceSettings, AppError> {
    let config = current_config(&state)?;

    Ok(voya_app::settings_save::settings_from_app_config(&config).appearance)
}

#[tauri::command]
#[specta::specta]
pub fn load_app_settings(state: tauri::State<'_, AppState>) -> Result<AppSettingsV1, AppError> {
    Ok(voya_app::settings_save::settings_from_app_config(
        &current_config(&state)?,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn save_app_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: AppSettingsV1,
) -> Result<AppSettingsV1, AppError> {
    validate_app_settings(&settings).map_err(|error| AppError::State(error.to_string()))?;

    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let target = state
        .services()
        .config_from_settings(&settings, &original)
        .map_err(|error| AppError::State(error.to_string()))?;
    let runtime_changed = saved_config_requires_runtime_restart(&original, &target);
    let system_proxy_changed = original.system_proxy_item != target.system_proxy_item;
    let side_effects = TauriSettingsSideEffects {
        autostart: AutostartManager::new(),
        hotkeys: HotkeyManager::new(std::sync::Arc::new(TauriHotkeyRegistrar {
            app: app.clone(),
        })),
    };
    let applied_side_effects = match apply_settings_side_effects(&side_effects, &original, &target)
    {
        Ok(applied) => applied,
        Err(failure) => {
            tracing::error!(stage = ?failure.stage, error = ?failure.source, "settings side effect failed");
            log_settings_compensation_errors(&failure.compensation_errors);
            return Err(failure.source);
        }
    };

    *mutation.config_mut() = target.clone();
    if let Err(error) = commit_config_mutation(mutation).await {
        let compensation_errors =
            compensate_settings_side_effects(&side_effects, &original, applied_side_effects);
        log_settings_compensation_errors(&compensation_errors);
        return Err(error);
    }

    let apply_result = match settings_runtime_action(runtime_changed, system_proxy_changed) {
        SettingsRuntimeAction::Restart => {
            restart_if_connected_after_config_change(&app, &state, &target, "Settings saved").await
        }
        SettingsRuntimeAction::ReapplySystemProxy => {
            apply_system_proxy_if_connected_after_config_change(&app, &state, &target).await
        }
        SettingsRuntimeAction::None => Ok(()),
    };

    if let Err(error) = apply_result {
        report_post_commit_error(
            &app,
            "Settings saved; runtime update failed",
            &format!("{error:?}"),
            AppNoticeLevel::Warning,
        );
    }

    if original != target {
        if let Err(error) = emit_settings_bundle_invalidation(&app, "app-settings-saved") {
            tracing::error!(
                ?error,
                "failed to broadcast committed settings invalidation"
            );
        }
    }

    Ok(voya_app::settings_save::settings_from_app_config(&target))
}

#[derive(Clone)]
struct TauriSettingsSideEffects {
    autostart: AutostartManager,
    hotkeys: HotkeyManager,
}

impl SettingsSideEffectAdapter for TauriSettingsSideEffects {
    type Error = AppError;

    fn apply_autostart(&self, config: &AppConfig) -> Result<(), Self::Error> {
        let mut config = config.clone();
        let enabled = config.gui_item.auto_run;
        self.autostart
            .set_enabled(&mut config, enabled)
            .map(|_| ())
            .map_err(autostart_error)
    }

    fn apply_hotkeys(&self, config: &AppConfig) -> Result<(), Self::Error> {
        self.hotkeys
            .register_from_config(config)
            .map(|_| ())
            .map_err(hotkey_error)
    }
}

fn log_settings_compensation_errors(errors: &[AppError]) {
    for error in errors {
        tracing::error!(?error, "failed to compensate settings side effect");
    }
}

#[tauri::command]
#[specta::specta]
pub fn generate_qr_code(content: String) -> Result<QrCodeImage, AppError> {
    validate_ipc_qr_content(
        &content,
        "QR content",
        IPC_QR_CONTENT_MAX_CHARS,
        AppError::Qr,
    )?;

    QrCodeManager.generate_svg(&content).map_err(qr_error)
}

// `async` because capturing every display is a multi-hundred-millisecond
// blocking operation that would otherwise freeze the window.
#[tauri::command]
#[specta::specta]
pub async fn scan_screen_qr() -> Result<QrScanResult, AppError> {
    run_blocking("screen QR scan", || QrCodeManager.scan_screen()).await
}

#[tauri::command]
#[specta::specta]
pub async fn fetch_certificate(
    request: CertificateFetchRequest,
) -> Result<CertificateFetchResult, AppError> {
    validate_required_ipc_text(
        &request.address,
        "certificate address",
        IPC_NAME_MAX_CHARS,
        AppError::Certificate,
    )?;
    if let Some(server_name) = request.server_name.as_deref() {
        validate_ipc_text(
            server_name,
            "certificate server name",
            IPC_NAME_MAX_CHARS,
            AppError::Certificate,
        )?;
    }

    fetch_certificate_impl(request)
        .await
        .map_err(certificate_error)
}

#[tauri::command]
#[specta::specta]
pub fn calculate_certificate_sha256(pem: String) -> Result<Vec<String>, AppError> {
    validate_required_ipc_text(
        &pem,
        "certificate PEM",
        IPC_QR_CONTENT_MAX_CHARS * 8,
        AppError::Certificate,
    )?;

    calculate_certificate_sha256_impl(&pem).map_err(certificate_error)
}

pub(crate) fn register_show_window_shortcut_for_config<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: &AppConfig,
) -> Result<HotkeyStatus, AppError> {
    let registrar = std::sync::Arc::new(TauriHotkeyRegistrar { app: app.clone() });

    HotkeyManager::new(registrar)
        .register_from_config(config)
        .map_err(hotkey_error)
}
