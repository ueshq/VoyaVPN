//! Which `AppErrorKind` each manager failure earns.
//!
//! Two things are asserted, and both matter:
//!
//! - **The classification**, by table. A wrong kind is invisible to the
//!   compiler — every arm returns the same `AppError` — so only a case listing
//!   catches a `Database` that was filed as `Internal`, or an elevation
//!   requirement that stopped being `ElevationRequired`.
//! - **Exhaustiveness**, by a `match` guard per error type. Adding a variant
//!   makes the guard non-exhaustive, which fails the build and points at the
//!   table row that is missing.

use std::{io, path::PathBuf};

use voya_contracts::DatabaseErrorCode;
use voya_db::DbError;
use voya_net::DownloadError;
use voya_platform::coreinfo::CoreInfoError;

use voya_core::validation::ValidationCode as CoreValidationCode;

use super::*;

/// A short, comparable name for a kind, so a failed table row reads as
/// `expected "elevationRequired", got "internal"` instead of a struct dump.
fn kind_name(kind: &AppErrorKind) -> &'static str {
    match kind {
        AppErrorKind::Validation { .. } => "validation",
        AppErrorKind::NotFound { .. } => "notFound",
        AppErrorKind::ElevationRequired => "elevationRequired",
        AppErrorKind::Unsupported => "unsupported",
        AppErrorKind::MissingCore { .. } => "missingCore",
        AppErrorKind::Network => "network",
        AppErrorKind::Io => "io",
        AppErrorKind::Database { .. } => "database",
        AppErrorKind::Internal => "internal",
    }
}

#[test]
fn traffic_mode_errors_report_whether_the_mode_was_applied() {
    for (error, expected) in [
        (
            TrafficModeChangeError::Apply(ProxyRuntimeError::InvalidStatePort),
            "was saved but could not be applied",
        ),
        (
            TrafficModeChangeError::CloseConnections(ProxyRuntimeError::InvalidStatePort),
            "was applied, but existing connections could not be closed",
        ),
    ] {
        let mapped = AppError::from(error);
        assert_eq!(mapped.subsystem, AppErrorSubsystem::ProxyRuntime);
        assert_eq!(mapped.kind, AppErrorKind::Internal);
        assert!(mapped.message.contains(expected));
    }
}

fn assert_kinds<E>(cases: Vec<(&'static str, E, &'static str, AppErrorSubsystem)>)
where
    E: Into<AppError>,
{
    for (label, error, expected_kind, expected_subsystem) in cases {
        let mapped: AppError = error.into();
        assert_eq!(kind_name(&mapped.kind), expected_kind, "{label}");
        assert_eq!(mapped.subsystem, expected_subsystem, "{label}");
        assert!(!mapped.message.is_empty(), "{label} lost its message");
    }
}

fn db_row_error() -> DbError {
    DbError::InvalidEnum {
        enum_name: "ProfileProtocol",
        value: "unknown".to_string(),
    }
}

fn io_error() -> io::Error {
    io::Error::other("disk went away")
}

fn download_error() -> DownloadError {
    DownloadError::ClientBuild {
        url: "https://example.test/sub".to_string(),
        reason: "no TLS backend".to_string(),
    }
}

// ---------------------------------------------------------------------------
// The shared database mapper
// ---------------------------------------------------------------------------

#[test]
fn database_failures_keep_their_code_and_reset_hint() {
    let schema = DbError::UnsupportedDatabaseSchema {
        path: PathBuf::from("/tmp/voyavpn.sqlite"),
        found: Some(2),
        expected: 1,
        manual_reset_command: "rm /tmp/voyavpn.sqlite".to_string(),
    };

    let mapped = database_error(&schema, AppErrorSubsystem::Profile);

    assert_eq!(
        mapped.kind,
        AppErrorKind::Database {
            code: DatabaseErrorCode::SchemaUnsupported,
            reset_command: Some("rm /tmp/voyavpn.sqlite".to_string()),
        }
    );
    assert_eq!(mapped.subsystem, AppErrorSubsystem::Profile);
}

/// The whole point of one mapper: the same `DbError` has to arrive as the same
/// `kind` no matter which manager it travelled through. It used to reach the
/// frontend as `Database` from `save_profile` and as `Runtime`/`Speedtest` from
/// the runtime and speedtest commands.
#[test]
fn the_same_database_failure_is_classified_identically_through_every_manager() {
    let mapped: Vec<AppError> = vec![
        ProfileManagerError::Database(db_row_error()).into(),
        SubscriptionManagerError::Database(db_row_error()).into(),
        RoutingManagerError::Database(db_row_error()).into(),
        ExportManagerError::Database(db_row_error()).into(),
        UpdateManagerError::Database(db_row_error()).into(),
        SpeedtestError::Database(db_row_error()).into(),
        RuntimeError::Database(db_row_error()).into(),
        ConfigMutationError::Database(db_row_error()).into(),
        // Nested through two managers, which is where the old shell mappers
        // disagreed the most.
        SubscriptionManagerError::Profile(ProfileManagerError::Database(db_row_error())).into(),
        SpeedtestError::Profile(ProfileManagerError::Database(db_row_error())).into(),
    ];

    for error in mapped {
        assert_eq!(
            error.kind,
            AppErrorKind::Database {
                code: DatabaseErrorCode::Corrupt,
                reset_command: None,
            },
            "{error:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Elevation
// ---------------------------------------------------------------------------

/// Both elevation paths have to agree, and neither may be recognised from its
/// message: `runtime-action.ts` used to find the supervisor case by searching
/// for the word "authorization".
#[test]
fn both_elevation_paths_produce_the_same_kind() {
    let tun: AppError = TunManagerError::ElevationRequired.into();
    let supervisor: AppError = SupervisorError::ElevationNotGranted.into();
    let through_runtime: AppError =
        RuntimeError::Supervisor(SupervisorError::ElevationNotGranted).into();
    let through_connection_mode: AppError =
        ConnectionModeError::Tun(TunManagerError::ElevationRequired).into();

    for error in [tun, supervisor, through_runtime, through_connection_mode] {
        assert_eq!(error.kind, AppErrorKind::ElevationRequired, "{error:?}");
    }
}

/// Leaving VPN mode on macOS is a platform limit, not a bug, so it must not be
/// reported as an internal error.
#[test]
fn leaving_vpn_mode_on_macos_is_unsupported() {
    let error: AppError = ConnectionModeError::Tun(TunManagerError::VpnRequired).into();
    assert_eq!(error.kind, AppErrorKind::Unsupported);
    assert_eq!(error.subsystem, AppErrorSubsystem::Tun);
}

/// A cancelled or failed *answer* to the prompt must not read as "ask again":
/// the retired substring match treated `native authorization was cancelled` as
/// a reason to re-prompt and re-run the action.
#[test]
fn answering_the_elevation_prompt_is_never_a_request_to_show_it_again() {
    for error in [
        ElevationError::Cancelled,
        ElevationError::Failed {
            status_code: Some(1),
            message: "native authorization failed".to_string(),
        },
        ElevationError::MissingUsername,
    ] {
        let mapped: AppError = error.into();
        assert_eq!(mapped.kind, AppErrorKind::Internal, "{mapped:?}");
        assert!(
            mapped.message.contains("authorization") || mapped.message.contains("user"),
            "the message is still shown to the user: {mapped:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Missing core
// ---------------------------------------------------------------------------

#[test]
fn core_seed_and_filesystem_failures_are_io_not_missing_core() {
    let mapped = core_info_error(
        &CoreInfoError::InvalidCoreSeedDir {
            path: PathBuf::from("/tmp/seeds"),
        },
        AppErrorSubsystem::Runtime,
    );

    assert_eq!(mapped.kind, AppErrorKind::Io);
}

#[test]
fn missing_core_candidates_split_and_trim_the_joined_list() {
    assert_eq!(
        missing_core_candidates("sing-box, sing-box.exe , "),
        vec!["sing-box".to_string(), "sing-box.exe".to_string()]
    );
    assert!(missing_core_candidates("").is_empty());
}

#[test]
fn the_missing_core_message_names_the_core_without_leaking_the_path() {
    let message = MISSING_CORE_MESSAGE;

    assert!(message.contains("sing_box"), "{message}");
    assert!(!message.contains('/'), "{message}");
}

// ---------------------------------------------------------------------------
// Per-manager classification tables
// ---------------------------------------------------------------------------

#[test]
fn profile_failures_are_classified() {
    assert_kinds(vec![
        (
            "database",
            ProfileManagerError::Database(db_row_error()),
            "database",
            AppErrorSubsystem::Profile,
        ),
        (
            "not found",
            ProfileManagerError::ProfileNotFound("p-1".to_string()),
            "notFound",
            AppErrorSubsystem::Profile,
        ),
        (
            "missing id",
            ProfileManagerError::MissingProfileId,
            "validation",
            AppErrorSubsystem::Profile,
        ),
    ]);
}

#[test]
fn subscription_failures_are_classified() {
    assert_kinds(vec![
        (
            "download",
            SubscriptionManagerError::Download(download_error()),
            "network",
            AppErrorSubsystem::Subscription,
        ),
        (
            "not found",
            SubscriptionManagerError::SubscriptionNotFound("s-1".to_string()),
            "notFound",
            AppErrorSubsystem::Subscription,
        ),
        (
            "missing remarks",
            SubscriptionManagerError::MissingRemarks,
            "validation",
            AppErrorSubsystem::Subscription,
        ),
        (
            "missing url",
            SubscriptionManagerError::MissingUrl,
            "validation",
            AppErrorSubsystem::Subscription,
        ),
        (
            "invalid url",
            SubscriptionManagerError::InvalidSubscriptionUrl,
            "validation",
            AppErrorSubsystem::Subscription,
        ),
        (
            "invalid filter",
            SubscriptionManagerError::InvalidFilter("(".to_string()),
            "validation",
            AppErrorSubsystem::Subscription,
        ),
        (
            "nothing importable",
            SubscriptionManagerError::NoImportableProfiles,
            "notFound",
            AppErrorSubsystem::Subscription,
        ),
    ]);
}

/// A blocked subscription URL is the address the user typed, so it reaches the
/// field rather than reading as a transport failure.
#[test]
fn a_forbidden_subscription_url_is_a_field_rejection() {
    let mapped: AppError =
        SubscriptionManagerError::Download(DownloadError::ForbiddenSubscriptionUrl {
            url: "http://127.0.0.1/sub".to_string(),
            reason: "loopback".to_string(),
        })
        .into();

    let AppErrorKind::Validation { issues } = &mapped.kind else {
        panic!("expected a validation failure, got {mapped:?}");
    };
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].field, "url");
}

#[test]
fn routing_failures_are_classified() {
    assert_kinds(vec![
        (
            "routing not found",
            RoutingManagerError::RoutingNotFound("r-1".to_string()),
            "notFound",
            AppErrorSubsystem::Routing,
        ),
        (
            "rule not found",
            RoutingManagerError::RuleNotFound {
                routing_id: "r-1".to_string(),
                rule_id: "rule-1".to_string(),
            },
            "notFound",
            AppErrorSubsystem::Routing,
        ),
        (
            "missing id",
            RoutingManagerError::MissingRoutingId,
            "validation",
            AppErrorSubsystem::Routing,
        ),
        (
            "invalid move",
            RoutingManagerError::InvalidMove {
                rule_id: "rule-1".to_string(),
                reason: "already last".to_string(),
            },
            "validation",
            AppErrorSubsystem::Routing,
        ),
    ]);
}

#[test]
fn a_rule_not_found_names_the_rule_rather_than_its_routing() {
    let mapped: AppError = RoutingManagerError::RuleNotFound {
        routing_id: "r-1".to_string(),
        rule_id: "rule-1".to_string(),
    }
    .into();

    assert_eq!(
        mapped.kind,
        AppErrorKind::NotFound {
            entity: AppErrorEntity::RoutingRule,
            id: Some("rule-1".to_string()),
        }
    );
}

#[test]
fn proxy_runtime_failures_are_classified() {
    assert_kinds(vec![
        (
            "poisoned monitor lock",
            ProxyRuntimeError::MonitorLockPoisoned,
            "internal",
            AppErrorSubsystem::ProxyRuntime,
        ),
        (
            "no runtime",
            ProxyRuntimeError::MonitorRuntimeUnavailable,
            "internal",
            AppErrorSubsystem::ProxyRuntime,
        ),
        (
            "bad state port",
            ProxyRuntimeError::InvalidStatePort,
            "internal",
            AppErrorSubsystem::ProxyRuntime,
        ),
    ]);
}

#[test]
fn runtime_failures_are_classified() {
    assert_kinds(vec![
        (
            "no active profile",
            RuntimeError::MissingActiveProfileId,
            "notFound",
            AppErrorSubsystem::Runtime,
        ),
        (
            "active profile missing",
            RuntimeError::ActiveProfileNotFound("p-1".to_string()),
            "notFound",
            AppErrorSubsystem::Runtime,
        ),
        (
            "generation validation",
            RuntimeError::Validation {
                errors: vec![validation_message(
                    CoreValidationCode::RoutingRuleWithoutOutbound {
                        rule: "rule".to_string(),
                    },
                )],
                warnings: Vec::new(),
            },
            "validation",
            AppErrorSubsystem::Runtime,
        ),
        (
            "config write",
            RuntimeError::WriteConfig {
                path: PathBuf::from("/tmp/config.json"),
                source: io_error(),
            },
            "io",
            AppErrorSubsystem::Runtime,
        ),
        (
            "config dir",
            RuntimeError::CreateConfigDir {
                path: PathBuf::from("/tmp/configs"),
                source: io_error(),
            },
            "io",
            AppErrorSubsystem::Runtime,
        ),
        (
            "config remove",
            RuntimeError::RemoveConfig {
                path: PathBuf::from("/tmp/config.json"),
                source: io_error(),
            },
            "io",
            AppErrorSubsystem::Runtime,
        ),
    ]);
}

#[test]
fn every_generation_error_reaches_the_active_profile_field() {
    let mapped: AppError = RuntimeError::Validation {
        errors: vec![
            validation_message(CoreValidationCode::RoutingRuleWithoutOutbound {
                rule: "rule".to_string(),
            }),
            validation_message(CoreValidationCode::InvalidPort),
        ],
        warnings: vec![validation_message(CoreValidationCode::InvalidAddress)],
    }
    .into();

    let AppErrorKind::Validation { issues } = &mapped.kind else {
        panic!("expected a validation failure, got {mapped:?}");
    };
    assert_eq!(issues.len(), 2, "warnings are not rejections");
    assert!(issues.iter().all(|issue| issue.field == "activeProfile"));
}

#[test]
fn export_failures_are_classified() {
    assert_kinds(vec![
        (
            "empty selection",
            ExportManagerError::EmptySelection,
            "validation",
            AppErrorSubsystem::Export,
        ),
        (
            "profile missing",
            ExportManagerError::ProfileNotFound("p-1".to_string()),
            "notFound",
            AppErrorSubsystem::Export,
        ),
    ]);
}

#[test]
fn empty_speedtest_selection_is_a_validation_failure() {
    assert_kinds(vec![(
        "empty selection",
        SpeedtestError::EmptySelection,
        "validation",
        AppErrorSubsystem::Speedtest,
    )]);
}

#[test]
fn tun_and_system_proxy_failures_are_classified() {
    assert_kinds(vec![
        (
            "unsupported platform",
            TunManagerError::UnsupportedPlatform,
            "internal",
            AppErrorSubsystem::Tun,
        ),
        (
            "provider mismatch",
            TunManagerError::ProviderPathMismatch {
                expected: "/Applications/VoyaVPN.app".to_string(),
                resolved: "/tmp/VoyaVPN.app".to_string(),
            },
            "internal",
            AppErrorSubsystem::Tun,
        ),
    ]);
    assert_kinds(vec![(
        "dirty marker write",
        SystemProxyManagerError::DirtyMarkerWrite {
            path: "proxy-dirty".into(),
            source: io::Error::other("read-only"),
        },
        "io",
        AppErrorSubsystem::SysProxy,
    )]);
}

#[test]
fn qr_and_dns_failures_are_classified() {
    assert_kinds(vec![
        (
            "empty content",
            QrCodeError::EmptyContent,
            "validation",
            AppErrorSubsystem::Qr,
        ),
        (
            "generation",
            QrCodeError::Generate("too long".to_string()),
            "internal",
            AppErrorSubsystem::Qr,
        ),
        (
            "picked image",
            QrCodeError::InvalidImage("the pixel data is not base64"),
            "validation",
            AppErrorSubsystem::Qr,
        ),
    ]);

    let dns: AppError = DnsSettingsError::Validation(vec![ValidationIssue::new(
        "direct",
        voya_contracts::ValidationCode::DnsAddressEmpty,
    )])
    .into();
    let AppErrorKind::Validation { issues } = &dns.kind else {
        panic!("expected a validation failure, got {dns:?}");
    };
    assert_eq!(issues[0].field, "direct");
    assert_eq!(dns.subsystem, AppErrorSubsystem::Dns);
}

#[test]
fn rejected_ipc_text_names_the_argument_that_carried_it() {
    let mapped = input_text_error(
        &InputSafetyError::TooLong,
        "subscription id",
        AppErrorSubsystem::Subscription,
    );

    let AppErrorKind::Validation { issues } = &mapped.kind else {
        panic!("expected a validation failure, got {mapped:?}");
    };
    assert_eq!(issues[0].field, "subscription id");
    assert!(mapped.message.contains("too long"), "{mapped:?}");
}

#[test]
fn settings_failures_reach_the_field_and_keep_a_typed_side_effect() {
    let validation: AppError = AppSettingsValidationError::InvalidTunMtu.into();
    let AppErrorKind::Validation { issues } = &validation.kind else {
        panic!("expected a validation failure, got {validation:?}");
    };
    assert_eq!(issues[0].field, "network.tun.mtu");
    assert_eq!(validation.subsystem, AppErrorSubsystem::Config);

    // A rejected side effect keeps the adapter's own already-typed error rather
    // than being relabelled as a settings problem.
    let side_effect: AppError = SettingsSaveError::Autostart(AppError::internal(
        AppErrorSubsystem::Autostart,
        "autostart refused".to_string(),
    ))
    .into();
    assert_eq!(side_effect.subsystem, AppErrorSubsystem::Autostart);
}

// ---------------------------------------------------------------------------
// Exhaustiveness guards
//
// These never run a mapping: they exist so that adding a variant to any of the
// error types above stops compiling here until a table row is added for it.
// ---------------------------------------------------------------------------

#[expect(dead_code, reason = "compile-time exhaustiveness guards")]
mod guards {
    use super::*;

    const fn profile(error: &ProfileManagerError) {
        match error {
            ProfileManagerError::Database(_)
            | ProfileManagerError::ProfileNotFound(_)
            | ProfileManagerError::MissingProfileId
            | ProfileManagerError::SubscriptionReadOnly(_) => (),
        }
    }

    const fn subscription(error: &SubscriptionManagerError) {
        match error {
            SubscriptionManagerError::Database(_)
            | SubscriptionManagerError::Profile(_)
            | SubscriptionManagerError::Download(_)
            | SubscriptionManagerError::SubscriptionNotFound(_)
            | SubscriptionManagerError::MissingRemarks
            | SubscriptionManagerError::MissingUrl
            | SubscriptionManagerError::InvalidSubscriptionUrl
            | SubscriptionManagerError::InvalidFilter(_)
            | SubscriptionManagerError::NoImportableProfiles => (),
        }
    }

    const fn routing(error: &RoutingManagerError) {
        match error {
            RoutingManagerError::Database(_)
            | RoutingManagerError::RoutingNotFound(_)
            | RoutingManagerError::MissingRoutingId
            | RoutingManagerError::RuleNotFound { .. }
            | RoutingManagerError::InvalidMove { .. } => (),
        }
    }
    const fn export(error: &ExportManagerError) {
        match error {
            ExportManagerError::Database(_)
            | ExportManagerError::Share(_)
            | ExportManagerError::EmptySelection
            | ExportManagerError::ProfileNotFound(_) => (),
        }
    }

    const fn update(error: &UpdateManagerError) {
        match error {
            UpdateManagerError::Database(_) | UpdateManagerError::Ruleset(_) => (),
        }
    }

    const fn speedtest(error: &SpeedtestError) {
        match error {
            SpeedtestError::Database(_)
            | SpeedtestError::Profile(_)
            | SpeedtestError::Network(_)
            | SpeedtestError::Io(_)
            | SpeedtestError::CoreInfo(_)
            | SpeedtestError::Path(_)
            | SpeedtestError::Process(_)
            | SpeedtestError::SingboxConfig(_)
            | SpeedtestError::Cancelled
            | SpeedtestError::CreateConfigDir { .. }
            | SpeedtestError::WriteConfig { .. }
            | SpeedtestError::NoAvailablePort(_)
            | SpeedtestError::InvalidSocksPort(_)
            | SpeedtestError::EmptySelection
            | SpeedtestError::ProbeCoreHost(_)
            | SpeedtestError::BackgroundTask(_) => (),
        }
    }

    const fn proxy_runtime(error: &ProxyRuntimeError) {
        match error {
            ProxyRuntimeError::Api(_)
            | ProxyRuntimeError::InvalidTrafficMode(_)
            | ProxyRuntimeError::MonitorLockPoisoned
            | ProxyRuntimeError::MonitorRuntimeUnavailable
            | ProxyRuntimeError::InvalidStatePort
            | ProxyRuntimeError::UnknownGroupMember(_) => (),
        }
    }

    const fn runtime(error: &RuntimeError) {
        match error {
            RuntimeError::MissingActiveProfileId
            | RuntimeError::ActiveProfileNotFound(_)
            | RuntimeError::ActivePolicyGroupNotFound(_)
            | RuntimeError::Validation { .. }
            | RuntimeError::CreateConfigDir { .. }
            | RuntimeError::WriteConfig { .. }
            | RuntimeError::RemoveConfig { .. }
            | RuntimeError::SingboxConfig(_)
            | RuntimeError::CoreInfo(_)
            | RuntimeError::Database(_)
            | RuntimeError::Path(_)
            | RuntimeError::Supervisor(_)
            | RuntimeError::SettingsApply(_) => (),
        }
    }

    const fn supervisor(error: &SupervisorError) {
        match error {
            SupervisorError::CommandChannelClosed
            | SupervisorError::ResponseDropped
            | SupervisorError::ElevationNotGranted
            | SupervisorError::Process(_)
            | SupervisorError::NativeTun(_)
            | SupervisorError::Job(_)
            | SupervisorError::Elevation(_)
            | SupervisorError::UnknownSudoKillTarget { .. }
            | SupervisorError::MissingNativeTunConfigPath { .. }
            | SupervisorError::MissingCoreLaunch { .. }
            | SupervisorError::SudoKillFailed { .. } => (),
        }
    }

    const fn elevation(error: &ElevationError) {
        match error {
            ElevationError::MissingUsername
            | ElevationError::Cancelled
            | ElevationError::Failed { .. }
            | ElevationError::Privilege(_)
            | ElevationError::Process(_)
            | ElevationError::Io(_) => (),
        }
    }

    const fn tun(error: &TunManagerError) {
        match error {
            TunManagerError::ElevationRequired
            | TunManagerError::VpnRequired
            | TunManagerError::UnsupportedPlatform
            | TunManagerError::ProviderPathMismatch { .. } => (),
        }
    }

    const fn system_proxy(error: &SystemProxyManagerError) {
        match error {
            SystemProxyManagerError::Path(_)
            | SystemProxyManagerError::SystemProxy(_)
            | SystemProxyManagerError::DirtyMarkerInspect { .. }
            | SystemProxyManagerError::DirtyMarkerWrite { .. }
            | SystemProxyManagerError::DirtyMarkerRemove { .. } => (),
        }
    }

    const fn connection_mode(error: &ConnectionModeError) {
        match error {
            ConnectionModeError::Tun(_)
            | ConnectionModeError::SystemProxy(_)
            | ConnectionModeError::Commit(_)
            | ConnectionModeError::SystemProxyRolledBack { .. }
            | ConnectionModeError::SystemProxyRollbackFailed { .. }
            | ConnectionModeError::Task { .. } => (),
        }
    }

    const fn autostart(error: &AutostartManagerError) {
        match error {
            AutostartManagerError::Autostart(_)
            | AutostartManagerError::CurrentExe(_)
            | AutostartManagerError::HomeDir => (),
        }
    }

    const fn qr(error: &QrCodeError) {
        match error {
            QrCodeError::EmptyContent | QrCodeError::Generate(_) | QrCodeError::InvalidImage(_) => {
            }
        }
    }

    const fn config_mutation(error: &ConfigMutationError) {
        match error {
            ConfigMutationError::Database(_) => (),
        }
    }

    const fn settings_save(error: &SettingsSaveError) {
        match error {
            SettingsSaveError::Validation(_)
            | SettingsSaveError::Autostart(_)
            | SettingsSaveError::Commit(_) => (),
        }
    }

    const fn app_error_kind(kind: &AppErrorKind) {
        match kind {
            AppErrorKind::Validation { .. }
            | AppErrorKind::NotFound { .. }
            | AppErrorKind::ElevationRequired
            | AppErrorKind::Unsupported
            | AppErrorKind::MissingCore { .. }
            | AppErrorKind::Network
            | AppErrorKind::Io
            | AppErrorKind::Database { .. }
            | AppErrorKind::Internal => (),
        }
    }
}

/// A core validator finding, for the error mappings that carry one.
fn validation_message(code: CoreValidationCode) -> voya_core::validation::ValidationMessage {
    voya_core::validation::ValidationMessage::new(code)
}
