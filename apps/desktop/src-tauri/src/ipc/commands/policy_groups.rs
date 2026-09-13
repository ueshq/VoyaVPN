//! Thin adapters over `voya_app::policy_groups` and the running group's Clash API.

use voya_app::contract_map::{
    policy_group_entry_to_contract, policy_group_from_contract, policy_group_runtime_to_contract,
    policy_group_to_contract,
};
use voya_app::policy_groups::{group_test_url, PolicyGroupManager};
use voya_contracts::{PolicyGroup, PolicyGroupListing, PolicyGroupRuntime};

use super::{lifecycle::*, support::*, *};

/// How long the running core waits on each member's probe.
const GROUP_DELAY_TIMEOUT_MS: u32 = 5_000;

#[tauri::command]
#[specta::specta]
pub async fn list_policy_groups(
    state: tauri::State<'_, AppState>,
) -> Result<PolicyGroupListing, AppError> {
    let config = current_config(&state);
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
    validate_ipc_text_list(
        &group.member_ids,
        "node id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::PolicyGroup,
    )?;
    let saved = mutate_config(&state, async |unit_of_work, _config| {
        Ok(PolicyGroupManager::new_in(unit_of_work)
            .save(policy_group_from_contract(group))
            .await?)
    })
    .await?;
    emit_policy_group_invalidation(&app, "policy-group-saved", saved.config_changed);
    if saved.config.active_group_id == saved.value.id {
        restart_after_config_change(&app, &state, &saved.config, ConfigChange::POLICY_GROUP).await;
    }

    Ok(policy_group_to_contract(saved.value))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_policy_groups<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    validate_ipc_text_list(
        &ids,
        "policy group id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::PolicyGroup,
    )?;
    let deleted = mutate_config(&state, async |unit_of_work, config| {
        Ok(PolicyGroupManager::new_in(unit_of_work)
            .delete(config, &ids)
            .await?)
    })
    .await?;
    emit_policy_group_invalidation(&app, "policy-groups-deleted", deleted.config_changed);
    core_flow(&app, &state)
        .disconnect_removed_profile(&current_config(&state))
        .await
        .map_err(AppError::from)?;

    Ok(u32::try_from(deleted.value).unwrap_or(u32::MAX))
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
    validate_required_ipc_text(
        &id,
        "policy group id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::PolicyGroup,
    )?;
    let active = mutate_config(&state, async |unit_of_work, config| {
        Ok(PolicyGroupManager::new_in(unit_of_work)
            .set_active(config, &id)
            .await?)
    })
    .await?;
    emit_policy_group_invalidation(&app, "active-policy-group-changed", true);

    Ok(policy_group_to_contract(active.value))
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
    validate_required_ipc_text(
        &group_id,
        "policy group id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::PolicyGroup,
    )?;
    validate_required_ipc_text(
        &profile_id,
        "node id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::PolicyGroup,
    )?;
    let selected = mutate_config(&state, async |unit_of_work, _config| {
        Ok(PolicyGroupManager::new_in(unit_of_work)
            .select_member(&group_id, &profile_id)
            .await?)
    })
    .await?;

    let snapshot = state.supervisor().status().await.map_err(AppError::from)?;
    if snapshot.state == SupervisorConnectionState::Connected
        && snapshot.active_group_id.as_deref() == Some(group_id.as_str())
    {
        let live = async {
            let (_, members) = state
                .services()
                .policy_groups()
                .resolve(&group_id)
                .await
                .map_err(AppError::from)?;
            state
                .proxy_runtime()
                .select_group_member(&snapshot.clash_api_access(), &members, &profile_id)
                .await
                .map_err(AppError::from)
        }
        .await;
        if let Err(error) = live {
            report_post_commit_error(
                &app,
                NoticeCode::PolicyGroupSelectionRuntimeUpdateFailed,
                &format!("{error:?}"),
                AppNoticeLevel::Warning,
            );
        }
    }
    emit_invalidation(
        &app,
        NoticeCode::PolicyGroupRefreshFailed,
        "policy-group-member-selected",
        invalidation::policy_group_runtime_scopes(),
    );

    Ok(policy_group_to_contract(selected.value))
}

/// The running group's current member and delays; `None` while no group runs.
#[tauri::command]
#[specta::specta]
pub async fn policy_group_runtime(
    state: tauri::State<'_, AppState>,
) -> Result<Option<PolicyGroupRuntime>, AppError> {
    let snapshot = state.supervisor().status().await.map_err(AppError::from)?;
    let Some(group_id) = running_group_id(&snapshot) else {
        return Ok(None);
    };
    let (_, members) = state
        .services()
        .policy_groups()
        .resolve(&group_id)
        .await
        .map_err(AppError::from)?;
    let runtime = state
        .proxy_runtime()
        .group_state(&snapshot.clash_api_access(), &members)
        .await
        .map_err(AppError::from)?;

    Ok(Some(policy_group_runtime_to_contract(group_id, runtime)))
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
    let Some(group_id) = running_group_id(&snapshot) else {
        return Ok(None);
    };
    let (group, members) = state
        .services()
        .policy_groups()
        .resolve(&group_id)
        .await
        .map_err(AppError::from)?;
    let access = snapshot.clash_api_access();
    let delays = state
        .proxy_runtime()
        .test_group_delay(
            &access,
            &members,
            group_test_url(&group),
            GROUP_DELAY_TIMEOUT_MS,
        )
        .await
        .map_err(AppError::from)?;
    let mut runtime = state
        .proxy_runtime()
        .group_state(&access, &members)
        .await
        .map_err(AppError::from)?;
    for member in &mut runtime.members {
        if let Some(delay) = delays.get(&member.profile_id) {
            member.delay_ms = Some(*delay);
        }
    }
    emit_invalidation(
        &app,
        NoticeCode::PolicyGroupRefreshFailed,
        "policy-group-delay-tested",
        invalidation::policy_group_runtime_scopes(),
    );

    Ok(Some(policy_group_runtime_to_contract(group_id, runtime)))
}

fn running_group_id(snapshot: &SupervisorSnapshot) -> Option<String> {
    snapshot
        .active_group_id
        .clone()
        .filter(|_| snapshot.state == SupervisorConnectionState::Connected)
}
