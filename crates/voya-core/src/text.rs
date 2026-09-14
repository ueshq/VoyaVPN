//! Text helpers shared by the parsers, the generator and the crates above them.

use base64::{engine::general_purpose::STANDARD, Engine as _};

/// `value` trimmed, or `None` when nothing but whitespace is left.
#[must_use]
pub fn nonempty_str(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// [`nonempty_str`], owned.
#[must_use]
pub fn nonempty_string(value: Option<&str>) -> Option<String> {
    nonempty_str(value).map(str::to_string)
}

/// Decodes standard or URL-safe base64, padded or not, ignoring whitespace.
///
/// `None` when the text is not base64 or does not decode to UTF-8.
pub(crate) fn decode_base64_text(input: &str) -> Option<String> {
    let mut normalized = input
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .replace('_', "/")
        .replace('-', "+");
    if normalized.len() % 4 != 0 {
        normalized.extend(std::iter::repeat_n('=', 4 - normalized.len() % 4));
    }
    let bytes = STANDARD.decode(normalized.as_bytes()).ok()?;
    String::from_utf8(bytes).ok()
}

/// The text a whole-payload base64 body (a subscription, pasted import text)
/// stands for, trimmed.
///
/// `None` unless every non-whitespace character is in the base64 alphabet and
/// the payload decodes to non-empty UTF-8, so plain share links pass through
/// to the caller untouched.
#[must_use]
pub fn decode_base64_payload(input: &str) -> Option<String> {
    let mut characters = input.chars().filter(|ch| !ch.is_whitespace()).peekable();
    if characters.peek().is_none()
        || !characters
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '_' | '-' | '='))
    {
        return None;
    }
    nonempty_string(Some(&decode_base64_text(input)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonempty_helpers_trim_and_drop_blank_values() {
        assert_eq!(nonempty_str(Some("  node  ")), Some("node"));
        assert_eq!(nonempty_str(Some(" \n ")), None);
        assert_eq!(nonempty_str(None), None);
        assert_eq!(nonempty_string(Some(" node ")), Some("node".to_string()));
    }

    #[test]
    fn base64_text_accepts_url_safe_unpadded_and_wrapped_input() {
        // "vmess://a?b" standard: dm1lc3M6Ly9hP2I= ; URL-safe drops padding.
        assert_eq!(
            decode_base64_text("dm1lc3M6Ly9hP2I").as_deref(),
            Some("vmess://a?b")
        );
        assert_eq!(
            decode_base64_text("dm1l\nc3M6\r\nLy9hP2I=").as_deref(),
            Some("vmess://a?b")
        );
        assert_eq!(decode_base64_text("_-8").as_deref(), None);
        assert_eq!(decode_base64_text("").as_deref(), Some(""));
    }

    #[test]
    fn base64_payload_rejects_plain_text_and_blank_results() {
        assert_eq!(
            decode_base64_payload(" dm1lc3M6Ly9hP2I= \n").as_deref(),
            Some("vmess://a?b")
        );
        assert_eq!(decode_base64_payload("vless://node@example.com:443"), None);
        assert_eq!(decode_base64_payload("   "), None);
        // "   " encodes to "ICAg", which decodes to nothing but whitespace.
        assert_eq!(decode_base64_payload("ICAg"), None);
    }
}
