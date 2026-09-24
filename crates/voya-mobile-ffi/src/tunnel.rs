//! The tunnel, owned by the host app.
//!
//! On a phone the core runs inside the tunnel provider — a NetworkExtension
//! `NEPacketTunnelProvider` on iOS, a `VpnService` on Android — which only the
//! host app can start. ADR 0005's `NativeTunController` is already the seam for
//! "run the core without spawning a child process", and
//! `SupervisorActor::plan_start` branches on `backend.is_native()` *before* it
//! tears the old core down, so nothing above here changes.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use voya_platform::tun::{
    NativeTunController, NativeTunError, NativeTunProviderState, NativeTunStartRequest,
    NativeTunStatus, TunBackend,
};

use crate::handoff::{build_handoff, HandoffError};

/// What the host app does with the tunnel, as uniffi sees it.
///
/// `start` receives the whole runtime configuration inline: the provider runs
/// in a process of its own on iOS and cannot read the app's container, so the
/// handshake carries the config rather than a path to it.
#[uniffi::export(with_foreign)]
pub trait TunnelHost: Send + Sync {
    /// Starts the provider with a handshake payload, blocking until it is up or
    /// has failed. `include_all_networks` asks iOS to keep traffic from leaving
    /// outside the tunnel.
    fn start(&self, handoff_json: String, include_all_networks: bool) -> Result<(), TunnelError>;
    fn stop(&self) -> Result<(), TunnelError>;
    /// The provider's state as one of `NativeTunProviderState`'s camelCase
    /// names; anything else reads as an error.
    fn status(&self) -> String;
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum TunnelError {
    /// The user declined the VPN configuration prompt, or revoked it later.
    #[error("the system did not authorize the VPN configuration")]
    PermissionDenied,
    #[error("the tunnel provider is not installed in this build")]
    MissingProvider,
    #[error("the tunnel could not be started or stopped: {detail}")]
    Failed { detail: String },
}

/// Bridges `NativeTunController` onto the host's callback.
pub struct HostTunController {
    host: Arc<dyn TunnelHost>,
    /// The container both processes can reach: where rule sets are staged and
    /// where the provider writes its status and log.
    shared_dir: PathBuf,
    /// The last failure, so `status` can explain a provider that is down.
    last_error: Mutex<Option<String>>,
}

impl HostTunController {
    #[must_use]
    pub fn new(host: Arc<dyn TunnelHost>, shared_dir: PathBuf) -> Self {
        Self {
            host,
            shared_dir,
            last_error: Mutex::new(None),
        }
    }

    fn record(&self, error: &TunnelError) {
        if let Ok(mut last) = self.last_error.lock() {
            *last = Some(error.to_string());
        }
    }

    fn clear(&self) {
        if let Ok(mut last) = self.last_error.lock() {
            *last = None;
        }
    }
}

impl NativeTunController for HostTunController {
    fn status(&self, backend: TunBackend) -> NativeTunStatus {
        let state = provider_state(&self.host.status());
        let message = self.last_error.lock().ok().and_then(|last| last.clone());

        NativeTunStatus {
            backend,
            provider_state: state,
            component_ready: true,
            message,
        }
    }

    fn start(&self, request: NativeTunStartRequest) -> Result<(), NativeTunError> {
        let handoff = build_handoff(&request, &self.shared_dir)
            .map_err(|error| handoff_failed(&request, &error))?;

        match self.host.start(handoff, request.kill_switch) {
            Ok(()) => {
                self.clear();
                Ok(())
            }
            Err(error) => {
                self.record(&error);
                Err(start_failed(&request, &error))
            }
        }
    }

    fn stop(&self, backend: TunBackend) -> Result<(), NativeTunError> {
        match self.host.stop() {
            Ok(()) => {
                self.clear();
                Ok(())
            }
            Err(error) => {
                self.record(&error);
                Err(NativeTunError::ControllerUnavailable {
                    backend,
                    message: error.to_string(),
                })
            }
        }
    }
}

/// Reads the provider state the host reported. An unknown word is an error
/// rather than a guess: a provider whose state cannot be read is not running.
fn provider_state(reported: &str) -> NativeTunProviderState {
    match reported {
        "running" => NativeTunProviderState::Running,
        "starting" => NativeTunProviderState::Starting,
        "stopped" => NativeTunProviderState::Stopped,
        "permissionRequired" => NativeTunProviderState::PermissionRequired,
        "missingComponent" => NativeTunProviderState::MissingComponent,
        _ => NativeTunProviderState::Error,
    }
}

fn start_failed(request: &NativeTunStartRequest, error: &TunnelError) -> NativeTunError {
    match error {
        // The user's answer to a system prompt, not a malfunction: the frontend
        // turns this kind into "authorization was not granted".
        TunnelError::PermissionDenied => NativeTunError::PermissionRequired {
            backend: request.backend,
            message: error.to_string(),
        },
        TunnelError::MissingProvider | TunnelError::Failed { .. } => {
            NativeTunError::ControllerUnavailable {
                backend: request.backend,
                message: error.to_string(),
            }
        }
    }
}

fn handoff_failed(request: &NativeTunStartRequest, error: &HandoffError) -> NativeTunError {
    NativeTunError::ControllerUnavailable {
        backend: request.backend,
        message: format!(
            "could not prepare the tunnel handshake from {}: {error}",
            display(&request.main_config_path)
        ),
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests;
