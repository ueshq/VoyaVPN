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
    input_safety::{self, map_ipc_input, IPC_ID_MAX_CHARS, IPC_PROXY_URL_MAX_CHARS},
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
    input_safety::require_ids(&ids, "subscription id", SUBSYSTEM)?;
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
    pub written: SubscriptionWrite,
}

/// What an update wrote, which is what its host has to announce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionWrite {
    /// Nothing downloaded had nodes to import and no fetch failure was
    /// recorded.
    Nothing,
    /// Only fetch failures were recorded: the subscription list has to
    /// refresh, but no node changed, so the node caches and the running
    /// connection are left alone.
    FailuresOnly,
    /// Nodes were replaced — possibly the running one — and this says whether
    /// the persisted config changed too.
    Nodes { config_changed: bool },
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
        // Every fetch failed (or nothing was fetchable), so there is no
        // import to apply — but the failures still have to reach the
        // metadata table, or the list would keep claiming the subscription
        // was never updated. The manager writes through the pool, outside
        // the mutation, exactly like the fetches above.
        let failures = services
            .subscriptions()
            .persist_prepared_failures(&prepared)
            .await?;
        return Ok(SubscriptionUpdate {
            result: prepared.into_result(),
            written: if failures > 0 {
                SubscriptionWrite::FailuresOnly
            } else {
                SubscriptionWrite::Nothing
            },
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
        written: SubscriptionWrite::Nodes {
            config_changed: updated.config_changed,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use tempfile::TempDir;
    use voya_core::{AppConfig, SubItem};
    use voya_platform::paths::AppPaths;

    use super::*;

    /// A subscription pointed at a port nothing listens on fails during
    /// preparation, so the update takes the no-imports early return — the
    /// exact path that once dropped the failure instead of writing it to the
    /// metadata table.
    #[tokio::test]
    async fn failed_fetches_are_persisted_when_nothing_was_importable() {
        let app_dir = TempDir::new().expect("temp dir for the use-case test should exist");
        let services = AppServices::connect(
            &app_dir.path().join(voya_db::DATABASE_NAME),
            AppPaths::new(app_dir.path()),
        )
        .await
        .expect("test services should connect");
        let mutations = services.config_mutations(Arc::new(RwLock::new(AppConfig::default())));
        services
            .subscriptions()
            .save_subscription(SubItem {
                id: "dead".to_string(),
                remarks: "Dead source".to_string(),
                url: "http://127.0.0.1:1/sub".to_string(),
                ..SubItem::default()
            })
            .await
            .expect("subscription should save");

        let update = update_subscriptions_use_case(
            &services,
            &mutations,
            None,
            false,
            None,
            TargetOs::Linux,
        )
        .await
        .expect("update use case should succeed");

        assert_eq!(update.result.skipped, 1, "{:?}", update.result);
        assert_eq!(
            update.written,
            SubscriptionWrite::FailuresOnly,
            "metadata-only writes must still announce themselves so the list refreshes"
        );
        let metadata = services
            .list_subscription_metadata()
            .await
            .expect("metadata read should succeed")
            .into_iter()
            .find(|metadata| metadata.subscription_id == "dead")
            .expect("the failed attempt should have been persisted");
        assert_eq!(metadata.last_attempt_failed, Some(true));
        let diagnostic = metadata.last_attempt_error.clone();
        assert!(
            diagnostic.is_some_and(|error| !error.trim().is_empty()),
            "the redacted diagnostic should still say something: {metadata:?}"
        );
    }
}
