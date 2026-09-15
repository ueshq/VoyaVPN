use super::{post_commit::*, support::*, *};

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
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn save_routing<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    item: RoutingContract,
) -> Result<RoutingContract, AppError> {
    let saved = mutate_config(&state, async |unit_of_work, config| {
        Ok(RoutingManager::new_in(unit_of_work)
            .save_routing(config, routing_from_contract(item))
            .await?)
    })
    .await?;
    emit_routing_invalidation(&app, "routing-saved", saved.config_changed);
    restart_after_config_change(&app, &state, &saved.config, ConfigChange::ROUTING_SAVED).await;

    Ok(routing_to_contract(saved.value))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_routings<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, AppError> {
    validate_ipc_text_list(
        &ids,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Routing,
    )?;
    let deleted = mutate_config(&state, async |unit_of_work, config| {
        Ok(RoutingManager::new_in(unit_of_work)
            .delete_routings(config, &ids)
            .await?)
    })
    .await?;
    emit_routing_invalidation(&app, "routings-deleted", deleted.config_changed);
    restart_after_config_change(&app, &state, &deleted.config, ConfigChange::ROUTING_DELETED).await;

    Ok(deleted.value)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_routing<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<RoutingContract, AppError> {
    validate_required_ipc_text(
        &id,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Routing,
    )?;
    let active = mutate_config(&state, async |unit_of_work, config| {
        Ok(RoutingManager::new_in(unit_of_work)
            .set_active_routing(config, &id)
            .await?)
    })
    .await?;
    emit_routing_invalidation(&app, "active-routing-changed", true);
    restart_after_config_change(&app, &state, &active.config, ConfigChange::ROUTING_SELECTED).await;

    Ok(routing_to_contract(active.value))
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
        AppErrorSubsystem::Routing,
    )?;
    let saved = mutate_config(&state, async |unit_of_work, _config| {
        Ok(RoutingManager::new_in(unit_of_work)
            .save_rule(&routing_id, rule_from_contract(rule))
            .await?)
    })
    .await?;

    emit_routing_invalidation(&app, "routing-rule-saved", false);
    restart_after_config_change(
        &app,
        &state,
        &saved.config,
        ConfigChange::ROUTING_RULE_SAVED,
    )
    .await;

    Ok(routing_to_contract(saved.value))
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
        AppErrorSubsystem::Routing,
    )?;
    validate_ipc_text_list(
        &rule_ids,
        "routing rule id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Routing,
    )?;
    let saved = mutate_config(&state, async |unit_of_work, _config| {
        Ok(RoutingManager::new_in(unit_of_work)
            .delete_rules(&routing_id, &rule_ids)
            .await?)
    })
    .await?;

    emit_routing_invalidation(&app, "routing-rules-deleted", false);
    restart_after_config_change(
        &app,
        &state,
        &saved.config,
        ConfigChange::ROUTING_RULES_DELETED,
    )
    .await;

    Ok(routing_to_contract(saved.value))
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
        AppErrorSubsystem::Routing,
    )?;
    validate_required_ipc_text(
        &rule_id,
        "routing rule id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Routing,
    )?;
    let saved = mutate_config(&state, async |unit_of_work, _config| {
        Ok(RoutingManager::new_in(unit_of_work)
            .move_rule(
                &routing_id,
                &rule_id,
                move_action_from_contract(action),
                position,
            )
            .await?)
    })
    .await?;

    emit_routing_invalidation(&app, "routing-rule-moved", false);
    restart_after_config_change(
        &app,
        &state,
        &saved.config,
        ConfigChange::ROUTING_RULE_MOVED,
    )
    .await;

    Ok(routing_to_contract(saved.value))
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
    validate_required_ipc_text(
        &routing_id,
        "routing id",
        IPC_ID_MAX_CHARS,
        AppErrorSubsystem::Routing,
    )?;
    let saved = mutate_config(&state, async |unit_of_work, _config| {
        Ok(RoutingManager::new_in(unit_of_work)
            .reset_rules_to_default(&routing_id)
            .await?)
    })
    .await?;

    emit_routing_invalidation(&app, "routing-rules-reset", false);
    restart_after_config_change(
        &app,
        &state,
        &saved.config,
        ConfigChange::ROUTING_RULES_RESET,
    )
    .await;

    Ok(routing_to_contract(saved.value))
}
