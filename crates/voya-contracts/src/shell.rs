use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{CoreState, CoreType, SystemProxyType, TunBackend, ValidationIssue};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum TitleBarLayout {
    Macos,
    Windows,
    None,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowChromeConfig {
    pub title_bar_layout: TitleBarLayout,
}

/// One failed IPC command.
///
/// The three parts answer three different questions and nothing else may be
/// inferred from any of them:
///
/// - `kind` is **what the frontend branches on**. Every discrimination the UI
///   performs has to be expressible here; the previous 23-arm union carried a
///   string in 21 of its arms, which is why elevation retry was triggered by
///   substring-matching the word "authorization" in `message`.
/// - `subsystem` says which part of the app failed. It is for grouping, logging
///   and copy ("Profiles: …"), never for control flow — the same `kind` means
///   the same thing whichever subsystem raised it.
/// - `message` is an English diagnostic. Show it, log it, that is all. It is
///   never parsed, and rewording it must never change behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub kind: AppErrorKind,
    pub subsystem: AppErrorSubsystem,
    pub message: String,
}

impl AppError {
    #[must_use]
    pub fn new(subsystem: AppErrorSubsystem, kind: AppErrorKind, message: String) -> Self {
        Self {
            kind,
            subsystem,
            message,
        }
    }

    /// A failure with no actionable structure behind it.
    #[must_use]
    pub fn internal(subsystem: AppErrorSubsystem, message: String) -> Self {
        Self::new(subsystem, AppErrorKind::Internal, message)
    }

    /// Rejected input, addressed to the fields that caused it.
    #[must_use]
    pub fn validation(
        subsystem: AppErrorSubsystem,
        message: String,
        issues: Vec<ValidationIssue>,
    ) -> Self {
        Self::new(subsystem, AppErrorKind::Validation { issues }, message)
    }
}

/// What went wrong, in the terms the frontend acts on.
///
/// Serialized internally tagged, so a failure reads `{ type: "elevationRequired" }`
/// and TypeScript narrows the payload from the tag alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AppErrorKind {
    /// Submitted values were rejected. `issues` name the offending fields and
    /// carry a [`ValidationCode`](crate::ValidationCode) the frontend
    /// translates, which is the pairing the DNS pane already renders.
    Validation { issues: Vec<ValidationIssue> },
    /// A referenced row does not exist. The UI's remedy is to refresh the list
    /// it selected from, so the entity matters more than the id.
    NotFound {
        entity: AppErrorEntity,
        id: Option<String>,
    },
    /// The action needs one-time system authorization. This is the *only*
    /// signal that may open a privilege prompt: TUN and the elevated
    /// supervisor spawn both raise it, and no message text can substitute.
    ElevationRequired,
    /// The core executable is not installed where the app looks for it.
    MissingCore {
        core_type: CoreType,
        search_dir: String,
        candidates: Vec<String>,
        download_url: String,
    },
    /// A download, subscription fetch or Clash API call failed. Retryable.
    Network,
    /// A filesystem or child-process operation failed.
    Io,
    /// Persistence failed. `code` separates the cases that have different
    /// remedies, and `reset_command` carries the manual recovery line when the
    /// database itself reported one.
    Database {
        code: DatabaseErrorCode,
        reset_command: Option<String>,
    },
    /// Anything with no better classification: a poisoned lock, a join error,
    /// an invariant the app itself broke.
    Internal,
}

/// Which part of the app produced a failure. Diagnostic grouping only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppErrorSubsystem {
    /// The shell itself: window chrome, event emission, background tasks.
    App,
    Autostart,
    /// Reading or writing the persisted application configuration.
    Config,
    Dns,
    Export,
    Profile,
    ProxyRuntime,
    Qr,
    Routing,
    /// Core lifecycle: config generation, supervisor, connect/disconnect.
    Runtime,
    Speedtest,
    Subscription,
    SysProxy,
    Tun,
    Update,
}

/// The kind of row a [`AppErrorKind::NotFound`] refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppErrorEntity {
    Profile,
    Routing,
    RoutingRule,
    Subscription,
    /// The core-info table has no entry for the requested core type.
    CoreInfo,
}

/// Why a persistence call failed, at the granularity the UI can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseErrorCode {
    /// The stored schema is not the one this build expects; the database has to
    /// be reset manually before anything else will work.
    SchemaUnsupported,
    /// A stored payload could not be decoded — one bad row, or a damaged file.
    Corrupt,
    /// Another writer holds the database. Retrying is the remedy.
    Locked,
    /// The database file could not be read or written.
    Io,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CoreSeedInstallStatus {
    Installed,
    AlreadyInstalled,
    SeedMissing,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CoreSeedInstallResult {
    pub core_type: CoreType,
    pub status: CoreSeedInstallStatus,
    pub installed_files: Vec<String>,
}

/// What the runtime is doing, as both an answer and an announcement.
///
/// The four runtime commands return this, and `TransientStreamEvent::CoreState`
/// carries exactly the same struct, so the frontend stores whichever arrives
/// first without reshaping it. It used to be two types — this one plus an
/// identically-shaped `CoreStateEvent` — which cost three converters in the
/// shell and three more on the frontend to move one fact between them.
///
/// `Deserialize` exists because it travels as an event payload, not because
/// anything sends one to the backend.
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusResponse {
    /// Milliseconds since this core connected; absent outside a confirmed connection.
    #[specta(type = Option<f64>)]
    pub connected_duration_ms: Option<u64>,
    pub state: CoreState,
    /// The confirmed running tunnel, independent of the saved connection mode.
    /// Absent during transitions, pending cleanup, and local-proxy operation.
    pub active_tun_backend: Option<TunBackend>,
    pub active_profile_id: Option<String>,
    pub main_pid: Option<u32>,
    pub pre_pid: Option<u32>,
    pub running_core_type: Option<CoreType>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppUpdaterState {
    Ready,
    Unconfigured,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdaterStatus {
    pub current_version: String,
    pub state: AppUpdaterState,
    pub message: Option<String>,
}

/// The OS proxy state, as both an answer and an announcement.
///
/// `TransientStreamEvent::SysProxyChanged` carries this struct; the narrower
/// `SysProxyChanged` it replaced duplicated four of these fields and re-declared
/// `SystemProxyType` under a second name.
#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxyStatusResponse {
    pub management: SystemProxyManagement,
    /// Read-only OS observation. Unknown must never be presented as restored.
    pub observation: SystemProxyObservation,
    pub manual_cleanup_required: bool,
    pub requested_mode: SystemProxyType,
    /// The app's applied policy; always Unchanged on manual platforms.
    pub effective_mode: SystemProxyType,
    pub proxy: Option<String>,
    pub exceptions: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SystemProxyManagement {
    Automatic,
    Manual,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SystemProxyObservation {
    Unknown,
    Clear,
    LocalProxy,
    OtherProxy,
}
