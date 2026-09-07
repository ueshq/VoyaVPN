use std::path::PathBuf;

use sqlx::migrate::MigrateError;
use thiserror::Error;

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
}
