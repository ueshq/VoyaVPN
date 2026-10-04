//! What a settings save does besides storing the bundle: the login entry.

use voya_contracts as contracts;
use voya_core::AppConfig;

// Kept at this path for the hosts, which read the settings bundle through it.
pub use crate::contract_map::settings_from_app_config;

/// The one OS side effect a settings save has: the login entry. A trait so the
/// save transaction's rollback paths can be tested without touching the machine.
///
/// `Clone + Send + 'static` because the call forks an OS helper, so the save
/// runs it on a blocking thread rather than on the async worker.
pub trait ApplyAutostart: Clone + Send + Sync + 'static {
    fn apply_autostart(&self, config: &AppConfig) -> Result<(), contracts::AppError>;
}

impl ApplyAutostart for crate::autostart::AutostartManager {
    fn apply_autostart(&self, config: &AppConfig) -> Result<(), contracts::AppError> {
        self.set_enabled(config.behavior.autostart)
            .map_err(contracts::AppError::from)
    }
}

/// A host without a login entry. A phone's always-on VPN is a system setting
/// the user turns on, not something an app arranges for itself.
#[derive(Clone)]
pub struct NoAutostart;

impl ApplyAutostart for NoAutostart {
    fn apply_autostart(&self, _config: &AppConfig) -> Result<(), contracts::AppError> {
        Ok(())
    }
}

/// Whether saving `target` over `original` has to rewrite the login entry.
///
/// The entry carries the launch flag `start_minimized` is read against, so an
/// enabled entry is also rewritten when that option changes.
#[must_use]
pub fn autostart_changes(original: &AppConfig, target: &AppConfig) -> bool {
    original.behavior.autostart != target.behavior.autostart
        || (target.behavior.autostart
            && original.behavior.start_minimized != target.behavior.start_minimized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(autostart: bool) -> AppConfig {
        let mut config = AppConfig::default();
        config.behavior.autostart = autostart;
        config
    }

    #[test]
    fn toggling_start_minimized_rewrites_only_an_enabled_login_entry() {
        let original = config(true);
        let mut target = original.clone();
        target.behavior.start_minimized = true;
        assert!(autostart_changes(&original, &target));

        let original = config(false);
        let mut target = original.clone();
        target.behavior.start_minimized = true;
        assert!(!autostart_changes(&original, &target));
    }
}
