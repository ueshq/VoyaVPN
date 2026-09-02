//! Derivation and application of the Hiddify-style top-level connection mode
//! (proxy-only / system proxy / VPN) over the two persisted primitives:
//! `system_proxy_item.sys_proxy_type` and `tun_mode_item.enable_tun`.

use voya_contracts::ConnectionMode;
use voya_core::{AppConfig, SysProxyType};

/// Derives the current connection mode and whether PAC is the selected system
/// proxy flavor. TUN wins over everything; `Unchanged` counts as proxy-only
/// because the OS proxy is not being managed.
#[must_use]
pub fn derive_connection_mode(config: &AppConfig) -> (ConnectionMode, bool) {
    let pac_enabled = matches!(config.system_proxy_item.sys_proxy_type, SysProxyType::Pac);
    if config.tun_mode_item.enable_tun {
        return (ConnectionMode::Vpn, pac_enabled);
    }
    match config.system_proxy_item.sys_proxy_type {
        SysProxyType::Pac | SysProxyType::ForcedChange => {
            (ConnectionMode::SystemProxy, pac_enabled)
        }
        SysProxyType::ForcedClear | SysProxyType::Unchanged => (ConnectionMode::ProxyOnly, false),
    }
}

/// Applies a connection mode onto the config primitives. Entering `Vpn` keeps
/// the stored system proxy type untouched (runtime interplay rules already
/// force-disable or fall back per platform); leaving it turns TUN off.
pub fn apply_connection_mode(config: &mut AppConfig, mode: ConnectionMode, pac_enabled: bool) {
    match mode {
        ConnectionMode::ProxyOnly => {
            config.tun_mode_item.enable_tun = false;
            config.system_proxy_item.sys_proxy_type = SysProxyType::ForcedClear;
        }
        ConnectionMode::SystemProxy => {
            config.tun_mode_item.enable_tun = false;
            config.system_proxy_item.sys_proxy_type = if pac_enabled {
                SysProxyType::Pac
            } else {
                SysProxyType::ForcedChange
            };
        }
        ConnectionMode::Vpn => {
            config.tun_mode_item.enable_tun = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(sys_proxy: SysProxyType, tun: bool) -> AppConfig {
        let mut config = AppConfig::default();
        config.system_proxy_item.sys_proxy_type = sys_proxy;
        config.tun_mode_item.enable_tun = tun;
        config
    }

    #[test]
    fn derivation_covers_every_primitive_combination() {
        for (sys_proxy, tun, expected_mode, expected_pac) in [
            (
                SysProxyType::ForcedClear,
                false,
                ConnectionMode::ProxyOnly,
                false,
            ),
            (
                SysProxyType::Unchanged,
                false,
                ConnectionMode::ProxyOnly,
                false,
            ),
            (
                SysProxyType::ForcedChange,
                false,
                ConnectionMode::SystemProxy,
                false,
            ),
            (SysProxyType::Pac, false, ConnectionMode::SystemProxy, true),
            (SysProxyType::ForcedClear, true, ConnectionMode::Vpn, false),
            (SysProxyType::Pac, true, ConnectionMode::Vpn, true),
        ] {
            let (mode, pac) = derive_connection_mode(&config_with(sys_proxy, tun));
            assert_eq!(
                (mode, pac),
                (expected_mode, expected_pac),
                "{sys_proxy:?} tun={tun}"
            );
        }
    }

    #[test]
    fn apply_proxy_only_clears_both_primitives() {
        let mut config = config_with(SysProxyType::Pac, true);
        apply_connection_mode(&mut config, ConnectionMode::ProxyOnly, false);
        assert!(!config.tun_mode_item.enable_tun);
        assert_eq!(
            config.system_proxy_item.sys_proxy_type,
            SysProxyType::ForcedClear
        );
    }

    #[test]
    fn apply_system_proxy_selects_pac_flavor() {
        let mut config = config_with(SysProxyType::ForcedClear, true);
        apply_connection_mode(&mut config, ConnectionMode::SystemProxy, true);
        assert!(!config.tun_mode_item.enable_tun);
        assert_eq!(config.system_proxy_item.sys_proxy_type, SysProxyType::Pac);

        apply_connection_mode(&mut config, ConnectionMode::SystemProxy, false);
        assert_eq!(
            config.system_proxy_item.sys_proxy_type,
            SysProxyType::ForcedChange
        );
    }

    #[test]
    fn apply_vpn_preserves_stored_system_proxy_type() {
        let mut config = config_with(SysProxyType::ForcedChange, false);
        apply_connection_mode(&mut config, ConnectionMode::Vpn, false);
        assert!(config.tun_mode_item.enable_tun);
        assert_eq!(
            config.system_proxy_item.sys_proxy_type,
            SysProxyType::ForcedChange
        );
    }

    #[test]
    fn round_trip_apply_then_derive_is_stable() {
        for (mode, pac) in [
            (ConnectionMode::ProxyOnly, false),
            (ConnectionMode::SystemProxy, false),
            (ConnectionMode::SystemProxy, true),
            (ConnectionMode::Vpn, false),
        ] {
            let mut config = AppConfig::default();
            apply_connection_mode(&mut config, mode, pac);
            let (derived_mode, derived_pac) = derive_connection_mode(&config);
            assert_eq!(derived_mode, mode);
            if mode == ConnectionMode::SystemProxy {
                assert_eq!(derived_pac, pac);
            }
        }
    }
}
