//! Thin adapters over `voya_app::policy_groups` and the running group's Clash API.

use voya_app::contract_map::policy_group_entry_to_contract;
use voya_app::policy_groups::{
    delete_policy_groups_use_case, running_policy_group_runtime, save_policy_group_use_case,
    select_member_use_case, set_active_policy_group_use_case, test_running_policy_group_delay,
};
use voya_contracts::{PolicyGroup, PolicyGroupListing, PolicyGroupRuntime};

use super::{post_commit::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn list_policy_groups(
    state: tauri::State<'_, AppState>,
) -> Result<PolicyGroupListing, AppError> {
    let config = state.config_mutations().current_config();
    let entries = state
        .services()
        .policy_groups()
        .list(&config)
        .await
        .map_err(AppError::from)?;

    Ok(PolicyGroupListing {
        entries: entries
            .into_iter()
            .map(policy_group_entry_to_contract)
            .collect(),
    })
}

/// Saves a group. Editing the group a running core uses restarts the core so
/// the change takes effect.
#[tauri::command]
#[specta::specta]
pub async fn save_policy_group<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    group: PolicyGroup,
) -> Result<PolicyGroup, AppError> {
    let saved = save_policy_group_use_case(state.config_mutations(), group).await?;
    emit_invalidation(
        &app,
        "policy-group-saved",
        invalidation::policy_group_scopes(saved.config_changed),
    );
    if saved.config.active_group_id == saved.value.id {
        restart_after_config_change(&app, &state, &saved.config, ConfigChange::POLICY_GROUP).await;
    }

    Ok(saved.value)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_policy_groups<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    let deleted = delete_policy_groups_use_case(state.config_mutations(), ids).await?;
    emit_then_disconnect_removed(&app, &state, |app| {
        emit_invalidation(
            app,
            "policy-groups-deleted",
            invalidation::policy_group_scopes(deleted.config_changed),
        )
    })
    .await?;

    Ok(deleted.value)
}

/// Makes a group what connecting uses. Connecting or restarting is the
/// caller's next step, exactly as after choosing a node.
#[tauri::command]
#[specta::specta]
pub async fn set_active_policy_group<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<PolicyGroup, AppError> {
    let active = set_active_policy_group_use_case(state.config_mutations(), id).await?;
    emit_invalidation(
        &app,
        "active-policy-group-changed",
        invalidation::policy_group_scopes(true),
    );

    Ok(active.value)
}

/// Stores a selector's member and, when that group is running, switches the
/// core to it live. The choice is kept even if the live switch fails.
#[tauri::command]
#[specta::specta]
pub async fn select_policy_group_member<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    group_id: String,
    profile_id: String,
) -> Result<PolicyGroup, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&group_id, IPC_ID_MAX_CHARS),
        "policy group id",
        AppErrorSubsystem::PolicyGroup,
    )?;
    map_ipc_input(
        input_safety::validate_required_text(&profile_id, IPC_ID_MAX_CHARS),
        "node id",
        AppErrorSubsystem::PolicyGroup,
    )?;
    let (group, live_error) = select_member_use_case(
        state.config_mutations(),
        &state.supervisor(),
        &state.services().policy_groups(),
        state.proxy_runtime(),
        &group_id,
        &profile_id,
    )
    .await?;
    if let Some(message) = live_error {
        report_post_commit_error(
            &app,
            NoticeCode::PolicyGroupSelectionRuntimeUpdateFailed,
            &message,
            AppNoticeLevel::Warning,
        );
    }
    emit_invalidation(
        &app,
        "policy-group-member-selected",
        invalidation::policy_group_runtime_scopes(),
    );

    Ok(group)
}

/// The running group's current member and delays; `None` while no group runs.
#[tauri::command]
#[specta::specta]
pub async fn policy_group_runtime(
    state: tauri::State<'_, AppState>,
) -> Result<Option<PolicyGroupRuntime>, AppError> {
    let snapshot = state.supervisor().status().await.map_err(AppError::from)?;
    running_policy_group_runtime(
        &snapshot,
        &state.services().policy_groups(),
        state.proxy_runtime(),
    )
    .await
    .map_err(AppError::from)
}

/// Probes every member of the running group through the core and returns the
/// group with those delays; `None` while no group runs.
#[tauri::command]
#[specta::specta]
pub async fn test_policy_group_delay<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<Option<PolicyGroupRuntime>, AppError> {
    let snapshot = state.supervisor().status().await.map_err(AppError::from)?;
    let runtime = test_running_policy_group_delay(
        &snapshot,
        &state.services().policy_groups(),
        state.proxy_runtime(),
    )
    .await
    .map_err(AppError::from)?;
    if runtime.is_some() {
        emit_invalidation(
            &app,
            "policy-group-delay-tested",
            invalidation::policy_group_runtime_scopes(),
        );
    }
    Ok(runtime)
}
