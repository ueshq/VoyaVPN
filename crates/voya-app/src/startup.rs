//! Launch concerns decided without Tauri: work a launch starts early, and what
//! a failed launch can offer.

use std::{
    error::Error,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use voya_contracts::DatabaseErrorCode;
use voya_db::DbError;
pub use voya_db::{manual_database_reset_command, DatabaseBackup, DATABASE_NAME};

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
#[must_use]
pub fn startup_failure_text(locale: &str) -> StartupFailureText {
    let source = match crate::language::ui_language_for_locale(locale) {
        "zh-Hant" => include_str!("../../../packages/i18n/src/locales/zh-Hant.json"),
        "zh-Hans" => include_str!("../../../packages/i18n/src/locales/zh-Hans.json"),
        _ => include_str!("../../../packages/i18n/src/locales/en.json"),
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

    #[test]
    fn schema_and_unreadable_database_failures_offer_a_reset() {
        let schema = boxed(DbError::UnsupportedDatabaseSchema {
            path: PathBuf::from("voyavpn.sqlite"),
            found: Some(10),
            expected: 11,
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
