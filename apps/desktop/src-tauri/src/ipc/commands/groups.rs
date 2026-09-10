use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn list_group_child_candidates(
    state: tauri::State<'_, AppState>,
    current_index_id: Option<String>,
    filter: Option<String>,
) -> Result<Vec<GroupChildContract>, AppError> {
    validate_present_ipc_text(
        current_index_id.as_deref(),
        "profile index id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Group,
    )?;
    validate_optional_ipc_text(
        filter.as_deref(),
        "group candidate filter",
        IPC_FILTER_MAX_CHARS,
        AppErrorSubsystem::Group,
    )?;
    state
        .services()
        .groups()
        .list_child_candidates(current_index_id.as_deref(), filter.as_deref())
        .await
        .map(|items| items.into_iter().map(group_child_to_contract).collect())
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn preview_group_profile(
    state: tauri::State<'_, AppState>,
    profile: ProfileContract,
) -> Result<GroupPreviewContract, AppError> {
    let config = current_config(&state);

    state
        .services()
        .groups()
        .preview_group_profile(&config, &profile_from_contract(profile))
        .await
        .map(group_preview_to_contract)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn save_group_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    profile: ProfileContract,
) -> Result<ProfileListEntry, AppError> {
    let saved = mutate_config(&state, async |unit_of_work, config| {
        Ok(GroupManager::new_in(unit_of_work)
            .save_group_profile(config, profile_from_contract(profile))
            .await?)
    })
    .await?;
    emit_profile_invalidation(&app, "group-profile-saved", saved.config_changed);

    Ok(profile_list_to_contract(saved.value))
}
