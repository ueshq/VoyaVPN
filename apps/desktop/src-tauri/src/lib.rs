use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use specta_typescript::Typescript;
use tauri::{
    menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, RunEvent,
};
use tauri_specta::Event;
use voya_app::{
    config_mutation::ConfigMutationCoordinator,
    elevation::ElevationManager,
    logging::process_log_level_to_contract,
    proxy_runtime::{
        ProxyConnectionsSnapshot, ProxyMonitorController, ProxyRuntimeEventSink, ProxyTrafficEvent,
    },
    redaction::{redact_url_userinfo, redact_urls},
    services::{AppConfig, AppServices},
    shutdown::ShutdownLatch,
    speedtest::SpeedtestManager,
    statistics::{
        SharedAppConfigSource, StatisticsEventSink, StatisticsManager,
        StatisticsSnapshot as AppStatisticsSnapshot,
    },
    subscriptions::{
        AutoUpdateOutcome, SubscriptionAutoUpdateScheduler, SubscriptionAutoUpdateSink,
    },
    supervisor::{CoreSupervisor, NativeTunExitEvent, SupervisorDeps, SupervisorEventSink},
    sysproxy::SystemProxyManager,
};
use voya_platform::{
    coreinfo::{copy_seed_core_assets, TargetOs},
    filesystem::reject_incompatible_config,
    paths::{core_seed_resources_dir, AppPaths},
    process::{
        classify_core_log_line, ProcessLogSink, ProcessOutputStream, ProcessRole, StdProcessRunner,
    },
    sysproxy::{platform_pac_manager, SystemProxyService},
};

mod ipc;
mod logging;

/// Latches the exit teardown so it runs once per process.
static SHUTDOWN_LATCH: ShutdownLatch = ShutdownLatch::new();

const TRAY_SHOW: &str = "tray-show";
const TRAY_HIDE: &str = "tray-hide";
const TRAY_QUIT: &str = "tray-quit";
pub(crate) struct AppState {
    services: AppServices,
    config_mutations: Arc<ConfigMutationCoordinator>,
    core_seed_resource_dir: Option<PathBuf>,
    elevation_manager: ElevationManager,
    supervisor: CoreSupervisor,
    statistics_manager: StatisticsManager,
    subscription_auto_update: SubscriptionAutoUpdateScheduler,
    speedtest_manager: SpeedtestManager,
    system_proxy_manager: SystemProxyManager,
    proxy_monitor_controller: ProxyMonitorController,
}

impl AppState {
    pub(crate) fn services(&self) -> &AppServices {
        &self.services
    }

    pub(crate) fn config(&self) -> &RwLock<AppConfig> {
        self.config_mutations.config_lock()
    }

    pub(crate) fn config_mutations(&self) -> &ConfigMutationCoordinator {
        &self.config_mutations
    }

    pub(crate) fn runtime_paths(&self) -> &AppPaths {
        self.services.runtime_paths()
    }

    pub(crate) fn core_seed_resource_dir(&self) -> Option<&Path> {
        self.core_seed_resource_dir.as_deref()
    }

    pub(crate) fn elevation_manager(&self) -> &ElevationManager {
        &self.elevation_manager
    }

    pub(crate) fn supervisor(&self) -> CoreSupervisor {
        self.supervisor.clone()
    }

    pub(crate) fn statistics_manager(&self) -> &StatisticsManager {
        &self.statistics_manager
    }

    pub(crate) fn subscription_auto_update(&self) -> &SubscriptionAutoUpdateScheduler {
        &self.subscription_auto_update
    }

    pub(crate) fn speedtest_manager(&self) -> SpeedtestManager {
        self.speedtest_manager.clone()
    }

    pub(crate) fn system_proxy_manager(&self) -> SystemProxyManager {
        self.system_proxy_manager.clone()
    }

    pub(crate) fn proxy_monitor_controller(&self) -> ProxyMonitorController {
        self.proxy_monitor_controller.clone()
    }
}

pub fn export_bindings(path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    ipc::specta_builder().export(Typescript::default(), path)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = ipc::specta_builder();

    // The path is baked in at compile time, so a packaged debug build moved to
    // another machine would panic before a window exists. `pnpm generate:bindings`
    // (the `export-bindings` binary) is the canonical path; this convenience
    // export is limited to `tauri dev` and an explicit opt-in, and never fatal.
    #[cfg(debug_assertions)]
    if tauri::is_dev() || std::env::var_os("VOYAVPN_EXPORT_BINDINGS").is_some() {
        if let Err(error) =
            export_bindings(Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/bindings.ts"))
        {
            tracing::warn!(%error, "failed to export TypeScript IPC bindings");
        }
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    window.app_handle().exit(0);
                }
            }
        })
        .setup(move |app| {
            let app_config_dir = app.path().app_config_dir()?;
            reject_incompatible_config(&app_config_dir.join("guiNConfig.json"), 1)?;
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
                SystemProxyService::new(Arc::new(StdProcessRunner::new()), platform_pac_manager()),
                runtime_paths.clone(),
            );
            let skip_persisted_proxy_apply = match system_proxy_manager
                .restore_dirty_proxy_if_needed(&config)
            {
                Ok(restored) => {
                    if restored {
                        tracing::warn!("restored system proxy from previous dirty shutdown marker");
                    }
                    restored
                }
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "failed to restore system proxy from dirty shutdown marker"
                    );
                    true
                }
            };
            let shared_config = Arc::new(RwLock::new(config.clone()));
            let config_mutations = Arc::new(services.config_mutations(Arc::clone(&shared_config)));
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
            let runner: Arc<dyn voya_platform::process::ProcessRunner> = Arc::new(
                StdProcessRunner::with_log_sink(Arc::new(TauriProcessLogSink {
                    app: app.handle().clone(),
                })),
            );
            let elevation_manager = ElevationManager::new(
                Arc::clone(&runner),
                runtime_paths.temp_dir().to_path_buf(),
                runtime_paths.bin_dir().to_path_buf(),
            );
            // A crash never reaches the exit-time revoke, so a previous run can
            // leave a root launcher + NOPASSWD drop-in installed. Sweep it
            // before the supervisor can spawn anything through it.
            elevation_manager.revoke_stale_grant();
            let speedtest_runner = StdProcessRunner::with_log_sink(Arc::new(TauriProcessLogSink {
                app: app.handle().clone(),
            }));
            let runtime_handle = tauri::async_runtime::handle();
            let runtime_guard = runtime_handle.inner().enter();
            let supervisor = CoreSupervisor::spawn(
                SupervisorDeps::platform_with_runner(
                    Arc::clone(&runner),
                    elevation_manager.state(),
                )
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
            let subscription_auto_update = SubscriptionAutoUpdateScheduler::spawn(
                services.database().clone(),
                Arc::clone(&config_mutations),
                supervisor.clone(),
                TargetOs::current(),
                Arc::new(TauriSubscriptionAutoUpdateSink {
                    app: app.handle().clone(),
                }),
            );
            drop(runtime_guard);
            if !skip_persisted_proxy_apply {
                if let Err(error) = system_proxy_manager.apply_config(&config, false) {
                    tracing::warn!(?error, "failed to apply persisted system proxy mode");
                }
            } else {
                tracing::warn!("skipped persisted system proxy apply after dirty marker recovery");
            }
            if let Err(error) =
                ipc::commands::register_show_window_shortcut_for_config(app.handle(), &config)
            {
                tracing::warn!(?error, "failed to register persisted global hotkeys");
            }
            let speedtest_manager = services
                .speedtest_manager(core_seed_resource_dir.clone(), Arc::new(speedtest_runner));
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
            });

            specta_builder.mount_events(app);
            setup_tray(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build VoyaVPN");

    app.run(|app, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            shutdown_for_exit(app);
        }
    });
}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let menu = build_tray_menu(app.handle())?;

    let mut tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("VoyaVPN")
        .show_menu_on_left_click(true)
        .on_menu_event(
            |app, event: tauri::menu::MenuEvent| match event.id().as_ref() {
                TRAY_SHOW => show_main_window(app),
                TRAY_HIDE => hide_main_window(app),
                // `exit` raises ExitRequested and Exit, which is where the
                // teardown runs; calling it here as well would only repeat it.
                TRAY_QUIT => app.exit(0),
                _ => {}
            },
        );

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;

    Ok(())
}

pub(crate) fn refresh_tray_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id("main") else {
        return Ok(());
    };
    let menu = build_tray_menu(app)?;
    tray.set_menu(Some(menu))
}

fn build_tray_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<Menu<R>> {
    let show = MenuItem::with_id(app, TRAY_SHOW, "Show VoyaVPN", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, TRAY_HIDE, "Hide Window", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT, "Quit", true, None::<&str>)?;
    let quit_separator = PredefinedMenuItem::separator(app)?;

    Menu::with_items(
        app,
        &[&show as &dyn IsMenuItem<R>, &hide, &quit_separator, &quit],
    )
}

struct TauriProcessLogSink {
    app: tauri::AppHandle,
}

struct TauriStatisticsEventSink {
    app: tauri::AppHandle,
}

struct TauriProxyRuntimeEventSink {
    app: tauri::AppHandle,
}

struct TauriSupervisorEventSink {
    app: tauri::AppHandle,
}

struct TauriSubscriptionAutoUpdateSink {
    app: tauri::AppHandle,
}

impl SubscriptionAutoUpdateSink for TauriSubscriptionAutoUpdateSink {
    fn update_completed(&self, outcome: AutoUpdateOutcome) {
        if let Some(error) = &outcome.error {
            // Redacted at the source too; repeated here so a future failure
            // path cannot put a tokenized subscription URL in a toast.
            let error = redact_urls(error);
            let message = format!(
                "Automatic subscription update failed for {}: {error}",
                outcome.remarks
            );
            if let Err(emit_error) =
                ipc::commands::emit_runtime_log(&self.app, ipc::events::LogLevel::Warn, &message)
            {
                tracing::warn!(?emit_error, "failed to emit auto-update failure log");
            }
            // Only the first failure of a streak surfaces as a user notice;
            // retries stay in the log until the subscription recovers.
            if outcome.consecutive_failures == 1 {
                let notice = ipc::events::AppEvent::Notice(voya_contracts::AppNotice {
                    level: voya_contracts::AppNoticeLevel::Warning,
                    title: format!("Subscription auto-update failed: {}", outcome.remarks),
                    message: Some(error.clone()),
                });
                if let Err(emit_error) = notice.emit(&self.app) {
                    tracing::warn!(?emit_error, "failed to emit auto-update failure notice");
                }
            }
            return;
        }

        let imported = outcome.result.as_ref().map_or(0, |result| result.imported);
        let message = format!(
            "Automatic subscription update finished for {} ({imported} profiles imported)",
            outcome.remarks
        );
        if let Err(emit_error) =
            ipc::commands::emit_runtime_log(&self.app, ipc::events::LogLevel::Info, &message)
        {
            tracing::warn!(?emit_error, "failed to emit auto-update log");
        }

        let mut keys = vec![
            vec!["subscriptions".to_string()],
            vec!["subscription-metadata".to_string()],
            vec!["profiles".to_string()],
            vec!["profile-ex".to_string()],
        ];
        if outcome.config_changed {
            keys.push(vec!["active-profile".to_string()]);
        }
        let event = ipc::events::InvalidateEvent {
            keys: keys
                .into_iter()
                .map(|query_key| ipc::events::QueryInvalidation {
                    query_key,
                    reason: "subscription-auto-updated".to_string(),
                })
                .collect(),
        };
        if let Err(emit_error) = event.emit(&self.app) {
            tracing::warn!(?emit_error, "failed to emit auto-update invalidation");
        }
    }
}

impl SupervisorEventSink for TauriSupervisorEventSink {
    fn native_tun_exited(&self, event: NativeTunExitEvent) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let config = match state.config().read() {
                Ok(guard) => guard.clone(),
                Err(_) => AppConfig::default(),
            };
            let message = format!("Native TUN provider exited: {}", event.message);
            if let Err(error) =
                ipc::commands::emit_runtime_log(&app, ipc::events::LogLevel::Error, &message)
            {
                tracing::warn!(?error, "failed to emit native TUN exit log");
            }
            if let Err(error) = ipc::commands::emit_core_state(
                &app,
                ipc::events::CoreState::Disconnected,
                event.active_profile_id.clone(),
                None,
            ) {
                tracing::warn!(?error, "failed to emit native TUN disconnected state");
            }
            if let Err(error) = ipc::commands::restore_system_proxy_after_native_tun_failure(
                &app,
                &state,
                &config,
                "provider exit",
            ) {
                tracing::warn!(
                    ?error,
                    "failed to restore system proxy after native TUN provider exit"
                );
            }
            if let Err(error) = ipc::commands::emit_current_tun_status(&app, &state).await {
                tracing::warn!(?error, "failed to emit native TUN status after exit");
            }
            if let Err(error) = ipc::commands::emit_statistics_zero(&app) {
                tracing::warn!(
                    ?error,
                    "failed to emit zero statistics after native TUN exit"
                );
            }
        });
    }
}

impl StatisticsEventSink for TauriStatisticsEventSink {
    fn emit_statistics(&self, snapshot: AppStatisticsSnapshot) {
        let event =
            ipc::events::TransientStreamEvent::Statistics(ipc::events::StatisticsSnapshot {
                active_profile_id: snapshot.active_profile_id,
                proxy_upload_bytes_per_second: snapshot.proxy_upload_bytes_per_second,
                proxy_download_bytes_per_second: snapshot.proxy_download_bytes_per_second,
                direct_upload_bytes_per_second: snapshot.direct_upload_bytes_per_second,
                direct_download_bytes_per_second: snapshot.direct_download_bytes_per_second,
                upload_bytes_per_second: snapshot.upload_bytes_per_second,
                download_bytes_per_second: snapshot.download_bytes_per_second,
                server_stat: snapshot
                    .server_stat
                    .map(voya_app::contract_map::server_stat_to_contract),
            });

        if let Err(error) = event.emit(&self.app) {
            tracing::warn!(?error, "failed to emit statistics event");
        }
    }
}

impl ProxyRuntimeEventSink for TauriProxyRuntimeEventSink {
    fn emit_traffic(&self, event: ProxyTrafficEvent) {
        let event = ipc::events::TransientStreamEvent::ProxyTraffic(event);

        if let Err(error) = event.emit(&self.app) {
            tracing::warn!(?error, "failed to emit proxy traffic event");
        }
    }

    fn emit_connections(&self, event: ProxyConnectionsSnapshot) {
        let event = ipc::events::TransientStreamEvent::ProxyConnections(event);

        if let Err(error) = event.emit(&self.app) {
            tracing::warn!(?error, "failed to emit proxy connections event");
        }
    }
}

impl ProcessLogSink for TauriProcessLogSink {
    fn line(&self, role: ProcessRole, _stream: ProcessOutputStream, line: String) {
        // The stream carries no severity: sing-box writes every level to stderr
        // unless `log.output` is set, so the level comes from the line itself.
        let level = process_log_level_to_contract(classify_core_log_line(&line));
        let line = redact_process_log_line(&line);
        let event = ipc::events::TransientStreamEvent::LogLine(ipc::events::LogLineEvent {
            id: ipc::events::next_log_line_id(),
            level,
            line: format!("[{}] {line}", process_role_label(role)),
        });

        if let Err(error) = event.emit(&self.app) {
            tracing::warn!(?error, "failed to emit process log event");
        }
    }
}

fn redact_process_log_line(line: &str) -> String {
    redact_url_userinfo(line)
}

fn process_role_label(role: ProcessRole) -> &'static str {
    match role {
        ProcessRole::Main => "main",
        ProcessRole::Pre => "pre",
        ProcessRole::SudoKill => "sudo",
        ProcessRole::SysProxy => "sysproxy",
        ProcessRole::Probe => "probe",
        ProcessRole::Autostart => "autostart",
    }
}

fn shutdown_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    // Tauri raises ExitRequested and then Exit for every exit path, so this is
    // reached at least twice per quit. The sequence re-runs the sudoers revoke
    // and the per-service system-proxy restore, so it is latched to one pass.
    if !SHUTDOWN_LATCH.begin() {
        return;
    }
    // Stop the auto-update scheduler before the runtime disconnects so no
    // background subscription commit races the shutdown sequence.
    if let Some(state) = app.try_state::<AppState>() {
        state.subscription_auto_update().close();
    }
    disconnect_runtime_for_exit(app);
    revoke_elevation_for_exit(app);
    restore_system_proxy_for_exit(app);
}

fn revoke_elevation_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    // Runs after the runtime disconnect so the elevated core is already stopped
    // (via the launcher) before the launcher + sudoers drop-in are removed.
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    state.elevation_manager().revoke();
}

fn disconnect_runtime_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let runtime = state.services().runtime(state.supervisor());
    if let Err(error) = tauri::async_runtime::block_on(runtime.disconnect()) {
        tracing::warn!(?error, "failed to disconnect runtime on exit");
    }
}

fn restore_system_proxy_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Ok(config) = state.config().read().map(|guard| guard.clone()) else {
        tracing::warn!("failed to read app config while restoring system proxy on exit");
        return;
    };
    if let Err(error) = state.system_proxy_manager().restore(&config) {
        tracing::warn!(?error, "failed to restore system proxy on exit");
    }
    state.system_proxy_manager().stop_pac();
    state.statistics_manager().close();
    if let Err(error) = state.proxy_monitor_controller().stop() {
        tracing::warn!(?error, "failed to stop proxy monitor on exit");
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.show() {
            tracing::warn!(?error, "failed to show main window from tray");
        }

        if let Err(error) = window.set_focus() {
            tracing::warn!(?error, "failed to focus main window from tray");
        }
    }
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.hide() {
            tracing::warn!(?error, "failed to hide main window from tray");
        }
    }
}
