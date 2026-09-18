//! Parse imported text into a plan; persistence belongs to the caller's transaction.
use super::{is_http_url, Result, SubscriptionManagerError};
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};
use voya_core::{
    parse_share_link, parse_ss_sip008, parse_wireguard_config, text::decode_base64_payload,
    ImportLineCode, ImportLineIssue, ProfileItem, ShareError,
};

#[derive(Debug, Default)]
pub(super) struct ParsedImportText {
    pub(super) subscription_urls: Vec<String>,
    pub(super) profiles: Vec<ProfileItem>,
    pub(super) failed_lines: usize,
    pub(super) discarded_node_overrides: usize,
    pub(super) line_issues: Vec<ImportLineIssue>,
}

pub(super) fn parse_import_text(text: &str, subscription_id: &str) -> Result<ParsedImportText> {
    let mut profiles = Vec::new();
    let mut subscription_urls = Vec::new();
    let mut failed_lines = 0_usize;
    let mut discarded_node_overrides = 0_usize;
    let mut line_issues = Vec::new();
    let allow_subscription_import = subscription_id.trim().is_empty();
    let mut contents = Vec::new();
    if let Some(decoded) = decode_base64_payload(text) {
        contents.push(decoded);
    }
    contents.push(text.to_string());

    for content in contents {
        discarded_node_overrides =
            discarded_node_overrides.saturating_add(count_discarded_node_overrides(&content));
        let mut lines_seen = BTreeSet::new();
        for (line_index, line) in content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .enumerate()
        {
            if !subscription_id.is_empty() && !lines_seen.insert(line.to_string()) {
                continue;
            }
            let line_number = u32::try_from(line_index.saturating_add(1)).unwrap_or(u32::MAX);
            if allow_subscription_import && is_http_url(line) {
                subscription_urls.push(line.to_string());
                line_issues.push(ImportLineIssue {
                    line: line_number,
                    code: ImportLineCode::SubscriptionSourceAdded,
                });
                continue;
            }
            match parse_share_link(line) {
                Ok(profile) => profiles.push(profile),
                Err(error) if should_report_line_parse_error(line) => {
                    failed_lines = failed_lines.saturating_add(1);
                    line_issues.push(ImportLineIssue {
                        line: line_number,
                        code: import_line_code(error),
                    });
                }
                Err(_) => {}
            }
        }

        if let Ok(mut ss) = parse_ss_sip008(&content) {
            profiles.append(&mut ss);
        }
        if let Ok(mut wireguard) = parse_wireguard_config(&content) {
            profiles.append(&mut wireguard);
        }
    }

    if profiles.is_empty() && subscription_urls.is_empty() && failed_lines == 0 {
        Err(SubscriptionManagerError::NoImportableProfiles)
    } else {
        Ok(ParsedImportText {
            subscription_urls,
            failed_lines,
            discarded_node_overrides,
            line_issues,
            profiles,
        })
    }
}

fn import_line_code(error: ShareError) -> ImportLineCode {
    match error {
        ShareError::UnsupportedTransport { transport } => {
            ImportLineCode::UnsupportedTransport { transport }
        }
        ShareError::UnsupportedProtocol => ImportLineCode::UnsupportedProtocol,
        ShareError::MissingField { protocol, field } => ImportLineCode::MissingField {
            protocol: protocol.to_string(),
            field: field.to_string(),
        },
        ShareError::InvalidPort { protocol, port } => ImportLineCode::InvalidPort {
            protocol: protocol.to_string(),
            port,
        },
        other => ImportLineCode::ParseFailed {
            detail: other.to_string(),
        },
    }
}
// Compiled once per process: an import runs both over every decoded payload.
static QUERY_OVERRIDE_REGEX: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\?|&)(?:allowinsecure|allow_insecure|insecure|fp|muxenabled|mux_enabled|upmbps|downmbps|hopinterval)=",
    )
    .ok()
});
static JSON_OVERRIDE_REGEX: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)"(?:fingerprint|fp|allowinsecure|allow_insecure|muxenabled|mux_enabled|upmbps|downmbps|hopinterval)"\s*:"#,
    )
    .ok()
});

fn count_discarded_node_overrides(content: &str) -> usize {
    let count = |pattern: &Option<Regex>| {
        pattern
            .as_ref()
            .map_or(0, |pattern| pattern.find_iter(content).count())
    };
    count(&QUERY_OVERRIDE_REGEX).saturating_add(count(&JSON_OVERRIDE_REGEX))
}

// Share-link schemes recognized by `parse_share_link`. A parse failure on a
// line starting with one of these is worth surfacing to the user; any other
// line is treated as noise and skipped silently. Kept in sync with the scheme
// dispatch in `voya_core::parse_share_link`.
const REPORTABLE_SHARE_LINK_SCHEMES: [&str; 15] = [
    "vmess://",
    "ss://",
    "socks://",
    "socks4://",
    "socks5://",
    "trojan://",
    "vless://",
    "hysteria2://",
    "hy2://",
    "tuic://",
    "wireguard://",
    "anytls://",
    "naive://",
    "naive+https://",
    "naive+quic://",
];

fn should_report_line_parse_error(line: &str) -> bool {
    let line = line.trim();
    REPORTABLE_SHARE_LINK_SCHEMES
        .iter()
        .any(|prefix| line_has_prefix_ci(line, prefix))
}

fn line_has_prefix_ci(line: &str, prefix: &str) -> bool {
    line.get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_sources_become_ordered_actions_without_a_database() {
        let plan = parse_import_text(
            "https://one.test/sub\nhttps://two.test/sub\nhttps://one.test/sub",
            "",
        )
        .expect("subscription source plan");
        assert_eq!(
            plan.subscription_urls,
            [
                "https://one.test/sub",
                "https://two.test/sub",
                "https://one.test/sub"
            ]
        );
        assert!(plan.profiles.is_empty());
        assert_eq!(
            plan.line_issues
                .iter()
                .map(|issue| (issue.line, issue.code.clone()))
                .collect::<Vec<_>>(),
            [1, 2, 3].map(|line| (line, ImportLineCode::SubscriptionSourceAdded))
        );
    }

    #[test]
    fn subscription_payloads_cannot_add_another_source() {
        assert!(matches!(
            parse_import_text("https://one.test/sub", "existing-source"),
            Err(SubscriptionManagerError::NoImportableProfiles)
        ));
    }

    #[test]
    fn malformed_nodes_preserve_line_diagnostics_alongside_source_actions() {
        let plan = parse_import_text("https://one.test/sub\nvmess://invalid", "")
            .expect("partial import plan");
        assert_eq!(plan.subscription_urls, ["https://one.test/sub"]);
        assert_eq!(plan.failed_lines, 1);
        assert_eq!(
            plan.line_issues[0],
            ImportLineIssue {
                line: 1,
                code: ImportLineCode::SubscriptionSourceAdded,
            }
        );
        assert_eq!(plan.line_issues[1].line, 2);
        assert!(matches!(
            plan.line_issues[1].code,
            ImportLineCode::ParseFailed { .. }
        ));
    }

    #[test]
    fn retired_transport_lines_are_counted_and_reported_as_unsupported() {
        let plan = parse_import_text(
            "vless://00000000-0000-0000-0000-000000000001@example.com:443?encryption=none&type=xhttp#retired\ntrojan://secret@example.com:443?security=tls&type=ws#kept",
            "",
        )
        .expect("partial import plan");
        assert_eq!(plan.profiles.len(), 1);
        assert_eq!(plan.failed_lines, 1);
        assert_eq!(
            plan.line_issues,
            [ImportLineIssue {
                line: 1,
                code: ImportLineCode::UnsupportedTransport {
                    transport: "xhttp".to_string(),
                },
            }]
        );
    }
}
