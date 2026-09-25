use super::{post_commit::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub fn load_ui_preferences(
    state: tauri::State<'_, AppState>,
) -> Result<AppearanceSettings, AppError> {
    let config = state.config_mutations().current_config();

    Ok(voya_app::settings::save::settings_from_app_config(&config).appearance)
}

#[tauri::command]
#[specta::specta]
pub fn load_app_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, AppError> {
    Ok(voya_app::settings::save::settings_from_app_config(
        &state.config_mutations().current_config(),
    ))
}

/// Thin adapter over `voya_app::settings`.
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
    settings: AppSettings,
) -> Result<AppSettings, AppError> {
    let settings_language_before = state
        .config_mutations()
        .current_config()
        .appearance
        .language
        .clone();
    let outcome = voya_app::settings::save_app_settings(
        state.config_mutations(),
        &AutostartManager::new(),
        &settings,
    )
    .await
    .map_err(AppError::from)?;

    // The tray is a native menu built from `appearance.language`, so it is
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
        emit_invalidation(
            &app,
            "app-settings-saved",
            invalidation::settings_bundle_scopes(),
        );
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
        .settings_apply_status(&state.config_mutations().current_config())
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn apply_pending_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<voya_contracts::SettingsApplyStatus, AppError> {
    let captured = state.config_mutations().current_config();
    let flow = core_flow(&app, &state);
    let result = flow.apply_pending_settings(&captured).await;
    emit_invalidation(
        &app,
        "settings-applied",
        invalidation::settings_bundle_scopes(),
    );
    result.map_err(AppError::from)?;
    flow.settings_apply_status(&state.config_mutations().current_config())
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub fn generate_qr_code(content: String) -> Result<QrCodeImage, AppError> {
    map_ipc_input(
        input_safety::validate_qr_content(&content, IPC_QR_CONTENT_MAX_CHARS),
        "QR content",
        AppErrorSubsystem::Qr,
    )?;

    voya_app::qr::generate_svg(&content).map_err(AppError::from)
}

// `async` because capturing every display is a multi-hundred-millisecond
// blocking operation that would otherwise freeze the window.
#[tauri::command]
#[specta::specta]
pub async fn scan_screen_qr(window: tauri::WebviewWindow) -> Result<QrScanResult, AppError> {
    screen_qr::scan(window).await
}

/// Decodes the QR codes in a picture the user picked. The webview decodes the
/// file and sends grey pixels (base64, one byte each), so no image decoder
/// ships in either the bundle or the binary; locating codes in a large
/// picture takes long enough to keep off the async workers.
#[tauri::command]
#[specta::specta]
pub async fn decode_qr_image(
    width: u32,
    height: u32,
    luma_base64: String,
) -> Result<QrScanResult, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&luma_base64, IPC_QR_IMAGE_MAX_BASE64_CHARS),
        "QR image data",
        AppErrorSubsystem::Qr,
    )?;

    run_blocking("QR image decode", move || {
        voya_app::qr::decode_image(width, height, &luma_base64)
    })
    .await?
    .map_err(AppError::from)
}
