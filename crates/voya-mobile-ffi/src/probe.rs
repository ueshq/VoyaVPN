//! The probe core, owned by the host app.
//!
//! A latency test while nothing is connected needs a sing-box of its own. The
//! desktop spawns one as a child process; neither phone may spawn anything, so
//! the host app runs Libbox inside its own process and hands back a token it
//! can stop again. `voya-app`'s [`ProbeCoreLauncher`] is that seam, and this is
//! its mobile implementation.
//!
//! The probe instance is deliberately *not* the tunnel's: it runs in the app
//! process with its own working directory, so its control socket cannot
//! collide with the provider's, and it opens no TUN. While connected the test
//! goes through the running core's Clash API instead and this is never asked.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use voya_app::speedtest::{ProbeCore, ProbeCoreLauncher, SpeedtestError};

/// What the host app does with a probe core, as uniffi sees it.
///
/// Both calls block: the launcher runs them on a blocking thread, the way the
/// desktop's process spawn is run.
#[uniffi::export(with_foreign)]
pub trait ProbeCoreHost: Send + Sync {
    /// Starts an instance for this generated sing-box configuration and
    /// returns the id `stop` will be given. The instance must be listening on
    /// the config's SOCKS inbounds by the time this returns, or shortly after:
    /// the caller polls the ports.
    fn start(&self, config_json: String) -> Result<String, ProbeCoreError>;

    /// Stops the instance with this id. Called once per started core, and
    /// never for an id the host did not hand out.
    fn stop(&self, core_id: String) -> Result<(), ProbeCoreError>;
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ProbeCoreError {
    #[error("this build cannot run a probe core in the app process")]
    Unsupported,
    #[error("the probe core could not be started or stopped: {message}")]
    Failed { message: String },
}

/// Bridges [`ProbeCoreLauncher`] onto the host's callback.
pub struct HostProbeCoreLauncher {
    host: Arc<dyn ProbeCoreHost>,
    /// Every core the host started and nobody has stopped. The app process can
    /// be torn down between a run and its cleanup, so the launcher keeps the
    /// ids rather than trusting each session to come back.
    live: Arc<Mutex<Vec<String>>>,
    /// Only for the id a refusing host is never given: see [`stop`].
    next_local_id: AtomicU64,
}

impl HostProbeCoreLauncher {
    #[must_use]
    pub fn new(host: Arc<dyn ProbeCoreHost>) -> Self {
        Self {
            host,
            live: Arc::new(Mutex::new(Vec::new())),
            next_local_id: AtomicU64::new(0),
        }
    }
}

impl ProbeCoreLauncher for HostProbeCoreLauncher {
    fn start(&self, config_json: String) -> Result<Box<dyn ProbeCore>, SpeedtestError> {
        // A host that hands back the same id twice would have the first stop
        // take the second core down, so an empty answer is made unique here
        // rather than trusted.
        let core_id = match self.host.start(config_json) {
            Ok(core_id) if !core_id.is_empty() => core_id,
            Ok(_) => format!(
                "probe-core-{}",
                self.next_local_id.fetch_add(1, Ordering::Relaxed)
            ),
            Err(error) => return Err(probe_core_failed(&error)),
        };
        lock_ignoring_poison(&self.live).push(core_id.clone());

        Ok(Box::new(HostProbeCore {
            core_id: Some(core_id),
            host: Arc::clone(&self.host),
            live: Arc::clone(&self.live),
        }))
    }

    fn stop_all(&self) {
        for core_id in std::mem::take(&mut *lock_ignoring_poison(&self.live)) {
            stop_on_host(self.host.as_ref(), &core_id);
        }
    }
}

struct HostProbeCore {
    core_id: Option<String>,
    host: Arc<dyn ProbeCoreHost>,
    live: Arc<Mutex<Vec<String>>>,
}

impl ProbeCore for HostProbeCore {
    fn stop(mut self: Box<Self>) {
        stop_core(&self.live, self.host.as_ref(), self.core_id.take());
    }
}

impl Drop for HostProbeCore {
    fn drop(&mut self) {
        // `stop` empties the slot, so this only fires for a core nobody
        // stopped — a panic, or a `?` between start and session.
        let core_id = self.core_id.take();
        stop_core(&self.live, self.host.as_ref(), core_id);
    }
}

fn stop_core(live: &Mutex<Vec<String>>, host: &dyn ProbeCoreHost, core_id: Option<String>) {
    let Some(core_id) = core_id else { return };
    lock_ignoring_poison(live).retain(|live_id| live_id != &core_id);
    stop_on_host(host, &core_id);
}

fn stop_on_host(host: &dyn ProbeCoreHost, core_id: &str) {
    if let Err(error) = host.stop(core_id.to_owned()) {
        tracing::warn!(core_id, ?error, "the host could not stop a probe core");
    }
}

fn probe_core_failed(error: &ProbeCoreError) -> SpeedtestError {
    SpeedtestError::ProbeCoreHost(error.to_string())
}

/// The registry survives a panicking holder intact — it is a list of ids —
/// and refusing the lock would leak every probe core after one.
fn lock_ignoring_poison<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests;
