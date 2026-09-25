//! The one transaction behind the `save_app_settings` command.
//!
//! Saving settings is not a single write: it maps the contract onto an
//! `AppConfig`, applies the OS autostart entry *before* the database commit so a
//! rejected registration cannot be persisted, rolls that change back when a
//! later step fails,
//! and finally decides whether the running core has to be restarted or the
//! system proxy re-applied.
//!
//! That sequence used to live in `ipc/commands/app.rs`, where the shell's lib
//! test harness is disabled, so the ordering and the two compensation paths
//! were never exercised. It lives here for the same reason `core_flow` and
//! `connection_mode` do: the pieces were already unit-tested individually, but
//! the order they run in is what decides whether a failed save leaves the
//! machine configured for settings that were never stored.
//!
//! What deliberately stays in the shell is the *dispatch* of
//! [`SettingsRuntimeAction`]: restarting the core and re-applying the system
//! proxy are `core_flow` concerns wired to Tauri event emission, and the
//! outcome names the action so that adapter is a `match` with no policy in it.

pub mod apply;
pub mod save;

use voya_contracts::AppSettings;
use voya_core::AppConfig;

use crate::{
    config_mutation::{ConfigMutationCoordinator, ConfigMutationError},
    settings::save::{
        autostart_changes, config_from_settings, saved_config_requires_runtime_restart,
        settings_from_app_config, settings_runtime_action, validate_app_settings,
        AppSettingsValidationError, ApplyAutostart, SettingsRuntimeAction,
    },
};

/// What the caller needs after a committed settings save.
#[derive(Debug, Clone)]
pub struct SettingsSaveOutcome {
    /// The configuration as committed.
    pub config: AppConfig,
    /// The committed configuration read back through the contract, so the
    /// caller returns exactly what was stored rather than what was requested.
    pub settings: AppSettings,
    /// Whether the running core must be restarted, the system proxy re-applied,
    /// or nothing done at all.
    pub runtime_action: SettingsRuntimeAction,
    /// `false` when the save was a no-op, so the caller can skip broadcasting a
    /// cache invalidation nothing changed.
    pub changed: bool,
}

/// Why a settings save failed.
#[derive(Debug)]
pub enum SettingsSaveError {
    /// The submitted contract is not acceptable; nothing was touched.
    Validation(AppSettingsValidationError),
    /// The OS refused the login entry. Nothing was persisted and the entry was
    /// restored. The error is already typed, so it is kept as it is.
    Autostart(voya_contracts::AppError),
    /// The database commit failed. The login entry was restored.
    Commit(ConfigMutationError),
}

/// Validate, apply the OS side effects, commit, and report what the runtime has
/// to do about it.
///
/// The guard is held across the side effects on purpose: the target config is
/// derived from the configuration *inside* the guard, so a concurrent mutation
/// cannot be silently overwritten by a target computed from a stale snapshot.
pub async fn save_app_settings(
    coordinator: &ConfigMutationCoordinator,
    autostart: &dyn ApplyAutostart,
    settings: &AppSettings,
) -> Result<SettingsSaveOutcome, SettingsSaveError> {
    validate_app_settings(settings).map_err(SettingsSaveError::Validation)?;

    let mut mutation = coordinator
        .begin()
        .await
        .map_err(SettingsSaveError::Commit)?;
    let original = mutation.config().clone();
    let target = config_from_settings(settings, &original);
    let runtime_action = settings_runtime_action(
        saved_config_requires_runtime_restart(&original, &target),
        original.system_proxy_item != target.system_proxy_item,
    );

    // Autostart is applied before the commit so a registration the OS refuses
    // never becomes the stored truth; it is restored if anything after it fails.
    let autostart_changed = autostart_changes(&original, &target);
    if autostart_changed {
        if let Err(error) = autostart.apply_autostart(&target) {
            tracing::error!(?error, "settings autostart side effect failed");
            restore_autostart(autostart, &original);
            return Err(SettingsSaveError::Autostart(error));
        }
    }

    *mutation.config_mut() = target;
    let config = match mutation.commit().await {
        Ok(config) => config,
        Err(error) => {
            if autostart_changed {
                restore_autostart(autostart, &original);
            }
            return Err(SettingsSaveError::Commit(error));
        }
    };

    Ok(SettingsSaveOutcome {
        settings: settings_from_app_config(&config),
        changed: original != config,
        runtime_action,
        config,
    })
}

fn restore_autostart(autostart: &dyn ApplyAutostart, original: &AppConfig) {
    if let Err(error) = autostart.apply_autostart(original) {
        tracing::error!(
            ?error,
            "failed to restore the login entry after a failed save"
        );
    }
}

#[cfg(test)]
mod tests;
