//! Thin adapters over `voya_app::routing`'s contract-typed use cases.

use voya_app::routing::{
    delete_routing_rules_use_case, delete_routings_use_case, list_routings_use_case,
    move_routing_rule_use_case, reset_routing_rules_use_case, save_routing_rule_use_case,
    save_routing_use_case, set_active_routing_use_case,
};

use super::{post_commit::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn list_routings(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RoutingContract>, AppError> {
    list_routings_use_case(state.services()).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_routing<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    item: RoutingContract,
) -> Result<RoutingContract, AppError> {
    let saved = save_routing_use_case(state.config_mutations(), item).await?;
    finish_routing_change(
        &app,
        &state,
        &saved,
        "routing-saved",
        ConfigChange::ROUTING_SAVED,
    )
    .await;

    Ok(saved.value)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_routings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    map_ipc_input(
        input_safety::validate_text_list(&ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "routing id",
        AppErrorSubsystem::Routing,
    )?;
    let deleted = delete_routings_use_case(state.config_mutations(), ids).await?;
    finish_routing_change(
        &app,
        &state,
        &deleted,
        "routings-deleted",
        ConfigChange::ROUTING_DELETED,
    )
    .await;

    Ok(deleted.value)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_routing<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<RoutingContract, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&id, IPC_ID_MAX_CHARS),
        "routing id",
        AppErrorSubsystem::Routing,
    )?;
    let active = set_active_routing_use_case(state.config_mutations(), id).await?;
    finish_routing_change(
        &app,
        &state,
        &active,
        "active-routing-changed",
        ConfigChange::ROUTING_SELECTED,
    )
    .await;

    Ok(active.value)
}

#[tauri::command]
#[specta::specta]
pub async fn save_routing_rule<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
    rule: RoutingRuleContract,
) -> Result<RoutingContract, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&routing_id, IPC_ID_MAX_CHARS),
        "routing id",
        AppErrorSubsystem::Routing,
    )?;
    let saved = save_routing_rule_use_case(state.config_mutations(), routing_id, rule).await?;

    finish_routing_change(
        &app,
        &state,
        &saved,
        "routing-rule-saved",
        ConfigChange::ROUTING_RULE_SAVED,
    )
    .await;

    Ok(saved.value)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_routing_rules<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
    rule_ids: Vec<String>,
) -> Result<RoutingContract, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&routing_id, IPC_ID_MAX_CHARS),
        "routing id",
        AppErrorSubsystem::Routing,
    )?;
    map_ipc_input(
        input_safety::validate_text_list(&rule_ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "routing rule id",
        AppErrorSubsystem::Routing,
    )?;
    let saved =
        delete_routing_rules_use_case(state.config_mutations(), routing_id, rule_ids).await?;

    finish_routing_change(
        &app,
        &state,
        &saved,
        "routing-rules-deleted",
        ConfigChange::ROUTING_RULES_DELETED,
    )
    .await;

    Ok(saved.value)
}

#[tauri::command]
#[specta::specta]
pub async fn move_routing_rule<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
    rule_id: String,
    action: ContractMoveAction,
    position: Option<i32>,
) -> Result<RoutingContract, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&routing_id, IPC_ID_MAX_CHARS),
        "routing id",
        AppErrorSubsystem::Routing,
    )?;
    map_ipc_input(
        input_safety::validate_required_text(&rule_id, IPC_ID_MAX_CHARS),
        "routing rule id",
        AppErrorSubsystem::Routing,
    )?;
    let saved = move_routing_rule_use_case(
        state.config_mutations(),
        routing_id,
        rule_id,
        action,
        position,
    )
    .await?;

    finish_routing_change(
        &app,
        &state,
        &saved,
        "routing-rule-moved",
        ConfigChange::ROUTING_RULE_MOVED,
    )
    .await;

    Ok(saved.value)
}

/// Replaces a routing profile's rules with the default set, keeping its
/// per-app proxy rule.
#[tauri::command]
#[specta::specta]
pub async fn reset_routing_rules<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
) -> Result<RoutingContract, AppError> {
    map_ipc_input(
        input_safety::validate_required_text(&routing_id, IPC_ID_MAX_CHARS),
        "routing id",
        AppErrorSubsystem::Routing,
    )?;
    let saved = reset_routing_rules_use_case(state.config_mutations(), routing_id).await?;

    finish_routing_change(
        &app,
        &state,
        &saved,
        "routing-rules-reset",
        ConfigChange::ROUTING_RULES_RESET,
    )
    .await;

    Ok(saved.value)
}
