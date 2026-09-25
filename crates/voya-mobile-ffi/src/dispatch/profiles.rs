//! Nodes, their source groups and the policy groups over them.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    contract_map::{
        policy_group_entry_to_contract, profile_details_to_contract,
        profile_summary_listing_to_contract,
    },
    invalidation,
    profiles::{
        delete_profiles_use_case, move_profile_use_case, save_profile_use_case,
        set_active_profile_use_case,
    },
    subscriptions::{import_profiles_use_case, SubscriptionManager},
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
    let active = set_active_profile_use_case(&state.config_mutations, index_id).await?;
    // The active-node pointer lives in the persisted config, so the settings
    // bundle projected from it goes stale too.
    state
        .sinks
        .invalidate("active-profile-changed", invalidation::profile_scopes(true));

    answer("set_active_profile", &active.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportText {
    text: String,
    subscription_id: Option<String>,
}

pub(super) fn preview_import(args: &Value) -> Result<Value, AppError> {
    let ImportText { text, .. } = arguments("preview_import_profiles", args)?;
    answer(
        "preview_import_profiles",
        &SubscriptionManager::preview_import(&text)?,
    )
}

pub(super) async fn import_from_text(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let ImportText {
        text,
        subscription_id,
    } = arguments("import_profiles_from_text", args)?;
    let imported = import_profiles_use_case(&state.config_mutations, text, subscription_id).await?;
    state.sinks.invalidate(
        "profiles-imported",
        invalidation::subscription_scopes(true, imported.config_changed),
    );
    // Importing into a subscription replaces its nodes, which may include the
    // running one.
    super::runtime::disconnect_removed_profile(state).await?;

    answer("import_profiles_from_text", &imported.value)
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
    let runtime = voya_app::policy_groups::running_policy_group_runtime(
        &snapshot,
        &state.services.policy_groups(),
        &state.proxy_runtime,
    )
    .await?;

    answer("policy_group_runtime", &runtime)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileId {
    index_id: String,
}

pub(super) async fn get(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let ProfileId { index_id } = arguments("get_profile", args)?;
    let config = state.config_mutations.current_config();
    let details = state
        .services
        .profiles()
        .get_profile(&config, &index_id)
        .await?;

    answer("get_profile", &profile_details_to_contract(details))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveProfile {
    profile: voya_contracts::Profile,
}

pub(super) async fn save(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveProfile { profile } = arguments("save_profile", args)?;
    let saved = save_profile_use_case(&state.config_mutations, profile).await?;
    state.sinks.invalidate(
        "profile-saved",
        invalidation::profile_scopes(saved.config_changed),
    );

    answer("save_profile", &saved.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileIds {
    index_ids: Vec<String>,
}

pub(super) async fn delete(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let ProfileIds { index_ids } = arguments("delete_profiles", args)?;
    let deleted = delete_profiles_use_case(&state.config_mutations, index_ids).await?;
    state.sinks.invalidate(
        "profiles-deleted",
        invalidation::profile_scopes(deleted.config_changed),
    );
    // Deleting the node the core is running leaves it pointing at nothing.
    super::runtime::disconnect_removed_profile(state).await?;

    answer("delete_profiles", &deleted.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveProfile {
    subscription_id: Option<String>,
    index_id: String,
    action: voya_contracts::MoveAction,
    position: Option<i32>,
}

pub(super) async fn move_profile(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let MoveProfile {
        subscription_id,
        index_id,
        action,
        position,
    } = arguments("move_profile", args)?;
    move_profile_use_case(
        &state.config_mutations,
        subscription_id,
        index_id,
        action,
        position,
    )
    .await?;
    // Order lives in the profile rows, not in the persisted config.
    state
        .sinks
        .invalidate("profile-moved", invalidation::profile_scopes(false));

    Ok(Value::Null)
}

pub(super) async fn export_share_links(
    state: &MobileState,
    args: &Value,
) -> Result<Value, AppError> {
    let ProfileIds { index_ids } = arguments("export_profile_share_links", args)?;
    let config = state.config_mutations.current_config();

    answer(
        "export_profile_share_links",
        &state.services.export_profiles(&config, &index_ids).await?,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QrContent {
    content: String,
}

/// The share QR, rendered as SVG so the view scales it rather than a bitmap.
pub(super) fn generate_qr_code(args: &Value) -> Result<Value, AppError> {
    let QrContent { content } = arguments("generate_qr_code", args)?;

    answer("generate_qr_code", &voya_app::qr::generate_svg(&content)?)
}
