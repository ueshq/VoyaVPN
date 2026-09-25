//! The routing profile a fresh install starts with, and the reserved remarks
//! that mark the rules the Rules page manages on the user's behalf.
//!
//! Plain data only: `voya-app` seeds [`default_routing_item`] when the database
//! has no routing profile, and the renderer finds managed rules by remarks.
//! Seeded rules stay ordinary, editable rules; the remarks are their identity.

use crate::{RoutingItem, RuleType, RulesItem, BLOCK_TAG, DIRECT_TAG, PROXY_TAG};

/// The per-app proxy rule the per-app dialog writes.
pub const SENTINEL_PER_APP_PROXY: &str = "voya:per-app-proxy";
/// AI services that geo-block or hard-fail on a direct connection.
pub const SENTINEL_AI_SERVICES: &str = "voya:ai-services";
/// QUIC to port 443, blocked so clients fall back to TCP through the proxy.
pub const SENTINEL_BLOCK_QUIC: &str = "voya:block-quic";
/// Chinese public DNS resolvers, reached directly.
pub const SENTINEL_CN_DNS: &str = "voya:cn-dns";
/// Private and link-local destinations, reached directly.
pub const SENTINEL_BYPASS_LAN: &str = "voya:bypass-lan";
/// Advertising domains, blocked. Seeded disabled, so the Rules page always
/// lists it and the user opts in with its switch.
pub const SENTINEL_BLOCK_ADS: &str = "voya:block-ads";
/// Mainland China domains and addresses, reached directly.
pub const SENTINEL_CN_DIRECT: &str = "voya:cn-direct";

/// Every reserved remarks value, in no particular order.
pub const SENTINEL_REMARKS: [&str; 7] = [
    SENTINEL_PER_APP_PROXY,
    SENTINEL_AI_SERVICES,
    SENTINEL_BLOCK_QUIC,
    SENTINEL_CN_DNS,
    SENTINEL_BYPASS_LAN,
    SENTINEL_BLOCK_ADS,
    SENTINEL_CN_DIRECT,
];

/// Vendors whose endpoints the seeded profile sends through the proxy.
///
/// This used to be injected into every generated config ahead of the user's own
/// rules, with no way to opt out (ADR 0006). It is now the first rule of the
/// default routing profile, so it can be edited, disabled or deleted like any
/// other rule.
pub const AI_SERVICE_DOMAIN_SUFFIXES: &[&str] = &[
    "anthropic.com",
    "claude.ai",
    "claude.com",
    "claudeusercontent.com",
    "openai.com",
    "chatgpt.com",
    "oaistatic.com",
    "oaiusercontent.com",
    "openai.azure.com",
    "githubcopilot.com",
    "copilot-proxy.githubusercontent.com",
    "copilot-telemetry.githubusercontent.com",
    "cursor.com",
    "cursor.sh",
    "codeium.com",
    "windsurf.com",
    "sourcegraph.com",
    "perplexity.ai",
    "generativelanguage.googleapis.com",
    "aistudio.google.com",
    "gemini.google.com",
    "ai.google.dev",
    "poe.com",
    "x.ai",
    "grok.com",
    "cohere.ai",
    "mistral.ai",
    "huggingface.co",
];

const CN_PUBLIC_DNS_ADDRESSES: &[&str] = &[
    "223.5.5.5",
    "223.6.6.6",
    "119.29.29.29",
    "180.76.76.76",
    "2400:3200::1",
    "2400:3200:baba::1",
];

const CN_PUBLIC_DNS_DOMAINS: &[&str] = &[
    "domain:alidns.com",
    "domain:doh.pub",
    "full:dns.alidns.com",
    "full:dot.pub",
];

/// Whether `remarks` marks a rule the Rules page manages.
#[must_use]
pub fn is_sentinel_remarks(remarks: Option<&str>) -> bool {
    remarks.is_some_and(|remarks| SENTINEL_REMARKS.contains(&remarks))
}

/// Unions a managed rule's `domain`/`ip` matchers with the current seed.
///
/// Content is maintained here; the user keeps the switch, position, outbound
/// and deletion. Missing seed matchers are appended (user-added entries and
/// order stay), and a deleted managed rule is never recreated. Returns whether
/// the rule changed. Non-sentinel remarks and the per-app proxy (which has no
/// seed content) are left alone.
#[must_use]
pub fn refresh_managed_rule(rule: &mut RulesItem) -> bool {
    let Some(remarks) = rule.remarks.as_deref() else {
        return false;
    };
    if !is_sentinel_remarks(rule.remarks.as_deref()) {
        return false;
    }
    let Some(seed) = default_rule_set()
        .into_iter()
        .find(|candidate| candidate.remarks.as_deref() == Some(remarks))
    else {
        return false;
    };

    let mut changed = false;
    if let Some(seed_domains) = seed.domain.as_ref().filter(|items| !items.is_empty()) {
        let domains = rule.domain.get_or_insert_with(Vec::new);
        for domain in seed_domains {
            if !domains.contains(domain) {
                domains.push(domain.clone());
                changed = true;
            }
        }
    }
    if let Some(seed_ips) = seed.ip.as_ref().filter(|items| !items.is_empty()) {
        let ips = rule.ip.get_or_insert_with(Vec::new);
        for ip in seed_ips {
            if !ips.contains(ip) {
                ips.push(ip.clone());
                changed = true;
            }
        }
    }
    changed
}

/// The seeded rule set, in evaluation order. Anything no rule matches falls
/// through to the proxy.
///
/// QUIC is blocked last, so only UDP 443 that would have gone through the
/// node is refused (clients fall back to TCP through the proxy); China, LAN
/// and China-DNS destinations keep their direct QUIC. The AI-service rule
/// stays first: some of those services sit behind CDN addresses that
/// `geoip:cn` also lists. Its QUIC therefore goes to the node rather than
/// being blocked.
#[must_use]
pub fn default_rule_set() -> Vec<RulesItem> {
    vec![
        sentinel_rule(
            SENTINEL_AI_SERVICES,
            PROXY_TAG,
            RuleType::ALL,
            RulesItem {
                domain: Some(
                    AI_SERVICE_DOMAIN_SUFFIXES
                        .iter()
                        .map(|suffix| format!("domain:{suffix}"))
                        .collect(),
                ),
                ..RulesItem::default()
            },
        ),
        // Off by default: blocking ads can break sites, so the user opts in.
        // Disabled rules never reach the generated config.
        RulesItem {
            enabled: false,
            ..sentinel_rule(
                SENTINEL_BLOCK_ADS,
                BLOCK_TAG,
                RuleType::ALL,
                RulesItem {
                    domain: Some(vec!["geosite:category-ads-all".to_string()]),
                    ..RulesItem::default()
                },
            )
        },
        sentinel_rule(
            SENTINEL_CN_DNS,
            DIRECT_TAG,
            RuleType::ALL,
            RulesItem {
                ip: Some(owned(CN_PUBLIC_DNS_ADDRESSES)),
                domain: Some(owned(CN_PUBLIC_DNS_DOMAINS)),
                ..RulesItem::default()
            },
        ),
        bypass_lan_rule(),
        sentinel_rule(
            SENTINEL_CN_DIRECT,
            DIRECT_TAG,
            RuleType::ALL,
            RulesItem {
                domain: Some(vec!["geosite:cn".to_string()]),
                ip: Some(vec!["geoip:cn".to_string()]),
                ..RulesItem::default()
            },
        ),
        sentinel_rule(
            SENTINEL_BLOCK_QUIC,
            BLOCK_TAG,
            RuleType::Routing,
            RulesItem {
                network: Some("udp".to_string()),
                port: Some("443".to_string()),
                ..RulesItem::default()
            },
        ),
    ]
}

/// Private destinations, reached directly.
#[must_use]
pub(crate) fn bypass_lan_rule() -> RulesItem {
    sentinel_rule(
        SENTINEL_BYPASS_LAN,
        DIRECT_TAG,
        RuleType::ALL,
        RulesItem {
            ip: Some(vec!["geoip:private".to_string()]),
            domain: Some(vec!["geosite:private".to_string()]),
            ..RulesItem::default()
        },
    )
}

/// The routing profile a fresh install is seeded with.
#[must_use]
pub fn default_routing_item(remarks: &str) -> RoutingItem {
    RoutingItem {
        remarks: remarks.to_string(),
        rule_set: default_rule_set(),
        enabled: true,
        ..RoutingItem::default()
    }
}

/// The seeded profile's name in the interface language it is created under.
#[must_use]
pub fn seed_routing_remarks(language: &str) -> &'static str {
    if language.starts_with("zh-Hant") {
        "智慧分流"
    } else if language.starts_with("zh") {
        "智能分流"
    } else {
        "Smart routing"
    }
}

fn sentinel_rule(remarks: &str, outbound: &str, scope: RuleType, matchers: RulesItem) -> RulesItem {
    RulesItem {
        remarks: Some(remarks.to_string()),
        outbound_tag: Some(outbound.to_string()),
        rule_type: Some(scope),
        enabled: true,
        ..matchers
    }
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn sentinel_remarks_are_unique_and_recognized() {
        assert_eq!(
            SENTINEL_REMARKS.iter().collect::<BTreeSet<_>>().len(),
            SENTINEL_REMARKS.len()
        );
        assert!(SENTINEL_REMARKS
            .iter()
            .all(|remarks| is_sentinel_remarks(Some(remarks))));
        assert!(!is_sentinel_remarks(Some("voya:unknown")));
        assert!(!is_sentinel_remarks(None));
    }

    #[test]
    fn the_seed_keeps_its_evaluation_order_and_every_rule_is_managed() {
        let rules = default_rule_set();
        assert_eq!(
            rules
                .iter()
                .map(|rule| rule.remarks.as_deref())
                .collect::<Vec<_>>(),
            [
                Some(SENTINEL_AI_SERVICES),
                Some(SENTINEL_BLOCK_ADS),
                Some(SENTINEL_CN_DNS),
                Some(SENTINEL_BYPASS_LAN),
                Some(SENTINEL_CN_DIRECT),
                Some(SENTINEL_BLOCK_QUIC),
            ]
        );
        // Blocking ads is opt-in: the rule is listed so its switch has a row,
        // but it stays out of the generated config until the user enables it.
        assert!(rules
            .iter()
            .all(|rule| rule.enabled == (rule.remarks.as_deref() != Some(SENTINEL_BLOCK_ADS))));
        assert!(rules
            .iter()
            .all(|rule| is_sentinel_remarks(rule.remarks.as_deref())));
    }

    #[test]
    fn the_ai_service_rule_carries_every_suffix_as_a_domain_suffix_matcher() {
        let rules = default_rule_set();
        let domains = rules[0].domain.as_ref().expect("AI service domains");
        assert_eq!(AI_SERVICE_DOMAIN_SUFFIXES.len(), 28);
        assert_eq!(domains.len(), AI_SERVICE_DOMAIN_SUFFIXES.len());
        assert!(domains.iter().all(|domain| domain.starts_with("domain:")));
        assert_eq!(rules[0].outbound_tag.as_deref(), Some(PROXY_TAG));
        assert_eq!(rules[0].rule_type, Some(RuleType::ALL));
    }

    #[test]
    fn the_seed_is_named_in_the_interface_language() {
        assert_eq!(seed_routing_remarks("en"), "Smart routing");
        assert_eq!(seed_routing_remarks("zh-Hans"), "智能分流");
        assert_eq!(seed_routing_remarks("zh-Hant"), "智慧分流");
        assert_eq!(default_routing_item("Seed").rule_set, default_rule_set());
    }

    fn ai_services_rule(domains: Vec<String>) -> RulesItem {
        RulesItem {
            id: "rule-ai".to_string(),
            remarks: Some(SENTINEL_AI_SERVICES.to_string()),
            outbound_tag: Some(PROXY_TAG.to_string()),
            rule_type: Some(RuleType::ALL),
            domain: Some(domains),
            ..RulesItem::default()
        }
    }

    #[test]
    fn refresh_appends_missing_seed_matchers_and_preserves_user_entries() {
        let mut rule = ai_services_rule(vec![
            "domain:anthropic.com".to_string(),
            "domain:my-extra.example".to_string(),
        ]);
        assert!(refresh_managed_rule(&mut rule));
        let domains = rule.domain.as_ref().expect("domains");
        assert!(domains.contains(&"domain:claude.com".to_string()));
        assert!(domains.contains(&"domain:my-extra.example".to_string()));
        assert_eq!(domains[0], "domain:anthropic.com", "user order is kept");
        assert_eq!(domains[1], "domain:my-extra.example");

        assert!(
            !refresh_managed_rule(&mut rule),
            "an already-complete managed rule is unchanged"
        );
    }

    #[test]
    fn refresh_leaves_non_managed_rules_alone() {
        let mut custom = RulesItem {
            id: "rule-custom".to_string(),
            remarks: Some("mine".to_string()),
            domain: Some(vec!["full:example.test".to_string()]),
            ..RulesItem::default()
        };
        let before = custom.clone();
        assert!(!refresh_managed_rule(&mut custom));
        assert_eq!(custom, before);

        let mut anonymous = RulesItem {
            id: "rule-anon".to_string(),
            ..RulesItem::default()
        };
        assert!(!refresh_managed_rule(&mut anonymous));
        assert_eq!(anonymous.domain, None);
    }

    #[test]
    fn refresh_unions_ip_matchers_too() {
        let mut rule = RulesItem {
            id: "rule-cn-dns".to_string(),
            remarks: Some(SENTINEL_CN_DNS.to_string()),
            outbound_tag: Some(DIRECT_TAG.to_string()),
            rule_type: Some(RuleType::ALL),
            ip: Some(vec!["223.5.5.5".to_string()]),
            ..RulesItem::default()
        };
        assert!(refresh_managed_rule(&mut rule));
        let ips = rule.ip.expect("ips");
        assert!(ips.contains(&"119.29.29.29".to_string()));
        assert_eq!(ips[0], "223.5.5.5");
    }
}
