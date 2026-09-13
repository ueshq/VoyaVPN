use serde::Deserialize;

use super::{is_cancelled, CancellationFlag, SocksHttpProbe};
use std::time::Duration;

pub const DEFAULT_IP_LOOKUP_URL: &str = "https://ipwho.is/?fields=success,ip,country_code";

/// The custom endpoint's text remains available independently of country parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpLookupResult {
    pub text: String,
    pub ip: Option<String>,
    pub country_code: Option<String>,
}

#[derive(Deserialize)]
struct CountryResponse {
    success: Option<bool>,
    ip: Option<String>,
    country_code: Option<String>,
}

impl SocksHttpProbe {
    /// Uses this profile's SOCKS client, including remote DNS. A failed or
    /// cancelled lookup never discards the latency measured before it.
    pub async fn lookup_country(
        &self,
        url: &str,
        timeout: Duration,
        cancel: &CancellationFlag,
    ) -> Option<IpLookupResult> {
        if is_cancelled(cancel) {
            return None;
        }
        let url = if url.trim().is_empty() {
            DEFAULT_IP_LOOKUP_URL
        } else {
            url.trim()
        };
        let text = tokio::select! {
            text = self.optional_text(url, timeout) => text?,
            () = async {
                while !is_cancelled(cancel) {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            } => return None,
        };
        let country_code = parse_country_code(&text);
        let ip = parse_ip(&text);
        Some(IpLookupResult {
            text,
            ip,
            country_code,
        })
    }
}

/// The address an endpoint reports: a JSON `ip` field or a bare address line.
fn parse_ip(text: &str) -> Option<String> {
    let text = text.trim();
    let candidate = if text.starts_with('{') {
        let response: CountryResponse = serde_json::from_str(text).ok()?;
        if response.success == Some(false) {
            return None;
        }
        response.ip?
    } else {
        text.to_owned()
    };
    candidate
        .trim()
        .parse::<std::net::IpAddr>()
        .ok()
        .map(|address| address.to_string())
}

fn parse_country_code(text: &str) -> Option<String> {
    let text = text.trim();
    let code = if text.starts_with('{') {
        let response: CountryResponse = serde_json::from_str(text).ok()?;
        if response.success == Some(false) {
            return None;
        }
        response.country_code?
    } else {
        text.to_owned()
    };
    let code = code.trim();
    (code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_alphabetic()))
        .then(|| code.to_ascii_uppercase())
        .filter(|code| !matches!(code.as_str(), "XX" | "ZZ"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn country_codes_come_only_from_endpoint_responses() {
        for (text, expected) in [
            (" US\n", Some("US")),
            ("jp", Some("JP")),
            (
                r#"{"success":true,"country_code":"de","ip":"1.2.3.4"}"#,
                Some("DE"),
            ),
            (r#"{"country_code":"HK"}"#, Some("HK")),
            (r#"{"success":false,"country_code":"US"}"#, None),
            (r#"{"country_code":null}"#, None),
            (r#"{"country_code":12}"#, None),
            (r#"{"country":"Japan"}"#, None),
            ("{broken", None),
            ("🇯🇵 Japan", None),
            ("1.2.3.4", None),
            ("USA", None),
            ("U1", None),
            ("é", None),
            ("XX", None),
            ("ZZ", None),
            ("", None),
        ] {
            assert_eq!(parse_country_code(text).as_deref(), expected, "{text}");
        }
    }

    #[test]
    fn addresses_come_from_json_or_a_bare_line() {
        for (text, expected) in [
            (
                r#"{"success":true,"ip":"203.0.113.9","country_code":"JP"}"#,
                Some("203.0.113.9"),
            ),
            (r#"{"ip":"2001:db8::1"}"#, Some("2001:db8::1")),
            (" 198.51.100.7\n", Some("198.51.100.7")),
            (r#"{"success":false,"ip":"203.0.113.9"}"#, None),
            (r#"{"ip":"not an address"}"#, None),
            ("JP", None),
            ("{broken", None),
        ] {
            assert_eq!(parse_ip(text).as_deref(), expected, "{text}");
        }
    }
}
