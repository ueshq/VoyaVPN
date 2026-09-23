//! The Self-hosted node page. Every command returns the whole page state, and
//! every change is also broadcast as a `selfHost` invalidation, so a second
//! window or the background watch loop stays in step.

use voya_contracts::{SelfHostConfig, SelfHostState, SelfHostStats};

use super::{post_commit::*, *};

#[tauri::command]
#[specta::specta]
pub async fn get_self_host_state(
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostState, AppError> {
    Ok(state.self_host().state().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_self_host_config<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    config: SelfHostConfig,
) -> Result<SelfHostState, AppError> {
    let saved = state.self_host().save_config(config).await?;
    emit_invalidation(
        &app,
        "self-host-config-saved",
        invalidation::self_host_scopes(),
    );
    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn set_self_host_enabled<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<SelfHostState, AppError> {
    let changed = state.self_host().set_enabled(enabled).await?;
    emit_invalidation(
        &app,
        "self-host-enabled-changed",
        invalidation::self_host_scopes(),
    );
    Ok(changed)
}

#[tauri::command]
#[specta::specta]
pub async fn rotate_self_host_credentials<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostState, AppError> {
    let rotated = state.self_host().rotate_credentials().await?;
    emit_invalidation(
        &app,
        "self-host-credentials-rotated",
        invalidation::self_host_scopes(),
    );
    Ok(rotated)
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
pub async fn run_self_host_environment_check<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostState, AppError> {
    let checked = state.self_host().run_environment_check().await?;
    emit_invalidation(
        &app,
        "self-host-environment-checked",
        invalidation::self_host_scopes(),
    );
    Ok(checked)
}

/// Adds the Windows firewall rule; the UAC prompt blocks until answered, so
/// the work runs off the UI thread inside the manager.
#[tauri::command]
#[specta::specta]
pub async fn apply_self_host_firewall_rule<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<SelfHostState, AppError> {
    let applied = state.self_host().apply_firewall_rule().await?;
    emit_invalidation(
        &app,
        "self-host-firewall-rule-applied",
        invalidation::self_host_scopes(),
    );
    Ok(applied)
}
