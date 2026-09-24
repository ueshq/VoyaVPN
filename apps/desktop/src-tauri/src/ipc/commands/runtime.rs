//! Thin Tauri adapter over `voya_app::core_flow`.
//!
//! The connect/restart/disconnect/recovery choreography lives in voya-app so it
//! can be unit-tested (the shell lib harness is disabled on purpose). All this
//! module does is turn the flow's domain events into `TransientStreamEvent`s
//! and log — never propagate — an emission failure: by the time the sink is
//! called the core has already started or stopped, so failing the command here
//! would report a lie.

use std::sync::Arc;

use voya_app::core_flow::{CoreFlow, CoreFlowSink};

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
    fn log(&self, level: LogLevel, code: LogCode, detail: Option<&str>) {
        emit_app_log(&self.app, level, code, detail);
    }

    fn core_state(
        &self,
        state: CoreState,
        active_profile_id: Option<String>,
        snapshot: Option<&SupervisorSnapshot>,
    ) {
        if let Err(error) = emit_core_state(&self.app, state, active_profile_id, snapshot) {
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

    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str) {
        // What the IPv6 check found is news, not a failed post-commit step.
        if matches!(
            code,
            NoticeCode::NodeIpv6Unsupported { .. } | NoticeCode::NodeIpv6Restored { .. }
        ) {
            tracing::info!(?code, "node IPv6 egress changed");
            emit_or_warn(
                &self.app,
                AppEvent::Notice(AppNotice {
                    level,
                    code,
                    detail: None,
                }),
                "IPv6 egress notice",
            );
            return;
        }
        report_post_commit_error(&self.app, code, detail, level);
    }

    fn request_ipv6_egress_check(&self) {
        let app = self.app.clone();
        // The probe takes seconds and may reconnect, which needs the flow lock
        // the settling connect still holds: run it on its own flow, later.
        tauri::async_runtime::spawn(async move {
            use tauri::Manager;

            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            core_flow(&app, &state)
                .check_ipv6_egress(|| state.config_mutations().current_config())
                .await;
        });
    }
}
