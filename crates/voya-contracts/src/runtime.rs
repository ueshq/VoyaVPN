use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CoreType {
    #[default]
    SingBox,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SysProxyType {
    #[default]
    ForcedClear,
    ForcedChange,
    Unchanged,
    Pac,
}

/// Hiddify-style top-level connection mode. A derived view over the two
/// persisted primitives (system proxy type + TUN flag), never stored itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionMode {
    /// Local inbounds only; the OS proxy is cleared and TUN stays off.
    ProxyOnly,
    /// OS system proxy points at the local inbound (optionally via PAC).
    SystemProxy,
    /// TUN mode; all traffic is routed through the virtual interface.
    Vpn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionModeStatus {
    pub mode: ConnectionMode,
    pub pac_enabled: bool,
    pub pac_available: bool,
    pub vpn_available: bool,
    /// sing-box process rules only match traffic entering through TUN, so
    /// per-app rules are effective only while `mode` is `Vpn`.
    pub process_rules_effective: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerStatItem {
    pub index_id: String,
    #[specta(type = f64)]
    pub total_up: i64,
    #[specta(type = f64)]
    pub total_down: i64,
    #[specta(type = f64)]
    pub today_up: i64,
    #[specta(type = f64)]
    pub today_down: i64,
    #[specta(type = f64)]
    pub date_now: i64,
}
