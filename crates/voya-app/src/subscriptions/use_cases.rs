//! The contract-typed subscription use cases both hosts share.
//!
//! Each one checks its arguments, runs the mutation and returns what was
//! committed; a host keeps only its post-commit announcement.

use voya_contracts::{
    AppError, AppErrorSubsystem, ImportProfilesResult, Subscription, SubscriptionUpdateResult,
};
use voya_platform::coreinfo::TargetOs;

use super::SubscriptionManager;
use crate::{
    config_mutation::{CommittedMutation, ConfigMutationCoordinator},
    contract_map::{subscription_from_contract, subscription_to_contract},
    input_safety::{
        self, map_ipc_input, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS, IPC_PROXY_URL_MAX_CHARS,
    },
    services::AppServices,
    sysproxy::runtime_proxy_url,
};

const SUBSYSTEM: AppErrorSubsystem = AppErrorSubsystem::Subscription;

/// Saves a subscription row. It imports no nodes and leaves the persisted
/// config alone.
pub async fn save_subscription_use_case(
    mutations: &ConfigMutationCoordinator,
    item: Subscription,
) -> Result<CommittedMutation<Subscription>, AppError> {
    mutations
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
            Ok(subscription_to_contract(
                SubscriptionManager::new_in(unit_of_work)
                    .save_subscription(subscription_from_contract(item))
                    .await?,
            ))
        })
        .await
}

/// Deletes subscriptions and their nodes, which may include the running one.
pub async fn delete_subscriptions_use_case(
    mutations: &ConfigMutationCoordinator,
    ids: Vec<String>,
) -> Result<CommittedMutation<u32>, AppError> {
    map_ipc_input(
        input_safety::validate_text_list(&ids, IPC_ID_MAX_CHARS, IPC_LIST_MAX_ITEMS),
        "subscription id",
        SUBSYSTEM,
    )?;
    mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .delete_subscriptions(config, &ids)
                .await?)
        })
        .await
}

/// Imports share links, into a subscription or as manual nodes.
pub async fn import_profiles_use_case(
    mutations: &ConfigMutationCoordinator,
    text: String,
    subscription_id: Option<String>,
) -> Result<CommittedMutation<ImportProfilesResult>, AppError> {
    map_ipc_input(
        input_safety::validate_present_text(subscription_id.as_deref(), IPC_ID_MAX_CHARS),
        "subscription id",
        SUBSYSTEM,
    )?;
    mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .import_profiles_from_text(config, &text, subscription_id.as_deref())
                .await?)
        })
        .await
}

/// What updating subscriptions did.
pub struct SubscriptionUpdate {
    pub result: SubscriptionUpdateResult,
    /// `None` when nothing downloaded had nodes to import, so nothing was
    /// committed. Otherwise nodes were replaced (possibly the running one) and
    /// this says whether the persisted config changed too.
    pub config_changed: Option<bool>,
}

/// Downloads subscriptions, then imports what arrived.
///
/// The download happens outside the mutation: a network round trip must not
/// hold the config lock or a pooled database connection.
pub async fn update_subscriptions_use_case(
    services: &AppServices,
    mutations: &ConfigMutationCoordinator,
    subscription_id: Option<String>,
    prefer_proxy: bool,
    proxy_url: Option<String>,
    target_os: TargetOs,
) -> Result<SubscriptionUpdate, AppError> {
    map_ipc_input(
        input_safety::validate_present_text(subscription_id.as_deref(), IPC_ID_MAX_CHARS),
        "subscription id",
        SUBSYSTEM,
    )?;
    map_ipc_input(
        input_safety::validate_optional_text(proxy_url.as_deref(), IPC_PROXY_URL_MAX_CHARS),
        "proxy URL",
        SUBSYSTEM,
    )?;
    let snapshot = mutations.current_config();
    let proxy_url = runtime_proxy_url(prefer_proxy, proxy_url, &snapshot, target_os);
    let prepared = services
        .subscriptions()
        .prepare_subscription_update(
            subscription_id.as_deref(),
            prefer_proxy,
            proxy_url.as_deref(),
        )
        .await?;
    if !prepared.has_imports() {
        return Ok(SubscriptionUpdate {
            result: prepared.into_result(),
            config_changed: None,
        });
    }
    let updated = mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .apply_prepared_subscription_update(config, prepared)
                .await?)
        })
        .await?;
    Ok(SubscriptionUpdate {
        result: updated.value,
        config_changed: Some(updated.config_changed),
    })
}
