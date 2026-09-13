//! Validator findings as codes, not sentences.
//!
//! Node and routing validation findings are carried to the UI as localized codes.
//!
//! These types are deliberately voya-core's own rather than the contract ones:
//! this crate has no `voya-contracts` dependency, so the mapping lives in
//! `voya_app::contract_map::messages` alongside every other core → contract
//! conversion.

use serde::{Deserialize, Serialize};

/// What a validator rejected, with the values that made it reject.
///
/// One variant per message, carrying the interpolation parameters by name so
/// the frontend can put them wherever its locale wants them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ValidationCode {
    // ---- node fields ----
    InvalidAddress,
    InvalidPort,
    InvalidPassword,
    InvalidFlow,
    InvalidShadowsocksMethod,
    InvalidRealityPublicKey,
    UnsupportedProtocol { protocol: String },
    UnsupportedProtocolNetwork { protocol: String, network: String },
    UnsupportedShadowsocksNetwork { network: String },
    // ---- routing rules ----
    RoutingRuleWithoutOutbound { rule: String },
    RoutingRuleOutboundNotFound { rule: String, outbound: String },
    // ---- policy groups ----
    PolicyGroupWithoutValidMembers { group: String },
}

/// One hop of the path a validator walked to reach a finding.
///
/// Routing findings identify the rule and the node it targets.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ValidationScope {
    RoutingRuleOutbound { rule: String, outbound: String },
    PolicyGroupMember { group: String, member: String },
}

/// One finding: what went wrong, and where the validator was when it did.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationMessage {
    pub code: ValidationCode,
    /// Outermost hop first. Empty for a finding about the profile itself.
    pub scope: Vec<ValidationScope>,
}

impl ValidationMessage {
    #[must_use]
    pub const fn new(code: ValidationCode) -> Self {
        Self {
            code,
            scope: Vec::new(),
        }
    }

    /// The same finding, reported one hop further out.
    #[must_use]
    pub fn within(mut self, scope: ValidationScope) -> Self {
        self.scope.insert(0, scope);
        self
    }
}

impl From<ValidationCode> for ValidationMessage {
    fn from(code: ValidationCode) -> Self {
        Self::new(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_nest_outermost_first() {
        let message = ValidationMessage::new(ValidationCode::InvalidPort)
            .within(ValidationScope::RoutingRuleOutbound {
                rule: "Inner".to_string(),
                outbound: "Leaf".to_string(),
            })
            .within(ValidationScope::RoutingRuleOutbound {
                rule: "Outer".to_string(),
                outbound: "Inner".to_string(),
            });

        assert_eq!(
            message.scope,
            vec![
                ValidationScope::RoutingRuleOutbound {
                    rule: "Outer".to_string(),
                    outbound: "Inner".to_string(),
                },
                ValidationScope::RoutingRuleOutbound {
                    rule: "Inner".to_string(),
                    outbound: "Leaf".to_string(),
                },
            ]
        );
    }
}
