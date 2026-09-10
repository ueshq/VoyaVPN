//! Thin Tauri adapter over `voya_app::core_flow`.
//!
//! The connect/restart/disconnect/recovery choreography lives in voya-app so it
//! can be unit-tested (the shell lib harness is disabled on purpose). All this
//! module does is turn the flow's domain events into `TransientStreamEvent`s
//! and log — never propagate — an emission failure: by the time the sink is
//! called the core has already started or stopped, so failing the command here
//! would report a lie.

use std::sync::Arc;

use voya_app::core_flow::{CoreFlow, CoreFlowLevel, CoreFlowSink, CoreFlowState};

use super::{lifecycle::*, support::*, *};

pub(crate) fn core_flow<'flow, R>(
    app: &tauri::AppHandle<R>,
    state: &'flow AppState,
) -> CoreFlow<'flow>
where
    R: tauri::Runtime + 'static,
{
    CoreFlow::new(
        runtime_manager(state),
        state.system_proxy_manager(),
        tun_manager(state),
        Arc::new(TauriCoreFlowSink { app: app.clone() }),
    )
}

struct TauriCoreFlowSink<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R> CoreFlowSink for TauriCoreFlowSink<R>
where
    R: tauri::Runtime + 'static,
{
    fn log(&self, level: CoreFlowLevel, code: LogCode, detail: Option<&str>) {
        if let Err(error) = emit_app_log(&self.app, log_level(level), code, detail) {
            tracing::warn!(?error, "failed to emit core flow log line");
        }
    }

    fn core_state(
        &self,
        state: CoreFlowState,
        active_profile_id: Option<String>,
        snapshot: Option<&SupervisorSnapshot>,
    ) {
        if let Err(error) = emit_core_state(
            &self.app,
            core_state_event_kind(state),
            active_profile_id,
            snapshot,
        ) {
            tracing::warn!(?error, "failed to emit core state");
        }
    }

    fn system_proxy_changed(&self, status: &SystemProxyStatus) {
        if let Err(error) = emit_sysproxy_changed(&self.app, status) {
            tracing::warn!(?error, "failed to emit system proxy state");
        }
    }

    fn tun_changed(&self, status: &TunStatus) {
        if let Err(error) = emit_tun_changed(&self.app, status) {
            tracing::warn!(?error, "failed to emit TUN status");
        }
    }

    fn statistics_zero(&self) {
        if let Err(error) = emit_statistics_zero(&self.app) {
            tracing::warn!(?error, "failed to emit zero statistics");
        }
    }

    fn notice(&self, level: CoreFlowLevel, code: NoticeCode, detail: &str) {
        report_post_commit_error(&self.app, code, detail, notice_level(level));
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
