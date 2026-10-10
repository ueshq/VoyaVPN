//! The main window's life beyond its close button: hiding into the tray,
//! coming back, and what a close request does.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::ipc::events::Emit;
use tauri::Manager;
use voya_app::lifecycle::{close_request_decision, launch_hidden, CloseDecision};
use voya_app::tray::{tray_labels, TrayLabels};
use voya_platform::autostart::launched_by_autostart;

use crate::{tray::TRAY_ID, AppState};
use voya_contracts::AppEvent;

const MAIN_WINDOW: &str = "main";

/// Whether this launch has already said that closing keeps VoyaVPN running.
static IN_TRAY_NOTICE_SHOWN: AtomicBool = AtomicBool::new(false);

pub(crate) fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    if let Err(error) = window.unminimize() {
        tracing::warn!(?error, "failed to restore the main window");
    }
    if let Err(error) = window.show() {
        tracing::warn!(?error, "failed to show the main window");
    }
    if let Err(error) = window.set_focus() {
        tracing::warn!(?error, "failed to focus the main window");
    }
    #[cfg(target_os = "macos")]
    with_ns_window(app, window, "raise the main window", |ns_window| {
        // SAFETY: `with_ns_window` runs this on the main thread with the live
        // NSWindow of the main window.
        unsafe { voya_platform::window_chrome::raise(ns_window) }
    });
    refresh_tray(app);
}

/// Runs `action` on the main thread with the window's NSWindow. `purpose`
/// names the action in the warning logged when the handle cannot be had.
#[cfg(target_os = "macos")]
fn with_ns_window<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    purpose: &'static str,
    action: impl FnOnce(*mut std::ffi::c_void) + Send + 'static,
) {
    let run = move || match window.ns_window() {
        Ok(ns_window) => action(ns_window),
        Err(error) => tracing::warn!(?error, "failed to {purpose}"),
    };
    if let Err(error) = app.run_on_main_thread(run) {
        tracing::warn!(?error, "failed to {purpose}");
    }
}

pub(crate) fn hide_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    #[cfg(target_os = "macos")]
    if window.is_fullscreen().unwrap_or(false) {
        hide_leaving_fullscreen(app, window);
        return;
    }
    if let Err(error) = window.hide() {
        tracing::warn!(?error, "failed to hide the main window");
    }
    refresh_tray(app);
}

/// Hiding a full-screen window outright leaves its Space behind as a black
/// screen, so it leaves full screen first and hides when that has finished.
/// The tray follows through the focus change the hide causes.
#[cfg(target_os = "macos")]
fn hide_leaving_fullscreen<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
) {
    with_ns_window(
        app,
        window,
        "hide the full-screen main window",
        |ns_window| {
            // SAFETY: `with_ns_window` runs this on the main thread with the
            // live NSWindow of the main window.
            unsafe { voya_platform::window_chrome::hide_leaving_fullscreen(ns_window) }
        },
    );
}

/// Whether the main window is on screen; the tray offers Show or Hide by it.
/// A window minimized to the Dock still counts as visible to the OS, but the
/// user is looking for a way to get it back.
pub(crate) fn main_window_visible<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    app.get_webview_window(MAIN_WINDOW).is_some_and(|window| {
        window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false)
    })
}

/// The tray's Show/Hide entry follows the window.
fn refresh_tray<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Err(error) = crate::refresh_tray_menu(app) {
        tracing::warn!(?error, "failed to refresh the tray after a window change");
    }
}

/// Hides the window because the user chose to keep VoyaVPN in the tray.
///
/// A window that just vanishes reads as a quit while the connection is still
/// up, so the first such hide of each launch says so. The tray's own "Hide
/// Window" item does not come through here: that user is already at the tray.
pub(crate) fn hide_into_tray<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    hide_main_window(app);
    if !IN_TRAY_NOTICE_SHOWN.swap(true, Ordering::SeqCst) {
        let labels = current_labels(app);
        notify(app, labels.in_tray_title, Some(labels.in_tray_body));
    }
}

pub(crate) fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if main_window_visible(app) {
        hide_main_window(app);
    } else {
        show_main_window(app);
    }
}

/// Hides, quits or asks, per `behavior.closeAction`. The caller has already
/// prevented the native close.
pub(crate) fn handle_close_requested<R: tauri::Runtime>(window: &tauri::Window<R>) {
    let app = window.app_handle();
    let tray_available = app.tray_by_id(TRAY_ID).is_some();
    // Before startup has managed state there is nothing to keep running.
    let decision = app
        .try_state::<AppState>()
        .map_or(CloseDecision::Quit, |state| {
            close_request_decision(&state.config_mutations().current_config(), tray_available)
        });
    match decision {
        CloseDecision::Hide => hide_into_tray(app),
        CloseDecision::Quit => {
            // Without a tray icon a close quits whatever the setting says;
            // a user who chose to keep running is told why the app went away.
            if !tray_available && wanted_to_keep_running(app) {
                notify(app, current_labels(app).quit_without_tray, None);
            }
            crate::lifecycle::quit(app);
        }
        CloseDecision::Ask => {
            if let Err(error) = AppEvent::CloseRequested.emit(app) {
                tracing::warn!(
                    ?error,
                    "failed to ask how to close; keeping the app in the tray"
                );
                hide_into_tray(app);
            }
        }
    }
}

/// Whether the close action would have kept the app running had a tray icon
/// existed.
fn wanted_to_keep_running<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    app.try_state::<AppState>().is_some_and(|state| {
        close_request_decision(&state.config_mutations().current_config(), true)
            != CloseDecision::Quit
    })
}

/// Notification text in the language the app is set to, like the tray menu.
fn current_labels<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> TrayLabels {
    let language = app
        .try_state::<AppState>()
        .map(|state| {
            state
                .config_mutations()
                .current_config()
                .appearance
                .language
                .clone()
        })
        .unwrap_or_default();
    tray_labels(&language)
}

/// Posts an OS notification. macOS asks the user to allow them the first
/// time, through `UNUserNotificationCenter`; Windows and Linux have nothing
/// to ask and go through the notification plugin.
pub(crate) fn notify<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    title: &str,
    body: Option<&str>,
) {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        if !voya_platform::notifications::post(title, body) {
            tracing::warn!("notifications need the app bundle; none was shown");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        use tauri_plugin_notification::NotificationExt;

        let mut notification = app.notification().builder().title(title);
        if let Some(body) = body {
            notification = notification.body(body);
        }
        if let Err(error) = notification.show() {
            tracing::warn!(?error, "failed to show a notification");
        }
    }
}

/// Shows the window at the end of startup, unless this is a login launch that
/// asked to start in the tray. The window is created hidden so that launch
/// never flashes it.
pub(crate) fn show_after_launch<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let stay_hidden = app.tray_by_id(TRAY_ID).is_some()
        && app.try_state::<AppState>().is_some_and(|state| {
            launch_hidden(
                &state.config_mutations().current_config(),
                launched_by_autostart(std::env::args_os()),
            )
        });
    if !stay_hidden {
        show_main_window(app);
    }
}
