use std::{error::Error, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use specta::Type;

macro_rules! int_enum {
    ($name:ident { $($variant:ident = $value:expr),+ $(,)? }) => {
        impl $name {
            #[must_use]
            pub const fn as_i32(self) -> i32 {
                self as i32
            }

            #[must_use]
            pub const fn from_i32(value: i32) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl From<$name> for i32 {
            fn from(value: $name) -> Self {
                value.as_i32()
            }
        }

        impl TryFrom<i32> for $name {
            type Error = EnumDiscriminantError;

            fn try_from(value: i32) -> Result<Self, Self::Error> {
                Self::from_i32(value).ok_or(EnumDiscriminantError {
                    enum_name: stringify!($name),
                    value,
                })
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_i32(self.as_i32())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = i32::deserialize(deserializer)?;
                Self::try_from(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumDiscriminantError {
    pub enum_name: &'static str,
    pub value: i32,
}

impl fmt::Display for EnumDiscriminantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown {} discriminant {}",
            self.enum_name, self.value
        )
    }
}

impl Error for EnumDiscriminantError {}

#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum ConfigType {
    #[default]
    VMess = 1,
    Custom = 2,
    Shadowsocks = 3,
    SOCKS = 4,
    VLESS = 5,
    Trojan = 6,
    Hysteria2 = 7,
    TUIC = 8,
    WireGuard = 9,
    HTTP = 10,
    Anytls = 11,
    Naive = 12,
    PolicyGroup = 101,
    ProxyChain = 102,
}

int_enum!(ConfigType {
    VMess = 1,
    Custom = 2,
    Shadowsocks = 3,
    SOCKS = 4,
    VLESS = 5,
    Trojan = 6,
    Hysteria2 = 7,
    TUIC = 8,
    WireGuard = 9,
    HTTP = 10,
    Anytls = 11,
    Naive = 12,
    PolicyGroup = 101,
    ProxyChain = 102,
});

impl ConfigType {
    #[must_use]
    pub const fn is_complex_type(self) -> bool {
        matches!(self, Self::Custom | Self::PolicyGroup | Self::ProxyChain)
    }

    #[must_use]
    pub const fn is_group_type(self) -> bool {
        matches!(self, Self::PolicyGroup | Self::ProxyChain)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum CoreType {
    v2fly = 1,
    #[default]
    Xray = 2,
    v2fly_v5 = 4,
    mihomo = 13,
    hysteria = 21,
    naiveproxy = 22,
    tuic = 23,
    sing_box = 24,
    juicity = 25,
    hysteria2 = 26,
    brook = 27,
    overtls = 28,
    shadowquic = 29,
    mieru = 30,
    v2rayN = 99,
}

int_enum!(CoreType {
    v2fly = 1,
    Xray = 2,
    v2fly_v5 = 4,
    mihomo = 13,
    hysteria = 21,
    naiveproxy = 22,
    tuic = 23,
    sing_box = 24,
    juicity = 25,
    hysteria2 = 26,
    brook = 27,
    overtls = 28,
    shadowquic = 29,
    mieru = 30,
    v2rayN = 99,
});

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum GridOrientation {
    Horizontal = 0,
    #[default]
    Vertical = 1,
    Tab = 2,
}

int_enum!(GridOrientation {
    Horizontal = 0,
    Vertical = 1,
    Tab = 2,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum GlobalHotkey {
    ShowForm = 0,
    SystemProxyClear = 1,
    SystemProxySet = 2,
    SystemProxyUnchanged = 3,
    SystemProxyPac = 4,
}

int_enum!(GlobalHotkey {
    ShowForm = 0,
    SystemProxyClear = 1,
    SystemProxySet = 2,
    SystemProxyUnchanged = 3,
    SystemProxyPac = 4,
});

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum InboundProtocol {
    socks = 0,
    socks2 = 1,
    socks3 = 2,
    pac = 3,
    api = 4,
    api2 = 5,
    mixed = 6,
    speedtest = 21,
}

int_enum!(InboundProtocol {
    socks = 0,
    socks2 = 1,
    socks3 = 2,
    pac = 3,
    api = 4,
    api2 = 5,
    mixed = 6,
    speedtest = 21,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum MoveAction {
    Top = 1,
    Up = 2,
    Down = 3,
    Bottom = 4,
    Position = 5,
}

int_enum!(MoveAction {
    Top = 1,
    Up = 2,
    Down = 3,
    Bottom = 4,
    Position = 5,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum MultipleLoad {
    LeastPing = 0,
    Fallback = 1,
    Random = 2,
    RoundRobin = 3,
    LeastLoad = 4,
}

int_enum!(MultipleLoad {
    LeastPing = 0,
    Fallback = 1,
    Random = 2,
    RoundRobin = 3,
    LeastLoad = 4,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum PresetType {
    Default = 0,
    Russia = 1,
    Iran = 2,
}

int_enum!(PresetType {
    Default = 0,
    Russia = 1,
    Iran = 2,
});

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum RuleMode {
    #[default]
    Rule = 0,
    Global = 1,
    Direct = 2,
    Unchanged = 3,
}

int_enum!(RuleMode {
    Rule = 0,
    Global = 1,
    Direct = 2,
    Unchanged = 3,
});

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum RuleType {
    ALL = 0,
    Routing = 1,
    DNS = 2,
}

int_enum!(RuleType {
    ALL = 0,
    Routing = 1,
    DNS = 2,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum ServerColumnName {
    Def = 0,
    ConfigType = 1,
    Remarks = 2,
    Address = 3,
    Port = 4,
    Network = 5,
    StreamSecurity = 6,
    SubRemarks = 7,
    DelayVal = 8,
    SpeedVal = 9,
    IpInfo = 10,
    TodayDown = 11,
    TodayUp = 12,
    TotalDown = 13,
    TotalUp = 14,
}

int_enum!(ServerColumnName {
    Def = 0,
    ConfigType = 1,
    Remarks = 2,
    Address = 3,
    Port = 4,
    Network = 5,
    StreamSecurity = 6,
    SubRemarks = 7,
    DelayVal = 8,
    SpeedVal = 9,
    IpInfo = 10,
    TodayDown = 11,
    TodayUp = 12,
    TotalDown = 13,
    TotalUp = 14,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum SpeedActionType {
    Tcping = 0,
    Realping = 1,
    UdpTest = 2,
    Speedtest = 3,
    Mixedtest = 4,
    FastRealping = 5,
}

int_enum!(SpeedActionType {
    Tcping = 0,
    Realping = 1,
    UdpTest = 2,
    Speedtest = 3,
    Mixedtest = 4,
    FastRealping = 5,
});

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum SysProxyType {
    #[default]
    ForcedClear = 0,
    ForcedChange = 1,
    Unchanged = 2,
    Pac = 3,
}

int_enum!(SysProxyType {
    ForcedClear = 0,
    ForcedChange = 1,
    Unchanged = 2,
    Pac = 3,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum Theme {
    FollowSystem = 0,
    Dark = 1,
    Light = 2,
    Aquatic = 3,
    Desert = 4,
    Dusk = 5,
    NightSky = 6,
}

int_enum!(Theme {
    FollowSystem = 0,
    Dark = 1,
    Light = 2,
    Aquatic = 3,
    Desert = 4,
    Dusk = 5,
    NightSky = 6,
});

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum Transport {
    raw = 0,
    kcp = 1,
    ws = 2,
    httpupgrade = 3,
    xhttp = 4,
    h2 = 5,
    http = 6,
    quic = 7,
    grpc = 8,
}

int_enum!(Transport {
    raw = 0,
    kcp = 1,
    ws = 2,
    httpupgrade = 3,
    xhttp = 4,
    h2 = 5,
    http = 6,
    quic = 7,
    grpc = 8,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[repr(i32)]
#[specta(type = i32)]
pub enum ViewAction {
    CloseWindow = 0,
    ShowYesNo = 1,
    SaveFileDialog = 2,
    AddBatchRoutingRulesYesNo = 3,
    SetClipboardData = 4,
    AddServerViaClipboard = 5,
    ImportRulesFromClipboard = 6,
    ProfilesFocus = 7,
    ShareSub = 8,
    ShareServer = 9,
    ScanScreenTask = 10,
    ScanImageTask = 11,
    BrowseServer = 12,
    ImportRulesFromFile = 13,
    InitSettingFont = 14,
    PasswordInput = 15,
    SubEditWindow = 16,
    RoutingRuleSettingWindow = 17,
    RoutingRuleDetailsWindow = 18,
    AddServerWindow = 19,
    AddServer2Window = 20,
    AddGroupServerWindow = 21,
    DNSSettingWindow = 22,
    RoutingSettingWindow = 23,
    OptionSettingWindow = 24,
    FullConfigTemplateWindow = 25,
    GlobalHotkeySettingWindow = 26,
    SubSettingWindow = 27,
    DispatcherRefreshServersBiz = 28,
    DispatcherRefreshIcon = 29,
    DispatcherShowMsg = 30,
}

int_enum!(ViewAction {
    CloseWindow = 0,
    ShowYesNo = 1,
    SaveFileDialog = 2,
    AddBatchRoutingRulesYesNo = 3,
    SetClipboardData = 4,
    AddServerViaClipboard = 5,
    ImportRulesFromClipboard = 6,
    ProfilesFocus = 7,
    ShareSub = 8,
    ShareServer = 9,
    ScanScreenTask = 10,
    ScanImageTask = 11,
    BrowseServer = 12,
    ImportRulesFromFile = 13,
    InitSettingFont = 14,
    PasswordInput = 15,
    SubEditWindow = 16,
    RoutingRuleSettingWindow = 17,
    RoutingRuleDetailsWindow = 18,
    AddServerWindow = 19,
    AddServer2Window = 20,
    AddGroupServerWindow = 21,
    DNSSettingWindow = 22,
    RoutingSettingWindow = 23,
    OptionSettingWindow = 24,
    FullConfigTemplateWindow = 25,
    GlobalHotkeySettingWindow = 26,
    SubSettingWindow = 27,
    DispatcherRefreshServersBiz = 28,
    DispatcherRefreshIcon = 29,
    DispatcherShowMsg = 30,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_type_discriminants_match_v2rayn() {
        assert_eq!(ConfigType::VMess.as_i32(), 1);
        assert_eq!(ConfigType::Custom.as_i32(), 2);
        assert_eq!(ConfigType::Shadowsocks.as_i32(), 3);
        assert_eq!(ConfigType::SOCKS.as_i32(), 4);
        assert_eq!(ConfigType::VLESS.as_i32(), 5);
        assert_eq!(ConfigType::Trojan.as_i32(), 6);
        assert_eq!(ConfigType::Hysteria2.as_i32(), 7);
        assert_eq!(ConfigType::TUIC.as_i32(), 8);
        assert_eq!(ConfigType::WireGuard.as_i32(), 9);
        assert_eq!(ConfigType::HTTP.as_i32(), 10);
        assert_eq!(ConfigType::Anytls.as_i32(), 11);
        assert_eq!(ConfigType::Naive.as_i32(), 12);
        assert_eq!(ConfigType::PolicyGroup.as_i32(), 101);
        assert_eq!(ConfigType::ProxyChain.as_i32(), 102);
    }

    #[test]
    fn core_type_discriminants_match_v2rayn() {
        assert_eq!(CoreType::v2fly.as_i32(), 1);
        assert_eq!(CoreType::Xray.as_i32(), 2);
        assert_eq!(CoreType::v2fly_v5.as_i32(), 4);
        assert_eq!(CoreType::mihomo.as_i32(), 13);
        assert_eq!(CoreType::hysteria.as_i32(), 21);
        assert_eq!(CoreType::naiveproxy.as_i32(), 22);
        assert_eq!(CoreType::tuic.as_i32(), 23);
        assert_eq!(CoreType::sing_box.as_i32(), 24);
        assert_eq!(CoreType::juicity.as_i32(), 25);
        assert_eq!(CoreType::hysteria2.as_i32(), 26);
        assert_eq!(CoreType::brook.as_i32(), 27);
        assert_eq!(CoreType::overtls.as_i32(), 28);
        assert_eq!(CoreType::shadowquic.as_i32(), 29);
        assert_eq!(CoreType::mieru.as_i32(), 30);
        assert_eq!(CoreType::v2rayN.as_i32(), 99);
    }

    #[test]
    fn enum_json_uses_integer_discriminants() {
        assert_eq!(
            serde_json::to_string(&ConfigType::VLESS)
                .expect("config type should serialize as integer discriminant"),
            "5"
        );
        assert_eq!(
            serde_json::from_str::<CoreType>("24")
                .expect("core type integer discriminant should deserialize"),
            CoreType::sing_box
        );
    }
}
