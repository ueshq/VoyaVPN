//! The Hiddify-style connection mode over the two persisted primitives
//! (`system_proxy_item.sys_proxy_type` and `tun_mode_item.enable_tun`), and the
//! one transaction that changes them.
//!
//! The Tauri shell used to own that transaction twice — `set_connection_mode`
//! and `set_system_proxy_mode` — and both copies got the same two things wrong:
//!
//! * They pointed the machine's proxy settings at `127.0.0.1:<port>` whenever a
//!   *mode* was selected, not when a core was *connected*. Selecting "system
//!   proxy" (or launching with it persisted) black-holed every request until
//!   the user pressed Connect, because nothing was listening on that port. The
//!   rule this module enforces instead is the one the rest of the app already
//!   follows on automatic platforms: **the OS proxy follows the connected core**.
//!   On macOS only the local PAC listener follows the core; OS proxies are manual. The mode is always
//!   persisted; the OS is only touched while the supervisor reports
//!   `Connected`, and otherwise the caller gets the *planned* status so the UI
//!   and the tray still show what was chosen.
//! * They ran multi-second blocking OS work (`networksetup` once per network
//!   service, `pluginkit`, `sc.exe`) while holding the global config-mutation
//!   lock and a pooled database connection, and they ran it *before* the
//!   commit, so a commit failure re-ran all of it as compensation. Here the
//!   guard only covers the config + database work; the TUN preflight runs
//!   before it and the OS proxy apply after it, both on a blocking thread, and
//!   a failed apply rolls the persisted mode back instead.
//!
//! Since the shell's lib test harness is disabled, none of that was testable
//! there. It is here.

use std::sync::Arc;

use thiserror::Error;
use voya_contracts::{ConnectionMode, ConnectionModeStatus};
use voya_core::{AppConfig, SysProxyType};
use voya_platform::{
    coreinfo::TargetOs,
    sysproxy::{pac_available, SystemProxyStatus},
};

use crate::{
    config_mutation::{ConfigMutationCoordinator, ConfigMutationError},
    supervisor::SupervisorConnectionState,
    sysproxy::{SystemProxyManager, SystemProxyManagerError},
    tun::{TunManager, TunManagerError, TunStatus},
};

/// Derives the current connection mode and whether PAC is the selected system
/// proxy flavor. TUN wins over everything; `Unchanged` counts as proxy-only
/// because the OS proxy is not being managed.
#[must_use]
pub fn derive_connection_mode(config: &AppConfig) -> (ConnectionMode, bool) {
    let pac_enabled = matches!(config.system_proxy_item.sys_proxy_type, SysProxyType::Pac);
    if config.tun_mode_item.enable_tun {
        return (ConnectionMode::Vpn, pac_enabled);
    }
    match config.system_proxy_item.sys_proxy_type {
        SysProxyType::Pac | SysProxyType::ForcedChange => {
            (ConnectionMode::SystemProxy, pac_enabled)
        }
        SysProxyType::ForcedClear | SysProxyType::Unchanged => (ConnectionMode::ProxyOnly, false),
    }
}

/// Applies a connection mode onto the config primitives. Entering `Vpn` keeps
/// the stored system proxy type untouched (runtime interplay rules already
/// force-disable or fall back per platform); leaving it turns TUN off.
pub fn apply_connection_mode(config: &mut AppConfig, mode: ConnectionMode, pac_enabled: bool) {
    match mode {
        ConnectionMode::ProxyOnly => {
            config.tun_mode_item.enable_tun = false;
            config.system_proxy_item.sys_proxy_type = SysProxyType::ForcedClear;
        }
        ConnectionMode::SystemProxy => {
            config.tun_mode_item.enable_tun = false;
            config.system_proxy_item.sys_proxy_type = if pac_enabled {
                SysProxyType::Pac
            } else {
                SysProxyType::ForcedChange
            };
        }
        ConnectionMode::Vpn => {
            config.tun_mode_item.enable_tun = true;
        }
    }
}

/// The mode snapshot the UI renders. `pac_available` comes from the platform's
/// single source of truth so adding a PAC platform stays a one-line change.
#[must_use]
pub fn connection_mode_status(
    config: &AppConfig,
    tun_status: &TunStatus,
    target_os: TargetOs,
) -> ConnectionModeStatus {
    let (mode, pac_enabled) = derive_connection_mode(config);

    ConnectionModeStatus {
        mode,
        pac_enabled,
        pac_available: pac_available(target_os),
        vpn_available: tun_status.allow_enable_tun || tun_status.enabled,
        process_rules_effective: mode == ConnectionMode::Vpn,
    }
}

/// Everything the transaction tells the outside world once the configuration is
/// committed and the OS state has settled.
///
/// Every method returns `()`: by the time they are called the change is already
/// persisted, so a webview that is tearing down must never turn a successful
/// transaction into a failed command.
pub trait ConnectionModeSink: Send + Sync {
    fn system_proxy_changed(&self, status: &SystemProxyStatus);
    fn tun_changed(&self, status: &TunStatus);
    fn tray_refresh(&self);
}

/// What the caller needs after a committed mode change.
#[derive(Debug, Clone)]
pub struct ConnectionModeOutcome {
    /// The configuration as committed.
    pub config: AppConfig,
    pub status: ConnectionModeStatus,
    pub system_proxy_status: SystemProxyStatus,
    /// `false` when the mode was persisted but the machine was left alone
    /// because no core is running.
    pub system_proxy_applied: bool,
    pub tun_status: TunStatus,
    /// Only a TUN flip needs a running core to be restarted; a proxy-only or
    /// system-proxy switch is picked up live.
    pub tun_flag_changed: bool,
}

#[derive(Debug, Error)]
pub enum ConnectionModeError {
    #[error(transparent)]
    Tun(#[from] TunManagerError),
    #[error(transparent)]
    SystemProxy(#[from] SystemProxyManagerError),
    #[error(transparent)]
    Commit(#[from] ConfigMutationError),
    #[error("failed to apply the system proxy: {source}; the previous mode was restored")]
    SystemProxyRolledBack { source: SystemProxyManagerError },
    #[error(
        "failed to apply the system proxy: {source}; restoring the previous mode also failed: {rollback}"
    )]
    SystemProxyRollbackFailed {
        source: SystemProxyManagerError,
        rollback: ConfigMutationError,
    },
    #[error("{context} task failed: {message}")]
    Task {
        context: &'static str,
        message: String,
    },
}

/// The single owner of the connection-mode / system-proxy transaction.
#[derive(Clone)]
pub struct ConnectionModeManager {
    system_proxy: SystemProxyManager,
    tun: TunManager,
    sink: Arc<dyn ConnectionModeSink>,
    target_os: TargetOs,
}

impl ConnectionModeManager {
    #[must_use]
    pub fn new(
        system_proxy: SystemProxyManager,
        tun: TunManager,
        sink: Arc<dyn ConnectionModeSink>,
    ) -> Self {
        Self::with_target_os(system_proxy, tun, sink, TargetOs::current())
    }

    #[must_use]
    pub fn with_target_os(
        system_proxy: SystemProxyManager,
        tun: TunManager,
        sink: Arc<dyn ConnectionModeSink>,
        target_os: TargetOs,
    ) -> Self {
        Self {
            system_proxy,
            tun,
            sink,
            target_os,
        }
    }

    /// Switch the app between proxy-only / system proxy / VPN.
    ///
    /// `pac_enabled` of `None` keeps whatever PAC flavor is already stored.
    /// `connected` is the supervisor state the caller observed: it decides
    /// whether the machine's proxy settings are touched at all.
    pub async fn set_connection_mode(
        &self,
        coordinator: &ConfigMutationCoordinator,
        mode: ConnectionMode,
        pac_enabled: Option<bool>,
        connected: SupervisorConnectionState,
    ) -> Result<ConnectionModeOutcome, ConnectionModeError> {
        let snapshot = coordinator.current_config();
        let (_, current_pac) = derive_connection_mode(&snapshot);
        let pac_enabled = pac_enabled.unwrap_or(current_pac);
        if mode == ConnectionMode::SystemProxy && pac_enabled && !pac_available(self.target_os) {
            return Err(SystemProxyManagerError::PacUnavailable(self.target_os).into());
        }

        // Entering VPN must clear the elevation / native-provider preflight
        // before anything is written. The probe forks OS helpers, so it runs on
        // a blocking thread against a snapshot rather than under the mutation
        // guard's `&mut AppConfig`.
        let enable_tun = mode == ConnectionMode::Vpn;
        let tun_status = self.plan_tun(&snapshot, enable_tun).await?;

        let (original, committed) = self
            .commit(coordinator, |config| {
                apply_connection_mode(config, mode, pac_enabled);
            })
            .await?;
        let tun_flag_changed =
            original.tun_mode_item.enable_tun != committed.tun_mode_item.enable_tun;

        let (system_proxy_status, system_proxy_applied) = self
            .settle_system_proxy(coordinator, &original, &committed, connected)
            .await?;

        self.sink.system_proxy_changed(&system_proxy_status);
        self.sink.tun_changed(&tun_status);
        self.sink.tray_refresh();

        Ok(ConnectionModeOutcome {
            status: connection_mode_status(&committed, &tun_status, self.target_os),
            config: committed,
            system_proxy_status,
            system_proxy_applied,
            tun_status,
            tun_flag_changed,
        })
    }

    /// Change only the system proxy flavor, leaving TUN alone.
    pub async fn set_system_proxy_mode(
        &self,
        coordinator: &ConfigMutationCoordinator,
        mode: SysProxyType,
        connected: SupervisorConnectionState,
    ) -> Result<SystemProxyStatus, ConnectionModeError> {
        if mode == SysProxyType::Pac && !pac_available(self.target_os) {
            return Err(SystemProxyManagerError::PacUnavailable(self.target_os).into());
        }

        let (original, committed) = self
            .commit(coordinator, |config| {
                config.system_proxy_item.sys_proxy_type = mode;
            })
            .await?;
        let (status, _) = self
            .settle_system_proxy(coordinator, &original, &committed, connected)
            .await?;

        self.sink.system_proxy_changed(&status);
        self.sink.tray_refresh();

        Ok(status)
    }

    /// The guard's whole lifetime: read, mutate, write. No OS call, no process
    /// spawn, nothing that can block a tokio worker or another command for
    /// seconds behind the global mutation lock.
    async fn commit(
        &self,
        coordinator: &ConfigMutationCoordinator,
        mutate: impl FnOnce(&mut AppConfig),
    ) -> Result<(AppConfig, AppConfig), ConnectionModeError> {
        let mut mutation = coordinator.begin().await?;
        let original = mutation.config().clone();
        mutate(mutation.config_mut());
        let committed = mutation.commit().await?;

        Ok((original, committed))
    }

    /// Apply the committed mode to the machine — but only while a core is
    /// actually serving the port it points at.
    async fn settle_system_proxy(
        &self,
        coordinator: &ConfigMutationCoordinator,
        original: &AppConfig,
        committed: &AppConfig,
        connected: SupervisorConnectionState,
    ) -> Result<(SystemProxyStatus, bool), ConnectionModeError> {
        if connected != SupervisorConnectionState::Connected {
            // Nothing is listening, so writing the OS proxy would blackhole
            // every request. `connect` applies the persisted mode once the core
            // is up; report the plan so the UI and tray stay truthful.
            return Ok((self.planned_status(committed).await?, false));
        }

        match self.apply(committed).await? {
            Ok(status) => Ok((status, true)),
            Err(source) => {
                // The machine wins: a persisted mode the OS refused would
                // silently come back on the next connect.
                let rollback = self.rollback(coordinator, original).await;
                if let Some(error) = self.apply(original).await?.err() {
                    tracing::warn!(
                        ?error,
                        "failed to re-apply the previous system proxy mode after a rollback"
                    );
                }
                Err(match rollback {
                    Ok(()) => ConnectionModeError::SystemProxyRolledBack { source },
                    Err(rollback) => {
                        ConnectionModeError::SystemProxyRollbackFailed { source, rollback }
                    }
                })
            }
        }
    }

    async fn rollback(
        &self,
        coordinator: &ConfigMutationCoordinator,
        original: &AppConfig,
    ) -> Result<(), ConfigMutationError> {
        let mut mutation = coordinator.begin().await?;
        *mutation.config_mut() = original.clone();
        mutation.commit().await.map(|_| ())
    }

    /// Automatic proxy scripts fork `gsettings`/`reg` once per
    /// network service, so they never run on the caller's async worker.
    async fn apply(
        &self,
        config: &AppConfig,
    ) -> Result<Result<SystemProxyStatus, SystemProxyManagerError>, ConnectionModeError> {
        let config = config.clone();
        let manager = self.system_proxy.clone();

        run_blocking("system proxy apply", move || {
            manager.apply_runtime_config(&config)
        })
        .await
    }

    async fn planned_status(
        &self,
        config: &AppConfig,
    ) -> Result<SystemProxyStatus, ConnectionModeError> {
        let config = config.clone();
        let manager = self.system_proxy.clone();

        run_blocking("system proxy status", move || {
            manager.runtime_status(&config)
        })
        .await?
        .map_err(Into::into)
    }

    async fn plan_tun(
        &self,
        config: &AppConfig,
        enabled: bool,
    ) -> Result<TunStatus, ConnectionModeError> {
        let manager = self.tun.clone();
        let config = config.clone();

        run_blocking("TUN preflight", move || {
            manager.plan_set_enabled(&config, enabled)
        })
        .await?
        .map_err(Into::into)
    }
}

async fn run_blocking<T>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ConnectionModeError>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ConnectionModeError::Task {
            context,
            message: error.to_string(),
        })
}

#[cfg(test)]
mod tests;
