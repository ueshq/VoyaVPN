//! Launch concerns decided without Tauri: work a launch starts early, and what
//! a failed launch can offer.

use std::{
    error::Error,
    path::Path,
    sync::{Arc, RwLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use voya_contracts::DatabaseErrorCode;
use voya_core::AppConfig;
use voya_db::DbError;
pub use voya_db::{manual_database_reset_command, DatabaseBackup, DATABASE_NAME};
use voya_platform::{coreinfo::TargetOs, paths::AppPaths};

use crate::{config_mutation::ConfigMutationCoordinator, services::AppServices};

/// The first half of a launch: storage is open and the settings are read.
///
/// Both hosts start the same way, and the two failures that can stop a launch
/// — a database this build rejects, settings it cannot read — both happen
/// here, as a `DbError` each host turns into its own recovery offer. What a
/// host does with the loaded settings before anything may write (the desktop
/// undoes a proxy a crashed run left behind) happens between this and
/// [`AppOpening::finish`].
#[derive(Debug)]
pub struct AppOpening {
    pub services: AppServices,
    /// The settings as loaded for this platform.
    pub config: AppConfig,
}

/// A launch whose storage is ready for commands.
#[derive(Debug)]
pub struct OpenedApp {
    pub services: AppServices,
    pub config_mutations: Arc<ConfigMutationCoordinator>,
}

impl AppOpening {
    /// Opens the database and loads the settings for `target_os`.
    /// `system_locale` seeds the language on a fresh install only.
    pub async fn open(
        database_path: &Path,
        runtime_paths: AppPaths,
        target_os: TargetOs,
        system_locale: Option<&str>,
    ) -> Result<Self, DbError> {
        let started = Instant::now();
        let services = AppServices::connect(database_path, runtime_paths)
            .await?
            .with_target_os(target_os);
        log_startup_step("open database", started);

        // A fresh install starts in the platform's native VPN mode where it
        // has one, and a platform with a single capture path never loads in a
        // mode it does not offer.
        let started = Instant::now();
        let config = services.load_config_for(system_locale).await?;
        log_startup_step("load settings", started);

        Ok(Self { services, config })
    }

    /// Builds the mutation coordinator and seeds the default routing profile.
    ///
    /// The seed cannot fail a launch. A fresh install starts with the default
    /// routing profile rather than an empty Rules page, and a failure costs
    /// only that.
    pub async fn finish(self) -> OpenedApp {
        let Self { services, config } = self;
        let config_mutations = Arc::new(
            services
                .config_mutations(Arc::new(RwLock::new(config)))
                .with_target_os(services.target_os()),
        );

        let started = Instant::now();
        if let Err(error) = services.ensure_default_routing(&config_mutations).await {
            tracing::warn!(?error, "failed to seed the default routing profile");
        }
        log_startup_step("seed default routing", started);

        OpenedApp {
            services,
            config_mutations,
        }
    }
}

/// One `startup step` line. The desktop shell logs its own steps through this
/// too, so a launch reads as one sequence whichever layer ran the step.
pub fn log_startup_step(step: &'static str, started: Instant) {
    tracing::info!(
        step,
        elapsed_ms = started.elapsed().as_millis(),
        "startup step"
    );
}

/// Reads the OS trust store every HTTPS client shares on a thread of its own.
///
/// The first client a process builds would otherwise read it on the spot,
/// over 100 ms on macOS, and one is built while the window is still hidden.
pub fn preload_tls_roots_in_background() {
    if let Err(error) = std::thread::Builder::new()
        .name("voya-tls-roots".to_string())
        .spawn(voya_net::preload_tls_roots)
    {
        tracing::debug!(%error, "TLS roots will load with the first HTTPS client");
    }
}

/// Whether moving the database aside would let the next launch succeed.
///
/// A schema from another build, or a file this build cannot read, fails the
/// same way on every launch, and a fresh database fixes both. A locked database
/// or a filesystem error would still fail afterwards, so no reset is offered.
#[must_use]
pub fn offers_database_reset(error: &(dyn Error + 'static)) -> bool {
    error.downcast_ref::<DbError>().is_some_and(|error| {
        matches!(
            error.code(),
            DatabaseErrorCode::SchemaUnsupported | DatabaseErrorCode::Corrupt
        )
    })
}

/// The startup failure dialog's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupFailureText {
    pub title: String,
    pub reset_database: String,
    pub quit: String,
    pub reset_explanation: String,
    /// The second question before the reset, which the app cannot undo.
    pub confirm_title: String,
    pub confirm_message: String,
    pub confirm_reset: String,
    /// Shown when the database cannot be moved; `{{error}}` and `{{command}}`
    /// are filled in by the caller.
    pub move_failed: String,
}

/// The startup failure dialog in the language closest to `locale`, read from the
/// locale files the renderer uses. A failed launch may have no readable
/// settings, so the caller passes the system locale.
///
/// `build.rs` copies only the `startupFailure` object out of each locale file;
/// embedding the files whole would carry every UI string in the binary.
#[must_use]
pub fn startup_failure_text(locale: &str) -> StartupFailureText {
    let source = match crate::language::ui_language_for_locale(locale) {
        "zh-Hant" => include_str!(concat!(env!("OUT_DIR"), "/startup-failure.zh-Hant.json")),
        "zh-Hans" => include_str!(concat!(env!("OUT_DIR"), "/startup-failure.zh-Hans.json")),
        _ => include_str!(concat!(env!("OUT_DIR"), "/startup-failure.en.json")),
    };
    // The i18n gate validates these JSON sources; still avoid panicking here.
    let strings: serde_json::Value = serde_json::from_str(source).unwrap_or_default();
    let text = |key: &str| {
        strings
            .pointer(&format!("/startupFailure/{key}"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    StartupFailureText {
        title: text("title"),
        reset_database: text("resetDatabase"),
        quit: text("quit"),
        reset_explanation: text("resetExplanation"),
        confirm_title: text("confirmTitle"),
        confirm_message: text("confirmMessage"),
        confirm_reset: text("confirmReset"),
        move_failed: text("moveFailed"),
    }
}

/// Moves the database and its sidecars to a timestamped backup beside it.
pub fn reset_database(path: &Path) -> Result<Option<DatabaseBackup>, DbError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    voya_db::move_database_aside(path, stamp)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn boxed(error: impl Error + 'static) -> Box<dyn Error> {
        Box::new(error)
    }

    #[test]
    fn the_startup_failure_dialog_speaks_every_shipped_language() {
        let english = startup_failure_text("en-US");
        for locale in ["en", "zh-Hans-CN", "zh-TW"] {
            let text = startup_failure_text(locale);
            for value in [
                &text.title,
                &text.reset_database,
                &text.quit,
                &text.reset_explanation,
                &text.confirm_title,
                &text.confirm_message,
                &text.confirm_reset,
                &text.move_failed,
            ] {
                assert!(!value.is_empty(), "{locale}");
            }
            // Native custom-button results are matched by their labels.
            assert_ne!(text.reset_database, text.quit, "{locale}");
            assert_ne!(text.confirm_reset, text.quit, "{locale}");
            assert!(
                text.move_failed.contains("{{error}}") && text.move_failed.contains("{{command}}"),
                "{locale}"
            );
            if locale.starts_with("zh") {
                assert_ne!(text.title, english.title, "{locale}");
            }
        }
    }

    /// The sequence both hosts launch through: a fresh directory comes up
    /// with the default routing profile, and a second launch over the same
    /// database adds no second one.
    #[tokio::test]
    async fn opening_seeds_the_default_routing_once() {
        let app_dir =
            std::env::temp_dir().join(format!("voyavpn-opening-test-{}", uuid::Uuid::new_v4()));
        let database_path = app_dir.join(DATABASE_NAME);

        for launch in 0..2 {
            let opened = AppOpening::open(
                &database_path,
                AppPaths::new(&app_dir),
                TargetOs::Linux,
                Some("en-US"),
            )
            .await
            .expect("the database opens")
            .finish()
            .await;

            let routings = opened
                .services
                .list_routings()
                .await
                .expect("the routing profiles list");
            assert_eq!(routings.len(), 1, "launch {launch}");
            assert_eq!(
                opened.config_mutations.current_config().active_routing_id,
                routings[0].id,
                "launch {launch}"
            );
        }

        // Best effort: a pool that is still closing may hold the files open.
        let _ = std::fs::remove_dir_all(&app_dir);
    }

    #[test]
    fn schema_and_unreadable_database_failures_offer_a_reset() {
        let schema = boxed(DbError::UnsupportedDatabaseSchema {
            path: PathBuf::from("voyavpn.sqlite"),
            found: Some(10),
            expected: 11,
            reason: voya_db::SchemaRejectionReason::Version {
                found: 10,
                expected: 11,
            },
            manual_reset_command: "rm -f -- voyavpn.sqlite".to_string(),
        });
        assert!(offers_database_reset(schema.as_ref()));

        let settings = boxed(DbError::Json {
            path: PathBuf::from("app_settings.payload"),
            source: serde_json::from_str::<serde_json::Value>("{").expect_err("invalid JSON"),
        });
        assert!(offers_database_reset(settings.as_ref()));
    }

    #[test]
    fn failures_a_reset_cannot_fix_offer_nothing() {
        let io = boxed(DbError::Io {
            path: PathBuf::from("Application Support"),
            source: std::io::Error::other("permission denied"),
        });
        assert!(!offers_database_reset(io.as_ref()));

        let unrelated = boxed(std::io::Error::other("tray unavailable"));
        assert!(!offers_database_reset(unrelated.as_ref()));
    }
}
