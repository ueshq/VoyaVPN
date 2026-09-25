use voya_app::subscriptions::{
    delete_subscriptions_use_case, import_profiles_use_case, save_subscription_use_case,
    update_subscriptions_use_case,
};

use super::{post_commit::*, *};

#[tauri::command]
#[specta::specta]
pub async fn list_subscriptions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SubscriptionContract>, AppError> {
    state
        .services()
        .list_subscriptions()
        .await
        .map(|items| items.into_iter().map(subscription_to_contract).collect())
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_subscription_metadata(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SubscriptionMetadataContract>, AppError> {
    state
        .services()
        .list_subscription_metadata()
        .await
        .map(|items| {
            items
                .into_iter()
                .map(subscription_metadata_to_contract)
                .collect()
        })
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn save_subscription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    item: SubscriptionContract,
) -> Result<SubscriptionContract, AppError> {
    let saved = save_subscription_use_case(state.config_mutations(), item).await?;
    emit_invalidation(
        &app,
        "subscription-saved",
        invalidation::subscription_scopes(false, false),
    );

    Ok(saved.value)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_subscriptions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    let deleted = delete_subscriptions_use_case(state.config_mutations(), ids).await?;
    emit_then_disconnect_removed(&app, &state, |app| {
        emit_invalidation(
            app,
            "subscriptions-deleted",
            invalidation::subscription_scopes(true, deleted.config_changed),
        )
    })
    .await?;

    Ok(deleted.value)
}

#[tauri::command]
#[specta::specta]
pub async fn import_profiles_from_text<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    text: String,
    subscription_id: Option<String>,
) -> Result<ImportProfilesContract, AppError> {
    let imported =
        import_profiles_use_case(state.config_mutations(), text, subscription_id).await?;
    emit_then_disconnect_removed(&app, &state, |app| {
        emit_invalidation(
            app,
            "profiles-imported",
            invalidation::subscription_scopes(true, imported.config_changed),
        )
    })
    .await?;

    Ok(imported.value)
}

#[tauri::command]
#[specta::specta]
pub async fn update_subscriptions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    subscription_id: Option<String>,
    prefer_proxy: bool,
    proxy_url: Option<String>,
) -> Result<SubscriptionUpdateContract, AppError> {
    let update = update_subscriptions_use_case(
        state.services(),
        state.config_mutations(),
        subscription_id,
        prefer_proxy,
        proxy_url,
        TargetOs::current(),
    )
    .await?;
    if let Some(config_changed) = update.config_changed {
        emit_then_disconnect_removed(&app, &state, |app| {
            emit_invalidation(
                app,
                "subscriptions-updated",
                invalidation::subscription_scopes(true, config_changed),
            )
        })
        .await?;
    }

    Ok(update.result)
}

#[tauri::command]
#[specta::specta]
pub fn preview_import_profiles(text: String) -> Result<voya_contracts::ImportPreview, AppError> {
    SubscriptionManager::preview_import(&text).map_err(AppError::from)
}
