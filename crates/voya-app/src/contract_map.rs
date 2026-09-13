//! Explicit mappings between pure domain models and versioned public contracts.
//!
//! Keeping these translations in the orchestration layer prevents persistence
//! and IPC serialization concerns from leaking into `voya-core`.

mod data;
pub mod errors;
mod messages;
mod policy_groups;
mod profiles;

pub use data::*;
pub use errors::{core_info_error, database_error, input_text_error};
pub use messages::validation_issue_to_contract;
pub use policy_groups::{
    policy_group_entry_to_contract, policy_group_from_contract, policy_group_runtime_to_contract,
    policy_group_to_contract,
};
pub use profiles::{profile_from_contract, profile_to_contract};

use voya_contracts::{
    MoveAction, ProfileListEntry, ProfileListing as ProfileListingContract, ProfileMetrics,
    ProfileTraffic, SpeedtestOutcome,
};
use voya_core::{MoveAction as CoreMoveAction, ProfileListItem};

use crate::profiles::ProfileListing;

#[must_use]
pub const fn core_type_to_contract(_: voya_core::CoreType) -> voya_contracts::CoreType {
    voya_contracts::CoreType::SingBox
}

#[must_use]
pub const fn core_type_from_contract(_: voya_contracts::CoreType) -> voya_core::CoreType {
    voya_core::CoreType::sing_box
}

#[must_use]
pub const fn sysproxy_type_to_contract(
    value: voya_core::SysProxyType,
) -> voya_contracts::SystemProxyType {
    match value {
        voya_core::SysProxyType::ForcedClear => voya_contracts::SystemProxyType::ForcedClear,
        voya_core::SysProxyType::ForcedChange => voya_contracts::SystemProxyType::ForcedChange,
        voya_core::SysProxyType::Unchanged => voya_contracts::SystemProxyType::Unchanged,
    }
}

#[must_use]
pub const fn sysproxy_type_from_contract(
    value: voya_contracts::SystemProxyType,
) -> voya_core::SysProxyType {
    match value {
        voya_contracts::SystemProxyType::ForcedClear => voya_core::SysProxyType::ForcedClear,
        voya_contracts::SystemProxyType::ForcedChange => voya_core::SysProxyType::ForcedChange,
        voya_contracts::SystemProxyType::Unchanged => voya_core::SysProxyType::Unchanged,
    }
}

#[must_use]
pub const fn tls_fragment_mode_to_contract(
    value: voya_core::TlsFragmentMode,
) -> voya_contracts::TlsFragmentMode {
    match value {
        voya_core::TlsFragmentMode::Off => voya_contracts::TlsFragmentMode::Off,
        voya_core::TlsFragmentMode::TlsHello => voya_contracts::TlsFragmentMode::TlsHello,
        voya_core::TlsFragmentMode::Record => voya_contracts::TlsFragmentMode::Record,
    }
}

#[must_use]
pub const fn tls_fragment_mode_from_contract(
    value: voya_contracts::TlsFragmentMode,
) -> voya_core::TlsFragmentMode {
    match value {
        voya_contracts::TlsFragmentMode::Off => voya_core::TlsFragmentMode::Off,
        voya_contracts::TlsFragmentMode::TlsHello => voya_core::TlsFragmentMode::TlsHello,
        voya_contracts::TlsFragmentMode::Record => voya_core::TlsFragmentMode::Record,
    }
}

#[must_use]
pub const fn close_action_to_contract(
    value: voya_core::CloseAction,
) -> voya_contracts::CloseAction {
    match value {
        voya_core::CloseAction::MinimizeToTray => voya_contracts::CloseAction::MinimizeToTray,
        voya_core::CloseAction::Quit => voya_contracts::CloseAction::Quit,
        voya_core::CloseAction::Ask => voya_contracts::CloseAction::Ask,
    }
}

#[must_use]
pub const fn close_action_from_contract(
    value: voya_contracts::CloseAction,
) -> voya_core::CloseAction {
    match value {
        voya_contracts::CloseAction::MinimizeToTray => voya_core::CloseAction::MinimizeToTray,
        voya_contracts::CloseAction::Quit => voya_core::CloseAction::Quit,
        voya_contracts::CloseAction::Ask => voya_core::CloseAction::Ask,
    }
}

#[must_use]
pub const fn traffic_mode_to_contract(
    value: voya_core::TrafficMode,
) -> voya_contracts::TrafficMode {
    match value {
        voya_core::TrafficMode::Rule => voya_contracts::TrafficMode::Rule,
        voya_core::TrafficMode::Global => voya_contracts::TrafficMode::Global,
        voya_core::TrafficMode::Unchanged => voya_contracts::TrafficMode::Unchanged,
    }
}

#[must_use]
pub const fn traffic_mode_from_contract(
    value: voya_contracts::TrafficMode,
) -> voya_core::TrafficMode {
    match value {
        voya_contracts::TrafficMode::Rule => voya_core::TrafficMode::Rule,
        voya_contracts::TrafficMode::Global => voya_core::TrafficMode::Global,
        voya_contracts::TrafficMode::Unchanged => voya_core::TrafficMode::Unchanged,
    }
}

#[must_use]
pub fn process_candidate_to_contract(
    value: voya_platform::apps::ProcessCandidate,
) -> voya_contracts::ProcessCandidate {
    voya_contracts::ProcessCandidate {
        display_name: value.display_name,
        process_name: value.process_name,
        executable_path: value.executable_path,
        source: match value.source {
            voya_platform::apps::ProcessCandidateSource::RunningProcess => {
                voya_contracts::ProcessCandidateSource::RunningProcess
            }
            voya_platform::apps::ProcessCandidateSource::InstalledApplication => {
                voya_contracts::ProcessCandidateSource::InstalledApplication
            }
        },
    }
}

/// The supervisor's own snapshot, as the runtime status command returns it.
///
/// Pure, and read by three commands plus the tray, so it lives here rather than
/// in the shell where nothing can assert the state mapping. A settled
/// supervisor reports settled states, including pending cleanup; transitions come
/// from [`runtime_status_event`].
#[must_use]
pub fn runtime_status_response(
    snapshot: crate::supervisor::SupervisorSnapshot,
) -> voya_contracts::RuntimeStatusResponse {
    voya_contracts::RuntimeStatusResponse {
        connected_duration_ms: snapshot.connected_duration_ms,
        state: match snapshot.state {
            crate::supervisor::SupervisorConnectionState::CleanupPending => {
                voya_contracts::CoreState::CleanupPending
            }
            crate::supervisor::SupervisorConnectionState::Disconnected => {
                voya_contracts::CoreState::Disconnected
            }
            crate::supervisor::SupervisorConnectionState::Connected => {
                voya_contracts::CoreState::Connected
            }
        },
        active_profile_id: snapshot.active_profile_id,
        active_tun_backend: snapshot.active_tun_backend.map(crate::tun::tun_backend),
        main_pid: snapshot.main_pid,
        pre_pid: snapshot.pre_pid,
        running_core_type: snapshot.running_core_type.map(core_type_to_contract),
    }
}

/// The same status, announced mid-transition by the core flow.
///
/// `state` comes from the flow rather than the snapshot, because `Connecting`
/// and `Disconnecting` describe a supervisor that has not settled yet and so
/// cannot be read off one. `active_profile_id` prefers the snapshot and falls
/// back to the id the flow was invoked with, which is the only source while the
/// core is still starting.
#[must_use]
pub fn runtime_status_event(
    state: voya_contracts::CoreState,
    active_profile_id: Option<String>,
    snapshot: Option<&crate::supervisor::SupervisorSnapshot>,
) -> voya_contracts::RuntimeStatusResponse {
    voya_contracts::RuntimeStatusResponse {
        connected_duration_ms: snapshot
            .filter(|_| state == voya_contracts::CoreState::Connected)
            .and_then(|snapshot| snapshot.connected_duration_ms),
        state,
        active_tun_backend: snapshot
            .filter(|_| state == voya_contracts::CoreState::Connected)
            .and_then(|snapshot| snapshot.active_tun_backend)
            .map(crate::tun::tun_backend),
        active_profile_id: snapshot
            .and_then(|snapshot| snapshot.active_profile_id.clone())
            .or(active_profile_id),
        main_pid: snapshot.and_then(|snapshot| snapshot.main_pid),
        pre_pid: snapshot.and_then(|snapshot| snapshot.pre_pid),
        running_core_type: snapshot
            .and_then(|snapshot| snapshot.running_core_type)
            .map(core_type_to_contract),
    }
}

/// The outcome of copying a packaged core seed into app data.
#[must_use]
pub fn core_seed_install_result(
    outcome: voya_platform::coreinfo::CoreSeedCopyOutcome,
) -> voya_contracts::CoreSeedInstallResult {
    voya_contracts::CoreSeedInstallResult {
        core_type: core_type_to_contract(outcome.core_type),
        status: match outcome.status {
            voya_platform::coreinfo::CoreSeedCopyStatus::Copied => {
                voya_contracts::CoreSeedInstallStatus::Installed
            }
            voya_platform::coreinfo::CoreSeedCopyStatus::AlreadyInstalled => {
                voya_contracts::CoreSeedInstallStatus::AlreadyInstalled
            }
            voya_platform::coreinfo::CoreSeedCopyStatus::SeedMissing => {
                voya_contracts::CoreSeedInstallStatus::SeedMissing
            }
        },
        installed_files: outcome
            .copied_files
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
    }
}

#[must_use]
pub fn server_stat_to_contract(value: voya_core::ServerStatItem) -> voya_contracts::ServerStatItem {
    voya_contracts::ServerStatItem {
        index_id: value.index_id,
        total_up: value.total_up,
        total_down: value.total_down,
        today_up: value.today_up,
        today_down: value.today_down,
        date_now: value.date_now,
    }
}

#[must_use]
pub fn profile_list_to_contract(item: ProfileListItem) -> ProfileListEntry {
    ProfileListEntry {
        profile: profile_to_contract(item.profile),
        metrics: ProfileMetrics {
            delay_ms: item.profile_ex.delay,
            sort: item.profile_ex.sort,
            outcome: item
                .profile_ex
                .message
                .as_deref()
                .and_then(SpeedtestOutcome::from_stored),
            ip_info: item.profile_ex.ip_info,
            country_code: item.profile_ex.country_code,
        },
        traffic: ProfileTraffic {
            total_upload: item.server_stat.total_up,
            total_download: item.server_stat.total_down,
            today_upload: item.server_stat.today_up,
            today_download: item.server_stat.today_down,
            date: item.server_stat.date_now,
        },
        is_active: item.is_active,
    }
}

/// The listing plus the count of rows the storage layer had to skip.
///
/// `usize` saturates into the contract's `u32`: a count that large is already
/// nonsense to show, and clamping it keeps the listing renderable instead of
/// failing the command over a number the user only reads as "a lot".
#[must_use]
pub fn profile_listing_to_contract(listing: ProfileListing) -> ProfileListingContract {
    ProfileListingContract {
        entries: listing
            .items
            .into_iter()
            .map(profile_list_to_contract)
            .collect(),
        undecodable_profiles: u32::try_from(listing.undecodable_profiles).unwrap_or(u32::MAX),
    }
}

#[must_use]
pub const fn move_action_from_contract(action: MoveAction) -> CoreMoveAction {
    match action {
        MoveAction::Top => CoreMoveAction::Top,
        MoveAction::Up => CoreMoveAction::Up,
        MoveAction::Down => CoreMoveAction::Down,
        MoveAction::Bottom => CoreMoveAction::Bottom,
        MoveAction::Position => CoreMoveAction::Position,
    }
}

#[cfg(test)]
mod tests;
