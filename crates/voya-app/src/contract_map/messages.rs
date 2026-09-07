//! Core validator findings → contract validation issues.
//!
//! `voya-core` has no `voya-contracts` dependency, so it declares its own
//! `ValidationCode` / `ValidationScope` / `ValidationMessage` and this module
//! is the single place the two vocabularies meet — the same route
//! `protocol_to_contract` takes for profiles.
//!
//! The match is exhaustive on purpose: a code added in `voya-core` fails this
//! file to compile until the contract, the generated bindings and the eight
//! locale files have caught up, which is exactly the gate the free-string
//! messages never had.

use voya_contracts::{ValidationCode, ValidationIssue, ValidationScope};
use voya_core::validation::{
    ValidationCode as CoreValidationCode, ValidationMessage as CoreValidationMessage,
    ValidationScope as CoreValidationScope,
};

/// The field a group-builder finding is addressed to.
///
/// The group form has three inputs the validator can be talking about, and the
/// builder highlights the one that is named.
fn group_field(code: &CoreValidationCode) -> &'static str {
    match code {
        CoreValidationCode::NotAGroupProfile => "protocol",
        CoreValidationCode::InvalidSubscriptionFilter { .. } => "filter",
        _ => "children",
    }
}

/// One core finding as a contract issue addressed to `field`.
#[must_use]
pub fn validation_issue_to_contract(
    field: impl Into<String>,
    message: CoreValidationMessage,
) -> ValidationIssue {
    ValidationIssue {
        field: field.into(),
        code: validation_code_to_contract(message.code),
        scope: message
            .scope
            .into_iter()
            .map(validation_scope_to_contract)
            .collect(),
    }
}

/// Core findings from the group builder, each addressed to the input it is
/// about.
#[must_use]
pub fn group_validation_to_contract(messages: Vec<CoreValidationMessage>) -> Vec<ValidationIssue> {
    messages
        .into_iter()
        .map(|message| {
            let field = group_field(&message.code);
            validation_issue_to_contract(field, message)
        })
        .collect()
}

#[must_use]
pub fn validation_code_to_contract(code: CoreValidationCode) -> ValidationCode {
    match code {
        CoreValidationCode::InvalidAddress => ValidationCode::InvalidAddress,
        CoreValidationCode::InvalidPort => ValidationCode::InvalidPort,
        CoreValidationCode::InvalidPassword => ValidationCode::InvalidPassword,
        CoreValidationCode::InvalidFlow => ValidationCode::InvalidFlow,
        CoreValidationCode::InvalidShadowsocksMethod => ValidationCode::InvalidShadowsocksMethod,
        CoreValidationCode::InvalidRealityPublicKey => ValidationCode::InvalidRealityPublicKey,
        CoreValidationCode::InvalidFinalMask => ValidationCode::InvalidFinalMask,
        CoreValidationCode::UnsupportedNetwork { network } => {
            ValidationCode::UnsupportedNetwork { network }
        }
        CoreValidationCode::UnsupportedProtocol { protocol } => {
            ValidationCode::UnsupportedProtocol { protocol }
        }
        CoreValidationCode::UnsupportedProtocolNetwork { protocol, network } => {
            ValidationCode::UnsupportedProtocolNetwork { protocol, network }
        }
        CoreValidationCode::UnsupportedShadowsocksNetwork { network } => {
            ValidationCode::UnsupportedShadowsocksNetwork { network }
        }
        CoreValidationCode::NotAGroupProfile => ValidationCode::NotAGroupProfile,
        CoreValidationCode::GroupCycle { group, child } => {
            ValidationCode::GroupCycle { group, child }
        }
        CoreValidationCode::GroupCyclePath { path } => ValidationCode::GroupCyclePath { path },
        CoreValidationCode::GroupWithoutValidChild { group } => {
            ValidationCode::GroupWithoutValidChild { group }
        }
        CoreValidationCode::PolicyGroupWithoutValidChildren => {
            ValidationCode::PolicyGroupWithoutValidChildren
        }
        CoreValidationCode::ProxyChainWithoutValidChildren => {
            ValidationCode::ProxyChainWithoutValidChildren
        }
        CoreValidationCode::ProxyChainSingleHop => ValidationCode::ProxyChainSingleHop,
        CoreValidationCode::GroupChildNotFound { profile_id } => {
            ValidationCode::GroupChildNotFound { profile_id }
        }
        CoreValidationCode::GroupDuplicateChildIgnored { profile_id } => {
            ValidationCode::GroupDuplicateChildIgnored { profile_id }
        }
        CoreValidationCode::InvalidSubscriptionFilter { pattern } => {
            ValidationCode::InvalidSubscriptionFilter { pattern }
        }
        CoreValidationCode::RoutingRuleWithoutOutbound { rule } => {
            ValidationCode::RoutingRuleWithoutOutbound { rule }
        }
        CoreValidationCode::RoutingRuleOutboundNotFound { rule, outbound } => {
            ValidationCode::RoutingRuleOutboundNotFound { rule, outbound }
        }
    }
}

#[must_use]
pub fn validation_scope_to_contract(scope: CoreValidationScope) -> ValidationScope {
    match scope {
        CoreValidationScope::GroupChild { group, child } => {
            ValidationScope::GroupChild { group, child }
        }
        CoreValidationScope::RoutingRuleOutbound { rule, outbound } => {
            ValidationScope::RoutingRuleOutbound { rule, outbound }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_findings_are_addressed_to_the_input_they_are_about() {
        let issues = group_validation_to_contract(vec![
            CoreValidationMessage::new(CoreValidationCode::NotAGroupProfile),
            CoreValidationMessage::new(CoreValidationCode::InvalidSubscriptionFilter {
                pattern: "^(HK".to_string(),
            }),
            CoreValidationMessage::new(CoreValidationCode::ProxyChainSingleHop),
        ]);

        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.field.as_str())
                .collect::<Vec<_>>(),
            vec!["protocol", "filter", "children"]
        );
        assert_eq!(
            issues[1].code,
            ValidationCode::InvalidSubscriptionFilter {
                pattern: "^(HK".to_string()
            }
        );
    }

    #[test]
    fn findings_keep_the_breadcrumb_the_validator_walked() {
        let issue = validation_issue_to_contract(
            "activeProfile",
            CoreValidationMessage::new(CoreValidationCode::InvalidPort).within(
                CoreValidationScope::GroupChild {
                    group: "Group".to_string(),
                    child: "Leaf".to_string(),
                },
            ),
        );

        assert_eq!(issue.field, "activeProfile");
        assert_eq!(issue.code, ValidationCode::InvalidPort);
        assert_eq!(
            issue.scope,
            vec![ValidationScope::GroupChild {
                group: "Group".to_string(),
                child: "Leaf".to_string(),
            }]
        );
    }

    #[test]
    fn every_core_code_maps_to_a_distinct_contract_code() {
        // A code added in voya-core has to be given a contract code, a locale
        // key and a translation; mapping two onto one would hide the second.
        let codes = [
            CoreValidationCode::InvalidAddress,
            CoreValidationCode::InvalidPort,
            CoreValidationCode::InvalidPassword,
            CoreValidationCode::InvalidFlow,
            CoreValidationCode::InvalidShadowsocksMethod,
            CoreValidationCode::InvalidRealityPublicKey,
            CoreValidationCode::InvalidFinalMask,
            CoreValidationCode::UnsupportedNetwork {
                network: String::new(),
            },
            CoreValidationCode::UnsupportedProtocol {
                protocol: String::new(),
            },
            CoreValidationCode::UnsupportedProtocolNetwork {
                protocol: String::new(),
                network: String::new(),
            },
            CoreValidationCode::UnsupportedShadowsocksNetwork {
                network: String::new(),
            },
            CoreValidationCode::NotAGroupProfile,
            CoreValidationCode::GroupCycle {
                group: String::new(),
                child: String::new(),
            },
            CoreValidationCode::GroupCyclePath { path: Vec::new() },
            CoreValidationCode::GroupWithoutValidChild {
                group: String::new(),
            },
            CoreValidationCode::PolicyGroupWithoutValidChildren,
            CoreValidationCode::ProxyChainWithoutValidChildren,
            CoreValidationCode::ProxyChainSingleHop,
            CoreValidationCode::GroupChildNotFound {
                profile_id: String::new(),
            },
            CoreValidationCode::GroupDuplicateChildIgnored {
                profile_id: String::new(),
            },
            CoreValidationCode::InvalidSubscriptionFilter {
                pattern: String::new(),
            },
            CoreValidationCode::RoutingRuleWithoutOutbound {
                rule: String::new(),
            },
            CoreValidationCode::RoutingRuleOutboundNotFound {
                rule: String::new(),
                outbound: String::new(),
            },
        ];
        let tags = codes
            .into_iter()
            .map(|code| {
                serde_json::to_value(validation_code_to_contract(code))
                    .expect("serialize contract code")["code"]
                    .as_str()
                    .expect("code tag")
                    .to_string()
            })
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(tags.len(), 23);
    }
}
