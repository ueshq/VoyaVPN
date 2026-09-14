//! The tray icon's connected badge, drawn onto the app icon's pixels.
//!
//! The tray menu says "Disconnect" only once it is opened; a green dot on the
//! icon says "connected" at a glance. The drawing is plain RGBA arithmetic, so
//! it lives here with tests instead of in the shell, which has none.

/// The palette's "connected" green, #38a169.
const BADGE: [u8; 4] = [0x38, 0xa1, 0x69, 0xff];
/// A white ring keeps the dot readable on dark and light menu bars alike.
const RING: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

/// A copy of `rgba` with a green dot in the bottom-right corner.
///
/// A buffer that does not hold `width × height` RGBA pixels comes back
/// unchanged: an icon without the dot beats no icon.
#[must_use]
pub fn with_connected_badge(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut pixels = rgba.to_vec();
    let (Ok(width), Ok(height)) = (usize::try_from(width), usize::try_from(height)) else {
        return pixels;
    };
    let expected = width
        .checked_mul(height)
        .and_then(|area| area.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(pixels.len()) {
        return pixels;
    }

    let size = width.min(height) as f64;
    let radius = size * 0.22;
    let outer = radius + (size * 0.05).max(1.0);
    let center_x = width as f64 - outer;
    let center_y = height as f64 - outer;
    for y in 0..height {
        for x in 0..width {
            let dx = x as f64 + 0.5 - center_x;
            let dy = y as f64 + 0.5 - center_y;
            let distance = dx.hypot(dy);
            let color = if distance <= radius {
                BADGE
            } else if distance <= outer {
                RING
            } else {
                continue;
            };
            let start = (y * width + x) * 4;
            pixels[start..start + 4].copy_from_slice(&color);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], size: usize, x: usize, y: usize) -> &[u8] {
        let start = (y * size + x) * 4;
        &pixels[start..start + 4]
    }

    #[test]
    fn the_dot_sits_bottom_right_and_leaves_the_rest_of_the_icon_alone() {
        let size = 32_usize;
        let icon = vec![10_u8; size * size * 4];
        let badged = with_connected_badge(&icon, 32, 32);

        assert_eq!(badged.len(), icon.len());
        assert_eq!(pixel(&badged, size, 0, 0), &[10, 10, 10, 10]);
        assert_eq!(pixel(&badged, size, size - 1, 0), &[10, 10, 10, 10]);
        // The dot's centre is one ring width in from the corner.
        let outer = 32.0 * 0.22 + (32.0_f64 * 0.05).max(1.0);
        let center = (32.0 - outer).floor() as usize;
        assert_eq!(pixel(&badged, size, center, center), &BADGE);
    }

    #[test]
    fn a_buffer_that_is_not_the_stated_size_comes_back_unchanged() {
        assert_eq!(with_connected_badge(&[1, 2, 3], 4, 4), vec![1, 2, 3]);
        assert_eq!(with_connected_badge(&[], 0, 0), Vec::<u8>::new());
    }
}
