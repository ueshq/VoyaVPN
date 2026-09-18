//! Repeats the network check while the node is enabled.
//!
//! Each pass renews the router forward (its lease is an hour), refreshes the
//! public addresses the links use, and lets the manager notice a moved
//! address. A start or a UPnP toggle asks for a pass right away.

use std::time::Duration;

use tokio::sync::watch;

use super::manager::SelfHostManager;

const WATCH_INTERVAL: Duration = Duration::from_secs(10 * 60);

pub(super) async fn run_watch_loop(manager: SelfHostManager, mut shutdown: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
                continue;
            }
            () = manager.wait_for_check_request() => {}
            () = tokio::time::sleep(WATCH_INTERVAL) => {}
        }
        if let Err(error) = manager.periodic_check().await {
            tracing::warn!(%error, "self-hosted network check failed");
        }
    }
}
