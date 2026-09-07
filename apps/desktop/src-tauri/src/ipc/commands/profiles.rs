use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn list_profiles(
    state: tauri::State<'_, AppState>,
    subscription_id: Option<String>,
    filter: Option<String>,
) -> Result<Vec<ProfileListEntry>, AppError> {
    validate_present_ipc_text(
        subscription_id.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    validate_optional_ipc_text(
        filter.as_deref(),
        "profile filter",
        IPC_FILTER_MAX_CHARS,
        AppError::Profile,
    )?;
    let config = current_config(&state)?;

    state
        .services()
        .profiles()
        .list_profiles(&config, subscription_id.as_deref(), filter.as_deref())
        .await
        .map(|items| items.into_iter().map(profile_list_to_contract).collect())
        .map_err(profile_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile: ProfileContract,
) -> Result<ProfileListEntry, AppError> {
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let result = {
        let (unit_of_work, config) = mutation.split();
        ProfileManager::new_in(unit_of_work)
            .save_profile(config, profile_from_contract(profile))
            .await
            .map_err(profile_error)?
    };
    let config_changed = original != *mutation.config();
    commit_config_mutation(mutation).await?;
    emit_profile_invalidation(&app, "profile-saved", config_changed);

    Ok(profile_list_to_contract(result))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<u32, AppError> {
    validate_ipc_text_list(
        &index_ids,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let deleted = {
        let (unit_of_work, config) = mutation.split();
        ProfileManager::new_in(unit_of_work)
            .delete_profiles(config, &index_ids)
            .await
            .map_err(profile_error)?
    };
    let config_changed = original != *mutation.config();
    commit_config_mutation(mutation).await?;
    emit_profile_invalidation(&app, "profiles-deleted", config_changed);

    Ok(u32::try_from(deleted).unwrap_or(u32::MAX))
}

#[tauri::command]
#[specta::specta]
pub async fn copy_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<Vec<ProfileListEntry>, AppError> {
    validate_ipc_text_list(
        &index_ids,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let copied = {
        let (unit_of_work, config) = mutation.split();
        ProfileManager::new_in(unit_of_work)
            .copy_profiles(config, &index_ids)
            .await
            .map_err(profile_error)?
    };
    let config_changed = original != *mutation.config();
    commit_config_mutation(mutation).await?;
    emit_profile_invalidation(&app, "profiles-copied", config_changed);

    Ok(copied.into_iter().map(profile_list_to_contract).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_share_links(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    export_profiles_result(&state, index_ids, ExportProfilesFormat::ShareLinks).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_share_links_base64(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    export_profiles_result(&state, index_ids, ExportProfilesFormat::ShareLinksBase64).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_voya_bundle(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    export_profiles_result(&state, index_ids, ExportProfilesFormat::VoyaBundle).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_client_config(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    export_profiles_result(&state, index_ids, ExportProfilesFormat::ClientConfig).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_id: String,
) -> Result<ProfileListEntry, AppError> {
    validate_required_ipc_text(
        &index_id,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let mut mutation = begin_config_mutation(&state).await?;
    let active = {
        let (unit_of_work, config) = mutation.split();
        ProfileManager::new_in(unit_of_work)
            .set_active_profile(config, &index_id)
            .await
            .map_err(profile_error)?
    };
    commit_config_mutation(mutation).await?;
    // The active-profile pointer lives in the persisted config, so the settings
    // bundle projected from it is refreshed too.
    emit_profile_invalidation(&app, "active-profile-changed", true);

    Ok(profile_list_to_contract(active))
}

#[tauri::command]
#[specta::specta]
pub async fn move_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    subscription_id: Option<String>,
    index_id: String,
    action: ContractMoveAction,
    position: Option<i32>,
) -> Result<Vec<ProfileListEntry>, AppError> {
    validate_present_ipc_text(
        subscription_id.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    validate_required_ipc_text(
        &index_id,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let mutation = begin_config_mutation(&state).await?;
    let profiles = mutation
        .profiles()
        .move_profile(
            mutation.config(),
            subscription_id.as_deref(),
            &index_id,
            move_action_from_contract(action),
            position,
        )
        .await
        .map_err(profile_error)?;
    commit_config_mutation(mutation).await?;

    emit_profile_invalidation(&app, "profile-moved", false);

    Ok(profiles.into_iter().map(profile_list_to_contract).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn sort_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    subscription_id: Option<String>,
    sort_key: ProfileSortContract,
    ascending: bool,
) -> Result<Vec<ProfileListEntry>, AppError> {
    validate_present_ipc_text(
        subscription_id.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let mutation = begin_config_mutation(&state).await?;
    let profiles = mutation
        .profiles()
        .sort_profiles(
            mutation.config(),
            subscription_id.as_deref(),
            profile_sort_key_from_contract(sort_key),
            ascending,
        )
        .await
        .map_err(profile_error)?;
    commit_config_mutation(mutation).await?;

    emit_profile_invalidation(&app, "profiles-sorted", false);

    Ok(profiles.into_iter().map(profile_list_to_contract).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn dedupe_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    subscription_id: Option<String>,
    keep_older: Option<bool>,
) -> Result<ProfileDedupeContract, AppError> {
    validate_present_ipc_text(
        subscription_id.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppError::Profile,
    )?;
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let result = {
        let (unit_of_work, config) = mutation.split();
        ProfileManager::new_in(unit_of_work)
            .dedupe_profiles(
                config,
                subscription_id.as_deref(),
                keep_older.unwrap_or(false),
            )
            .await
            .map_err(profile_error)?
    };
    let config_changed = original != *mutation.config();
    commit_config_mutation(mutation).await?;
    emit_profile_invalidation(&app, "profiles-deduped", config_changed);

    Ok(profile_dedupe_to_contract(result))
}
