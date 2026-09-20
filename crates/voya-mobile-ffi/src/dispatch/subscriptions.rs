//! Subscriptions and their reported usage.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    contract_map::{
        subscription_metadata_to_contract, subscription_to_contract,
        subscription_update_to_contract,
    },
    invalidation,
    subscriptions::SubscriptionManager,
    sysproxy::runtime_proxy_url,
};
use voya_contracts::AppError;
use voya_platform::coreinfo::TargetOs;

use crate::app::MobileState;

use super::{answer, arguments};

pub(super) async fn list(state: &MobileState) -> Result<Value, AppError> {
    let items = state.services.subscriptions().list_subscriptions().await?;

    answer(
        "list_subscriptions",
        &items
            .into_iter()
            .map(subscription_to_contract)
            .collect::<Vec<_>>(),
    )
}

pub(super) async fn list_metadata(state: &MobileState) -> Result<Value, AppError> {
    let items = state
        .services
        .subscriptions()
        .list_subscription_metadata()
        .await?;

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
    let snapshot = state.config_mutations.current_config();
    let proxy_url = runtime_proxy_url(prefer_proxy, proxy_url, &snapshot, TargetOs::current());
    // The download happens outside the mutation: a network round trip must not
    // hold the config lock or a pooled database connection.
    let prepared = state
        .services
        .subscriptions()
        .prepare_subscription_update(
            subscription_id.as_deref(),
            prefer_proxy,
            proxy_url.as_deref(),
        )
        .await?;
    if !prepared.has_imports() {
        return answer(
            "update_subscriptions",
            &subscription_update_to_contract(prepared.into_result()),
        );
    }

    let updated = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .apply_prepared_subscription_update(config, prepared)
                .await?)
        })
        .await?;
    state.sinks.invalidate(
        "subscriptions-updated",
        invalidation::subscription_scopes(true, updated.config_changed),
    );

    answer(
        "update_subscriptions",
        &subscription_update_to_contract(updated.value),
    )
}
