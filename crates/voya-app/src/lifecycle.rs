//! What closing the window, launching at login and exiting do, decided without
//! Tauri.

use std::sync::atomic::{AtomicBool, Ordering};

use voya_contracts::CloseRequestAction;
use voya_core::{AppConfig, CloseAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// Hide the window; the app keeps running from the tray.
    Hide,
    Quit,
    /// Let the renderer ask the user.
    Ask,
}

/// What a close request on the main window does.
///
/// Without a tray icon a hidden window could never be brought back, so every
/// close quits in that case whatever the setting says.
#[must_use]
pub fn close_request_decision(config: &AppConfig, tray_available: bool) -> CloseDecision {
    if !tray_available {
        return CloseDecision::Quit;
    }
    match config.gui_item.close_action {
        CloseAction::MinimizeToTray => CloseDecision::Hide,
        CloseAction::Quit => CloseDecision::Quit,
        CloseAction::Ask => CloseDecision::Ask,
    }
}

/// Whether the main window stays hidden for this launch. Only a login launch
/// honours `start_minimized`: a user who opens the app expects to see it.
#[must_use]
pub fn launch_hidden(config: &AppConfig, launched_by_autostart: bool) -> bool {
    launched_by_autostart && config.gui_item.start_minimized
}

/// Stores an answer to the close prompt as the new close action. Returns
/// whether the configuration changed; a cancelled prompt never does.
pub fn remember_close_action(config: &mut AppConfig, action: CloseRequestAction) -> bool {
    let remembered = match action {
        CloseRequestAction::MinimizeToTray => CloseAction::MinimizeToTray,
        CloseRequestAction::Quit => CloseAction::Quit,
        CloseRequestAction::Cancel => return false,
    };
    let changed = config.gui_item.close_action != remembered;
    config.gui_item.close_action = remembered;
    changed
}

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
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread};

    use super::*;

    fn config(close_action: CloseAction) -> AppConfig {
        let mut config = AppConfig::default();
        config.gui_item.close_action = close_action;
        config
    }

    #[test]
    fn close_follows_the_setting_while_a_tray_exists() {
        for (action, decision) in [
            (CloseAction::MinimizeToTray, CloseDecision::Hide),
            (CloseAction::Quit, CloseDecision::Quit),
            (CloseAction::Ask, CloseDecision::Ask),
        ] {
            assert_eq!(close_request_decision(&config(action), true), decision);
            assert_eq!(
                close_request_decision(&config(action), false),
                CloseDecision::Quit
            );
        }
    }

    #[test]
    fn only_a_login_launch_starts_hidden() {
        let mut config = AppConfig::default();
        assert!(!launch_hidden(&config, true));
        config.gui_item.start_minimized = true;
        assert!(launch_hidden(&config, true));
        assert!(!launch_hidden(&config, false));
    }

    #[test]
    fn a_remembered_answer_becomes_the_close_action() {
        let mut config = config(CloseAction::Ask);
        assert!(!remember_close_action(
            &mut config,
            CloseRequestAction::Cancel
        ));
        assert_eq!(config.gui_item.close_action, CloseAction::Ask);
        assert!(remember_close_action(&mut config, CloseRequestAction::Quit));
        assert_eq!(config.gui_item.close_action, CloseAction::Quit);
        assert!(!remember_close_action(
            &mut config,
            CloseRequestAction::Quit
        ));
        assert!(remember_close_action(
            &mut config,
            CloseRequestAction::MinimizeToTray
        ));
        assert_eq!(config.gui_item.close_action, CloseAction::MinimizeToTray);
    }

    #[test]
    fn only_the_first_caller_claims_the_teardown() {
        let latch = ShutdownLatch::new();

        // The latch starts unclaimed, so the first caller wins and every
        // later one — including the repeated exit passes — loses.
        assert!(latch.begin());
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
