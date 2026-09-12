use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigType {
    #[default]
    #[serde(rename = "vmess")]
    VMess,
    Shadowsocks,
    #[serde(rename = "socks")]
    SOCKS,
    #[serde(rename = "vless")]
    VLESS,
    Trojan,
    Hysteria2,
    #[serde(rename = "tuic")]
    TUIC,
    WireGuard,
    #[serde(rename = "http")]
    HTTP,
    Anytls,
    Naive,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CoreType {
    #[default]
    #[serde(rename = "singBox")]
    sing_box,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InboundProtocol {
    socks,
    socks2,
    socks3,
    pac,
    api,
    api2,
    mixed,
    speedtest,
}

impl InboundProtocol {
    #[must_use]
    pub const fn port_offset(self) -> i32 {
        match self {
            Self::socks => 0,
            Self::socks2 => 1,
            Self::socks3 => 2,
            Self::pac => 3,
            Self::api => 4,
            Self::api2 => 5,
            Self::mixed => 6,
            Self::speedtest => 21,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MoveAction {
    Top,
    Up,
    Down,
    Bottom,
    Position,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrafficMode {
    #[default]
    Rule,
    Global,
    Unchanged,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleType {
    #[serde(rename = "all")]
    ALL,
    Routing,
    #[serde(rename = "dns")]
    DNS,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SysProxyType {
    #[default]
    ForcedClear,
    ForcedChange,
    Unchanged,
    Pac,
}
