//! Pure VoyaVPN domain logic.
//!
//! This crate stays free of OS APIs, Tauri APIs, process launching, and
//! filesystem path discovery. Data models, parsers, and config generation are
//! rooted here so they can be tested headlessly.

pub mod config;
pub mod context;
pub mod entities;
pub mod enums;
pub mod fmt;
pub mod groups;
pub(crate) mod protocol_common;
pub mod singbox;

#[cfg(test)]
pub(crate) mod golden;

pub use config::*;
pub use context::*;
pub use entities::*;
pub use enums::*;
pub use fmt::*;
pub use groups::*;
pub use singbox::*;

/// A visible marker for workspace smoke tests and package metadata.
pub const CRATE_BOUNDARY: &str = "pure-domain";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn boundary_is_pure_domain() {
        assert_eq!(CRATE_BOUNDARY, "pure-domain");
    }

    #[test]
    fn routing_rule_serialization_reaches_singbox_generator() {
        let context = CoreConfigContext {
            node: ProfileItem {
                index_id: "node".to_string(),
                config_type: ConfigType::SOCKS,
                core_type: Some(CoreType::sing_box),
                remarks: "Node".to_string(),
                address: "127.0.0.1".to_string(),
                port: 1080,
                username: "user".to_string(),
                password: "pass".to_string(),
                network: "tcp".to_string(),
                ..ProfileItem::default()
            },
            run_core_type: CoreType::sing_box,
            routing_item: Some(sample_routing_item()),
            ..CoreConfigContext::default()
        };

        let generated: Value = serde_json::from_str(
            &generate_singbox_config_json(&context)
                .expect("generated sing-box config should serialize"),
        )
        .expect("generated sing-box config should parse as JSON");
        let rules = generated["route"]["rules"]
            .as_array()
            .expect("generated sing-box route rules should be an array");

        assert!(rules.iter().any(|rule| {
            rule["outbound"] == DIRECT_TAG
                && rule["domain"].as_array().is_some_and(|domains| {
                    domains.iter().any(|domain| domain == "direct.example.com")
                })
        }));
        assert!(rules.iter().any(|rule| {
            rule["action"] == "reject"
                && rule["domain"].as_array().is_some_and(|domains| {
                    domains.iter().any(|domain| domain == "block.example.com")
                })
        }));
    }

    fn sample_routing_item() -> RoutingItem {
        RoutingItem {
            id: "routing".to_string(),
            remarks: "Split".to_string(),
            domain_strategy: "AsIs".to_string(),
            rule_set: vec![
                RulesItem {
                    id: "direct".to_string(),
                    outbound_tag: Some(DIRECT_TAG.to_string()),
                    domain: Some(vec!["full:direct.example.com".to_string()]),
                    rule_type: Some(RuleType::Routing),
                    ..RulesItem::default()
                },
                RulesItem {
                    id: "block".to_string(),
                    outbound_tag: Some(BLOCK_TAG.to_string()),
                    domain: Some(vec!["full:block.example.com".to_string()]),
                    rule_type: Some(RuleType::Routing),
                    ..RulesItem::default()
                },
            ],
            ..RoutingItem::default()
        }
    }
}
