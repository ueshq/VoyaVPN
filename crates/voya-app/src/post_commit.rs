//! What a committed configuration change owes the running core.
//!
//! Two hosts ask the same question after every mutation — does this change need
//! the core restarted, and if the restart fails, which notice names the
//! operation? — so the answer lives here rather than in either of them. The
//! Tauri shell's `ipc/commands/post_commit.rs` and the mobile host's
//! `dispatch/` both reach for these constants (ADR 0012).

use voya_contracts::{CoreFlowReason, NoticeCode};

/// One committed change, described by what it means to a running core.
///
/// `NoticeCode` carries data in some variants, so this clones rather than
/// copies; every use is a constant handed straight to the restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChange {
    /// Reason the core flow's log line names.
    pub reason: CoreFlowReason,
    /// Notice raised when the follow-up restart fails. The change is already
    /// persisted by then, so a failed restart is a warning, never an error.
    pub restart_failed_code: NoticeCode,
}

impl ConfigChange {
    /// Every routing mutation shares the same core-flow reason and only differs
    /// in which operation the failure notice names.
    const fn routing(restart_failed_code: NoticeCode) -> Self {
        Self {
            reason: CoreFlowReason::RoutingChanged,
            restart_failed_code,
        }
    }

    pub const ROUTING_SAVED: Self = Self::routing(NoticeCode::RoutingSavedRestartFailed);
    pub const ROUTING_DELETED: Self = Self::routing(NoticeCode::RoutingDeletedRestartFailed);
    pub const ROUTING_SELECTED: Self = Self::routing(NoticeCode::RoutingSelectedRestartFailed);
    pub const ROUTING_RULE_SAVED: Self = Self::routing(NoticeCode::RoutingRuleSavedRestartFailed);
    pub const ROUTING_RULES_DELETED: Self =
        Self::routing(NoticeCode::RoutingRulesDeletedRestartFailed);
    pub const ROUTING_RULE_MOVED: Self = Self::routing(NoticeCode::RoutingRuleMovedRestartFailed);
    pub const ROUTING_RULES_RESET: Self = Self::routing(NoticeCode::RoutingRulesResetRestartFailed);
    pub const TUN: Self = Self {
        reason: CoreFlowReason::TunChanged,
        restart_failed_code: NoticeCode::TunSavedRestartFailed,
    };
    pub const CONNECTION_MODE: Self = Self {
        reason: CoreFlowReason::ConnectionModeChanged,
        restart_failed_code: NoticeCode::ConnectionModeSavedRestartFailed,
    };
    pub const ACTIVE_PROFILE: Self = Self {
        reason: CoreFlowReason::ActiveProfileChanged,
        restart_failed_code: NoticeCode::ActiveProfileRestartFailed,
    };
    pub const POLICY_GROUP: Self = Self {
        reason: CoreFlowReason::PolicyGroupChanged,
        restart_failed_code: NoticeCode::PolicyGroupSavedRestartFailed,
    };
}

#[cfg(test)]
mod tests;
