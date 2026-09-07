use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn load_dns_settings(
    state: tauri::State<'_, AppState>,
) -> Result<DnsSettingsContract, AppError> {
    let config = current_config(&state)?;

    state
        .services()
        .dns()
        .load_settings(&config.simple_dns_item)
        .await
        .map(dns_to_contract)
        .map_err(dns_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_dns_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: DnsSettingsContract,
) -> Result<DnsSettingsContract, AppError> {
    let mut mutation = begin_config_mutation(&state).await?;
    let saved = mutation
        .dns()
        .save_settings(dns_from_contract(settings))
        .await
        .map_err(dns_error)?;
    mutation.config_mut().simple_dns_item = saved.simple_dns_item.clone();
    let config = commit_config_mutation(mutation).await?;
    emit_dns_invalidation(&app, "dns-settings-saved")?;
    restart_after_config_change(&app, &state, &config, ConfigChange::DNS).await;

    Ok(dns_to_contract(saved))
}
