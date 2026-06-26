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
