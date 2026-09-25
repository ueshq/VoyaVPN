//! Close-prompt answers and the tray's entry points into the app.
//!
//! The tray is a native menu, so a failed action has no caller to return an
//! error to; each one reports through the notice channel instead.

use tauri::Manager;
use voya_app::tray::TrafficMode;
use voya_app::tray::{TrayGroup, TrayMenuInput, TrayNode, TRAY_NODE_LIMIT};
use voya_contracts::{AppErrorKind, CloseRequestAction};

use super::{post_commit::*, support::*, *};

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
        let committed = state
            .config_mutations()
            .mutate(async |_unit_of_work, config| -> Result<_, AppError> {
                Ok(voya_app::lifecycle::remember_close_action(config, action))
            })
            .await?;
        if committed.config_changed {
            emit_invalidation(
                &app,
                "close-action-remembered",
                invalidation::settings_bundle_scopes(),
            );
        }
    }
    match action {
        CloseRequestAction::MinimizeToTray => crate::residency::hide_into_tray(&app),
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
    total_nodes: usize,
    active_node_id: Option<String>,
    groups: Vec<TrayGroup>,
    active_group_id: Option<String>,
}

impl TraySnapshot {
    pub(crate) fn input(&self, window_visible: bool) -> TrayMenuInput<'_> {
        TrayMenuInput {
            language: &self.language,
            connected: self.connected,
            traffic_mode: self.traffic_mode,
            nodes: &self.nodes,
            total_nodes: self.total_nodes,
            active_node_id: self.active_node_id.as_deref(),
            groups: &self.groups,
            active_group_id: self.active_group_id.as_deref(),
            window_visible,
        }
    }

    pub(crate) fn connected(&self) -> bool {
        self.connected
    }

    /// The traffic mode's tray label while connected, for the tooltip.
    pub(crate) fn traffic_mode_label(&self) -> Option<&'static str> {
        self.connected
            .then(|| voya_app::tray::traffic_mode_label(&self.language, self.traffic_mode))
            .flatten()
    }

    /// The active group's or node's name while a connection is up.
    pub(crate) fn connected_node(&self) -> Option<&str> {
        if !self.connected {
            return None;
        }
        if let Some(group) = self.active_group_id.as_deref() {
            return self
                .groups
                .iter()
                .find(|item| item.id == group)
                .map(|item| item.name.as_str());
        }
        let active = self.active_node_id.as_deref()?;
        self.nodes
            .iter()
            .find(|node| node.id == active)
            .map(|node| node.remarks.as_str())
    }
}

/// The tray before anything is read from the database: no nodes or groups
/// yet. Setup builds the menu from this so the window does not wait on two
/// table reads; the queued refresh fills it in.
pub(crate) fn initial_tray_snapshot(state: &AppState) -> TraySnapshot {
    let config = state.config_mutations().current_config();
    TraySnapshot {
        language: config.appearance.language.clone(),
        connected: false,
        traffic_mode: config.proxy.traffic_mode,
        nodes: Vec::new(),
        total_nodes: 0,
        active_node_id: None,
        groups: Vec::new(),
        active_group_id: None,
    }
}

pub(crate) async fn tray_snapshot(state: &AppState) -> TraySnapshot {
    let config = state.config_mutations().current_config();
    let connected = matches!(
        state.supervisor().status().await,
        Ok(snapshot) if snapshot.state == SupervisorConnectionState::Connected
    );
    let pinned = Some(config.index_id.as_str()).filter(|id| !id.is_empty());
    let (nodes, total_nodes, active_node_id) = match state
        .services()
        .list_profile_names_head(TRAY_NODE_LIMIT, pinned)
        .await
    {
        Ok(head) => {
            let active = pinned
                .filter(|active| head.names.iter().any(|(id, _)| id == active))
                .map(str::to_string);
            let nodes = head
                .names
                .into_iter()
                .map(|(id, remarks)| TrayNode { id, remarks })
                .collect();
            (nodes, head.total, active)
        }
        Err(error) => {
            tracing::warn!(?error, "failed to list nodes for the tray menu");
            (Vec::new(), 0, None)
        }
    };

    let (groups, active_group_id) = match state.services().list_policy_groups().await {
        Ok(groups) => {
            let active = groups
                .iter()
                .find(|group| group.id == config.active_group_id)
                .map(|group| group.id.clone());
            let groups = groups
                .into_iter()
                .map(|group| TrayGroup {
                    id: group.id,
                    name: group.name,
                })
                .collect();
            (groups, active)
        }
        Err(error) => {
            tracing::warn!(?error, "failed to list policy groups for the tray menu");
            (Vec::new(), None)
        }
    };

    TraySnapshot {
        language: config.appearance.language.clone(),
        connected,
        traffic_mode: config.proxy.traffic_mode,
        nodes,
        total_nodes,
        active_node_id,
        groups,
        active_group_id,
    }
}

pub(crate) async fn tray_connect<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let config = state.config_mutations().current_config();
    let mut result = core_flow(app, &state)
        .connect(&config)
        .await
        .map_err(AppError::from);
    // The window asks for one-time authorization and tries again; so does the tray.
    if matches!(&result, Err(error) if error.kind == AppErrorKind::ElevationRequired) {
        let elevation = state.elevation_manager().clone();
        result = match run_blocking("elevation request", move || elevation.request()).await {
            Ok(Ok(())) => core_flow(app, &state)
                .connect(&config)
                .await
                .map_err(AppError::from),
            Ok(Err(error)) => Err(AppError::from(error)),
            Err(error) => Err(error),
        };
    }
    if let Err(error) = result {
        report_tray_failure(app, &error);
    }
}

pub(crate) async fn tray_disconnect<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let config = state.config_mutations().current_config();
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
    let activated = state
        .config_mutations()
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(ProfileManager::new_in(unit_of_work)
                .set_active_profile(config, &index_id)
                .await?)
        })
        .await;
    tray_commit(
        app,
        &state,
        activated,
        |app| {
            emit_invalidation(
                app,
                "active-profile-changed",
                invalidation::profile_scopes(true),
            )
        },
        ConfigChange::ACTIVE_PROFILE,
    )
    .await;
}

/// Makes a policy group active and, when connected, restarts onto it: the
/// same two steps the policy group card takes.
pub(crate) async fn tray_activate_group<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    group_id: String,
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let activated = state
        .config_mutations()
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(
                voya_app::policy_groups::PolicyGroupManager::new_in(unit_of_work)
                    .set_active(config, &group_id)
                    .await?,
            )
        })
        .await;
    tray_commit(
        app,
        &state,
        activated,
        |app| {
            emit_invalidation(
                app,
                "active-policy-group-changed",
                invalidation::policy_group_scopes(true),
            )
        },
        ConfigChange::POLICY_GROUP,
    )
    .await;
}

/// A tray mutation's post-commit tail: on success the same emit-then-restart
/// the matching command runs, on failure a notice — a tray click has no
/// caller to return an error to.
async fn tray_commit<R, F, T>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    activated: Result<CommittedMutation<T>, AppError>,
    emit: F,
    change: ConfigChange,
) where
    R: tauri::Runtime,
    F: FnOnce(&tauri::AppHandle<R>),
{
    match activated {
        Ok(committed) => {
            emit(app);
            restart_after_config_change(app, state, &committed.config, change).await;
        }
        Err(error) => report_tray_failure(app, &error),
    }
}

fn report_tray_failure<R: tauri::Runtime>(app: &tauri::AppHandle<R>, error: &AppError) {
    // The window may be hidden in the tray; bring it forward so the notice is seen.
    crate::residency::show_main_window(app);
    report_post_commit_error(
        app,
        NoticeCode::TrayActionFailed,
        &error.message,
        AppNoticeLevel::Warning,
    );
}
