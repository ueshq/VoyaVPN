//! One reconnecting reader for the running core's Clash websocket streams.
//!
//! The statistics collector reads `/traffic` and the proxy monitor reads
//! `/connections`, both from whichever core is running *now*. A core is
//! identified by how its Clash API is reached — the port and the bearer token
//! one launch minted together — and not by a process id: the core behind a
//! native TUN provider has no process this app can see, and a restart keeps
//! the port while changing the token.

use std::{future::Future, time::Duration};

use tokio::{sync::watch, time};
use voya_net::clash::{ClashWebSocketClient, ClashWebSocketEvent, ClashWebSocketResource};

use crate::{
    backoff::{
        sleep_or_shutdown, WebSocketReconnectBackoff, WS_CONNECT_TIMEOUT,
        WS_RECONNECT_INITIAL_DELAY, WS_RECONNECT_MAX_DELAY,
    },
    proxy_runtime::proxy_runtime_endpoint,
    supervisor::ClashApiAccess,
};

/// Streams `resource` from the running core into `on_event` until `shutdown`.
///
/// `access` is [`crate::supervisor::CoreSupervisor::subscribe_clash_api`]. The
/// endpoint is read from it before every connect, so a socket is never opened
/// with the token of a core that has since been replaced, and a session is
/// dropped as soon as the core it was opened against is gone. While nothing is
/// running the loop parks on the watch and dials nothing.
pub(crate) async fn follow_core_ws<F, Fut>(
    mut access: watch::Receiver<ClashApiAccess>,
    resource: ClashWebSocketResource,
    mut shutdown: watch::Receiver<bool>,
    mut on_event: F,
) where
    F: FnMut(ClashWebSocketEvent) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut reconnect_backoff =
        WebSocketReconnectBackoff::new(WS_RECONNECT_INITIAL_DELAY, WS_RECONNECT_MAX_DELAY);

    'follow: loop {
        if *shutdown.borrow() {
            break;
        }

        let endpoint = proxy_runtime_endpoint(&access.borrow_and_update());
        let Some(endpoint) = endpoint else {
            reconnect_backoff.reset();
            if wait_for_core_change(&mut access, &mut shutdown, WS_RECONNECT_MAX_DELAY).await
                == CoreWait::Shutdown
            {
                break;
            }
            continue;
        };

        let client = ClashWebSocketClient::new(endpoint);
        match time::timeout(WS_CONNECT_TIMEOUT, client.connect(resource)).await {
            Ok(Ok(mut session)) => loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return;
                        }
                    }
                    changed = access.changed() => {
                        if changed.is_ok() {
                            // A different core, or none: this socket belongs
                            // to the previous one.
                            reconnect_backoff.reset();
                            continue 'follow;
                        }
                        // The supervisor is gone, so nothing will ever change
                        // again; fall back to the reconnect delay.
                        break;
                    }
                    event = session.next_event() => match event {
                        Ok(event) => {
                            reconnect_backoff.reset();
                            on_event(event).await;
                        }
                        Err(error) => {
                            tracing::debug!(?error, ?resource, "core websocket read failed");
                            break;
                        }
                    }
                }
            },
            Ok(Err(error)) => {
                tracing::debug!(?error, ?resource, "failed to connect core websocket");
            }
            Err(error) => {
                tracing::debug!(?error, ?resource, "timed out connecting core websocket");
            }
        }

        // A restart drops the old core's socket before the new core is
        // published, so the retries in between dial an endpoint that is gone
        // and the delay grows. The delay therefore ends the moment the
        // supervisor publishes a core: sleeping it out would leave the new
        // core's first seconds of traffic unread.
        match wait_for_core_change(&mut access, &mut shutdown, reconnect_backoff.next_delay()).await
        {
            CoreWait::Shutdown => break,
            CoreWait::Changed => reconnect_backoff.reset(),
            CoreWait::Elapsed => {}
        }
    }
}

/// Why [`wait_for_core_change`] returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoreWait {
    Shutdown,
    /// The supervisor started, stopped or replaced a core.
    Changed,
    /// Nothing moved within the fallback.
    Elapsed,
}

/// Waits for the supervisor to start or stop something, for at most
/// `fallback`. Every state change passes through the supervisor's actor, so
/// nothing is polled once a second while the core is stopped; the fallback is
/// a safety net there, the reconnect delay after a failed dial, and the only
/// wake left once the supervisor itself is gone.
async fn wait_for_core_change<T>(
    changes: &mut watch::Receiver<T>,
    shutdown: &mut watch::Receiver<bool>,
    fallback: Duration,
) -> CoreWait {
    let slept = |stopped| {
        if stopped {
            CoreWait::Shutdown
        } else {
            CoreWait::Elapsed
        }
    };
    tokio::select! {
        changed = changes.changed() => {
            if changed.is_err() {
                return slept(sleep_or_shutdown(fallback, shutdown).await);
            }
            if *shutdown.borrow() {
                CoreWait::Shutdown
            } else {
                CoreWait::Changed
            }
        }
        stopped = sleep_or_shutdown(fallback, shutdown) => slept(stopped),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The loop parks here whenever no core is running. Waking on the
    /// supervisor's watch is what replaced a once-a-second `status()` poll, so
    /// the wake has to be prompt and the fallback has to stay a fallback.
    #[tokio::test(start_paused = true)]
    async fn core_wait_wakes_on_a_supervisor_change() {
        let (changes_tx, mut changes) = watch::channel(ClashApiAccess::default());
        let (_shutdown_tx, mut shutdown) = watch::channel(false);

        // What the supervisor's actor does after it starts a core.
        changes_tx.send_replace(ClashApiAccess::unauthenticated(9090));

        let started = time::Instant::now();
        let woke = wait_for_core_change(&mut changes, &mut shutdown, WS_RECONNECT_MAX_DELAY).await;

        assert_eq!(woke, CoreWait::Changed, "a core change is not a shutdown");
        assert_eq!(
            started.elapsed(),
            Duration::ZERO,
            "the change must wake the loop, not the fallback timer"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn core_wait_returns_promptly_on_shutdown() {
        let (_changes_tx, mut changes) = watch::channel(ClashApiAccess::default());
        let (shutdown_tx, mut shutdown) = watch::channel(false);

        shutdown_tx.send_replace(true);

        let started = time::Instant::now();
        let woke = wait_for_core_change(&mut changes, &mut shutdown, WS_RECONNECT_MAX_DELAY).await;

        assert_eq!(woke, CoreWait::Shutdown, "shutdown must break the loop");
        assert_eq!(started.elapsed(), Duration::ZERO);
    }

    /// Once the supervisor is gone the watch can never move again, so the only
    /// thing left to do is wait — not spin on a closed channel.
    #[tokio::test(start_paused = true)]
    async fn core_wait_falls_back_when_the_supervisor_is_gone() {
        let (changes_tx, mut changes) = watch::channel(ClashApiAccess::default());
        let (_shutdown_tx, mut shutdown) = watch::channel(false);

        drop(changes_tx);

        let started = time::Instant::now();
        let woke = wait_for_core_change(&mut changes, &mut shutdown, WS_RECONNECT_MAX_DELAY).await;

        assert_eq!(woke, CoreWait::Elapsed);
        assert!(
            started.elapsed() >= WS_RECONNECT_MAX_DELAY,
            "a closed change channel must wait, not busy-loop: {:?}",
            started.elapsed()
        );
    }

    /// After a failed dial the loop waits out a reconnect delay. A core
    /// published during that delay is the one worth dialling, so the wait
    /// ends there instead of running the rest of the delay against the old
    /// endpoint — and an undisturbed delay still runs its full length.
    #[tokio::test(start_paused = true)]
    async fn reconnect_delay_ends_when_a_core_is_published() {
        let (changes_tx, mut changes) = watch::channel(ClashApiAccess::unauthenticated(9090));
        let (_shutdown_tx, mut shutdown) = watch::channel(false);
        let delay = Duration::from_secs(16);

        let started = time::Instant::now();
        let publish = tokio::spawn(async move {
            time::sleep(Duration::from_secs(2)).await;
            changes_tx.send_replace(ClashApiAccess::unauthenticated(9091));
            changes_tx
        });
        let woke = wait_for_core_change(&mut changes, &mut shutdown, delay).await;
        assert_eq!(woke, CoreWait::Changed);
        assert_eq!(started.elapsed(), Duration::from_secs(2));

        let _changes_tx = publish.await.expect("the publisher does not panic");
        let started = time::Instant::now();
        let woke = wait_for_core_change(&mut changes, &mut shutdown, delay).await;
        assert_eq!(woke, CoreWait::Elapsed);
        assert_eq!(started.elapsed(), delay);
    }

    /// The reason a core is identified by its access and not by a process id:
    /// with nothing to dial the follower parks, and it leaves that park the
    /// moment a core is published — whether or not that core has a pid.
    #[tokio::test(start_paused = true)]
    async fn follower_dials_nothing_until_a_core_is_published_and_stops_on_shutdown() {
        let (_access_tx, access) = watch::channel(ClashApiAccess::default());
        let (shutdown_tx, shutdown) = watch::channel(false);

        let follower = tokio::spawn(follow_core_ws(
            access,
            ClashWebSocketResource::Traffic,
            shutdown,
            |_event| async {},
        ));
        time::sleep(WS_RECONNECT_MAX_DELAY * 3).await;
        assert!(!follower.is_finished(), "no core is not a reason to exit");

        shutdown_tx.send_replace(true);
        time::timeout(Duration::from_secs(1), follower)
            .await
            .expect("shutdown ends the follower")
            .expect("the follower does not panic");
    }
}
