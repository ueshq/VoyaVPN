//! The tray icon and its menu: connection, traffic mode and node controls,
//! rebuilt whenever the state they show changes.
//!
//! The menu itself is modelled and translated in `voya_app::tray`, where it is
//! tested; this module only turns that model into native items and routes
//! their clicks.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::image::Image;
use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use voya_app::tray::{tray_menu, tray_tooltip, with_connected_badge, TrayEntry, TrayItemId};
use voya_platform::coreinfo::TargetOs;

use crate::{
    ipc::{
        commands,
        events::{AppEvent, ShellTabTarget},
    },
    residency, AppState,
};

pub(crate) const TRAY_ID: &str = "main";

/// Set while a rebuild is queued, so a burst of state changes costs one.
static REFRESH_QUEUED: AtomicBool = AtomicBool::new(false);

pub(super) fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    // Built without the node list, then refreshed off the startup path.
    let snapshot = commands::initial_tray_snapshot(&app.state::<AppState>());
    let window_visible = residency::main_window_visible(app.handle());
    let menu = build_menu(app.handle(), &tray_menu(&snapshot.input(window_visible)))?;
    // Windows convention: left click opens the window and right click the
    // menu. Elsewhere the menu opens on any click.
    let left_click_toggles_window = TargetOs::current() == TargetOs::Windows;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(tray_tooltip(
            snapshot.connected_node(),
            snapshot.traffic_mode_label(),
        ))
        .show_menu_on_left_click(!left_click_toggles_window)
        .on_menu_event(|app, event: MenuEvent| handle_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(move |tray, event| {
            if left_click_toggles_window
                && matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                )
            {
                residency::toggle_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = tray_icon(app.handle(), snapshot.connected()) {
        tray = tray.icon(icon);
    }

    tray.build(app)?;

    refresh_tray_menu(app.handle())
}

/// Queues a rebuild of the tray from the app's current state.
///
/// The rebuild reads the supervisor and the node list, so it runs on the async
/// runtime; callers only queue it and never wait on it.
pub(crate) fn refresh_tray_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    if app.tray_by_id(TRAY_ID).is_none() || REFRESH_QUEUED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        REFRESH_QUEUED.store(false, Ordering::SeqCst);
        if let Err(error) = rebuild_tray(&app).await {
            tracing::warn!(?error, "failed to rebuild the tray menu");
        }
    });

    Ok(())
}

async fn rebuild_tray<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let (Some(tray), Some(state)) = (app.tray_by_id(TRAY_ID), app.try_state::<AppState>()) else {
        return Ok(());
    };
    let snapshot = commands::tray_snapshot(&state).await;
    let window_visible = residency::main_window_visible(app);
    tray.set_menu(Some(build_menu(
        app,
        &tray_menu(&snapshot.input(window_visible)),
    )?))?;
    tray.set_icon(tray_icon(app, snapshot.connected()))?;
    tray.set_tooltip(Some(tray_tooltip(
        snapshot.connected_node(),
        snapshot.traffic_mode_label(),
    )))
}

/// The app icon, with a green dot while connected.
fn tray_icon<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    connected: bool,
) -> Option<Image<'static>> {
    let base = app.default_window_icon()?;
    let (width, height) = (base.width(), base.height());
    let rgba = if connected {
        with_connected_badge(base.rgba(), width, height)
    } else {
        base.rgba().to_vec()
    };
    Some(Image::new_owned(rgba, width, height))
}

fn build_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    entries: &[TrayEntry],
) -> tauri::Result<Menu<R>> {
    let items = native_items(app, entries)?;
    Menu::with_items(app, &item_refs(&items))
}

fn native_items<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    entries: &[TrayEntry],
) -> tauri::Result<Vec<Box<dyn IsMenuItem<R>>>> {
    entries
        .iter()
        .map(|entry| -> tauri::Result<Box<dyn IsMenuItem<R>>> {
            Ok(match entry {
                TrayEntry::Item { id, label, enabled } => Box::new(MenuItem::with_id(
                    app,
                    id.encode(),
                    label,
                    *enabled,
                    None::<&str>,
                )?),
                TrayEntry::Check { id, label, checked } => Box::new(CheckMenuItem::with_id(
                    app,
                    id.encode(),
                    label,
                    true,
                    *checked,
                    None::<&str>,
                )?),
                TrayEntry::Separator => Box::new(PredefinedMenuItem::separator(app)?),
                TrayEntry::Submenu { label, entries } => {
                    let children = native_items(app, entries)?;
                    Box::new(Submenu::with_items(
                        app,
                        label,
                        true,
                        &item_refs(&children),
                    )?)
                }
            })
        })
        .collect()
}

fn item_refs<R: tauri::Runtime>(items: &[Box<dyn IsMenuItem<R>>]) -> Vec<&dyn IsMenuItem<R>> {
    items.iter().map(|item| &**item).collect()
}

fn handle_menu_event<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str) {
    let Some(item) = TrayItemId::parse(id) else {
        return;
    };
    match item {
        TrayItemId::Show => residency::show_main_window(app),
        TrayItemId::Hide => residency::hide_main_window(app),
        // `exit` raises ExitRequested and Exit, which is where the teardown
        // runs; calling it here as well would only repeat it.
        TrayItemId::Quit => app.exit(0),
        TrayItemId::AllNodes => {
            residency::show_main_window(app);
            commands::emit_or_warn(
                app,
                AppEvent::SelectTab(ShellTabTarget::Profiles),
                "node list tab request",
            );
        }
        action => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                match action {
                    TrayItemId::Connect => commands::tray_connect(&app).await,
                    TrayItemId::Disconnect => commands::tray_disconnect(&app).await,
                    TrayItemId::TrafficMode(mode) => {
                        commands::tray_set_traffic_mode(&app, mode).await;
                    }
                    TrayItemId::Node(id) => commands::tray_activate_node(&app, id).await,
                    TrayItemId::Group(id) => commands::tray_activate_group(&app, id).await,
                    _ => {}
                }
                // A native check item toggles itself when clicked; the rebuild
                // puts it back in line with what actually happened.
                if let Err(error) = refresh_tray_menu(&app) {
                    tracing::warn!(?error, "failed to queue a tray menu refresh");
                }
            });
        }
    }
}
