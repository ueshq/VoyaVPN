//! Parse imported text into a plan; persistence belongs to the caller's transaction.
use super::{is_http_url, Result, SubscriptionManagerError};
use regex::Regex;
use std::collections::BTreeSet;
use voya_core::{
    parse_share_link, parse_ss_sip008, parse_voya_profile_bundle, parse_wireguard_config,
    ProfileItem,
};
use voya_net::decode_base64_payload;

#[derive(Debug, Default)]
pub(super) struct ParsedImportText {
    pub(super) subscription_urls: Vec<String>,
    pub(super) profiles: Vec<ProfileItem>,
    pub(super) failed_lines: usize,
    pub(super) discarded_node_overrides: usize,
    pub(super) messages: Vec<String>,
}

pub(super) fn parse_import_text(text: &str, subscription_id: &str) -> Result<ParsedImportText> {
    let mut profiles = Vec::new();
    let mut subscription_urls = Vec::new();
    let mut failed_lines = 0_usize;
    let mut discarded_node_overrides = 0_usize;
    let mut messages = Vec::new();
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
            if allow_subscription_import && is_http_url(line) {
                subscription_urls.push(line.to_string());
                messages.push(format!(
                    "Line {} added as a subscription source; run subscription update to import its nodes.",
                    line_index + 1
                ));
                continue;
            }
            match parse_share_link(line) {
                Ok(profile) => profiles.push(profile),
                Err(error) if should_report_line_parse_error(line) => {
                    failed_lines = failed_lines.saturating_add(1);
                    messages.push(format!("Line {} was skipped: {error}", line_index + 1));
                }
                Err(_) => {}
            }
        }

        if let Ok(mut bundle) = parse_voya_profile_bundle(&content, subscription_id) {
            profiles.append(&mut bundle);
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
            messages,
            profiles,
        })
    }
}
fn count_discarded_node_overrides(content: &str) -> usize {
    let query_count = Regex::new(
        r"(?i)(?:\?|&)(?:allowinsecure|allow_insecure|insecure|fp|muxenabled|mux_enabled|upmbps|downmbps|hopinterval)=",
    )
    .map(|pattern| pattern.find_iter(content).count())
    .unwrap_or(0);
    let json_count = Regex::new(
        r#"(?i)"(?:fingerprint|fp|allowinsecure|allow_insecure|muxenabled|mux_enabled|upmbps|downmbps|hopinterval)"\s*:"#,
    )
    .map(|pattern| pattern.find_iter(content).count())
    .unwrap_or(0);

    query_count.saturating_add(json_count)
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
        assert_eq!(plan.messages.len(), 3);
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
        assert!(plan.messages[0].starts_with("Line 1 added"));
        assert!(plan.messages[1].starts_with("Line 2 was skipped"));
    }
}
