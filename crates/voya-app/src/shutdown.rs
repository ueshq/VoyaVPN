//! Exit teardown coordination.

use std::sync::atomic::{AtomicBool, Ordering};

/// One-shot latch guarding the application's exit teardown.
///
/// Tauri raises `RunEvent::ExitRequested` and then `RunEvent::Exit` for every
/// exit path, and a tray "Quit" item that calls `AppHandle::exit` adds a third
/// pass. The teardown re-runs privileged and OS-global work — the sudoers
/// revoke and the per-service `networksetup`/registry proxy restore — so it
/// must execute exactly once no matter how many times it is invoked.
#[derive(Debug, Default)]
pub struct ShutdownLatch {
    started: AtomicBool,
}

impl ShutdownLatch {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            started: AtomicBool::new(false),
        }
    }

    /// Claim the teardown. Returns `true` for exactly one caller, ever.
    pub fn begin(&self) -> bool {
        !self.started.swap(true, Ordering::SeqCst)
    }

    /// Whether the teardown has already been claimed.
    #[must_use]
    pub fn started(&self) -> bool {
        self.started.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread};

    use super::*;

    #[test]
    fn only_the_first_caller_claims_the_teardown() {
        let latch = ShutdownLatch::new();

        assert!(!latch.started());
        assert!(latch.begin());
        assert!(latch.started());
        assert!(!latch.begin());
        assert!(!latch.begin());
    }

    #[test]
    fn concurrent_exit_paths_produce_exactly_one_winner() {
        let latch = Arc::new(ShutdownLatch::new());
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let latch = Arc::clone(&latch);
                thread::spawn(move || latch.begin())
            })
            .collect();

        let winners = handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .filter(|claimed| *claimed)
            .count();

        assert_eq!(winners, 1);
    }
}
