//! Applied snapshots belong to the runtime, not a renderer session. A successful
//! operation acknowledges only its captured configuration, never a later save.
use std::sync::{Arc, Mutex};
use voya_contracts::{SettingsApplyAction, SettingsApplyStatus};
use voya_core::AppConfig;

/// Whether the saved configuration changed something the running core reads
/// from its generated config. Only generation inputs belong here: fields the
/// UI alone consumes (node sorting, appearance) must not interrupt traffic.
#[must_use]
pub(crate) fn saved_config_requires_runtime_restart(
    original: &AppConfig,
    updated: &AppConfig,
) -> bool {
    original.active_profile_id != updated.active_profile_id
        // A group leaves the profile id empty, so one group to another is only
        // visible here.
        || original.active_group_id != updated.active_group_id
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

#[derive(Debug, Default)]
struct Applied {
    core: Option<AppConfig>,
    proxy: Option<AppConfig>,
}

#[derive(Debug, Clone, Default)]
pub struct SettingsApplication {
    applied: Arc<Mutex<Applied>>,
    pub(crate) flow_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SettingsApplication {
    pub(crate) fn core_applied(&self, config: &AppConfig) {
        self.applied
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .core = Some(config.clone());
    }

    pub(crate) fn proxy_applied(&self, config: &AppConfig) {
        self.applied
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .proxy = Some(config.clone());
    }

    /// The configuration the running core was started from. A saved change
    /// waits for an apply, so this — not the saved settings — says which port
    /// the core listens on.
    #[must_use]
    pub fn applied_core(&self) -> Option<AppConfig> {
        self.applied
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .core
            .clone()
    }

    /// The saved configuration with the running core's inbounds: what a
    /// request through the local proxy has to dial while a port change is
    /// still waiting to be applied.
    #[must_use]
    pub fn running_core_config(&self, saved: &AppConfig) -> AppConfig {
        let mut config = saved.clone();
        if let Some(core) = self.applied_core() {
            config.inbounds = core.inbounds;
        }
        config
    }

    pub(crate) fn proxy_failed(&self) {
        self.applied
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .proxy = None;
    }

    pub(crate) fn traffic_mode_applied(&self, mode: voya_core::TrafficMode) {
        if let Some(core) = self
            .applied
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .core
            .as_mut()
        {
            core.proxy.traffic_mode = mode;
        }
    }

    #[must_use]
    pub fn status(&self, saved: &AppConfig, connected: bool) -> SettingsApplyStatus {
        let applied = self
            .applied
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let action = if !connected {
            SettingsApplyAction::None
        } else if applied
            .core
            .as_ref()
            .is_none_or(|core| saved_config_requires_runtime_restart(core, saved))
        {
            SettingsApplyAction::Reconnect
        } else if applied
            .proxy
            .as_ref()
            .is_none_or(|proxy| proxy.system_proxy != saved.system_proxy)
        {
            SettingsApplyAction::ReapplyProxy
        } else {
            SettingsApplyAction::None
        };
        SettingsApplyStatus { action, connected }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_map::{config_from_settings, settings_from_app_config};
    use voya_core::TrafficMode;

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
    fn runtime_restart_policy_tracks_a_switch_between_policy_groups() {
        let original = AppConfig {
            active_group_id: "group-a".to_string(),
            ..AppConfig::default()
        };
        let mut other = original.clone();
        other.active_group_id = "group-b".to_string();
        assert!(saved_config_requires_runtime_restart(&original, &other));
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

    #[test]
    fn the_running_core_keeps_its_inbounds_until_a_saved_port_is_applied() {
        let tracker = SettingsApplication::default();
        let started = AppConfig::default();
        let mut saved = started.clone();
        saved.inbounds[0].local_port += 1;
        saved.appearance.language = "zh-Hans".to_string();
        // Nothing running yet: the saved settings are all there is.
        assert_eq!(tracker.running_core_config(&saved), saved);

        tracker.core_applied(&started);
        let running = tracker.running_core_config(&saved);
        assert_eq!(running.local_port(), started.local_port());
        assert_eq!(running.appearance.language, "zh-Hans");
    }

    #[test]
    fn saves_and_failed_applies_do_not_acknowledge_runtime_changes() {
        let tracker = SettingsApplication::default();
        let initial = AppConfig::default();
        tracker.core_applied(&initial);
        tracker.proxy_applied(&initial);
        let mut saved = initial.clone();
        saved.inbounds[0].local_port += 1;
        assert_eq!(
            tracker.status(&saved, true).action,
            SettingsApplyAction::Reconnect
        );
        // A renderer reload sees the same application-owned snapshots.
        assert_eq!(
            tracker.clone().status(&saved, true).action,
            SettingsApplyAction::Reconnect
        );
        assert_eq!(
            tracker.status(&saved, false).action,
            SettingsApplyAction::None
        );
        tracker.core_applied(&saved);
        tracker.proxy_applied(&saved);
        assert_eq!(
            tracker.status(&saved, true).action,
            SettingsApplyAction::None
        );
    }

    #[test]
    fn changes_during_apply_and_proxy_failures_remain_pending() {
        let tracker = SettingsApplication::default();
        let captured = AppConfig::default();
        let mut latest = captured.clone();
        latest.inbounds[0].local_port += 1;
        tracker.core_applied(&captured);
        tracker.proxy_applied(&captured);
        assert_eq!(
            tracker.status(&latest, true).action,
            SettingsApplyAction::Reconnect
        );
        tracker.core_applied(&latest);
        latest.system_proxy.exceptions = "localhost".into();
        assert_eq!(
            tracker.status(&latest, true).action,
            SettingsApplyAction::ReapplyProxy
        );
        tracker.proxy_applied(&latest);
        latest.appearance.language = "zh-Hans".into();
        assert_eq!(
            tracker.status(&latest, true).action,
            SettingsApplyAction::None
        );
    }

    #[test]
    fn live_traffic_mode_acknowledges_only_that_field() {
        let tracker = SettingsApplication::default();
        let mut saved = AppConfig::default();
        tracker.core_applied(&saved);
        tracker.proxy_applied(&saved);
        saved.proxy.traffic_mode = TrafficMode::Global;
        tracker.traffic_mode_applied(saved.proxy.traffic_mode);
        assert_eq!(
            tracker.status(&saved, true).action,
            SettingsApplyAction::None
        );
        saved.inbounds[0].local_port += 1;
        saved.proxy.traffic_mode = TrafficMode::Rule;
        tracker.traffic_mode_applied(saved.proxy.traffic_mode);
        assert_eq!(
            tracker.status(&saved, true).action,
            SettingsApplyAction::Reconnect
        );
        tracker.core_applied(&saved);
        tracker.proxy_failed();
        assert_eq!(
            tracker.status(&saved, true).action,
            SettingsApplyAction::ReapplyProxy
        );
    }
}
