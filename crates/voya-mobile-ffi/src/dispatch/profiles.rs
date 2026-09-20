//! Nodes, their source groups and the policy groups over them.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    contract_map::{
        import_profiles_to_contract, policy_group_entry_to_contract,
        policy_group_runtime_to_contract, profile_details_to_contract,
        profile_summary_listing_to_contract,
    },
    invalidation,
    profiles::ProfileManager,
    proxy_runtime::ProxyRuntimeManager,
    subscriptions::SubscriptionManager,
    supervisor::SupervisorConnectionState,
};
use voya_contracts::{AppError, PolicyGroupListing};

use crate::app::MobileState;

use super::{answer, arguments};

pub(super) async fn list_summaries(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();
    let listing = state.services.profiles().list_summaries(&config).await?;

    answer(
        "list_profile_summaries",
        &profile_summary_listing_to_contract(listing),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActiveProfile {
    index_id: String,
}

pub(super) async fn set_active(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let ActiveProfile { index_id } = arguments("set_active_profile", args)?;
    let active = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(ProfileManager::new_in(unit_of_work)
                .set_active_profile(config, &index_id)
                .await?)
        })
        .await?;
    // The active-node pointer lives in the persisted config, so the settings
    // bundle projected from it goes stale too.
    state
        .sinks
        .invalidate("active-profile-changed", invalidation::profile_scopes(true));

    answer(
        "set_active_profile",
        &profile_details_to_contract(active.value),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportText {
    text: String,
    subscription_id: Option<String>,
}

pub(super) async fn import_from_text(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let ImportText {
        text,
        subscription_id,
    } = arguments("import_profiles_from_text", args)?;
    let imported = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(SubscriptionManager::new_in(unit_of_work)
                .import_profiles_from_text(config, &text, subscription_id.as_deref())
                .await?)
        })
        .await?;
    state.sinks.invalidate(
        "profiles-imported",
        invalidation::subscription_scopes(true, imported.config_changed),
    );

    answer(
        "import_profiles_from_text",
        &import_profiles_to_contract(imported.value),
    )
}

pub(super) async fn list_policy_groups(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();
    let entries = state.services.policy_groups().list(&config).await?;

    answer(
        "list_policy_groups",
        &PolicyGroupListing {
            entries: entries
                .into_iter()
                .map(policy_group_entry_to_contract)
                .collect(),
        },
    )
}

/// The running policy group as the core sees it, or `None` when no group is in
/// use. Reads the live member and each member's delay through the core's own
/// Clash API — on a phone that is the loopback inside the tunnel provider,
/// which is reachable per device rather than per process.
pub(super) async fn policy_group_runtime(state: &MobileState) -> Result<Value, AppError> {
    let snapshot = state.supervisor.status().await?;
    let Some(group_id) = snapshot
        .active_group_id
        .clone()
        .filter(|_| snapshot.state == SupervisorConnectionState::Connected)
    else {
        return Ok(Value::Null);
    };
    let (_, members) = state.services.policy_groups().resolve(&group_id).await?;
    let runtime = ProxyRuntimeManager::new()
        .group_state(&snapshot.clash_api_access(), &members)
        .await?;

    answer(
        "policy_group_runtime",
        &Some(policy_group_runtime_to_contract(group_id, runtime)),
    )
}
