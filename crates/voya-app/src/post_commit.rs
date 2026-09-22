//! What a committed configuration change owes the running core.
//!
//! Two hosts ask the same question after every mutation — does this change need
//! the core restarted, and if the restart fails, which notice names the
//! operation? — so the answer lives here rather than in either of them. The
//! Tauri shell's `ipc/commands/post_commit.rs` and the mobile host's
//! `dispatch/` both reach for these constants (ADR 0012).

use voya_contracts::{AppNoticeLevel, CoreFlowReason, InvalidationScope, NoticeCode};
use voya_core::AppConfig;

use crate::core_flow::CoreFlow;

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
    /// Notice raised when the cache invalidation emit fails.
    pub refresh_failed_code: NoticeCode,
}

impl ConfigChange {
    /// Every routing mutation shares the same core-flow reason and only differs
    /// in which operation the failure notice names.
    const fn routing(restart_failed_code: NoticeCode) -> Self {
        Self {
            reason: CoreFlowReason::RoutingChanged,
            restart_failed_code,
            refresh_failed_code: NoticeCode::RoutingRefreshFailed,
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
        refresh_failed_code: NoticeCode::ConnectionModeRefreshFailed,
    };
    pub const CONNECTION_MODE: Self = Self {
        reason: CoreFlowReason::ConnectionModeChanged,
        restart_failed_code: NoticeCode::ConnectionModeSavedRestartFailed,
        refresh_failed_code: NoticeCode::ConnectionModeRefreshFailed,
    };
    pub const ACTIVE_PROFILE: Self = Self {
        reason: CoreFlowReason::ActiveProfileChanged,
        restart_failed_code: NoticeCode::ActiveProfileRestartFailed,
        refresh_failed_code: NoticeCode::ProfileRefreshFailed,
    };
    pub const POLICY_GROUP: Self = Self {
        reason: CoreFlowReason::PolicyGroupChanged,
        restart_failed_code: NoticeCode::PolicyGroupSavedRestartFailed,
        refresh_failed_code: NoticeCode::PolicyGroupRefreshFailed,
    };
}

/// How a host announces the two follow-ups every committed change may owe.
///
/// Emission is best-effort by design: the change is already persisted when
/// [`finish_config_change`] runs, so a dead event channel becomes a warning
/// notice and never changes the command's result.
pub trait PostCommitSink: Send + Sync {
    /// Announce the query caches this change invalidated.
    fn invalidate(
        &self,
        reason: &str,
        scopes: &[InvalidationScope],
        refresh_failed_code: NoticeCode,
    );
    /// A change that was already committed, whose follow-up work failed.
    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str);
}

/// The tail every committed configuration change shares: announce the caches,
/// then restart a connected core for the change. The change is already
/// persisted by the time this runs, so a failed restart is a warning notice
/// rather than a command error.
///
/// `scopes` empty means the caller already announced them and this call owes
/// only the restart.
pub async fn finish_config_change(
    sink: &dyn PostCommitSink,
    flow: &CoreFlow<'_>,
    reason: &str,
    scopes: &[InvalidationScope],
    config: &AppConfig,
    change: ConfigChange,
) {
    if !scopes.is_empty() {
        sink.invalidate(reason, scopes, change.refresh_failed_code);
    }

    if let Err(error) = flow.restart_if_connected(config, change.reason).await {
        sink.notice(
            AppNoticeLevel::Warning,
            change.restart_failed_code,
            &format!("{error:?}"),
        );
    }
}

#[cfg(test)]
mod tests;
