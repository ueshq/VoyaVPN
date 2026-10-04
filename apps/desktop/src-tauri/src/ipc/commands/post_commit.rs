//! What follows a committed change: cache invalidation, the core restart it
//! may need, and the status events it changes.

pub(super) use voya_app::post_commit::{self, ConfigChange, PostCommitSink};

use super::{support::*, *};

/// Broadcasts one invalidation bundle and reports a failed emit as a notice.
///
/// Which scopes a change touches, and the event they travel in, come from
/// `voya_app::invalidation`. Emitting is best-effort by design: the change is
/// already committed when this runs, so a dead event channel becomes a warning
/// notice and never changes the command's result.
pub(crate) fn emit_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    (failure_code, scopes): invalidation::InvalidationBundle,
) where
    R: tauri::Runtime,
{
    if invalidation::shows_in_tray(&scopes) {
        if let Err(error) = crate::refresh_tray_menu(app) {
            tracing::warn!(?error, "failed to queue a tray menu refresh");
        }
    }
    if let Err(error) = invalidation::invalidate_event(reason, scopes).emit(app) {
        report_post_commit_error(
            app,
            failure_code,
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
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

/// The restart a committed change may owe a connected core, for callers that
/// already announced their caches.
///
/// The choreography is `voya_app::post_commit::finish_config_change`, shared
/// with the mobile host; this supplies the Tauri sinks.
pub(super) async fn restart_after_config_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
    change: ConfigChange,
) where
    R: tauri::Runtime,
{
    post_commit::finish_config_change(
        &TauriPostCommitSink { app: app.clone() },
        &core_flow(app, state),
        "",
        None,
        config,
        change,
    )
    .await;
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
    post_commit::finish_config_change(
        &TauriPostCommitSink { app: app.clone() },
        &core_flow(app, state),
        reason,
        Some(invalidation::routing_scopes(committed.config_changed)),
        &committed.config,
        change,
    )
    .await;
}

struct TauriPostCommitSink<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R> PostCommitSink for TauriPostCommitSink<R>
where
    R: tauri::Runtime,
{
    fn invalidate(&self, reason: &str, bundle: invalidation::InvalidationBundle) {
        emit_invalidation(&self.app, reason, bundle);
    }

    fn notice(&self, level: AppNoticeLevel, code: NoticeCode, detail: &str) {
        report_post_commit_error(&self.app, code, detail, level);
    }
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
        .disconnect_removed_profile(&state.config_mutations().current_config())
        .await
        .map_err(AppError::from)
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
