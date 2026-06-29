use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use specta_typescript::Typescript;
use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    Manager, RunEvent,
};
use tauri_specta::Event;
use tokio::sync::Mutex as AsyncMutex;
use voya_app::{
    clash::{ClashConnectionsSnapshot, ClashEventSink, ClashMonitorController, ClashTrafficEvent},
    diagnostics::{prepare_diagnostics_settings, DiagnosticsClient},
    elevation::ElevationManager,
    redaction::redact_url_userinfo,
    runtime::RuntimeManager,
    speedtest::SpeedtestManager,
    statistics::{
        SharedAppConfigSource, StatisticsEventSink, StatisticsManager,
        StatisticsSnapshot as AppStatisticsSnapshot,
    },
    supervisor::{CoreSupervisor, SupervisorDeps},
    sysproxy::SystemProxyManager,
};
use voya_core::{AppConfig, ProfileItem, SysProxyType};
use voya_db::{AppConfigStore, Database, DATABASE_NAME};
use voya_platform::{
    coreinfo::copy_seed_core_assets,
    paths::{core_seed_resources_dir, AppPaths, StorageMode},
    process::{ProcessLogSink, ProcessOutputStream, ProcessRole, StdProcessRunner},
    sysproxy::{platform_pac_manager, SystemProxyService},
};

mod ipc;

const TRAY_SHOW: &str = "tray-show";
const TRAY_HIDE: &str = "tray-hide";
const TRAY_QUIT: &str = "tray-quit";
const TRAY_CONNECT: &str = "tray-connect";
const TRAY_DISCONNECT: &str = "tray-disconnect";
const TRAY_PROXY_CLEAR: &str = "tray-proxy-clear";
const TRAY_PROXY_SET: &str = "tray-proxy-set";
const TRAY_PROXY_UNCHANGED: &str = "tray-proxy-unchanged";
const TRAY_PROXY_PAC: &str = "tray-proxy-pac";
const TRAY_SERVER_PREFIX: &str = "tray-server:";

enum TrayPrivilegedAction {
    Connect,
    Disconnect,
    Proxy(SysProxyType),
}

pub(crate) struct AppState {
    database: Database,
    config_store: AppConfigStore,
    config: Arc<RwLock<AppConfig>>,
    runtime_paths: AppPaths,
    core_seed_resource_dir: Option<PathBuf>,
    elevation_manager: ElevationManager,
    supervisor: CoreSupervisor,
    statistics_manager: StatisticsManager,
    speedtest_manager: SpeedtestManager,
    system_proxy_manager: SystemProxyManager,
    clash_monitor_controller: ClashMonitorController,
    diagnostics_client: Arc<AsyncMutex<DiagnosticsClient>>,
}

impl AppState {
    pub(crate) fn database(&self) -> &Database {
        &self.database
    }

    pub(crate) fn config_store(&self) -> &AppConfigStore {
        &self.config_store
    }

    pub(crate) fn config(&self) -> &RwLock<AppConfig> {
        self.config.as_ref()
    }

    pub(crate) fn runtime_paths(&self) -> &AppPaths {
        &self.runtime_paths
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

    pub(crate) fn speedtest_manager(&self) -> SpeedtestManager {
        self.speedtest_manager.clone()
    }

    pub(crate) fn system_proxy_manager(&self) -> SystemProxyManager {
        self.system_proxy_manager.clone()
    }

    pub(crate) fn clash_monitor_controller(&self) -> ClashMonitorController {
        self.clash_monitor_controller.clone()
    }

    pub(crate) fn diagnostics_client(&self) -> Arc<AsyncMutex<DiagnosticsClient>> {
        Arc::clone(&self.diagnostics_client)
    }
}

pub fn export_bindings(path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    ipc::specta_builder().export(Typescript::default(), path)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = ipc::specta_builder();

    #[cfg(debug_assertions)]
    export_bindings(Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/bindings.ts"))
        .expect("failed to export TypeScript IPC bindings");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            let app_config_dir = app.path().app_config_dir()?;
            let runtime_paths = AppPaths::new(&app_config_dir, StorageMode::UserData);
            runtime_paths.ensure_dirs()?;
            let system_proxy_manager = SystemProxyManager::new(
                SystemProxyService::new(Arc::new(StdProcessRunner::new()), platform_pac_manager()),
                runtime_paths.clone(),
            );
            let config_store = AppConfigStore::new(app_config_dir.join("guiNConfig.json"));
            let mut config = config_store.load()?;
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
            let original_config = config.clone();
            let app_version = app.package_info().version.to_string();
            prepare_diagnostics_settings(
                &mut config,
                &app_version,
                ipc::commands::diagnostics_release_channel(),
            );
            if original_config != config {
                config_store.save(&config)?;
            }
            let shared_config = Arc::new(RwLock::new(config.clone()));
            let database = tauri::async_runtime::block_on(Database::connect(
                app_config_dir.join(DATABASE_NAME),
            ))?;
            tauri::async_runtime::block_on(
                voya_app::profiles::ProfileExManager::new(&database).init(),
            )?;
            let core_seed_resource_dir = Some(core_seed_resources_dir(app.path().resource_dir()?));
            if let Some(seed_dir) = &core_seed_resource_dir {
                if let Err(error) = copy_seed_core_assets(&runtime_paths, seed_dir) {
                    tracing::warn!(
                        ?error,
                        "failed to copy packaged core seed assets at startup"
                    );
                }
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
            let speedtest_runner = StdProcessRunner::with_log_sink(Arc::new(TauriProcessLogSink {
                app: app.handle().clone(),
            }));
            let runtime_handle = tauri::async_runtime::handle();
            let runtime_guard = runtime_handle.inner().enter();
            let supervisor = CoreSupervisor::spawn(SupervisorDeps::platform_with_runner(
                Arc::clone(&runner),
                elevation_manager.state(),
            ));
            let statistics_manager = StatisticsManager::spawn(
                database.clone(),
                supervisor.clone(),
                Arc::new(SharedAppConfigSource::new(Arc::clone(&shared_config))),
                Arc::new(TauriStatisticsEventSink {
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
                ipc::commands::register_global_hotkeys_for_config(app.handle(), &config)
            {
                tracing::warn!(?error, "failed to register persisted global hotkeys");
            }
            let speedtest_manager = SpeedtestManager::new(
                runtime_paths.clone(),
                core_seed_resource_dir.clone(),
                Arc::new(speedtest_runner),
            );
            app.manage(AppState {
                database,
                config_store,
                config: shared_config,
                runtime_paths,
                core_seed_resource_dir,
                elevation_manager,
                supervisor,
                statistics_manager,
                speedtest_manager,
                system_proxy_manager,
                clash_monitor_controller: ClashMonitorController::new(),
                diagnostics_client: Arc::new(AsyncMutex::new(DiagnosticsClient::new())),
            });

            specta_builder.mount_events(app);
            setup_tray(app)?;
            ipc::commands::record_app_start_diagnostics(app.handle());

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
                TRAY_QUIT => {
                    shutdown_for_exit(app);
                    app.exit(0);
                }
                id => {
                    if let Some(action) = tray_privileged_action(id) {
                        handle_tray_privileged_action(app, action);
                    } else if let Some(server_id) = id.strip_prefix(TRAY_SERVER_PREFIX) {
                        spawn_tray_set_active_server(app, server_id.to_string());
                    }
                }
            },
        );

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    tray.build(app)?;

    Ok(())
}

fn tray_privileged_action(id: &str) -> Option<TrayPrivilegedAction> {
    match id {
        TRAY_CONNECT => Some(TrayPrivilegedAction::Connect),
        TRAY_DISCONNECT => Some(TrayPrivilegedAction::Disconnect),
        TRAY_PROXY_CLEAR => Some(TrayPrivilegedAction::Proxy(SysProxyType::ForcedClear)),
        TRAY_PROXY_SET => Some(TrayPrivilegedAction::Proxy(SysProxyType::ForcedChange)),
        TRAY_PROXY_UNCHANGED => Some(TrayPrivilegedAction::Proxy(SysProxyType::Unchanged)),
        TRAY_PROXY_PAC if cfg!(target_os = "windows") => {
            Some(TrayPrivilegedAction::Proxy(SysProxyType::Pac))
        }
        _ => None,
    }
}

fn handle_tray_privileged_action<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    action: TrayPrivilegedAction,
) {
    match action {
        TrayPrivilegedAction::Connect => spawn_tray_connect(app),
        TrayPrivilegedAction::Disconnect => spawn_tray_disconnect(app),
        TrayPrivilegedAction::Proxy(mode) => spawn_tray_proxy_mode(app, mode),
    }
}

pub(crate) fn refresh_tray_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id("main") else {
        return Ok(());
    };
    let menu = build_tray_menu(app)?;
    tray.set_menu(Some(menu))
}

fn build_tray_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<Menu<R>> {
    let state = app.state::<AppState>();
    let config = state
        .config()
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let profiles = tauri::async_runtime::block_on(state.database().profiles().list())
        .unwrap_or_else(|error| {
            tracing::warn!(?error, "failed to load profiles for tray menu");
            Vec::new()
        });

    let show = MenuItem::with_id(app, TRAY_SHOW, "Show VoyaVPN", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, TRAY_HIDE, "Hide Window", true, None::<&str>)?;
    let connect = MenuItem::with_id(app, TRAY_CONNECT, "Connect", true, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, TRAY_DISCONNECT, "Disconnect", true, None::<&str>)?;
    let servers_menu = build_tray_servers_menu(app, &config, profiles)?;
    let proxy_menu = build_tray_proxy_menu(app, &config)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT, "Quit", true, None::<&str>)?;
    let status_separator = PredefinedMenuItem::separator(app)?;
    let quit_separator = PredefinedMenuItem::separator(app)?;

    Menu::with_items(
        app,
        &[
            &show as &dyn IsMenuItem<R>,
            &hide,
            &status_separator,
            &connect,
            &disconnect,
            &servers_menu,
            &proxy_menu,
            &quit_separator,
            &quit,
        ],
    )
}

fn build_tray_servers_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: &AppConfig,
    profiles: Vec<ProfileItem>,
) -> tauri::Result<Submenu<R>> {
    let limit = usize::try_from(config.gui_item.tray_menu_servers_limit.max(0)).unwrap_or(0);
    if limit == 0 || profiles.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "tray-server-empty",
            "No recent servers",
            false,
            None::<&str>,
        )?;
        return Submenu::with_id_and_items(app, "tray-servers", "Recent Servers", true, &[&empty]);
    }
    if profiles.len() > limit {
        let hidden = MenuItem::with_id(
            app,
            "tray-server-hidden",
            "Recent servers hidden by limit",
            false,
            None::<&str>,
        )?;
        return Submenu::with_id_and_items(app, "tray-servers", "Recent Servers", true, &[&hidden]);
    }

    let mut items = Vec::new();
    for profile in profiles {
        let active = profile.index_id == config.index_id;
        let label = if active {
            format!("✓ {}", tray_profile_label(&profile))
        } else {
            tray_profile_label(&profile)
        };
        items.push(MenuItem::with_id(
            app,
            format!("{TRAY_SERVER_PREFIX}{}", profile.index_id),
            label,
            true,
            None::<&str>,
        )?);
    }
    let refs = items
        .iter()
        .map(|item| item as &dyn IsMenuItem<R>)
        .collect::<Vec<_>>();

    Submenu::with_id_and_items(app, "tray-servers", "Recent Servers", true, &refs)
}

fn build_tray_proxy_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: &AppConfig,
) -> tauri::Result<Submenu<R>> {
    let active = config.system_proxy_item.sys_proxy_type;
    let pac_available = cfg!(target_os = "windows");
    let mut items = vec![
        CheckMenuItem::with_id(
            app,
            TRAY_PROXY_CLEAR,
            "Clear System Proxy",
            true,
            active == SysProxyType::ForcedClear,
            None::<&str>,
        )?,
        CheckMenuItem::with_id(
            app,
            TRAY_PROXY_SET,
            "Set System Proxy",
            true,
            active == SysProxyType::ForcedChange,
            None::<&str>,
        )?,
        CheckMenuItem::with_id(
            app,
            TRAY_PROXY_UNCHANGED,
            "Do Not Change",
            true,
            active == SysProxyType::Unchanged,
            None::<&str>,
        )?,
    ];
    if pac_available {
        items.push(CheckMenuItem::with_id(
            app,
            TRAY_PROXY_PAC,
            "PAC",
            true,
            active == SysProxyType::Pac,
            None::<&str>,
        )?);
    }
    let refs = items
        .iter()
        .map(|item| item as &dyn IsMenuItem<R>)
        .collect::<Vec<_>>();

    Submenu::with_id_and_items(app, "tray-proxy", "System Proxy", true, &refs)
}

struct TauriProcessLogSink {
    app: tauri::AppHandle,
}

struct TauriStatisticsEventSink {
    app: tauri::AppHandle,
}

struct TauriClashEventSink {
    app: tauri::AppHandle,
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
                server_stat: snapshot.server_stat,
            });

        if let Err(error) = event.emit(&self.app) {
            tracing::warn!(?error, "failed to emit statistics event");
        }
    }
}

impl ClashEventSink for TauriClashEventSink {
    fn emit_traffic(&self, event: ClashTrafficEvent) {
        let event = ipc::events::TransientStreamEvent::ClashTraffic(event);

        if let Err(error) = event.emit(&self.app) {
            tracing::warn!(?error, "failed to emit Clash traffic event");
        }
    }

    fn emit_connections(&self, event: ClashConnectionsSnapshot) {
        let event = ipc::events::TransientStreamEvent::ClashConnections(event);

        if let Err(error) = event.emit(&self.app) {
            tracing::warn!(?error, "failed to emit Clash connections event");
        }
    }
}

impl ProcessLogSink for TauriProcessLogSink {
    fn line(&self, role: ProcessRole, stream: ProcessOutputStream, line: String) {
        let level = if stream == ProcessOutputStream::Stderr {
            ipc::events::LogLevel::Warn
        } else {
            ipc::events::LogLevel::Info
        };
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

fn tray_profile_label(profile: &ProfileItem) -> String {
    let mut label = if profile.remarks.trim().is_empty() {
        profile.address.clone()
    } else {
        profile.remarks.clone()
    };
    if label.chars().count() > 64 {
        label = label.chars().take(61).collect::<String>();
        label.push_str("...");
    }
    label
}

fn spawn_tray_connect<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = ipc::commands::connect_active_profile(app.clone(), state).await {
            tracing::warn!(?error, "tray connect failed");
        }
        if let Err(error) = refresh_tray_menu(&app) {
            tracing::warn!(?error, "failed to refresh tray after connect");
        }
    });
}

fn spawn_tray_disconnect<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = ipc::commands::disconnect_core(app.clone(), state).await {
            tracing::warn!(?error, "tray disconnect failed");
        }
        if let Err(error) = refresh_tray_menu(&app) {
            tracing::warn!(?error, "failed to refresh tray after disconnect");
        }
    });
}

fn spawn_tray_proxy_mode<R: tauri::Runtime>(app: &tauri::AppHandle<R>, mode: SysProxyType) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = ipc::commands::set_system_proxy_mode(app.clone(), state, mode) {
            tracing::warn!(?error, "tray system proxy mode switch failed");
        }
    });
}

fn spawn_tray_set_active_server<R: tauri::Runtime>(app: &tauri::AppHandle<R>, index_id: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(error) = ipc::commands::set_active_profile(app.clone(), state, index_id).await {
            tracing::warn!(?error, "tray server switch failed");
        }
        if let Err(error) = refresh_tray_menu(&app) {
            tracing::warn!(?error, "failed to refresh tray after server switch");
        }
    });
}

fn shutdown_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
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
    let runtime = RuntimeManager::new(
        state.database(),
        state.runtime_paths().clone(),
        state.supervisor(),
    );
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
    if let Err(error) = state.clash_monitor_controller().stop() {
        tracing::warn!(?error, "failed to stop Clash monitor on exit");
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
