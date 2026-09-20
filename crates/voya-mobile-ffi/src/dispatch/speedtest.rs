//! Latency tests, run and cancelled.
//!
//! The run itself is `voya_app::speedtest`, shared with the Tauri shell. What
//! differs here is only where the probe core comes from: while disconnected
//! the host app starts one in its own process (`crate::probe`), and while
//! connected the manager measures through the provider's core instead.

use serde::Deserialize;
use serde_json::Value;
use voya_contracts::{AppError, LogCode, LogLevel, SpeedtestTarget};

use crate::{
    app::MobileState,
    events::{EventChannel, TransientStreamEvent},
};

use super::{answer, arguments};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunRequest {
    request: voya_contracts::SpeedtestRequest,
}

pub(super) async fn run(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let RunRequest { request } = arguments("run_speedtest", args)?;
    let SpeedtestTarget::Profiles {
        profile_ids: index_ids,
    } = request.target;
    let config = state.config_mutations.current_config();
    let sinks = state.sinks.clone();
    let result = state
        .services
        .run_speedtest(&state.speedtest, &config, index_ids, move |results| {
            // Results stream as each page finishes, so a long run fills the
            // node list in rather than landing all at once at the end.
            sinks.emit(
                EventChannel::TransientStream,
                &TransientStreamEvent::SpeedtestResults(results),
            );
        })
        .await?;

    // Measurements live on the node rows, so the list they came from is stale.
    state.sinks.invalidate(
        "speedtest-updated",
        voya_app::invalidation::profile_scopes(false),
    );

    answer("run_speedtest", &result)
}

pub(super) fn cancel(state: &MobileState) -> Result<Value, AppError> {
    if state.speedtest.cancel() {
        state.sinks.log(
            LogLevel::Info,
            LogCode::SpeedtestCancellationRequested,
            None,
        );
    }

    answer("cancel_speedtest", &state.speedtest.status())
}

pub(super) fn status(state: &MobileState) -> Result<Value, AppError> {
    answer("speedtest_status", &state.speedtest.status())
}
