//! The Self-hosted node page. Every command returns the whole page state. The
//! manager announces each change itself, through `SelfHostEventSink`, so the
//! commands emit nothing: the watch loop's changes and theirs take one path.

use voya_contracts::{SelfHostConfig, SelfHostState, SelfHostStats};

use super::*;

#[tauri::command]
#[specta::specta]
pub async fn get_self_host_state(
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostState, AppError> {
    Ok(state.self_host().state().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_self_host_config(
    state: tauri::State<'_, AppState>,
    config: SelfHostConfig,
) -> Result<SelfHostState, AppError> {
    Ok(state.self_host().save_config(config).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_self_host_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<SelfHostState, AppError> {
    Ok(state.self_host().set_enabled(enabled).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn rotate_self_host_credentials(
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostState, AppError> {
    Ok(state.self_host().rotate_credentials().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_self_host_stats(
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostStats, AppError> {
    Ok(state.self_host().stats().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn run_self_host_environment_check(
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostState, AppError> {
    Ok(state.self_host().run_environment_check().await?)
}

/// Adds the Windows firewall rule; the UAC prompt blocks until answered, so
/// the work runs off the UI thread inside the manager.
#[tauri::command]
#[specta::specta]
pub async fn apply_self_host_firewall_rule(
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostState, AppError> {
    Ok(state.self_host().apply_firewall_rule().await?)
}
