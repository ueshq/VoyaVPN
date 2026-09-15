use std::sync::{Arc, Mutex, RwLock};

use voya_db::Database;

use super::*;

/// Records the OS side effects in the order they were asked for, so the
/// compensation paths are observable without touching the machine.
#[derive(Clone, Default)]
struct RecordingSideEffects {
    calls: Arc<Mutex<Vec<String>>>,
    fail_autostart: bool,
}

impl RecordingSideEffects {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("side effect calls").clone()
    }

    fn push(&self, call: String) {
        self.calls.lock().expect("side effect calls").push(call);
    }
}

impl SettingsSideEffectAdapter for RecordingSideEffects {
    type Error = String;

    fn apply_autostart(&self, config: &AppConfig) -> Result<(), Self::Error> {
        self.push(format!("autostart:{}", config.gui_item.auto_run));
        if self.fail_autostart {
            return Err("autostart refused".to_string());
        }
        Ok(())
    }
}

struct Harness {
    coordinator: ConfigMutationCoordinator,
}

impl Harness {
    async fn new() -> Self {
        Self::with_config(AppConfig::default()).await
    }

    async fn with_config(config: AppConfig) -> Self {
        let database = Database::connect_in_memory()
            .await
            .expect("in-memory database");

        Self {
            coordinator: ConfigMutationCoordinator::new(database, Arc::new(RwLock::new(config))),
        }
    }

    fn stored(&self) -> AppConfig {
        self.coordinator.current_config()
    }
}

fn baseline() -> AppSettingsV1 {
    settings_from_app_config(&AppConfig::default())
}

#[tokio::test]
async fn a_ui_only_change_commits_without_touching_the_runtime() {
    let harness = Harness::new().await;
    let side_effects = RecordingSideEffects::default();
    let mut settings = baseline();
    settings.appearance.language = "zh-Hans".to_string();

    let outcome = save_app_settings(&harness.coordinator, &side_effects, &settings)
        .await
        .expect("ui-only save");

    assert_eq!(outcome.runtime_action, SettingsRuntimeAction::None);
    assert!(outcome.changed);
    assert_eq!(outcome.settings.appearance.language, "zh-Hans");
    assert_eq!(harness.stored().ui_item.current_language, "zh-Hans");
    // The autostart entry did not change, so it was not touched.
    assert!(side_effects.calls().is_empty());
}

#[tokio::test]
async fn a_generation_input_change_asks_for_a_core_restart() {
    let harness = Harness::new().await;
    let side_effects = RecordingSideEffects::default();
    let mut settings = baseline();
    settings.network.tun.mtu = 1400;

    let outcome = save_app_settings(&harness.coordinator, &side_effects, &settings)
        .await
        .expect("tun mtu save");

    assert_eq!(outcome.runtime_action, SettingsRuntimeAction::Restart);
    assert_eq!(harness.stored().tun_mode_item.mtu, 1400);
}

/// The system proxy is re-applied rather than restarting the core: nothing the
/// core reads from its generated config changed.
#[tokio::test]
async fn a_system_proxy_change_only_reapplies_the_proxy() {
    let harness = Harness::new().await;
    let side_effects = RecordingSideEffects::default();
    let mut settings = baseline();
    settings.network.system_proxy.exceptions = "example.test".to_string();

    let outcome = save_app_settings(&harness.coordinator, &side_effects, &settings)
        .await
        .expect("system proxy save");

    assert_eq!(
        outcome.runtime_action,
        SettingsRuntimeAction::ReapplySystemProxy
    );
    assert_eq!(
        harness.stored().system_proxy_item.system_proxy_exceptions,
        "example.test"
    );
}

#[tokio::test]
async fn saving_the_same_settings_reports_no_change() {
    let harness = Harness::new().await;
    let side_effects = RecordingSideEffects::default();

    let outcome = save_app_settings(&harness.coordinator, &side_effects, &baseline())
        .await
        .expect("no-op save");

    assert!(!outcome.changed);
    assert_eq!(outcome.runtime_action, SettingsRuntimeAction::None);
}

#[tokio::test]
async fn an_unsupported_schema_is_rejected_before_anything_runs() {
    let harness = Harness::new().await;
    let side_effects = RecordingSideEffects::default();
    let mut settings = baseline();
    settings.schema_version += 1;
    settings.behavior.autostart = true;

    let error = save_app_settings(&harness.coordinator, &side_effects, &settings)
        .await
        .expect_err("unsupported schema");

    assert!(matches!(error, SettingsSaveError::Validation(_)));
    assert!(side_effects.calls().is_empty());
    assert!(!harness.stored().gui_item.auto_run);
}

/// A refused autostart change must attempt to restore the OS entry and leave
/// the stored configuration untouched.
#[tokio::test]
async fn a_refused_side_effect_rolls_back_and_persists_nothing() {
    let harness = Harness::new().await;
    let side_effects = RecordingSideEffects {
        fail_autostart: true,
        ..RecordingSideEffects::default()
    };
    let mut settings = baseline();
    settings.behavior.autostart = true;

    let error = save_app_settings(&harness.coordinator, &side_effects, &settings)
        .await
        .expect_err("autostart refused");

    assert!(matches!(
        error,
        SettingsSaveError::SideEffect {
            stage: SettingsSideEffectStage::Autostart,
            ..
        }
    ));
    assert_eq!(
        side_effects.calls().as_slice(),
        ["autostart:true".to_string(), "autostart:false".to_string()]
    );
    assert!(!harness.stored().gui_item.auto_run);
}

/// The side effects run before the commit, so a failed commit has to undo them;
/// otherwise the app would launch at login for settings that were never stored.
#[tokio::test]
async fn a_failed_commit_rolls_back_the_applied_side_effects() {
    // `app_state.active_profile_id` is a foreign key onto `profile_items`, so
    // committing with an active profile that was never imported fails inside
    // the transaction — after the side effects have already been applied.
    let harness = Harness::with_config(AppConfig {
        index_id: "ghost-profile".to_string(),
        ..AppConfig::default()
    })
    .await;
    let side_effects = RecordingSideEffects::default();
    let mut settings = baseline();
    settings.behavior.autostart = true;

    let error = save_app_settings(&harness.coordinator, &side_effects, &settings)
        .await
        .expect_err("commit should fail on the dangling active profile");

    assert!(matches!(error, SettingsSaveError::Commit(_)));
    assert_eq!(
        side_effects.calls().as_slice(),
        ["autostart:true".to_string(), "autostart:false".to_string()]
    );
    assert!(!harness.stored().gui_item.auto_run);
}
