//! The WebView's `navigator.clipboard.read*` shows WebKit's "Paste"
//! confirmation on every call, so clipboard reads go through the platform.
use voya_app::qr::QrCodeManager;
use voya_contracts::{AppError, AppErrorKind, AppErrorSubsystem, QrScanResult};
use voya_platform::clipboard;

/// Returns an empty string when the clipboard holds no text.
#[tauri::command]
#[specta::specta]
pub async fn read_clipboard_text() -> Result<String, AppError> {
    super::support::run_blocking("clipboard read", clipboard::read_text)
        .await?
        .map(Option::unwrap_or_default)
        .map_err(|failure| {
            AppError::new(
                AppErrorSubsystem::App,
                AppErrorKind::Io,
                failure.to_string(),
            )
        })
}

#[tauri::command]
#[specta::specta]
pub async fn scan_clipboard_qr() -> Result<QrScanResult, AppError> {
    super::support::run_blocking("clipboard QR scan", || {
        QrCodeManager.decode_clipboard_image(clipboard::read_image())
    })
    .await
}
