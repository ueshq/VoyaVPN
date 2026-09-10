use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(
    tag = "scope",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SpeedtestTarget {
    All,
    Profiles { profile_ids: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeedtestRequest {
    pub target: SpeedtestTarget,
}

/// How a probe ended, as a code rather than a sentence.
///
/// This is **persisted**: it is what `profile_ex.message` holds, so the prose
/// that used to live there ("Speedtesting", "request timed out", "Skipped")
/// froze the user's language at the moment the test ran. Rows written by
/// earlier builds still decode — see [`SpeedtestOutcome::from_stored`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SpeedtestOutcome {
    /// Selected and queued behind another probe.
    Waiting,
    /// The probe is running right now.
    Testing,
    /// The probe finished; `delay` carries the measurement.
    Completed,
    TimedOut,
    ProxyConnectFailed,
    ProxyConnectionRefused,
    ProxyConnectionClosed,
    Cancelled,
    /// The run ended before this profile's turn came up.
    Skipped,
    /// The profile's own configuration is invalid, so nothing was probed.
    InvalidProfile,
    /// The test core could not be started, or its config could not be written.
    CoreUnavailable,
    /// No free local port was available for the probe.
    NoAvailablePort,
    /// A failure with no more specific code.
    Failed,
    /// A stored value this build cannot classify — written by a build that
    /// spelled the column differently. Better than silently dropping the row.
    Unknown,
}

impl SpeedtestOutcome {
    /// The exact text written into `profile_ex.message`.
    ///
    /// The serde tag, spelled once here so the persisted vocabulary and the
    /// wire vocabulary can never drift apart.
    #[must_use]
    pub const fn as_stored(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Testing => "testing",
            Self::Completed => "completed",
            Self::TimedOut => "timedOut",
            Self::ProxyConnectFailed => "proxyConnectFailed",
            Self::ProxyConnectionRefused => "proxyConnectionRefused",
            Self::ProxyConnectionClosed => "proxyConnectionClosed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
            Self::InvalidProfile => "invalidProfile",
            Self::CoreUnavailable => "coreUnavailable",
            Self::NoAvailablePort => "noAvailablePort",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    /// Decode one stored `profile_ex.message` value.
    ///
    /// Three generations of value can be in the column and all of them have to
    /// come back as something the UI can render:
    ///
    /// 1. A code this build wrote — matched exactly.
    /// 2. The English prose earlier builds wrote ("Speedtesting wait",
    ///    "request timed out", "Skipped") — matched case-insensitively against
    ///    the table below, so an upgraded install keeps its last results.
    /// 3. A bare number, which is what the pre-code builds stored for a
    ///    successful latency or download probe (`delay.to_string()`).
    ///
    /// Anything else becomes [`SpeedtestOutcome::Unknown`] rather than failing
    /// the read: one unrecognisable cell must never hide the profile row.
    #[must_use]
    pub fn from_stored(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        let known = [
            Self::Waiting,
            Self::Testing,
            Self::Completed,
            Self::TimedOut,
            Self::ProxyConnectFailed,
            Self::ProxyConnectionRefused,
            Self::ProxyConnectionClosed,
            Self::Cancelled,
            Self::Skipped,
            Self::InvalidProfile,
            Self::CoreUnavailable,
            Self::NoAvailablePort,
            Self::Failed,
            Self::Unknown,
        ];
        if let Some(outcome) = known
            .into_iter()
            .find(|outcome| outcome.as_stored() == value)
        {
            return Some(outcome);
        }
        if value.parse::<f64>().is_ok() {
            return Some(Self::Completed);
        }

        Some(legacy_outcome(&value.to_ascii_lowercase()).unwrap_or(Self::Unknown))
    }
}

/// The prose earlier builds persisted, mapped onto the codes that replaced it.
fn legacy_outcome(lowercase: &str) -> Option<SpeedtestOutcome> {
    match lowercase {
        "speedtesting" => Some(SpeedtestOutcome::Testing),
        "speedtesting wait" => Some(SpeedtestOutcome::Waiting),
        "request timed out" => Some(SpeedtestOutcome::TimedOut),
        "proxy connection failed" => Some(SpeedtestOutcome::ProxyConnectFailed),
        "proxy connection refused" => Some(SpeedtestOutcome::ProxyConnectionRefused),
        "proxy connection closed" => Some(SpeedtestOutcome::ProxyConnectionClosed),
        "udp test failed" | "udptestfailed" => Some(SpeedtestOutcome::Failed),
        "skipped" => Some(SpeedtestOutcome::Skipped),
        _ => None,
    }
}

/// One profile's probe result.
///
/// `detail` is an untranslated technical line shown under the outcome. It is
/// never persisted and never carries a local filesystem path: the failure
/// classification used to fall through to the error's own `Display`, which for
/// a config-write failure printed the app-data directory into a column the
/// profile table renders.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeedtestResult {
    pub index_id: String,
    pub delay: Option<i32>,
    pub outcome: SpeedtestOutcome,
    pub detail: Option<String>,
    pub ip_info: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeedtestRunResult {
    pub cancelled: bool,
    pub selected_count: u32,
    pub completed_count: u32,
    pub results: Vec<SpeedtestResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeedtestStatus {
    pub running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_contract_uses_explicit_target() {
        let request = SpeedtestRequest {
            target: SpeedtestTarget::Profiles {
                profile_ids: vec!["node-1".to_string()],
            },
        };
        let value = serde_json::to_value(request).expect("serialize speed test request");

        assert!(value.get("kind").is_none());
        assert_eq!(value["target"]["scope"], "profiles");
        assert_eq!(value["target"]["profileIds"][0], "node-1");
    }

    #[test]
    fn requests_no_longer_accept_a_probe_kind() {
        for kind in ["tcpConnect", "latency", "udp", "download", "mixed"] {
            assert!(
                serde_json::from_value::<SpeedtestRequest>(serde_json::json!({
                    "kind": kind, "target": { "scope": "all" }
                }))
                .is_err()
            );
        }
        assert!(
            serde_json::from_value::<SpeedtestRequest>(serde_json::json!({
                "target": { "scope": "all" }
            }))
            .is_ok()
        );
    }

    #[test]
    fn stored_outcomes_round_trip_through_their_persisted_spelling() {
        for outcome in [
            SpeedtestOutcome::Waiting,
            SpeedtestOutcome::Testing,
            SpeedtestOutcome::Completed,
            SpeedtestOutcome::TimedOut,
            SpeedtestOutcome::ProxyConnectFailed,
            SpeedtestOutcome::ProxyConnectionRefused,
            SpeedtestOutcome::ProxyConnectionClosed,
            SpeedtestOutcome::Cancelled,
            SpeedtestOutcome::Skipped,
            SpeedtestOutcome::InvalidProfile,
            SpeedtestOutcome::CoreUnavailable,
            SpeedtestOutcome::NoAvailablePort,
            SpeedtestOutcome::Failed,
            SpeedtestOutcome::Unknown,
        ] {
            assert_eq!(
                SpeedtestOutcome::from_stored(outcome.as_stored()),
                Some(outcome),
                "{outcome:?} did not survive its stored spelling"
            );
            // The persisted spelling has to be the wire spelling too, or an
            // upgrade would need a second vocabulary to translate between them.
            assert_eq!(
                serde_json::to_value(outcome).expect("serialize outcome"),
                serde_json::Value::String(outcome.as_stored().to_string())
            );
        }
    }

    #[test]
    fn rows_written_by_earlier_builds_still_decode() {
        // Exactly what `clear_previous_results`, `speedtest_error_message` and
        // `run_realping` used to write into `profile_ex.message` / `ip_info`.
        let legacy: &[(&str, SpeedtestOutcome)] = &[
            ("Speedtesting", SpeedtestOutcome::Testing),
            ("Speedtesting wait", SpeedtestOutcome::Waiting),
            ("request timed out", SpeedtestOutcome::TimedOut),
            (
                "proxy connection failed",
                SpeedtestOutcome::ProxyConnectFailed,
            ),
            (
                "proxy connection refused",
                SpeedtestOutcome::ProxyConnectionRefused,
            ),
            (
                "proxy connection closed",
                SpeedtestOutcome::ProxyConnectionClosed,
            ),
            ("UDP test failed", SpeedtestOutcome::Failed),
            ("udpTestFailed", SpeedtestOutcome::Failed),
            ("cancelled", SpeedtestOutcome::Cancelled),
            ("Skipped", SpeedtestOutcome::Skipped),
            // A successful latency probe stored the millisecond count, and a
            // download probe the byte rate.
            ("42", SpeedtestOutcome::Completed),
            ("2048", SpeedtestOutcome::Completed),
            ("-1", SpeedtestOutcome::Completed),
            // Anything else must degrade, never fail the row.
            (
                "failed to write speedtest config /Users/someone/Library/config.json: denied",
                SpeedtestOutcome::Unknown,
            ),
        ];

        for (stored, expected) in legacy {
            assert_eq!(
                SpeedtestOutcome::from_stored(stored),
                Some(*expected),
                "legacy value `{stored}` decoded wrongly"
            );
        }

        assert_eq!(SpeedtestOutcome::from_stored(""), None);
        assert_eq!(SpeedtestOutcome::from_stored("   "), None);
    }
}
