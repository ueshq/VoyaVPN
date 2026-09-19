use specta_typescript::Typescript;
use std::{error::Error, path::Path};
use tauri::RunEvent;

mod app_state;
mod bootstrap;
mod event_sinks;
mod ipc;
mod lifecycle;
mod logging;
mod residency;
mod tray;

pub(crate) use app_state::AppState;
use bootstrap::{database_path, initialize, record_startup_failure, report_startup_failure};
pub(crate) use event_sinks::TauriProxyRuntimeEventSink;
use lifecycle::shutdown_for_exit;
pub(crate) use tray::refresh_tray_menu;

pub fn export_bindings(path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    ipc::specta_builder().export(Typescript::default(), path)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = ipc::specta_builder();

    // The path is baked in at compile time, so a packaged debug build moved to
    // another machine would panic before a window exists. `pnpm generate:bindings`
    // (the `export-bindings` example) is the canonical path; this convenience
    // export is limited to `tauri dev` and an explicit opt-in, and never fatal.
    #[cfg(debug_assertions)]
    if tauri::is_dev() || std::env::var_os("VOYAVPN_EXPORT_BINDINGS").is_some() {
        if let Err(error) =
            export_bindings(Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/bindings.ts"))
        {
            tracing::warn!(%error, "failed to export TypeScript IPC bindings");
        }
    }

    #[expect(
        clippy::expect_used,
        reason = "only the Tauri runtime itself failing lands here; `initialize` reports \
                  every failure the user can act on"
    )]
    let app = tauri::Builder::default()
        // First, so a second launch hands over before anything else starts.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            residency::show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        // OS notifications for a user whose window is hidden in the tray; the
        // renderer decides when to show one (`src/ipc/notifications.ts`).
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    residency::handle_close_requested(window);
                }
            }
        })
        .setup(move |app| {
            // Startup logging and background tasks emit typed events. Mount
            // their registry first: even a recoverable startup warning would
            // otherwise panic in the log-panel layer before setup completes.
            specta_builder.mount_events(app);
            if let Err(error) = initialize(app) {
                // Nothing can be shown from here: every dialog API round-trips
                // through the main thread's event loop, which only starts after
                // `setup` returns, so a blocking dialog would hang the launch.
                // The message is stashed and rendered on `RunEvent::Ready`.
                tracing::error!(%error, "VoyaVPN failed to start");
                let resettable_database =
                    voya_app::startup::offers_database_reset(error.as_ref())
                        .then(|| database_path(app).ok())
                        .flatten();
                record_startup_failure(error.to_string(), resettable_database);
            } else {
                residency::show_after_launch(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build VoyaVPN");

    app.run(|app, event| {
        match event {
            // The first point at which a native dialog can be shown.
            RunEvent::Ready => {
                #[cfg(target_os = "macos")]
                ipc::window::install_native_caption_inset(app);
                report_startup_failure(app);
            }
            RunEvent::ExitRequested { .. } => shutdown_for_exit(app),
            RunEvent::Exit => shutdown_for_exit(app),
            // The dock icon brings back a window hidden into the tray.
            #[cfg(target_os = "macos")]
            RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => residency::show_main_window(app),
            _ => {}
        }
    });
}
