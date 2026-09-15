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

#[must_use]
fn validation_code_to_contract(code: CoreValidationCode) -> ValidationCode {
    match code {
        CoreValidationCode::InvalidAddress => ValidationCode::InvalidAddress,
        CoreValidationCode::InvalidPort => ValidationCode::InvalidPort,
        CoreValidationCode::InvalidPassword => ValidationCode::InvalidPassword,
        CoreValidationCode::InvalidFlow => ValidationCode::InvalidFlow,
        CoreValidationCode::InvalidShadowsocksMethod => ValidationCode::InvalidShadowsocksMethod,
        CoreValidationCode::InvalidRealityPublicKey => ValidationCode::InvalidRealityPublicKey,
        CoreValidationCode::UnsupportedProtocol { protocol } => {
            ValidationCode::UnsupportedProtocol { protocol }
        }
        CoreValidationCode::UnsupportedProtocolNetwork { protocol, network } => {
            ValidationCode::UnsupportedProtocolNetwork { protocol, network }
        }
        CoreValidationCode::UnsupportedShadowsocksNetwork { network } => {
            ValidationCode::UnsupportedShadowsocksNetwork { network }
        }
        CoreValidationCode::RoutingRuleWithoutOutbound { rule } => {
            ValidationCode::RoutingRuleWithoutOutbound { rule }
        }
        CoreValidationCode::RoutingRuleOutboundNotFound { rule, outbound } => {
            ValidationCode::RoutingRuleOutboundNotFound { rule, outbound }
        }
        CoreValidationCode::PolicyGroupWithoutValidMembers { group } => {
            ValidationCode::PolicyGroupWithoutValidMembers { group }
        }
    }
}

#[must_use]
fn validation_scope_to_contract(scope: CoreValidationScope) -> ValidationScope {
    match scope {
        CoreValidationScope::RoutingRuleOutbound { rule, outbound } => {
            ValidationScope::RoutingRuleOutbound { rule, outbound }
        }
        CoreValidationScope::PolicyGroupMember { group, member } => {
            ValidationScope::PolicyGroupMember { group, member }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn findings_keep_the_breadcrumb_the_validator_walked() {
        let issue = validation_issue_to_contract(
            "activeProfile",
            CoreValidationMessage::new(CoreValidationCode::InvalidPort).within(
                CoreValidationScope::RoutingRuleOutbound {
                    rule: "Rule".to_string(),
                    outbound: "Leaf".to_string(),
                },
            ),
        );

        assert_eq!(issue.field, "activeProfile");
        assert_eq!(issue.code, ValidationCode::InvalidPort);
        assert_eq!(
            issue.scope,
            vec![ValidationScope::RoutingRuleOutbound {
                rule: "Rule".to_string(),
                outbound: "Leaf".to_string(),
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

        assert_eq!(tags.len(), 11);
    }
}
