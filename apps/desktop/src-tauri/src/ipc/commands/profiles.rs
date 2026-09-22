use super::{post_commit::*, support::*, *};

/// Every node as the node table shows it; `get_profile` has one in full.
#[tauri::command]
#[specta::specta]
pub async fn list_profile_summaries(
    state: tauri::State<'_, AppState>,
) -> Result<ProfileSummaryListing, AppError> {
    let config = state.config_mutations().current_config();

    state
        .services()
        .profiles()
        .list_summaries(&config)
        .await
        .map(profile_summary_listing_to_contract)
        .map_err(AppError::from)
}

/// One node in full, for the editor and the details dialog.
#[tauri::command]
#[specta::specta]
pub async fn get_profile(
    state: tauri::State<'_, AppState>,
    index_id: String,
) -> Result<ProfileDetails, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&index_id, IPC_ID_MAX_CHARS),
        "node id",
        AppErrorSubsystem::Profile,
    )?;
    let config = state.config_mutations().current_config();

    state
        .services()
        .profiles()
        .get_profile(&config, &index_id)
        .await
        .map(profile_details_to_contract)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn save_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile: ProfileContract,
) -> Result<ProfileDetails, AppError> {
    let saved = state
        .config_mutations()
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(ProfileManager::new_in(unit_of_work)
                .save_profile(config, profile_from_contract(profile))
                .await?)
        })
        .await?;
    emit_profile_invalidation(&app, "profile-saved", saved.config_changed);

    Ok(profile_details_to_contract(saved.value))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<u32, AppError> {
    map_ipc_input(
        input_safety::validate_text_list(&index_ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "node id",
        AppErrorSubsystem::Profile,
    )?;
    let deleted = state
        .config_mutations()
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(ProfileManager::new_in(unit_of_work)
                .delete_profiles(config, &index_ids)
                .await?)
        })
        .await?;
    emit_then_disconnect_removed(&app, &state, |app| {
        emit_profile_invalidation(app, "profiles-deleted", deleted.config_changed)
    })
    .await?;

    Ok(u32::try_from(deleted.value).unwrap_or(u32::MAX))
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_share_links(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    map_ipc_input(
        input_safety::validate_text_list(&index_ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "node id",
        AppErrorSubsystem::Export,
    )?;
    let config = state.config_mutations().current_config();

    state
        .services()
        .export_profiles(&config, &index_ids)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_id: String,
) -> Result<ProfileDetails, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&index_id, IPC_ID_MAX_CHARS),
        "node id",
        AppErrorSubsystem::Profile,
    )?;
    let active = state
        .config_mutations()
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(ProfileManager::new_in(unit_of_work)
                .set_active_profile(config, &index_id)
                .await?)
        })
        .await?;
    // The active-profile pointer lives in the persisted config, so the settings
    // bundle projected from it is refreshed too.
    emit_profile_invalidation(&app, "active-profile-changed", true);

    Ok(profile_details_to_contract(active.value))
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
) -> Result<(), AppError> {
    map_ipc_input(
        input_safety::validate_present_text(subscription_id.as_deref(), IPC_ID_MAX_CHARS),
        "subscription id",
        AppErrorSubsystem::Profile,
    )?;
    map_ipc_input(
        input_safety::validate_required_text(&index_id, IPC_ID_MAX_CHARS),
        "node id",
        AppErrorSubsystem::Profile,
    )?;
    state
        .config_mutations()
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
            Ok(ProfileManager::new_in(unit_of_work)
                .move_profile(
                    subscription_id.as_deref(),
                    &index_id,
                    move_action_from_contract(action),
                    position,
                )
                .await?)
        })
        .await?;

    emit_profile_invalidation(&app, "profile-moved", false);

    Ok(())
}
