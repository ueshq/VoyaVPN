use std::path::Path;

use super::{support::*, *};

#[tauri::command]
#[specta::specta]
pub fn app_update_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<AppUpdaterStatus, AppError> {
    let current_version = app.package_info().version.to_string();

    Ok(updater_status(&app, current_version))
}

/// The Mac App Store build has no updater plugin: the store delivers updates.
#[cfg(feature = "mac-app-store")]
fn updater_status<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    current_version: String,
) -> AppUpdaterStatus {
    AppUpdaterStatus {
        current_version,
        state: AppUpdaterState::Unsupported,
        message: Some("updates are delivered by the Mac App Store".to_owned()),
    }
}

#[cfg(not(feature = "mac-app-store"))]
fn updater_status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    current_version: String,
) -> AppUpdaterStatus {
    use tauri_plugin_updater::UpdaterExt;

    match app.updater() {
        Ok(_) => AppUpdaterStatus {
            current_version,
            state: AppUpdaterState::Ready,
            message: None,
        },
        Err(error) => AppUpdaterStatus {
            current_version,
            state: app_updater_state_for_error(&error),
            message: Some(error.to_string()),
        },
    }
}

#[tauri::command]
#[specta::specta]
pub async fn update_geo_assets(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ResourceUpdateFile>, AppError> {
    let config = state.config_mutations().current_config();
    let proxy_url = runtime_proxy_url(true, None, &config);

    state
        .services()
        .updates()
        .update_geo_assets(proxy_url)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_srs_assets(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ResourceUpdateFile>, AppError> {
    let config = state.config_mutations().current_config();
    let proxy_url = runtime_proxy_url(true, None, &config);

    state
        .services()
        .updates()
        .update_srs_assets(proxy_url)
        .await
        .map_err(AppError::from)
}

/// Re-install a core binary from the packaged seed (`{resource_dir}/core-seeds/<core>/`)
/// into `bin/<core>/`. This is the recovery action behind the missing-core prompt: the
/// startup seed copy already runs automatically, but this lets the UI re-run it on demand
/// when the binary is absent (e.g. cleared bin dir, antivirus removal, or a skipped first run).
// `async` because it copies (and on macOS, scans) core binaries; done inline
// that filesystem work would stall the webview's main thread.
#[tauri::command]
#[specta::specta]
pub async fn install_core_seed(
    state: tauri::State<'_, AppState>,
) -> Result<CoreSeedInstallResult, AppError> {
    let Some(seed_dir) = state.core_seed_resource_dir().map(Path::to_path_buf) else {
        return Ok(CoreSeedInstallResult {
            status: CoreSeedInstallStatus::SeedMissing,
            installed_files: Vec::new(),
        });
    };
    let runtime_paths = state.runtime_paths().clone();

    run_blocking("core seed install", move || {
        // macOS launches the seed inside the signed bundle; nothing is copied.
        if TargetOs::current() == TargetOs::Macos {
            if let Some(executable) = discover_packaged_seed_executable(&seed_dir, TargetOs::Macos)
                .map_err(core_seed_install_error)?
            {
                return Ok(CoreSeedInstallResult {
                    status: CoreSeedInstallStatus::AlreadyInstalled,
                    installed_files: vec![executable.to_string_lossy().into_owned()],
                });
            }
        }

        let outcome =
            copy_seed_core_asset(&runtime_paths, &seed_dir).map_err(core_seed_install_error)?;

        Ok(core_seed_install_result(outcome))
    })
    .await?
}
