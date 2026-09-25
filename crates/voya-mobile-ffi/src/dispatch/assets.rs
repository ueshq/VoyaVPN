//! The rule-library updater: the `.srs` rule sets.

use serde_json::Value;
use voya_app::sysproxy::runtime_proxy_url;
use voya_contracts::AppError;
use voya_platform::coreinfo::TargetOs;

use crate::app::MobileState;

use super::answer;

pub(super) async fn update_srs(state: &MobileState) -> Result<Value, AppError> {
    answer(
        "update_srs_assets",
        &state
            .services
            .updates()
            .update_srs_assets(through_the_running_core(state))
            .await?,
    )
}

/// The local proxy to fetch through, when there is one.
///
/// Rule sets are exactly the thing a user in a censored network cannot reach
/// directly, so the download prefers the running core. A phone in VPN mode has
/// no local proxy port to offer, and the answer is then `None` — the request
/// goes through the tunnel anyway, which is the same effect.
fn through_the_running_core(state: &MobileState) -> Option<String> {
    runtime_proxy_url(
        true,
        None,
        &state.config_mutations.current_config(),
        TargetOs::current(),
    )
}
