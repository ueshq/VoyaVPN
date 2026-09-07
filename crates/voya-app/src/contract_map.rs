//! Explicit mappings between pure domain models and versioned public contracts.
//!
//! Keeping these translations in the orchestration layer prevents persistence
//! and IPC serialization concerns from leaking into `voya-core`.

mod data;
pub mod errors;

pub use data::*;
pub use errors::{certificate_error, core_info_error, database_error, input_text_error};

use voya_contracts::{
    GroupChildCandidate as GroupChildContract, GroupPreview as GroupPreviewContract,
    GroupPreviewRoute as GroupPreviewRouteContract, GroupValidation as GroupValidationContract,
    LoadStrategy, MoveAction, Profile as ProfileContract,
    ProfileDedupeResult as ProfileDedupeContract, ProfileKind, ProfileListEntry, ProfileMetrics,
    ProfileProtocol, ProfileSortKey, ProfileTraffic, ProfileTransport,
    ServerEndpoint as ContractServerEndpoint, TlsMode, TlsSettings,
};
use voya_core::{
    ConfigType, GroupChildCandidate, GroupPreview, MoveAction as CoreMoveAction, MultipleLoad,
    ProfileDedupeResult, ProfileItem, ProfileListItem, ProfileProtocol as CoreProfileProtocol,
    ProfileSortKey as CoreProfileSortKey, ProfileTransport as CoreProfileTransport,
    ServerEndpoint as CoreServerEndpoint, TlsMode as CoreTlsMode, TlsSettings as CoreTlsSettings,
};

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
) -> voya_contracts::SysProxyType {
    match value {
        voya_core::SysProxyType::ForcedClear => voya_contracts::SysProxyType::ForcedClear,
        voya_core::SysProxyType::ForcedChange => voya_contracts::SysProxyType::ForcedChange,
        voya_core::SysProxyType::Unchanged => voya_contracts::SysProxyType::Unchanged,
        voya_core::SysProxyType::Pac => voya_contracts::SysProxyType::Pac,
    }
}

#[must_use]
pub const fn sysproxy_type_from_contract(
    value: voya_contracts::SysProxyType,
) -> voya_core::SysProxyType {
    match value {
        voya_contracts::SysProxyType::ForcedClear => voya_core::SysProxyType::ForcedClear,
        voya_contracts::SysProxyType::ForcedChange => voya_core::SysProxyType::ForcedChange,
        voya_contracts::SysProxyType::Unchanged => voya_core::SysProxyType::Unchanged,
        voya_contracts::SysProxyType::Pac => voya_core::SysProxyType::Pac,
    }
}

#[must_use]
pub const fn traffic_mode_to_contract(
    value: voya_core::TrafficMode,
) -> voya_contracts::TrafficMode {
    match value {
        voya_core::TrafficMode::Rule => voya_contracts::TrafficMode::Rule,
        voya_core::TrafficMode::Global => voya_contracts::TrafficMode::Global,
        voya_core::TrafficMode::Direct => voya_contracts::TrafficMode::Direct,
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
        voya_contracts::TrafficMode::Direct => voya_core::TrafficMode::Direct,
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
/// supervisor only ever reports the two terminal states; the transitions come
/// from [`runtime_status_event`].
#[must_use]
pub fn runtime_status_response(
    snapshot: crate::supervisor::SupervisorSnapshot,
) -> voya_contracts::RuntimeStatusResponse {
    voya_contracts::RuntimeStatusResponse {
        state: match snapshot.state {
            crate::supervisor::SupervisorConnectionState::Disconnected => {
                voya_contracts::CoreState::Disconnected
            }
            crate::supervisor::SupervisorConnectionState::Connected => {
                voya_contracts::CoreState::Connected
            }
        },
        active_profile_id: snapshot.active_profile_id,
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
        state,
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
pub fn profile_to_contract(item: ProfileItem) -> ProfileContract {
    ProfileContract {
        id: item.index_id,
        subscription_id: item.subscription_id,
        display_log: item.display_log,
        remarks: item.remarks,
        protocol: protocol_to_contract(item.protocol),
        transport: item.transport.map(transport_to_contract),
        tls: item.tls.map(tls_to_contract),
    }
}

#[must_use]
pub fn profile_from_contract(profile: ProfileContract) -> ProfileItem {
    ProfileItem {
        index_id: profile.id,
        subscription_id: profile.subscription_id,
        display_log: profile.display_log,
        remarks: profile.remarks,
        protocol: protocol_from_contract(profile.protocol),
        transport: profile.transport.map(transport_from_contract),
        tls: profile.tls.map(tls_from_contract),
    }
}

#[must_use]
pub fn profile_list_to_contract(item: ProfileListItem) -> ProfileListEntry {
    ProfileListEntry {
        profile: profile_to_contract(item.profile),
        metrics: ProfileMetrics {
            delay_ms: item.profile_ex.delay,
            speed_bytes_per_second: item.profile_ex.speed,
            sort: item.profile_ex.sort,
            message: item.profile_ex.message,
            ip_info: item.profile_ex.ip_info,
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

#[must_use]
pub fn profile_dedupe_to_contract(result: ProfileDedupeResult) -> ProfileDedupeContract {
    ProfileDedupeContract {
        total: result.total,
        kept: result.kept,
        removed_profile_ids: result.removed_index_ids,
    }
}

#[must_use]
pub const fn profile_sort_key_from_contract(key: ProfileSortKey) -> CoreProfileSortKey {
    match key {
        ProfileSortKey::Sort => CoreProfileSortKey::Sort,
        ProfileSortKey::Protocol => CoreProfileSortKey::ConfigType,
        ProfileSortKey::Remarks => CoreProfileSortKey::Remarks,
        ProfileSortKey::Address => CoreProfileSortKey::Address,
        ProfileSortKey::Port => CoreProfileSortKey::Port,
        ProfileSortKey::Transport => CoreProfileSortKey::Network,
        ProfileSortKey::Tls => CoreProfileSortKey::StreamSecurity,
        ProfileSortKey::Delay => CoreProfileSortKey::Delay,
        ProfileSortKey::Speed => CoreProfileSortKey::Speed,
        ProfileSortKey::IpInfo => CoreProfileSortKey::IpInfo,
        ProfileSortKey::SubscriptionId => CoreProfileSortKey::SubscriptionId,
    }
}

#[must_use]
pub fn group_child_to_contract(item: GroupChildCandidate) -> GroupChildContract {
    GroupChildContract {
        profile_id: item.index_id,
        remarks: item.remarks,
        address: item.address,
        protocol: profile_kind(item.config_type),
        subscription_id: item.subscription_id,
        is_group: item.is_group,
        selectable: item.selectable,
        reason: item.reason,
    }
}

#[must_use]
pub fn group_preview_to_contract(preview: GroupPreview) -> GroupPreviewContract {
    GroupPreviewContract {
        validation: GroupValidationContract {
            valid: preview.validation.valid,
            child_profile_ids: preview.validation.child_index_ids,
            errors: preview.validation.errors,
            warnings: preview.validation.warnings,
        },
        singbox_routes: preview
            .singbox_routes
            .into_iter()
            .map(|route| GroupPreviewRouteContract {
                tag: route.tag,
                kind: route.kind,
                dialer_proxy: route.dialer_proxy,
                download_dialer_proxy: route.download_dialer_proxy,
                detour: route.detour,
                outbounds: route.outbounds,
            })
            .collect(),
    }
}

fn protocol_to_contract(protocol: CoreProfileProtocol) -> ProfileProtocol {
    match protocol {
        CoreProfileProtocol::Vmess {
            server,
            uuid,
            cipher,
        } => ProfileProtocol::Vmess {
            server: server_to_contract(server),
            uuid,
            cipher,
        },
        CoreProfileProtocol::Custom { source, filter } => {
            ProfileProtocol::Custom { source, filter }
        }
        CoreProfileProtocol::Shadowsocks {
            server,
            password,
            method,
            udp_over_tcp,
        } => ProfileProtocol::Shadowsocks {
            server: server_to_contract(server),
            password,
            method,
            udp_over_tcp,
        },
        CoreProfileProtocol::Socks {
            server,
            username,
            password,
        } => ProfileProtocol::Socks {
            server: server_to_contract(server),
            username,
            password,
        },
        CoreProfileProtocol::Vless {
            server,
            uuid,
            flow,
            encryption,
        } => ProfileProtocol::Vless {
            server: server_to_contract(server),
            uuid,
            flow,
            encryption,
        },
        CoreProfileProtocol::Trojan { server, password } => ProfileProtocol::Trojan {
            server: server_to_contract(server),
            password,
        },
        CoreProfileProtocol::Hysteria2 {
            server,
            password,
            port_hops,
            obfuscation_password,
        } => ProfileProtocol::Hysteria2 {
            server: server_to_contract(server),
            password,
            port_hops,
            obfuscation_password,
        },
        CoreProfileProtocol::Tuic {
            server,
            uuid,
            password,
            congestion_control,
        } => ProfileProtocol::Tuic {
            server: server_to_contract(server),
            uuid,
            password,
            congestion_control,
        },
        CoreProfileProtocol::WireGuard {
            server,
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
        } => ProfileProtocol::WireGuard {
            server: server_to_contract(server),
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
        },
        CoreProfileProtocol::Http {
            server,
            username,
            password,
        } => ProfileProtocol::Http {
            server: server_to_contract(server),
            username,
            password,
        },
        CoreProfileProtocol::Anytls { server, password } => ProfileProtocol::Anytls {
            server: server_to_contract(server),
            password,
        },
        CoreProfileProtocol::Naive {
            server,
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
        } => ProfileProtocol::Naive {
            server: server_to_contract(server),
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
        },
        CoreProfileProtocol::PolicyGroup {
            child_profile_ids,
            source_subscription_id,
            filter,
            strategy,
        } => ProfileProtocol::PolicyGroup {
            child_profile_ids,
            source_subscription_id,
            filter,
            strategy: load_strategy(strategy),
        },
        CoreProfileProtocol::ProxyChain { child_profile_ids } => {
            ProfileProtocol::ProxyChain { child_profile_ids }
        }
    }
}

fn protocol_from_contract(protocol: ProfileProtocol) -> CoreProfileProtocol {
    match protocol {
        ProfileProtocol::Vmess {
            server,
            uuid,
            cipher,
        } => CoreProfileProtocol::Vmess {
            server: server_from_contract(server),
            uuid,
            cipher,
        },
        ProfileProtocol::Custom { source, filter } => {
            CoreProfileProtocol::Custom { source, filter }
        }
        ProfileProtocol::Shadowsocks {
            server,
            password,
            method,
            udp_over_tcp,
        } => CoreProfileProtocol::Shadowsocks {
            server: server_from_contract(server),
            password,
            method,
            udp_over_tcp,
        },
        ProfileProtocol::Socks {
            server,
            username,
            password,
        } => CoreProfileProtocol::Socks {
            server: server_from_contract(server),
            username,
            password,
        },
        ProfileProtocol::Vless {
            server,
            uuid,
            flow,
            encryption,
        } => CoreProfileProtocol::Vless {
            server: server_from_contract(server),
            uuid,
            flow,
            encryption,
        },
        ProfileProtocol::Trojan { server, password } => CoreProfileProtocol::Trojan {
            server: server_from_contract(server),
            password,
        },
        ProfileProtocol::Hysteria2 {
            server,
            password,
            port_hops,
            obfuscation_password,
        } => CoreProfileProtocol::Hysteria2 {
            server: server_from_contract(server),
            password,
            port_hops,
            obfuscation_password,
        },
        ProfileProtocol::Tuic {
            server,
            uuid,
            password,
            congestion_control,
        } => CoreProfileProtocol::Tuic {
            server: server_from_contract(server),
            uuid,
            password,
            congestion_control,
        },
        ProfileProtocol::WireGuard {
            server,
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
        } => CoreProfileProtocol::WireGuard {
            server: server_from_contract(server),
            private_key,
            peer_public_key,
            preshared_key,
            interface_address,
            allowed_ips,
            reserved,
            mtu,
        },
        ProfileProtocol::Http {
            server,
            username,
            password,
        } => CoreProfileProtocol::Http {
            server: server_from_contract(server),
            username,
            password,
        },
        ProfileProtocol::Anytls { server, password } => CoreProfileProtocol::Anytls {
            server: server_from_contract(server),
            password,
        },
        ProfileProtocol::Naive {
            server,
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
        } => CoreProfileProtocol::Naive {
            server: server_from_contract(server),
            username,
            password,
            quic,
            congestion_control,
            insecure_concurrency,
            udp_over_tcp,
        },
        ProfileProtocol::PolicyGroup {
            child_profile_ids,
            source_subscription_id,
            filter,
            strategy,
        } => CoreProfileProtocol::PolicyGroup {
            child_profile_ids,
            source_subscription_id,
            filter,
            strategy: load_strategy_from_contract(strategy),
        },
        ProfileProtocol::ProxyChain { child_profile_ids } => {
            CoreProfileProtocol::ProxyChain { child_profile_ids }
        }
    }
}

fn transport_to_contract(transport: CoreProfileTransport) -> ProfileTransport {
    match transport {
        CoreProfileTransport::Tcp { header, host, path } => {
            ProfileTransport::Tcp { header, host, path }
        }
        CoreProfileTransport::Kcp { header, seed, mtu } => {
            ProfileTransport::Kcp { header, seed, mtu }
        }
        CoreProfileTransport::Websocket { host, path } => {
            ProfileTransport::Websocket { host, path }
        }
        CoreProfileTransport::HttpUpgrade { host, path } => {
            ProfileTransport::HttpUpgrade { host, path }
        }
        CoreProfileTransport::Xhttp {
            host,
            path,
            mode,
            extra,
        } => ProfileTransport::Xhttp {
            host,
            path,
            mode,
            extra,
        },
        CoreProfileTransport::Http2 { host, path } => ProfileTransport::Http2 { host, path },
        CoreProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        } => ProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        },
        CoreProfileTransport::Quic { host, path } => ProfileTransport::Quic { host, path },
    }
}

fn transport_from_contract(transport: ProfileTransport) -> CoreProfileTransport {
    match transport {
        ProfileTransport::Tcp { header, host, path } => {
            CoreProfileTransport::Tcp { header, host, path }
        }
        ProfileTransport::Kcp { header, seed, mtu } => {
            CoreProfileTransport::Kcp { header, seed, mtu }
        }
        ProfileTransport::Websocket { host, path } => {
            CoreProfileTransport::Websocket { host, path }
        }
        ProfileTransport::HttpUpgrade { host, path } => {
            CoreProfileTransport::HttpUpgrade { host, path }
        }
        ProfileTransport::Xhttp {
            host,
            path,
            mode,
            extra,
        } => CoreProfileTransport::Xhttp {
            host,
            path,
            mode,
            extra,
        },
        ProfileTransport::Http2 { host, path } => CoreProfileTransport::Http2 { host, path },
        ProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        } => CoreProfileTransport::Grpc {
            authority,
            service_name,
            mode,
        },
        ProfileTransport::Quic { host, path } => CoreProfileTransport::Quic { host, path },
    }
}

fn tls_to_contract(tls: CoreTlsSettings) -> TlsSettings {
    TlsSettings {
        mode: match tls.mode {
            CoreTlsMode::Tls => TlsMode::Tls,
            CoreTlsMode::Reality => TlsMode::Reality,
        },
        server_name: tls.server_name,
        alpn: tls.alpn,
        reality_public_key: tls.reality_public_key,
        reality_short_id: tls.reality_short_id,
        reality_spider_x: tls.reality_spider_x,
        mldsa65_verify: tls.mldsa65_verify,
        certificate_pem: tls.certificate_pem,
        certificate_sha256: tls.certificate_sha256,
        ech_config: tls.ech_config,
        final_mask: tls.final_mask,
    }
}

fn tls_from_contract(tls: TlsSettings) -> CoreTlsSettings {
    CoreTlsSettings {
        mode: match tls.mode {
            TlsMode::Tls => CoreTlsMode::Tls,
            TlsMode::Reality => CoreTlsMode::Reality,
        },
        server_name: tls.server_name,
        alpn: tls.alpn,
        reality_public_key: tls.reality_public_key,
        reality_short_id: tls.reality_short_id,
        reality_spider_x: tls.reality_spider_x,
        mldsa65_verify: tls.mldsa65_verify,
        certificate_pem: tls.certificate_pem,
        certificate_sha256: tls.certificate_sha256,
        ech_config: tls.ech_config,
        final_mask: tls.final_mask,
    }
}

fn server_to_contract(server: CoreServerEndpoint) -> ContractServerEndpoint {
    ContractServerEndpoint {
        address: server.address,
        port: server.port,
    }
}

fn server_from_contract(server: ContractServerEndpoint) -> CoreServerEndpoint {
    CoreServerEndpoint {
        address: server.address,
        port: server.port,
    }
}

const fn profile_kind(config_type: ConfigType) -> ProfileKind {
    match config_type {
        ConfigType::VMess => ProfileKind::Vmess,
        ConfigType::Custom => ProfileKind::Custom,
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
        ConfigType::PolicyGroup => ProfileKind::PolicyGroup,
        ConfigType::ProxyChain => ProfileKind::ProxyChain,
    }
}

const fn load_strategy(value: MultipleLoad) -> LoadStrategy {
    match value {
        MultipleLoad::LeastPing => LoadStrategy::LeastPing,
        MultipleLoad::Fallback => LoadStrategy::Fallback,
        MultipleLoad::Random => LoadStrategy::Random,
        MultipleLoad::RoundRobin => LoadStrategy::RoundRobin,
        MultipleLoad::LeastLoad => LoadStrategy::LeastLoad,
    }
}

const fn load_strategy_from_contract(value: LoadStrategy) -> MultipleLoad {
    match value {
        LoadStrategy::LeastPing => MultipleLoad::LeastPing,
        LoadStrategy::Fallback => MultipleLoad::Fallback,
        LoadStrategy::Random => MultipleLoad::Random,
        LoadStrategy::RoundRobin => MultipleLoad::RoundRobin,
        LoadStrategy::LeastLoad => MultipleLoad::LeastLoad,
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
