//! Routing profiles and the rules inside them.
//!
//! Every mutation here ends the same way the shell's does: announce the
//! caches, then restart a connected core for the committed change. Which
//! change that is — and which notice names a failed restart — is
//! `voya_app::post_commit::ConfigChange`, shared by both hosts. The bodies
//! themselves are `voya_app::routing`'s use cases.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    invalidation,
    post_commit::ConfigChange,
    routing::{
        delete_routing_rules_use_case, delete_routings_use_case, list_routings_use_case,
        move_routing_rule_use_case, reset_routing_rules_use_case, save_routing_rule_use_case,
        save_routing_use_case, set_active_routing_use_case,
    },
};
use voya_contracts::{AppError, MoveAction, Routing, RoutingRule};

use crate::app::MobileState;

use super::{answer, arguments, runtime::finish_config_change};

pub(super) async fn list(state: &MobileState) -> Result<Value, AppError> {
    answer(
        "list_routings",
        &list_routings_use_case(&state.services).await?,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveRouting {
    item: Routing,
}

pub(super) async fn save(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveRouting { item } = arguments("save_routing", args)?;
    let saved = save_routing_use_case(&state.config_mutations, item).await?;
    finish_config_change(
        state,
        "routing-saved",
        invalidation::routing_scopes(saved.config_changed),
        &saved.config,
        ConfigChange::ROUTING_SAVED,
    )
    .await;

    answer("save_routing", &saved.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ids {
    ids: Vec<String>,
}

pub(super) async fn delete(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let Ids { ids } = arguments("delete_routings", args)?;
    let deleted = delete_routings_use_case(&state.config_mutations, ids).await?;
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
    let active = set_active_routing_use_case(&state.config_mutations, id).await?;
    finish_config_change(
        state,
        "active-routing-changed",
        invalidation::routing_scopes(active.config_changed),
        &active.config,
        ConfigChange::ROUTING_SELECTED,
    )
    .await;

    answer("set_active_routing", &active.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveRule {
    routing_id: String,
    rule: RoutingRule,
}

pub(super) async fn save_rule(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveRule { routing_id, rule } = arguments("save_routing_rule", args)?;
    let saved = save_routing_rule_use_case(&state.config_mutations, routing_id, rule).await?;
    finish_config_change(
        state,
        "routing-rule-saved",
        invalidation::routing_scopes(saved.config_changed),
        &saved.config,
        ConfigChange::ROUTING_RULE_SAVED,
    )
    .await;

    answer("save_routing_rule", &saved.value)
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
    let deleted =
        delete_routing_rules_use_case(&state.config_mutations, routing_id, rule_ids).await?;
    finish_config_change(
        state,
        "routing-rules-deleted",
        invalidation::routing_scopes(deleted.config_changed),
        &deleted.config,
        ConfigChange::ROUTING_RULES_DELETED,
    )
    .await;

    answer("delete_routing_rules", &deleted.value)
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
    let moved = move_routing_rule_use_case(
        &state.config_mutations,
        routing_id,
        rule_id,
        action,
        position,
    )
    .await?;
    finish_config_change(
        state,
        "routing-rule-moved",
        invalidation::routing_scopes(moved.config_changed),
        &moved.config,
        ConfigChange::ROUTING_RULE_MOVED,
    )
    .await;

    answer("move_routing_rule", &moved.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetRules {
    routing_id: String,
}

pub(super) async fn reset_rules(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let ResetRules { routing_id } = arguments("reset_routing_rules", args)?;
    let reset = reset_routing_rules_use_case(&state.config_mutations, routing_id).await?;
    finish_config_change(
        state,
        "routing-rules-reset",
        invalidation::routing_scopes(reset.config_changed),
        &reset.config,
        ConfigChange::ROUTING_RULES_RESET,
    )
    .await;

    answer("reset_routing_rules", &reset.value)
}
