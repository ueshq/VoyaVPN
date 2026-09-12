//! Native desktop snapshots. Pixels never cross the IPC boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenCaptureFailure {
    PermissionDenied,
    Unsupported,
    CaptureFailed,
    Timeout,
    Busy,
}

#[derive(Debug)]
pub struct ScreenFrame {
    pub width: usize,
    pub height: usize,
    pub luma: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct ScreenCaptureBatch {
    pub frames: Vec<ScreenFrame>,
    /// Preserve partial capture failures while still decoding other displays.
    pub failure: Option<ScreenCaptureFailure>,
}

pub trait ScreenCaptureAdapter: Send + 'static {
    /// Run before hiding the app so system permission UI remains accessible.
    fn preflight(&self) -> Result<(), ScreenCaptureFailure>;
    fn capture(&self) -> Result<ScreenCaptureBatch, ScreenCaptureFailure>;
}

/// The shell supplies the original visibility/focus state and Tauri dispatch.
pub trait ScreenCaptureWindow: Send {
    fn hide(&self) -> Result<(), ScreenCaptureFailure>;
    fn restore(&self);
}

pub struct NativeScreenCapture;

impl ScreenCaptureAdapter for NativeScreenCapture {
    fn preflight(&self) -> Result<(), ScreenCaptureFailure> {
        check_permission()
    }

    fn capture(&self) -> Result<ScreenCaptureBatch, ScreenCaptureFailure> {
        let monitors = xcap::Monitor::all().map_err(|error| {
            tracing::warn!(%error, "could not enumerate displays for QR scan");
            capture_failure(error)
        })?;
        if monitors.is_empty() {
            return Err(ScreenCaptureFailure::Unsupported);
        }
        let mut batch = ScreenCaptureBatch::default();
        for monitor in monitors {
            match monitor.capture_image() {
                Ok(image) => {
                    let width = image.width() as usize;
                    let height = image.height() as usize;
                    if width == 0 || height == 0 {
                        batch.failure = Some(ScreenCaptureFailure::CaptureFailed);
                        continue;
                    }
                    batch.frames.push(ScreenFrame {
                        width,
                        height,
                        luma: rgba_to_luma(image.as_raw()),
                    });
                }
                Err(error) => {
                    tracing::warn!(%error, "could not capture a display for QR scan");
                    batch.failure = Some(capture_failure(error));
                }
            }
        }
        if batch.frames.is_empty() {
            return Err(batch.failure.unwrap_or(ScreenCaptureFailure::CaptureFailed));
        }
        Ok(batch)
    }
}

/// Rec. 601 greyscale, composited over white so a transparent QR image keeps
/// its contrast. Opaque pixels (every screen capture) are unaffected.
pub(crate) fn rgba_to_luma(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .map(|pixel| {
            let luma = (299 * u32::from(pixel[0])
                + 587 * u32::from(pixel[1])
                + 114 * u32::from(pixel[2])
                + 500)
                / 1000;
            let alpha = u32::from(pixel[3]);
            ((luma * alpha + 255 * (255 - alpha) + 127) / 255) as u8
        })
        .collect()
}

fn capture_failure(error: xcap::XCapError) -> ScreenCaptureFailure {
    match error {
        xcap::XCapError::NotSupported => ScreenCaptureFailure::Unsupported,
        _ => ScreenCaptureFailure::CaptureFailed,
    }
}

#[cfg(target_os = "macos")]
fn check_permission() -> Result<(), ScreenCaptureFailure> {
    #[link(name = "CoreGraphics", kind = "framework")]
    // SAFETY: These system functions have the documented C ABI and no pointer arguments.
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    // SAFETY: Both functions are available from our minimum macOS version, 10.15.
    let allowed = unsafe { CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess() };
    if allowed {
        Ok(())
    } else {
        Err(ScreenCaptureFailure::PermissionDenied)
    }
}

#[cfg(not(target_os = "macos"))]
fn check_permission() -> Result<(), ScreenCaptureFailure> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greyscale_composites_transparency_over_white() {
        let rgba = [
            0, 0, 0, 255, // opaque black
            255, 255, 255, 255, // opaque white
            0, 0, 0, 0, // transparent
            0, 0, 0, 128, // half-transparent black
            255, 0, 0, 255, // opaque red
        ];
        assert_eq!(rgba_to_luma(&rgba), vec![0, 255, 255, 127, 76]);
    }
}
