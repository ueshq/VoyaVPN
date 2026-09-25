//! Policy groups: the saved definitions and the running group's live member.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    invalidation,
    policy_groups::{
        delete_policy_groups_use_case, save_policy_group_use_case, select_member_use_case,
        set_active_policy_group_use_case, test_running_policy_group_delay,
    },
    post_commit::ConfigChange,
};
use voya_contracts::{AppError, AppNoticeLevel, NoticeCode, PolicyGroup};

use crate::app::MobileState;

use super::{answer, arguments, runtime::finish_config_change};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveGroup {
    group: PolicyGroup,
}

pub(super) async fn save(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveGroup { group } = arguments("save_policy_group", args)?;
    let saved = save_policy_group_use_case(&state.config_mutations, group).await?;

    if saved.config.active_group_id == saved.value.id {
        // Editing the group traffic is going through changes the outbound the
        // core is using, so it restarts; editing any other one does not.
        finish_config_change(
            state,
            "policy-group-saved",
            invalidation::policy_group_scopes(saved.config_changed),
            &saved.config,
            ConfigChange::POLICY_GROUP,
        )
        .await;
    } else {
        state.sinks.invalidate(
            "policy-group-saved",
            invalidation::policy_group_scopes(saved.config_changed),
        );
    }

    answer("save_policy_group", &saved.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ids {
    ids: Vec<String>,
}

pub(super) async fn delete(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let Ids { ids } = arguments("delete_policy_groups", args)?;
    let deleted = delete_policy_groups_use_case(&state.config_mutations, ids).await?;
    state.sinks.invalidate(
        "policy-groups-deleted",
        invalidation::policy_group_scopes(deleted.config_changed),
    );
    // Deleting the group the core is running leaves it pointing at nothing.
    super::runtime::disconnect_removed_profile(state).await?;

    answer("delete_policy_groups", &deleted.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Id {
    id: String,
}

pub(super) async fn set_active(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let Id { id } = arguments("set_active_policy_group", args)?;
    let active = set_active_policy_group_use_case(&state.config_mutations, id).await?;
    state.sinks.invalidate(
        "active-policy-group-changed",
        invalidation::policy_group_scopes(true),
    );

    answer("set_active_policy_group", &active.value)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectMember {
    group_id: String,
    profile_id: String,
}

/// Stores a selector's member and, when that group is running, switches the
/// core to it live. The choice is kept even if the live switch fails.
pub(super) async fn select_member(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SelectMember {
        group_id,
        profile_id,
    } = arguments("select_policy_group_member", args)?;
    let (group, live_error) = select_member_use_case(
        &state.config_mutations,
        &state.supervisor,
        &state.services.policy_groups(),
        &state.proxy_runtime,
        &group_id,
        &profile_id,
    )
    .await?;
    if let Some(message) = live_error {
        state.sinks.notice(
            AppNoticeLevel::Warning,
            NoticeCode::PolicyGroupSelectionRuntimeUpdateFailed,
            Some(message),
        );
    }
    state.sinks.invalidate(
        "policy-group-member-selected",
        invalidation::policy_group_runtime_scopes(),
    );

    answer("select_policy_group_member", &group)
}

/// Probes every member of the running group through the core and returns the
/// group with those delays; `null` while no group runs.
pub(super) async fn test_delay(state: &MobileState) -> Result<Value, AppError> {
    let snapshot = state.supervisor.status().await?;
    let runtime = test_running_policy_group_delay(
        &snapshot,
        &state.services.policy_groups(),
        &state.proxy_runtime,
    )
    .await?;
    if runtime.is_some() {
        state.sinks.invalidate(
            "policy-group-delay-tested",
            invalidation::policy_group_runtime_scopes(),
        );
    }

    answer("test_policy_group_delay", &runtime)
}
