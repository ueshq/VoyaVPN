use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub fn load_ui_preferences(
    state: tauri::State<'_, AppState>,
) -> Result<AppearanceSettings, AppError> {
    let config = current_config(&state);

    Ok(voya_app::settings_save::settings_from_app_config(&config).appearance)
}

#[tauri::command]
#[specta::specta]
pub fn load_app_settings(state: tauri::State<'_, AppState>) -> Result<AppSettingsV1, AppError> {
    Ok(voya_app::settings_save::settings_from_app_config(
        &current_config(&state),
    ))
}

/// Thin adapter over `voya_app::settings_flow`.
///
/// Validation, the pre-commit OS side effects, the commit and both rollback
/// paths are the transaction in voya-app, where they are unit-tested; the only
/// things left here are turning `AppError`s back out of it and dispatching the
/// runtime action it selected.
#[tauri::command]
#[specta::specta]
pub async fn save_app_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: AppSettingsV1,
) -> Result<AppSettingsV1, AppError> {
    let settings_language_before = current_config(&state).ui_item.current_language.clone();
    let side_effects = TauriSettingsSideEffects {
        autostart: AutostartManager::new(),
    };
    let outcome = voya_app::settings_flow::save_app_settings(
        state.config_mutations(),
        &side_effects,
        &settings,
    )
    .await
    .map_err(AppError::from)?;

    // The tray is a native menu built from `ui_item.current_language`, so it is
    // the one surface a language change cannot reach on its own: the webview
    // re-renders, the tray keeps whatever words it was built with.
    if outcome.settings.appearance.language != settings_language_before {
        if let Err(error) = crate::refresh_tray_menu(&app) {
            report_post_commit_error(
                &app,
                NoticeCode::TrayRefreshFailed,
                &error.to_string(),
                AppNoticeLevel::Warning,
            );
        }
    }

    if outcome.changed {
        emit_settings_bundle_invalidation(&app, "app-settings-saved");
    }

    Ok(outcome.settings)
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings_apply_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<voya_contracts::SettingsApplyStatus, AppError> {
    core_flow(&app, &state)
        .settings_apply_status(&current_config(&state))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn apply_pending_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<voya_contracts::SettingsApplyStatus, AppError> {
    let captured = current_config(&state);
    let flow = core_flow(&app, &state);
    let result = flow.apply_pending_settings(&captured).await;
    emit_settings_bundle_invalidation(&app, "settings-applied");
    result.map_err(AppError::from)?;
    flow.settings_apply_status(&current_config(&state))
        .await
        .map_err(AppError::from)
}

#[derive(Clone)]
struct TauriSettingsSideEffects {
    autostart: AutostartManager,
}

impl SettingsSideEffectAdapter for TauriSettingsSideEffects {
    type Error = AppError;

    fn apply_autostart(&self, config: &AppConfig) -> Result<(), Self::Error> {
        let mut config = config.clone();
        let enabled = config.gui_item.auto_run;
        self.autostart
            .set_enabled(&mut config, enabled)
            .map(|_| ())
            .map_err(AppError::from)
    }
}

#[tauri::command]
#[specta::specta]
pub fn generate_qr_code(content: String) -> Result<QrCodeImage, AppError> {
    validate_ipc_qr_content(
        &content,
        "QR content",
        IPC_QR_CONTENT_MAX_CHARS,
        AppErrorSubsystem::Qr,
    )?;

    QrCodeManager.generate_svg(&content).map_err(AppError::from)
}

// `async` because capturing every display is a multi-hundred-millisecond
// blocking operation that would otherwise freeze the window.
#[tauri::command]
#[specta::specta]
pub async fn scan_screen_qr(window: tauri::WebviewWindow) -> Result<QrScanResult, AppError> {
    screen_qr::scan(window).await
}
