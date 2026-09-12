//! Applied snapshots belong to the runtime, not a renderer session. A successful
//! operation acknowledges only its captured configuration, never a later save.
use crate::settings_save::saved_config_requires_runtime_restart;
use std::sync::{Arc, Mutex};
use voya_contracts::{SettingsApplyAction, SettingsApplyStatus};
use voya_core::AppConfig;

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
            core.proxy_ui_item.traffic_mode = mode;
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
            .is_none_or(|proxy| proxy.system_proxy_item != saved.system_proxy_item)
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

    #[test]
    fn saves_and_failed_applies_do_not_acknowledge_runtime_changes() {
        let tracker = SettingsApplication::default();
        let initial = AppConfig::default();
        tracker.core_applied(&initial);
        tracker.proxy_applied(&initial);
        let mut saved = initial.clone();
        saved.inbound[0].local_port += 1;
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
        latest.inbound[0].local_port += 1;
        tracker.core_applied(&captured);
        tracker.proxy_applied(&captured);
        assert_eq!(
            tracker.status(&latest, true).action,
            SettingsApplyAction::Reconnect
        );
        tracker.core_applied(&latest);
        latest.system_proxy_item.custom_system_proxy_pac_path = Some("proxy.pac".into());
        assert_eq!(
            tracker.status(&latest, true).action,
            SettingsApplyAction::ReapplyProxy
        );
        tracker.proxy_applied(&latest);
        latest.ui_item.current_language = "zh-Hans".into();
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
        saved.proxy_ui_item.traffic_mode = voya_core::TrafficMode::Global;
        tracker.traffic_mode_applied(saved.proxy_ui_item.traffic_mode);
        assert_eq!(
            tracker.status(&saved, true).action,
            SettingsApplyAction::None
        );
        saved.inbound[0].local_port += 1;
        saved.proxy_ui_item.traffic_mode = voya_core::TrafficMode::Rule;
        tracker.traffic_mode_applied(saved.proxy_ui_item.traffic_mode);
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
