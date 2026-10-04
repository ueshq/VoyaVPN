//! A background loop that is told to stop and then waited for.
//!
//! The statistics aggregator and the subscription scheduler both end the same
//! way: the exit path signals them and has to wait, because Tauri ends the
//! process with `std::process::exit` and work that was only signalled is lost.

use std::{
    future::Future,
    sync::{Mutex, PoisonError},
};

use tokio::{sync::watch, task::JoinHandle};

pub(crate) struct ShutdownTask {
    shutdown: watch::Sender<bool>,
    /// Taken by [`Self::shutdown`], so the `Drop` below does not then abort a
    /// task that already stopped on its own. Behind a lock because the hosts
    /// hold their managers by shared reference.
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl ShutdownTask {
    /// Spawns `run` with the signal it has to watch.
    pub(crate) fn spawn<Fut>(run: impl FnOnce(watch::Receiver<bool>) -> Fut) -> Self
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (shutdown, shutdown_rx) = watch::channel(false);
        Self {
            shutdown,
            handle: Mutex::new(Some(tokio::spawn(run(shutdown_rx)))),
        }
    }

    /// The same signal, for a companion task that stops with this one.
    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    /// Requests shutdown and waits for the loop to actually stop. A second
    /// call finds nothing left to wait for.
    pub(crate) async fn shutdown(&self) {
        let _ = self.shutdown.send(true);
        // The guard is dropped before the await: holding a std lock across one
        // would be a deadlock waiting to happen. A poisoned slot still holds
        // the handle, and skipping it would skip the wait.
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(handle) = handle {
            // A `JoinError` means the loop panicked or was aborted; either way
            // it is no longer running, which is all this call promises.
            let _ = handle.await;
        }
    }

    #[cfg(test)]
    pub(crate) fn is_joined(&self) -> bool {
        self.handle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_none()
    }
}

impl Drop for ShutdownTask {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        if let Some(handle) = self
            .handle
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
        {
            handle.abort();
        }
    }
}
