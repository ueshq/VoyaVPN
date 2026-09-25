//! The Hiddify-style connection mode over the two persisted primitives
//! (`system_proxy.mode` and `tun.enabled`), and the
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
//!   macOS has no system proxy mode: it captures traffic only through its
//!   PacketTunnel VPN, so leaving VPN mode is refused there. The mode is always
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
use voya_contracts::{ConnectionMode, ConnectionModeStatus, TunBackend, TunStatus};
use voya_core::{AppConfig, SysProxyType};
use voya_platform::{
    coreinfo::TargetOs,
    sysproxy::SystemProxyStatus,
    tun::{tun_backend, TunBackend as PlatformTunBackend},
};

use crate::{
    config_mutation::{ConfigMutationCoordinator, ConfigMutationError},
    supervisor::SupervisorConnectionState,
    sysproxy::{SystemProxyManager, SystemProxyManagerError},
    tun::{TunManager, TunManagerError},
};

/// Derives the current connection mode. TUN wins over everything; every other
/// state uses the system proxy.
#[must_use]
pub fn derive_connection_mode(config: &AppConfig) -> ConnectionMode {
    if config.tun.enabled {
        ConnectionMode::Vpn
    } else {
        ConnectionMode::SystemProxy
    }
}

/// Applies a connection mode onto the config primitives. Entering `Vpn` keeps
/// the stored system proxy type untouched (runtime interplay rules already
/// force-disable or fall back per platform); leaving it turns TUN off.
pub fn apply_connection_mode(config: &mut AppConfig, mode: ConnectionMode) {
    match mode {
        ConnectionMode::SystemProxy => {
            config.tun.enabled = false;
            config.system_proxy.mode = SysProxyType::ForcedChange;
        }
        ConnectionMode::Vpn => {
            config.tun.enabled = true;
        }
    }
}

/// The mode snapshot the UI renders.
#[must_use]
pub fn connection_mode_status(config: &AppConfig, tun_status: &TunStatus) -> ConnectionModeStatus {
    let mode = derive_connection_mode(config);
    // The macOS NetworkExtension tunnel is the only capture path there, and it
    // cannot match traffic by process.
    let packet_tunnel = matches!(
        tun_status.backend,
        TunBackend::MacosPacketTunnel | TunBackend::IosPacketTunnel | TunBackend::AndroidVpnService
    );

    ConnectionModeStatus {
        mode,
        vpn_available: tun_status.allow_enable_tun || tun_status.enabled,
        system_proxy_available: !packet_tunnel,
        process_rules_supported: !packet_tunnel,
        process_rules_effective: !packet_tunnel && mode == ConnectionMode::Vpn,
    }
}

/// Whether `target_os` offers the system proxy mode.
///
/// macOS captures traffic only through its PacketTunnel VPN, and both phones
/// are the same shape: the tunnel provider is the only capture path, and
/// neither OS lets an app point the system at a local proxy.
#[must_use]
fn system_proxy_mode_available(target_os: TargetOs) -> bool {
    !matches!(
        tun_backend(target_os),
        PlatformTunBackend::MacosPacketTunnel
            | PlatformTunBackend::IosPacketTunnel
            | PlatformTunBackend::AndroidVpnService
    )
}

/// The capture mode a fresh install starts in: the native VPN where the app
/// ships one (the Windows service, the macOS PacketTunnel, either phone's
/// tunnel provider). Linux keeps the system proxy, because its process TUN
/// needs a root launcher installed first.
pub fn seed_platform_connection_defaults(config: &mut AppConfig, target_os: TargetOs) {
    if tun_backend(target_os).is_native() {
        config.tun.enabled = true;
    }
}

/// Keep a configuration within what the platform offers. On macOS that is VPN
/// mode with the OS proxy left alone, whatever an older build or a stale
/// settings view saved. Returns whether anything changed.
pub fn enforce_platform_connection_mode(config: &mut AppConfig, target_os: TargetOs) -> bool {
    if system_proxy_mode_available(target_os) {
        return false;
    }
    let changed = !config.tun.enabled || config.system_proxy.mode != SysProxyType::Unchanged;
    config.tun.enabled = true;
    config.system_proxy.mode = SysProxyType::Unchanged;
    changed
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
    /// Only a TUN flip needs a running core to be restarted; a system-proxy
    /// flavor change is picked up live.
    pub tun_flag_changed: bool,
    /// Whether the commit rewrote the persisted configuration. Always true
    /// for a mode switch today, but reported so a caller can follow
    /// `config_changed` rather than assume.
    pub config_changed: bool,
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
}

impl ConnectionModeManager {
    #[must_use]
    pub fn new(
        system_proxy: SystemProxyManager,
        tun: TunManager,
        sink: Arc<dyn ConnectionModeSink>,
    ) -> Self {
        Self {
            system_proxy,
            tun,
            sink,
        }
    }

    /// Switch the app between system proxy and TUN mode.
    ///
    /// `connected` is the supervisor state the caller observed: it decides
    /// whether the machine's proxy settings are touched at all.
    pub async fn set_connection_mode(
        &self,
        coordinator: &ConfigMutationCoordinator,
        mode: ConnectionMode,
        connected: SupervisorConnectionState,
    ) -> Result<ConnectionModeOutcome, ConnectionModeError> {
        let snapshot = coordinator.current_config();

        // Entering VPN must clear the elevation / native-provider preflight
        // before anything is written. The probe forks OS helpers, so it runs on
        // a blocking thread against a snapshot rather than under the mutation
        // guard's `&mut AppConfig`.
        let enable_tun = mode == ConnectionMode::Vpn;
        let tun_status = self.plan_tun(&snapshot, enable_tun).await?;

        let (original, committed, config_changed) = self
            .commit(coordinator, |config| {
                apply_connection_mode(config, mode);
            })
            .await?;
        let tun_flag_changed = original.tun.enabled != committed.tun.enabled;

        let (system_proxy_status, system_proxy_applied) = self
            .settle_system_proxy(coordinator, &original, &committed, connected)
            .await?;

        self.sink.system_proxy_changed(&system_proxy_status);
        self.sink.tun_changed(&tun_status);
        self.sink.tray_refresh();

        Ok(ConnectionModeOutcome {
            status: connection_mode_status(&committed, &tun_status),
            config: committed,
            system_proxy_status,
            system_proxy_applied,
            tun_status,
            tun_flag_changed,
            config_changed,
        })
    }

    /// The guard's whole lifetime: read, mutate, write. No OS call, no process
    /// spawn, nothing that can block a tokio worker or another command for
    /// seconds behind the global mutation lock.
    ///
    /// Returns the pre-mutation configuration alongside the commit so the
    /// caller can compute `tun_flag_changed` and roll back, and the
    /// coordinator's `config_changed` so a cache refresh can follow it.
    async fn commit(
        &self,
        coordinator: &ConfigMutationCoordinator,
        mutate: impl FnOnce(&mut AppConfig),
    ) -> Result<(AppConfig, AppConfig, bool), ConnectionModeError> {
        let committed = coordinator
            .mutate(async move |_unit_of_work, config| {
                let original = config.clone();
                mutate(config);
                Ok::<AppConfig, ConnectionModeError>(original)
            })
            .await?;
        Ok((committed.value, committed.config, committed.config_changed))
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
