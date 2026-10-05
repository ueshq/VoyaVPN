//! Connect, disconnect, restart and the two status reads behind them.
//!
//! The choreography itself is `voya_app::core_flow`, unit-tested there and
//! shared with the Tauri shell; this module only turns the flow's domain events
//! into the transient channel and hands back the status.

use std::sync::Arc;

use voya_app::{
    config_mutation::AppConfig,
    connection_mode::ConnectionModeSink,
    contract_map::{runtime_status_event, runtime_status_response},
    core_flow::{CoreFlow, CoreFlowSink},
    post_commit::{self, ConfigChange, PostCommitSink},
    runtime::{RuntimeError, RuntimeManager},
    supervisor::{SupervisorError, SupervisorSnapshot},
    tun::TunManager,
};
use voya_contracts::{
    AppError, AppErrorKind, AppErrorSubsystem, AppNoticeLevel, CoreState, LogCode, LogLevel,
    NoticeCode, TunStatus,
};
use voya_platform::sysproxy::SystemProxyStatus;
use voya_platform::tun::NativeTunError;

use crate::{
    app::MobileState,
    events::{EventChannel, TransientStreamEvent},
    sinks::HostSinks,
};

use super::{answer, Answer};

pub(super) async fn connect(state: &MobileState) -> Answer {
    answer(
        "connect_active_profile",
        &runtime_status_response(
            core_flow(state)
                .connect(|| state.config_mutations.current_config())
                .await
                .map_err(declined_vpn_configuration)?,
        ),
    )
}

/// Turns a declined iOS VPN-configuration prompt into `elevationRequired`, so
/// the home screen offers its user-initiated "Authorize again" retry.
///
/// The shared `From<SupervisorError>` mapping deliberately does not do this
/// (`contract_map::errors` keeps that kind to two sources, so a declined
/// desktop privilege prompt never re-prompts). On iOS the system prompt is the
/// only path to a tunnel, and the retry is a tap, not an automatic re-prompt —
/// so the exception lives here, at the mobile shell, and nowhere else.
fn declined_vpn_configuration(error: RuntimeError) -> AppError {
    if let RuntimeError::Supervisor(SupervisorError::NativeTun(ref source)) = error {
        if matches!(source, NativeTunError::PermissionRequired { .. }) {
            return AppError::new(
                AppErrorSubsystem::Runtime,
                AppErrorKind::ElevationRequired,
                error.to_string(),
            );
        }
    }
    error.into()
}

pub(super) async fn disconnect(state: &MobileState) -> Answer {
    answer(
        "disconnect_core",
        &runtime_status_response(
            core_flow(state)
                .disconnect(|| state.config_mutations.current_config())
                .await?,
        ),
    )
}

pub(super) async fn restart(state: &MobileState) -> Answer {
    answer(
        "restart_core",
        &runtime_status_response(
            core_flow(state)
                .restart(|| state.config_mutations.current_config())
                .await
                .map_err(declined_vpn_configuration)?,
        ),
    )
}

pub(super) async fn status(state: &MobileState) -> Answer {
    answer(
        "runtime_status",
        &runtime_status_response(runtime_manager(state).status().await?),
    )
}

pub(super) async fn tun_status(state: &MobileState) -> Answer {
    let config = state.config_mutations.current_config();
    answer(
        "tun_status",
        &tun_manager(state).status_off_thread(&config).await?,
    )
}

/// The exit address of the running connection, looked up through its own proxy.
pub(super) async fn check_connection_ip(state: &MobileState) -> Answer {
    let config = state
        .services
        .running_core_config(&state.config_mutations.current_config());
    let connected = state.supervisor.is_connected();
    let exit = voya_app::connection_ip::check_connection_ip(&config, connected).await?;

    answer("check_connection_ip", &exit)
}

/// The tail every committed configuration change shares. The choreography
/// itself is `voya_app::post_commit::finish_config_change`, shared with the
/// Tauri shell; this only supplies the host's sinks and core flow.
pub(super) async fn finish_config_change(
    state: &MobileState,
    reason: &str,
    bundle: voya_app::invalidation::InvalidationBundle,
    config: &AppConfig,
    change: ConfigChange,
) {
    post_commit::finish_config_change(
        &MobilePostCommitSink {
            sinks: Arc::clone(&state.sinks),
        },
        &core_flow(state),
        reason,
        Some(bundle),
        config,
        change,
    )
    .await;
}

/// The restart-only tail, for a caller that already announced its caches.
async fn restart_after_config_change(
    state: &MobileState,
    config: &AppConfig,
    change: ConfigChange,
) {
    post_commit::finish_config_change(
        &MobilePostCommitSink {
            sinks: Arc::clone(&state.sinks),
        },
        &core_flow(state),
        "",
        None,
        config,
        change,
    )
    .await;
}

struct MobilePostCommitSink {
    sinks: Arc<HostSinks>,
}

impl PostCommitSink for MobilePostCommitSink {
    fn invalidate(&self, reason: &str, bundle: voya_app::invalidation::InvalidationBundle) {
        self.sinks.invalidate(reason, bundle);
    }

    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str) {
        self.sinks.notice(level, code, detail);
    }
}

/// Disconnects the core if the node or group it is running no longer exists.
///
/// Every path that can delete or replace the running target ends here, or the
/// core stays connected to a profile that is gone.
pub(super) async fn disconnect_removed_profile(state: &MobileState) -> Result<(), AppError> {
    core_flow(state)
        .disconnect_removed_profile(|| state.config_mutations.current_config())
        .await?;

    Ok(())
}

pub(super) fn core_flow(state: &MobileState) -> CoreFlow<'_> {
    CoreFlow::new(
        runtime_manager(state),
        state.system_proxy_manager.clone(),
        tun_manager(state),
        Arc::new(HostCoreFlowSink {
            sinks: Arc::clone(&state.sinks),
            state: std::sync::Weak::clone(&state.this),
        }),
        state.proxy_runtime.clone(),
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

/// What the supervisor calls when the core goes down without being asked.
///
/// The same recovery the desktop runs (`event_sinks.rs`): the core flow settles
/// the app into "disconnected" — the state event, zeroed statistics, the TUN
/// status — and raises the notice. Without it the phone kept showing
/// "connected" over a tunnel that was gone.
///
/// Both callbacks arrive on the supervisor's own actor, so the recovery runs
/// on a task of the app's runtime: it re-enters the runtime manager, and the
/// actor must stay free for the commands that recovery may issue.
pub(crate) struct SupervisorRecoverySink {
    state: std::sync::Weak<MobileState>,
    runtime: tokio::runtime::Handle,
}

impl SupervisorRecoverySink {
    pub(crate) const fn new(
        state: std::sync::Weak<MobileState>,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        Self { state, runtime }
    }
}

impl voya_app::supervisor::SupervisorEventSink for SupervisorRecoverySink {
    fn native_tun_exited(&self, event: voya_app::supervisor::NativeTunExitEvent) {
        // An app that is going away has nobody left to tell.
        let Some(state) = self.state.upgrade() else {
            return;
        };
        self.runtime.spawn(async move {
            core_flow(&state)
                .handle_native_tun_exit(|| state.config_mutations.current_config(), event)
                .await;
        });
    }

    fn core_exited(&self, event: voya_app::supervisor::CoreExitEvent) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        self.runtime.spawn(async move {
            core_flow(&state)
                .handle_core_exit(|| state.config_mutations.current_config(), event)
                .await;
        });
    }
}

struct HostCoreFlowSink {
    sinks: Arc<HostSinks>,
    /// For the background IPv6 egress check.
    state: std::sync::Weak<MobileState>,
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
        ConnectionModeSink::system_proxy_changed(&*self.sinks, status);
    }

    fn tun_changed(&self, status: &TunStatus) {
        ConnectionModeSink::tun_changed(&*self.sinks, status);
    }

    fn statistics_zero(&self) {
        self.sinks.emit(
            EventChannel::TransientStream,
            &TransientStreamEvent::Statistics(voya_app::statistics::zero_statistics_snapshot()),
        );
    }

    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str) {
        self.sinks.notice(level, code, detail);
    }

    fn request_ipv6_egress_check(&self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        // The probe takes seconds and may reconnect, which needs the flow lock
        // the settling connect still holds: run it on its own flow, later.
        let runtime = state.runtime.clone();
        runtime.spawn(async move {
            core_flow(&state)
                .check_ipv6_egress(|| state.config_mutations.current_config())
                .await;
        });
    }
}

pub(super) async fn connection_mode_status(state: &MobileState) -> Answer {
    let config = state.config_mutations.current_config();
    let status = tun_manager(state).status_off_thread(&config).await?;

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

/// A phone captures traffic only through its tunnel provider, so a request to
/// leave VPN mode commits nothing and answers with the mode that was kept. The
/// command stays wired because the frontend shares one settings surface.
pub(super) async fn set_connection_mode(state: &MobileState, args: &str) -> Answer {
    let SetMode { mode } = super::arguments("set_connection_mode", args)?;
    let connected = state.supervisor.status().await?.state;
    let outcome = voya_app::connection_mode::ConnectionModeManager::new(
        state.system_proxy_manager.clone(),
        tun_manager(state),
        Arc::clone(&state.sinks) as Arc<dyn ConnectionModeSink>,
    )
    .set_connection_mode(&state.config_mutations, mode, connected)
    .await?;

    state.sinks.invalidate(
        "connection-mode-changed",
        voya_app::invalidation::connection_mode_scopes(),
    );

    if outcome.tun_flag_changed {
        restart_after_config_change(state, &outcome.config, ConfigChange::CONNECTION_MODE).await;
    }

    answer("set_connection_mode", &outcome.status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use voya_platform::tun::TunBackend;

    #[test]
    fn a_declined_vpn_configuration_offers_authorize_again() {
        let error = RuntimeError::Supervisor(SupervisorError::NativeTun(
            NativeTunError::PermissionRequired {
                backend: TunBackend::IosPacketTunnel,
                message: "the system did not authorize the VPN configuration".to_string(),
            },
        ));

        let mapped = declined_vpn_configuration(error);
        assert_eq!(mapped.kind, AppErrorKind::ElevationRequired);
        assert_eq!(mapped.subsystem, AppErrorSubsystem::Runtime);
    }

    #[test]
    fn other_tun_failures_stay_internal() {
        let error = RuntimeError::Supervisor(SupervisorError::NativeTun(
            NativeTunError::ControllerUnavailable {
                backend: TunBackend::IosPacketTunnel,
                message: "provider crashed".to_string(),
            },
        ));

        assert_eq!(
            declined_vpn_configuration(error).kind,
            AppErrorKind::Internal
        );
    }
}
