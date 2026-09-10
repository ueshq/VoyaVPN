use thiserror::Error;

use crate::{ConfigType, ProfileItem};

/// Protocol share-link parser/exporter interface.
pub trait ShareFmt {
    fn config_type(&self) -> ConfigType;
    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError>;
    fn export(&self, item: &ProfileItem) -> Result<String, ShareError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShareError {
    #[error("share link is empty")]
    EmptyInput,
    #[error("unsupported share protocol")]
    UnsupportedProtocol,
    #[error("invalid {protocol} URI: {reason}")]
    InvalidUri {
        protocol: &'static str,
        reason: String,
    },
    #[error("invalid {protocol} base64 payload")]
    InvalidBase64 { protocol: &'static str },
    #[error("invalid {protocol} JSON payload: {reason}")]
    InvalidJson {
        protocol: &'static str,
        reason: String,
    },
    #[error("{protocol} is missing required field {field}")]
    MissingField {
        protocol: &'static str,
        field: &'static str,
    },
    #[error("{protocol} has invalid port {port}")]
    InvalidPort {
        protocol: &'static str,
        port: String,
    },
    #[error("{protocol} cannot export config type {actual:?}")]
    WrongConfigType {
        protocol: &'static str,
        actual: ConfigType,
    },
    #[error("invalid full custom config")]
    InvalidFullConfig,
    #[error("invalid Voya node bundle: {reason}")]
    InvalidVoyaBundle { reason: String },
}

/// Full-config import formats VoyaVPN can actually run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomConfigKind {
    SingBox,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomConfigImport {
    pub kind: CustomConfigKind,
    pub extension: String,
    pub contents: String,
    pub profile: ProfileItem,
}
