//! One command in, one JSON value out.
//!
//! The dispatchers are the mobile counterparts of
//! `apps/desktop/src-tauri/src/ipc/commands/*.rs`, and they stay as thin: a
//! decision either host would have to make belongs in `voya-app`, not copied
//! here (ADR 0012).
//!
//! Every command the backend registers is accounted for. A command reaches
//! either a dispatcher or [`UNSUPPORTED_ON_MOBILE`], and the test at the bottom
//! of this file — reading the generated `packages/contracts/commands.json` —
//! fails when a new one reaches neither.

use serde::Deserialize;
use serde_json::Value;
use voya_contracts::{AppError, AppErrorKind, AppErrorSubsystem};

use crate::app::MobileState;

mod profiles;
mod runtime;
mod settings;
mod subscriptions;

/// Commands this platform will never answer.
///
/// Each one is a capability a phone does not have, not a gap: a phone sets no
/// system proxy, elevates nothing, has no second window to photograph, runs no
/// self-hosted node, and has no window chrome. Listing them is what makes the
/// coverage test meaningful — the alternative is a wildcard that silently
/// swallows the next new command.
pub const UNSUPPORTED_ON_MOBILE: &[&str] = &[
    // No system proxy: the tunnel provider is the only capture path, and
    // neither OS lets an app point the system at a local port.
    "system_proxy_status",
    // Elevation and the root-owned launcher are desktop-only; a phone asks for
    // VPN consent instead, which the host answers.
    "tun_request_elevation",
    "tun_provider_diagnostics",
    // The whole screen belongs to one app, so there is no second window to
    // photograph; a camera scan replaces it, in the host app.
    "scan_screen_qr",
    "decode_qr_image",
    // The app updates itself through the App Store or Play.
    "app_update_status",
    // No window chrome, no tray, no close prompt.
    "get_window_chrome_config",
    "set_window_acrylic",
    "resolve_close_request",
    // A phone is not an exit node (ADR 0011 is desktop-only).
    "get_self_host_state",
    "get_self_host_stats",
    "save_self_host_config",
    "set_self_host_enabled",
    "rotate_self_host_credentials",
    "run_self_host_environment_check",
    "apply_self_host_firewall_rule",
    // Per-app rules match a process name, which neither OS exposes.
    "list_process_candidates",
    // The desktop writes a log file through a save dialog; a phone shares the
    // text through the system share sheet, which needs no backend call.
    "export_logs",
    // The clipboard is a framework call the host app makes for itself; see the
    // clipboard seam in `@voya/client/platform`.
    "read_clipboard_text",
    // The core is inside the tunnel provider and ships with it; there is no
    // seed to stage into an app-data directory.
    "install_core_seed",
];

/// Commands that belong on a phone but have no dispatcher yet.
///
/// Separate from [`UNSUPPORTED_ON_MOBILE`] on purpose: putting a command here
/// says "this is a gap", while putting it there says "this will never work",
/// and conflating the two would make the unsupported list a lie. Both lists are
/// checked, so a command cannot quietly fall out of either.
///
/// This list shrinks to empty as the screens land, one feature at a time.
pub const NOT_YET_DISPATCHED: &[&str] = &[
    // Settings, DNS and the routing screens.
    "apply_pending_settings",
    "get_settings_apply_status",
    "save_app_settings",
    "load_dns_settings",
    "save_dns_settings",
    "list_routings",
    "save_routing",
    "set_active_routing",
    "delete_routings",
    "save_routing_rule",
    "delete_routing_rules",
    "move_routing_rule",
    "reset_routing_rules",
    "connection_mode_status",
    "set_connection_mode",
    "set_tun_enabled",
    // The node editor, its QR export and the per-node menu.
    "get_profile",
    "save_profile",
    "delete_profiles",
    "move_profile",
    "export_profile_share_links",
    "generate_qr_code",
    "save_subscription",
    "delete_subscriptions",
    // Policy groups beyond reading them.
    "save_policy_group",
    "delete_policy_groups",
    "set_active_policy_group",
    "select_policy_group_member",
    "test_policy_group_delay",
    // Speedtest: needs the in-process probe core (`ProbeCoreHost`).
    "run_speedtest",
    "cancel_speedtest",
    // The network-activity screen.
    "proxy_list_connections",
    "proxy_close_connection",
    "proxy_start_monitor",
    "proxy_stop_monitor",
    "proxy_set_traffic_mode",
    // The rule-library updater.
    "update_geo_assets",
    "update_srs_assets",
];

/// Runs one command.
pub async fn invoke(
    state: &MobileState,
    command: &str,
    args_json: &str,
) -> Result<String, AppError> {
    let args: Value = if args_json.trim().is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(args_json).map_err(|error| malformed_arguments(command, &error))?
    };

    let value = route(state, command, &args).await?;

    serde_json::to_string(&value).map_err(|error| AppError {
        kind: AppErrorKind::Internal,
        subsystem: AppErrorSubsystem::App,
        message: format!("could not encode the answer to {command}: {error}"),
    })
}

async fn route(state: &MobileState, command: &str, args: &Value) -> Result<Value, AppError> {
    match command {
        "load_ui_preferences" => settings::load_ui_preferences(state).await,
        "load_app_settings" => settings::load_app_settings(state).await,
        "set_log_streaming" => settings::set_log_streaming(state, args),

        "list_profile_summaries" => profiles::list_summaries(state).await,
        "set_active_profile" => profiles::set_active(state, args).await,
        "import_profiles_from_text" => profiles::import_from_text(state, args).await,
        "list_policy_groups" => profiles::list_policy_groups(state).await,
        "policy_group_runtime" => profiles::policy_group_runtime(state).await,

        "list_subscriptions" => subscriptions::list(state).await,
        "list_subscription_metadata" => subscriptions::list_metadata(state).await,
        "update_subscriptions" => subscriptions::update(state, args).await,

        "connect_active_profile" => runtime::connect(state).await,
        "disconnect_core" => runtime::disconnect(state).await,
        "restart_core" => runtime::restart(state).await,
        "runtime_status" => runtime::status(state).await,
        "tun_status" => runtime::tun_status(state).await,
        "speedtest_status" => runtime::speedtest_status(state).await,
        "check_connection_ip" => runtime::check_connection_ip(state).await,

        _ if UNSUPPORTED_ON_MOBILE.contains(&command) => Err(unsupported(command)),
        // Everything left is either a gap this build has not filled yet or a
        // command that does not exist; both reach the frontend as the same
        // typed kind, because from a screen's side they are the same thing.
        _ => Err(AppError {
            kind: AppErrorKind::Unsupported,
            subsystem: AppErrorSubsystem::App,
            message: format!("{command} has no dispatcher on this platform yet"),
        }),
    }
}

fn unsupported(command: &str) -> AppError {
    AppError {
        kind: AppErrorKind::Unsupported,
        subsystem: AppErrorSubsystem::App,
        message: format!("{command} is not available on this platform"),
    }
}

fn malformed_arguments(command: &str, error: &serde_json::Error) -> AppError {
    AppError {
        kind: AppErrorKind::Internal,
        subsystem: AppErrorSubsystem::App,
        message: format!("the arguments for {command} are not a JSON object: {error}"),
    }
}

/// Reads one command's named arguments, the way Tauri would.
pub(crate) fn arguments<T: for<'de> Deserialize<'de>>(
    command: &str,
    args: &Value,
) -> Result<T, AppError> {
    serde_json::from_value(args.clone()).map_err(|error| AppError {
        kind: AppErrorKind::Internal,
        subsystem: AppErrorSubsystem::App,
        message: format!("the arguments for {command} do not match its signature: {error}"),
    })
}

/// Turns a command's return value into JSON.
pub(crate) fn answer<T: serde::Serialize>(command: &str, value: &T) -> Result<Value, AppError> {
    serde_json::to_value(value).map_err(|error| AppError {
        kind: AppErrorKind::Internal,
        subsystem: AppErrorSubsystem::App,
        message: format!("could not encode the answer to {command}: {error}"),
    })
}

#[cfg(test)]
mod tests;
