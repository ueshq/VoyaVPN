use thiserror::Error;
use voya_contracts as contracts;
use voya_core::{
    AppConfig, AppearanceConfig, BehaviorConfig, CoreConfig, HysteriaConfig, InboundConfig,
    MultiplexingConfig, ProxyConfig, SpeedtestConfig, SystemProxyConfig, TunConfig,
};
use voya_db::AppStateRecord;

use crate::{
    contract_map::{
        close_action_from_contract, close_action_to_contract, dns_from_contract, dns_to_contract,
        sysproxy_type_from_contract, sysproxy_type_to_contract, tls_fragment_mode_from_contract,
        tls_fragment_mode_to_contract, traffic_mode_from_contract, traffic_mode_to_contract,
    },
    input_safety,
};

/// Why a submitted settings bundle was rejected.
///
/// Every variant names the `AppSettings` path it is about, so the settings
/// surface can mark the offending input the way the DNS pane already does
/// instead of showing one banner for the whole form. `label` stays alongside
/// `field` because the two audiences differ: the message keeps reading
/// "invalid UI language", the form keys off `appearance.language`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppSettingsValidationError {
    #[error("invalid {label}: {}", input_safety_text(reason))]
    InvalidText {
        field: &'static str,
        label: &'static str,
        reason: contracts::ValidationCode,
    },
    #[error("TUN MTU must be between 576 and 65535")]
    InvalidTunMtu,
    #[error("Hysteria bandwidth values cannot be negative")]
    NegativeHysteriaBandwidth { field: &'static str },
    #[error("Hysteria hop interval must be at least 5 seconds")]
    InvalidHysteriaHopInterval,
    #[error("TLS fragment fallback delay must be between 1 and 10000 ms")]
    InvalidFragmentFallbackDelay,
    #[error("the settings must keep one local proxy inbound")]
    InboundRequired,
    #[error("local proxy port must be between 1024 and 65514")]
    InvalidInboundPort,
    #[error("LAN authentication needs both a username and a password")]
    IncompleteInboundCredentials,
}

impl AppSettingsValidationError {
    /// The `AppSettings` path the rejection is about.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        match self {
            Self::InvalidFragmentFallbackDelay => "core.fragmentFallbackDelayMs",
            Self::InboundRequired => "network.inbounds",
            Self::InvalidInboundPort => "network.inbounds.0.localPort",
            Self::IncompleteInboundCredentials => "network.inbounds.0.password",
            Self::InvalidText { field, .. } | Self::NegativeHysteriaBandwidth { field } => field,
            Self::InvalidTunMtu => "network.tun.mtu",
            Self::InvalidHysteriaHopInterval => "hysteria.hopIntervalSeconds",
        }
    }

    /// The rejection as a code the settings dialog can translate.
    ///
    /// The `Display` text stays as the diagnostic that reaches the logs and
    /// `AppError::message`; this is what the user reads. Both halves are needed:
    /// `field` says which input to mark, `code` says what to write next to it.
    #[must_use]
    pub fn code(&self) -> contracts::ValidationCode {
        match self {
            Self::InvalidText { reason, .. } => reason.clone(),
            Self::InvalidTunMtu => contracts::ValidationCode::TunMtuOutOfRange {
                min: TUN_MTU_RANGE.0,
                max: TUN_MTU_RANGE.1,
            },
            Self::NegativeHysteriaBandwidth { .. } => {
                contracts::ValidationCode::NegativeHysteriaBandwidth
            }
            Self::InvalidFragmentFallbackDelay => {
                contracts::ValidationCode::FragmentFallbackDelayOutOfRange {
                    min: FRAGMENT_FALLBACK_DELAY_RANGE_MS.0,
                    max: FRAGMENT_FALLBACK_DELAY_RANGE_MS.1,
                }
            }
            Self::InboundRequired => contracts::ValidationCode::InboundRequired,
            Self::InvalidInboundPort => contracts::ValidationCode::InboundPortOutOfRange {
                min: INBOUND_PORT_RANGE.0,
                max: INBOUND_PORT_RANGE.1,
            },
            Self::IncompleteInboundCredentials => {
                contracts::ValidationCode::InboundCredentialsIncomplete
            }
            Self::InvalidHysteriaHopInterval => {
                contracts::ValidationCode::HysteriaHopIntervalTooShort {
                    minimum_seconds: MIN_HYSTERIA_HOP_INTERVAL_SECONDS,
                }
            }
        }
    }
}

/// Accepted TUN MTU, spelled once so the check and the message it produces
/// cannot drift.
const TUN_MTU_RANGE: (u32, u32) = (576, 65_535);
const MIN_HYSTERIA_HOP_INTERVAL_SECONDS: u32 = 5;
const FRAGMENT_FALLBACK_DELAY_RANGE_MS: (u32, u32) = (1, 10_000);
/// Accepted mixed port. Every other local port is derived from it, the highest
/// being the speedtest probes at `+21`, so the ceiling keeps all of them valid.
const INBOUND_PORT_RANGE: (u32, u32) = (1024, 65_514);
const INBOUND_CREDENTIAL_MAX_CHARS: usize = 256;

pub fn validate_app_settings(
    settings: &contracts::AppSettings,
) -> Result<(), AppSettingsValidationError> {
    input_safety::validate_required_text(settings.appearance.language.trim(), 256).map_err(
        |error| AppSettingsValidationError::InvalidText {
            field: "appearance.language",
            label: "UI language",
            reason: input_safety_reason(error),
        },
    )?;
    if !(i32::try_from(TUN_MTU_RANGE.0).unwrap_or(i32::MAX)
        ..=i32::try_from(TUN_MTU_RANGE.1).unwrap_or(i32::MAX))
        .contains(&settings.network.tun.mtu)
    {
        return Err(AppSettingsValidationError::InvalidTunMtu);
    }
    if settings.hysteria.upload_mbps < 0 {
        return Err(AppSettingsValidationError::NegativeHysteriaBandwidth {
            field: "hysteria.uploadMbps",
        });
    }
    if settings.hysteria.download_mbps < 0 {
        return Err(AppSettingsValidationError::NegativeHysteriaBandwidth {
            field: "hysteria.downloadMbps",
        });
    }
    if settings.hysteria.hop_interval_seconds
        < i32::try_from(MIN_HYSTERIA_HOP_INTERVAL_SECONDS).unwrap_or(i32::MAX)
    {
        return Err(AppSettingsValidationError::InvalidHysteriaHopInterval);
    }
    if !(i32::try_from(FRAGMENT_FALLBACK_DELAY_RANGE_MS.0).unwrap_or(i32::MAX)
        ..=i32::try_from(FRAGMENT_FALLBACK_DELAY_RANGE_MS.1).unwrap_or(i32::MAX))
        .contains(&settings.core.fragment_fallback_delay_ms)
    {
        return Err(AppSettingsValidationError::InvalidFragmentFallbackDelay);
    }
    validate_inbound(settings)?;
    Ok(())
}

fn validate_inbound(settings: &contracts::AppSettings) -> Result<(), AppSettingsValidationError> {
    let Some(inbound) = settings.network.inbounds.first() else {
        return Err(AppSettingsValidationError::InboundRequired);
    };
    if !(i32::try_from(INBOUND_PORT_RANGE.0).unwrap_or(i32::MAX)
        ..=i32::try_from(INBOUND_PORT_RANGE.1).unwrap_or(i32::MAX))
        .contains(&inbound.local_port)
    {
        return Err(AppSettingsValidationError::InvalidInboundPort);
    }
    for (field, label, value) in [
        (
            "network.inbounds.0.username",
            "LAN username",
            &inbound.username,
        ),
        (
            "network.inbounds.0.password",
            "LAN password",
            &inbound.password,
        ),
    ] {
        input_safety::validate_text(value, INBOUND_CREDENTIAL_MAX_CHARS).map_err(|error| {
            AppSettingsValidationError::InvalidText {
                field,
                label,
                reason: input_safety_reason(error),
            }
        })?;
    }
    // Credentials only guard the separate LAN inbound, and sing-box needs both
    // halves: a lone username would silently leave that port open.
    if inbound.lan_connections_allowed
        && inbound.separate_lan_port
        && inbound.username.trim().is_empty() != inbound.password.trim().is_empty()
    {
        return Err(AppSettingsValidationError::IncompleteInboundCredentials);
    }
    Ok(())
}

/// Whether the saved configuration changed something the running core reads
/// from its generated config. Only generation inputs belong here: fields the
/// UI alone consumes (node sorting, appearance) must not interrupt traffic.
#[must_use]
pub fn saved_config_requires_runtime_restart(original: &AppConfig, updated: &AppConfig) -> bool {
    original.index_id != updated.index_id
        || original.core != updated.core
        || original.tun != updated.tun
        || original.active_routing_id != updated.active_routing_id
        || original.multiplexing != updated.multiplexing
        || original.hysteria != updated.hysteria
        // Traffic mode changes generated routing.
        || original.proxy.traffic_mode != updated.proxy.traffic_mode
        || original.inbounds != updated.inbounds
        || original.dns != updated.dns
}

/// Why a text field was rejected, as the code the settings dialog translates.
const fn input_safety_reason(error: input_safety::InputSafetyError) -> contracts::ValidationCode {
    match error {
        input_safety::InputSafetyError::EmptyValue => contracts::ValidationCode::TextRequired,
        input_safety::InputSafetyError::TooLong => contracts::ValidationCode::TextTooLong,
        input_safety::InputSafetyError::ControlCharacters => {
            contracts::ValidationCode::TextControlCharacters
        }
        input_safety::InputSafetyError::TooManyItems => contracts::ValidationCode::TooManyItems,
    }
}

/// The English half of a rejection, for the log line and `AppError::message`.
fn input_safety_text(code: &contracts::ValidationCode) -> &'static str {
    match code {
        contracts::ValidationCode::TextRequired => "value is required",
        contracts::ValidationCode::TextTooLong => "value is too long",
        contracts::ValidationCode::TextControlCharacters => "control characters are not allowed",
        contracts::ValidationCode::TooManyItems => "too many items",
        _ => "value is not valid",
    }
}

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
        active_profile_id: (!config.index_id.is_empty()).then(|| config.index_id.clone()),
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
        index_id: state.active_profile_id.clone().unwrap_or_default(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRuntimeAction {
    None,
    ReapplySystemProxy,
    Restart,
}

#[must_use]
pub const fn settings_runtime_action(
    runtime_restart_required: bool,
    system_proxy_reapply_required: bool,
) -> SettingsRuntimeAction {
    if runtime_restart_required {
        SettingsRuntimeAction::Restart
    } else if system_proxy_reapply_required {
        SettingsRuntimeAction::ReapplySystemProxy
    } else {
        SettingsRuntimeAction::None
    }
}

/// The one OS side effect a settings save has: the login entry. A trait so the
/// save transaction's rollback paths can be tested without touching the machine.
pub trait ApplyAutostart: Sync {
    fn apply_autostart(&self, config: &AppConfig) -> Result<(), contracts::AppError>;
}

impl ApplyAutostart for crate::autostart::AutostartManager {
    fn apply_autostart(&self, config: &AppConfig) -> Result<(), contracts::AppError> {
        let mut config = config.clone();
        let enabled = config.behavior.autostart;
        self.set_enabled(&mut config, enabled)
            .map(|_| ())
            .map_err(contracts::AppError::from)
    }
}

/// A host without a login entry. A phone's always-on VPN is a system setting
/// the user turns on, not something an app arranges for itself.
pub struct NoAutostart;

impl ApplyAutostart for NoAutostart {
    fn apply_autostart(&self, _config: &AppConfig) -> Result<(), contracts::AppError> {
        Ok(())
    }
}

/// Whether saving `target` over `original` has to rewrite the login entry.
///
/// The entry carries the launch flag `start_minimized` is read against, so an
/// enabled entry is also rewritten when that option changes.
#[must_use]
pub fn autostart_changes(original: &AppConfig, target: &AppConfig) -> bool {
    original.behavior.autostart != target.behavior.autostart
        || (target.behavior.autostart
            && original.behavior.start_minimized != target.behavior.start_minimized)
}

#[cfg(test)]
mod tests {
    use voya_core::DnsConfig;

    use voya_core::{SysProxyType, TrafficMode};

    use super::*;

    /// Every scalar and string in `AppConfig` gets a distinct value, so a
    /// same-typed neighbour swap in either mapper (`upload_mbps`/`download_mbps`,
    /// `direct`/`remote`/`bootstrap` DNS, `username`/`password`, …) fails here instead of
    /// silently shipping. Struct literals guarantee that every field is
    /// assigned; nothing but distinct values proves it is assigned correctly.
    fn distinctly_valued_config() -> AppConfig {
        AppConfig {
            index_id: "active-profile-id".to_string(),
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
            active_profile_id: Some(config.index_id.clone()),
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

        assert!(restored.index_id.is_empty());
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

        assert_eq!(restored.index_id, config.index_id);
        assert_eq!(restored.active_routing_id, config.active_routing_id);
        assert_eq!(restored.appearance, AppConfig::default().appearance);
    }

    fn config(autostart: bool) -> AppConfig {
        let mut config = AppConfig::default();
        config.behavior.autostart = autostart;
        config
    }

    #[test]
    fn toggling_start_minimized_rewrites_only_an_enabled_login_entry() {
        let original = config(true);
        let mut target = original.clone();
        target.behavior.start_minimized = true;
        assert!(autostart_changes(&original, &target));

        let original = config(false);
        let mut target = original.clone();
        target.behavior.start_minimized = true;
        assert!(!autostart_changes(&original, &target));
    }

    #[test]
    fn restart_dominates_proxy_reapply_as_the_single_runtime_action() {
        assert_eq!(
            settings_runtime_action(true, true),
            SettingsRuntimeAction::Restart
        );
        assert_eq!(
            settings_runtime_action(false, true),
            SettingsRuntimeAction::ReapplySystemProxy
        );
        assert_eq!(
            settings_runtime_action(false, false),
            SettingsRuntimeAction::None
        );
    }

    #[test]
    fn settings_validation_rejects_runtime_limits() {
        let mut settings = contracts::AppSettings::default();
        settings.network.tun.mtu = 575;
        assert_eq!(
            validate_app_settings(&settings),
            Err(AppSettingsValidationError::InvalidTunMtu)
        );

        settings.network.tun.mtu = 1500;
        for (port, accepted) in [(1023, false), (1024, true), (65_514, true), (65_515, false)] {
            settings.network.inbounds[0].local_port = port;
            assert_eq!(validate_app_settings(&settings).is_ok(), accepted, "{port}");
        }
    }

    #[test]
    fn runtime_restart_policy_ignores_appearance_only_changes() {
        let original = AppConfig::default();
        let mut appearance = original.clone();
        appearance.appearance.language = "zh-Hans".to_string();
        assert!(!saved_config_requires_runtime_restart(
            &original,
            &appearance
        ));

        let mut network = original.clone();
        network.inbounds[0].local_port += 1;
        assert!(saved_config_requires_runtime_restart(&original, &network));
    }

    #[test]
    fn runtime_restart_policy_tracks_traffic_mode() {
        let original = AppConfig::default();

        let mut mode = original.clone();
        mode.proxy.traffic_mode = TrafficMode::Global;
        assert!(saved_config_requires_runtime_restart(&original, &mode));
    }

    #[test]
    fn saving_unchanged_settings_never_requires_a_runtime_restart() {
        let mut original = AppConfig {
            index_id: "profile-a".to_string(),
            active_group_id: String::new(),
            ..AppConfig::default()
        };
        original.active_routing_id = "routing-a".to_string();
        original.appearance.language = "zh-Hans".to_string();

        let target = config_from_settings(&settings_from_app_config(&original), &original);

        assert_eq!(target, original);
        assert!(!saved_config_requires_runtime_restart(&original, &target));
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

    /// The settings surface marks the input a rejection is about, so every
    /// rejection has to name an `AppSettings` path — not the human label the
    /// message uses, which no form can key off.
    #[test]
    fn every_settings_rejection_names_the_contract_path_it_is_about() {
        let cases: [(contracts::AppSettings, &str); 9] = [
            (
                settings_with(|settings| settings.appearance.language = "  ".to_string()),
                "appearance.language",
            ),
            (
                settings_with(|settings| settings.network.tun.mtu = 1),
                "network.tun.mtu",
            ),
            (
                settings_with(|settings| settings.hysteria.download_mbps = -1),
                "hysteria.downloadMbps",
            ),
            (
                settings_with(|settings| settings.hysteria.hop_interval_seconds = 1),
                "hysteria.hopIntervalSeconds",
            ),
            (
                settings_with(|settings| settings.core.fragment_fallback_delay_ms = 0),
                "core.fragmentFallbackDelayMs",
            ),
            (
                settings_with(|settings| settings.network.inbounds.clear()),
                "network.inbounds",
            ),
            (
                settings_with(|settings| settings.network.inbounds[0].local_port = 80),
                "network.inbounds.0.localPort",
            ),
            (
                settings_with(|settings| {
                    let inbound = &mut settings.network.inbounds[0];
                    inbound.lan_connections_allowed = true;
                    inbound.separate_lan_port = true;
                    inbound.username = "guest".to_string();
                }),
                "network.inbounds.0.password",
            ),
            (
                settings_with(|settings| {
                    settings.network.inbounds[0].username = "guest\u{7}".to_string();
                }),
                "network.inbounds.0.username",
            ),
        ];

        for (settings, field) in cases {
            let error =
                validate_app_settings(&settings).expect_err("the case should be rejected: {field}");
            assert_eq!(error.field(), field, "{error}");
        }
    }

    fn settings_with(patch: impl FnOnce(&mut contracts::AppSettings)) -> contracts::AppSettings {
        let mut settings = contracts::AppSettings::default();
        patch(&mut settings);
        settings
    }
}
