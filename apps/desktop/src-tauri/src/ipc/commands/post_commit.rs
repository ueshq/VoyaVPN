//! What follows a committed change: cache invalidation, the core restart it
//! may need, and the status events it changes.

pub(super) use voya_app::post_commit::{self, ConfigChange, PostCommitSink};

use super::{support::*, *};

/// Broadcasts one invalidation bundle and reports a failed emit as a notice.
///
/// The seven `emit_*_invalidation` wrappers used to differ only in the notice
/// title and the key list; both now come from `voya_app::invalidation`, which
/// returns the pair together. Emitting is best-effort by design: the change is
/// already committed when this runs, so a dead event channel becomes a warning
/// notice and never changes the command's result.
///
/// `scopes` is deduplicated and ordered so the payload is deterministic
/// regardless of how a caller assembled its list.
pub(crate) fn emit_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    (failure_code, scopes): invalidation::InvalidationBundle,
) where
    R: tauri::Runtime,
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

/// The tail every committed configuration change shares.
///
/// The choreography itself is `voya_app::post_commit::finish_config_change`,
/// shared with the mobile host; this supplies the Tauri sinks. `scopes` empty
/// means the caller already announced them and this call owes only the restart.
pub(super) async fn finish_config_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    reason: &str,
    scopes: &[InvalidationScope],
    config: &AppConfig,
    change: ConfigChange,
) where
    R: tauri::Runtime,
{
    post_commit::finish_config_change(
        &TauriPostCommitSink { app: app.clone() },
        &core_flow(app, state),
        reason,
        scopes,
        config,
        change,
    )
    .await;
}

/// The restart-only tail, for callers that already announced their caches.
pub(super) async fn restart_after_config_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
    change: ConfigChange,
) where
    R: tauri::Runtime,
{
    finish_config_change(app, state, "", &[], config, change).await;
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
    finish_config_change(
        app,
        state,
        reason,
        &invalidation::routing_scopes(committed.config_changed).1,
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
    fn invalidate(
        &self,
        reason: &str,
        scopes: &[InvalidationScope],
        refresh_failed_code: NoticeCode,
    ) {
        emit_invalidation(&self.app, reason, (refresh_failed_code, scopes.to_vec()));
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
