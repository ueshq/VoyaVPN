use std::path::Path;

use super::{support::*, *};

#[tauri::command]
#[specta::specta]
pub fn app_update_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<AppUpdaterStatus, AppError> {
    let current_version = app.package_info().version.to_string();

    Ok(match app.updater() {
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
    })
}

#[tauri::command]
#[specta::specta]
pub async fn update_geo_assets(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ResourceUpdateFile>, AppError> {
    let config = current_config(&state)?;
    let proxy_url = runtime_proxy_url(true, None, &config);

    update_manager(&state)
        .update_geo_assets(&config, proxy_url)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_srs_assets(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ResourceUpdateFile>, AppError> {
    let config = current_config(&state)?;
    let proxy_url = runtime_proxy_url(true, None, &config);

    update_manager(&state)
        .update_srs_assets(&config, proxy_url)
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
    core_type: ContractCoreType,
) -> Result<CoreSeedInstallResult, AppError> {
    let domain_core_type = voya_app::contract_map::core_type_from_contract(core_type);
    let Some(seed_dir) = state.core_seed_resource_dir().map(Path::to_path_buf) else {
        return Ok(CoreSeedInstallResult {
            core_type,
            status: CoreSeedInstallStatus::SeedMissing,
            installed_files: Vec::new(),
        });
    };
    let runtime_paths = state.runtime_paths().clone();

    run_blocking("core seed install", move || {
        if TargetOs::current() == TargetOs::Macos {
            let core_info = get_core_info(domain_core_type).ok_or_else(|| {
                core_seed_install_error(CoreInfoError::MissingCoreInfo(domain_core_type))
            })?;
            if let Some(executable) =
                discover_packaged_seed_executable(&seed_dir, core_info, TargetOs::Macos)
                    .map_err(core_seed_install_error)?
            {
                return Ok(CoreSeedInstallResult {
                    core_type,
                    status: CoreSeedInstallStatus::AlreadyInstalled,
                    installed_files: vec![executable.to_string_lossy().into_owned()],
                });
            }
        }

        let outcome = copy_seed_core_asset(&runtime_paths, &seed_dir, domain_core_type)
            .map_err(core_seed_install_error)?;

        Ok(core_seed_install_result(outcome))
    })
    .await?
}
