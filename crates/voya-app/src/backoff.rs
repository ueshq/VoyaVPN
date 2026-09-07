//! Shared reconnect backoff for the Clash-compatible websocket monitors.
//!
//! The statistics aggregator and the proxy runtime monitor both reconnect to
//! sing-box's websocket endpoints, so the jittered exponential schedule and the
//! shutdown-aware wait live here instead of being tuned in one copy only. The
//! per-manager delay bounds stay with their managers.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::{sync::watch, time};

/// Fraction of the current delay that may be added as jitter, so several
/// monitors reconnecting after the same core restart spread out.
const WS_RECONNECT_JITTER_DIVISOR: u32 = 4;

/// Exponential reconnect schedule: doubles per attempt, saturates at `max`,
/// and adds jitter on top. `reset()` is called once a connection succeeds.
#[derive(Debug, Clone)]
pub(crate) struct WebSocketReconnectBackoff {
    attempt: u32,
    initial: Duration,
    max: Duration,
}

impl WebSocketReconnectBackoff {
    pub(crate) const fn new(initial: Duration, max: Duration) -> Self {
        Self {
            attempt: 0,
            initial,
            max,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.attempt = 0;
    }

    pub(crate) fn next_delay(&mut self) -> Duration {
        let delay = websocket_reconnect_delay(
            self.attempt,
            self.initial,
            self.max,
            reconnect_jitter_seed(),
        );
        self.attempt = self.attempt.saturating_add(1);
        delay
    }
}

/// Waits for `duration` unless shutdown is requested first. Returns `true` when
/// the caller should stop looping.
pub(crate) async fn sleep_or_shutdown(
    duration: Duration,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    let sleep = time::sleep(duration);
    tokio::pin!(sleep);

    tokio::select! {
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
        _ = &mut sleep => false,
    }
}

fn websocket_reconnect_delay(
    attempt: u32,
    initial: Duration,
    max: Duration,
    jitter_seed: u64,
) -> Duration {
    let multiplier = 1_u32.checked_shl(attempt.min(16)).unwrap_or(u32::MAX);
    let scaled = initial.saturating_mul(multiplier);
    let base = if scaled > max { max } else { scaled };

    base.saturating_add(reconnect_jitter(base, jitter_seed))
}

fn reconnect_jitter(base: Duration, jitter_seed: u64) -> Duration {
    let jitter_limit_nanos =
        (base.as_nanos() / u128::from(WS_RECONNECT_JITTER_DIVISOR)).min(u128::from(u64::MAX));
    if jitter_limit_nanos == 0 {
        return Duration::ZERO;
    }

    let jitter_nanos = u128::from(jitter_seed) % (jitter_limit_nanos + 1);
    Duration::from_nanos(u64::try_from(jitter_nanos).unwrap_or(u64::MAX))
}

fn reconnect_jitter_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX) ^ u64::from(std::process::id())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_reconnect_delay_backs_off_with_cap_and_jitter() {
        let initial = Duration::from_secs(1);
        let max = Duration::from_secs(8);

        let first = websocket_reconnect_delay(0, initial, max, 0);
        let second = websocket_reconnect_delay(1, initial, max, 0);
        let capped = websocket_reconnect_delay(12, initial, max, u64::MAX);

        assert_eq!(first, Duration::from_secs(1));
        assert_eq!(second, Duration::from_secs(2));
        assert!(capped >= max);
        assert!(capped <= max + Duration::from_secs(2));
    }

    #[test]
    fn a_zero_delay_never_panics_and_adds_no_jitter() {
        assert_eq!(
            websocket_reconnect_delay(0, Duration::ZERO, Duration::ZERO, u64::MAX),
            Duration::ZERO
        );
    }

    #[tokio::test]
    async fn shutdown_interrupts_the_wait() {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        shutdown_tx.send(true).expect("shutdown receiver is alive");

        assert!(sleep_or_shutdown(Duration::from_secs(60), &mut shutdown_rx).await);
    }
}
