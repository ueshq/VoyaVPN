use super::{lifecycle::*, support::*, *};

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
        .map_err(subscription_error)
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
        .map_err(subscription_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_subscription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    item: SubscriptionContract,
) -> Result<SubscriptionContract, AppError> {
    let mutation = begin_config_mutation(&state).await?;
    // Saving a subscription only writes the subscription row; it never touches
    // the persisted app config, so no `active-profile` invalidation is needed.
    let saved = mutation
        .subscriptions()
        .save_subscription(subscription_from_contract(item))
        .await
        .map_err(subscription_error)?;
    commit_config_mutation(mutation).await?;
    emit_subscription_invalidation(&app, "subscription-saved", false, false)?;

    Ok(subscription_to_contract(saved))
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
        AppError::Subscription,
    )?;
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let deleted = {
        let (unit_of_work, config) = mutation.split();
        SubscriptionManager::new_in(unit_of_work)
            .delete_subscriptions(config, &ids)
            .await
            .map_err(subscription_error)?
    };
    let config_changed = original != *mutation.config();
    commit_config_mutation(mutation).await?;
    emit_subscription_invalidation(&app, "subscriptions-deleted", true, config_changed)?;

    Ok(deleted)
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
        AppError::Subscription,
    )?;
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let result = {
        let (unit_of_work, config) = mutation.split();
        SubscriptionManager::new_in(unit_of_work)
            .import_profiles_from_text(config, &text, subscription_id.as_deref())
            .await
            .map_err(subscription_error)?
    };
    let config_changed = original != *mutation.config();
    commit_config_mutation(mutation).await?;
    emit_subscription_invalidation(&app, "profiles-imported", true, config_changed)?;

    Ok(import_profiles_to_contract(result))
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
        AppError::Subscription,
    )?;
    validate_optional_ipc_text(
        proxy_url.as_deref(),
        "proxy URL",
        IPC_PROXY_URL_MAX_CHARS,
        AppError::Subscription,
    )?;
    let snapshot = current_config(&state)?;
    let proxy_url = runtime_proxy_url(prefer_proxy, proxy_url, &snapshot);
    let prepared = state
        .services()
        .subscriptions()
        .prepare_subscription_update(
            &snapshot,
            subscription_id.as_deref(),
            prefer_proxy,
            proxy_url.as_deref(),
        )
        .await
        .map_err(subscription_error)?;
    if !prepared.has_imports() {
        return Ok(subscription_update_to_contract(prepared.into_result()));
    }
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let result = {
        let (unit_of_work, config) = mutation.split();
        SubscriptionManager::new_in(unit_of_work)
            .apply_prepared_subscription_update(config, prepared)
            .await
            .map_err(subscription_error)?
    };
    let config_changed = original != *mutation.config();
    commit_config_mutation(mutation).await?;
    emit_subscription_invalidation(&app, "subscriptions-updated", true, config_changed)?;

    Ok(subscription_update_to_contract(result))
}
