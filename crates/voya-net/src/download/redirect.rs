//! Redirect policy and the loopback/link-local host deny-list shared by the download stack.
//!
//! Both the subscription URL policy and the redirect policy route through
//! [`is_denied_local_host`], which is the only thing standing between a hostile subscription
//! provider and the loopback services this app runs (the sing-box mixed inbound and the Clash
//! API), so every address form that a kernel delivers locally is denied here.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use thiserror::Error;

pub(crate) const HTTP_REDIRECT_LIMIT: usize = 5;

pub(crate) fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        match validate_redirect_attempt(attempt.previous(), attempt.url()) {
            Ok(()) => attempt.follow(),
            Err(error) => attempt.error(error),
        }
    })
}

fn validate_redirect_attempt(
    previous: &[reqwest::Url],
    next: &reqwest::Url,
) -> Result<(), RedirectPolicyError> {
    if previous.len() > HTTP_REDIRECT_LIMIT {
        return Err(RedirectPolicyError::TooManyRedirects {
            limit: HTTP_REDIRECT_LIMIT,
        });
    }

    if let Some(previous) = previous
        .last()
        .filter(|previous| previous.scheme() == "https" && next.scheme() == "http")
    {
        return Err(RedirectPolicyError::HttpsDowngrade {
            from: previous.as_str().to_string(),
            to: next.as_str().to_string(),
        });
    }
    if url_has_denied_local_host(next) && !previous.last().is_some_and(url_has_denied_local_host) {
        return Err(RedirectPolicyError::LocalNetworkTarget {
            to: next.as_str().to_string(),
        });
    }

    Ok(())
}

pub(crate) fn is_denied_local_host(host: &str) -> bool {
    let normalized = host
        .trim_end_matches('.')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if normalized == "localhost" || normalized.ends_with(".localhost") {
        return true;
    }

    normalized.parse::<IpAddr>().is_ok_and(is_denied_local_ip)
}

fn is_denied_local_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_denied_local_ipv4(ip),
        IpAddr::V6(ip) => is_denied_local_ipv6(ip),
    }
}

fn is_denied_local_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    // 0.0.0.0 is delivered to the local host by connect() on Linux and macOS.
    ip.is_unspecified() || ip.is_loopback() || (octets[0] == 169 && octets[1] == 254)
}

fn is_denied_local_ipv6(ip: Ipv6Addr) -> bool {
    if ip.is_unspecified() || ip.is_loopback() {
        return true;
    }
    // IPv4-mapped (::ffff:127.0.0.1) and IPv4-compatible (::127.0.0.1) forms are delivered to
    // the embedded IPv4 address by dual-stack sockets, so they inherit the IPv4 policy.
    if let Some(embedded) = ip.to_ipv4_mapped().or_else(|| ipv4_compatible(ip)) {
        return is_denied_local_ipv4(embedded);
    }

    (ip.segments()[0] & 0xffc0) == 0xfe80
}

/// Extracts the IPv4 address embedded in a deprecated IPv4-compatible `::a.b.c.d` address.
fn ipv4_compatible(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ip.segments();
    if segments[..6].iter().any(|segment| *segment != 0) {
        return None;
    }

    Some(Ipv4Addr::from(
        (u32::from(segments[6]) << 16) | u32::from(segments[7]),
    ))
}

fn url_has_denied_local_host(url: &reqwest::Url) -> bool {
    url.host_str().is_some_and(is_denied_local_host)
}

#[derive(Debug, Error)]
enum RedirectPolicyError {
    #[error("too many redirects: maximum {limit}")]
    TooManyRedirects { limit: usize },
    #[error("refusing HTTPS to HTTP redirect from {from} to {to}")]
    HttpsDowngrade { from: String, to: String },
    #[error("refusing redirect to loopback or link-local URL {to}")]
    LocalNetworkTarget { to: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_policy_rejects_https_to_http_downgrade() {
        let previous = [reqwest::Url::parse("https://example.test/sub").expect("previous URL")];
        let next = reqwest::Url::parse("http://example.test/sub").expect("next URL");

        let error = validate_redirect_attempt(&previous, &next)
            .expect_err("HTTPS to HTTP redirect should fail");

        assert!(
            matches!(error, RedirectPolicyError::HttpsDowngrade { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn redirect_policy_rejects_public_to_local_target() {
        let previous = [reqwest::Url::parse("https://example.test/sub").expect("previous URL")];
        let next = reqwest::Url::parse("https://127.0.0.1/sub").expect("next URL");

        let error = validate_redirect_attempt(&previous, &next)
            .expect_err("public to loopback redirect should fail");

        assert!(
            matches!(error, RedirectPolicyError::LocalNetworkTarget { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn local_host_guard_rejects_unspecified_and_ipv4_mapped_forms() {
        for host in [
            "localhost",
            "127.0.0.1",
            "127.9.9.9",
            "0.0.0.0",
            "169.254.1.10",
            "[::1]",
            "[::]",
            "[::ffff:127.0.0.1]",
            "[::ffff:169.254.1.1]",
            "[::127.0.0.1]",
            "[fe80::1]",
        ] {
            assert!(is_denied_local_host(host), "{host} should be denied");
        }

        for host in [
            "example.test",
            "1.1.1.1",
            "192.168.1.10",
            "[::ffff:1.1.1.1]",
            "[2606:4700::1111]",
        ] {
            assert!(!is_denied_local_host(host), "{host} should be allowed");
        }
    }

    #[test]
    fn redirect_policy_rejects_ipv4_mapped_loopback_target() {
        let previous = [reqwest::Url::parse("https://example.test/sub").expect("previous URL")];
        let next = reqwest::Url::parse("https://[::ffff:127.0.0.1]/sub").expect("next URL");

        let error = validate_redirect_attempt(&previous, &next)
            .expect_err("IPv4-mapped loopback redirect should fail");

        assert!(
            matches!(error, RedirectPolicyError::LocalNetworkTarget { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn redirect_policy_rejects_more_than_configured_limit() {
        let previous = (0..=HTTP_REDIRECT_LIMIT)
            .map(|index| {
                reqwest::Url::parse(&format!("https://example.test/r{index}"))
                    .expect("previous URL")
            })
            .collect::<Vec<_>>();
        let next = reqwest::Url::parse("https://example.test/final").expect("next URL");

        let error = validate_redirect_attempt(&previous, &next)
            .expect_err("redirect chain above limit should fail");

        assert!(
            matches!(error, RedirectPolicyError::TooManyRedirects { limit } if limit == HTTP_REDIRECT_LIMIT),
            "{error:?}"
        );
    }
}
