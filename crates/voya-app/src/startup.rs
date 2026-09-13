//! What a failed launch can offer, decided without Tauri.

use std::{
    error::Error,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use voya_contracts::DatabaseErrorCode;
use voya_db::DbError;
pub use voya_db::{manual_database_reset_command, DatabaseBackup, DATABASE_NAME};

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
    fn schema_and_unreadable_database_failures_offer_a_reset() {
        let schema = boxed(DbError::UnsupportedDatabaseSchema {
            path: PathBuf::from("voyavpn.sqlite"),
            found: Some(9),
            expected: 10,
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
