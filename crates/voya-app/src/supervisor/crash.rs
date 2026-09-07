//! Crash-restart bookkeeping for the core supervisor.
//!
//! A core that dies immediately on every spawn — a listening port already
//! bound, a rule-set file that is not readable, a config removed by a racing
//! disconnect — used to be respawned by the actor as fast as the reaper could
//! report the exit. This module bounds that loop and gives the shell the
//! vocabulary to tell the user what happened, so the UI can never sit on
//! "Connected" while the core churns or stays down.

use std::{
    fmt,
    time::{Duration, Instant},
};

use super::SupervisorSnapshot;

/// Monotonic time source for the crash bookkeeping.
///
/// Injected so the uptime window can be exercised without sleeping.
pub trait SupervisorClock: Send + Sync {
    fn now(&self) -> Instant;
}

/// The process clock used in production.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemSupervisorClock;

impl SupervisorClock for SystemSupervisorClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Bounds the crash-restart loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashRestartPolicy {
    /// Restarts allowed inside one crash streak before the supervisor gives up.
    pub max_restarts: u32,
    /// Uptime that clears the streak: a core that ran this long died for a new
    /// reason rather than because it never managed to start.
    pub healthy_uptime: Duration,
    /// Delay before the second restart of a streak; doubles per attempt.
    pub initial_delay: Duration,
    /// Ceiling for the doubled delay.
    pub max_delay: Duration,
}

impl Default for CrashRestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 3,
            healthy_uptime: Duration::from_secs(30),
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(8),
        }
    }
}

impl CrashRestartPolicy {
    /// Delay before restart `attempt` (1-based).
    ///
    /// The first restart of a streak is immediate, so an isolated crash still
    /// recovers without a visible stall; every later attempt backs off.
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        if attempt <= 1 {
            return Duration::ZERO;
        }

        let multiplier = 1_u32
            .checked_shl(attempt.saturating_sub(2).min(16))
            .unwrap_or(u32::MAX);
        let scaled = self.initial_delay.saturating_mul(multiplier);
        if scaled > self.max_delay {
            self.max_delay
        } else {
            scaled
        }
    }

    /// Whether `attempt` is still inside the budget.
    #[must_use]
    pub const fn allows(&self, attempt: u32) -> bool {
        attempt <= self.max_restarts
    }
}

/// Consecutive-crash bookkeeping for the currently running core.
#[derive(Debug, Default)]
pub struct CrashTracker {
    restarts: u32,
    last_start: Option<Instant>,
}

impl CrashTracker {
    /// Record that a core was (re)spawned.
    pub fn record_start(&mut self, now: Instant) {
        self.last_start = Some(now);
    }

    /// Start a fresh streak, called for every user-initiated start.
    pub fn reset(&mut self) {
        self.restarts = 0;
        self.last_start = None;
    }

    /// Number this crash's restart would carry.
    ///
    /// A core that stayed up for `healthy_uptime` opens a new streak, so an
    /// app that runs for hours is not held hostage by crashes from last week.
    pub fn next_attempt(&mut self, now: Instant, healthy_uptime: Duration) -> u32 {
        let stayed_healthy = self
            .last_start
            .is_some_and(|start| now.saturating_duration_since(start) >= healthy_uptime);
        if stayed_healthy {
            self.restarts = 0;
        }
        self.restarts = self.restarts.saturating_add(1);
        self.restarts
    }
}

/// Why the supervisor stopped trying to keep the core alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreExitGiveUp {
    /// The core exited with status 0, which is never treated as a crash.
    IntentionalExit,
    /// The start request did not ask for crash restarts.
    RestartNotRequested,
    /// The crash budget is exhausted.
    CrashLoop { restarts: u32 },
    /// Tearing down what was left of the exited core failed.
    StopFailed(String),
    /// The respawn itself failed.
    RestartFailed(String),
}

impl fmt::Display for CoreExitGiveUp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntentionalExit => formatter.write_str("the core exited normally"),
            Self::RestartNotRequested => {
                formatter.write_str("automatic restart is disabled for this session")
            }
            Self::CrashLoop { restarts } => write!(
                formatter,
                "the core kept exiting after {restarts} automatic restarts"
            ),
            Self::StopFailed(error) => {
                write!(formatter, "stopping the exited core failed: {error}")
            }
            Self::RestartFailed(error) => {
                write!(formatter, "restarting the core failed: {error}")
            }
        }
    }
}

/// What the supervisor did about a tracked core process exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreExitOutcome {
    /// The core was respawned; `snapshot` describes the replacement.
    Restarted {
        attempt: u32,
        snapshot: SupervisorSnapshot,
    },
    /// A restart is waiting on its backoff timer. A second event reports how
    /// that restart ended.
    RestartScheduled { attempt: u32, delay: Duration },
    /// The core stays down.
    GaveUp(CoreExitGiveUp),
}

impl From<CoreExitGiveUp> for CoreExitOutcome {
    fn from(reason: CoreExitGiveUp) -> Self {
        Self::GaveUp(reason)
    }
}

/// A tracked core process exited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreExitEvent {
    pub active_profile_id: Option<String>,
    pub process_id: u32,
    pub exit_code: Option<i32>,
    pub outcome: CoreExitOutcome,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_restart_is_immediate_and_later_ones_back_off_to_the_cap() {
        let policy = CrashRestartPolicy::default();

        assert_eq!(policy.delay_for(1), Duration::ZERO);
        assert_eq!(policy.delay_for(2), Duration::from_secs(1));
        assert_eq!(policy.delay_for(3), Duration::from_secs(2));
        assert_eq!(policy.delay_for(4), Duration::from_secs(4));
        assert_eq!(policy.delay_for(64), policy.max_delay);
    }

    #[test]
    fn the_budget_rejects_the_restart_after_the_last_allowed_one() {
        let policy = CrashRestartPolicy::default();

        assert!(policy.allows(policy.max_restarts));
        assert!(!policy.allows(policy.max_restarts + 1));
    }

    #[test]
    fn rapid_crashes_accumulate_until_a_healthy_run_clears_the_streak() {
        let base = Instant::now();
        let healthy = Duration::from_secs(30);
        let mut tracker = CrashTracker::default();

        tracker.record_start(base);
        assert_eq!(
            tracker.next_attempt(base + Duration::from_millis(50), healthy),
            1
        );
        tracker.record_start(base + Duration::from_millis(60));
        assert_eq!(
            tracker.next_attempt(base + Duration::from_millis(120), healthy),
            2
        );

        tracker.record_start(base + Duration::from_millis(130));
        assert_eq!(
            tracker.next_attempt(base + Duration::from_secs(600), healthy),
            1
        );
    }

    #[test]
    fn a_user_initiated_start_clears_the_streak() {
        let base = Instant::now();
        let healthy = Duration::from_secs(30);
        let mut tracker = CrashTracker::default();

        tracker.record_start(base);
        assert_eq!(tracker.next_attempt(base, healthy), 1);
        assert_eq!(tracker.next_attempt(base, healthy), 2);

        tracker.reset();
        tracker.record_start(base);
        assert_eq!(tracker.next_attempt(base, healthy), 1);
    }

    #[test]
    fn give_up_reasons_read_as_sentences() {
        assert_eq!(
            CoreExitGiveUp::CrashLoop { restarts: 3 }.to_string(),
            "the core kept exiting after 3 automatic restarts"
        );
        assert_eq!(
            CoreExitGiveUp::RestartFailed("spawn failed".to_string()).to_string(),
            "restarting the core failed: spawn failed"
        );
    }
}
