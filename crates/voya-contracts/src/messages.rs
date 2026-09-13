//! Machine codes for the text a user reads.
//!
//! The app gates locale alignment in CI, but that gate
//! only sees `apps/desktop/src` and `packages/ui/src`. Any sentence built in
//! Rust therefore reached the screen in English whatever the interface
//! language was — notice titles, validator messages, tray labels, speedtest
//! status text — and three of those were persisted into SQLite, so switching
//! the language later could not fix them either.
//!
//! Every user-visible string the backend produces now crosses the boundary as
//! a **code plus its parameters**, and the frontend resolves it against the
//! locale files (`apps/desktop/src/ipc/messages.ts` holds the code → key
//! registries). The enums are internally tagged, so one payload is both the
//! discriminator and the interpolation bag: the frontend calls
//! `t(KEY_BY_CODE[value.code], value)` and i18next reads `{{network}}` out of
//! the very same object.
//!
//! What deliberately stays English:
//!
//! * `detail` fields. They carry an error's `Display` text for the logs and
//!   the "why" line under a notice — the same contract `AppError::message`
//!   already has. They are never parsed and never persisted.
//! * `LogLineBody::Core`. Those bytes are the core process's own output.

use serde::{Deserialize, Serialize};
use specta::Type;

/// What the backend is telling the user about, as a code the frontend
/// translates.
///
/// One variant per distinct message rather than a message assembled from
/// parts: a notice is a whole sentence in every locale, and languages do not
/// agree on how to build one out of a subject and a verb.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NoticeCode {
    // ---- a committed change could not be broadcast to the query caches ----
    ProfileRefreshFailed,
    SubscriptionRefreshFailed,
    RoutingRefreshFailed,
    DnsRefreshFailed,
    ProxyViewRefreshFailed,
    ConnectionModeRefreshFailed,
    SettingsRefreshFailed,
    // ---- the change was saved but the core could not be restarted on it ----
    RoutingSavedRestartFailed,
    RoutingDeletedRestartFailed,
    RoutingSelectedRestartFailed,
    RoutingRuleSavedRestartFailed,
    RoutingRulesDeletedRestartFailed,
    RoutingRuleMovedRestartFailed,
    DnsSavedRestartFailed,
    TunSavedRestartFailed,
    ConnectionModeSavedRestartFailed,
    SettingsSavedRuntimeUpdateFailed,
    ProxyModeSavedRuntimeUpdateFailed,
    // ---- the change was saved but a follow-up side effect failed ----
    SettingsSavedSystemProxyUpdateFailed,
    SystemProxyStatusRefreshFailed,
    TunStatusRefreshFailed,
    TrayRefreshFailed,
    // ---- the core or the tunnel stopped on its own ----
    CoreStopped,
    NativeTunStopped,
    CoreStartedSystemProxyFailed,
    SystemProxyRestoreFailed,
    // ---- background work ----
    SubscriptionAutoUpdateFailed { remarks: String },
}

/// Which core operation a log line is about.
///
/// The flow used to interpolate an English fragment ("connect", "Routing
/// changed") into its log sentences; the fragment is a code now so the whole
/// sentence can be assembled in the reader's language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CoreFlowReason {
    Connect,
    Restart,
    Disconnect,
    RoutingChanged,
    DnsChanged,
    TunChanged,
    ConnectionModeChanged,
    SettingsSaved,
}

/// A log line the **app** wrote, named by code.
///
/// Only lines the app authors are in here. Everything the core process prints
/// travels as [`LogLineBody::Core`] and is shown byte for byte.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LogCode {
    Connecting,
    Connected,
    Restarting,
    Restarted,
    Disconnecting,
    Disconnected,
    RestartingAfterChange {
        reason: CoreFlowReason,
    },
    RestartedAfterChange {
        reason: CoreFlowReason,
    },
    /// The core exited and was restarted straight away.
    CoreExitRestarted {
        attempt: u32,
    },
    /// The core exited and a restart is scheduled.
    CoreExitRetryScheduled {
        attempt: u32,
        delay_ms: u32,
    },
    /// The core exited too often and the app stopped restarting it.
    CoreExitGaveUp,
    NativeTunExited,
    /// An operation failed before the supervisor was touched, so the core that
    /// was already running is still serving traffic.
    PreviousCoreStillRunning {
        reason: CoreFlowReason,
    },
    /// The supervisor could not say what state it ended up in.
    RuntimeStatusRefreshFailed {
        reason: CoreFlowReason,
    },
    /// A core operation failed; `detail` carries the error.
    CoreOperationFailed {
        reason: CoreFlowReason,
    },
    /// A change was committed and its follow-up work failed.
    PostCommitFailed,
    SpeedtestCancellationRequested,
    SubscriptionAutoUpdateFailed {
        remarks: String,
    },
    SubscriptionAutoUpdateFinished {
        remarks: String,
        imported: u32,
    },
}

/// One line in the Logs panel.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(
    tag = "source",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LogLineBody {
    /// Output of the core process, passed through unchanged. It is the core's
    /// own text in the core's own wording, so translating it is not ours to do.
    Core { line: String },
    /// A `tracing` event from the app's own instrumentation, forwarded to the
    /// panel so a failure is visible without opening the file log. Structured
    /// developer diagnostics with a module target — deliberately untranslated,
    /// for the same reason a stack trace is.
    Diagnostic { line: String },
    /// A line the app wrote *for the user*. `detail` is an untranslated
    /// technical diagnostic (an error's `Display`), shown after the translated
    /// sentence.
    App {
        code: LogCode,
        detail: Option<String>,
    },
}

/// Why a submitted or generated value was rejected.
///
/// Produced by `voya_core`'s node/group validators, by the DNS pane's checks
/// and by settings validation. [`ValidationCode::Untranslated`] is the
/// deliberate escape hatch for the managers that have no code yet: it is
/// greppable, it shows the English diagnostic verbatim, and adding a code for
/// one is a purely additive change.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ValidationCode {
    SubscriptionReadOnly {
        subscription_id: String,
    },
    // ---- node fields (voya_core::context::validation) ----
    InvalidAddress,
    InvalidPort,
    InvalidPassword,
    InvalidFlow,
    InvalidShadowsocksMethod,
    InvalidRealityPublicKey,
    UnsupportedProtocol {
        protocol: String,
    },
    UnsupportedProtocolNetwork {
        protocol: String,
        network: String,
    },
    UnsupportedShadowsocksNetwork {
        network: String,
    },
    // ---- routing rules ----
    RoutingRuleWithoutOutbound {
        rule: String,
    },
    RoutingRuleOutboundNotFound {
        rule: String,
        outbound: String,
    },
    // ---- DNS ----
    DnsAddressEmpty,
    DnsAddressPort {
        port: String,
    },
    DnsHostsLine {
        line: u32,
    },
    DnsExpectedIps,
    // ---- application settings ----
    TextRequired,
    TextTooLong,
    TextControlCharacters,
    TooManyItems,
    UnsupportedSettingsSchema {
        found: u32,
        expected: u32,
    },
    TunMtuOutOfRange {
        min: u32,
        max: u32,
    },
    NegativeHysteriaBandwidth,
    HysteriaHopIntervalTooShort {
        minimum_seconds: u32,
    },
    /// A rejection this contract has no code for. The English `message` is the
    /// failing manager's own diagnostic and is rendered verbatim.
    Untranslated {
        message: String,
    },
}

/// Where a validation message was produced, when the validator had to walk
/// into a group's children or a routing rule's outbound to find it.
///
/// This replaces the `"group child A / B: "` prefixes the validators used to
/// glue onto the front of a message, which made the message itself
/// untranslatable. The frontend renders the breadcrumb from these entries and
/// the translated message after it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ValidationScope {
    RoutingRuleOutbound { rule: String, outbound: String },
}

/// One rejected value.
///
/// `field` is a stable identifier, not a display label: the DNS pane keys its
/// inputs by `direct`/`remote`/`bootstrap`/`hosts`, the settings surface by its
/// contract path (`network.tun.mtu`), and the group builder by `children`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationIssue {
    pub field: String,
    pub code: ValidationCode,
    /// Empty unless the validator descended into a group or a rule outbound.
    pub scope: Vec<ValidationScope>,
}

impl ValidationIssue {
    #[must_use]
    pub fn new(field: impl Into<String>, code: ValidationCode) -> Self {
        Self {
            field: field.into(),
            code,
            scope: Vec::new(),
        }
    }

    /// An issue with no code of its own, carrying the manager's English text.
    #[must_use]
    pub fn untranslated(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(
            field,
            ValidationCode::Untranslated {
                message: message.into(),
            },
        )
    }
}

/// Why one line of imported text did not simply become a node.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ImportLineCode {
    /// The line was a subscription URL and was added as a source instead.
    SubscriptionSourceAdded,
    /// The share link names a transport sing-box cannot carry.
    UnsupportedTransport { transport: String },
    /// Any other share-link parse failure, with its untranslated diagnostic.
    ParseFailed { detail: String },
}

/// One reported line of an import, numbered from 1.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportLineIssue {
    pub line: u32,
    pub code: ImportLineCode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_serialize_as_camel_case_tagged_objects() {
        let value = serde_json::to_value(NoticeCode::SubscriptionAutoUpdateFailed {
            remarks: "Feed".to_string(),
        })
        .expect("serialize notice code");

        assert_eq!(value["code"], "subscriptionAutoUpdateFailed");
        assert_eq!(value["remarks"], "Feed");
    }

    #[test]
    fn log_line_body_separates_core_output_from_app_authored_lines() {
        let core = serde_json::to_value(LogLineBody::Core {
            line: "inbound/mixed".to_string(),
        })
        .expect("serialize core line");
        let app = serde_json::to_value(LogLineBody::App {
            code: LogCode::RestartingAfterChange {
                reason: CoreFlowReason::DnsChanged,
            },
            detail: None,
        })
        .expect("serialize app line");

        assert_eq!(core["source"], "core");
        assert_eq!(core["line"], "inbound/mixed");
        assert_eq!(app["source"], "app");
        assert_eq!(app["code"]["code"], "restartingAfterChange");
        assert_eq!(app["code"]["reason"], "dnsChanged");
    }

    #[test]
    fn validation_issue_carries_its_breadcrumb() {
        let issue = ValidationIssue {
            field: "activeProfile".to_string(),
            code: ValidationCode::InvalidPort,
            scope: vec![ValidationScope::RoutingRuleOutbound {
                rule: "Group".to_string(),
                outbound: "Leaf".to_string(),
            }],
        };
        let value = serde_json::to_value(&issue).expect("serialize issue");

        assert_eq!(value["field"], "activeProfile");
        assert_eq!(value["code"]["code"], "invalidPort");
        assert_eq!(value["scope"][0]["kind"], "routingRuleOutbound");
        assert_eq!(value["scope"][0]["rule"], "Group");
    }

    #[test]
    fn untranslated_keeps_the_managers_own_diagnostic() {
        let issue = ValidationIssue::untranslated("pem", "certificate is not valid PEM");

        assert_eq!(
            issue.code,
            ValidationCode::Untranslated {
                message: "certificate is not valid PEM".to_string()
            }
        );
    }
}
