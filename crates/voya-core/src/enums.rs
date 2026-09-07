use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigType {
    #[default]
    #[serde(rename = "vmess")]
    VMess,
    Custom,
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
    PolicyGroup,
    ProxyChain,
}

impl ConfigType {
    #[must_use]
    pub const fn is_complex_type(self) -> bool {
        matches!(self, Self::Custom | Self::PolicyGroup | Self::ProxyChain)
    }

    #[must_use]
    pub const fn is_group_type(self) -> bool {
        matches!(self, Self::PolicyGroup | Self::ProxyChain)
    }

    #[must_use]
    pub const fn sort_rank(self) -> u8 {
        match self {
            Self::VMess => 0,
            Self::Custom => 1,
            Self::Shadowsocks => 2,
            Self::SOCKS => 3,
            Self::VLESS => 4,
            Self::Trojan => 5,
            Self::Hysteria2 => 6,
            Self::TUIC => 7,
            Self::WireGuard => 8,
            Self::HTTP => 9,
            Self::Anytls => 10,
            Self::Naive => 11,
            Self::PolicyGroup => 12,
            Self::ProxyChain => 13,
        }
    }
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
pub enum MultipleLoad {
    /// Also the strategy a policy group decodes to when `strategy` is absent,
    /// matching the fallback the sing-box selector builder already uses.
    #[default]
    LeastPing,
    Fallback,
    Random,
    RoundRobin,
    LeastLoad,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrafficMode {
    #[default]
    Rule,
    Global,
    Direct,
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
