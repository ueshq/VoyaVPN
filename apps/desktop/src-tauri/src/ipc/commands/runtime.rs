//! Thin Tauri adapter over `voya_app::core_flow`.
//!
//! The connect/restart/disconnect/recovery choreography lives in voya-app so it
//! can be unit-tested (the shell lib harness is disabled on purpose). All this
//! module does is turn the flow's domain events into `TransientStreamEvent`s
//! and log — never propagate — an emission failure: by the time the sink is
//! called the core has already started or stopped, so failing the command here
//! would report a lie.

use std::sync::Arc;

use voya_app::{
    contract_map::{core_flow_log_level, core_flow_notice_level, core_state_to_contract},
    core_flow::{CoreFlow, CoreFlowLevel, CoreFlowSink, CoreFlowState},
};

use super::{post_commit::*, support::*, *};

#[tauri::command]
#[specta::specta]
pub async fn connect_active_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = state.config_mutations().current_config();
    let flow = core_flow(&app, &state);

    flow.connect(&config)
        .await
        .map(runtime_status_response)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn disconnect_core<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = state.config_mutations().current_config();
    let flow = core_flow(&app, &state);

    flow.disconnect(&config)
        .await
        .map(runtime_status_response)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn restart_core<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    let config = state.config_mutations().current_config();
    let flow = core_flow(&app, &state);

    flow.restart(&config)
        .await
        .map(runtime_status_response)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_status(
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeStatusResponse, AppError> {
    runtime_manager(&state)
        .status()
        .await
        .map(runtime_status_response)
        .map_err(AppError::from)
}

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
        emit_app_log(&self.app, core_flow_log_level(level), code, detail);
    }

    fn core_state(
        &self,
        state: CoreFlowState,
        active_profile_id: Option<String>,
        snapshot: Option<&SupervisorSnapshot>,
    ) {
        if let Err(error) = emit_core_state(
            &self.app,
            core_state_to_contract(state),
            active_profile_id,
            snapshot,
        ) {
            tracing::warn!(?error, "failed to emit core state");
        }
        if let Err(error) = crate::refresh_tray_menu(&self.app) {
            tracing::warn!(?error, "failed to queue a tray menu refresh");
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
        report_post_commit_error(&self.app, code, detail, core_flow_notice_level(level));
    }
}
