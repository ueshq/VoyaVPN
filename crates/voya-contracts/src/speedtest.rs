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
    Profiles { profile_ids: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeedtestRequest {
    pub target: SpeedtestTarget,
}

/// How a probe ended, as a code rather than a sentence.
///
/// Persisted in `profile_ex.message` using the same camelCase codes as IPC.
/// Unrecognised stored values become `Unknown` without hiding the profile.
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
    /// A stored value this build cannot classify, without dropping the row.
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

    /// Decode a current status code without failing the row on unknown values.
    #[must_use]
    pub fn from_stored(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(value);
        Some(Self::deserialize(deserializer).unwrap_or(Self::Unknown))
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
    pub country_code: Option<String>,
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
        let target = serde_json::json!({ "scope": "profiles", "profileIds": ["node-1"] });
        for kind in ["tcpConnect", "latency", "udp", "download", "mixed"] {
            assert!(
                serde_json::from_value::<SpeedtestRequest>(serde_json::json!({
                    "kind": kind, "target": target
                }))
                .is_err()
            );
        }
        assert!(serde_json::from_value::<SpeedtestRequest>(
            serde_json::json!({ "target": target })
        )
        .is_ok());
    }

    #[test]
    fn requests_can_no_longer_target_every_node() {
        assert!(
            serde_json::from_value::<SpeedtestRequest>(serde_json::json!({
                "target": { "scope": "all" }
            }))
            .is_err()
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
    fn noncanonical_stored_values_are_unknown() {
        for stored in [
            "Speedtesting",
            "Speedtesting wait",
            "request timed out",
            "proxy connection failed",
            "proxy connection refused",
            "proxy connection closed",
            "UDP test failed",
            "udpTestFailed",
            "Skipped",
            "42",
            "2048",
            "-1",
            "NaN",
            "a future status code",
        ] {
            assert_eq!(
                SpeedtestOutcome::from_stored(stored),
                Some(SpeedtestOutcome::Unknown),
                "noncanonical value `{stored}` must not be interpreted as a current status"
            );
        }
        assert_eq!(SpeedtestOutcome::from_stored(""), None);
        assert_eq!(SpeedtestOutcome::from_stored("   "), None);
        assert_eq!(
            SpeedtestOutcome::from_stored(" timedOut "),
            Some(SpeedtestOutcome::TimedOut)
        );
    }
}
