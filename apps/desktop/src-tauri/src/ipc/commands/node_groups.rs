use super::{lifecycle::*, support::*, *};
use voya_app::node_groups::NodeGroupManager;
use voya_contracts::{NodeGroup, NodeGroupAssignment, NodeGroupsSnapshot};

#[tauri::command]
#[specta::specta]
pub async fn list_node_groups(
    state: tauri::State<'_, AppState>,
) -> Result<NodeGroupsSnapshot, AppError> {
    state
        .services()
        .node_groups()
        .list()
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn save_node_group<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    id: Option<String>,
    name: String,
) -> Result<NodeGroup, AppError> {
    validate_present_ipc_text(
        id.as_deref(),
        "group id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Group,
    )?;
    validate_required_ipc_text(
        &name,
        "group name",
        IPC_NAME_MAX_CHARS,
        AppErrorSubsystem::Group,
    )?;
    let saved = mutate_config(&state, async |unit, _| {
        Ok(NodeGroupManager::new_in(unit)
            .save(id.as_deref(), &name)
            .await?)
    })
    .await?;
    emit_profile_invalidation(&app, "node-group-saved", false);
    Ok(saved.value)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_node_group<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    validate_required_ipc_text(&id, "group id", IPC_ID_MAX_CHARS, AppErrorSubsystem::Group)?;
    mutate_config(&state, async |unit, _| {
        Ok(NodeGroupManager::new_in(unit).delete(&id).await?)
    })
    .await?;
    emit_profile_invalidation(&app, "node-group-deleted", false);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn move_node_group<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    id: String,
    action: voya_contracts::MoveAction,
) -> Result<(), AppError> {
    validate_required_ipc_text(&id, "group id", IPC_ID_MAX_CHARS, AppErrorSubsystem::Group)?;
    mutate_config(&state, async |unit, _| {
        Ok(NodeGroupManager::new_in(unit)
            .move_group(&id, action)
            .await?)
    })
    .await?;
    emit_profile_invalidation(&app, "node-group-moved", false);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn assign_node_groups<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    assignments: Vec<NodeGroupAssignment>,
) -> Result<(), AppError> {
    let ids = assignments
        .iter()
        .map(|a| a.profile_id.clone())
        .collect::<Vec<_>>();
    validate_ipc_text_list(&ids, "node id", IPC_ID_MAX_CHARS, AppErrorSubsystem::Group)?;
    for assignment in &assignments {
        validate_present_ipc_text(
            assignment.group_id.as_deref(),
            "group id",
            IPC_ID_MAX_CHARS,
            AppErrorSubsystem::Group,
        )?;
    }
    mutate_config(&state, async |unit, _| {
        Ok(NodeGroupManager::new_in(unit).assign(&assignments).await?)
    })
    .await?;
    emit_profile_invalidation(&app, "node-groups-assigned", false);
    Ok(())
}
