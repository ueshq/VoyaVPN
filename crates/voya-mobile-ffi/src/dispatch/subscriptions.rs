//! Subscriptions and their reported usage.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    contract_map::{subscription_metadata_to_contract, subscription_to_contract},
    invalidation,
    subscriptions::{
        delete_subscriptions_use_case, save_subscription_use_case, update_subscriptions_use_case,
    },
};
use voya_contracts::AppError;
use voya_platform::coreinfo::TargetOs;

use crate::app::MobileState;

use super::{answer, arguments};

pub(super) async fn list(state: &MobileState) -> Result<Value, AppError> {
    let items = state.services.list_subscriptions().await?;

    answer(
        "list_subscriptions",
        &items
            .into_iter()
            .map(subscription_to_contract)
            .collect::<Vec<_>>(),
    )
}

pub(super) async fn list_metadata(state: &MobileState) -> Result<Value, AppError> {
    let items = state.services.list_subscription_metadata().await?;

    answer(
        "list_subscription_metadata",
        &items
            .into_iter()
            .map(subscription_metadata_to_contract)
            .collect::<Vec<_>>(),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateRequest {
    subscription_id: Option<String>,
    prefer_proxy: bool,
    proxy_url: Option<String>,
}

pub(super) async fn update(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let UpdateRequest {
        subscription_id,
        prefer_proxy,
        proxy_url,
    } = arguments("update_subscriptions", args)?;
    let update = update_subscriptions_use_case(
        &state.services,
        &state.config_mutations,
        subscription_id,
        prefer_proxy,
        proxy_url,
        TargetOs::current(),
    )
    .await?;
    if let Some(config_changed) = update.config_changed {
        state.sinks.invalidate(
            "subscriptions-updated",
            invalidation::subscription_scopes(true, config_changed),
        );
        // An update replaces the subscription's nodes, which may include the
        // running one.
        super::runtime::disconnect_removed_profile(state).await?;
    }

    answer("update_subscriptions", &update.result)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveSubscription {
    item: voya_contracts::Subscription,
}

pub(super) async fn save(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveSubscription { item } = arguments("save_subscription", args)?;
    let saved = save_subscription_use_case(&state.config_mutations, item).await?;
    // Saving writes the subscription row only: no node is imported and the
    // persisted config is untouched.
    state.sinks.invalidate(
        "subscription-saved",
        invalidation::subscription_scopes(false, false),
    );

    answer("save_subscription", &saved.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ids {
    ids: Vec<String>,
}

pub(super) async fn delete(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let Ids { ids } = arguments("delete_subscriptions", args)?;
    let deleted = delete_subscriptions_use_case(&state.config_mutations, ids).await?;
    state.sinks.invalidate(
        "subscriptions-deleted",
        invalidation::subscription_scopes(true, deleted.config_changed),
    );
    // Deleting a source deletes its nodes, which may include the running one.
    super::runtime::disconnect_removed_profile(state).await?;

    answer("delete_subscriptions", &deleted.value)
}
