//! Connect, disconnect, restart and the two status reads behind them.
//!
//! The choreography itself is `voya_app::core_flow`, unit-tested there and
//! shared with the Tauri shell; this module only turns the flow's domain events
//! into the transient channel and hands back the status.

use std::sync::Arc;

use serde_json::Value;
use voya_app::{
    config_mutation::AppConfig,
    contract_map::{runtime_status_event, runtime_status_response},
    core_flow::{CoreFlow, CoreFlowSink},
    post_commit::{self, ConfigChange, PostCommitSink},
    runtime::RuntimeManager,
    supervisor::SupervisorSnapshot,
    tun::TunManager,
};
use voya_contracts::{
    AppError, AppNotice, AppNoticeLevel, CoreState, InvalidationScope, LogCode, LogLevel,
    NoticeCode, TunStatus,
};
use voya_platform::sysproxy::SystemProxyStatus;

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

/// The exit address of the running connection, looked up through its own proxy.
pub(super) async fn check_connection_ip(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();
    let snapshot = state.supervisor.status().await?;
    let exit = voya_app::connection_ip::check_connection_ip(&config, &snapshot).await?;

    answer("check_connection_ip", &exit)
}

/// The tail every committed configuration change shares. The choreography
/// itself is `voya_app::post_commit::finish_config_change`, shared with the
/// Tauri shell; this only supplies the host's sinks and core flow.
pub(super) async fn finish_config_change(
    state: &MobileState,
    reason: &str,
    scopes: voya_app::invalidation::InvalidationBundle,
    config: &AppConfig,
    change: ConfigChange,
) {
    post_commit::finish_config_change(
        &MobilePostCommitSink {
            sinks: Arc::clone(&state.sinks),
        },
        &core_flow(state),
        reason,
        &scopes.1,
        config,
        change,
    )
    .await;
}

struct MobilePostCommitSink {
    sinks: Arc<HostSinks>,
}

impl PostCommitSink for MobilePostCommitSink {
    fn invalidate(
        &self,
        reason: &str,
        scopes: &[InvalidationScope],
        refresh_failed_code: NoticeCode,
    ) {
        self.sinks
            .invalidate(reason, (refresh_failed_code, scopes.to_vec()));
    }

    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str) {
        self.sinks.notice(level, code, Some(detail.to_string()));
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
        state.system_proxy_manager.clone(),
        tun_manager(state),
        Arc::new(HostCoreFlowSink {
            sinks: Arc::clone(&state.sinks),
            state: state.this.get().cloned(),
        }),
    )
}

fn runtime_manager(state: &MobileState) -> RuntimeManager<'_> {
    // No core seed directory: the tunnel provider ships its own Libbox and
    // never looks for an executable on disk.
    state.services.runtime(state.supervisor.clone(), None)
}

/// Every TUN status the phone reports comes from the host's tunnel, the same
/// controller the supervisor starts it through.
fn tun_manager(state: &MobileState) -> TunManager {
    state
        .services
        .tun_manager(Arc::clone(&state.elevation), None)
        .with_native_tun_controller(Arc::clone(&state.native_tun))
}

struct HostCoreFlowSink {
    sinks: Arc<HostSinks>,
    /// For the background IPv6 egress check; `None` before the state is
    /// shared, when no core can be connected yet.
    state: Option<std::sync::Weak<MobileState>>,
}

impl CoreFlowSink for HostCoreFlowSink {
    fn log(&self, level: LogLevel, code: LogCode, detail: Option<&str>) {
        self.sinks.log(level, code, detail.map(str::to_string));
    }

    fn core_state(
        &self,
        state: CoreState,
        active_profile_id: Option<String>,
        snapshot: Option<&SupervisorSnapshot>,
    ) {
        self.sinks.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::CoreState(runtime_status_event(
                state,
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
            &TransientStreamEvent::Statistics(voya_app::statistics::zero_statistics_snapshot()),
        );
    }

    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str) {
        self.sinks.emit(
            EventChannel::App,
            &AppEvent::Notice(AppNotice {
                code,
                detail: Some(detail.to_string()),
                level,
            }),
        );
    }

    fn request_ipv6_egress_check(&self) {
        let Some(state) = self.state.as_ref().and_then(std::sync::Weak::upgrade) else {
            return;
        };
        // Called from a command running on the app's runtime. The probe takes
        // seconds and may reconnect, which needs the flow lock the settling
        // connect still holds: run it on its own flow, later.
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        runtime.spawn(async move {
            core_flow(&state)
                .check_ipv6_egress(|| state.config_mutations.current_config())
                .await;
        });
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
        ConfigChange::TUN,
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
        state.system_proxy_manager.clone(),
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
            (NoticeCode::ConnectionModeRefreshFailed, Vec::new()),
            &outcome.config,
            ConfigChange::CONNECTION_MODE,
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
