//! Tauri owns window visibility; capture and decoding belong to the app/platform.
use std::sync::LazyLock;
use voya_app::qr::{QrCodeManager, ScreenQrCapture};
use voya_contracts::{AppError, AppErrorSubsystem, QrScanResult};
use voya_platform::screen_capture::{
    NativeScreenCapture, ScreenCaptureFailure, ScreenCaptureWindow,
};

static CAPTURE: LazyLock<ScreenQrCapture> = LazyLock::new(ScreenQrCapture::default);

struct ScanWindow {
    window: tauri::WebviewWindow,
    visible: bool,
    focused: bool,
}

impl ScreenCaptureWindow for ScanWindow {
    fn hide(&self) -> Result<(), ScreenCaptureFailure> {
        if self.visible {
            self.window.hide().map_err(|error| {
                tracing::warn!(%error, "could not hide window for QR scan");
                ScreenCaptureFailure::CaptureFailed
            })?;
        }
        Ok(())
    }

    fn restore(&self) {
        if self.visible {
            if let Err(error) = self.window.show() {
                tracing::error!(%error, "could not restore window after QR capture");
            }
            if self.focused {
                if let Err(error) = self.window.set_focus() {
                    tracing::warn!(%error, "could not restore focus after QR capture");
                }
            }
        }
    }
}

pub(super) async fn scan(window: tauri::WebviewWindow) -> Result<QrScanResult, AppError> {
    let scan_window = ScanWindow {
        visible: window
            .is_visible()
            .map_err(|error| AppError::internal(AppErrorSubsystem::Qr, error.to_string()))?,
        focused: window
            .is_focused()
            .map_err(|error| AppError::internal(AppErrorSubsystem::Qr, error.to_string()))?,
        window,
    };
    let batch = match CAPTURE.capture(NativeScreenCapture, scan_window).await {
        Ok(batch) => batch,
        Err(error) => return Ok(QrCodeManager.scan_failure(error)),
    };
    // The window has already been restored, even when decoding is expensive.
    super::support::run_blocking("screen QR decode", move || {
        QrCodeManager.decode_screens(batch)
    })
    .await
}
