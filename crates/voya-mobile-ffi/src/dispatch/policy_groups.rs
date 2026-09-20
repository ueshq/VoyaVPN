//! Policy groups: the saved definitions and the running group's live member.

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    contract_map::{
        policy_group_from_contract, policy_group_runtime_to_contract, policy_group_to_contract,
    },
    invalidation,
    policy_groups::PolicyGroupManager,
    post_commit::ConfigChange,
    supervisor::{SupervisorConnectionState, SupervisorSnapshot},
};
use voya_contracts::{AppError, AppNoticeLevel, NoticeCode, PolicyGroup};

use crate::app::MobileState;

use super::{answer, arguments, runtime::finish_config_change};

/// How long a member's probe may take before it counts as unreachable.
const GROUP_DELAY_TIMEOUT_MS: u32 = 5_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveGroup {
    group: PolicyGroup,
}

pub(super) async fn save(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SaveGroup { group } = arguments("save_policy_group", args)?;
    let saved = state
        .config_mutations
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
            Ok(PolicyGroupManager::new_in(unit_of_work)
                .save(policy_group_from_contract(group))
                .await?)
        })
        .await?;

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

    answer("save_policy_group", &policy_group_to_contract(saved.value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ids {
    ids: Vec<String>,
}

pub(super) async fn delete(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let Ids { ids } = arguments("delete_policy_groups", args)?;
    let deleted = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(PolicyGroupManager::new_in(unit_of_work)
                .delete(config, &ids)
                .await?)
        })
        .await?;
    state.sinks.invalidate(
        "policy-groups-deleted",
        invalidation::policy_group_scopes(deleted.config_changed),
    );
    // Deleting the group the core is running leaves it pointing at nothing.
    super::runtime::disconnect_removed_profile(state).await?;

    answer(
        "delete_policy_groups",
        &u32::try_from(deleted.value).unwrap_or(u32::MAX),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Id {
    id: String,
}

pub(super) async fn set_active(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let Id { id } = arguments("set_active_policy_group", args)?;
    let active = state
        .config_mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(PolicyGroupManager::new_in(unit_of_work)
                .set_active(config, &id)
                .await?)
        })
        .await?;
    state.sinks.invalidate(
        "active-policy-group-changed",
        invalidation::policy_group_scopes(true),
    );

    answer(
        "set_active_policy_group",
        &policy_group_to_contract(active.value),
    )
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
    let selected = state
        .config_mutations
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
            Ok(PolicyGroupManager::new_in(unit_of_work)
                .select_member(&group_id, &profile_id)
                .await?)
        })
        .await?;

    let snapshot = state.supervisor.status().await?;
    if snapshot.state == SupervisorConnectionState::Connected
        && snapshot.active_group_id.as_deref() == Some(group_id.as_str())
    {
        let live = async {
            let (_, members) = state.services.policy_groups().resolve(&group_id).await?;
            state
                .proxy_runtime
                .select_group_member(&snapshot.clash_api_access(), &members, &profile_id)
                .await?;
            Ok::<_, AppError>(())
        }
        .await;

        if let Err(error) = live {
            state.sinks.notice(
                AppNoticeLevel::Warning,
                NoticeCode::PolicyGroupSelectionRuntimeUpdateFailed,
                Some(format!("{error:?}")),
            );
        }
    }
    state.sinks.invalidate(
        "policy-group-member-selected",
        invalidation::policy_group_runtime_scopes(),
    );

    answer(
        "select_policy_group_member",
        &policy_group_to_contract(selected.value),
    )
}

/// Probes every member of the running group through the core and returns the
/// group with those delays; `null` while no group runs.
pub(super) async fn test_delay(state: &MobileState) -> Result<Value, AppError> {
    let snapshot = state.supervisor.status().await?;
    let Some(group_id) = running_group_id(&snapshot) else {
        return Ok(Value::Null);
    };
    let (group, members) = state.services.policy_groups().resolve(&group_id).await?;
    let access = snapshot.clash_api_access();
    let delays = state
        .proxy_runtime
        .test_group_delay(
            &access,
            &members,
            voya_app::policy_groups::group_test_url(&group),
            GROUP_DELAY_TIMEOUT_MS,
        )
        .await?;
    let mut runtime = state.proxy_runtime.group_state(&access, &members).await?;
    for member in &mut runtime.members {
        if let Some(delay) = delays.get(&member.profile_id) {
            member.delay_ms = Some(*delay);
        }
    }
    state.sinks.invalidate(
        "policy-group-delay-tested",
        invalidation::policy_group_runtime_scopes(),
    );

    answer(
        "test_policy_group_delay",
        &Some(policy_group_runtime_to_contract(group_id, runtime)),
    )
}

pub(super) fn running_group_id(snapshot: &SupervisorSnapshot) -> Option<String> {
    snapshot
        .active_group_id
        .clone()
        .filter(|_| snapshot.state == SupervisorConnectionState::Connected)
}
