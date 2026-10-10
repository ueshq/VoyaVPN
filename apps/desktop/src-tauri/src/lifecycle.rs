//! Ordered, once-only shutdown of background work and OS resources.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Once,
    },
    time::Duration,
};

use crate::{tray::TRAY_ID, AppState};
use tauri::Manager;
use voya_app::lifecycle::exit_step;

/// How long each waiting step of the teardown may take. Each sits above the
/// step's own worst case, so it only ends a wait that would never have ended:
/// stopping an elevated core alone may take the privileged kill's 30 seconds.
const AUTO_UPDATE_EXIT_LIMIT: Duration = Duration::from_secs(10);
const SELF_HOST_EXIT_LIMIT: Duration = Duration::from_secs(20);
const DISCONNECT_EXIT_LIMIT: Duration = Duration::from_secs(35);
const STATISTICS_EXIT_LIMIT: Duration = Duration::from_secs(5);

/// Runs the exit teardown once per process. A caller that arrives while it is
/// running waits for it: the process must not end with the core half stopped.
static TEARDOWN: Once = Once::new();
/// Set by the first quit request; later ones are the same request.
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Quits from inside the app: the tray's Quit item and a close that quits.
///
/// The teardown waits on the core, the router and a final statistics flush,
/// each for seconds. Run on the main thread it froze the window and the tray
/// menu for that long, so the app leaves the screen first and the teardown
/// runs on its own thread, which then ends the process.
pub(crate) fn quit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if QUIT_REQUESTED.swap(true, Ordering::SeqCst) {
        return;
    }
    leave_the_screen(app);
    let handle = app.clone();
    let teardown = std::thread::Builder::new()
        .name("voya-quit".to_string())
        .spawn(move || {
            shutdown_for_exit(&handle);
            handle.exit(0);
        });
    if let Err(error) = teardown {
        tracing::warn!(%error, "could not start the quit thread; exiting directly");
        // The exit events run the teardown on the main thread instead.
        app.exit(0);
    }
}

/// Hides the window and the tray icon, so nothing that can no longer answer
/// stays on screen while the teardown runs. Both calls go through the main
/// thread, so this must not be called from inside the teardown: a main thread
/// waiting for the teardown would never serve them.
pub(super) fn leave_the_screen<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.hide() {
            tracing::debug!(?error, "failed to hide the main window while quitting");
        }
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Err(error) = tray.set_visible(false) {
            tracing::debug!(?error, "failed to hide the tray icon while quitting");
        }
    }
}

pub(super) fn shutdown_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    // Tauri raises ExitRequested and then Exit for every exit path, and `quit`
    // gets here first on its own thread. The sequence re-runs the sudoers
    // revoke and the per-service system-proxy restore, so it runs once.
    TEARDOWN.call_once(|| teardown(app));
}

fn teardown<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    // Stopped before the runtime is torn down so the scheduler stops taking on
    // new subscriptions, abandons an in-flight download, and discards a fetch
    // that already finished rather than publishing it into a runtime that is
    // going away. This waits for the loop to actually leave: Tauri ends the
    // process with `std::process::exit`, so a commit that had already begun
    // would otherwise be raced by the exit and redone next launch.
    if let Some(state) = app.try_state::<AppState>() {
        tauri::async_runtime::block_on(exit_step(
            "subscription auto-update",
            AUTO_UPDATE_EXIT_LIMIT,
            state.subscription_auto_update().shutdown(),
        ));
        // Tauri exits the process directly, so nothing here is ever dropped:
        // a speedtest still in flight would leave its temporary sing-box probe
        // cores running with open outbound tunnels after the app is gone.
        state.speedtest_manager().shutdown();
        // The self-hosted core, its router forwards and its watch loop go down
        // before the connection core, whose tunnel may carry the unmap calls.
        tauri::async_runtime::block_on(exit_step(
            "self-hosted node",
            SELF_HOST_EXIT_LIMIT,
            state.self_host().shutdown(),
        ));
    }
    // The root launcher is the only passwordless way to kill an elevated core,
    // so it is only removed once the core is confirmed stopped. Removing it
    // while a root sing-box still holds the TUN device would strand that
    // process with no kill path at all; leaving it installed lets the next
    // launch's stale-grant sweep clean up instead.
    if disconnect_runtime_for_exit(app) {
        revoke_elevation_for_exit(app);
    } else {
        tracing::error!(
            "keeping the TUN elevation launcher installed: the core did not stop, \
             so removing it would leave an elevated core with no kill path"
        );
    }
    restore_system_proxy_for_exit(app);
    stop_monitoring_for_exit(app);
}

fn revoke_elevation_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    // Runs after the runtime disconnect so the elevated core is already stopped
    // (via the launcher) before the launcher + sudoers drop-in are removed.
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    state.elevation_manager().revoke();
}

/// Stop the core. Returns whether it is known to be gone.
fn disconnect_runtime_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        // Startup never got far enough to spawn anything through the launcher.
        return true;
    };
    let runtime = crate::ipc::commands::runtime_manager(&state);
    match tauri::async_runtime::block_on(exit_step(
        "core disconnect",
        DISCONNECT_EXIT_LIMIT,
        runtime.disconnect(),
    )) {
        Some(Ok(_)) => true,
        Some(Err(error)) => {
            tracing::warn!(?error, "failed to disconnect runtime on exit");
            false
        }
        // Not known to be gone, which is the case the caller keeps the
        // elevation launcher for.
        None => false,
    }
}

fn restore_system_proxy_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let config = state.config_mutations().current_config();
    if let Err(error) = state.system_proxy_manager().restore(&config) {
        tracing::warn!(?error, "failed to restore system proxy on exit");
    }
}

/// Runs last, once the core is down, so the whole session's traffic counts.
fn stop_monitoring_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    // Waits for the statistics loop's final flush: nothing is dropped on the
    // way out, so buffered traffic would otherwise be lost.
    tauri::async_runtime::block_on(exit_step(
        "statistics flush",
        STATISTICS_EXIT_LIMIT,
        state.statistics_manager().shutdown(),
    ));
    if let Err(error) = state.proxy_monitor_controller().stop() {
        tracing::warn!(?error, "failed to stop proxy monitor on exit");
    }
}
