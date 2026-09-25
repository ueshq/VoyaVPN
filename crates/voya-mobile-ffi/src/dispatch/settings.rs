//! Settings and logging.

use serde::Deserialize;
use serde_json::Value;
use voya_app::settings::save::settings_from_app_config;
use voya_contracts::AppError;

use voya_app::config_mutation::AppConfig;

use crate::app::MobileState;

use super::{answer, arguments};

pub(super) async fn load_ui_preferences(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();

    answer(
        "load_ui_preferences",
        &settings_from_app_config(&config).appearance,
    )
}

pub(super) async fn load_app_settings(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();

    answer("load_app_settings", &settings_from_app_config(&config))
}

#[derive(Debug, Deserialize)]
struct LogStreaming {
    enabled: bool,
}

/// Whether the Logs screen is showing and so wants lines delivered.
///
/// The flag is the host's: nothing downstream of it needs the database, and a
/// stream nobody reads is the one cost worth avoiding on a phone.
pub(super) fn set_log_streaming(state: &MobileState, args: &Value) -> Result<Value, AppError> {
    let LogStreaming { enabled } = arguments("set_log_streaming", args)?;
    state.sinks.set_log_streaming(enabled);

    Ok(Value::Null)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveSettings {
    settings: voya_contracts::AppSettings,
}

/// Validation, the pre-commit side effects, the commit and both rollback paths
/// are the transaction in `voya_app::settings`, where they are unit-tested.
/// What is left here is announcing the caches and the runtime action it chose.
pub(super) async fn save_app_settings(
    state: &MobileState,
    args: &Value,
) -> Result<Value, AppError> {
    let SaveSettings { settings } = arguments("save_app_settings", args)?;
    let outcome = voya_app::settings::save_app_settings(
        &state.config_mutations,
        &MobileSettingsSideEffects,
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

/// Nothing a phone's settings can ask for happens outside the app.
///
/// Autostart is the desktop's one pre-commit side effect, and a phone has no
/// login item: an always-on VPN is a system setting the user turns on in
/// Settings, not something an app arranges for itself.
struct MobileSettingsSideEffects;

impl voya_app::settings::save::SettingsSideEffectAdapter for MobileSettingsSideEffects {
    // `AppError` rather than `Infallible`: the conversion into a command
    // failure is written for it, and a future side effect that *can* fail
    // needs no change here.
    type Error = AppError;

    fn apply_autostart(&self, _config: &AppConfig) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(super) async fn settings_apply_status(state: &MobileState) -> Result<Value, AppError> {
    let config = state.config_mutations.current_config();

    answer(
        "get_settings_apply_status",
        &super::runtime::core_flow(state)
            .settings_apply_status(&config)
            .await?,
    )
}

pub(super) async fn apply_pending_settings(state: &MobileState) -> Result<Value, AppError> {
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
