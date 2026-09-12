use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TrafficMode {
    #[default]
    Rule,
    Global,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrafficModeResponse {
    pub mode: TrafficMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProxyConnectionsSnapshot {
    #[specta(type = f64)]
    pub download_total: u64,
    #[specta(type = f64)]
    pub upload_total: u64,
    pub connections: Vec<ProxyConnectionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProxyConnectionItem {
    pub id: Option<String>,
    pub network: Option<String>,
    pub connection_type: Option<String>,
    pub host: String,
    pub source: String,
    pub destination: String,
    #[specta(type = f64)]
    pub upload: u64,
    #[specta(type = f64)]
    pub download: u64,
    pub start: String,
    pub chains: Vec<String>,
    pub rule: Option<String>,
    pub rule_payload: Option<String>,
    pub process: Option<String>,
    pub process_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProxyMonitorState {
    Running,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProxyMonitorStatus {
    pub state: ProxyMonitorState,
    pub running: bool,
    pub stale: bool,
    pub message: Option<String>,
}

impl ProxyMonitorStatus {
    #[must_use]
    pub fn running() -> Self {
        Self {
            state: ProxyMonitorState::Running,
            running: true,
            stale: false,
            message: None,
        }
    }

    #[must_use]
    pub fn stopped() -> Self {
        Self {
            state: ProxyMonitorState::Stopped,
            running: false,
            stale: true,
            message: None,
        }
    }

    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            state: ProxyMonitorState::Failed,
            running: false,
            stale: true,
            message: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::TrafficMode;

    #[test]
    fn traffic_mode_rejects_numeric_and_pascal_case_values() {
        assert!(serde_json::from_value::<TrafficMode>(serde_json::json!(0)).is_err());
        assert!(serde_json::from_value::<TrafficMode>(serde_json::json!("Rule")).is_err());
        assert!(serde_json::from_value::<TrafficMode>(serde_json::json!("direct")).is_err());
        assert_eq!(
            serde_json::from_value::<TrafficMode>(serde_json::json!("rule"))
                .expect("camelCase string enum should be accepted"),
            TrafficMode::Rule
        );
    }
}
