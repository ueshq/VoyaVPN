use std::collections::BTreeSet;

use qrcode::{render::svg, EcLevel, QrCode};
use thiserror::Error;
pub use voya_contracts::{QrCodeImage, QrScanFailureReason, QrScanResult, QrScanStatus};
use voya_platform::screen_capture::{ScreenCaptureBatch, ScreenCaptureFailure, ScreenFrame};

mod screen;
pub use screen::ScreenQrCapture;

const QR_MIN_DIMENSION: u32 = 256;

#[derive(Debug, Clone, Copy, Default)]
pub struct QrCodeManager;

impl QrCodeManager {
    pub fn generate_svg(&self, content: &str) -> Result<QrCodeImage, QrCodeError> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(QrCodeError::EmptyContent);
        }

        let code = generate_with_fallback(trimmed.as_bytes())?;
        let svg = code
            .render::<svg::Color<'_>>()
            .min_dimensions(QR_MIN_DIMENSION, QR_MIN_DIMENSION)
            .dark_color(svg::Color("#111827"))
            .light_color(svg::Color("#ffffff"))
            .build();

        Ok(QrCodeImage {
            mime_type: "image/svg+xml".to_string(),
            svg,
        })
    }

    #[must_use]
    pub fn decode_screens(&self, batch: ScreenCaptureBatch) -> QrScanResult {
        let mut texts = Vec::new();
        let mut seen = BTreeSet::new();
        let mut failure = batch.failure;
        for frame in batch.frames {
            if !decode_frame(&frame, &mut texts, &mut seen) {
                failure = Some(ScreenCaptureFailure::CaptureFailed);
            }
        }
        QrScanResult {
            status: if texts.is_empty() {
                if failure.is_some() {
                    QrScanStatus::Unavailable
                } else {
                    QrScanStatus::NotFound
                }
            } else {
                QrScanStatus::Found
            },
            texts,
            source: "screen".to_string(),
            message: None,
            failure_reason: failure.map(capture_failure_reason),
        }
    }

    #[must_use]
    pub fn scan_failure(&self, failure: ScreenCaptureFailure) -> QrScanResult {
        QrScanResult {
            status: QrScanStatus::Unavailable,
            texts: Vec::new(),
            source: "screen".to_string(),
            message: None,
            failure_reason: Some(capture_failure_reason(failure)),
        }
    }
}

/// Appends every new payload found in `frame`; `false` when the frame is malformed.
fn decode_frame(frame: &ScreenFrame, texts: &mut Vec<String>, seen: &mut BTreeSet<String>) -> bool {
    if frame.width == 0
        || frame.height == 0
        || frame.width.checked_mul(frame.height) != Some(frame.luma.len())
    {
        return false;
    }
    let mut image =
        rqrr::PreparedImage::prepare_from_greyscale(frame.width, frame.height, |x, y| {
            frame.luma[y * frame.width + x]
        });
    for grid in image.detect_grids() {
        if let Ok((_, text)) = grid.decode() {
            let text = text.trim();
            if !text.is_empty() && seen.insert(text.to_string()) {
                texts.push(text.to_string());
            }
        }
    }
    true
}

fn capture_failure_reason(failure: ScreenCaptureFailure) -> QrScanFailureReason {
    match failure {
        ScreenCaptureFailure::PermissionDenied => QrScanFailureReason::PermissionDenied,
        ScreenCaptureFailure::Unsupported => QrScanFailureReason::Unsupported,
        ScreenCaptureFailure::CaptureFailed => QrScanFailureReason::CaptureFailed,
        ScreenCaptureFailure::Timeout => QrScanFailureReason::Timeout,
        ScreenCaptureFailure::Busy => QrScanFailureReason::Busy,
    }
}

fn generate_with_fallback(content: &[u8]) -> Result<QrCode, QrCodeError> {
    let mut last_error = None;
    for level in [EcLevel::H, EcLevel::Q, EcLevel::M, EcLevel::L] {
        match QrCode::with_error_correction_level(content, level) {
            Ok(code) => return Ok(code),
            Err(error) => last_error = Some(error),
        }
    }

    Err(QrCodeError::Generate(
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "QR code generation failed".to_string()),
    ))
}

#[derive(Debug, Error)]
pub enum QrCodeError {
    #[error("QR content is empty")]
    EmptyContent,
    #[error("{0}")]
    Generate(String),
}

#[cfg(test)]
mod qr_tests {
    use super::*;

    #[test]
    fn qr_generation_returns_backend_svg() {
        let image = QrCodeManager
            .generate_svg("vless://00000000-0000-0000-0000-000000000000@example.test:443")
            .expect("qr image");

        assert_eq!(image.mime_type, "image/svg+xml");
        assert!(image.svg.contains("<svg"));
        assert!(image.svg.contains("#111827"));
    }

    #[test]
    fn qr_generation_rejects_empty_content() {
        let error = QrCodeManager
            .generate_svg("   ")
            .expect_err("empty content should fail");

        assert!(matches!(error, QrCodeError::EmptyContent));
    }

    fn qr_frame(payloads: &[&str]) -> ScreenFrame {
        let codes: Vec<_> = payloads
            .iter()
            .map(|text| QrCode::new(text.as_bytes()).expect("QR"))
            .collect();
        let scale = 5;
        let side = (codes.iter().map(QrCode::width).max().unwrap_or(21) + 8) * scale;
        let width = side * codes.len().max(1);
        let mut frame = ScreenFrame {
            width,
            height: side,
            luma: vec![255; width * side],
        };
        for (index, code) in codes.iter().enumerate() {
            for y in 0..code.width() {
                for x in 0..code.width() {
                    if code[(x, y)] == qrcode::Color::Dark {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                frame.luma[((y + 4) * scale + dy) * width
                                    + index * side
                                    + (x + 4) * scale
                                    + dx] = 0;
                            }
                        }
                    }
                }
            }
        }
        frame
    }

    #[test]
    fn decodes_all_codes_across_displays_and_deduplicates_payloads() {
        let a = "vless://one@example.test:443";
        let b = "trojan://two@example.test:443";
        let result = QrCodeManager.decode_screens(ScreenCaptureBatch {
            frames: vec![qr_frame(&[a, b]), qr_frame(&[a])],
            failure: None,
        });
        assert_eq!(result.status, QrScanStatus::Found);
        assert_eq!(result.texts.len(), 2);
        assert!(result.texts.contains(&a.to_string()));
        assert!(result.texts.contains(&b.to_string()));
        assert_eq!(result.failure_reason, None);
    }

    #[test]
    fn blank_screen_and_partial_capture_have_distinct_results() {
        let blank = QrCodeManager.decode_screens(ScreenCaptureBatch {
            frames: vec![qr_frame(&[])],
            failure: None,
        });
        assert_eq!(blank.status, QrScanStatus::NotFound);
        assert!(blank.texts.is_empty());
        let partial = QrCodeManager.decode_screens(ScreenCaptureBatch {
            frames: vec![qr_frame(&["vless://node@example.test:443"])],
            failure: Some(ScreenCaptureFailure::CaptureFailed),
        });
        assert_eq!(partial.status, QrScanStatus::Found);
        assert_eq!(
            partial.failure_reason,
            Some(QrScanFailureReason::CaptureFailed)
        );
    }

    #[test]
    fn malformed_frame_and_native_failures_are_typed() {
        let result = QrCodeManager.decode_screens(ScreenCaptureBatch {
            frames: vec![ScreenFrame {
                width: 10,
                height: 10,
                luma: vec![0],
            }],
            failure: None,
        });
        assert_eq!(result.status, QrScanStatus::Unavailable);
        for (failure, reason) in [
            (
                ScreenCaptureFailure::PermissionDenied,
                QrScanFailureReason::PermissionDenied,
            ),
            (
                ScreenCaptureFailure::Unsupported,
                QrScanFailureReason::Unsupported,
            ),
            (
                ScreenCaptureFailure::CaptureFailed,
                QrScanFailureReason::CaptureFailed,
            ),
            (ScreenCaptureFailure::Timeout, QrScanFailureReason::Timeout),
            (ScreenCaptureFailure::Busy, QrScanFailureReason::Busy),
        ] {
            let result = QrCodeManager.scan_failure(failure);
            assert_eq!(result.failure_reason, Some(reason));
            assert!(result.texts.is_empty());
        }
    }
}
