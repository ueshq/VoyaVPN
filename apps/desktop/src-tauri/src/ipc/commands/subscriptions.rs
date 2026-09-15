use super::{post_commit::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn list_subscriptions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SubscriptionContract>, AppError> {
    state
        .services()
        .subscriptions()
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
        .subscriptions()
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
    let saved = mutate_config(&state, async |unit_of_work, _config| {
        Ok(SubscriptionManager::new_in(unit_of_work)
            .save_subscription(subscription_from_contract(item))
            .await?)
    })
    .await?;
    emit_subscription_invalidation(&app, "subscription-saved", false, false);

    Ok(subscription_to_contract(saved.value))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_subscriptions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    validate_ipc_text_list(
        &ids,
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Subscription,
    )?;
    let deleted = mutate_config(&state, async |unit_of_work, config| {
        Ok(SubscriptionManager::new_in(unit_of_work)
            .delete_subscriptions(config, &ids)
            .await?)
    })
    .await?;
    emit_subscription_invalidation(&app, "subscriptions-deleted", true, deleted.config_changed);
    core_flow(&app, &state)
        .disconnect_removed_profile(&current_config(&state))
        .await
        .map_err(AppError::from)?;

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
    validate_present_ipc_text(
        subscription_id.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Subscription,
    )?;
    let imported = mutate_config(&state, async |unit_of_work, config| {
        Ok(SubscriptionManager::new_in(unit_of_work)
            .import_profiles_from_text(config, &text, subscription_id.as_deref())
            .await?)
    })
    .await?;
    emit_subscription_invalidation(&app, "profiles-imported", true, imported.config_changed);
    core_flow(&app, &state)
        .disconnect_removed_profile(&current_config(&state))
        .await
        .map_err(AppError::from)?;

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
    validate_present_ipc_text(
        subscription_id.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Subscription,
    )?;
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppErrorSubsystem::Subscription,
    )?;
    let snapshot = current_config(&state);
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
    let updated = mutate_config(&state, async |unit_of_work, config| {
        Ok(SubscriptionManager::new_in(unit_of_work)
            .apply_prepared_subscription_update(config, prepared)
            .await?)
    })
    .await?;
    emit_subscription_invalidation(&app, "subscriptions-updated", true, updated.config_changed);
    core_flow(&app, &state)
        .disconnect_removed_profile(&current_config(&state))
        .await
        .map_err(AppError::from)?;

    Ok(subscription_update_to_contract(updated.value))
}
