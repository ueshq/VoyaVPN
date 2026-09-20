//! Connect, disconnect, restart and the two status reads behind them.
//!
//! The choreography itself is `voya_app::core_flow`, unit-tested there and
//! shared with the Tauri shell; this module only turns the flow's domain events
//! into the transient channel and hands back the status.

use std::sync::Arc;

use serde_json::Value;
use voya_app::{
    contract_map::{runtime_status_event, runtime_status_response},
    core_flow::{CoreFlow, CoreFlowLevel, CoreFlowSink, CoreFlowState},
    runtime::RuntimeManager,
    supervisor::SupervisorSnapshot,
    sysproxy::SystemProxyManager,
    tun::{TunManager, TunStatus},
};
use voya_contracts::{
    AppError, AppNotice, AppNoticeLevel, CoreState, LogCode, LogLevel, NoticeCode,
};
use voya_platform::sysproxy::{SystemProxyService, SystemProxyStatus};

use crate::{
    app::MobileState,
    events::{AppEvent, EventChannel, TransientStreamEvent},
    sinks::HostSinks,
};

use super::answer;

pub(super) async fn connect(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();

    answer(
        "connect_active_profile",
        &runtime_status_response(core_flow(state).connect(&config).await?),
    )
}

pub(super) async fn disconnect(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();

    answer(
        "disconnect_core",
        &runtime_status_response(core_flow(state).disconnect(&config).await?),
    )
}

pub(super) async fn restart(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();

    answer(
        "restart_core",
        &runtime_status_response(core_flow(state).restart(&config).await?),
    )
}

pub(super) async fn status(state: &MobileState) -> Result<Value, AppError> {
    answer(
        "runtime_status",
        &runtime_status_response(runtime_manager(state).status().await?),
    )
}

pub(super) async fn tun_status(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();
    answer("tun_status", &tun_manager(state).status(&config)?)
}

/// Whether a speedtest run is in flight.
///
/// Always "no" until the probe core lands: nothing can start a run on a phone
/// yet (`run_speedtest` is on `NOT_YET_DISPATCHED`), and the frontend reads
/// this on every mount, so answering honestly beats refusing.
pub(super) async fn speedtest_status(state: &MobileState) -> Result<Value, AppError> {
    let _ = state;

    answer(
        "speedtest_status",
        &voya_contracts::SpeedtestStatus { running: false },
    )
}

/// The exit address of the running connection, looked up through its own proxy.
pub(super) async fn check_connection_ip(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();
    let snapshot = state.supervisor.status().await?;
    let exit = voya_app::connection_ip::check_connection_ip(&config, &snapshot).await?;

    answer(
        "check_connection_ip",
        &voya_contracts::ConnectionIpResult {
            country_code: exit.country_code,
            ip: exit.ip,
        },
    )
}

/// The tail every committed configuration change shares: announce the caches,
/// then restart a connected core for the change. The change is already
/// persisted by the time this runs, so a failed restart is a warning notice
/// rather than a command error.
pub(super) async fn finish_config_change(
    state: &MobileState,
    reason: &str,
    scopes: Vec<voya_contracts::InvalidationScope>,
    config: &voya_app::config_mutation::AppConfig,
    change: voya_app::post_commit::ConfigChange,
) {
    state.sinks.invalidate(reason, scopes);

    if let Err(error) = core_flow(state)
        .restart_if_connected(config, change.reason)
        .await
    {
        state.sinks.notice(
            AppNoticeLevel::Warning,
            change.restart_failed_code,
            Some(format!("{error:?}")),
        );
    }
}

/// Disconnects the core if the node or group it is running no longer exists.
///
/// Every path that can delete or replace the running target ends here, or the
/// core stays connected to a profile that is gone.
pub(super) async fn disconnect_removed_profile(state: &MobileState) -> Result<(), AppError> {
    let config = state.config_mutations.current_config();
    core_flow(state).disconnect_removed_profile(&config).await?;

    Ok(())
}

pub(super) fn core_flow(state: &MobileState) -> CoreFlow<'_> {
    CoreFlow::new(
        runtime_manager(state),
        system_proxy_manager(state),
        tun_manager(state),
        Arc::new(HostCoreFlowSink {
            sinks: Arc::clone(&state.sinks),
        }),
    )
}

fn runtime_manager(state: &MobileState) -> RuntimeManager<'_> {
    // No core seed directory: the tunnel provider ships its own Libbox and
    // never looks for an executable on disk.
    state.services.runtime(state.supervisor.clone())
}

fn tun_manager(state: &MobileState) -> TunManager {
    TunManager::new(Arc::clone(&state.elevation))
}

/// A manager over a service that will refuse, which is the honest shape: the
/// platform reports `SystemProxyManagement::Unsupported` for both phones, so
/// every path through `core_flow` skips the proxy before it reaches this.
fn system_proxy_manager(state: &MobileState) -> SystemProxyManager {
    SystemProxyManager::new(
        SystemProxyService::new(crate::app::no_process_runner()),
        state.services.runtime_paths().clone(),
    )
}

struct HostCoreFlowSink {
    sinks: Arc<HostSinks>,
}

impl CoreFlowSink for HostCoreFlowSink {
    fn log(&self, level: CoreFlowLevel, code: LogCode, detail: Option<&str>) {
        self.sinks
            .log(log_level(level), code, detail.map(str::to_string));
    }

    fn core_state(
        &self,
        state: CoreFlowState,
        active_profile_id: Option<String>,
        snapshot: Option<&SupervisorSnapshot>,
    ) {
        self.sinks.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::CoreState(runtime_status_event(
                core_state_event_kind(state),
                active_profile_id,
                snapshot,
            )),
        );
    }

    fn system_proxy_changed(&self, status: &SystemProxyStatus) {
        self.sinks.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::SysProxyChanged(
                voya_app::contract_map::system_proxy_status_to_contract(status.clone()),
            ),
        );
    }

    fn tun_changed(&self, status: &TunStatus) {
        self.sinks.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::TunChanged(status.clone()),
        );
    }

    fn statistics_zero(&self) {
        self.sinks.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::Statistics(zero_statistics()),
        );
    }

    fn notice(&self, level: CoreFlowLevel, code: NoticeCode, detail: &str) {
        self.sinks.emit(
            EventChannel::App,
            &AppEvent::Notice(AppNotice {
                code,
                detail: Some(detail.to_string()),
                level: notice_level(level),
            }),
        );
    }
}

const fn log_level(level: CoreFlowLevel) -> LogLevel {
    match level {
        CoreFlowLevel::Info => LogLevel::Info,
        CoreFlowLevel::Warn => LogLevel::Warn,
        CoreFlowLevel::Error => LogLevel::Error,
    }
}

const fn notice_level(level: CoreFlowLevel) -> AppNoticeLevel {
    match level {
        CoreFlowLevel::Info => AppNoticeLevel::Info,
        CoreFlowLevel::Warn => AppNoticeLevel::Warning,
        CoreFlowLevel::Error => AppNoticeLevel::Error,
    }
}

const fn core_state_event_kind(state: CoreFlowState) -> CoreState {
    match state {
        CoreFlowState::CleanupPending => CoreState::CleanupPending,
        CoreFlowState::Connecting => CoreState::Connecting,
        CoreFlowState::Connected => CoreState::Connected,
        CoreFlowState::Disconnecting => CoreState::Disconnecting,
        CoreFlowState::Disconnected => CoreState::Disconnected,
    }
}

/// A sample with nothing flowing, published when a core stops so the live
/// rates fall to zero rather than freezing at the last reading.
fn zero_statistics() -> voya_contracts::StatisticsSnapshot {
    voya_contracts::StatisticsSnapshot {
        active_profile_id: None,
        proxy_upload_bytes_per_second: 0.0,
        proxy_download_bytes_per_second: 0.0,
        direct_upload_bytes_per_second: 0.0,
        direct_download_bytes_per_second: 0.0,
        upload_bytes_per_second: 0.0,
        download_bytes_per_second: 0.0,
        server_stat: None,
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TunEnabled {
    enabled: bool,
}

pub(super) async fn set_tun_enabled(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let TunEnabled { enabled } = super::arguments("set_tun_enabled", args)?;
    let planned = state
        .config_mutations
        .mutate(async |_unit_of_work, config| -> Result<_, AppError> {
            let status = tun_manager(state).plan_set_enabled(config, enabled)?;
            TunManager::apply_enabled(config, enabled);
            Ok(status)
        })
        .await?;

    state.sinks.emit(
        EventChannel::TransientStream,
        &TransientStreamEvent::TunChanged(planned.value.clone()),
    );
    // `enable_tun` is committed, and the settings bundle mirrors it; without
    // this a stale bundle would rewrite the flag back on the next save.
    finish_config_change(
        state,
        "tun-enabled-changed",
        voya_app::invalidation::connection_mode_scopes(),
        &planned.config,
        voya_app::post_commit::ConfigChange::TUN,
    )
    .await;

    answer("set_tun_enabled", &planned.value)
}

pub(super) async fn connection_mode_status(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();
    let status = tun_manager(state).status(&config)?;

    answer(
        "connection_mode_status",
        &voya_app::connection_mode::connection_mode_status(&config, &status),
    )
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetMode {
    mode: voya_contracts::ConnectionMode,
}

/// A phone captures traffic only through its tunnel provider, so the manager
/// refuses to leave VPN mode here exactly as it does on macOS. The command
/// stays wired because the frontend shares one settings surface.
pub(super) async fn set_connection_mode(
    state: &MobileState,
    args: &Value,
) -> Result<Value, AppError> {
    let SetMode { mode } = super::arguments("set_connection_mode", args)?;
    let connected = state.supervisor.status().await?.state;
    let outcome = voya_app::connection_mode::ConnectionModeManager::new(
        system_proxy_manager(state),
        tun_manager(state),
        Arc::new(HostConnectionModeSink {
            sinks: Arc::clone(&state.sinks),
        }),
    )
    .set_connection_mode(&state.config_mutations, mode, connected)
    .await?;

    state.sinks.invalidate(
        "connection-mode-changed",
        voya_app::invalidation::connection_mode_scopes(),
    );

    if outcome.tun_flag_changed {
        finish_config_change(
            state,
            "connection-mode-restart",
            Vec::new(),
            &outcome.config,
            voya_app::post_commit::ConfigChange::CONNECTION_MODE,
        )
        .await;
    }

    answer("set_connection_mode", &outcome.status)
}

struct HostConnectionModeSink {
    sinks: Arc<HostSinks>,
}

impl voya_app::connection_mode::ConnectionModeSink for HostConnectionModeSink {
    fn system_proxy_changed(&self, status: &SystemProxyStatus) {
        self.sinks.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::SysProxyChanged(
                voya_app::contract_map::system_proxy_status_to_contract(status.clone()),
            ),
        );
    }

    fn tun_changed(&self, status: &TunStatus) {
        self.sinks.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::TunChanged(status.clone()),
        );
    }

    fn tray_refresh(&self) {
        // No tray on a phone; the tab bar is rendered from the same stores the
        // events above already move.
    }
}
