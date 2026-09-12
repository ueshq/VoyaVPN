//! Capture lifecycle separated from decoding, so the window returns before QR work.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use voya_platform::screen_capture::{
    ScreenCaptureAdapter, ScreenCaptureBatch, ScreenCaptureFailure, ScreenCaptureWindow,
};

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
const HIDE_SETTLE: Duration = Duration::from_millis(150);

#[derive(Default)]
pub struct ScreenQrCapture {
    running: Arc<AtomicBool>,
}

struct CapturePermit(Arc<AtomicBool>);

impl Drop for CapturePermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct RestoreWindow<W: ScreenCaptureWindow>(W);

impl<W: ScreenCaptureWindow> Drop for RestoreWindow<W> {
    fn drop(&mut self) {
        self.0.restore();
    }
}

impl ScreenQrCapture {
    pub async fn capture<A: ScreenCaptureAdapter, W: ScreenCaptureWindow>(
        &self,
        adapter: A,
        window: W,
    ) -> Result<ScreenCaptureBatch, ScreenCaptureFailure> {
        self.capture_with_timeout(adapter, window, CAPTURE_TIMEOUT)
            .await
    }

    async fn capture_with_timeout<A: ScreenCaptureAdapter, W: ScreenCaptureWindow>(
        &self,
        adapter: A,
        window: W,
        timeout: Duration,
    ) -> Result<ScreenCaptureBatch, ScreenCaptureFailure> {
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| ScreenCaptureFailure::Busy)?;
        let permit = CapturePermit(Arc::clone(&self.running));
        // A timed-out blocking task retains its permit until it actually exits.
        // Abandoning the async waiter must never start a second native capture.
        let preflight = tokio::task::spawn_blocking(move || {
            adapter.preflight()?;
            Ok::<_, ScreenCaptureFailure>((adapter, permit))
        });
        let (adapter, permit) = tokio::time::timeout(timeout, preflight)
            .await
            .map_err(|_| ScreenCaptureFailure::Timeout)?
            .map_err(|_| ScreenCaptureFailure::CaptureFailed)??;
        let restore = RestoreWindow(window);
        restore.0.hide()?;
        // Let the compositor remove the app and its menu from the next snapshot.
        tokio::time::sleep(HIDE_SETTLE).await;
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            adapter.capture()
        });
        let result = tokio::time::timeout(timeout, task).await;
        drop(restore);
        result
            .map_err(|_| ScreenCaptureFailure::Timeout)?
            .map_err(|_| ScreenCaptureFailure::CaptureFailed)?
    }
}

#[cfg(test)]
mod tests;
