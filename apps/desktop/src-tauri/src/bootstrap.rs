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
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::Instant,
};
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use voya_app::{
    elevation::ElevationManager,
    proxy_runtime::{ProxyMonitorController, ProxyRuntimeManager},
    services::AppServices,
    supervisor::{CoreSupervisor, SupervisorDeps},
    sysproxy::SystemProxyManager,
    tun::ProviderRegistrationCache,
};
use voya_platform::{
    coreinfo::{copy_seed_core_asset, TargetOs},
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
    let started = Instant::now();
    let app_config_dir = app_data_dir(app)?;
    let runtime_paths = AppPaths::new(&app_config_dir);
    runtime_paths.ensure_dirs()?;
    // Installed before the first `tracing::warn!` below so the startup
    // recovery paths are captured too.
    logging::install(app.handle().clone(), runtime_paths.log_dir());
    let services = timed("open database", || {
        tauri::async_runtime::block_on(AppServices::connect(
            &database_path(app)?,
            runtime_paths.clone(),
        ))
        .map_err(Box::<dyn Error>::from)
    })?;
    // A fresh install starts in the platform's native VPN mode where it has one,
    // and macOS never loads in a system proxy mode it does not offer.
    let system_locale = voya_platform::locale::system_locale();
    let config = timed("load settings", || {
        tauri::async_runtime::block_on(
            services.load_config_for(TargetOs::current(), system_locale.as_deref()),
        )
    })?;
    let system_proxy_manager = SystemProxyManager::new(
        SystemProxyService::new(Arc::new(StdProcessRunner::new())),
        runtime_paths.clone(),
    );
    // Startup only *undoes* a proxy a crashed run left behind. The
    // persisted mode is deliberately not applied: nothing is listening
    // on the local port until the user connects, and `connect` applies
    // the mode itself once the core is up. Applying it here pointed the
    // machine at a dead port on every launch.
    match timed("restore system proxy", || {
        system_proxy_manager.restore_dirty_proxy_if_needed(&config)
    }) {
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
    let config_mutations = Arc::new(
        services
            .config_mutations(Arc::clone(&shared_config))
            .with_target_os(TargetOs::current()),
    );
    // A fresh install starts with the default routing profile rather than an
    // empty Rules page. A failure only costs the seed, never startup.
    if let Err(error) = timed("seed default routing", || {
        tauri::async_runtime::block_on(services.ensure_default_routing(&config_mutations))
    }) {
        tracing::warn!(?error, "failed to seed the default routing profile");
    }
    timed("sweep orphaned metrics", || {
        tauri::async_runtime::block_on(services.initialize_profile_metrics())
    })?;
    let seed_dir = core_seed_resources_dir(app.path().resource_dir()?);
    // macOS launches the seed inside the signed bundle (only the disconnected
    // speedtest does), so only Windows and Linux stage it into app data.
    if TargetOs::current() != TargetOs::Macos {
        if let Err(error) = timed("stage core seed", || {
            copy_seed_core_asset(&runtime_paths, &seed_dir)
        }) {
            tracing::warn!(
                ?error,
                "failed to copy packaged core seed assets at startup"
            );
        }
    }
    let core_seed_resource_dir = Some(seed_dir);
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
    timed("revoke stale grant", || {
        elevation_manager.revoke_stale_grant()
    });
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
        Arc::clone(&shared_config),
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
    let speedtest_manager = services.speedtest_manager(
        core_seed_resource_dir.clone(),
        Arc::new(speedtest_runner),
        supervisor.clone(),
    );
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

    timed("build tray", || setup_tray(app))?;
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis(),
        "startup initialized"
    );
    Ok(())
}

/// Runs one startup step and logs how long it took, so a slow launch can be
/// attributed from the log file without a profiler.
fn timed<T>(step: &'static str, run: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let value = run();
    tracing::info!(
        step,
        elapsed_ms = started.elapsed().as_millis(),
        "startup step"
    );
    value
}

/// Where this launch keeps its database and runtime files.
///
/// Development builds can use a different database baseline than the installed
/// app, so they keep their files together in a separate directory; packaged
/// debug builds still use the installed path.
fn app_data_dir(app: &tauri::App) -> Result<PathBuf, Box<dyn Error>> {
    let app_config_dir = app.path().app_config_dir()?;
    Ok(if tauri::is_dev() {
        app_config_dir.join("dev")
    } else {
        app_config_dir
    })
}

/// The database file this launch opens.
pub(super) fn database_path(app: &tauri::App) -> Result<PathBuf, Box<dyn Error>> {
    Ok(app_data_dir(app)?.join(voya_app::startup::DATABASE_NAME))
}

/// The startup failure dialog in the system language: the settings that would
/// name the chosen language may be exactly what failed to load.
fn failure_text() -> voya_app::startup::StartupFailureText {
    voya_app::startup::startup_failure_text(
        &voya_platform::locale::system_locale().unwrap_or_default(),
    )
}

/// A fatal startup failure waiting for an event loop to show it on.
struct StartupFailure {
    message: String,
    /// Set when moving this database aside would let the next launch start.
    resettable_database: Option<PathBuf>,
}

static STARTUP_FAILURE: Mutex<Option<StartupFailure>> = Mutex::new(None);

pub(super) fn record_startup_failure(message: String, resettable_database: Option<PathBuf>) {
    if let Ok(mut failure) = STARTUP_FAILURE.lock() {
        *failure = Some(StartupFailure {
            message,
            resettable_database,
        });
    }
}

/// Tell the user why the app is closing, then close it.
///
/// The dialog is deliberately non-blocking: `RunEvent::Ready` is delivered on
/// the main thread, and the dialog itself has to be dispatched to that same
/// thread, so waiting for it here would deadlock the event loop it needs.
pub(super) fn report_startup_failure<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(failure) = STARTUP_FAILURE.lock().ok().and_then(|mut slot| slot.take()) else {
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
    let text = failure_text();
    let Some(database) = failure.resettable_database else {
        app.dialog()
            .message(failure.message)
            .title(text.title.clone())
            .kind(MessageDialogKind::Error)
            .show(move |_| handle.exit(1));
        return;
    };
    // The database fails the same way on every launch, so the dialog offers the
    // one remedy: move it aside and start again with a fresh one.
    app.dialog()
        .message(format!("{}\n\n{}", failure.message, text.reset_explanation))
        .title(text.title.clone())
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::OkCancelCustom(
            text.reset_database.clone(),
            text.quit.clone(),
        ))
        .show(move |reset| {
            if reset {
                confirm_database_reset(&handle, database.clone());
            } else {
                handle.exit(1);
            }
        });
}

/// Asks once more: after the reset no node, subscription or custom rule shows.
fn confirm_database_reset<R: tauri::Runtime>(app: &tauri::AppHandle<R>, database: PathBuf) {
    let handle = app.clone();
    let text = failure_text();
    app.dialog()
        .message(text.confirm_message.clone())
        .title(text.confirm_title.clone())
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            text.confirm_reset.clone(),
            text.quit.clone(),
        ))
        .show(move |reset| {
            if reset {
                reset_database_and_restart(&handle, &database);
            } else {
                handle.exit(1);
            }
        });
}

/// Moves the database aside and relaunches; a failed move is reported and the
/// app quits, leaving the original database where it was.
fn reset_database_and_restart<R: tauri::Runtime>(app: &tauri::AppHandle<R>, database: &Path) {
    match voya_app::startup::reset_database(database) {
        Ok(backup) => {
            if let Some(backup) = backup {
                tracing::warn!(
                    backup = %backup.database.display(),
                    "moved the unusable database aside"
                );
            }
            app.request_restart();
        }
        Err(error) => {
            tracing::error!(%error, "failed to move the unusable database aside");
            let handle = app.clone();
            let text = failure_text();
            let command = voya_app::startup::manual_database_reset_command(database);
            app.dialog()
                .message(
                    text.move_failed
                        .replace("{{error}}", &error.to_string())
                        .replace("{{command}}", &command.to_string()),
                )
                .title(text.title)
                .kind(MessageDialogKind::Error)
                .show(move |_| handle.exit(1));
        }
    }
}
