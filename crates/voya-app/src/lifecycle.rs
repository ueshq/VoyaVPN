//! What closing the window, launching at login and exiting do, decided without
//! Tauri.

use std::{future::Future, time::Duration};

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
    match config.behavior.close_action {
        CloseAction::MinimizeToTray => CloseDecision::Hide,
        CloseAction::Quit => CloseDecision::Quit,
        CloseAction::Ask => CloseDecision::Ask,
    }
}

/// Whether the main window stays hidden for this launch. Only a login launch
/// honours `start_minimized`: a user who opens the app expects to see it.
#[must_use]
pub fn launch_hidden(config: &AppConfig, launched_by_autostart: bool) -> bool {
    launched_by_autostart && config.behavior.start_minimized
}

/// Stores an answer to the close prompt as the new close action. Returns
/// whether the configuration changed; a cancelled prompt never does.
pub fn remember_close_action(config: &mut AppConfig, action: CloseRequestAction) -> bool {
    let remembered = match action {
        CloseRequestAction::MinimizeToTray => CloseAction::MinimizeToTray,
        CloseRequestAction::Quit => CloseAction::Quit,
        CloseRequestAction::Cancel => return false,
    };
    let changed = config.behavior.close_action != remembered;
    config.behavior.close_action = remembered;
    changed
}

/// Runs one step of the exit teardown, abandoning it after `limit`.
///
/// The teardown runs its steps one after another on the thread that is
/// quitting, and each waits on work owned by other tasks: a commit in flight,
/// a router that has stopped answering, a privileged kill. Every one of those
/// is bounded on its own, but a step that never returned would keep the app
/// from ever exiting, so each also gets a deadline here. `None` means the step
/// was abandoned; the caller decides what that leaves undone.
pub async fn exit_step<T>(
    what: &'static str,
    limit: Duration,
    step: impl Future<Output = T>,
) -> Option<T> {
    match tokio::time::timeout(limit, step).await {
        Ok(value) => Some(value),
        Err(_) => {
            tracing::error!(
                step = what,
                ?limit,
                "exit step did not finish in time; exiting without it"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(close_action: CloseAction) -> AppConfig {
        let mut config = AppConfig::default();
        config.behavior.close_action = close_action;
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
        config.behavior.start_minimized = true;
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
        assert_eq!(config.behavior.close_action, CloseAction::Ask);
        assert!(remember_close_action(&mut config, CloseRequestAction::Quit));
        assert_eq!(config.behavior.close_action, CloseAction::Quit);
        assert!(!remember_close_action(
            &mut config,
            CloseRequestAction::Quit
        ));
        assert!(remember_close_action(
            &mut config,
            CloseRequestAction::MinimizeToTray
        ));
        assert_eq!(config.behavior.close_action, CloseAction::MinimizeToTray);
    }
}
