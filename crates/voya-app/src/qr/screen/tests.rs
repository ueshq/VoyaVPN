use super::*;
use std::sync::Mutex;

type CaptureResult = Result<ScreenCaptureBatch, ScreenCaptureFailure>;
struct Adapter {
    permission: Result<(), ScreenCaptureFailure>,
    capture: Box<dyn Fn() -> CaptureResult + Send>,
}
impl ScreenCaptureAdapter for Adapter {
    fn preflight(&self) -> Result<(), ScreenCaptureFailure> {
        self.permission
    }
    fn capture(&self) -> CaptureResult {
        (self.capture)()
    }
}
#[derive(Clone, Default)]
struct Window {
    events: Arc<Mutex<Vec<&'static str>>>,
    hide_fails: bool,
}
impl ScreenCaptureWindow for Window {
    fn hide(&self) -> Result<(), ScreenCaptureFailure> {
        self.events.lock().expect("events").push("hide");
        if self.hide_fails {
            Err(ScreenCaptureFailure::CaptureFailed)
        } else {
            Ok(())
        }
    }
    fn restore(&self) {
        self.events.lock().expect("events").push("restore");
    }
}
fn adapter(capture: impl Fn() -> CaptureResult + Send + 'static) -> Adapter {
    Adapter {
        permission: Ok(()),
        capture: Box::new(capture),
    }
}

#[tokio::test]
async fn permission_rejection_keeps_window_visible_and_releases_gate() {
    let scan = ScreenQrCapture::default();
    let window = Window::default();
    let denied = Adapter {
        permission: Err(ScreenCaptureFailure::PermissionDenied),
        capture: Box::new(|| panic!("capture after permission rejection")),
    };
    assert_eq!(
        scan.capture(denied, window.clone()).await.err(),
        Some(ScreenCaptureFailure::PermissionDenied)
    );
    assert!(window.events.lock().expect("events").is_empty());
    assert!(!scan.running.load(Ordering::Acquire));
}

#[tokio::test]
async fn restores_after_capture_success_failure_and_panic() {
    for outcome in 0..3 {
        let scan = ScreenQrCapture::default();
        let window = Window::default();
        let events = Arc::clone(&window.events);
        let result = scan
            .capture(
                adapter(move || {
                    assert_eq!(*events.lock().expect("events"), ["hide"]);
                    events.lock().expect("events").push("capture");
                    match outcome {
                        0 => Ok(ScreenCaptureBatch::default()),
                        1 => Err(ScreenCaptureFailure::CaptureFailed),
                        _ => panic!("capture worker failed"),
                    }
                }),
                window.clone(),
            )
            .await;
        assert_eq!(result.is_ok(), outcome == 0);
        assert_eq!(
            *window.events.lock().expect("events"),
            ["hide", "capture", "restore"]
        );
        assert!(!scan.running.load(Ordering::Acquire));
    }
}

#[tokio::test]
async fn hide_failure_restores_without_capturing() {
    let scan = ScreenQrCapture::default();
    let window = Window {
        hide_fails: true,
        ..Window::default()
    };
    assert!(scan
        .capture(
            adapter(|| panic!("capture after hide failure")),
            window.clone()
        )
        .await
        .is_err());
    assert_eq!(*window.events.lock().expect("events"), ["hide", "restore"]);
    assert!(!scan.running.load(Ordering::Acquire));
}

#[tokio::test]
async fn timeout_restores_but_retains_gate_until_worker_finishes() {
    let scan = ScreenQrCapture::default();
    let window = Window::default();
    let (release, wait) = std::sync::mpsc::channel();
    let result = scan
        .capture_with_timeout(
            adapter(move || {
                wait.recv_timeout(Duration::from_secs(5))
                    .expect("release capture");
                Ok(ScreenCaptureBatch::default())
            }),
            window.clone(),
            Duration::from_millis(200),
        )
        .await;
    assert_eq!(result.err(), Some(ScreenCaptureFailure::Timeout));
    assert_eq!(*window.events.lock().expect("events"), ["hide", "restore"]);
    assert_eq!(
        scan.capture(adapter(|| panic!("overlapping capture")), Window::default())
            .await
            .err(),
        Some(ScreenCaptureFailure::Busy)
    );
    release.send(()).expect("release");
    tokio::time::timeout(Duration::from_secs(2), async {
        while scan.running.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker exits");
    assert!(scan
        .capture(
            adapter(|| Ok(ScreenCaptureBatch::default())),
            Window::default()
        )
        .await
        .is_ok());
}

#[tokio::test]
async fn cancelling_waiter_restores_without_releasing_live_worker() {
    let scan = Arc::new(ScreenQrCapture::default());
    let window = Window::default();
    let (release, wait) = std::sync::mpsc::channel();
    let (started, wait_started) = tokio::sync::oneshot::channel();
    let started = Mutex::new(Some(started));
    let task_scan = Arc::clone(&scan);
    let task_window = window.clone();
    let task = tokio::spawn(async move {
        task_scan
            .capture(
                adapter(move || {
                    started
                        .lock()
                        .expect("started")
                        .take()
                        .expect("sender")
                        .send(())
                        .expect("signal");
                    wait.recv_timeout(Duration::from_secs(5)).expect("release");
                    Ok(ScreenCaptureBatch::default())
                }),
                task_window,
            )
            .await
    });
    wait_started.await.expect("capture begins");
    task.abort();
    assert!(task.await.expect_err("cancelled").is_cancelled());
    assert_eq!(*window.events.lock().expect("events"), ["hide", "restore"]);
    assert!(scan.running.load(Ordering::Acquire));
    release.send(()).expect("release");
}
