//! macOS latency tests while connected: measure through the running core.
//!
//! Once the PacketTunnel is up, a probe core's own connections would go through
//! the tunnel too. The runtime config therefore carries one unrouted probe
//! outbound per node (`voya_core::latency_probe_tag`), and each measurement is
//! a Clash API delay request against the PacketTunnel core. While disconnected
//! the manager launches probe cores from the packaged seed instead.

use std::ops::RangeInclusive;

use futures_util::{stream, StreamExt};
use voya_core::{is_latency_probe_candidate, latency_probe_tag, ConfigType};
use voya_net::clash::{ClashError, ClashRestClient};

use super::manager::{persist_speedtest_result_with_retry, record_item_failures};
use super::*;
use crate::proxy_runtime::proxy_runtime_endpoint;
use crate::supervisor::{CoreSupervisor, SupervisorConnectionState};

/// Delay requests in flight at once; sing-box's own group test runs ten.
const RUNNING_CORE_CONCURRENCY: usize = 8;
/// sing-box parses the delay `timeout` as a 16-bit millisecond count, and the
/// Clash transport abandons any request after 30 s.
const RUNNING_CORE_TIMEOUT_MS: RangeInclusive<u32> = 1_000..=25_000;
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Reaches the running core for a speedtest run.
pub trait RunningCoreProbe: Send + Sync {
    /// A delay client bound to the core running now; `None` while nothing is
    /// connected, which sends the run to probe cores.
    fn connect(&self) -> BoxFuture<'static, Option<Arc<dyn RunningCoreDelay>>>;
}

/// One run's view of the running core.
pub trait RunningCoreDelay: Send + Sync {
    fn delay(
        &self,
        tag: String,
        test_url: String,
        timeout_ms: u32,
    ) -> BoxFuture<'static, std::result::Result<u32, ClashError>>;
}

#[derive(Clone)]
pub struct SupervisorRunningCoreProbe {
    supervisor: CoreSupervisor,
}

impl SupervisorRunningCoreProbe {
    #[must_use]
    pub const fn new(supervisor: CoreSupervisor) -> Self {
        Self { supervisor }
    }
}

impl RunningCoreProbe for SupervisorRunningCoreProbe {
    fn connect(&self) -> BoxFuture<'static, Option<Arc<dyn RunningCoreDelay>>> {
        let supervisor = self.supervisor.clone();
        Box::pin(async move {
            let snapshot = match supervisor.status().await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    tracing::warn!(?error, "speedtest could not read the core status");
                    return None;
                }
            };
            if snapshot.state != SupervisorConnectionState::Connected {
                return None;
            }
            let endpoint = proxy_runtime_endpoint(&snapshot.clash_api_access())?;
            Some(Arc::new(ClashRunningCoreDelay {
                client: ClashRestClient::new(endpoint),
            }) as Arc<dyn RunningCoreDelay>)
        })
    }
}

struct ClashRunningCoreDelay {
    client: ClashRestClient,
}

impl RunningCoreDelay for ClashRunningCoreDelay {
    fn delay(
        &self,
        tag: String,
        test_url: String,
        timeout_ms: u32,
    ) -> BoxFuture<'static, std::result::Result<u32, ClashError>> {
        let client = self.client.clone();
        Box::pin(async move { client.proxy_delay(&tag, &test_url, timeout_ms).await })
    }
}

impl SpeedtestManager {
    pub(super) async fn run_through_running_core<F>(
        &self,
        core: Arc<dyn RunningCoreDelay>,
        database: &Database,
        config: &AppConfig,
        items: &[ServerTestItem],
        cancel: CancellationFlag,
        on_result: &F,
    ) -> Result<Vec<SpeedtestResult>>
    where
        F: Fn(SpeedtestResult) + Send + Sync,
    {
        let env = load_runtime_core_gen_env(database, &self.paths, config, self.target_os).await?;
        let builder = CoreConfigContextBuilder::new(&env);
        let mut failures = Vec::new();
        let mut measurable = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if item.profile.config_type() == ConfigType::WireGuard {
                // A second WireGuard device with the node's key would take its
                // session over from the connection (see
                // `is_latency_probe_candidate`), so these carry no probe.
                failures.push(SpeedtestItemFailure::new(
                    item.index_id.clone(),
                    SpeedtestOutcome::Skipped,
                ));
                continue;
            }
            let build = builder.build(config, &item.profile);
            if !build.success() || !is_latency_probe_candidate(&item.profile) {
                failures.push(SpeedtestItemFailure::new(
                    item.index_id.clone(),
                    SpeedtestOutcome::InvalidProfile,
                ));
                continue;
            }
            measurable.push((index, latency_probe_tag(&build.context, &item.profile)));
        }
        let mut results = record_item_failures(database, failures, items, on_result).await?;

        let test_url = latency_test_url(&config.speed_test_item).to_string();
        let timeout_ms = running_core_timeout_ms(&config.speed_test_item);
        // Indices rather than borrows: the stream's futures must not hold a
        // reference into `items`, or the run's future stops being `Send`-able
        // across the higher-ranked lifetimes `tokio::spawn` asks for.
        let mut pending = stream::iter(measurable.into_iter().map(|(index, tag)| {
            let core = Arc::clone(&core);
            let test_url = test_url.clone();
            let cancel = Arc::clone(&cancel);
            async move {
                // Queued requests that would start after a cancel are never sent.
                let measured = if is_cancelled(&cancel) {
                    None
                } else {
                    Some(core.delay(tag, test_url, timeout_ms).await)
                };
                (index, measured)
            }
        }))
        .buffer_unordered(RUNNING_CORE_CONCURRENCY);

        loop {
            let next = tokio::select! {
                next = pending.next() => next,
                () = cancelled(&cancel) => break,
            };
            let Some((index, measured)) = next else {
                break;
            };
            let (Some(item), Some(measured)) = (items.get(index), measured) else {
                continue;
            };
            let result = running_core_result(item.index_id.clone(), measured);
            if persist_speedtest_result_with_retry(database, &result, &item.profile).await? {
                on_result(result.clone());
                results.push(result);
            }
        }

        Ok(results)
    }
}

fn running_core_timeout_ms(item: &SpeedTestItem) -> u32 {
    u32::try_from(item.speed_test_timeout)
        .unwrap_or(0)
        .saturating_mul(1_000)
        .clamp(
            *RUNNING_CORE_TIMEOUT_MS.start(),
            *RUNNING_CORE_TIMEOUT_MS.end(),
        )
}

/// Maps the core's answer onto the persisted outcome vocabulary.
fn running_core_result(
    index_id: String,
    measured: std::result::Result<u32, ClashError>,
) -> SpeedtestResult {
    match measured {
        Ok(delay) => SpeedtestResult {
            index_id,
            delay: Some(i32::try_from(delay).unwrap_or(i32::MAX)),
            outcome: SpeedtestOutcome::Completed,
            detail: None,
            // The core measures the node itself; there is no local proxy port
            // for an exit-country lookup to go through.
            ip_info: None,
            country_code: None,
        },
        Err(error) => {
            tracing::warn!(index_id = %index_id, ?error, "running-core delay test failed");
            let (outcome, detail) = match error {
                // No outbound under that tag: the node, or a setting its
                // outbound depends on, changed after the core started.
                ClashError::Status(404) => (SpeedtestOutcome::ReconnectRequired, None),
                ClashError::Status(408 | 504) => (SpeedtestOutcome::TimedOut, None),
                ClashError::Status(503) => (SpeedtestOutcome::ProxyConnectFailed, None),
                ClashError::Status(_) => (SpeedtestOutcome::Failed, None),
                other => (
                    SpeedtestOutcome::Failed,
                    Some(redact_urls(&other.to_string())),
                ),
            };
            make_failure_result(index_id, outcome, detail)
        }
    }
}

async fn cancelled(cancel: &CancellationFlag) {
    while !is_cancelled(cancel) {
        time::sleep(CANCEL_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests;
