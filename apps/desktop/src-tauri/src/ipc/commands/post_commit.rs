//! What follows a committed change: cache invalidation, the core restart it
//! may need, and the status events it changes.

use super::{support::*, *};

/// Broadcasts one invalidation bundle and reports a failed emit as a notice.
///
/// The seven `emit_*_invalidation` wrappers below were eight copies of this
/// body (the eighth was hand-rolled in `lib.rs`) that differed only in the
/// notice title and the key list. Emitting is best-effort by design: the change
/// is already committed when this runs, so a dead event channel becomes a
/// warning notice and never changes the command's result.
///
/// `scopes` is deduplicated and ordered so the payload is deterministic
/// regardless of how a caller assembled its list.
pub(crate) fn emit_invalidation<R, I>(
    app: &tauri::AppHandle<R>,
    failure_code: NoticeCode,
    reason: &str,
    scopes: I,
) where
    R: tauri::Runtime,
    I: IntoIterator<Item = InvalidationScope>,
{
    let scopes: BTreeSet<InvalidationScope> = scopes.into_iter().collect();
    // Nodes, the active node and the traffic mode all sit behind these scopes,
    // and the tray shows each of them.
    if let Err(error) = crate::refresh_tray_menu(app) {
        tracing::warn!(?error, "failed to queue a tray menu refresh");
    }
    if let Err(error) = (InvalidateEvent {
        keys: scopes
            .into_iter()
            .map(|scope| QueryInvalidation {
                scope,
                reason: reason.to_string(),
            })
            .collect(),
    })
    .emit(app)
    {
        report_post_commit_error(
            app,
            failure_code,
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
}

/// Profile-list mutations. `config_changed` reports that the commit also
/// rewrote the persisted `AppConfig` (see `voya_app::invalidation`).
pub(super) fn emit_profile_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    config_changed: bool,
) where
    R: tauri::Runtime,
{
    emit_invalidation(
        app,
        NoticeCode::ProfileRefreshFailed,
        reason,
        invalidation::profile_scopes(config_changed),
    );
}

pub(super) fn emit_policy_group_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    config_changed: bool,
) where
    R: tauri::Runtime,
{
    emit_invalidation(
        app,
        NoticeCode::PolicyGroupRefreshFailed,
        reason,
        invalidation::policy_group_scopes(config_changed),
    );
}

pub(crate) fn emit_subscription_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    profiles_changed: bool,
    config_changed: bool,
) where
    R: tauri::Runtime,
{
    emit_invalidation(
        app,
        NoticeCode::SubscriptionRefreshFailed,
        reason,
        invalidation::subscription_scopes(profiles_changed, config_changed),
    );
}

pub(super) fn emit_routing_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    config_changed: bool,
) where
    R: tauri::Runtime,
{
    emit_invalidation(
        app,
        NoticeCode::RoutingRefreshFailed,
        reason,
        invalidation::routing_scopes(config_changed),
    );
}

pub(super) fn emit_dns_invalidation<R>(app: &tauri::AppHandle<R>, reason: &str)
where
    R: tauri::Runtime,
{
    emit_invalidation(
        app,
        NoticeCode::DnsRefreshFailed,
        reason,
        invalidation::dns_scopes(),
    );
}

/// Proxy-runtime commands. `config_changed` is true only for the traffic-mode
/// command, which also persists a settings field.
pub(super) fn emit_proxy_runtime_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    config_changed: bool,
) where
    R: tauri::Runtime,
{
    emit_invalidation(
        app,
        NoticeCode::ProxyViewRefreshFailed,
        reason,
        invalidation::proxy_runtime_scopes(config_changed),
    );
}

/// The TUN / system-proxy mode commands, which persist settings fields the
/// bundle mirrors without going through `save_app_settings`.
pub(super) fn emit_connection_mode_invalidation<R>(app: &tauri::AppHandle<R>, reason: &str)
where
    R: tauri::Runtime,
{
    emit_invalidation(
        app,
        NoticeCode::ConnectionModeRefreshFailed,
        reason,
        invalidation::connection_mode_scopes(),
    );
}

pub(super) fn emit_proxy_monitor_status<R>(app: &tauri::AppHandle<R>, status: &ProxyMonitorStatus)
where
    R: tauri::Runtime,
{
    emit_or_warn(
        app,
        TransientStreamEvent::ProxyMonitorStatus(status.clone()),
        "proxy monitor status event",
    );
}

pub(crate) fn emit_tun_changed<R>(
    app: &tauri::AppHandle<R>,
    status: &TunStatus,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    emit_event(app, TransientStreamEvent::TunChanged(status.clone()))
}

/// Names one committed configuration change for the restart that follows it.
///
/// Every mutating command used to spell both strings out inline around an
/// identical eight-line `if let Err(..) { report_post_commit_error(..) }`
/// block; the label was the only thing that varied.
#[derive(Clone)]
pub(super) struct ConfigChange {
    /// Reason the core flow's log line names.
    reason: CoreFlowReason,
    /// Notice raised when the follow-up restart fails.
    restart_failed_code: NoticeCode,
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

    pub(super) const ROUTING_SAVED: Self = Self::routing(NoticeCode::RoutingSavedRestartFailed);
    pub(super) const ROUTING_DELETED: Self = Self::routing(NoticeCode::RoutingDeletedRestartFailed);
    pub(super) const ROUTING_SELECTED: Self =
        Self::routing(NoticeCode::RoutingSelectedRestartFailed);
    pub(super) const ROUTING_RULE_SAVED: Self =
        Self::routing(NoticeCode::RoutingRuleSavedRestartFailed);
    pub(super) const ROUTING_RULES_DELETED: Self =
        Self::routing(NoticeCode::RoutingRulesDeletedRestartFailed);
    pub(super) const ROUTING_RULE_MOVED: Self =
        Self::routing(NoticeCode::RoutingRuleMovedRestartFailed);
    pub(super) const ROUTING_RULES_RESET: Self =
        Self::routing(NoticeCode::RoutingRulesResetRestartFailed);
    pub(super) const TUN: Self = Self {
        reason: CoreFlowReason::TunChanged,
        restart_failed_code: NoticeCode::TunSavedRestartFailed,
    };
    pub(super) const CONNECTION_MODE: Self = Self {
        reason: CoreFlowReason::ConnectionModeChanged,
        restart_failed_code: NoticeCode::ConnectionModeSavedRestartFailed,
    };
    pub(super) const ACTIVE_PROFILE: Self = Self {
        reason: CoreFlowReason::ActiveProfileChanged,
        restart_failed_code: NoticeCode::ActiveProfileRestartFailed,
    };
    pub(super) const POLICY_GROUP: Self = Self {
        reason: CoreFlowReason::PolicyGroupChanged,
        restart_failed_code: NoticeCode::PolicyGroupSavedRestartFailed,
    };
}

/// Restarts the core after a committed configuration change, if it is running.
///
/// The whole sequence — the connecting/connected events, the system proxy, the
/// TUN status, and the recovery when the restart fails — lives in
/// `voya_app::core_flow`, shared with connect/disconnect and the crash paths.
/// The change is already persisted when this runs, so a restart failure is a
/// warning notice rather than a command error.
pub(super) async fn restart_after_config_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
    change: ConfigChange,
) where
    R: tauri::Runtime,
{
    if let Err(error) = core_flow(app, state)
        .restart_if_connected(config, change.reason)
        .await
        .map_err(AppError::from)
    {
        report_post_commit_error(
            app,
            change.restart_failed_code,
            &format!("{error:?}"),
            AppNoticeLevel::Warning,
        );
    }
}

/// The tail every routing mutation shares: refresh the routing caches, then
/// restart the connected core for the committed change. `config_changed` is
/// whatever the commit itself reported.
pub(super) async fn finish_routing_change<R, T>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    committed: &CommittedMutation<T>,
    reason: &str,
    change: ConfigChange,
) where
    R: tauri::Runtime,
{
    emit_routing_invalidation(app, reason, committed.config_changed);
    restart_after_config_change(app, state, &committed.config, change).await;
}

/// The tail the removal-family commands share: emit the subsystem's
/// invalidation, then disconnect the core if the change removed the node or
/// group it was running. Every path that can delete or replace the running
/// target must end here, or the core stays connected to a deleted profile.
pub(super) async fn emit_then_disconnect_removed<R, F>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    emit: F,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
    F: FnOnce(&tauri::AppHandle<R>),
{
    emit(app);
    disconnect_removed_profile(app, state).await
}

/// Disconnects the core if the node or group it is running no longer exists.
///
/// Split out of [`emit_then_disconnect_removed`] for the auto-update sink,
/// which emits its invalidation before spawning the disconnect off its
/// callback thread.
pub(crate) async fn disconnect_removed_profile<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    core_flow(app, state)
        .disconnect_removed_profile(&current_config(state))
        .await
        .map_err(AppError::from)
}

pub(super) fn emit_settings_bundle_invalidation<R>(app: &tauri::AppHandle<R>, reason: &str)
where
    R: tauri::Runtime,
{
    emit_invalidation(
        app,
        NoticeCode::SettingsRefreshFailed,
        reason,
        invalidation::settings_bundle_scopes(),
    );
}

pub(super) fn emit_sysproxy_changed<R>(
    app: &tauri::AppHandle<R>,
    status: &SystemProxyStatus,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    emit_event(
        app,
        TransientStreamEvent::SysProxyChanged(system_proxy_status_to_contract(status.clone())),
    )
}
