use super::{support::*, *};

pub(super) fn emit_profile_invalidation<R, I>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    affected_index_ids: I,
    active_changed: bool,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
    I: IntoIterator<Item = String>,
{
    let mut keys = BTreeSet::new();
    keys.insert(vec!["profiles".to_string()]);
    keys.insert(vec!["profile-ex".to_string()]);
    if active_changed {
        keys.insert(vec!["active-profile".to_string()]);
    }
    for index_id in affected_index_ids {
        if !index_id.is_empty() {
            keys.insert(vec!["profile".to_string(), index_id]);
        }
    }

    if let Err(error) = (InvalidateEvent {
        keys: keys
            .into_iter()
            .map(|query_key| QueryInvalidation {
                query_key,
                reason: reason.to_string(),
            })
            .collect(),
    })
    .emit(app)
    {
        report_post_commit_error(
            app,
            "Profile refresh failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
    Ok(())
}

pub(super) fn emit_subscription_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    profiles_changed: bool,
    config_changed: bool,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    let mut keys = BTreeSet::new();
    keys.insert(vec!["subscriptions".to_string()]);
    keys.insert(vec!["subscription-metadata".to_string()]);
    if profiles_changed {
        keys.insert(vec!["profiles".to_string()]);
        keys.insert(vec!["profile-ex".to_string()]);
    }
    if config_changed {
        keys.insert(vec!["active-profile".to_string()]);
    }

    if let Err(error) = (InvalidateEvent {
        keys: keys
            .into_iter()
            .map(|query_key| QueryInvalidation {
                query_key,
                reason: reason.to_string(),
            })
            .collect(),
    })
    .emit(app)
    {
        report_post_commit_error(
            app,
            "Subscription refresh failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
    Ok(())
}

pub(super) fn emit_routing_invalidation<R, I>(
    app: &tauri::AppHandle<R>,
    reason: &str,
    affected_ids: I,
    active_changed: bool,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
    I: IntoIterator<Item = String>,
{
    let mut keys = BTreeSet::new();
    keys.insert(vec!["routings".to_string()]);
    if active_changed {
        keys.insert(vec!["active-routing".to_string()]);
    }
    for id in affected_ids {
        if !id.is_empty() {
            keys.insert(vec!["routing".to_string(), id]);
        }
    }

    if let Err(error) = (InvalidateEvent {
        keys: keys
            .into_iter()
            .map(|query_key| QueryInvalidation {
                query_key,
                reason: reason.to_string(),
            })
            .collect(),
    })
    .emit(app)
    {
        report_post_commit_error(
            app,
            "Routing refresh failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
    Ok(())
}

pub(super) fn emit_dns_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    if let Err(error) = (InvalidateEvent {
        keys: [
            vec!["dns".to_string()],
            vec!["app-config".to_string()],
            vec!["active-dns".to_string()],
        ]
        .into_iter()
        .map(|query_key| QueryInvalidation {
            query_key,
            reason: reason.to_string(),
        })
        .collect(),
    })
    .emit(app)
    {
        report_post_commit_error(
            app,
            "DNS refresh failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
    Ok(())
}

pub(super) fn emit_preset_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    if let Err(error) = (InvalidateEvent {
        keys: [
            vec!["dns".to_string()],
            vec!["app-config".to_string()],
            vec!["active-dns".to_string()],
            vec!["routings".to_string()],
            vec!["active-routing".to_string()],
        ]
        .into_iter()
        .map(|query_key| QueryInvalidation {
            query_key,
            reason: reason.to_string(),
        })
        .collect(),
    })
    .emit(app)
    {
        report_post_commit_error(
            app,
            "Configuration refresh failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
    Ok(())
}

pub(super) fn emit_proxy_runtime_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    if let Err(error) = (InvalidateEvent {
        keys: [
            vec!["proxy-groups".to_string()],
            vec!["proxy-connections".to_string()],
            vec!["app-config".to_string()],
        ]
        .into_iter()
        .map(|query_key| QueryInvalidation {
            query_key,
            reason: reason.to_string(),
        })
        .collect(),
    })
    .emit(app)
    {
        report_post_commit_error(
            app,
            "Proxy view refresh failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
    Ok(())
}

pub(super) fn emit_proxy_monitor_status<R>(app: &tauri::AppHandle<R>, status: &ProxyMonitorStatus)
where
    R: tauri::Runtime,
{
    if let Err(error) = TransientStreamEvent::ProxyMonitorStatus(status.clone()).emit(app) {
        tracing::warn!(?error, ?status, "failed to emit proxy monitor status event");
    }
}

pub(crate) fn emit_tun_changed<R>(
    app: &tauri::AppHandle<R>,
    status: &TunStatus,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::TunChanged(crate::ipc::events::TunChanged {
        enabled: status.enabled,
        backend: status.backend,
        provider_state: status.provider_state,
        native_component_ready: status.native_component_ready,
        last_provider_error: status.last_provider_error.clone(),
    })
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

pub(crate) fn emit_current_tun_status<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    let config = current_config(state)?;
    let status = tun_manager(state).status(&config).map_err(tun_error)?;
    emit_tun_changed(app, &status)
}

pub(super) async fn restart_if_connected_after_routing_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    restart_if_connected_after_config_change(app, state, config, "Routing changed").await
}

pub(super) async fn restart_if_connected_after_config_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
    reason: &str,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    let status = runtime_manager(state)
        .status()
        .await
        .map_err(runtime_error)?;
    if status.state != SupervisorConnectionState::Connected {
        return Ok(());
    }

    if let Err(error) = emit_runtime_log(app, LogLevel::Info, &format!("{reason}; restarting core"))
    {
        tracing::warn!(?error, "failed to emit core restart log");
    }
    if let Err(error) = emit_core_state(
        app,
        CoreState::Connecting,
        Some(config.index_id.clone()).filter(|value| !value.is_empty()),
        None,
    ) {
        tracing::warn!(?error, "failed to emit connecting state");
    }

    match runtime_manager(state).restart(config).await {
        Ok(snapshot) => {
            if let Err(error) = emit_runtime_log(
                app,
                LogLevel::Info,
                &format!("Core supervisor restarted after {reason}"),
            ) {
                tracing::warn!(?error, "failed to emit core restart success log");
            }
            if let Err(error) = emit_core_state(app, CoreState::Connected, None, Some(&snapshot)) {
                tracing::warn!(?error, "failed to emit connected state");
            }
            match apply_system_proxy(app, state, config, false) {
                Ok(status) => {
                    if let Err(error) = emit_sysproxy_changed(app, &status) {
                        tracing::warn!(?error, "failed to emit system proxy state");
                    }
                }
                Err(error) => report_post_commit_error(
                    app,
                    "Core restarted; system proxy update failed",
                    &error.to_string(),
                    AppNoticeLevel::Warning,
                ),
            }
            if let Err(error) = emit_current_tun_status(app, state) {
                tracing::warn!(?error, "failed to emit current TUN status");
            }
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            if let Err(emit_error) = emit_runtime_log(app, LogLevel::Error, &message) {
                tracing::warn!(?emit_error, "failed to emit core restart error log");
            }
            if let Err(emit_error) = emit_core_state(app, CoreState::Disconnected, None, None) {
                tracing::warn!(?emit_error, "failed to emit disconnected state");
            }
            if let Err(restore_error) = restore_system_proxy_after_native_tun_failure(
                app,
                state,
                config,
                "config-change restart failure",
            ) {
                report_post_commit_error(
                    app,
                    "Core restart and system proxy recovery failed",
                    &format!("{restore_error:?}"),
                    AppNoticeLevel::Error,
                );
            }
            Err(runtime_error(error))
        }
    }
}

pub(super) async fn apply_system_proxy_if_connected_after_config_change<R>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    config: &AppConfig,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    let status = runtime_manager(state)
        .status()
        .await
        .map_err(runtime_error)?;
    if status.state != SupervisorConnectionState::Connected {
        return Ok(());
    }

    match apply_system_proxy(app, state, config, false) {
        Ok(status) => {
            if let Err(error) = emit_sysproxy_changed(app, &status) {
                tracing::warn!(?error, "failed to emit system proxy state");
            }
        }
        Err(error) => report_post_commit_error(
            app,
            "Settings saved; system proxy update failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        ),
    }
    Ok(())
}

pub(super) fn emit_settings_bundle_invalidation<R>(
    app: &tauri::AppHandle<R>,
    reason: &str,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    if let Err(error) = (InvalidateEvent {
        keys: ["app-config", "ui-preferences", "config-sources"]
            .into_iter()
            .map(|key| QueryInvalidation {
                query_key: vec![key.to_string()],
                reason: reason.to_string(),
            })
            .collect(),
    })
    .emit(app)
    {
        report_post_commit_error(
            app,
            "Settings refresh failed",
            &error.to_string(),
            AppNoticeLevel::Warning,
        );
    }
    Ok(())
}

pub(super) fn emit_sysproxy_changed<R>(
    app: &tauri::AppHandle<R>,
    status: &SystemProxyStatus,
) -> Result<(), AppError>
where
    R: tauri::Runtime,
{
    TransientStreamEvent::SysProxyChanged(crate::ipc::events::SysProxyChanged {
        requested_mode: sysproxy_mode(status.requested_type),
        effective_mode: sysproxy_mode(status.effective_type),
        pac_available: status.pac_available,
        proxy: status.proxy.clone(),
    })
    .emit(app)
    .map_err(|error| AppError::EventEmit(error.to_string()))
}

pub(super) fn sysproxy_mode(mode: SysProxyType) -> crate::ipc::events::SysProxyMode {
    match mode {
        SysProxyType::ForcedClear => crate::ipc::events::SysProxyMode::ForcedClear,
        SysProxyType::ForcedChange => crate::ipc::events::SysProxyMode::ForcedChange,
        SysProxyType::Unchanged => crate::ipc::events::SysProxyMode::Unchanged,
        SysProxyType::Pac => crate::ipc::events::SysProxyMode::Pac,
    }
}
