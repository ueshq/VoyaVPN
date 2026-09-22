//! Routing profiles and the rules inside them.
//!
//! Every mutation here ends the same way the shell's does: announce the
//! caches, then restart a connected core for the committed change. Which
//! change that is — and which notice names a failed restart — is
//! `voya_app::post_commit::ConfigChange`, shared by both hosts.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    contract_map::{
        move_action_from_contract, routing_from_contract, routing_to_contract, rule_from_contract,
    },
    invalidation,
    post_commit::ConfigChange,
    routing::RoutingManager,
};
use voya_contracts::{AppError, MoveAction, Routing, RoutingRule};

use crate::app::MobileState;

use super::{answer, arguments, runtime::finish_config_change};

pub(super) async fn list(state: &MobileState) -> Result<Value, AppError> {
    let items = state.services.list_routings().await?;

    answer(
        "list_routings",
        &items
            .into_iter()
            .map(routing_to_contract)
            .collect::<Vec<_>>(),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveRouting {
    item: Routing,
}

pub(super) async fn save(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveRouting { item } = arguments("save_routing", args)?;
    let saved = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(RoutingManager::new_in(unit_of_work)
                .save_routing(config, routing_from_contract(item))
                .await?)
        })
        .await?;
    finish_config_change(
        state,
        "routing-saved",
        invalidation::routing_scopes(saved.config_changed),
        &saved.config,
        ConfigChange::ROUTING_SAVED,
    )
    .await;

    answer("save_routing", &routing_to_contract(saved.value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ids {
    ids: Vec<String>,
}

pub(super) async fn delete(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let Ids { ids } = arguments("delete_routings", args)?;
    let deleted = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(RoutingManager::new_in(unit_of_work)
                .delete_routings(config, &ids)
                .await?)
        })
        .await?;
    finish_config_change(
        state,
        "routings-deleted",
        invalidation::routing_scopes(deleted.config_changed),
        &deleted.config,
        ConfigChange::ROUTING_DELETED,
    )
    .await;

    answer("delete_routings", &deleted.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Id {
    id: String,
}

pub(super) async fn set_active(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let Id { id } = arguments("set_active_routing", args)?;
    let active = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(RoutingManager::new_in(unit_of_work)
                .set_active_routing(config, &id)
                .await?)
        })
        .await?;
    finish_config_change(
        state,
        "active-routing-changed",
        invalidation::routing_scopes(active.config_changed),
        &active.config,
        ConfigChange::ROUTING_SELECTED,
    )
    .await;

    answer("set_active_routing", &routing_to_contract(active.value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveRule {
    routing_id: String,
    rule: RoutingRule,
}

pub(super) async fn save_rule(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveRule { routing_id, rule } = arguments("save_routing_rule", args)?;
    let saved = state
        .config_mutations
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
            Ok(RoutingManager::new_in(unit_of_work)
                .save_rule(&routing_id, rule_from_contract(rule))
                .await?)
        })
        .await?;
    finish_config_change(
        state,
        "routing-rule-saved",
        invalidation::routing_scopes(saved.config_changed),
        &saved.config,
        ConfigChange::ROUTING_RULE_SAVED,
    )
    .await;

    answer("save_routing_rule", &routing_to_contract(saved.value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteRules {
    routing_id: String,
    rule_ids: Vec<String>,
}

pub(super) async fn delete_rules(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let DeleteRules {
        routing_id,
        rule_ids,
    } = arguments("delete_routing_rules", args)?;
    let deleted = state
        .config_mutations
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
            Ok(RoutingManager::new_in(unit_of_work)
                .delete_rules(&routing_id, &rule_ids)
                .await?)
        })
        .await?;
    finish_config_change(
        state,
        "routing-rules-deleted",
        invalidation::routing_scopes(deleted.config_changed),
        &deleted.config,
        ConfigChange::ROUTING_RULES_DELETED,
    )
    .await;

    answer("delete_routing_rules", &routing_to_contract(deleted.value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveRule {
    routing_id: String,
    rule_id: String,
    action: MoveAction,
    position: Option<i32>,
}

pub(super) async fn move_rule(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let MoveRule {
        routing_id,
        rule_id,
        action,
        position,
    } = arguments("move_routing_rule", args)?;
    let moved = state
        .config_mutations
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
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
    finish_config_change(
        state,
        "routing-rule-moved",
        invalidation::routing_scopes(moved.config_changed),
        &moved.config,
        ConfigChange::ROUTING_RULE_MOVED,
    )
    .await;

    answer("move_routing_rule", &routing_to_contract(moved.value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetRules {
    routing_id: String,
}

pub(super) async fn reset_rules(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let ResetRules { routing_id } = arguments("reset_routing_rules", args)?;
    let reset = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            let _ = config;
            Ok(RoutingManager::new_in(unit_of_work)
                .reset_rules_to_default(&routing_id)
                .await?)
        })
        .await?;
    finish_config_change(
        state,
        "routing-rules-reset",
        invalidation::routing_scopes(reset.config_changed),
        &reset.config,
        ConfigChange::ROUTING_RULES_RESET,
    )
    .await;

    answer("reset_routing_rules", &routing_to_contract(reset.value))
}
