//! One connect/restart/disconnect/recovery choreography for the core.
//!
//! The Tauri shell used to own four copies of this sequence — `connect`,
//! `restart`, the config-change restart, and the native-TUN provider-exit
//! recovery — and they had drifted apart: only one of them restored the system
//! proxy, only one of them reported the supervisor's real state after a
//! failure, and two of them turned an event-emission failure into a command
//! failure. Since the shell's lib test harness is disabled, none of it could be
//! tested either.
//!
//! The rules this module enforces, once, for every entry point:
//!
//! * On success the core state, the system proxy and the TUN status are all
//!   reported, in that order.
//! * On failure the supervisor is asked what actually happened. If the previous
//!   core survived (the failure happened before the supervisor was touched) the
//!   UI is told `Connected` and the OS proxy is left alone. Otherwise the UI is
//!   told `Disconnected` and the OS proxy is restored **unconditionally** — a
//!   dead core with the proxy still pointing at its port blackholes every
//!   connection, on every backend, not just native TUN.
//! * Emission is best effort. The sink returns nothing, so a webview that is
//!   tearing down can never rewrite a `MissingCore` error into an emit error.

use std::sync::Arc;

use voya_contracts::{CoreFlowReason, LogCode, NoticeCode};
use voya_core::AppConfig;
use voya_platform::{coreinfo::TargetOs, sysproxy::SystemProxyStatus};

use crate::{
    runtime::{RuntimeError, RuntimeManager},
    supervisor::{
        CoreExitEvent, CoreExitOutcome, NativeTunExitEvent, SupervisorConnectionState,
        SupervisorSnapshot,
    },
    sysproxy::{runtime_system_proxy_config, SystemProxyManager, SystemProxyManagerError},
    tun::{TunManager, TunStatus},
};

/// Severity for the flow's log lines and user notices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreFlowLevel {
    Info,
    Warn,
    Error,
}

/// Connection state the flow reports to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreFlowState {
    Connecting,
    Connected,
    Disconnecting,
    Disconnected,
}

/// Everything the flow tells the outside world.
///
/// Every method returns `()`: by the time these are called the OS-level side
/// effects have already happened, so a failed emit must be logged by the
/// adapter and never propagated.
/// Everything the flow tells the outside world, as codes rather than
/// sentences.
///
/// `log` and `notice` used to take an English `&str` that the shell forwarded
/// straight to the Logs panel and the toast store, which is why none of it was
/// translatable. They take a code and an optional untranslated `detail` now;
/// only the core process's own output stays raw, and it never comes through
/// here.
pub trait CoreFlowSink: Send + Sync {
    fn log(&self, level: CoreFlowLevel, code: LogCode, detail: Option<&str>);
    fn core_state(
        &self,
        state: CoreFlowState,
        active_profile_id: Option<String>,
        snapshot: Option<&SupervisorSnapshot>,
    );
    fn system_proxy_changed(&self, status: &SystemProxyStatus);
    fn tun_changed(&self, status: &TunStatus);
    fn statistics_zero(&self);
    fn notice(&self, level: CoreFlowLevel, code: NoticeCode, detail: &str);
}

pub struct CoreFlow<'flow> {
    runtime: RuntimeManager<'flow>,
    system_proxy: SystemProxyManager,
    tun: TunManager,
    sink: Arc<dyn CoreFlowSink>,
    target_os: TargetOs,
}

impl<'flow> CoreFlow<'flow> {
    #[must_use]
    pub fn new(
        runtime: RuntimeManager<'flow>,
        system_proxy: SystemProxyManager,
        tun: TunManager,
        sink: Arc<dyn CoreFlowSink>,
    ) -> Self {
        Self::with_target_os(runtime, system_proxy, tun, sink, TargetOs::current())
    }

    #[must_use]
    pub fn with_target_os(
        runtime: RuntimeManager<'flow>,
        system_proxy: SystemProxyManager,
        tun: TunManager,
        sink: Arc<dyn CoreFlowSink>,
        target_os: TargetOs,
    ) -> Self {
        Self {
            runtime,
            system_proxy,
            tun,
            sink,
            target_os,
        }
    }

    /// Start the core for the active profile.
    pub async fn connect(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
        self.announce_start(config, LogCode::Connecting);
        let result = self.runtime.connect(config).await;

        self.settle(config, result, LogCode::Connected, CoreFlowReason::Connect)
            .await
    }

    /// Restart the core, whatever state it is in.
    pub async fn restart(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
        self.announce_start(config, LogCode::Restarting);
        let result = self.runtime.restart(config).await;

        self.settle(config, result, LogCode::Restarted, CoreFlowReason::Restart)
            .await
    }

    /// Restart the core only if it is running, after a configuration change.
    pub async fn restart_if_connected(
        &self,
        config: &AppConfig,
        reason: CoreFlowReason,
    ) -> Result<(), RuntimeError> {
        if self.runtime.status().await?.state != SupervisorConnectionState::Connected {
            return Ok(());
        }

        self.announce_start(config, LogCode::RestartingAfterChange { reason });
        match self.runtime.restart_if_connected(config).await {
            Ok(Some(snapshot)) => {
                self.settle_connected(config, &snapshot, LogCode::RestartedAfterChange { reason })
                    .await;
                Ok(())
            }
            // A disconnect won the runtime lock between the check above and the
            // restart, so there is nothing to restart any more.
            Ok(None) => {
                self.reconcile(config, reason).await;
                Ok(())
            }
            Err(error) => {
                self.report_failure(reason, &error);
                self.reconcile(config, reason).await;
                Err(error)
            }
        }
    }

    /// Stop the core and hand the machine's proxy settings back.
    pub async fn disconnect(&self, config: &AppConfig) -> Result<SupervisorSnapshot, RuntimeError> {
        self.sink
            .log(CoreFlowLevel::Info, LogCode::Disconnecting, None);
        self.sink
            .core_state(CoreFlowState::Disconnecting, None, None);

        match self.runtime.disconnect().await {
            Ok(snapshot) => {
                self.sink
                    .log(CoreFlowLevel::Info, LogCode::Disconnected, None);
                self.settle_disconnected(config, None, Some(&snapshot))
                    .await;
                Ok(snapshot)
            }
            Err(error) => {
                self.report_failure(CoreFlowReason::Disconnect, &error);
                self.reconcile(config, CoreFlowReason::Disconnect).await;
                Err(error)
            }
        }
    }

    /// React to a core process that exited on its own.
    pub async fn handle_core_exit(&self, config: &AppConfig, event: CoreExitEvent) {
        let exit = describe_exit(&event);
        match event.outcome {
            CoreExitOutcome::Restarted { attempt, snapshot } => {
                self.sink.log(
                    CoreFlowLevel::Warn,
                    LogCode::CoreExitRestarted { attempt },
                    Some(&exit),
                );
                // The pid changed, so the UI needs the new snapshot.
                self.sink.core_state(
                    CoreFlowState::Connected,
                    event.active_profile_id,
                    Some(&snapshot),
                );
            }
            CoreExitOutcome::RestartScheduled { attempt, delay } => {
                self.sink.log(
                    CoreFlowLevel::Warn,
                    LogCode::CoreExitRetryScheduled {
                        attempt,
                        delay_ms: u32::try_from(delay.as_millis()).unwrap_or(u32::MAX),
                    },
                    Some(&exit),
                );
                self.sink
                    .core_state(CoreFlowState::Connecting, event.active_profile_id, None);
            }
            CoreExitOutcome::GaveUp(reason) => {
                let detail = format!("{exit}: {reason}");
                self.sink
                    .log(CoreFlowLevel::Error, LogCode::CoreExitGaveUp, Some(&detail));
                self.sink
                    .notice(CoreFlowLevel::Error, NoticeCode::CoreStopped, &detail);
                self.settle_disconnected(config, event.active_profile_id, None)
                    .await;
            }
        }
    }

    /// React to a native TUN provider that reached a terminal state.
    pub async fn handle_native_tun_exit(&self, config: &AppConfig, event: NativeTunExitEvent) {
        self.sink.log(
            CoreFlowLevel::Error,
            LogCode::NativeTunExited,
            Some(&event.message),
        );
        self.sink.notice(
            CoreFlowLevel::Error,
            NoticeCode::NativeTunStopped,
            &event.message,
        );
        self.settle_disconnected(config, event.active_profile_id, None)
            .await;
    }

    fn announce_start(&self, config: &AppConfig, code: LogCode) {
        self.sink.log(CoreFlowLevel::Info, code, None);
        self.sink
            .core_state(CoreFlowState::Connecting, active_profile_id(config), None);
    }

    /// A failed core operation, logged with the error as its detail.
    fn report_failure(&self, reason: CoreFlowReason, error: &RuntimeError) {
        self.sink.log(
            CoreFlowLevel::Error,
            LogCode::CoreOperationFailed { reason },
            Some(&error.to_string()),
        );
    }

    async fn settle(
        &self,
        config: &AppConfig,
        result: Result<SupervisorSnapshot, RuntimeError>,
        success_code: LogCode,
        reason: CoreFlowReason,
    ) -> Result<SupervisorSnapshot, RuntimeError> {
        match result {
            Ok(snapshot) => {
                self.settle_connected(config, &snapshot, success_code).await;
                Ok(snapshot)
            }
            Err(error) => {
                self.report_failure(reason, &error);
                self.reconcile(config, reason).await;
                Err(error)
            }
        }
    }

    async fn settle_connected(
        &self,
        config: &AppConfig,
        snapshot: &SupervisorSnapshot,
        code: LogCode,
    ) {
        self.sink.log(CoreFlowLevel::Info, code, None);
        self.sink
            .core_state(CoreFlowState::Connected, None, Some(snapshot));
        match self.apply_system_proxy(config) {
            Ok(status) => self.sink.system_proxy_changed(&status),
            Err(error) => self.sink.notice(
                CoreFlowLevel::Warn,
                NoticeCode::CoreStartedSystemProxyFailed,
                &error.to_string(),
            ),
        }
        self.report_tun_status(config).await;
    }

    async fn settle_disconnected(
        &self,
        config: &AppConfig,
        active_profile_id: Option<String>,
        snapshot: Option<&SupervisorSnapshot>,
    ) {
        self.sink
            .core_state(CoreFlowState::Disconnected, active_profile_id, snapshot);
        // Unconditional: a dead core with the OS proxy still applied is a
        // blackhole on every backend, and `restore` is idempotent.
        match self.system_proxy.restore(config) {
            Ok(status) => self.sink.system_proxy_changed(&status),
            Err(error) => self.sink.notice(
                CoreFlowLevel::Warn,
                NoticeCode::SystemProxyRestoreFailed,
                &error.to_string(),
            ),
        }
        self.report_tun_status(config).await;
        self.sink.statistics_zero();
    }

    /// Report the state the supervisor is really in after a failed operation.
    async fn reconcile(&self, config: &AppConfig, reason: CoreFlowReason) {
        match self.runtime.status().await {
            Ok(snapshot) if snapshot.state == SupervisorConnectionState::Connected => {
                // The failure happened before the supervisor was touched, so
                // the previous core is still serving the OS proxy.
                self.sink.log(
                    CoreFlowLevel::Warn,
                    LogCode::PreviousCoreStillRunning { reason },
                    None,
                );
                self.sink
                    .core_state(CoreFlowState::Connected, None, Some(&snapshot));
            }
            Ok(snapshot) => {
                self.settle_disconnected(config, None, Some(&snapshot))
                    .await;
            }
            Err(error) => {
                self.sink.log(
                    CoreFlowLevel::Warn,
                    LogCode::RuntimeStatusRefreshFailed { reason },
                    Some(&error.to_string()),
                );
                self.settle_disconnected(config, None, None).await;
            }
        }
    }

    fn apply_system_proxy(
        &self,
        config: &AppConfig,
    ) -> Result<SystemProxyStatus, SystemProxyManagerError> {
        let runtime = runtime_system_proxy_config(config, false, self.target_os);
        self.system_proxy
            .apply_config(&runtime.config, runtime.force_disable)
    }

    /// The TUN probe forks `pluginkit`/`sc.exe`/`systemextensionsctl`, so it
    /// never runs on the caller's async worker.
    async fn report_tun_status(&self, config: &AppConfig) {
        let tun = self.tun.clone();
        let config = config.clone();

        match tokio::task::spawn_blocking(move || tun.status(&config)).await {
            Ok(Ok(status)) => self.sink.tun_changed(&status),
            Ok(Err(error)) => tracing::warn!(?error, "failed to read TUN status for the core flow"),
            Err(error) => tracing::warn!(?error, "TUN status task failed"),
        }
    }
}

fn active_profile_id(config: &AppConfig) -> Option<String> {
    let index_id = config.index_id.trim();
    (!index_id.is_empty()).then(|| index_id.to_string())
}

fn describe_exit(event: &CoreExitEvent) -> String {
    match event.exit_code {
        Some(code) => format!("Core process {} exited with code {code}", event.process_id),
        None => format!("Core process {} exited", event.process_id),
    }
}

#[cfg(test)]
mod tests;
