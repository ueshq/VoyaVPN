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
        hotkeys: HotkeyManager::new(std::sync::Arc::new(TauriHotkeyRegistrar {
            app: app.clone(),
        })),
    };
    let outcome = voya_app::settings_flow::save_app_settings(
        state.config_mutations(),
        &side_effects,
        &settings,
    )
    .await
    .map_err(AppError::from)?;

    apply_settings_runtime_action(&app, &state, &outcome).await;

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

/// Bring the running core in line with the settings that were just committed.
///
/// Everything here happens after the commit, so a failure is a warning notice
/// rather than a command error: the settings *are* saved.
async fn apply_settings_runtime_action<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    outcome: &SettingsSaveOutcome,
) {
    match outcome.runtime_action {
        SettingsRuntimeAction::Restart => {
            restart_after_config_change(app, state, &outcome.config, ConfigChange::APP_SETTINGS)
                .await;
        }
        SettingsRuntimeAction::ReapplySystemProxy => {
            if let Err(error) = core_flow(app, state)
                .reapply_system_proxy_if_connected(&outcome.config)
                .await
            {
                report_post_commit_error(
                    app,
                    NoticeCode::SettingsSavedRuntimeUpdateFailed,
                    &format!("{:?}", AppError::from(error)),
                    AppNoticeLevel::Warning,
                );
            }
        }
        SettingsRuntimeAction::None => {}
    }
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
            .map_err(AppError::from)
    }

    fn apply_hotkeys(&self, config: &AppConfig) -> Result<(), Self::Error> {
        self.hotkeys
            .register_from_config(config)
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
        AppErrorSubsystem::Certificate,
    )?;
    if let Some(server_name) = request.server_name.as_deref() {
        validate_ipc_text(
            server_name,
            "certificate server name",
            IPC_NAME_MAX_CHARS,
            AppErrorSubsystem::Certificate,
        )?;
    }

    fetch_certificate_impl(request)
        .await
        .map_err(|error| certificate_error(&error))
}

#[tauri::command]
#[specta::specta]
pub fn calculate_certificate_sha256(pem: String) -> Result<Vec<String>, AppError> {
    validate_required_ipc_text(
        &pem,
        "certificate PEM",
        IPC_QR_CONTENT_MAX_CHARS * 8,
        AppErrorSubsystem::Certificate,
    )?;

    calculate_certificate_sha256_impl(&pem).map_err(|error| certificate_error(&error))
}

pub(crate) fn register_show_window_shortcut_for_config<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: &AppConfig,
) -> Result<HotkeyStatus, AppError> {
    let registrar = std::sync::Arc::new(TauriHotkeyRegistrar { app: app.clone() });

    HotkeyManager::new(registrar)
        .register_from_config(config)
        .map_err(AppError::from)
}
