//! Typed domain blobs as SQLite `TEXT`, and back.

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use voya_core::{ProfileItem, ProfileProtocol, ProfileTransport, RulesItem, TlsSettings};

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

/// A profile's `protocol`, `transport` and `tls` columns, in that order.
pub(crate) fn profile_blobs(
    item: &ProfileItem,
) -> Result<(String, Option<String>, Option<String>), BlobError> {
    Ok((
        profile_protocol_to_text(&item.protocol)?,
        item.transport
            .as_ref()
            .map(profile_transport_to_text)
            .transpose()?,
        item.tls.as_ref().map(tls_settings_to_text).transpose()?,
    ))
}

pub(crate) fn profile_protocol_to_text(value: &ProfileProtocol) -> Result<String, BlobError> {
    to_text("ProfileProtocol", value)
}

pub(crate) fn profile_protocol_from_text(value: &str) -> Result<ProfileProtocol, BlobError> {
    from_text("ProfileProtocol", value)
}

pub(crate) fn profile_transport_to_text(value: &ProfileTransport) -> Result<String, BlobError> {
    to_text("ProfileTransport", value)
}

pub(crate) fn profile_transport_from_text(value: &str) -> Result<ProfileTransport, BlobError> {
    from_text("ProfileTransport", value)
}

pub(crate) fn tls_settings_to_text(value: &TlsSettings) -> Result<String, BlobError> {
    to_text("TlsSettings", value)
}

pub(crate) fn tls_settings_from_text(value: &str) -> Result<TlsSettings, BlobError> {
    from_text("TlsSettings", value)
}

pub(crate) fn rules_to_text(value: &[RulesItem]) -> Result<String, BlobError> {
    to_text("RulesItem[]", value)
}

pub(crate) fn rules_from_text(value: &str) -> Result<Vec<RulesItem>, BlobError> {
    from_text("RulesItem[]", value)
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
