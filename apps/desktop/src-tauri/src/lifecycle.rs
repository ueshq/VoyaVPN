//! Ordered, once-only shutdown of background work and OS resources.
use crate::AppState;
use tauri::Manager;
use voya_app::lifecycle::ShutdownLatch;

/// Latches the exit teardown so it runs once per process.
static SHUTDOWN_LATCH: ShutdownLatch = ShutdownLatch::new();

pub(super) fn shutdown_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    // Tauri raises ExitRequested and then Exit for every exit path, so this is
    // reached at least twice per quit. The sequence re-runs the sudoers revoke
    // and the per-service system-proxy restore, so it is latched to one pass.
    if !SHUTDOWN_LATCH.begin() {
        return;
    }
    // Stopped before the runtime is torn down so the scheduler stops taking on
    // new subscriptions, abandons an in-flight download, and discards a fetch
    // that already finished rather than publishing it into a runtime that is
    // going away. This waits for the loop to actually leave: Tauri ends the
    // process with `std::process::exit`, so a commit that had already begun
    // would otherwise be raced by the exit and redone next launch.
    if let Some(state) = app.try_state::<AppState>() {
        tauri::async_runtime::block_on(state.subscription_auto_update().shutdown());
        // Tauri exits the process directly, so nothing here is ever dropped:
        // a speedtest still in flight would leave its temporary sing-box probe
        // cores running with open outbound tunnels after the app is gone.
        state.speedtest_manager().shutdown();
        // The self-hosted core, its router forwards and its watch loop go down
        // before the connection core, whose tunnel may carry the unmap calls.
        tauri::async_runtime::block_on(state.self_host().shutdown());
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
    let runtime = state.services().runtime(state.supervisor());
    match tauri::async_runtime::block_on(runtime.disconnect()) {
        Ok(_) => true,
        Err(error) => {
            tracing::warn!(?error, "failed to disconnect runtime on exit");
            false
        }
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
    tauri::async_runtime::block_on(state.statistics_manager().shutdown());
    if let Err(error) = state.proxy_monitor_controller().stop() {
        tracing::warn!(?error, "failed to stop proxy monitor on exit");
    }
}
