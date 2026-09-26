//! What a settings save does besides storing the bundle: whether the running
//! core has to restart or the system proxy be re-applied, and the login entry.

use voya_contracts as contracts;
use voya_core::AppConfig;

// Kept at this path for the hosts, which read the settings bundle through it.
pub use crate::contract_map::settings_from_app_config;

/// Whether the saved configuration changed something the running core reads
/// from its generated config. Only generation inputs belong here: fields the
/// UI alone consumes (node sorting, appearance) must not interrupt traffic.
#[must_use]
pub fn saved_config_requires_runtime_restart(original: &AppConfig, updated: &AppConfig) -> bool {
    original.active_profile_id != updated.active_profile_id
        || original.core != updated.core
        || original.tun != updated.tun
        || original.active_routing_id != updated.active_routing_id
        || original.multiplexing != updated.multiplexing
        || original.hysteria != updated.hysteria
        // Traffic mode changes generated routing.
        || original.proxy.traffic_mode != updated.proxy.traffic_mode
        || original.inbounds != updated.inbounds
        || original.dns != updated.dns
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRuntimeAction {
    None,
    ReapplySystemProxy,
    Restart,
}

#[must_use]
pub const fn settings_runtime_action(
    runtime_restart_required: bool,
    system_proxy_reapply_required: bool,
) -> SettingsRuntimeAction {
    if runtime_restart_required {
        SettingsRuntimeAction::Restart
    } else if system_proxy_reapply_required {
        SettingsRuntimeAction::ReapplySystemProxy
    } else {
        SettingsRuntimeAction::None
    }
}

/// The one OS side effect a settings save has: the login entry. A trait so the
/// save transaction's rollback paths can be tested without touching the machine.
pub trait ApplyAutostart: Sync {
    fn apply_autostart(&self, config: &AppConfig) -> Result<(), contracts::AppError>;
}

impl ApplyAutostart for crate::autostart::AutostartManager {
    fn apply_autostart(&self, config: &AppConfig) -> Result<(), contracts::AppError> {
        let mut config = config.clone();
        let enabled = config.behavior.autostart;
        self.set_enabled(&mut config, enabled)
            .map(|_| ())
            .map_err(contracts::AppError::from)
    }
}

/// A host without a login entry. A phone's always-on VPN is a system setting
/// the user turns on, not something an app arranges for itself.
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
    use voya_core::TrafficMode;

    use super::*;
    use crate::contract_map::config_from_settings;

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

    #[test]
    fn restart_dominates_proxy_reapply_as_the_single_runtime_action() {
        assert_eq!(
            settings_runtime_action(true, true),
            SettingsRuntimeAction::Restart
        );
        assert_eq!(
            settings_runtime_action(false, true),
            SettingsRuntimeAction::ReapplySystemProxy
        );
        assert_eq!(
            settings_runtime_action(false, false),
            SettingsRuntimeAction::None
        );
    }

    #[test]
    fn runtime_restart_policy_ignores_appearance_only_changes() {
        let original = AppConfig::default();
        let mut appearance = original.clone();
        appearance.appearance.language = "zh-Hans".to_string();
        assert!(!saved_config_requires_runtime_restart(
            &original,
            &appearance
        ));

        let mut network = original.clone();
        network.inbounds[0].local_port += 1;
        assert!(saved_config_requires_runtime_restart(&original, &network));
    }

    #[test]
    fn runtime_restart_policy_tracks_traffic_mode() {
        let original = AppConfig::default();

        let mut mode = original.clone();
        mode.proxy.traffic_mode = TrafficMode::Global;
        assert!(saved_config_requires_runtime_restart(&original, &mode));
    }

    #[test]
    fn saving_unchanged_settings_never_requires_a_runtime_restart() {
        let mut original = AppConfig {
            active_profile_id: "profile-a".to_string(),
            active_group_id: String::new(),
            ..AppConfig::default()
        };
        original.active_routing_id = "routing-a".to_string();
        original.appearance.language = "zh-Hans".to_string();

        let target = config_from_settings(&settings_from_app_config(&original), &original);

        assert_eq!(target, original);
        assert!(!saved_config_requires_runtime_restart(&original, &target));
    }
}
