use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn list_profiles(
    state: tauri::State<'_, AppState>,
    subscription_id: Option<String>,
    filter: Option<String>,
) -> Result<ProfileListing, AppError> {
    validate_present_ipc_text(
        subscription_id.as_deref(),
        "subscription id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Profile,
    )?;
    validate_optional_ipc_text(
        filter.as_deref(),
        "profile filter",
        IPC_FILTER_MAX_CHARS,
        AppErrorSubsystem::Profile,
    )?;
    let config = current_config(&state)?;

    state
        .services()
        .profiles()
        .list_profiles(&config, subscription_id.as_deref(), filter.as_deref())
        .await
        .map(profile_listing_to_contract)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn save_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile: ProfileContract,
) -> Result<ProfileListEntry, AppError> {
    let saved = mutate_config(&state, async |unit_of_work, config| {
        Ok(ProfileManager::new_in(unit_of_work)
            .save_profile(config, profile_from_contract(profile))
            .await?)
    })
    .await?;
    emit_profile_invalidation(&app, "profile-saved", saved.config_changed);

    Ok(profile_list_to_contract(saved.value))
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
        AppErrorSubsystem::Profile,
    )?;
    let deleted = mutate_config(&state, async |unit_of_work, config| {
        Ok(ProfileManager::new_in(unit_of_work)
            .delete_profiles(config, &index_ids)
            .await?)
    })
    .await?;
    emit_profile_invalidation(&app, "profiles-deleted", deleted.config_changed);

    Ok(u32::try_from(deleted.value).unwrap_or(u32::MAX))
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
        AppErrorSubsystem::Profile,
    )?;
    let copied = mutate_config(&state, async |unit_of_work, config| {
        Ok(ProfileManager::new_in(unit_of_work)
            .copy_profiles(config, &index_ids)
            .await?)
    })
    .await?;
    emit_profile_invalidation(&app, "profiles-copied", copied.config_changed);

    Ok(copied
        .value
        .into_iter()
        .map(profile_list_to_contract)
        .collect())
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
        AppErrorSubsystem::Profile,
    )?;
    let active = mutate_config(&state, async |unit_of_work, config| {
        Ok(ProfileManager::new_in(unit_of_work)
            .set_active_profile(config, &index_id)
            .await?)
    })
    .await?;
    // The active-profile pointer lives in the persisted config, so the settings
    // bundle projected from it is refreshed too.
    emit_profile_invalidation(&app, "active-profile-changed", true);

    Ok(profile_list_to_contract(active.value))
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
        AppErrorSubsystem::Profile,
    )?;
    validate_required_ipc_text(
        &index_id,
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Profile,
    )?;
    let profiles = mutate_config(&state, async |unit_of_work, config| {
        Ok(ProfileManager::new_in(unit_of_work)
            .move_profile(
                config,
                subscription_id.as_deref(),
                &index_id,
                move_action_from_contract(action),
                position,
            )
            .await?)
    })
    .await?;

    emit_profile_invalidation(&app, "profile-moved", false);

    Ok(profiles
        .value
        .into_iter()
        .map(profile_list_to_contract)
        .collect())
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
        AppErrorSubsystem::Profile,
    )?;
    let profiles = mutate_config(&state, async |unit_of_work, config| {
        Ok(ProfileManager::new_in(unit_of_work)
            .sort_profiles(
                config,
                subscription_id.as_deref(),
                profile_sort_key_from_contract(sort_key),
                ascending,
            )
            .await?)
    })
    .await?;

    emit_profile_invalidation(&app, "profiles-sorted", false);

    Ok(profiles
        .value
        .into_iter()
        .map(profile_list_to_contract)
        .collect())
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
        AppErrorSubsystem::Profile,
    )?;
    let deduped = mutate_config(&state, async |unit_of_work, config| {
        Ok(ProfileManager::new_in(unit_of_work)
            .dedupe_profiles(
                config,
                subscription_id.as_deref(),
                keep_older.unwrap_or(false),
            )
            .await?)
    })
    .await?;
    emit_profile_invalidation(&app, "profiles-deduped", deduped.config_changed);

    Ok(profile_dedupe_to_contract(deduped.value))
}
