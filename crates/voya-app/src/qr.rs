use std::collections::BTreeSet;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use qrcode::{render::svg, EcLevel, QrCode};
use thiserror::Error;
use voya_contracts::{QrCodeImage, QrScanFailureReason, QrScanResult, QrScanStatus};
use voya_platform::screen_capture::{ScreenCaptureBatch, ScreenCaptureFailure, ScreenFrame};

mod screen;
pub use screen::ScreenQrCapture;

const QR_MIN_DIMENSION: u32 = 256;

/// Longest side of a picture the renderer may hand over for decoding. It
/// scales a picked image down to this first, so a phone photo crosses IPC as
/// a few megabytes of grey pixels rather than tens.
pub const QR_IMAGE_MAX_SIDE: u32 = 1600;

pub fn generate_svg(content: &str) -> Result<QrCodeImage, QrCodeError> {
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
pub fn decode_screens(batch: ScreenCaptureBatch) -> QrScanResult {
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

/// Decodes every QR code in a picture the user picked, which the renderer
/// sends as base64 luma: one byte per pixel, row by row. The webview
/// already decodes image formats, so none ships in the binary, and the
/// same decoder serves the screen scan and the picked file.
pub fn decode_image(
    width: u32,
    height: u32,
    luma_base64: &str,
) -> Result<QrScanResult, QrCodeError> {
    if width == 0 || height == 0 || width > QR_IMAGE_MAX_SIDE || height > QR_IMAGE_MAX_SIDE {
        return Err(QrCodeError::InvalidImage("the image size is out of range"));
    }
    let (Ok(width), Ok(height)) = (usize::try_from(width), usize::try_from(height)) else {
        return Err(QrCodeError::InvalidImage("the image size is out of range"));
    };
    // Checked before decoding, so a mismatched size allocates nothing.
    if luma_base64.len() != (width * height).div_ceil(3) * 4 {
        return Err(QrCodeError::InvalidImage(
            "the pixel data does not match the image size",
        ));
    }
    let luma = STANDARD
        .decode(luma_base64)
        .map_err(|_| QrCodeError::InvalidImage("the pixel data is not base64"))?;
    let mut texts = Vec::new();
    let frame = ScreenFrame {
        width,
        height,
        luma,
    };
    if !decode_frame(&frame, &mut texts, &mut BTreeSet::new()) {
        return Err(QrCodeError::InvalidImage(
            "the pixel data does not match the image size",
        ));
    }
    Ok(QrScanResult {
        status: if texts.is_empty() {
            QrScanStatus::NotFound
        } else {
            QrScanStatus::Found
        },
        texts,
        source: "image".to_string(),
        message: None,
        failure_reason: None,
    })
}

#[must_use]
pub fn scan_failure(failure: ScreenCaptureFailure) -> QrScanResult {
    QrScanResult {
        status: QrScanStatus::Unavailable,
        texts: Vec::new(),
        source: "screen".to_string(),
        message: None,
        failure_reason: Some(capture_failure_reason(failure)),
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
    #[error("QR image is invalid: {0}")]
    InvalidImage(&'static str),
}

#[cfg(test)]
mod qr_tests {
    use super::*;

    #[test]
    fn qr_generation_returns_backend_svg() {
        let image = generate_svg("vless://00000000-0000-0000-0000-000000000000@example.test:443")
            .expect("qr image");

        assert_eq!(image.mime_type, "image/svg+xml");
        assert!(image.svg.contains("<svg"));
        assert!(image.svg.contains("#111827"));
    }

    #[test]
    fn qr_generation_rejects_empty_content() {
        let error = generate_svg("   ").expect_err("empty content should fail");

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
        let result = decode_screens(ScreenCaptureBatch {
            frames: vec![qr_frame(&[a, b]), qr_frame(&[a])],
            failure: None,
        });
        assert_eq!(result.status, QrScanStatus::Found);
        assert_eq!(result.texts.len(), 2);
        assert!(result.texts.contains(&a.to_string()));
        assert!(result.texts.contains(&b.to_string()));
        assert_eq!(result.failure_reason, None);
    }

    fn decode_picked(frame: &ScreenFrame) -> Result<QrScanResult, QrCodeError> {
        decode_image(
            u32::try_from(frame.width).expect("width fits"),
            u32::try_from(frame.height).expect("height fits"),
            &STANDARD.encode(&frame.luma),
        )
    }

    #[test]
    fn picked_images_decode_every_code_they_hold() {
        let a = "vless://one@example.test:443";
        let b = "trojan://two@example.test:443";

        let found = decode_picked(&qr_frame(&[a, b])).expect("a well-formed image decodes");
        assert_eq!(found.status, QrScanStatus::Found);
        assert_eq!(found.texts.len(), 2);
        assert!(found.texts.contains(&a.to_string()));
        assert!(found.texts.contains(&b.to_string()));
        assert_eq!(found.source, "image");
        assert_eq!(found.failure_reason, None);

        let blank = decode_picked(&qr_frame(&[])).expect("a blank image decodes");
        assert_eq!(blank.status, QrScanStatus::NotFound);
        assert!(blank.texts.is_empty());
    }

    #[test]
    fn picked_images_with_a_bad_size_or_pixels_are_rejected() {
        let three_pixels = STANDARD.encode([0_u8; 3]);
        for (width, height, pixels) in [
            (0, 3, three_pixels.as_str()),
            (QR_IMAGE_MAX_SIDE + 1, 1, three_pixels.as_str()),
            (2, 3, three_pixels.as_str()),
            (1, 3, "!!!!"),
        ] {
            let error =
                decode_image(width, height, pixels).expect_err("the image should be rejected");
            assert!(
                matches!(error, QrCodeError::InvalidImage(_)),
                "{width}x{height}: {error}"
            );
        }
    }

    #[test]
    fn blank_screen_and_partial_capture_have_distinct_results() {
        let blank = decode_screens(ScreenCaptureBatch {
            frames: vec![qr_frame(&[])],
            failure: None,
        });
        assert_eq!(blank.status, QrScanStatus::NotFound);
        assert!(blank.texts.is_empty());
        let partial = decode_screens(ScreenCaptureBatch {
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
        let result = decode_screens(ScreenCaptureBatch {
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
            let result = scan_failure(failure);
            assert_eq!(result.failure_reason, Some(reason));
            assert!(result.texts.is_empty());
        }
    }
}
