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

/// Names one committed configuration change for the restart that follows it.
///
/// Every mutating command used to spell both strings out inline around an
/// identical eight-line `if let Err(..) { report_post_commit_error(..) }`
/// block; the label was the only thing that varied.
#[derive(Clone, Copy)]
pub(super) struct ConfigChange {
    /// Reason recorded by the core flow's log line.
    reason: &'static str,
    /// Notice title used when the follow-up restart fails.
    restart_failed_title: &'static str,
}

impl ConfigChange {
    /// Every routing mutation shares the same core-flow reason and only differs
    /// in which operation the failure notice names.
    const fn routing(restart_failed_title: &'static str) -> Self {
        Self {
            reason: "Routing changed",
            restart_failed_title,
        }
    }

    pub(super) const ROUTING_SAVED: Self = Self::routing("Routing saved; core restart failed");
    pub(super) const ROUTING_DELETED: Self = Self::routing("Routing deleted; core restart failed");
    pub(super) const ROUTING_SELECTED: Self =
        Self::routing("Routing selected; core restart failed");
    pub(super) const ROUTING_RULE_SAVED: Self =
        Self::routing("Routing rule saved; core restart failed");
    pub(super) const ROUTING_RULES_DELETED: Self =
        Self::routing("Routing rules deleted; core restart failed");
    pub(super) const ROUTING_RULE_MOVED: Self =
        Self::routing("Routing rule moved; core restart failed");
    pub(super) const DNS: Self = Self {
        reason: "DNS changed",
        restart_failed_title: "DNS saved; core restart failed",
    };
    pub(super) const CONFIG_TEMPLATE: Self = Self {
        reason: "Config template imported",
        restart_failed_title: "Template imported; core restart failed",
    };
    pub(super) const TUN: Self = Self {
        reason: "TUN changed",
        restart_failed_title: "TUN saved; core restart failed",
    };
    pub(super) const CONNECTION_MODE: Self = Self {
        reason: "Connection mode changed",
        restart_failed_title: "Connection mode saved; core restart failed",
    };
    pub(super) const APP_SETTINGS: Self = Self {
        reason: "Settings saved",
        restart_failed_title: "Settings saved; runtime update failed",
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
        .map_err(runtime_error)
    {
        report_post_commit_error(
            app,
            change.restart_failed_title,
            &format!("{error:?}"),
            AppNoticeLevel::Warning,
        );
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
