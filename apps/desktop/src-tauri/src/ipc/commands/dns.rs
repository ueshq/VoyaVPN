use voya_app::dns::{normalize_simple_dns, validated_settings};

use super::{post_commit::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn load_dns_settings(
    state: tauri::State<'_, AppState>,
) -> Result<DnsSettingsContract, AppError> {
    let config = current_config(&state);

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
    mutate_config(&state, async |_unit_of_work, config| {
        config.simple_dns_item = saved.clone();
        Ok::<_, AppError>(())
    })
    .await?;
    emit_dns_invalidation(&app, "dns-settings-saved");

    Ok(simple_dns_to_contract(saved))
}
