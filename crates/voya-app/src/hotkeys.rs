use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use thiserror::Error;
use voya_core::{AppConfig, GlobalHotkey, KeyEventItem};
use voya_platform::hotkeys::{
    all_hotkey_actions, hotkey_registrations, normalize_key_event_items, HotkeyError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GlobalHotkeyBinding {
    pub action: GlobalHotkey,
    pub accelerator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GlobalHotkeyAction {
    pub action: GlobalHotkey,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    pub actions: Vec<GlobalHotkeyAction>,
    pub settings: Vec<KeyEventItem>,
    pub registered: Vec<GlobalHotkeyBinding>,
}

pub trait HotkeyRegistrar: Send + Sync {
    fn unregister_all(&self) -> Result<(), HotkeyManagerError>;
    fn register(&self, bindings: &[GlobalHotkeyBinding]) -> Result<(), HotkeyManagerError>;
}

#[derive(Clone)]
pub struct HotkeyManager {
    registrar: Arc<dyn HotkeyRegistrar>,
}

impl HotkeyManager {
    #[must_use]
    pub fn new(registrar: Arc<dyn HotkeyRegistrar>) -> Self {
        Self { registrar }
    }

    #[must_use]
    pub fn status(&self, config: &AppConfig) -> Result<HotkeyStatus, HotkeyManagerError> {
        status_from_settings(&config.global_hotkeys)
    }

    pub fn register_from_config(
        &self,
        config: &AppConfig,
    ) -> Result<HotkeyStatus, HotkeyManagerError> {
        let status = self.status(config)?;
        self.registrar.unregister_all()?;
        self.registrar.register(&status.registered)?;

        Ok(status)
    }

    pub fn save_settings(
        &self,
        config: &mut AppConfig,
        settings: Vec<KeyEventItem>,
    ) -> Result<HotkeyStatus, HotkeyManagerError> {
        config.global_hotkeys = normalize_key_event_items(&settings);
        self.register_from_config(config)
    }

    pub fn trigger_action(
        &self,
        action: GlobalHotkey,
        sink: &dyn HotkeyActionSink,
    ) -> Result<(), HotkeyManagerError> {
        if !all_hotkey_actions().contains(&action) {
            return Err(HotkeyManagerError::UnsupportedAction(action.as_i32()));
        }
        sink.handle(action)
    }
}

pub trait HotkeyActionSink {
    fn handle(&self, action: GlobalHotkey) -> Result<(), HotkeyManagerError>;
}

#[derive(Debug, Error)]
pub enum HotkeyManagerError {
    #[error(transparent)]
    Platform(#[from] HotkeyError),
    #[error("global hotkey registration failed: {0}")]
    Register(String),
    #[error("unsupported global hotkey action discriminant {0}")]
    UnsupportedAction(i32),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopHotkeyRegistrar;

impl HotkeyRegistrar for NoopHotkeyRegistrar {
    fn unregister_all(&self) -> Result<(), HotkeyManagerError> {
        Ok(())
    }

    fn register(&self, _bindings: &[GlobalHotkeyBinding]) -> Result<(), HotkeyManagerError> {
        Ok(())
    }
}

fn status_from_settings(settings: &[KeyEventItem]) -> Result<HotkeyStatus, HotkeyManagerError> {
    let normalized = normalize_key_event_items(settings);
    let registered = hotkey_registrations(&normalized)?
        .into_iter()
        .map(|registration| GlobalHotkeyBinding {
            action: registration.action,
            accelerator: registration.accelerator,
        })
        .collect();

    Ok(HotkeyStatus {
        actions: all_hotkey_actions()
            .iter()
            .copied()
            .map(|action| GlobalHotkeyAction {
                action,
                label: action_label(action).to_string(),
            })
            .collect(),
        settings: normalized,
        registered,
    })
}

const fn action_label(action: GlobalHotkey) -> &'static str {
    match action {
        GlobalHotkey::ShowForm => "Show window",
        GlobalHotkey::SystemProxyClear => "Clear system proxy",
        GlobalHotkey::SystemProxySet => "Set system proxy",
        GlobalHotkey::SystemProxyUnchanged => "Leave system proxy unchanged",
        GlobalHotkey::SystemProxyPac => "Set PAC proxy",
    }
}

#[cfg(test)]
mod hotkey_app_tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeHotkeyRegistrar {
        registered: Mutex<Vec<Vec<GlobalHotkeyBinding>>>,
        unregisters: Mutex<u32>,
    }

    impl HotkeyRegistrar for FakeHotkeyRegistrar {
        fn unregister_all(&self) -> Result<(), HotkeyManagerError> {
            *self.unregisters.lock().expect("unregisters") += 1;
            Ok(())
        }

        fn register(&self, bindings: &[GlobalHotkeyBinding]) -> Result<(), HotkeyManagerError> {
            self.registered
                .lock()
                .expect("registered")
                .push(bindings.to_vec());
            Ok(())
        }
    }

    struct FakeActionSink {
        actions: Mutex<Vec<GlobalHotkey>>,
    }

    impl HotkeyActionSink for FakeActionSink {
        fn handle(&self, action: GlobalHotkey) -> Result<(), HotkeyManagerError> {
            self.actions.lock().expect("actions").push(action);
            Ok(())
        }
    }

    #[test]
    fn hotkey_manager_registers_enabled_settings_with_fake_registrar() {
        let registrar = Arc::new(FakeHotkeyRegistrar::default());
        let manager = HotkeyManager::new(registrar.clone());
        let mut config = AppConfig::default();

        let status = manager
            .save_settings(
                &mut config,
                vec![
                    KeyEventItem {
                        global_hotkey: GlobalHotkey::ShowForm,
                        control: true,
                        alt: true,
                        shift: false,
                        key_code: Some(86),
                    },
                    KeyEventItem {
                        global_hotkey: GlobalHotkey::SystemProxyPac,
                        control: true,
                        alt: false,
                        shift: true,
                        key_code: Some(80),
                    },
                ],
            )
            .expect("save hotkeys");

        assert_eq!(status.actions.len(), 5);
        assert_eq!(config.global_hotkeys.len(), 5);
        assert_eq!(
            registrar.registered.lock().expect("registered")[0],
            vec![
                GlobalHotkeyBinding {
                    action: GlobalHotkey::ShowForm,
                    accelerator: "Ctrl+Alt+KeyV".to_string(),
                },
                GlobalHotkeyBinding {
                    action: GlobalHotkey::SystemProxyPac,
                    accelerator: "Ctrl+Shift+KeyP".to_string(),
                },
            ]
        );
        assert_eq!(*registrar.unregisters.lock().expect("unregisters"), 1);
    }

    #[test]
    fn hotkey_manager_dispatches_five_global_hotkey_actions() {
        let manager = HotkeyManager::new(Arc::new(NoopHotkeyRegistrar));
        let sink = FakeActionSink {
            actions: Mutex::new(Vec::new()),
        };

        for action in all_hotkey_actions() {
            manager
                .trigger_action(*action, &sink)
                .expect("trigger action");
        }

        assert_eq!(
            sink.actions.lock().expect("actions").as_slice(),
            all_hotkey_actions()
        );
    }

    #[test]
    fn hotkey_manager_rejects_duplicate_settings_before_registering() {
        let registrar = Arc::new(FakeHotkeyRegistrar::default());
        let manager = HotkeyManager::new(registrar.clone());
        let mut config = AppConfig::default();

        let error = manager
            .save_settings(
                &mut config,
                vec![
                    KeyEventItem {
                        global_hotkey: GlobalHotkey::ShowForm,
                        control: true,
                        alt: false,
                        shift: false,
                        key_code: Some(65),
                    },
                    KeyEventItem {
                        global_hotkey: GlobalHotkey::SystemProxySet,
                        control: true,
                        alt: false,
                        shift: false,
                        key_code: Some(65),
                    },
                ],
            )
            .expect_err("duplicate");

        assert!(matches!(
            error,
            HotkeyManagerError::Platform(HotkeyError::DuplicateAccelerator(_))
        ));
        assert!(registrar.registered.lock().expect("registered").is_empty());
    }
}
