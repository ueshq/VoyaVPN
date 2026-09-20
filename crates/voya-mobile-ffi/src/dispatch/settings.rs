//! Settings and logging.

use serde::Deserialize;
use serde_json::Value;
use voya_app::settings::save::settings_from_app_config;
use voya_contracts::AppError;

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
