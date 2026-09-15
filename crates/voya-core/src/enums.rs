use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConfigType {
    #[default]
    VMess,
    Shadowsocks,
    SOCKS,
    VLESS,
    Trojan,
    Hysteria2,
    TUIC,
    WireGuard,
    HTTP,
    Anytls,
    Naive,
}

impl ConfigType {
    /// The spelling stored in `profile_items.config_type`.
    ///
    /// Share-link schemes and sing-box outbound types are separate tables:
    /// the three vocabularies disagree (`wireGuard`, `ss://`, `shadowsocks`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VMess => "vmess",
            Self::Shadowsocks => "shadowsocks",
            Self::SOCKS => "socks",
            Self::VLESS => "vless",
            Self::Trojan => "trojan",
            Self::Hysteria2 => "hysteria2",
            Self::TUIC => "tuic",
            Self::WireGuard => "wireGuard",
            Self::HTTP => "http",
            Self::Anytls => "anytls",
            Self::Naive => "naive",
        }
    }
}

/// A stored config type this build does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unknown config type")]
pub struct UnknownConfigType;

impl FromStr for ConfigType {
    type Err = UnknownConfigType;

    /// Parses exactly the spellings [`ConfigType::as_str`] produces.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "vmess" => Ok(Self::VMess),
            "shadowsocks" => Ok(Self::Shadowsocks),
            "socks" => Ok(Self::SOCKS),
            "vless" => Ok(Self::VLESS),
            "trojan" => Ok(Self::Trojan),
            "hysteria2" => Ok(Self::Hysteria2),
            "tuic" => Ok(Self::TUIC),
            "wireGuard" => Ok(Self::WireGuard),
            "http" => Ok(Self::HTTP),
            "anytls" => Ok(Self::Anytls),
            "naive" => Ok(Self::Naive),
            _ => Err(UnknownConfigType),
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundProtocol {
    socks,
    socks2,
    socks3,
    api2,
    speedtest,
}

impl InboundProtocol {
    #[must_use]
    pub const fn port_offset(self) -> i32 {
        match self {
            Self::socks => 0,
            Self::socks2 => 1,
            Self::socks3 => 2,
            // Offset 3 belonged to the retired local PAC listener, and 4 and 6
            // to listeners nothing opened. The remaining offsets keep their
            // values so generated ports stay stable.
            Self::api2 => 5,
            Self::speedtest => 21,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveAction {
    Top,
    Up,
    Down,
    Bottom,
    Position,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SysProxyType {
    #[default]
    ForcedClear,
    ForcedChange,
    Unchanged,
}

/// How TLS handshakes are split; see `voya_contracts::TlsFragmentMode`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TlsFragmentMode {
    #[default]
    Off,
    TlsHello,
    Record,
}

/// What closing the main window does; see `voya_contracts::CloseAction`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CloseAction {
    #[default]
    MinimizeToTray,
    Quit,
    Ask,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_types_keep_their_stored_spelling() {
        // Every variant, with the text existing databases already hold.
        let stored = [
            (ConfigType::VMess, "vmess"),
            (ConfigType::Shadowsocks, "shadowsocks"),
            (ConfigType::SOCKS, "socks"),
            (ConfigType::VLESS, "vless"),
            (ConfigType::Trojan, "trojan"),
            (ConfigType::Hysteria2, "hysteria2"),
            (ConfigType::TUIC, "tuic"),
            (ConfigType::WireGuard, "wireGuard"),
            (ConfigType::HTTP, "http"),
            (ConfigType::Anytls, "anytls"),
            (ConfigType::Naive, "naive"),
        ];

        for (config_type, spelling) in stored {
            assert_eq!(config_type.as_str(), spelling);
            assert_eq!(spelling.parse::<ConfigType>(), Ok(config_type));
        }
        assert_eq!("wireguard".parse::<ConfigType>(), Err(UnknownConfigType));
        assert_eq!("VMess".parse::<ConfigType>(), Err(UnknownConfigType));
    }
}
