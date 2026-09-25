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
    CoreState, LogLevel, ProfileDetails, ProfileKind, ProfileMetrics, ProfileSummary,
    ProfileSummaryEntry, ProfileSummaryListing as ProfileSummaryListingContract, ProfileTraffic,
    SpeedtestOutcome,
};
use voya_core::{ConfigType, ProfileExItem, ProfileListItem};
use voya_platform::process::ProcessLogLevel;

use crate::profiles::{ProfileSummaryItem, ProfileSummaryListing};

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

/// A settled supervisor state in the contract's vocabulary.
///
/// A settled supervisor never reports a transition, so `Connecting` and
/// `Disconnecting` have no source here; they come from [`runtime_status_event`].
#[must_use]
pub const fn supervisor_state_to_contract(
    state: crate::supervisor::SupervisorConnectionState,
) -> CoreState {
    match state {
        crate::supervisor::SupervisorConnectionState::CleanupPending => CoreState::CleanupPending,
        crate::supervisor::SupervisorConnectionState::Disconnected => CoreState::Disconnected,
        crate::supervisor::SupervisorConnectionState::Connected => CoreState::Connected,
    }
}

/// Map a classified core log line onto the public log severity.
#[must_use]
pub const fn process_log_level_to_contract(level: ProcessLogLevel) -> LogLevel {
    match level {
        ProcessLogLevel::Trace => LogLevel::Trace,
        ProcessLogLevel::Debug => LogLevel::Debug,
        ProcessLogLevel::Info => LogLevel::Info,
        ProcessLogLevel::Warn => LogLevel::Warn,
        ProcessLogLevel::Error => LogLevel::Error,
    }
}

/// The supervisor's own snapshot, as the runtime status command returns it.
///
/// Pure, and read by three commands plus the tray, so it lives here rather than
/// in the shell where nothing can assert the state mapping. It is the event
/// below with the state read off the snapshot: the supervisor already clears the
/// duration and tunnel outside a confirmed connection, so the event's gating
/// changes nothing for a settled snapshot.
#[must_use]
pub fn runtime_status_response(
    snapshot: crate::supervisor::SupervisorSnapshot,
) -> voya_contracts::RuntimeStatusResponse {
    runtime_status_event(
        supervisor_state_to_contract(snapshot.state),
        None,
        Some(&snapshot),
    )
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
    state: CoreState,
    active_profile_id: Option<String>,
    snapshot: Option<&crate::supervisor::SupervisorSnapshot>,
) -> voya_contracts::RuntimeStatusResponse {
    voya_contracts::RuntimeStatusResponse {
        connected_duration_ms: snapshot
            .filter(|_| state == CoreState::Connected)
            .and_then(|snapshot| snapshot.connected_duration_ms),
        state,
        active_tun_backend: snapshot
            .filter(|_| state == CoreState::Connected)
            .and_then(|snapshot| snapshot.active_tun_backend)
            .map(crate::tun::tun_backend),
        active_profile_id: snapshot
            .and_then(|snapshot| snapshot.active_profile_id.clone())
            .or(active_profile_id),
        main_pid: snapshot.and_then(|snapshot| snapshot.main_pid),
        pre_pid: snapshot.and_then(|snapshot| snapshot.pre_pid),
    }
}

/// The outcome of copying a packaged core seed into app data.
#[must_use]
pub fn core_seed_install_result(
    outcome: voya_platform::coreinfo::CoreSeedCopyOutcome,
) -> voya_contracts::CoreSeedInstallResult {
    voya_contracts::CoreSeedInstallResult {
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
pub fn system_proxy_status_to_contract(
    status: voya_platform::sysproxy::SystemProxyStatus,
) -> voya_contracts::SystemProxyStatusResponse {
    voya_contracts::SystemProxyStatusResponse {
        management: match status.management {
            voya_platform::sysproxy::SystemProxyManagement::Automatic => {
                voya_contracts::SystemProxyManagement::Automatic
            }
            voya_platform::sysproxy::SystemProxyManagement::Unsupported => {
                voya_contracts::SystemProxyManagement::Unsupported
            }
        },
        requested_mode: sysproxy_type_to_contract(status.requested_type),
        effective_mode: sysproxy_type_to_contract(status.effective_type),
        proxy: status.proxy,
        exceptions: status.exceptions,
    }
}

#[must_use]
pub const fn profile_kind_to_contract(config_type: ConfigType) -> ProfileKind {
    match config_type {
        ConfigType::VMess => ProfileKind::Vmess,
        ConfigType::Shadowsocks => ProfileKind::Shadowsocks,
        ConfigType::SOCKS => ProfileKind::Socks,
        ConfigType::VLESS => ProfileKind::Vless,
        ConfigType::Trojan => ProfileKind::Trojan,
        ConfigType::Hysteria2 => ProfileKind::Hysteria2,
        ConfigType::TUIC => ProfileKind::Tuic,
        ConfigType::WireGuard => ProfileKind::WireGuard,
        ConfigType::HTTP => ProfileKind::Http,
        ConfigType::Anytls => ProfileKind::Anytls,
        ConfigType::Naive => ProfileKind::Naive,
    }
}

fn metrics_to_contract(profile_ex: ProfileExItem) -> ProfileMetrics {
    ProfileMetrics {
        delay_ms: profile_ex.delay,
        sort: profile_ex.sort,
        outcome: profile_ex
            .message
            .as_deref()
            .and_then(SpeedtestOutcome::from_stored),
        ip_info: profile_ex.ip_info,
        country_code: profile_ex.country_code,
    }
}

#[must_use]
pub fn profile_details_to_contract(item: ProfileListItem) -> ProfileDetails {
    ProfileDetails {
        profile: profile_to_contract(item.profile),
        metrics: metrics_to_contract(item.profile_ex),
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

#[must_use]
pub fn profile_summary_to_contract(item: ProfileSummaryItem) -> ProfileSummaryEntry {
    let profile = item.profile;
    ProfileSummaryEntry {
        profile: ProfileSummary {
            kind: profile_kind_to_contract(profile.config_type()),
            address: profile.address().to_owned(),
            port: profile.port(),
            id: profile.index_id,
            subscription_id: profile.subscription_id,
            remarks: profile.remarks,
        },
        metrics: metrics_to_contract(item.profile_ex),
        is_active: item.is_active,
    }
}

/// The listing plus the count of rows the storage layer had to skip.
///
/// `usize` saturates into the contract's `u32`: a count that large is already
/// nonsense to show, and clamping it keeps the listing renderable instead of
/// failing the command over a number the user only reads as "a lot".
#[must_use]
pub fn profile_summary_listing_to_contract(
    listing: ProfileSummaryListing,
) -> ProfileSummaryListingContract {
    ProfileSummaryListingContract {
        entries: listing
            .items
            .into_iter()
            .map(profile_summary_to_contract)
            .collect(),
        undecodable_profiles: u32::try_from(listing.undecodable_profiles).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
mod tests;
