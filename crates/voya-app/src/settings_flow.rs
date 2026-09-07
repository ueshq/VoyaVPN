//! The one transaction behind the `save_app_settings` command.
//!
//! Saving settings is not a single write: it maps the contract onto an
//! `AppConfig`, applies OS-level side effects (the autostart entry and the
//! global hotkeys) *before* the database commit so a rejected registration
//! cannot be persisted, rolls those side effects back when a later step fails,
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

use voya_contracts::AppSettingsV1;
use voya_core::AppConfig;

use crate::{
    config_mutation::{ConfigMutationCoordinator, ConfigMutationError},
    settings_save::{
        apply_settings_side_effects, compensate_settings_side_effects, config_from_settings,
        saved_config_requires_runtime_restart, settings_from_app_config, settings_runtime_action,
        validate_app_settings, AppSettingsValidationError, SettingsContractError,
        SettingsRuntimeAction, SettingsSideEffectAdapter, SettingsSideEffectStage,
    },
};

/// What the caller needs after a committed settings save.
#[derive(Debug, Clone)]
pub struct SettingsSaveOutcome {
    /// The configuration as committed.
    pub config: AppConfig,
    /// The committed configuration read back through the contract, so the
    /// caller returns exactly what was stored rather than what was requested.
    pub settings: AppSettingsV1,
    /// Whether the running core must be restarted, the system proxy re-applied,
    /// or nothing done at all.
    pub runtime_action: SettingsRuntimeAction,
    /// `false` when the save was a no-op, so the caller can skip broadcasting a
    /// cache invalidation nothing changed.
    pub changed: bool,
}

/// Why a settings save failed.
///
/// Generic over the side-effect adapter's error so the shell's typed
/// `AppError` survives the round trip instead of being flattened into a string.
#[derive(Debug)]
pub enum SettingsSaveError<E> {
    /// The submitted contract is not acceptable; nothing was touched.
    Validation(AppSettingsValidationError),
    /// The contract could not be mapped onto an `AppConfig`.
    Contract(SettingsContractError),
    /// An OS-level side effect was rejected. Nothing was persisted and the
    /// side effects applied before it were rolled back.
    SideEffect {
        stage: SettingsSideEffectStage,
        source: E,
    },
    /// The database commit failed. The applied side effects were rolled back.
    Commit(ConfigMutationError),
}

/// Validate, apply the OS side effects, commit, and report what the runtime has
/// to do about it.
///
/// The guard is held across the side effects on purpose: the target config is
/// derived from the configuration *inside* the guard, so a concurrent mutation
/// cannot be silently overwritten by a target computed from a stale snapshot.
pub async fn save_app_settings<A>(
    coordinator: &ConfigMutationCoordinator,
    side_effects: &A,
    settings: &AppSettingsV1,
) -> Result<SettingsSaveOutcome, SettingsSaveError<A::Error>>
where
    A: SettingsSideEffectAdapter,
    A::Error: std::fmt::Debug,
{
    validate_app_settings(settings).map_err(SettingsSaveError::Validation)?;

    let mut mutation = coordinator
        .begin()
        .await
        .map_err(SettingsSaveError::Commit)?;
    let original = mutation.config().clone();
    let target = config_from_settings(settings, &original).map_err(SettingsSaveError::Contract)?;
    let runtime_action = settings_runtime_action(
        saved_config_requires_runtime_restart(&original, &target),
        original.system_proxy_item != target.system_proxy_item,
    );

    // Autostart and hotkeys are applied before the commit so a registration the
    // OS refuses never becomes the stored truth; both are rolled back below if
    // anything after them fails.
    let applied = match apply_settings_side_effects(side_effects, &original, &target) {
        Ok(applied) => applied,
        Err(failure) => {
            tracing::error!(
                stage = ?failure.stage,
                error = ?failure.source,
                "settings side effect failed"
            );
            log_compensation_errors(&failure.compensation_errors);
            return Err(SettingsSaveError::SideEffect {
                stage: failure.stage,
                source: failure.source,
            });
        }
    };

    *mutation.config_mut() = target;
    let config = match mutation.commit().await {
        Ok(config) => config,
        Err(error) => {
            log_compensation_errors(&compensate_settings_side_effects(
                side_effects,
                &original,
                applied,
            ));
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

fn log_compensation_errors<E: std::fmt::Debug>(errors: &[E]) {
    for error in errors {
        tracing::error!(?error, "failed to compensate settings side effect");
    }
}

#[cfg(test)]
mod tests;
