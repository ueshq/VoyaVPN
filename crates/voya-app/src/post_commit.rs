//! What a committed configuration change owes the running core.
//!
//! Two hosts ask the same question after every mutation — does this change need
//! the core restarted, and if the restart fails, which notice names the
//! operation? — so the answer lives here rather than in either of them. The
//! Tauri shell's `ipc/commands/post_commit.rs` and the mobile host's
//! `dispatch/` both reach for these constants (ADR 0012).

use voya_contracts::{AppNotice, AppNoticeLevel, CoreFlowReason, LogCode, LogLevel, NoticeCode};
use voya_core::AppConfig;

use crate::{core_flow::CoreFlow, invalidation::InvalidationBundle};

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

/// What a host owes a notice raised after a change was committed: the notice
/// itself, and for a failure the line that records it on the Logs page.
#[derive(Debug, Clone)]
pub struct NoticeEffects {
    /// `None` for news that is not a failure.
    pub log_level: Option<LogLevel>,
    pub notice: AppNotice,
}

/// Decides both halves in one place, so the desktop and the phones cannot
/// drift apart on which notices are logged or what detail they carry.
///
/// `detail` is the untranslated error behind `code`: it goes to the log line
/// and the notice's second line, never into the title the user reads.
#[must_use]
pub fn notice_effects(level: AppNoticeLevel, code: NoticeCode, detail: &str) -> NoticeEffects {
    // What the IPv6 check found is news, not a failed post-commit step, and
    // its code already carries everything there is to say.
    if matches!(
        code,
        NoticeCode::NodeIpv6Unsupported { .. } | NoticeCode::NodeIpv6Restored { .. }
    ) {
        return NoticeEffects {
            log_level: None,
            notice: AppNotice {
                level,
                code,
                detail: None,
            },
        };
    }

    NoticeEffects {
        log_level: Some(match level {
            AppNoticeLevel::Info => LogLevel::Info,
            AppNoticeLevel::Warning => LogLevel::Warn,
            AppNoticeLevel::Error => LogLevel::Error,
        }),
        notice: AppNotice {
            level,
            code,
            detail: Some(detail.to_string()),
        },
    }
}

/// The code every post-commit failure is logged under.
pub const POST_COMMIT_LOG_CODE: LogCode = LogCode::PostCommitFailed;

/// How a host announces the two follow-ups every committed change may owe.
///
/// Emission is best-effort by design: the change is already persisted when
/// [`finish_config_change`] runs, so a dead event channel becomes a warning
/// notice and never changes the command's result.
pub trait PostCommitSink: Send + Sync {
    /// Announce the query caches this change invalidated; the bundle names
    /// the notice to raise when the announcement itself fails.
    fn invalidate(&self, reason: &str, bundle: InvalidationBundle);
    /// A change that was already committed, whose follow-up work failed.
    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str);
}

/// The tail every committed configuration change shares: announce the caches,
/// then restart a connected core for the change. The change is already
/// persisted by the time this runs, so a failed restart is a warning notice
/// rather than a command error.
///
/// `bundle` is `None` when the caller already announced its caches and this
/// call owes only the restart.
pub async fn finish_config_change(
    sink: &dyn PostCommitSink,
    flow: &CoreFlow<'_>,
    reason: &str,
    bundle: Option<InvalidationBundle>,
    config: &AppConfig,
    change: ConfigChange,
) {
    if let Some(bundle) = bundle {
        sink.invalidate(reason, bundle);
    }

    if let Err(error) = flow.restart_if_connected(config, change.reason).await {
        sink.notice(
            AppNoticeLevel::Warning,
            change.restart_failed_code,
            &error.to_string(),
        );
    }
}

#[cfg(test)]
mod tests;
