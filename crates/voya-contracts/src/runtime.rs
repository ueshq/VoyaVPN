use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CoreType {
    #[default]
    SingBox,
}

/// What the core process is doing right now.
///
/// The single vocabulary for the runtime's connection state: the status command
/// returns it inside [`crate::RuntimeStatusResponse`] and the transient
/// `coreState` stream carries the very same struct, so there is nothing to
/// translate between a response and an event. `Connecting`/`Disconnecting` are
/// transitions only the event stream ever reports — a status read observes a
/// settled supervisor and answers `Connected`, `Disconnected`, or `CleanupPending`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CoreState {
    CleanupPending,
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SystemProxyType {
    #[default]
    ForcedClear,
    ForcedChange,
    Unchanged,
}

/// How traffic is captured. A derived view over the two persisted primitives
/// (system proxy type + TUN flag), never stored itself. It is independent of
/// the traffic mode, which decides where captured traffic goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionMode {
    /// Local inbound for system proxy use. Windows and Linux only.
    SystemProxy,
    /// TUN mode; all traffic is routed through the virtual interface.
    Vpn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionModeStatus {
    pub mode: ConnectionMode,
    pub vpn_available: bool,
    /// Whether the platform offers the system proxy mode at all. macOS only
    /// captures traffic through its PacketTunnel VPN.
    pub system_proxy_available: bool,
    /// Whether the platform's tunnel can match traffic by process. The macOS
    /// NetworkExtension tunnel cannot, so per-app rules are not offered there.
    pub process_rules_supported: bool,
    /// sing-box process rules only match traffic entering through TUN, so
    /// per-app rules are effective only while they are supported and `mode` is
    /// `Vpn`.
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
