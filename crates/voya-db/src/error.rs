use std::path::PathBuf;

use sqlx::migrate::MigrateError;
use thiserror::Error;
use voya_contracts::DatabaseErrorCode;

pub mod blob {
    use serde::{de::DeserializeOwned, Serialize};
    use thiserror::Error;
    use voya_core::{ProfileProtocol, ProfileTransport, RulesItem, TlsSettings};

    #[derive(Debug, Error)]
    pub enum BlobError {
        #[error("failed to serialize {type_name}: {source}")]
        Serialize {
            type_name: &'static str,
            #[source]
            source: serde_json::Error,
        },
        #[error("failed to deserialize {type_name}: {source}")]
        Deserialize {
            type_name: &'static str,
            #[source]
            source: serde_json::Error,
        },
    }

    pub fn profile_protocol_to_text(value: &ProfileProtocol) -> Result<String, BlobError> {
        to_text("ProfileProtocol", value)
    }

    pub fn profile_protocol_from_text(value: &str) -> Result<ProfileProtocol, BlobError> {
        from_text("ProfileProtocol", value)
    }

    pub fn profile_transport_to_text(value: &ProfileTransport) -> Result<String, BlobError> {
        to_text("ProfileTransport", value)
    }

    pub fn profile_transport_from_text(value: &str) -> Result<ProfileTransport, BlobError> {
        from_text("ProfileTransport", value)
    }

    pub fn tls_settings_to_text(value: &TlsSettings) -> Result<String, BlobError> {
        to_text("TlsSettings", value)
    }

    pub fn tls_settings_from_text(value: &str) -> Result<TlsSettings, BlobError> {
        from_text("TlsSettings", value)
    }

    pub fn rules_to_text(value: &[RulesItem]) -> Result<String, BlobError> {
        to_text("RulesItem[]", value)
    }

    pub fn rules_from_text(value: &str) -> Result<Vec<RulesItem>, BlobError> {
        if value.trim().is_empty() {
            return Ok(Vec::new());
        }

        serde_json::from_str(value).map_err(|source| BlobError::Deserialize {
            type_name: "RulesItem[]",
            source,
        })
    }

    fn to_text<T>(type_name: &'static str, value: &T) -> Result<String, BlobError>
    where
        T: Serialize + ?Sized,
    {
        serde_json::to_string(value).map_err(|source| BlobError::Serialize { type_name, source })
    }

    fn from_text<T>(type_name: &'static str, value: &str) -> Result<T, BlobError>
    where
        T: DeserializeOwned,
    {
        serde_json::from_str(value).map_err(|source| BlobError::Deserialize { type_name, source })
    }
}

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] MigrateError),
    #[error(transparent)]
    Blob(#[from] blob::BlobError),
    #[error("invalid {enum_name} value `{value}` in database")]
    InvalidEnum {
        enum_name: &'static str,
        value: String,
    },
    #[error(
        "unsupported Voya database schema at {path}: found version {found:?}, expected version {expected}; reset it manually with: {manual_reset_command}"
    )]
    UnsupportedDatabaseSchema {
        path: PathBuf,
        found: Option<i64>,
        expected: i64,
        manual_reset_command: String,
    },
    #[error("filesystem error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON config error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

impl DbError {
    /// True when the error describes one stored row's payload instead of the
    /// database as a whole.
    ///
    /// The persisted blobs are the raw serde shape of the domain types, so a row
    /// written by a newer build (an unknown protocol `kind`, a field this build
    /// does not know) or edited by hand fails to decode on its own. List paths
    /// skip such rows — failing the whole query would hide every other server and
    /// leave the user unable to see, connect to, or delete anything — while
    /// single-row lookups still report them. Everything else (a missing column, a
    /// closed pool) is a whole-database fault and must keep propagating.
    #[must_use]
    pub fn is_row_payload(&self) -> bool {
        matches!(self, Self::Blob(_) | Self::InvalidEnum { .. })
    }

    /// Classifies the failure for the IPC contract.
    ///
    /// The codes exist because the remedies differ: a schema mismatch needs the
    /// reset command, a decode failure names one unusable row, contention just
    /// needs retrying. Every caller shares this one classification — the shell
    /// used to flatten every persistence failure to `to_string()` at eight
    /// separate call sites, so the same `DbError` reached the frontend as a
    /// different variant depending on which command hit it.
    ///
    /// It lives here rather than in `voya-app` because sqlx is a `voya-db`
    /// dependency: the driver's result code is the only reliable way to tell a
    /// locked database from any other statement failure.
    #[must_use]
    pub fn code(&self) -> DatabaseErrorCode {
        match self {
            Self::UnsupportedDatabaseSchema { .. } | Self::Migrate(_) => {
                DatabaseErrorCode::SchemaUnsupported
            }
            Self::Blob(_) | Self::InvalidEnum { .. } | Self::Json { .. } => {
                DatabaseErrorCode::Corrupt
            }
            Self::Io { .. } => DatabaseErrorCode::Io,
            Self::Sqlx(error) => sqlx_code(error),
        }
    }

    /// The manual recovery command the database reported, when it reported one.
    ///
    /// Only the schema check knows how to word it, because only it knows which
    /// file and which version are involved.
    #[must_use]
    pub fn reset_command(&self) -> Option<&str> {
        match self {
            Self::UnsupportedDatabaseSchema {
                manual_reset_command,
                ..
            } => Some(manual_reset_command.as_str()),
            _ => None,
        }
    }
}

// SQLite primary result codes. Named here rather than pulled from
// `libsqlite3-sys` so classification costs no extra dependency.
const SQLITE_BUSY: i32 = 5;
const SQLITE_LOCKED: i32 = 6;
const SQLITE_READONLY: i32 = 8;
const SQLITE_IOERR: i32 = 10;
const SQLITE_CORRUPT: i32 = 11;
const SQLITE_FULL: i32 = 13;
const SQLITE_CANTOPEN: i32 = 14;
const SQLITE_NOTADB: i32 = 26;

fn sqlx_code(error: &sqlx::Error) -> DatabaseErrorCode {
    match error {
        // Every pooled connection is held by someone else: contention by
        // definition, and retrying is exactly the remedy.
        sqlx::Error::PoolTimedOut => DatabaseErrorCode::Locked,
        sqlx::Error::Io(_) => DatabaseErrorCode::Io,
        sqlx::Error::Database(error) => sqlite_result_code(error.code().as_deref()),
        _ => DatabaseErrorCode::Other,
    }
}

/// Classifies a SQLite result code string as sqlx reports it.
///
/// sqlx hands back the *extended* code (`SQLITE_BUSY_TIMEOUT` is 773), whose low
/// byte is the primary code (`SQLITE_BUSY`, 5). Masking is what keeps the
/// classification stable as SQLite adds extended variants.
fn sqlite_result_code(code: Option<&str>) -> DatabaseErrorCode {
    let Some(primary) = code
        .and_then(|code| code.parse::<i32>().ok())
        .map(|code| code & 0xff)
    else {
        return DatabaseErrorCode::Other;
    };

    match primary {
        SQLITE_BUSY | SQLITE_LOCKED => DatabaseErrorCode::Locked,
        SQLITE_READONLY | SQLITE_IOERR | SQLITE_FULL | SQLITE_CANTOPEN => DatabaseErrorCode::Io,
        SQLITE_CORRUPT | SQLITE_NOTADB => DatabaseErrorCode::Corrupt,
        _ => DatabaseErrorCode::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_and_migration_failures_ask_for_a_reset() {
        let schema = DbError::UnsupportedDatabaseSchema {
            path: PathBuf::from("/tmp/voyavpn.sqlite"),
            found: Some(9),
            expected: 1,
            manual_reset_command: "rm /tmp/voyavpn.sqlite".to_string(),
        };

        assert_eq!(schema.code(), DatabaseErrorCode::SchemaUnsupported);
        assert_eq!(schema.reset_command(), Some("rm /tmp/voyavpn.sqlite"));
    }

    #[test]
    fn row_payload_failures_report_corruption_and_no_reset_command() {
        let invalid_enum = DbError::InvalidEnum {
            enum_name: "ProfileProtocol",
            value: "unknown".to_string(),
        };

        assert!(invalid_enum.is_row_payload());
        assert_eq!(invalid_enum.code(), DatabaseErrorCode::Corrupt);
        assert_eq!(invalid_enum.reset_command(), None);
    }

    #[test]
    fn filesystem_failures_report_io() {
        let io = DbError::Io {
            path: PathBuf::from("/tmp/voyavpn.sqlite"),
            source: std::io::Error::other("disk went away"),
        };

        assert_eq!(io.code(), DatabaseErrorCode::Io);
    }

    /// sqlx reports the *extended* result code, so the low byte is what decides
    /// the classification: `SQLITE_BUSY_TIMEOUT` (773) has to read as busy just
    /// like plain `SQLITE_BUSY` (5).
    #[test]
    fn sqlite_result_codes_are_classified_by_their_primary_byte() {
        let cases = [
            (Some("5"), DatabaseErrorCode::Locked),
            (Some("6"), DatabaseErrorCode::Locked),
            (Some("261"), DatabaseErrorCode::Locked),
            (Some("773"), DatabaseErrorCode::Locked),
            (Some("262"), DatabaseErrorCode::Locked),
            (Some("10"), DatabaseErrorCode::Io),
            (Some("266"), DatabaseErrorCode::Io),
            (Some("13"), DatabaseErrorCode::Io),
            (Some("14"), DatabaseErrorCode::Io),
            (Some("8"), DatabaseErrorCode::Io),
            (Some("11"), DatabaseErrorCode::Corrupt),
            (Some("26"), DatabaseErrorCode::Corrupt),
            (Some("1"), DatabaseErrorCode::Other),
            (Some("not a number"), DatabaseErrorCode::Other),
            (None, DatabaseErrorCode::Other),
        ];

        for (code, expected) in cases {
            assert_eq!(sqlite_result_code(code), expected, "{code:?}");
        }
    }

    #[test]
    fn a_pool_timeout_reads_as_contention() {
        assert_eq!(
            DbError::Sqlx(sqlx::Error::PoolTimedOut).code(),
            DatabaseErrorCode::Locked
        );
    }

    #[test]
    fn an_unclassified_driver_failure_falls_back_to_other() {
        assert_eq!(
            DbError::Sqlx(sqlx::Error::RowNotFound).code(),
            DatabaseErrorCode::Other
        );
    }
}
