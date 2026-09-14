//! Host-name helpers shared by config generation and the platform layer.

/// One DNS label: 1 to 63 ASCII letters, digits or hyphens that neither starts
/// nor ends with a hyphen.
#[must_use]
pub fn is_dns_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// sing-box parses `dns.servers[].server` with `netip.ParseAddr`, which rejects
/// the bracketed IPv6 authority form that URL parsing hands back.
pub(crate) fn strip_host_brackets(host: &str) -> String {
    host.trim_matches(['[', ']']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_labels_follow_the_hostname_rules() {
        for label in ["a", "node-1", "EXAMPLE", "0", &"a".repeat(63)] {
            assert!(is_dns_label(label), "{label:?} should be a label");
        }
        for label in [
            "",
            "-node",
            "node-",
            "no_de",
            "no.de",
            "nöde",
            &"a".repeat(64),
        ] {
            assert!(!is_dns_label(label), "{label:?} should not be a label");
        }
    }
}
