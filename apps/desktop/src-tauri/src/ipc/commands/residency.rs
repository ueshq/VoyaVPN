//! Close-prompt answers and the tray's entry points into the app.
//!
//! The tray is a native menu, so a failed action has no caller to return an
//! error to; each one reports through the notice channel instead.

use tauri::Manager;
use voya_app::services::TrafficMode;
use voya_app::tray::{TrayMenuInput, TrayNode};
use voya_contracts::CloseRequestAction;

use super::{lifecycle::*, support::*, *};

/// Carries out the user's answer to the close prompt, optionally keeping it
/// as the close action from now on.
#[tauri::command]
#[specta::specta]
pub async fn resolve_close_request<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, AppState>,
    action: CloseRequestAction,
    remember: bool,
) -> Result<(), AppError> {
    if remember {
        let committed = mutate_config(&state, async |_unit_of_work, config| {
            Ok(voya_app::residency::remember_close_action(config, action))
        })
        .await?;
        if committed.config_changed {
            emit_settings_bundle_invalidation(&app, "close-action-remembered");
        }
    }
    match action {
        CloseRequestAction::MinimizeToTray => crate::residency::hide_main_window(&app),
        CloseRequestAction::Quit => app.exit(0),
        CloseRequestAction::Cancel => {}
    }

    Ok(())
}

/// What the tray shows, read from the running app in one pass.
pub(crate) struct TraySnapshot {
    language: String,
    connected: bool,
    traffic_mode: TrafficMode,
    nodes: Vec<TrayNode>,
    active_node_id: Option<String>,
}

impl TraySnapshot {
    pub(crate) fn input(&self) -> TrayMenuInput<'_> {
        TrayMenuInput {
            language: &self.language,
            connected: self.connected,
            traffic_mode: self.traffic_mode,
            nodes: &self.nodes,
            active_node_id: self.active_node_id.as_deref(),
        }
    }

    /// The active node's name while a connection is up.
    pub(crate) fn connected_node(&self) -> Option<&str> {
        let active = self.active_node_id.as_deref().filter(|_| self.connected)?;
        self.nodes
            .iter()
            .find(|node| node.id == active)
            .map(|node| node.remarks.as_str())
    }
}

pub(crate) async fn tray_snapshot(state: &AppState) -> TraySnapshot {
    let config = current_config(state);
    let connected = matches!(
        state.supervisor().status().await,
        Ok(snapshot) if snapshot.state == SupervisorConnectionState::Connected
    );
    let (nodes, active_node_id) = match state
        .services()
        .profiles()
        .list_profiles(&config, None, None)
        .await
    {
        Ok(listing) => {
            let active = listing
                .items
                .iter()
                .find(|item| item.is_active)
                .map(|item| item.profile.index_id.clone());
            let nodes = listing
                .items
                .into_iter()
                .map(|item| TrayNode {
                    id: item.profile.index_id,
                    remarks: item.profile.remarks,
                })
                .collect();
            (nodes, active)
        }
        Err(error) => {
            tracing::warn!(?error, "failed to list nodes for the tray menu");
            (Vec::new(), None)
        }
    };

    TraySnapshot {
        language: config.ui_item.current_language.clone(),
        connected,
        traffic_mode: config.proxy_ui_item.traffic_mode,
        nodes,
        active_node_id,
    }
}

pub(crate) async fn tray_connect<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let config = current_config(&state);
    if let Err(error) = core_flow(app, &state).connect(&config).await {
        report_tray_failure(app, &AppError::from(error));
    }
}

pub(crate) async fn tray_disconnect<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let config = current_config(&state);
    if let Err(error) = core_flow(app, &state).disconnect(&config).await {
        report_tray_failure(app, &AppError::from(error));
    }
}

pub(crate) async fn tray_set_traffic_mode<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    mode: TrafficMode,
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Err(error) = apply_traffic_mode(app, &state, traffic_mode_to_contract(mode)).await {
        report_tray_failure(app, &error);
    }
}

/// Makes a node active and, when connected, restarts onto it: the same two
/// steps the node list takes.
pub(crate) async fn tray_activate_node<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    index_id: String,
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let activated = mutate_config(&state, async |unit_of_work, config| {
        Ok(ProfileManager::new_in(unit_of_work)
            .set_active_profile(config, &index_id)
            .await?)
    })
    .await;
    match activated {
        Ok(committed) => {
            emit_profile_invalidation(app, "active-profile-changed", true);
            restart_after_config_change(
                app,
                &state,
                &committed.config,
                ConfigChange::ACTIVE_PROFILE,
            )
            .await;
        }
        Err(error) => report_tray_failure(app, &error),
    }
}

fn report_tray_failure<R: tauri::Runtime>(app: &tauri::AppHandle<R>, error: &AppError) {
    report_post_commit_error(
        app,
        NoticeCode::TrayActionFailed,
        &format!("{error:?}"),
        AppNoticeLevel::Warning,
    );
}
