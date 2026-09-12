//! Native clipboard reads for the import menu.
//!
//! WebKit shows a "Paste" confirmation for every `navigator.clipboard.read*`
//! call that is not a user paste gesture, so the renderer reads through the
//! shell instead. As with screen capture, pixels never cross the IPC boundary.

use thiserror::Error;

use crate::screen_capture::{rgba_to_luma, ScreenFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ClipboardFailure {
    #[error("the clipboard is not supported in this session")]
    Unsupported,
    #[error("the clipboard is held by another application")]
    Busy,
    #[error("the clipboard could not be read")]
    ReadFailed,
}

/// Returns `Ok(None)` when the clipboard holds no text.
pub fn read_text() -> Result<Option<String>, ClipboardFailure> {
    present(open()?.get_text(), "text")
}

/// Returns a greyscale copy of the clipboard image, or `Ok(None)` when the
/// clipboard holds no image.
pub fn read_image() -> Result<Option<ScreenFrame>, ClipboardFailure> {
    let Some(image) = present(open()?.get_image(), "image")? else {
        return Ok(None);
    };
    let expected_len = image
        .width
        .checked_mul(image.height)
        .and_then(|pixels| pixels.checked_mul(4));
    if image.width == 0 || image.height == 0 || expected_len != Some(image.bytes.len()) {
        tracing::warn!(
            width = image.width,
            height = image.height,
            "clipboard image has an unexpected layout"
        );
        return Err(ClipboardFailure::ReadFailed);
    }
    Ok(Some(ScreenFrame {
        width: image.width,
        height: image.height,
        luma: rgba_to_luma(&image.bytes),
    }))
}

fn open() -> Result<arboard::Clipboard, ClipboardFailure> {
    arboard::Clipboard::new().map_err(|error| {
        tracing::warn!(%error, "could not open the clipboard");
        failure(&error)
    })
}

fn present<T>(
    read: Result<T, arboard::Error>,
    content: &'static str,
) -> Result<Option<T>, ClipboardFailure> {
    match read {
        Ok(value) => Ok(Some(value)),
        // Empty, or holding a different kind of content.
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(error) => {
            tracing::warn!(%error, content, "could not read the clipboard");
            Err(failure(&error))
        }
    }
}

fn failure(error: &arboard::Error) -> ClipboardFailure {
    match error {
        arboard::Error::ClipboardNotSupported => ClipboardFailure::Unsupported,
        arboard::Error::ClipboardOccupied => ClipboardFailure::Busy,
        _ => ClipboardFailure::ReadFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_content_is_not_a_failure() {
        assert_eq!(
            present::<String>(Err(arboard::Error::ContentNotAvailable), "text"),
            Ok(None)
        );
        assert_eq!(
            present(Ok("vless://node".to_string()), "text"),
            Ok(Some("vless://node".to_string()))
        );
    }

    #[test]
    fn native_errors_map_to_typed_failures() {
        for (error, expected) in [
            (
                arboard::Error::ClipboardNotSupported,
                ClipboardFailure::Unsupported,
            ),
            (arboard::Error::ClipboardOccupied, ClipboardFailure::Busy),
            (
                arboard::Error::ConversionFailure,
                ClipboardFailure::ReadFailed,
            ),
            (
                arboard::Error::Unknown {
                    description: "selection owner timed out".to_string(),
                },
                ClipboardFailure::ReadFailed,
            ),
        ] {
            assert_eq!(present::<String>(Err(error), "text"), Err(expected));
        }
    }
}
