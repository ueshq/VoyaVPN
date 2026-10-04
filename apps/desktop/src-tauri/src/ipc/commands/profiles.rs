use voya_app::profiles::{
    delete_profiles_use_case, move_profile_use_case, save_profile_use_case,
    set_active_profile_use_case,
};

use super::{post_commit::*, *};

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
    let config = state.config_mutations().current_config();

    voya_app::profiles::get_profile_use_case(state.services(), &config, &index_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile: ProfileContract,
) -> Result<ProfileDetails, AppError> {
    let saved = save_profile_use_case(state.config_mutations(), profile).await?;
    emit_invalidation(
        &app,
        "profile-saved",
        invalidation::profile_scopes(saved.config_changed),
    );

    Ok(saved.value)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_profiles<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<u32, AppError> {
    let deleted = delete_profiles_use_case(state.config_mutations(), index_ids).await?;
    emit_then_disconnect_removed(&app, &state, |app| {
        emit_invalidation(
            app,
            "profiles-deleted",
            invalidation::profile_scopes(deleted.config_changed),
        )
    })
    .await?;

    Ok(deleted.value)
}

#[tauri::command]
#[specta::specta]
pub async fn export_profile_share_links(
    state: tauri::State<'_, AppState>,
    index_ids: Vec<String>,
) -> Result<ExportProfilesResult, AppError> {
    let config = state.config_mutations().current_config();

    state.services().export_profiles(&config, &index_ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_id: String,
) -> Result<ProfileDetails, AppError> {
    let active = set_active_profile_use_case(state.config_mutations(), index_id).await?;
    // The active-profile pointer lives in the persisted config, so the settings
    // bundle projected from it is refreshed too.
    emit_invalidation(
        &app,
        "active-profile-changed",
        invalidation::profile_scopes(true),
    );

    Ok(active.value)
}

#[tauri::command]
#[specta::specta]
pub async fn move_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    index_id: String,
    action: ContractMoveAction,
    position: Option<i32>,
) -> Result<(), AppError> {
    move_profile_use_case(state.config_mutations(), index_id, action, position).await?;
    emit_invalidation(&app, "profile-moved", invalidation::profile_scopes(false));

    Ok(())
}
