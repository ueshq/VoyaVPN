use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn import_config_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    selection: ConfigTemplateSelection,
    prefer_proxy: bool,
    proxy_url: Option<String>,
) -> Result<ConfigTemplateImportResult, AppError> {
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppErrorSubsystem::Preset,
    )?;
    if let ConfigTemplateSelection::Custom { sources } = &selection {
        for (label, value) in [
            ("Geo source URL", sources.geo_source_url.as_deref()),
            ("SRS source URL", sources.srs_source_url.as_deref()),
            (
                "routing template source URL",
                sources.route_rules_template_source_url.as_deref(),
            ),
        ] {
            validate_optional_ipc_text(
                value,
                label,
                IPC_PROXY_URL_MAX_CHARS,
                AppErrorSubsystem::Preset,
            )?;
        }
    }
    let snapshot = current_config(&state);
    let proxy_url = runtime_proxy_url(prefer_proxy, proxy_url, &snapshot);
    let prepared = state
        .services()
        .presets()
        .prepare_config_template_import(
            selection,
            &ConfigTemplateImportOptions {
                prefer_proxy,
                proxy_url,
            },
        )
        .await
        .map_err(AppError::from)?;
    let imported = mutate_config(&state, async |unit_of_work, config| {
        Ok(PresetManager::new_in(unit_of_work)
            .apply_prepared_config_template_import(config, prepared)
            .await?)
    })
    .await?;
    emit_preset_invalidation(&app, "config-template-imported");
    restart_after_config_change(
        &app,
        &state,
        &imported.config,
        ConfigChange::CONFIG_TEMPLATE,
    )
    .await;

    Ok(imported.value)
}
