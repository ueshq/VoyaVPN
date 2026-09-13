//! Startup assembly and deferred startup-failure reporting.
use crate::{
    event_sinks::{
        TauriProcessLogSink, TauriStatisticsEventSink, TauriSubscriptionAutoUpdateSink,
        TauriSupervisorEventSink,
    },
    logging,
    tray::setup_tray,
    AppState,
};
use std::{
    error::Error,
    sync::{Arc, Mutex, RwLock},
};
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use voya_app::{
    elevation::ElevationManager,
    proxy_runtime::{ProxyMonitorController, ProxyRuntimeManager},
    services::AppServices,
    statistics::SharedAppConfigSource,
    supervisor::{CoreSupervisor, SupervisorDeps},
    sysproxy::SystemProxyManager,
    tun::ProviderRegistrationCache,
};
use voya_platform::{
    coreinfo::{copy_seed_core_assets, TargetOs},
    paths::{core_seed_resources_dir, AppPaths},
    process::{JobAssignedRunner, PlatformProcessJobFactory, ProcessRunner, StdProcessRunner},
    sysproxy::SystemProxyService,
};

/// Everything the app needs before it can serve a single command.
///
/// Split out of `setup` so a failure is a value rather than a `?` that
/// disappears into `build().expect(..)`: in a packaged build (no console on
/// Windows, a bundle on macOS) that made the process vanish with no
/// explanation, including user-actionable database schema failures.
pub(super) fn initialize(app: &mut tauri::App) -> Result<(), Box<dyn Error>> {
    let app_config_dir = app.path().app_config_dir()?;
    // Development builds can use a different database baseline than the
    // installed app. Keep their database and runtime files together in a
    // separate directory; packaged debug builds still use the installed path.
    let app_config_dir = if tauri::is_dev() {
        app_config_dir.join("dev")
    } else {
        app_config_dir
    };
    let runtime_paths = AppPaths::new(&app_config_dir);
    runtime_paths.ensure_dirs()?;
    // Installed before the first `tracing::warn!` below so the startup
    // recovery paths are captured too.
    logging::install(app.handle().clone(), runtime_paths.log_dir());
    let services = tauri::async_runtime::block_on(AppServices::connect(
        &app_config_dir.join("voyavpn.sqlite"),
        runtime_paths.clone(),
    ))?;
    let config = tauri::async_runtime::block_on(services.load_config())?;
    let system_proxy_manager = SystemProxyManager::new(
        SystemProxyService::new(Arc::new(StdProcessRunner::new())),
        runtime_paths.clone(),
    );
    // Startup only *undoes* a proxy a crashed run left behind. The
    // persisted mode is deliberately not applied: nothing is listening
    // on the local port until the user connects, and `connect` applies
    // the mode itself once the core is up. Applying it here pointed the
    // machine at a dead port on every launch.
    match system_proxy_manager.restore_dirty_proxy_if_needed(&config) {
        Ok(true) => {
            tracing::warn!("restored system proxy from previous dirty shutdown marker");
        }
        Ok(false) => {}
        Err(error) => tracing::warn!(
            ?error,
            "failed to restore system proxy from dirty shutdown marker"
        ),
    }
    let shared_config = Arc::new(RwLock::new(config.clone()));
    let config_mutations = Arc::new(services.config_mutations(Arc::clone(&shared_config)));
    // A fresh install starts with the default routing profile rather than an
    // empty Rules page. A failure only costs the seed, never startup.
    if let Err(error) =
        tauri::async_runtime::block_on(services.ensure_default_routing(&config_mutations))
    {
        tracing::warn!(?error, "failed to seed the default routing profile");
    }
    tauri::async_runtime::block_on(services.initialize_profile_metrics())?;
    let core_seed_resource_dir = Some(core_seed_resources_dir(app.path().resource_dir()?));
    match (TargetOs::current(), core_seed_resource_dir.as_ref()) {
        (TargetOs::Macos, _) => {
            tracing::debug!("skipped packaged core seed copy at startup on macOS");
        }
        (_, Some(seed_dir)) => {
            if let Err(error) = copy_seed_core_assets(&runtime_paths, seed_dir) {
                tracing::warn!(
                    ?error,
                    "failed to copy packaged core seed assets at startup"
                );
            }
        }
        (_, None) => {}
    }
    let runner: Arc<dyn ProcessRunner> = Arc::new(StdProcessRunner::with_log_sink(Arc::new(
        TauriProcessLogSink {
            app: app.handle().clone(),
        },
    )));
    let elevation_manager = ElevationManager::new(
        Arc::clone(&runner),
        runtime_paths.temp_dir().to_path_buf(),
        runtime_paths.bin_dir().to_path_buf(),
    );
    // A crash never reaches the exit-time revoke, so a previous run can
    // leave a root launcher + NOPASSWD drop-in installed. Sweep it
    // before the supervisor can spawn anything through it.
    elevation_manager.revoke_stale_grant();
    // Probe cores get the same kill-with-the-app job object the supervisor
    // gives the real core below via `SupervisorDeps::platform_with_runner`.
    let speedtest_runner = JobAssignedRunner::new(
        StdProcessRunner::with_log_sink(Arc::new(TauriProcessLogSink {
            app: app.handle().clone(),
        })),
        &PlatformProcessJobFactory,
    );
    let runtime_handle = tauri::async_runtime::handle();
    let runtime_guard = runtime_handle.inner().enter();
    let supervisor = CoreSupervisor::spawn(
        SupervisorDeps::platform_with_runner(Arc::clone(&runner), elevation_manager.state())
            .with_event_sink(Arc::new(TauriSupervisorEventSink {
                app: app.handle().clone(),
            })),
    );
    let statistics_manager = services.spawn_statistics(
        supervisor.clone(),
        Arc::new(SharedAppConfigSource::new(Arc::clone(&shared_config))),
        Arc::new(TauriStatisticsEventSink {
            app: app.handle().clone(),
        }),
    );
    let subscription_auto_update = services.spawn_subscription_auto_update(
        Arc::clone(&config_mutations),
        supervisor.clone(),
        TargetOs::current(),
        Arc::new(TauriSubscriptionAutoUpdateSink {
            app: app.handle().clone(),
        }),
    );
    drop(runtime_guard);
    let speedtest_manager =
        services.speedtest_manager(core_seed_resource_dir.clone(), Arc::new(speedtest_runner));
    app.manage(AppState {
        services,
        config_mutations,
        core_seed_resource_dir,
        elevation_manager,
        supervisor,
        statistics_manager,
        subscription_auto_update,
        speedtest_manager,
        system_proxy_manager,
        proxy_monitor_controller: ProxyMonitorController::new(),
        proxy_runtime: ProxyRuntimeManager::new(),
        provider_registration_cache: Arc::new(ProviderRegistrationCache::new()),
    });

    setup_tray(app)?;
    Ok(())
}

/// A fatal startup failure waiting for an event loop to show it on.
static STARTUP_FAILURE: Mutex<Option<String>> = Mutex::new(None);

pub(super) fn record_startup_failure(message: String) {
    if let Ok(mut failure) = STARTUP_FAILURE.lock() {
        *failure = Some(message);
    }
}

/// Tell the user why the app is closing, then close it.
///
/// The dialog is deliberately non-blocking: `RunEvent::Ready` is delivered on
/// the main thread, and the dialog itself has to be dispatched to that same
/// thread, so waiting for it here would deadlock the event loop it needs.
pub(super) fn report_startup_failure<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(message) = STARTUP_FAILURE.lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    // The window is live but unusable — no `AppState` was ever managed — so it
    // is hidden rather than left behind the dialog failing every command.
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.hide() {
            tracing::warn!(
                ?error,
                "failed to hide the main window after a startup failure"
            );
        }
    }

    let handle = app.clone();
    app.dialog()
        .message(message)
        .title("VoyaVPN could not start")
        .kind(MessageDialogKind::Error)
        .show(move |_| handle.exit(1));
}
