//! Validator findings as codes, not sentences.
//!
//! `validate_node`, the group traversal in [`crate::context`] and the group
//! builder in [`crate::groups`] all report problems that end up on screen: the
//! group builder renders them directly, and `connect` carries them out through
//! `RuntimeError::Validation`. They used to be `String`s built with `format!`,
//! which meant a Persian or Hungarian user read them in English and
//! `pnpm check:i18n` — which only sees frontend source — could not tell.
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
    InvalidFinalMask,
    UnsupportedNetwork {
        network: String,
    },
    UnsupportedProtocol {
        protocol: String,
    },
    UnsupportedProtocolNetwork {
        protocol: String,
        network: String,
    },
    UnsupportedShadowsocksNetwork {
        network: String,
    },
    // ---- policy groups and proxy chains ----
    NotAGroupProfile,
    GroupCycle {
        group: String,
        child: String,
    },
    /// The cycle the group builder found, which knows the whole path.
    GroupCyclePath {
        path: Vec<String>,
    },
    GroupWithoutValidChild {
        group: String,
    },
    PolicyGroupWithoutValidChildren,
    ProxyChainWithoutValidChildren,
    ProxyChainSingleHop,
    GroupChildNotFound {
        profile_id: String,
    },
    GroupDuplicateChildIgnored {
        profile_id: String,
    },
    InvalidSubscriptionFilter {
        pattern: String,
    },
    // ---- routing rules ----
    RoutingRuleWithoutOutbound {
        rule: String,
    },
    RoutingRuleOutboundNotFound {
        rule: String,
        outbound: String,
    },
}

/// One hop of the path a validator walked to reach a finding.
///
/// The traversal used to glue `"group child A / B: "` onto the front of the
/// child's own message. That prefix is the reason a child's finding could not
/// be translated: it was prose wrapped around prose. Now the hops travel
/// alongside the code and the renderer assembles the breadcrumb.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ValidationScope {
    GroupChild { group: String, child: String },
    RoutingRuleOutbound { rule: String, outbound: String },
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
            .within(ValidationScope::GroupChild {
                group: "Inner".to_string(),
                child: "Leaf".to_string(),
            })
            .within(ValidationScope::GroupChild {
                group: "Outer".to_string(),
                child: "Inner".to_string(),
            });

        assert_eq!(
            message.scope,
            vec![
                ValidationScope::GroupChild {
                    group: "Outer".to_string(),
                    child: "Inner".to_string(),
                },
                ValidationScope::GroupChild {
                    group: "Inner".to_string(),
                    child: "Leaf".to_string(),
                },
            ]
        );
    }
}
