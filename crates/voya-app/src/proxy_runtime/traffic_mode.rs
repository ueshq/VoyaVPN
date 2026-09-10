use super::*;

const APPLY_BUDGET: Duration = Duration::from_secs(5);
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(1);
const RETRY_INTERVAL: Duration = Duration::from_millis(100);

impl<T: ClashHttpTransport> ProxyRuntimeManager<T> {
    /// Reload errors reject the operation; a mode failure is post-commit and is
    /// returned separately so the shell can report a warning without lying
    /// about whether the reload succeeded.
    pub async fn reload_config_with_mode(
        &self,
        access: &ClashApiAccess,
        path: Option<&str>,
        mode: TrafficMode,
    ) -> Result<Option<ProxyRuntimeError>> {
        self.reload_config(access, path).await?;
        Ok(self.apply_saved_traffic_mode(access, mode).await.err())
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
    use serde_json::{json, Value};
    use std::{future::Future, pin::Pin};
    use voya_net::clash::{ClashHttpMethod, ClashHttpRequest};

    #[derive(Clone, Default)]
    struct ModeTransport(Arc<Mutex<ModeState>>);

    #[derive(Default)]
    struct ModeState {
        requests: Vec<ClashHttpRequest>,
        mode: String,
        fail_attempts: usize,
        hang: bool,
    }

    impl ClashHttpTransport for ModeTransport {
        fn send_json<'a>(
            &'a self,
            request: ClashHttpRequest,
        ) -> Pin<Box<dyn Future<Output = voya_net::clash::Result<Value>> + Send + 'a>> {
            let (hang, fail) = {
                let mut state = self.0.lock().expect("state");
                state.requests.push(request.clone());
                let fail = state.fail_attempts > 0;
                state.fail_attempts = state.fail_attempts.saturating_sub(1);
                if !fail && !state.hang {
                    if request.method == ClashHttpMethod::Patch {
                        state.mode = request.body.as_ref().expect("mode body")["mode"]
                            .as_str()
                            .expect("mode")
                            .to_string();
                    } else if request.url.ends_with("/configs?force=true") {
                        state.mode = "direct".to_string(); // Simulate an old cache on reload.
                    }
                }
                (state.hang, fail)
            };
            Box::pin(async move {
                if hang {
                    std::future::pending::<()>().await;
                }
                if fail {
                    Err(ClashError::Request("not listening yet".into()))
                } else {
                    Ok(Value::Null)
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

    #[tokio::test]
    async fn disconnected_saves_and_unchanged_preferences_do_not_call_the_core() {
        let transport = ModeTransport::default();
        let manager = ProxyRuntimeManager::with_transport(transport.clone());
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
        ProxyRuntimeManager::with_transport(transport.clone())
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
        let manager = ProxyRuntimeManager::with_transport(transport.clone());
        let access = access();
        assert!(manager
            .set_traffic_mode_if_running(&access, TrafficMode::Direct)
            .await
            .is_err());
        manager
            .set_traffic_mode_if_running(&access, TrafficMode::Direct)
            .await
            .expect("retry same mode");
        assert_eq!(transport.0.lock().expect("state").mode, "direct");
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_api_obeys_attempt_timeout_and_total_startup_budget() {
        let transport = ModeTransport::default();
        transport.0.lock().expect("state").hang = true;
        let start = time::Instant::now();
        let result = ProxyRuntimeManager::with_transport(transport.clone())
            .apply_saved_traffic_mode(&access(), TrafficMode::Rule)
            .await;
        assert!(result.is_err());
        assert_eq!(start.elapsed(), APPLY_BUDGET);
        assert_eq!(transport.0.lock().expect("state").requests.len(), 5);
    }

    #[tokio::test]
    async fn reload_reapplies_saved_mode_after_restoring_old_cache() {
        let transport = ModeTransport::default();
        let result = ProxyRuntimeManager::with_transport(transport.clone())
            .reload_config_with_mode(&access(), None, TrafficMode::Global)
            .await
            .expect("reload");
        assert!(result.is_none());
        let state = transport.0.lock().expect("state");
        assert_eq!(state.mode, "global");
        assert_eq!(
            state.requests.last().expect("last request").body,
            Some(json!({"mode":"global"}))
        );
    }
}
