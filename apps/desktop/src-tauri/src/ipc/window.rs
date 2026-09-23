use voya_contracts::{AppError, TitleBarLayout, WindowChromeConfig};

#[cfg(target_os = "macos")]
pub(crate) fn install_native_caption_inset(app: &tauri::AppHandle) {
    use tauri::Manager;

    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
    else {
        return;
    };
    let Some(position) = &config.traffic_light_position else {
        return;
    };
    let (left, top) = (position.x, position.y);
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.with_webview(move |webview| {
        // SAFETY: with_webview runs on the main thread and lends the live NSWindow.
        unsafe {
            voya_platform::window_chrome::install_native_caption_inset(
                webview.ns_window(),
                left,
                top,
            );
        }
    }) {
        tracing::warn!(%error, "could not install native caption inset");
    }
}

/// The five window-chrome actions the custom title bar drives.
///
/// They used to go through `@tauri-apps/api/window`'s `getCurrentWindow()`,
/// which pulls `window.js`/`dpi.js`/`image.js` into the startup vendor chunk.
/// A thin Rust command keeps the same behavior and leaves those modules out.
#[tauri::command]
#[specta::specta]
pub fn minimize_window(window: tauri::WebviewWindow) -> Result<(), AppError> {
    window.minimize().map_err(window_error)
}

#[tauri::command]
#[specta::specta]
pub fn toggle_maximize_window(window: tauri::WebviewWindow) -> Result<(), AppError> {
    if window.is_maximized().map_err(window_error)? {
        window.unmaximize().map_err(window_error)
    } else {
        window.maximize().map_err(window_error)
    }
}

#[tauri::command]
#[specta::specta]
pub fn close_window(window: tauri::WebviewWindow) -> Result<(), AppError> {
    window.close().map_err(window_error)
}

#[tauri::command]
#[specta::specta]
pub fn is_window_maximized(window: tauri::WebviewWindow) -> Result<bool, AppError> {
    window.is_maximized().map_err(window_error)
}

/// Whether the window is on screen rather than hidden into the tray.
#[tauri::command]
#[specta::specta]
pub fn is_window_visible(window: tauri::WebviewWindow) -> Result<bool, AppError> {
    window.is_visible().map_err(window_error)
}

fn window_error(error: tauri::Error) -> AppError {
    AppError::internal(voya_contracts::AppErrorSubsystem::App, error.to_string())
}

/// macOS overlays native traffic lights on the webview; Windows renders caption
/// buttons in its borderless window. Linux and the web fallback use `none`.
#[tauri::command]
#[specta::specta]
pub fn get_window_chrome_config() -> Result<WindowChromeConfig, AppError> {
    #[cfg(target_os = "windows")]
    let title_bar_layout = TitleBarLayout::Windows;
    #[cfg(target_os = "macos")]
    let title_bar_layout = TitleBarLayout::Macos;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let title_bar_layout = TitleBarLayout::None;

    Ok(WindowChromeConfig { title_bar_layout })
}

/// Tint the Windows Acrylic blur material to match the in-app light/dark theme.
/// The frontend drives its own (non-system) theme, so this command sets the tint
/// explicitly per mode to keep the native material's base color aligned with the
/// UI. Acrylic has a single variant; light and dark differ only by `color`.
///
/// Non-Windows platforms are a no-op: window effects are an OS capability, and
/// macOS / Linux / web fall back to the flat CSS neutral-gray veil.
#[tauri::command]
#[specta::specta]
#[allow(unused_variables)]
pub fn set_window_acrylic(window: tauri::WebviewWindow, dark: bool) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        use tauri::window::{Color, Effect, EffectsBuilder};
        // Higher alpha reads as a more solid, controllable gray; lower is glassier.
        // Neutral gray, one tint per mode — kept in sync with the `.voyavpn-acrylic`
        // veil in globals.css so the native material and the CSS layer agree.
        let color = if dark {
            Color(1, 4, 9, 200)
        } else {
            Color(246, 248, 250, 200)
        };
        window
            .set_effects(
                EffectsBuilder::new()
                    .effect(Effect::Acrylic)
                    .color(color)
                    .build(),
            )
            .map_err(window_error)?;
    }
    Ok(())
}
