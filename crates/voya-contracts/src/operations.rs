use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfilesResult {
    pub text: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QrCodeImage {
    pub mime_type: String,
    pub svg: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum QrScanStatus {
    Found,
    NotFound,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum QrScanFailureReason {
    PermissionDenied,
    Unsupported,
    CaptureFailed,
    Timeout,
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QrScanResult {
    pub status: QrScanStatus,
    pub texts: Vec<String>,
    pub source: String,
    pub message: Option<String>,
    /// Also set on a successful scan when some displays could not be captured.
    pub failure_reason: Option<QrScanFailureReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdateFile {
    pub name: String,
    pub bytes: u32,
    pub used_proxy: bool,
}
