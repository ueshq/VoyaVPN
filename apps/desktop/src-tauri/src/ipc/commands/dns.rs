use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn load_dns_settings(
    state: tauri::State<'_, AppState>,
) -> Result<DnsSettingsContract, AppError> {
    let config = current_config(&state);

    state
        .services()
        .dns()
        .load_settings(&config.simple_dns_item)
        .await
        .map(dns_to_contract)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn save_dns_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: DnsSettingsContract,
) -> Result<DnsSettingsContract, AppError> {
    let saved = mutate_config(&state, async |unit_of_work, config| {
        let saved = DnsManager::new_in(unit_of_work)
            .save_settings(dns_from_contract(settings))
            .await?;
        config.simple_dns_item = saved.simple_dns_item.clone();
        Ok::<_, AppError>(saved)
    })
    .await?;
    emit_dns_invalidation(&app, "dns-settings-saved");

    Ok(dns_to_contract(saved.value))
}
