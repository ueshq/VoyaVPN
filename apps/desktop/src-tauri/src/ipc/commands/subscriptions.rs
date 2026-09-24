use super::{post_commit::*, support::*, *};

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
    // Saving a subscription only writes the subscription row: it imports no
    // profiles and never touches the persisted app config, so neither the
    // profile list nor the settings bundle needs refreshing.
    let saved = state
        .config_mutations()
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .save_subscription(subscription_from_contract(item))
                .await?)
        })
        .await?;
    emit_invalidation(
        &app,
        "subscription-saved",
        invalidation::subscription_scopes(false, false),
    );

    Ok(subscription_to_contract(saved.value))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_subscriptions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    map_ipc_input(
        input_safety::validate_text_list(&ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "subscription id",
        AppErrorSubsystem::Subscription,
    )?;
    let deleted = state
        .config_mutations()
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .delete_subscriptions(config, &ids)
                .await?)
        })
        .await?;
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
    map_ipc_input(
        input_safety::validate_present_text(subscription_id.as_deref(), IPC_ID_MAX_CHARS),
        "subscription id",
        AppErrorSubsystem::Subscription,
    )?;
    let imported = state
        .config_mutations()
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .import_profiles_from_text(config, &text, subscription_id.as_deref())
                .await?)
        })
        .await?;
    emit_then_disconnect_removed(&app, &state, |app| {
        emit_invalidation(
            app,
            "profiles-imported",
            invalidation::subscription_scopes(true, imported.config_changed),
        )
    })
    .await?;

    Ok(import_profiles_to_contract(imported.value))
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
    map_ipc_input(
        input_safety::validate_present_text(subscription_id.as_deref(), IPC_ID_MAX_CHARS),
        "subscription id",
        AppErrorSubsystem::Subscription,
    )?;
    map_ipc_input(
        input_safety::validate_optional_text(proxy_url.as_deref(), IPC_PROXY_URL_MAX_CHARS),
        "proxy URL",
        AppErrorSubsystem::Subscription,
    )?;
    let snapshot = state.config_mutations().current_config();
    let proxy_url = runtime_proxy_url(prefer_proxy, proxy_url, &snapshot);
    let prepared = state
        .services()
        .subscriptions()
        .prepare_subscription_update(
            subscription_id.as_deref(),
            prefer_proxy,
            proxy_url.as_deref(),
        )
        .await
        .map_err(AppError::from)?;
    if !prepared.has_imports() {
        return Ok(subscription_update_to_contract(prepared.into_result()));
    }
    let updated = state
        .config_mutations()
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .apply_prepared_subscription_update(config, prepared)
                .await?)
        })
        .await?;
    emit_then_disconnect_removed(&app, &state, |app| {
        emit_invalidation(
            app,
            "subscriptions-updated",
            invalidation::subscription_scopes(true, updated.config_changed),
        )
    })
    .await?;

    Ok(subscription_update_to_contract(updated.value))
}

#[tauri::command]
#[specta::specta]
pub fn preview_import_profiles(text: String) -> Result<voya_contracts::ImportPreview, AppError> {
    SubscriptionManager::preview_import(&text).map_err(AppError::from)
}
