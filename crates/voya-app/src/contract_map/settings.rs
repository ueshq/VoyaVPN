//! The settings contract mapped onto the domain configuration and back.

use voya_contracts as contracts;
use voya_core::{
    AppConfig, AppearanceConfig, BehaviorConfig, CoreConfig, HysteriaConfig, InboundConfig,
    MultiplexingConfig, ProxyConfig, SpeedtestConfig, SystemProxyConfig, TunConfig,
};
use voya_db::AppStateRecord;

use super::{
    close_action_from_contract, close_action_to_contract, dns_from_contract, dns_to_contract,
    sysproxy_type_from_contract, sysproxy_type_to_contract, tls_fragment_mode_from_contract,
    tls_fragment_mode_to_contract, traffic_mode_from_contract, traffic_mode_to_contract,
};

#[must_use]
pub fn settings_from_app_config(config: &AppConfig) -> contracts::AppSettings {
    contracts::AppSettings {
        appearance: contracts::AppearanceSettings {
            language: config.appearance.language.clone(),
            theme: theme_from_config(config.appearance.theme.as_deref()),
        },
        behavior: contracts::BehaviorSettings {
            autostart: config.behavior.autostart,
            auto_check_ip: config.behavior.auto_check_ip,
            close_action: close_action_to_contract(config.behavior.close_action),
            start_minimized: config.behavior.start_minimized,
            auto_create_subscription_group: config.behavior.auto_create_subscription_group,
        },
        core: contracts::CoreSettings {
            log_enabled: config.core.log_enabled,
            log_level: config.core.log_level.clone(),
            mux_enabled: config.core.mux_enabled,
            default_allow_insecure: config.core.default_allow_insecure,
            default_fingerprint: config.core.default_fingerprint.clone(),
            default_user_agent: config.core.default_user_agent.clone(),
            send_through: config.core.send_through.clone(),
            bind_interface: config.core.bind_interface.clone(),
            tls_fragment: tls_fragment_mode_to_contract(config.core.tls_fragment),
            fragment_fallback_delay_ms: config.core.fragment_fallback_delay_ms,
            cache_file_enabled: config.core.cache_file_enabled,
        },
        network: contracts::NetworkSettings {
            tun: contracts::TunSettings {
                enabled: config.tun.enabled,
                auto_route: config.tun.auto_route,
                strict_route: config.tun.strict_route,
                stack: config.tun.stack.clone(),
                mtu: config.tun.mtu,
                ipv6_enabled: config.tun.ipv6_enabled,
                icmp_routing: config.tun.icmp_routing.clone(),
            },
            system_proxy: contracts::SystemProxySettings {
                mode: sysproxy_type_to_contract(config.system_proxy.mode),
                exceptions: config.system_proxy.exceptions.clone(),
                bypass_local: config.system_proxy.bypass_local,
            },
            inbounds: config
                .inbounds
                .iter()
                .map(|item| contracts::InboundSettings {
                    local_port: item.local_port,
                    sniffing_enabled: item.sniffing_enabled,
                    lan_connections_allowed: item.lan_connections_allowed,
                    separate_lan_port: item.separate_lan_port,
                    username: item.username.clone(),
                    password: item.password.clone(),
                    secondary_port_enabled: item.secondary_port_enabled,
                })
                .collect(),
        },
        dns: dns_to_contract(config.dns.clone()),
        speed_test: contracts::SpeedtestSettings {
            timeout_seconds: config.speed_test.timeout_seconds,
            latency_url: config.speed_test.latency_url.clone(),
            ip_lookup_url: config.speed_test.ip_lookup_url.clone(),
            page_size: config.speed_test.page_size,
            delay_interval_seconds: config.speed_test.delay_interval_seconds,
        },
        multiplexing: contracts::MultiplexingSettings {
            protocol: config.multiplexing.protocol.clone(),
            max_connections: config.multiplexing.max_connections,
            padding: config.multiplexing.padding,
        },
        hysteria: contracts::HysteriaSettings {
            upload_mbps: config.hysteria.upload_mbps,
            download_mbps: config.hysteria.download_mbps,
            hop_interval_seconds: config.hysteria.hop_interval_seconds,
        },
        proxy: contracts::ProxySettings {
            traffic_mode: traffic_mode_to_contract(config.proxy.traffic_mode),
        },
    }
}

/// Map the settings contract onto a full `AppConfig`.
///
/// The active profile and routing ids are not part of the settings contract, so
/// they are carried over from the configuration being replaced.
#[must_use]
pub fn config_from_settings(settings: &contracts::AppSettings, current: &AppConfig) -> AppConfig {
    app_config_from_settings(settings, &state_from_app_config(current))
}

pub(crate) fn state_from_app_config(config: &AppConfig) -> AppStateRecord {
    AppStateRecord {
        active_profile_id: (!config.active_profile_id.is_empty())
            .then(|| config.active_profile_id.clone()),
        active_routing_id: (!config.active_routing_id.is_empty())
            .then(|| config.active_routing_id.clone()),
        active_group_id: (!config.active_group_id.is_empty())
            .then(|| config.active_group_id.clone()),
    }
}

#[must_use]
pub fn app_config_from_settings(
    settings: &contracts::AppSettings,
    state: &AppStateRecord,
) -> AppConfig {
    AppConfig {
        active_profile_id: state.active_profile_id.clone().unwrap_or_default(),
        active_group_id: state.active_group_id.clone().unwrap_or_default(),
        active_routing_id: state.active_routing_id.clone().unwrap_or_default(),
        core: CoreConfig {
            log_enabled: settings.core.log_enabled,
            log_level: settings.core.log_level.clone(),
            mux_enabled: settings.core.mux_enabled,
            default_allow_insecure: settings.core.default_allow_insecure,
            default_fingerprint: settings.core.default_fingerprint.clone(),
            default_user_agent: settings.core.default_user_agent.clone(),
            send_through: settings.core.send_through.clone(),
            bind_interface: settings.core.bind_interface.clone(),
            tls_fragment: tls_fragment_mode_from_contract(settings.core.tls_fragment),
            fragment_fallback_delay_ms: settings.core.fragment_fallback_delay_ms,
            cache_file_enabled: settings.core.cache_file_enabled,
        },
        tun: TunConfig {
            enabled: settings.network.tun.enabled,
            auto_route: settings.network.tun.auto_route,
            strict_route: settings.network.tun.strict_route,
            stack: settings.network.tun.stack.clone(),
            mtu: settings.network.tun.mtu,
            ipv6_enabled: settings.network.tun.ipv6_enabled,
            icmp_routing: settings.network.tun.icmp_routing.clone(),
        },
        behavior: BehaviorConfig {
            autostart: settings.behavior.autostart,
            auto_check_ip: settings.behavior.auto_check_ip,
            close_action: close_action_from_contract(settings.behavior.close_action),
            start_minimized: settings.behavior.start_minimized,
            auto_create_subscription_group: settings.behavior.auto_create_subscription_group,
        },
        appearance: AppearanceConfig {
            theme: theme_to_config(settings.appearance.theme).map(str::to_string),
            language: settings.appearance.language.clone(),
        },
        speed_test: SpeedtestConfig {
            timeout_seconds: settings.speed_test.timeout_seconds,
            latency_url: settings.speed_test.latency_url.clone(),
            ip_lookup_url: settings.speed_test.ip_lookup_url.clone(),
            page_size: settings.speed_test.page_size,
            delay_interval_seconds: settings.speed_test.delay_interval_seconds,
        },
        multiplexing: MultiplexingConfig {
            protocol: settings.multiplexing.protocol.clone(),
            max_connections: settings.multiplexing.max_connections,
            padding: settings.multiplexing.padding,
        },
        hysteria: HysteriaConfig {
            upload_mbps: settings.hysteria.upload_mbps,
            download_mbps: settings.hysteria.download_mbps,
            hop_interval_seconds: settings.hysteria.hop_interval_seconds,
        },
        proxy: ProxyConfig {
            traffic_mode: traffic_mode_from_contract(settings.proxy.traffic_mode),
        },
        system_proxy: SystemProxyConfig {
            mode: sysproxy_type_from_contract(settings.network.system_proxy.mode),
            exceptions: settings.network.system_proxy.exceptions.clone(),
            bypass_local: settings.network.system_proxy.bypass_local,
        },
        inbounds: settings
            .network
            .inbounds
            .iter()
            .map(|item| InboundConfig {
                local_port: item.local_port,
                sniffing_enabled: item.sniffing_enabled,
                lan_connections_allowed: item.lan_connections_allowed,
                separate_lan_port: item.separate_lan_port,
                username: item.username.clone(),
                password: item.password.clone(),
                secondary_port_enabled: item.secondary_port_enabled,
            })
            .collect(),
        dns: dns_from_contract(settings.dns.clone()),
    }
}

/// `None` is the stored shape of "follow the system theme", matching
/// `AppearanceConfig::default()` so contract defaults map onto domain defaults exactly.
///
/// Unlike the system-proxy and traffic-mode tables that used to sit here, this
/// pair is *not* a restatement of serde's `rename_all = "camelCase"`: the domain
/// stores `Light`/`Dark`/absent, which is neither what `ThemeMode` serializes to
/// nor a shape `Option<ThemeMode>` could express. Deleting it would rewrite
/// `appearance.theme` on every install.
const fn theme_to_config(theme: contracts::ThemeMode) -> Option<&'static str> {
    match theme {
        contracts::ThemeMode::System => None,
        contracts::ThemeMode::Light => Some("Light"),
        contracts::ThemeMode::Dark => Some("Dark"),
    }
}

fn theme_from_config(value: Option<&str>) -> contracts::ThemeMode {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("light") => contracts::ThemeMode::Light,
        Some("dark") => contracts::ThemeMode::Dark,
        _ => contracts::ThemeMode::System,
    }
}

#[cfg(test)]
mod tests {
    use voya_core::{DnsConfig, SysProxyType, TrafficMode};

    use super::*;

    /// Every scalar and string in `AppConfig` gets a distinct value, so a
    /// same-typed neighbour swap in either mapper (`upload_mbps`/`download_mbps`,
    /// `direct`/`remote`/`bootstrap` DNS, `username`/`password`, …) fails here instead of
    /// silently shipping. Struct literals guarantee that every field is
    /// assigned; nothing but distinct values proves it is assigned correctly.
    fn distinctly_valued_config() -> AppConfig {
        AppConfig {
            active_profile_id: "active-profile-id".to_string(),
            active_group_id: String::new(),
            active_routing_id: "active-routing-id".to_string(),
            core: CoreConfig {
                log_enabled: true,
                log_level: "debug".to_string(),
                mux_enabled: true,
                default_allow_insecure: true,
                default_fingerprint: "fingerprint-value".to_string(),
                default_user_agent: "user-agent-value".to_string(),
                send_through: Some("send-through-value".to_string()),
                bind_interface: Some("bind-interface-value".to_string()),
                tls_fragment: voya_core::TlsFragmentMode::TlsHello,
                fragment_fallback_delay_ms: 51,
                cache_file_enabled: false,
            },
            tun: TunConfig {
                enabled: true,
                auto_route: false,
                strict_route: true,
                stack: "gvisor".to_string(),
                mtu: 1301,
                ipv6_enabled: true,
                icmp_routing: "icmp-routing-value".to_string(),
            },
            behavior: BehaviorConfig {
                autostart: true,
                auto_check_ip: false,
                close_action: voya_core::CloseAction::Ask,
                start_minimized: true,
                auto_create_subscription_group: false,
            },
            appearance: AppearanceConfig {
                theme: Some("Dark".to_string()),
                language: "zh-Hans".to_string(),
            },
            speed_test: SpeedtestConfig {
                timeout_seconds: 21,
                latency_url: "https://speed.test/latency".to_string(),
                ip_lookup_url: "https://speed.test/ip".to_string(),
                page_size: Some(23),
                delay_interval_seconds: Some(24),
            },
            multiplexing: MultiplexingConfig {
                protocol: "h2mux".to_string(),
                max_connections: 31,
                padding: Some(true),
            },
            hysteria: HysteriaConfig {
                upload_mbps: 41,
                download_mbps: 42,
                hop_interval_seconds: 43,
            },
            proxy: ProxyConfig {
                traffic_mode: TrafficMode::Global,
            },
            system_proxy: SystemProxyConfig {
                mode: SysProxyType::Unchanged,
                exceptions: "exceptions-value".to_string(),
                bypass_local: false,
            },
            inbounds: vec![InboundConfig {
                local_port: 61,
                sniffing_enabled: false,
                lan_connections_allowed: true,
                separate_lan_port: false,
                username: "inbound-user".to_string(),
                password: "inbound-pass".to_string(),
                secondary_port_enabled: true,
            }],
            dns: DnsConfig {
                add_common_hosts: Some(false),
                fake_ip: Some(true),
                global_fake_ip: Some(false),
                block_binding_query: Some(true),
                direct: Some("direct-dns-value".to_string()),
                remote: Some("remote-dns-value".to_string()),
                bootstrap: Some("bootstrap-dns-value".to_string()),
                direct_strategy: Some(voya_core::DnsStrategy::PreferIpv4),
                proxy_strategy: Some(voya_core::DnsStrategy::Ipv6Only),
                hosts: Some("hosts-value".to_string()),
                direct_expected_ips: Some("direct-expected-ips-value".to_string()),
            },
        }
    }

    #[test]
    fn settings_mapping_round_trips_every_distinct_field() {
        let config = distinctly_valued_config();
        let state = AppStateRecord {
            active_profile_id: Some(config.active_profile_id.clone()),
            active_routing_id: Some(config.active_routing_id.clone()),
            active_group_id: None,
        };

        let settings = settings_from_app_config(&config);
        let restored = app_config_from_settings(&settings, &state);

        assert_eq!(restored, config);
    }

    /// The state record, not the settings contract, owns the active ids, so an
    /// empty selection must survive the trip as an empty string rather than
    /// being resurrected from the previous configuration.
    #[test]
    fn settings_mapping_carries_the_active_ids_from_the_state_record() {
        let config = distinctly_valued_config();
        let settings = settings_from_app_config(&config);

        let restored = app_config_from_settings(&settings, &AppStateRecord::default());

        assert!(restored.active_profile_id.is_empty());
        assert!(restored.active_routing_id.is_empty());
        assert_eq!(
            restored.hysteria.upload_mbps, config.hysteria.upload_mbps,
            "everything outside the state record must still round trip"
        );
    }

    /// `config_from_settings` is what the save transaction uses: it must take
    /// the active ids from the configuration being replaced, because they are
    /// not part of the settings contract at all.
    #[test]
    fn config_from_settings_preserves_the_active_selection() {
        let config = distinctly_valued_config();
        let settings = settings_from_app_config(&AppConfig::default());

        let restored = config_from_settings(&settings, &config);

        assert_eq!(restored.active_profile_id, config.active_profile_id);
        assert_eq!(restored.active_routing_id, config.active_routing_id);
        assert_eq!(restored.appearance, AppConfig::default().appearance);
    }

    #[test]
    fn contract_defaults_match_domain_defaults() {
        let mut mapped = app_config_from_settings(
            &contracts::AppSettings::default(),
            &AppStateRecord::default(),
        );
        mapped.dns = crate::dns::normalize_dns(mapped.dns);

        assert_eq!(
            mapped,
            AppConfig::default(),
            "a fresh install is configured from AppSettings::default(), so it must \
             produce exactly AppConfig::default()"
        );
    }
}
