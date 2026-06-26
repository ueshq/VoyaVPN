use serde::Serialize;
use specta::Type;

use super::commands::AppError;

/// Title-bar layout selected per platform. Windows uses a fully self-drawn
/// borderless bar (minimize/maximize/close + drag region); every other platform
/// keeps its native window frame and draws no custom title bar (`None`).
// Only one variant is constructed in a given single-platform build, so the
// others are legitimately never built; allow dead_code rather than cfg the enum.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum TitleBarLayout {
    Windows,
    None,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowChromeConfig {
    pub title_bar_layout: TitleBarLayout,
}

/// Report the window decoration this build should render. Only Windows gets the
/// custom borderless title bar; macOS and Linux keep their native frame, and the
/// web fallback (no Tauri runtime) resolves to `none` on the frontend.
#[tauri::command]
#[specta::specta]
pub fn get_window_chrome_config() -> Result<WindowChromeConfig, AppError> {
    #[cfg(target_os = "windows")]
    let title_bar_layout = TitleBarLayout::Windows;
    #[cfg(not(target_os = "windows"))]
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
            Color(24, 25, 27, 200)
        } else {
            Color(234, 235, 238, 200)
        };
        window
            .set_effects(
                EffectsBuilder::new()
                    .effect(Effect::Acrylic)
                    .color(color)
                    .build(),
            )
            .map_err(|error| AppError::State(error.to_string()))?;
    }
    Ok(())
}
