//! Settings and logging.

use serde::Deserialize;
use voya_app::settings::save::settings_from_app_config;

use crate::app::MobileState;

use super::{answer, arguments, Answer};

pub(super) async fn load_ui_preferences(state: &MobileState) -> Answer {
    let config = state.config_mutations.current_config();

    answer(
        "load_ui_preferences",
        &settings_from_app_config(&config).appearance,
    )
}

pub(super) async fn load_app_settings(state: &MobileState) -> Answer {
    let config = state.config_mutations.current_config();

    answer("load_app_settings", &settings_from_app_config(&config))
}

/// The desktop gates core output while its Logs panel is closed; this gates
/// the shared log batcher the same way. Lines keep queueing — the newest 500
/// of them — and the page receives them the moment it streams again.
pub(super) fn set_log_streaming(args: &str) -> Answer {
    let SetLogStreaming { enabled } = arguments("set_log_streaming", args)?;
    crate::logging::set_streaming(enabled);
    answer("set_log_streaming", &())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetLogStreaming {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveSettings {
    settings: voya_contracts::AppSettings,
}

/// Validation, the pre-commit side effects, the commit and both rollback paths
/// are the transaction in `voya_app::settings`, where they are unit-tested.
/// What is left here is announcing the caches.
pub(super) async fn save_app_settings(state: &MobileState, args: &str) -> Answer {
    let SaveSettings { settings } = arguments("save_app_settings", args)?;
    let outcome = voya_app::settings::save_app_settings(
        &state.config_mutations,
        &voya_app::settings::save::NoAutostart,
        &settings,
    )
    .await?;

    if outcome.changed {
        state.sinks.invalidate(
            "app-settings-saved",
            voya_app::invalidation::settings_bundle_scopes(),
        );
    }

    answer("save_app_settings", &outcome.settings)
}

pub(super) async fn settings_apply_status(state: &MobileState) -> Answer {
    let config = state.config_mutations.current_config();

    answer(
        "get_settings_apply_status",
        &super::runtime::core_flow(state)
            .settings_apply_status(&config)
            .await?,
    )
}

pub(super) async fn apply_pending_settings(state: &MobileState) -> Answer {
    let captured = state.config_mutations.current_config();
    let flow = super::runtime::core_flow(state);
    let result = flow.apply_pending_settings(&captured).await;
    // Announced whether or not the apply succeeded: it runs after the commit,
    // so the bundle is stale either way.
    state.sinks.invalidate(
        "settings-applied",
        voya_app::invalidation::settings_bundle_scopes(),
    );
    result?;

    answer(
        "apply_pending_settings",
        &flow
            .settings_apply_status(&state.config_mutations.current_config())
            .await?,
    )
}
