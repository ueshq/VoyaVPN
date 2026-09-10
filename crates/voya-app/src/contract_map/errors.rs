//! Manager errors → the [`AppError`] every IPC command returns.
//!
//! These conversions used to be ~20 free functions in
//! `apps/desktop/src-tauri/src/ipc/commands/support.rs`, where the shell's lib
//! test harness is disabled on purpose (see AGENTS.md), so none of them could
//! be exercised. They live here for the same reason `invalidation` does: which
//! [`AppErrorKind`] a failure deserves is policy, and policy belongs where it
//! can be asserted.
//!
//! Three rules hold across everything below, and the tests enforce them:
//!
//! - **One database mapper.** Every `DbError`, however deeply nested, goes
//!   through [`database_error`]. The shell used to flatten it at eight separate
//!   call sites, so the same failure reached the frontend as `Database` through
//!   `save_profile` but as `Runtime`/`Speedtest` through the runtime and
//!   speedtest commands.
//! - **Elevation is a kind, never a sentence.** `TunManagerError::ElevationRequired`
//!   and `SupervisorError::ElevationNotGranted` are the only two sources of
//!   [`AppErrorKind::ElevationRequired`], and the only two failures allowed to
//!   open a privilege prompt. A cancelled or failed *answer* to that prompt is
//!   deliberately not one of them: re-prompting after the user declined is
//!   exactly what the retired `authorization` substring match did.

use voya_contracts::{AppError, AppErrorEntity, AppErrorKind, AppErrorSubsystem, ValidationIssue};

use super::validation_issue_to_contract;
use voya_core::CoreType;
use voya_db::DbError;
use voya_net::{certificates::CertificateError, ruleset::RulesetGeoError, DownloadError};
use voya_platform::coreinfo::CoreInfoError;

use crate::{
    autostart::AutostartManagerError, config_mutation::ConfigMutationError,
    connection_mode::ConnectionModeError, dns::DnsManagerError, elevation::ElevationError,
    exports::ExportManagerError, groups::GroupManagerError, input_safety::InputSafetyError,
    profiles::ProfileManagerError, proxy_runtime::ProxyRuntimeError, qr::QrCodeError,
    routing::RoutingManagerError, runtime::RuntimeError, settings_flow::SettingsSaveError,
    settings_save::AppSettingsValidationError, speedtest::SpeedtestError,
    subscriptions::SubscriptionManagerError, supervisor::SupervisorError,
    sysproxy::SystemProxyManagerError, tun::TunManagerError, updates::UpdateManagerError,
};

use AppErrorSubsystem as Sub;

/// What the missing-core dialog shows instead of the real filesystem path.
///
/// The app-data directory embeds the OS user name, and this payload crosses the
/// IPC boundary into a dialog, so the label stays deliberately generic.
const MISSING_CORE_SEARCH_DIR_LABEL: &str = "application core directory";

/// The single `DbError` → `AppError` mapping.
///
/// `subsystem` names the manager that hit the database, never the database
/// itself: the code says what went wrong, the subsystem says which screen to
/// blame.
#[must_use]
pub fn database_error(error: &DbError, subsystem: AppErrorSubsystem) -> AppError {
    AppError::new(
        subsystem,
        AppErrorKind::Database {
            code: error.code(),
            reset_command: error.reset_command().map(ToString::to_string),
        },
        error.to_string(),
    )
}

/// Rejected free text from an IPC argument, addressed to that argument.
#[must_use]
pub fn input_text_error(
    error: &InputSafetyError,
    field: &str,
    subsystem: AppErrorSubsystem,
) -> AppError {
    AppError::validation(
        subsystem,
        format!("invalid {field}: {error}"),
        vec![ValidationIssue::untranslated(field, error.to_string())],
    )
}

/// Core discovery, which owns the missing-core payload the frontend's install
/// prompt is built from.
#[must_use]
pub fn core_info_error(error: &CoreInfoError, subsystem: AppErrorSubsystem) -> AppError {
    match error {
        CoreInfoError::ExecutableNotFound {
            core_type,
            search_dir: _,
            candidates,
            url,
        } => AppError::new(
            subsystem,
            AppErrorKind::MissingCore {
                core_type: super::core_type_to_contract(*core_type),
                search_dir: MISSING_CORE_SEARCH_DIR_LABEL.to_string(),
                candidates: missing_core_candidates(candidates),
                download_url: (*url).to_string(),
            },
            missing_core_message(*core_type),
        ),
        CoreInfoError::MissingCoreInfo(_) => {
            not_found(subsystem, AppErrorEntity::CoreInfo, None, error)
        }
        _ => io(subsystem, error),
    }
}

/// The message the missing-core dialog shows.
///
/// Deliberately not `CoreInfoError`'s own text: that one names the search
/// directory and every candidate file name, which belong in the structured
/// payload rather than in a sentence shown to a user.
fn missing_core_message(core_type: CoreType) -> String {
    format!(
        "core {core_type:?} executable is missing; install or update the core package and try again"
    )
}

/// Splits `CoreInfoError`'s comma-joined candidate list back into entries.
fn missing_core_candidates(candidates: &str) -> Vec<String> {
    candidates
        .split(',')
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// TLS certificate probing, driven straight from the profile editor's inputs.
#[must_use]
pub fn certificate_error(error: &CertificateError) -> AppError {
    match error {
        CertificateError::EmptyAddress => invalid(Sub::Certificate, "address", error),
        CertificateError::InvalidServerName => invalid(Sub::Certificate, "serverName", error),
        CertificateError::InvalidPem => invalid(Sub::Certificate, "pem", error),
        CertificateError::Timeout
        | CertificateError::Tcp(_)
        | CertificateError::Tls(_)
        | CertificateError::MissingPeerCertificate => network(Sub::Certificate, error),
    }
}

impl From<ProfileManagerError> for AppError {
    fn from(error: ProfileManagerError) -> Self {
        match &error {
            ProfileManagerError::Database(source) => database_error(source, Sub::Profile),
            ProfileManagerError::ProfileNotFound(id) => not_found(
                Sub::Profile,
                AppErrorEntity::Profile,
                Some(id.clone()),
                &error,
            ),
            ProfileManagerError::MissingProfileId => invalid(Sub::Profile, "profileId", &error),
            ProfileManagerError::InvalidMove { .. } => invalid(Sub::Profile, "position", &error),
        }
    }
}

impl From<GroupManagerError> for AppError {
    fn from(error: GroupManagerError) -> Self {
        match error {
            GroupManagerError::Database(source) => database_error(&source, Sub::Group),
            // A nested profile failure keeps the profile subsystem: the group
            // editor lists profiles, and the row is what actually failed.
            GroupManagerError::Profile(source) => Self::from(source),
            GroupManagerError::NotGroupProfile => {
                invalid(Sub::Group, "protocol", &GroupManagerError::NotGroupProfile)
            }
            GroupManagerError::Validation(errors) => Self::validation(
                Sub::Group,
                "group validation failed".to_string(),
                errors
                    .into_iter()
                    .map(|message| validation_issue_to_contract("childProfileIds", message))
                    .collect(),
            ),
            GroupManagerError::SingboxConfig(source) => internal(Sub::Group, &source),
        }
    }
}

impl From<SubscriptionManagerError> for AppError {
    fn from(error: SubscriptionManagerError) -> Self {
        match error {
            SubscriptionManagerError::Database(source) => {
                database_error(&source, Sub::Subscription)
            }
            SubscriptionManagerError::Profile(source) => Self::from(source),
            SubscriptionManagerError::Group(source) => Self::from(*source),
            SubscriptionManagerError::Download(source) => {
                download_error(&source, Sub::Subscription)
            }
            SubscriptionManagerError::SubscriptionNotFound(ref id) => not_found(
                Sub::Subscription,
                AppErrorEntity::Subscription,
                Some(id.clone()),
                &error,
            ),
            SubscriptionManagerError::MissingRemarks => {
                invalid(Sub::Subscription, "remarks", &error)
            }
            SubscriptionManagerError::MissingUrl
            | SubscriptionManagerError::InvalidSubscriptionUrl => {
                invalid(Sub::Subscription, "url", &error)
            }
            SubscriptionManagerError::InvalidFilter(_) => {
                invalid(Sub::Subscription, "filter", &error)
            }
            // The fetch succeeded and held nothing usable, which reads to the
            // user as "the list you asked for is not there".
            SubscriptionManagerError::NoImportableProfiles => {
                not_found(Sub::Subscription, AppErrorEntity::Profile, None, &error)
            }
        }
    }
}

impl From<RoutingManagerError> for AppError {
    fn from(error: RoutingManagerError) -> Self {
        match error {
            RoutingManagerError::Database(source) => database_error(&source, Sub::Routing),
            RoutingManagerError::RoutingNotFound(ref id) => not_found(
                Sub::Routing,
                AppErrorEntity::Routing,
                Some(id.clone()),
                &error,
            ),
            RoutingManagerError::RuleNotFound { ref rule_id, .. } => not_found(
                Sub::Routing,
                AppErrorEntity::RoutingRule,
                Some(rule_id.clone()),
                &error,
            ),
            RoutingManagerError::MissingRoutingId => invalid(Sub::Routing, "routingId", &error),
            RoutingManagerError::InvalidMove { .. } => invalid(Sub::Routing, "position", &error),
        }
    }
}

impl From<ExportManagerError> for AppError {
    fn from(error: ExportManagerError) -> Self {
        match error {
            ExportManagerError::Database(source) => database_error(&source, Sub::Export),
            ExportManagerError::Share(ref source) => internal(Sub::Export, source),
            ExportManagerError::Singbox(ref source) => internal(Sub::Export, source),
            ExportManagerError::EmptySelection => invalid(Sub::Export, "profileIds", &error),
            ExportManagerError::ProfileNotFound(ref id) => not_found(
                Sub::Export,
                AppErrorEntity::Profile,
                Some(id.clone()),
                &error,
            ),
        }
    }
}

impl From<UpdateManagerError> for AppError {
    fn from(error: UpdateManagerError) -> Self {
        match error {
            UpdateManagerError::Database(source) => database_error(&source, Sub::Update),
            UpdateManagerError::RulesetGeo(source) => ruleset_geo_error(&source),
        }
    }
}

impl From<SpeedtestError> for AppError {
    fn from(error: SpeedtestError) -> Self {
        match error {
            SpeedtestError::Database(source) => database_error(&source, Sub::Speedtest),
            SpeedtestError::Profile(source) => Self::from(source),
            SpeedtestError::CoreInfo(ref source) => core_info_error(source, Sub::Speedtest),
            SpeedtestError::Network(ref source) => network(Sub::Speedtest, source),
            SpeedtestError::Io(ref source) => io(Sub::Speedtest, source),
            SpeedtestError::Path(ref source) => io(Sub::Speedtest, source),
            SpeedtestError::Process(ref source) => io(Sub::Speedtest, source),
            SpeedtestError::CreateConfigDir { .. }
            | SpeedtestError::WriteConfig { .. }
            | SpeedtestError::RemoveConfig { .. } => io(Sub::Speedtest, &error),
            SpeedtestError::MissingCoreInfo(_) => {
                not_found(Sub::Speedtest, AppErrorEntity::CoreInfo, None, &error)
            }
            SpeedtestError::Validation { .. } => invalid(Sub::Speedtest, "profile", &error),
            SpeedtestError::SingboxConfig(ref source) => internal(Sub::Speedtest, source),
            SpeedtestError::Cancelled
            | SpeedtestError::NoAvailablePort(_)
            | SpeedtestError::InvalidSocksPort(_)
            | SpeedtestError::JobLockPoisoned
            | SpeedtestError::BackgroundTask(_) => internal(Sub::Speedtest, &error),
        }
    }
}

impl From<ProxyRuntimeError> for AppError {
    fn from(error: ProxyRuntimeError) -> Self {
        match error {
            ProxyRuntimeError::Api(ref source) => network(Sub::ProxyRuntime, source),
            ProxyRuntimeError::GroupNotFound(ref name) => not_found(
                Sub::ProxyRuntime,
                AppErrorEntity::ProxyGroup,
                Some(name.clone()),
                &error,
            ),
            ProxyRuntimeError::NodeNotFound(ref name) => not_found(
                Sub::ProxyRuntime,
                AppErrorEntity::ProxyNode,
                Some(name.clone()),
                &error,
            ),
            ProxyRuntimeError::GroupNotSelector(_) => {
                invalid(Sub::ProxyRuntime, "groupName", &error)
            }
            ProxyRuntimeError::InvalidTrafficMode(_) => invalid(Sub::ProxyRuntime, "mode", &error),
            ProxyRuntimeError::MonitorLockPoisoned
            | ProxyRuntimeError::MonitorRuntimeUnavailable
            | ProxyRuntimeError::InvalidStatePort => internal(Sub::ProxyRuntime, &error),
        }
    }
}

impl From<RuntimeError> for AppError {
    fn from(error: RuntimeError) -> Self {
        match error {
            RuntimeError::Database(source) => database_error(&source, Sub::Runtime),
            RuntimeError::CoreInfo(ref source) => core_info_error(source, Sub::Runtime),
            RuntimeError::Supervisor(source) => Self::from(source),
            RuntimeError::MissingActiveProfileId => {
                not_found(Sub::Runtime, AppErrorEntity::Profile, None, &error)
            }
            RuntimeError::ActiveProfileNotFound(ref id) => not_found(
                Sub::Runtime,
                AppErrorEntity::Profile,
                Some(id.clone()),
                &error,
            ),
            RuntimeError::MissingCoreInfo(_) => {
                not_found(Sub::Runtime, AppErrorEntity::CoreInfo, None, &error)
            }
            RuntimeError::Validation { ref errors, .. } => Self::validation(
                Sub::Runtime,
                error.to_string(),
                errors
                    .iter()
                    .map(|message| validation_issue_to_contract("activeProfile", message.clone()))
                    .collect(),
            ),
            RuntimeError::CreateConfigDir { .. }
            | RuntimeError::WriteConfig { .. }
            | RuntimeError::RemoveConfig { .. } => io(Sub::Runtime, &error),
            RuntimeError::Path(ref source) => io(Sub::Runtime, source),
            RuntimeError::ContextBuild(ref source) => internal(Sub::Runtime, source),
            RuntimeError::SingboxConfig(ref source) => internal(Sub::Runtime, source),
        }
    }
}

impl From<SupervisorError> for AppError {
    fn from(error: SupervisorError) -> Self {
        match error {
            // The whole point of this pass: the frontend used to find this case
            // by searching the message for "authorization".
            SupervisorError::ElevationNotGranted(_) => AppError::new(
                Sub::Runtime,
                AppErrorKind::ElevationRequired,
                error.to_string(),
            ),
            SupervisorError::Process(ref source) => io(Sub::Runtime, source),
            SupervisorError::CommandChannelClosed
            | SupervisorError::ResponseDropped
            | SupervisorError::TunCleanup(_)
            | SupervisorError::NativeTun(_)
            | SupervisorError::Job(_)
            | SupervisorError::Elevation(_)
            | SupervisorError::UnknownSudoKillTarget { .. }
            | SupervisorError::MissingNativeTunConfigPath { .. }
            | SupervisorError::SudoKillFailed { .. } => internal(Sub::Runtime, &error),
        }
    }
}

impl From<TunManagerError> for AppError {
    fn from(error: TunManagerError) -> Self {
        match error {
            TunManagerError::ElevationRequired => {
                AppError::new(Sub::Tun, AppErrorKind::ElevationRequired, error.to_string())
            }
            TunManagerError::UnsupportedPlatform | TunManagerError::ProviderPathMismatch { .. } => {
                internal(Sub::Tun, &error)
            }
        }
    }
}

impl From<SystemProxyManagerError> for AppError {
    fn from(error: SystemProxyManagerError) -> Self {
        Self::new(Sub::SysProxy, sysproxy_kind(&error), error.to_string())
    }
}

/// Split out because `ConnectionModeError`'s rollback variants only *borrow*
/// their system-proxy source: they need the same classification under their own
/// message, and the error is not `Clone`.
fn sysproxy_kind(error: &SystemProxyManagerError) -> AppErrorKind {
    match error {
        // Asking for PAC where the OS has no PAC support is a rejected request,
        // and the mode is a settings field.
        SystemProxyManagerError::PacUnavailable(_) => AppErrorKind::Validation {
            issues: vec![ValidationIssue::untranslated(
                "network.systemProxy.mode",
                error.to_string(),
            )],
        },
        SystemProxyManagerError::Path(_)
        | SystemProxyManagerError::SystemProxy(_)
        | SystemProxyManagerError::DirtyMarkerInspect { .. }
        | SystemProxyManagerError::DirtyMarkerWrite { .. }
        | SystemProxyManagerError::DirtyMarkerRemove { .. } => AppErrorKind::Io,
    }
}

impl From<ConnectionModeError> for AppError {
    fn from(error: ConnectionModeError) -> Self {
        match error {
            ConnectionModeError::Tun(source) => Self::from(source),
            ConnectionModeError::SystemProxy(source) => Self::from(source),
            ConnectionModeError::Commit(source) => Self::from(source),
            // The rollback cases keep the underlying classification but say in
            // their message what was and was not restored.
            ConnectionModeError::SystemProxyRolledBack { ref source } => {
                Self::new(Sub::SysProxy, sysproxy_kind(source), error.to_string())
            }
            ConnectionModeError::SystemProxyRollbackFailed { .. }
            | ConnectionModeError::Task { .. } => internal(Sub::SysProxy, &error),
        }
    }
}

impl From<ElevationError> for AppError {
    fn from(error: ElevationError) -> Self {
        match error {
            ElevationError::Process(ref source) => io(Sub::Tun, source),
            ElevationError::Io(ref source) => io(Sub::Tun, source),
            // Cancelled/Failed are answers to a prompt that was already shown.
            // Classifying them as `ElevationRequired` would tell the frontend
            // to show it again, which is what the substring match used to do.
            ElevationError::MissingUsername
            | ElevationError::Cancelled
            | ElevationError::Failed { .. }
            | ElevationError::Privilege(_) => internal(Sub::Tun, &error),
        }
    }
}

impl From<AutostartManagerError> for AppError {
    fn from(error: AutostartManagerError) -> Self {
        match error {
            AutostartManagerError::CurrentExe(ref source) => io(Sub::Autostart, source),
            AutostartManagerError::HomeDir | AutostartManagerError::Autostart(_) => {
                internal(Sub::Autostart, &error)
            }
        }
    }
}

impl From<QrCodeError> for AppError {
    fn from(error: QrCodeError) -> Self {
        match error {
            QrCodeError::EmptyContent => invalid(Sub::Qr, "content", &error),
            QrCodeError::Generate(_) => internal(Sub::Qr, &error),
        }
    }
}

impl From<DnsManagerError> for AppError {
    fn from(error: DnsManagerError) -> Self {
        match error {
            DnsManagerError::Validation(issues) => Self::validation(
                Sub::Dns,
                "DNS settings validation failed".to_string(),
                issues,
            ),
        }
    }
}

impl From<ConfigMutationError> for AppError {
    fn from(error: ConfigMutationError) -> Self {
        match error {
            ConfigMutationError::Database(source) => database_error(&source, Sub::Config),
        }
    }
}

impl From<AppSettingsValidationError> for AppError {
    fn from(error: AppSettingsValidationError) -> Self {
        Self::validation(
            Sub::Config,
            error.to_string(),
            vec![ValidationIssue::new(error.field(), error.code())],
        )
    }
}

impl From<SettingsSaveError<AppError>> for AppError {
    fn from(error: SettingsSaveError<AppError>) -> Self {
        match error {
            SettingsSaveError::Validation(source) => Self::from(source),
            // The adapter's already-typed failure keeps its subsystem.
            SettingsSaveError::SideEffect { source, .. } => source,
            SettingsSaveError::Commit(source) => Self::from(source),
        }
    }
}

fn download_error(error: &DownloadError, subsystem: AppErrorSubsystem) -> AppError {
    match error {
        // A rejected URL is the user's typed subscription address, not a
        // transport failure, so it reaches the field that holds it.
        DownloadError::ForbiddenSubscriptionUrl { .. } => invalid(subsystem, "url", error),
        _ => network(subsystem, error),
    }
}

fn ruleset_geo_error(error: &RulesetGeoError) -> AppError {
    match error {
        RulesetGeoError::Download(source) => download_error(source, Sub::Update),
        RulesetGeoError::AssetIo { .. } => io(Sub::Update, error),
        RulesetGeoError::InvalidAsset { .. } | RulesetGeoError::Manifest(_) => {
            internal(Sub::Update, error)
        }
    }
}

/// A rejection with no [`voya_contracts::ValidationCode`] of its own.
///
/// The issue carries the failing manager's English `Display` text, which the
/// frontend renders verbatim. Managers whose messages a user actually acts on
/// (the core validators, DNS, settings) build their issues from codes instead;
/// giving one of these a code is a purely additive change.
fn invalid(subsystem: AppErrorSubsystem, field: &str, error: &impl std::fmt::Display) -> AppError {
    let message = error.to_string();
    AppError::validation(
        subsystem,
        message.clone(),
        vec![ValidationIssue::untranslated(field, message)],
    )
}

fn not_found(
    subsystem: AppErrorSubsystem,
    entity: AppErrorEntity,
    id: Option<String>,
    error: &impl std::fmt::Display,
) -> AppError {
    AppError::new(
        subsystem,
        AppErrorKind::NotFound { entity, id },
        error.to_string(),
    )
}

fn network(subsystem: AppErrorSubsystem, error: &impl std::fmt::Display) -> AppError {
    AppError::new(subsystem, AppErrorKind::Network, error.to_string())
}

fn io(subsystem: AppErrorSubsystem, error: &impl std::fmt::Display) -> AppError {
    AppError::new(subsystem, AppErrorKind::Io, error.to_string())
}

fn internal(subsystem: AppErrorSubsystem, error: &impl std::fmt::Display) -> AppError {
    AppError::internal(subsystem, error.to_string())
}

#[cfg(test)]
mod tests;
