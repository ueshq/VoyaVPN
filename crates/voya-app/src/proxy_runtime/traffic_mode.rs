use super::*;
use crate::{
    config_mutation::{ConfigMutationCoordinator, ConfigMutationError},
    supervisor::{SupervisorConnectionState, SupervisorSnapshot},
};

const APPLY_BUDGET: Duration = Duration::from_secs(5);
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(1);
const RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// A committed preference must invalidate the UI even when applying it fails.
#[derive(Debug)]
pub struct TrafficModeChangeOutcome {
    pub mode: TrafficMode,
    pub config_changed: bool,
    pub runtime_result: std::result::Result<(), TrafficModeChangeError>,
}

#[derive(Debug, Error)]
pub enum TrafficModeChangeError {
    #[error("traffic mode was saved but could not be applied to the running core: {0}")]
    Apply(#[source] ProxyRuntimeError),
    #[error("traffic mode was applied, but existing connections could not be closed: {0}")]
    CloseConnections(#[source] ProxyRuntimeError),
}

impl ProxyRuntimeManager {
    /// An explicit user switch applies to existing connections too. Startup
    /// and config reload use the mode-only helpers below instead.
    pub async fn change_traffic_mode(
        &self,
        coordinator: &ConfigMutationCoordinator,
        snapshot: &SupervisorSnapshot,
        mode: TrafficMode,
    ) -> std::result::Result<TrafficModeChangeOutcome, ConfigMutationError> {
        let committed = coordinator
            .mutate(async |_unit_of_work, config| {
                config.proxy_ui_item.traffic_mode = mode;
                Ok::<_, ConfigMutationError>(())
            })
            .await?;
        let runtime_result = if snapshot.state == SupervisorConnectionState::Connected
            && mode != TrafficMode::Unchanged
        {
            self.apply_traffic_mode_change(&snapshot.clash_api_access(), mode)
                .await
        } else {
            Ok(())
        };

        Ok(TrafficModeChangeOutcome {
            mode,
            config_changed: committed.config_changed,
            runtime_result,
        })
    }

    async fn apply_traffic_mode_change(
        &self,
        access: &ClashApiAccess,
        mode: TrafficMode,
    ) -> std::result::Result<(), TrafficModeChangeError> {
        // A connected core with a missing endpoint is an error, not an
        // offline save. Do not silently skip the requested live update.
        time::timeout(ATTEMPT_TIMEOUT, self.set_traffic_mode(access, mode))
            .await
            .map_err(|_| TrafficModeChangeError::Apply(mode_timeout()))?
            .map_err(TrafficModeChangeError::Apply)?;
        let client = self
            .client(access)
            .map_err(TrafficModeChangeError::CloseConnections)?;
        time::timeout(ATTEMPT_TIMEOUT, client.close_connection(None))
            .await
            .map_err(|_| {
                TrafficModeChangeError::CloseConnections(
                    ClashError::Request("timed out closing existing connections".to_string())
                        .into(),
                )
            })?
            .map_err(|error| TrafficModeChangeError::CloseConnections(error.into()))
    }

    /// A saved preference is valid while disconnected: there is no core to call.
    /// Do not gate this on whether persistence changed; the same mode retries a
    /// previous post-commit failure against the current core.
    pub async fn set_traffic_mode_if_running(
        &self,
        access: &ClashApiAccess,
        mode: TrafficMode,
    ) -> Result<()> {
        if mode == TrafficMode::Unchanged || proxy_runtime_endpoint(access).is_none() {
            return Ok(());
        }
        time::timeout(ATTEMPT_TIMEOUT, self.set_traffic_mode(access, mode))
            .await
            .map_err(|_| mode_timeout())?
    }

    /// A new/reloaded core may not listen yet and can restore an old cached mode.
    /// Reapply the app's preference with a bounded wait before announcing ready.
    pub async fn apply_saved_traffic_mode(
        &self,
        access: &ClashApiAccess,
        mode: TrafficMode,
    ) -> Result<()> {
        time::timeout(APPLY_BUDGET, async {
            loop {
                match self.set_traffic_mode_if_running(access, mode).await {
                    Ok(()) => return Ok(()),
                    Err(error) => {
                        tracing::debug!(%error, "waiting to apply saved traffic mode");
                        time::sleep(RETRY_INTERVAL).await;
                    }
                }
            }
        })
        .await
        .map_err(|_| mode_timeout())?
    }
}

fn mode_timeout() -> ProxyRuntimeError {
    ClashError::Request("timed out applying traffic mode to the running core".to_string()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{future::Future, pin::Pin, sync::RwLock};
    use voya_core::AppConfig;
    use voya_db::Database;
    use voya_net::clash::{ClashHttpMethod, ClashHttpRequest};

    #[derive(Clone, Default)]
    struct ModeTransport(Arc<Mutex<ModeState>>);

    #[derive(Default)]
    struct ModeState {
        requests: Vec<ClashHttpRequest>,
        mode: String,
        fail_attempts: usize,
        hang: bool,
        fail_close: bool,
        hang_close: bool,
        coordinator: Option<Arc<ConfigMutationCoordinator>>,
        saved_modes_at_request: Vec<TrafficMode>,
    }

    impl ClashHttpTransport for ModeTransport {
        fn send<'a>(
            &'a self,
            request: ClashHttpRequest,
        ) -> Pin<Box<dyn Future<Output = voya_net::clash::Result<String>> + Send + 'a>> {
            let (hang, fail) = {
                let mut state = self.0.lock().expect("state");
                state.requests.push(request.clone());
                if let Some(coordinator) = &state.coordinator {
                    let saved = coordinator.current_config().proxy_ui_item.traffic_mode;
                    state.saved_modes_at_request.push(saved);
                }
                let close = request.method == ClashHttpMethod::Delete;
                let fail = state.fail_attempts > 0 || (close && state.fail_close);
                let hang = state.hang || (close && state.hang_close);
                state.fail_attempts = state.fail_attempts.saturating_sub(1);
                if !fail && !hang {
                    if request.method == ClashHttpMethod::Patch {
                        state.mode = request.body.as_ref().expect("mode body")["mode"]
                            .as_str()
                            .expect("mode")
                            .to_string();
                    } else if request.url.ends_with("/configs?force=true") {
                        state.mode = "direct".to_string(); // Simulate an old cache on reload.
                    }
                }
                (hang, fail)
            };
            Box::pin(async move {
                if hang {
                    std::future::pending::<()>().await;
                }
                if fail {
                    Err(ClashError::Request("not listening yet".into()))
                } else {
                    Ok(String::new())
                }
            })
        }
    }

    fn access() -> ClashApiAccess {
        ClashApiAccess::new(
            Some(19371),
            Some(crate::supervisor::ClashApiSecret::generate()),
        )
    }

    fn snapshot(connected: bool) -> SupervisorSnapshot {
        SupervisorSnapshot {
            connected_duration_ms: None,
            state: if connected {
                SupervisorConnectionState::Connected
            } else {
                SupervisorConnectionState::Disconnected
            },
            active_tun_backend: None,
            active_profile_id: None,
            active_group_id: None,
            main_pid: None,
            pre_pid: None,
            clash_api_port: connected.then_some(19371),
            clash_api_secret: access().secret,
        }
    }

    async fn coordinator(config: AppConfig) -> (Database, Arc<ConfigMutationCoordinator>) {
        let database = Database::connect_in_memory().await.expect("database");
        let coordinator = Arc::new(ConfigMutationCoordinator::new(
            database.clone(),
            Arc::new(RwLock::new(config)),
        ));
        (database, coordinator)
    }

    #[tokio::test]
    async fn explicit_modes_only_save_while_offline_and_preserve_proxy_and_tun_preferences() {
        for proxy in [
            voya_core::SysProxyType::Unchanged,
            voya_core::SysProxyType::ForcedChange,
        ] {
            for tun in [false, true] {
                let mut config = AppConfig::default();
                config.system_proxy_item.sys_proxy_type = proxy;
                config.tun_mode_item.enable_tun = tun;
                let (database, coordinator) = coordinator(config.clone()).await;
                let transport = ModeTransport::default();
                let manager = ProxyRuntimeManager::with_transport(Arc::new(transport.clone()));
                for mode in [
                    TrafficMode::Global,
                    TrafficMode::Rule,
                    TrafficMode::Unchanged,
                ] {
                    let outcome = manager
                        .change_traffic_mode(&coordinator, &snapshot(false), mode)
                        .await
                        .expect("save");
                    outcome.runtime_result.expect("offline has no runtime work");
                    config.proxy_ui_item.traffic_mode = mode;
                    assert_eq!(coordinator.current_config(), config);
                    assert_eq!(
                        database
                            .settings()
                            .load()
                            .await
                            .expect("saved settings")
                            .proxy
                            .traffic_mode,
                        crate::contract_map::traffic_mode_to_contract(mode)
                    );
                }
                assert!(transport.0.lock().expect("transport").requests.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn explicit_switch_persists_before_applying_and_closing_connections_even_for_the_same_mode(
    ) {
        let (_, coordinator) = coordinator(AppConfig::default()).await;
        let transport = ModeTransport::default();
        transport.0.lock().expect("transport").coordinator = Some(coordinator.clone());
        let manager = ProxyRuntimeManager::with_transport(Arc::new(transport.clone()));
        let snapshot = snapshot(true);
        for expected_changed in [true, false] {
            let outcome = manager
                .change_traffic_mode(&coordinator, &snapshot, TrafficMode::Global)
                .await
                .expect("persist");
            outcome.runtime_result.expect("live mode switch");
            assert_eq!(outcome.config_changed, expected_changed);
        }
        let state = transport.0.lock().expect("transport");
        assert_eq!(
            state
                .requests
                .iter()
                .map(|request| request.method)
                .collect::<Vec<_>>(),
            [
                ClashHttpMethod::Patch,
                ClashHttpMethod::Delete,
                ClashHttpMethod::Patch,
                ClashHttpMethod::Delete
            ]
        );
        assert_eq!(state.saved_modes_at_request, [TrafficMode::Global; 4]);
        assert_eq!(state.requests[1].url, "http://127.0.0.1:19371/connections");
        assert_eq!(
            state.requests[1].bearer_token,
            state.requests[0].bearer_token
        );
    }

    #[tokio::test]
    async fn failed_live_steps_keep_the_saved_preference_and_can_be_retried() {
        for close_failure in [false, true] {
            let (database, coordinator) = coordinator(AppConfig::default()).await;
            let transport = ModeTransport::default();
            {
                let mut state = transport.0.lock().expect("transport");
                state.fail_attempts = usize::from(!close_failure);
                state.fail_close = close_failure;
            }
            let manager = ProxyRuntimeManager::with_transport(Arc::new(transport.clone()));
            let result = manager
                .change_traffic_mode(&coordinator, &snapshot(true), TrafficMode::Global)
                .await
                .expect("preference committed");
            let error = result.runtime_result.expect_err("live step failed");
            assert_eq!(
                matches!(error, TrafficModeChangeError::CloseConnections(_)),
                close_failure
            );
            assert_eq!(
                database
                    .settings()
                    .load()
                    .await
                    .expect("saved")
                    .proxy
                    .traffic_mode,
                voya_contracts::TrafficMode::Global
            );
            {
                let mut state = transport.0.lock().expect("transport");
                assert_eq!(state.requests.len(), if close_failure { 2 } else { 1 });
                assert_eq!(state.mode == "global", close_failure);
                state.fail_close = false;
            }
            let retry = manager
                .change_traffic_mode(&coordinator, &snapshot(true), TrafficMode::Global)
                .await
                .expect("retry saved mode");
            assert!(!retry.config_changed);
            retry.runtime_result.expect("retry applies and closes");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn explicit_switch_times_out_each_live_step_and_identifies_the_failed_stage() {
        for close_timeout in [false, true] {
            let transport = ModeTransport::default();
            {
                let mut state = transport.0.lock().expect("transport");
                state.hang = !close_timeout;
                state.hang_close = close_timeout;
            }
            let start = time::Instant::now();
            let error = ProxyRuntimeManager::with_transport(Arc::new(transport.clone()))
                .apply_traffic_mode_change(&access(), TrafficMode::Global)
                .await
                .expect_err("bounded request");
            assert_eq!(time::Instant::now() - start, ATTEMPT_TIMEOUT);
            assert_eq!(
                matches!(error, TrafficModeChangeError::CloseConnections(_)),
                close_timeout
            );
            assert_eq!(
                transport.0.lock().expect("transport").requests.len(),
                if close_timeout { 2 } else { 1 }
            );
        }
    }

    #[tokio::test]
    async fn failed_persistence_never_changes_the_live_mode_or_closes_connections() {
        let (database, coordinator) = coordinator(AppConfig::default()).await;
        sqlx::query("CREATE TRIGGER reject_settings BEFORE INSERT ON app_settings BEGIN SELECT RAISE(ABORT, 'blocked'); END")
            .execute(database.pool()).await.expect("failure trigger");
        let transport = ModeTransport::default();
        assert!(
            ProxyRuntimeManager::with_transport(Arc::new(transport.clone()))
                .change_traffic_mode(&coordinator, &snapshot(true), TrafficMode::Global)
                .await
                .is_err()
        );
        assert_eq!(
            coordinator.current_config().proxy_ui_item.traffic_mode,
            TrafficMode::Rule
        );
        assert!(transport.0.lock().expect("transport").requests.is_empty());
    }

    #[tokio::test]
    async fn connected_core_without_an_api_reports_failure_instead_of_an_offline_save() {
        let (_, coordinator) = coordinator(AppConfig::default()).await;
        let mut snapshot = snapshot(true);
        snapshot.clash_api_port = None;
        let outcome = ProxyRuntimeManager::with_transport(Arc::new(ModeTransport::default()))
            .change_traffic_mode(&coordinator, &snapshot, TrafficMode::Global)
            .await
            .expect("save");
        assert!(matches!(
            outcome.runtime_result,
            Err(TrafficModeChangeError::Apply(
                ProxyRuntimeError::InvalidStatePort
            ))
        ));
    }

    #[tokio::test]
    async fn disconnected_saves_and_unchanged_preferences_do_not_call_the_core() {
        let transport = ModeTransport::default();
        let manager = ProxyRuntimeManager::with_transport(Arc::new(transport.clone()));
        manager
            .set_traffic_mode_if_running(&ClashApiAccess::default(), TrafficMode::Global)
            .await
            .expect("offline save");
        manager
            .apply_saved_traffic_mode(&access(), TrafficMode::Unchanged)
            .await
            .expect("unchanged");
        assert!(transport.0.lock().expect("state").requests.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn startup_overrides_cached_mode_after_api_becomes_ready() {
        let transport = ModeTransport::default();
        {
            let mut state = transport.0.lock().expect("state");
            state.mode = "direct".into();
            state.fail_attempts = 2;
        }
        let access = access();
        ProxyRuntimeManager::with_transport(Arc::new(transport.clone()))
            .apply_saved_traffic_mode(&access, TrafficMode::Global)
            .await
            .expect("apply saved mode");
        let state = transport.0.lock().expect("state");
        assert_eq!(state.mode, "global");
        assert_eq!(state.requests.len(), 3);
        assert_eq!(state.requests[0].url, "http://127.0.0.1:19371/configs");
        assert_eq!(
            state.requests[0].bearer_token.as_deref(),
            access
                .secret
                .as_ref()
                .map(crate::supervisor::ClashApiSecret::as_str)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn same_saved_mode_can_retry_a_failed_live_update() {
        let transport = ModeTransport::default();
        transport.0.lock().expect("state").fail_attempts = 1;
        let manager = ProxyRuntimeManager::with_transport(Arc::new(transport.clone()));
        let access = access();
        assert!(manager
            .set_traffic_mode_if_running(&access, TrafficMode::Global)
            .await
            .is_err());
        manager
            .set_traffic_mode_if_running(&access, TrafficMode::Global)
            .await
            .expect("retry same mode");
        assert_eq!(transport.0.lock().expect("state").mode, "global");
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_api_obeys_attempt_timeout_and_total_startup_budget() {
        let transport = ModeTransport::default();
        transport.0.lock().expect("state").hang = true;
        let start = time::Instant::now();
        let result = ProxyRuntimeManager::with_transport(Arc::new(transport.clone()))
            .apply_saved_traffic_mode(&access(), TrafficMode::Rule)
            .await;
        assert!(result.is_err());
        assert_eq!(start.elapsed(), APPLY_BUDGET);
        assert_eq!(transport.0.lock().expect("state").requests.len(), 5);
    }
}
