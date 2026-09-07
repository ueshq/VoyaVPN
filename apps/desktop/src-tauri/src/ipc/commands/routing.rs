use super::{lifecycle::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn list_routings(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RoutingContract>, AppError> {
    state
        .services()
        .routings()
        .list_routings()
        .await
        .map(|items| items.into_iter().map(routing_to_contract).collect())
        .map_err(routing_error)
}

#[tauri::command]
#[specta::specta]
pub async fn save_routing<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    item: RoutingContract,
) -> Result<RoutingContract, AppError> {
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let saved = {
        let (unit_of_work, config) = mutation.split();
        RoutingManager::new_in(unit_of_work)
            .save_routing(config, routing_from_contract(item))
            .await
            .map_err(routing_error)?
    };
    let changed = original != *mutation.config();
    let config = commit_config_mutation(mutation).await?;
    emit_routing_invalidation(&app, "routing-saved", changed);
    restart_after_config_change(&app, &state, &config, ConfigChange::ROUTING_SAVED).await;

    Ok(routing_to_contract(saved))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_routings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    validate_ipc_text_list(&ids, "routing id", IPC_ID_MAX_CHARS, AppError::Routing)?;
    let mut mutation = begin_config_mutation(&state).await?;
    let original = mutation.config().clone();
    let deleted = {
        let (unit_of_work, config) = mutation.split();
        RoutingManager::new_in(unit_of_work)
            .delete_routings(config, &ids)
            .await
            .map_err(routing_error)?
    };
    let changed = original != *mutation.config();
    let config = commit_config_mutation(mutation).await?;
    emit_routing_invalidation(&app, "routings-deleted", changed);
    restart_after_config_change(&app, &state, &config, ConfigChange::ROUTING_DELETED).await;

    Ok(deleted)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_routing<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<RoutingContract, AppError> {
    validate_required_ipc_text(&id, "routing id", IPC_ID_MAX_CHARS, AppError::Routing)?;
    let mut mutation = begin_config_mutation(&state).await?;
    let active = {
        let (unit_of_work, config) = mutation.split();
        RoutingManager::new_in(unit_of_work)
            .set_active_routing(config, &id)
            .await
            .map_err(routing_error)?
    };
    let config = commit_config_mutation(mutation).await?;
    emit_routing_invalidation(&app, "active-routing-changed", true);
    restart_after_config_change(&app, &state, &config, ConfigChange::ROUTING_SELECTED).await;

    Ok(routing_to_contract(active))
}

#[tauri::command]
#[specta::specta]
pub async fn save_routing_rule<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
    rule: RoutingRuleContract,
) -> Result<RoutingContract, AppError> {
    validate_required_ipc_text(
        &routing_id,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    let mutation = begin_config_mutation(&state).await?;
    let saved = mutation
        .routings()
        .save_rule(&routing_id, rule_from_contract(rule))
        .await
        .map_err(routing_error)?;
    let config = commit_config_mutation(mutation).await?;

    emit_routing_invalidation(&app, "routing-rule-saved", false);
    restart_after_config_change(&app, &state, &config, ConfigChange::ROUTING_RULE_SAVED).await;

    Ok(routing_to_contract(saved))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_routing_rules<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    routing_id: String,
    rule_ids: Vec<String>,
) -> Result<RoutingContract, AppError> {
    validate_required_ipc_text(
        &routing_id,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    validate_ipc_text_list(
        &rule_ids,
        "routing rule id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    let mutation = begin_config_mutation(&state).await?;
    let saved = mutation
        .routings()
        .delete_rules(&routing_id, &rule_ids)
        .await
        .map_err(routing_error)?;
    let config = commit_config_mutation(mutation).await?;

    emit_routing_invalidation(&app, "routing-rules-deleted", false);
    restart_after_config_change(&app, &state, &config, ConfigChange::ROUTING_RULES_DELETED).await;

    Ok(routing_to_contract(saved))
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
    validate_required_ipc_text(
        &routing_id,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    validate_required_ipc_text(
        &rule_id,
        "routing rule id",
        IPC_ID_MAX_CHARS,
        AppError::Routing,
    )?;
    let mutation = begin_config_mutation(&state).await?;
    let saved = mutation
        .routings()
        .move_rule(
            &routing_id,
            &rule_id,
            move_action_from_contract(action),
            position,
        )
        .await
        .map_err(routing_error)?;
    let config = commit_config_mutation(mutation).await?;

    emit_routing_invalidation(&app, "routing-rule-moved", false);
    restart_after_config_change(&app, &state, &config, ConfigChange::ROUTING_RULE_MOVED).await;

    Ok(routing_to_contract(saved))
}
