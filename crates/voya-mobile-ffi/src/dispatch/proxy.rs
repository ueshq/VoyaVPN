//! The network-activity screen: live connections, the traffic mode, and the
//! monitor that streams them.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;
use voya_app::{
    contract_map::{traffic_mode_from_contract, traffic_mode_to_contract},
    invalidation,
    supervisor::{ClashApiAccess, SupervisorSnapshot},
};
use voya_contracts::{AppError, ProxyMonitorStatus, TrafficMode, TrafficModeResponse};

use crate::app::MobileState;

use super::{answer, arguments};

pub(super) async fn list_connections(state: &MobileState) -> Result<Value, AppError> {
    let access = clash_api_access(state).await;

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

pub(super) async fn close_connection(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let CloseConnection { connection_id } = arguments("proxy_close_connection", args)?;
    let access = clash_api_access(state).await;
    let snapshot = state
        .proxy_runtime
        .close_connection(&access, connection_id.as_deref())
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

pub(super) async fn set_traffic_mode(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let SetTrafficMode { mode } = arguments("proxy_set_traffic_mode", args)?;
    let snapshot = state.supervisor.status().await?;
    let outcome = state
        .proxy_runtime
        .change_traffic_mode(
            &state.config_mutations,
            &snapshot,
            traffic_mode_from_contract(mode),
        )
        .await?;
    // The preference is already committed, including when a live step fails.
    state.services.acknowledge_traffic_mode(&snapshot, &outcome);
    state.sinks.invalidate(
        "proxy-traffic-mode-changed",
        invalidation::proxy_runtime_scopes(outcome.config_changed),
    );
    outcome.runtime_result?;

    answer(
        "proxy_set_traffic_mode",
        &TrafficModeResponse {
            mode: traffic_mode_to_contract(outcome.mode),
        },
    )
}

pub(super) async fn start_monitor(state: &MobileState) -> Result<Value, AppError> {
    let access = clash_api_access(state).await;
    let result = state
        .proxy_monitor
        .start(&access, Arc::clone(&state.sinks) as Arc<_>);

    answer("proxy_start_monitor", &report_monitor(state, result)?)
}

pub(super) async fn stop_monitor(state: &MobileState) -> Result<Value, AppError> {
    let result = state.proxy_monitor.stop();

    answer("proxy_stop_monitor", &report_monitor(state, result)?)
}

/// Announces where the monitor ended up, a failure included, and returns it.
fn report_monitor<E>(
    state: &MobileState,
    result: Result<ProxyMonitorStatus, E>,
) -> Result<ProxyMonitorStatus, AppError>
where
    AppError: From<E>,
    E: std::fmt::Display,
{
    let status = match &result {
        Ok(status) => status.clone(),
        Err(error) => ProxyMonitorStatus::failed(error.to_string()),
    };
    state.sinks.emit(
        crate::events::EventChannel::TransientStream,
        &crate::events::TransientStreamEvent::ProxyMonitorStatus(status.clone()),
    );

    result.map_err(AppError::from).map(|_| status)
}

/// How to reach the Clash API of the core that is actually running, if any.
///
/// The generated config decides both the port and the token it demands, so the
/// supervisor snapshot is the only authority for either. On a phone that API
/// lives inside the tunnel provider; loopback there is per device, not per
/// process, so the app reaches it exactly as macOS does.
async fn clash_api_access(state: &MobileState) -> ClashApiAccess {
    state
        .supervisor
        .status()
        .await
        .ok()
        .as_ref()
        .map(SupervisorSnapshot::clash_api_access)
        .unwrap_or_default()
}
