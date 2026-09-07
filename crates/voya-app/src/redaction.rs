/// Placeholder substituted for a whole URL by [`redact_urls`].
pub const REDACTED_URL: &str = "[redacted URL]";

/// Replace whole `http`/`https` URLs with [`REDACTED_URL`].
///
/// Subscription links routinely carry the account token in the query string, so
/// stripping only the userinfo (as [`redact_url_userinfo`] does) is not enough
/// for text that reaches a toast, the Logs panel or a pasted bug report. The
/// accepted URL characters mirror `URL_PATTERN` in
/// `packages/utils/src/operational-redaction.ts`, so a message redacted here
/// reads the same as one the frontend redacts on the manual paths.
#[must_use]
pub fn redact_urls(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut search_from = 0;
    let mut last_copied = 0;
    let mut redacted = String::with_capacity(value.len());
    let mut changed = false;

    while let Some(scheme_end) = find_scheme_separator(bytes, search_from) {
        let scheme_start = find_scheme_start(bytes, scheme_end);
        let scheme = &value[scheme_start..scheme_end];
        if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
            search_from = scheme_end + 3;
            continue;
        }

        let url_end = find_url_end(bytes, scheme_end + 3);
        redacted.push_str(&value[last_copied..scheme_start]);
        redacted.push_str(REDACTED_URL);
        last_copied = url_end;
        search_from = url_end;
        changed = true;
    }

    if changed {
        redacted.push_str(&value[last_copied..]);
        redacted
    } else {
        value.to_string()
    }
}

/// Redact userinfo from URLs embedded in process output.
///
/// The function intentionally avoids parsing the whole line as a URL because
/// core processes often emit URLs inside larger log messages.
#[must_use]
pub fn redact_url_userinfo(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut search_from = 0;
    let mut last_copied = 0;
    let mut redacted = String::with_capacity(value.len());
    let mut changed = false;

    while let Some(scheme_end) = find_scheme_separator(bytes, search_from) {
        let scheme_start = find_scheme_start(bytes, scheme_end);
        if !is_valid_url_scheme(&value[scheme_start..scheme_end]) {
            search_from = scheme_end + 3;
            continue;
        }

        let authority_start = scheme_end + 3;
        let authority_end = find_url_authority_end(bytes, authority_start);
        // The *last* '@' inside the authority terminates the userinfo: an
        // unescaped '@' is legal in a password and Go's `net/url` (what sing-box
        // echoes) splits the same way. Taking the first one would copy the tail
        // of the password through verbatim.
        let userinfo_end = bytes[authority_start..authority_end]
            .iter()
            .rposition(|byte| *byte == b'@')
            .map(|offset| authority_start + offset);
        let Some(userinfo_end) = userinfo_end else {
            search_from = authority_end;
            continue;
        };
        if userinfo_end == authority_start {
            search_from = authority_end;
            continue;
        }

        redacted.push_str(&value[last_copied..authority_start]);
        redacted.push_str("<redacted>@");
        last_copied = userinfo_end + 1;
        search_from = authority_end;
        changed = true;
    }

    if changed {
        redacted.push_str(&value[last_copied..]);
        redacted
    } else {
        value.to_string()
    }
}

fn find_scheme_separator(bytes: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(3)
        .position(|window| window == b"://")
        .map(|offset| start + offset)
}

fn find_scheme_start(bytes: &[u8], scheme_end: usize) -> usize {
    let mut index = scheme_end;
    while index > 0 && is_url_scheme_byte(bytes[index - 1]) {
        index -= 1;
    }
    index
}

fn is_valid_url_scheme(scheme: &str) -> bool {
    let mut bytes = scheme.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };

    first.is_ascii_alphabetic() && bytes.all(is_url_scheme_byte)
}

fn is_url_scheme_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')
}

/// A URL runs until the first character that cannot appear in one unescaped.
fn find_url_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| is_url_terminator(*byte))
        .map_or(bytes.len(), |offset| start + offset)
}

fn is_url_terminator(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'<' | b'>' | b'"' | b'\'' | b')' | b']')
}

fn find_url_authority_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| is_url_authority_terminator(*byte))
        .map_or(bytes.len(), |offset| start + offset)
}

fn is_url_authority_terminator(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'/' | b'?'
                | b'#'
                | b'"'
                | b'\''
                | b'`'
                | b'<'
                | b'>'
                | b'('
                | b')'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b','
                | b';'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_removes_url_embedded_credentials() {
        let redacted = redact_url_userinfo(
            "dial https://user:pass@example.test:443/path and socks5://alice:secret@127.0.0.1:1080",
        );

        assert!(redacted.contains("https://<redacted>@example.test:443/path"));
        assert!(redacted.contains("socks5://<redacted>@127.0.0.1:1080"));
        assert!(!redacted.contains("user:pass"));
        assert!(!redacted.contains("alice:secret"));
    }

    #[test]
    fn redaction_removes_proxy_uri_userinfo() {
        let redacted = redact_url_userinfo(
            "started outbound vless://00000000-0000-0000-0000-000000000000@edge.example:443?security=tls",
        );

        assert!(redacted.contains("vless://<redacted>@edge.example:443?security=tls"));
        assert!(!redacted.contains("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn url_redaction_removes_subscription_tokens() {
        let redacted = redact_urls(
            "MySub->download failed for https://sub.example.test/link?token=abc123&flag=1: timeout",
        );

        assert_eq!(
            redacted,
            "MySub->download failed for [redacted URL] timeout"
        );
        assert!(!redacted.contains("token=abc123"));
    }

    #[test]
    fn url_redaction_handles_debug_formatted_attempt_lists() {
        let redacted = redact_urls(
            r#"all attempts failed for https://sub.example.test/a?token=s3cret: ["https://sub.example.test/a?token=s3cret: connect error"]"#,
        );

        assert!(!redacted.contains("s3cret"));
        assert_eq!(redacted.matches(REDACTED_URL).count(), 2);
    }

    #[test]
    fn url_redaction_leaves_plain_text_alone() {
        let line = "subscription MySub has no URL configured";

        assert_eq!(redact_urls(line), line);
    }

    /// An unescaped '@' inside the password used to terminate the userinfo
    /// early, leaking everything after it into the log stream.
    #[test]
    fn redaction_removes_credentials_containing_an_at_sign() {
        let redacted = redact_url_userinfo("dial socks5://alice:p@ssw0rd@127.0.0.1:1080 now");

        assert_eq!(redacted, "dial socks5://<redacted>@127.0.0.1:1080 now");
        assert!(!redacted.contains("ssw0rd"));
    }

    #[test]
    fn redaction_preserves_urls_without_userinfo() {
        let line = "connect https://example.test/path@segment for admin@example.test";

        assert_eq!(redact_url_userinfo(line), line);
    }
}
