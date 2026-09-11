use crate::AppState;
use tauri::{
    menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use voya_app::tray::tray_labels;

const TRAY_SHOW: &str = "tray-show";
const TRAY_HIDE: &str = "tray-hide";
const TRAY_QUIT: &str = "tray-quit";
pub(super) fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
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

/// Builds the tray menu in the language the app is set to.
///
/// The tray is native, built before any webview exists and rebuilt off the main
/// thread, so it cannot call `t()`. `voya_app::tray` holds the table; the
/// language comes from the persisted `ui_item.current_language`, which is the
/// same value the frontend picks its locale from. Before startup has managed
/// `AppState` there is no configuration to read and the table's English
/// fallback applies.
fn build_tray_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<Menu<R>> {
    let labels = tray_labels(&current_interface_language(app));
    let show = MenuItem::with_id(app, TRAY_SHOW, labels.show, true, None::<&str>)?;
    let hide = MenuItem::with_id(app, TRAY_HIDE, labels.hide, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT, labels.quit, true, None::<&str>)?;
    let quit_separator = PredefinedMenuItem::separator(app)?;

    Menu::with_items(
        app,
        &[&show as &dyn IsMenuItem<R>, &hide, &quit_separator, &quit],
    )
}

/// The interface language the tray labels itself in.
///
/// Read straight from the shared `AppConfig` rather than from a command, so a
/// tray rebuild triggered off the main thread never has to wait on the runtime.
fn current_interface_language<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> String {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .config()
                .read()
                .ok()
                .map(|config| config.ui_item.current_language.clone())
        })
        .unwrap_or_default()
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
