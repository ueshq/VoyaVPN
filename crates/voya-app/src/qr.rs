use qrcode::{render::svg, EcLevel, QrCode};
use serde::Serialize;
use specta::Type;
use thiserror::Error;

const QR_MIN_DIMENSION: u32 = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QrCodeImage {
    pub mime_type: String,
    pub svg: String,
}

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
}
