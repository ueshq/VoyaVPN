//! The network-activity screen: live connections, the traffic mode, and the
//! monitor that streams them.

use std::sync::Arc;

use serde::Deserialize;
use voya_app::{invalidation, proxy_runtime::report_monitor_result};
use voya_contracts::TrafficMode;

use crate::app::MobileState;

use super::{answer, arguments, Answer};

pub(super) async fn list_connections(state: &MobileState) -> Answer {
    let access = state.supervisor.clash_api_access().await;

    answer(
        "proxy_list_connections",
        &state.proxy_runtime.connections(&access).await?,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseConnection {
    connection_id: Option<String>,
}

pub(super) async fn close_connection(state: &MobileState, args: &str) -> Answer {
    let CloseConnection { connection_id } = arguments("proxy_close_connection", args)?;
    let snapshot = voya_app::proxy_runtime::close_connection_use_case(
        &state.proxy_runtime,
        &state.supervisor,
        connection_id,
    )
    .await?;
    state.sinks.invalidate(
        "proxy-connection-closed",
        invalidation::proxy_runtime_scopes(false),
    );

    answer("proxy_close_connection", &snapshot)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetTrafficMode {
    mode: TrafficMode,
}

pub(super) async fn set_traffic_mode(state: &MobileState, args: &str) -> Answer {
    let SetTrafficMode { mode } = arguments("proxy_set_traffic_mode", args)?;
    let change = state
        .services
        .change_traffic_mode(
            &state.config_mutations,
            &state.supervisor,
            &state.proxy_runtime,
            mode,
        )
        .await?;
    // The preference is already committed, including when a live step fails.
    state.sinks.invalidate(
        "proxy-traffic-mode-changed",
        invalidation::proxy_runtime_scopes(change.config_changed),
    );

    answer("proxy_set_traffic_mode", &change.applied?)
}

pub(super) async fn start_monitor(state: &MobileState) -> Answer {
    let result = voya_app::proxy_runtime::start_monitor_use_case(
        &state.proxy_monitor,
        &state.supervisor,
        Arc::clone(&state.sinks) as Arc<_>,
    )
    .await;

    answer(
        "proxy_start_monitor",
        &report_monitor_result(result, |status| {
            state.sinks.emit(
                crate::events::EventChannel::TransientStream,
                &crate::events::TransientStreamEvent::ProxyMonitorStatus(status.clone()),
            );
        })?,
    )
}

pub(super) async fn stop_monitor(state: &MobileState) -> Answer {
    let result = state.proxy_monitor.stop();

    answer(
        "proxy_stop_monitor",
        &report_monitor_result(result, |status| {
            state.sinks.emit(
                crate::events::EventChannel::TransientStream,
                &crate::events::TransientStreamEvent::ProxyMonitorStatus(status.clone()),
            );
        })?,
    )
}
