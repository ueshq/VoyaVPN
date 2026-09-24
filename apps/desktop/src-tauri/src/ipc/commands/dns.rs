use voya_app::dns::{normalize_simple_dns, validated_settings};

use super::{post_commit::*, *};

#[tauri::command]
#[specta::specta]
pub async fn load_dns_settings(
    state: tauri::State<'_, AppState>,
) -> Result<DnsSettingsContract, AppError> {
    let config = state.config_mutations().current_config();

    Ok(simple_dns_to_contract(normalize_simple_dns(
        config.simple_dns_item,
    )))
}

#[tauri::command]
#[specta::specta]
pub async fn save_dns_settings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    settings: DnsSettingsContract,
) -> Result<DnsSettingsContract, AppError> {
    let saved = validated_settings(simple_dns_from_contract(settings))?;
    state
        .config_mutations()
        .mutate(async |_unit_of_work, config| -> Result<_, AppError> {
            config.simple_dns_item = saved.clone();
            Ok::<_, AppError>(())
        })
        .await?;
    emit_invalidation(&app, "dns-settings-saved", invalidation::dns_scopes());

    Ok(simple_dns_to_contract(saved))
}

#[tauri::command]
#[specta::specta]
pub fn get_default_dns_settings() -> Result<voya_contracts::DnsSettings, AppError> {
    Ok(voya_app::contract_map::default_dns_settings())
}
