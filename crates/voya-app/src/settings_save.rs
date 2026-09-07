use thiserror::Error;
use voya_contracts as contracts;
use voya_core::{
    AppConfig, CoreBasicItem, GrpcItem, GuiItem, HysteriaItem, InItem, KeyEventItem, Mux4SboxItem,
    ProxyUiItem, RoutingBasicItem, SimpleDnsItem, SpeedTestItem, SysProxyType, SystemProxyItem,
    TrafficMode, TunModeItem, UiItem,
};
use voya_db::AppStateRecord;

use crate::{input_safety, updates};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SettingsContractError {
    #[error("invalid {field} value `{value}` in AppSettingsV1")]
    InvalidValue { field: &'static str, value: String },
}

/// Why a submitted settings bundle was rejected.
///
/// Every variant names the `AppSettingsV1` path it is about, so the settings
/// surface can mark the offending input the way the DNS pane already does
/// instead of showing one banner for the whole form. `label` stays alongside
/// `field` because the two audiences differ: the message keeps reading
/// "invalid Geo source URL", the form keys off `sources.geo`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppSettingsValidationError {
    #[error("unsupported settings schema version {found}; expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
    #[error("invalid {label}: {reason}")]
    InvalidText {
        field: &'static str,
        label: &'static str,
        reason: &'static str,
    },
    #[error(transparent)]
    InvalidSource(#[from] updates::InvalidSourceUrl),
    #[error("TUN MTU must be between 576 and 65535")]
    InvalidTunMtu,
    #[error("Hysteria bandwidth values cannot be negative")]
    NegativeHysteriaBandwidth { field: &'static str },
    #[error("Hysteria hop interval must be at least 5 seconds")]
    InvalidHysteriaHopInterval,
}

impl AppSettingsValidationError {
    /// The `AppSettingsV1` path the rejection is about.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        match self {
            Self::UnsupportedSchema { .. } => "schemaVersion",
            Self::InvalidText { field, .. } | Self::NegativeHysteriaBandwidth { field } => field,
            Self::InvalidSource(error) => error.field.field,
            Self::InvalidTunMtu => "network.tun.mtu",
            Self::InvalidHysteriaHopInterval => "hysteria.hopIntervalSeconds",
        }
    }
}

pub fn validate_app_settings(
    settings: &contracts::AppSettingsV1,
) -> Result<(), AppSettingsValidationError> {
    if settings.schema_version != contracts::CURRENT_SCHEMA_VERSION {
        return Err(AppSettingsValidationError::UnsupportedSchema {
            found: settings.schema_version,
            expected: contracts::CURRENT_SCHEMA_VERSION,
        });
    }
    input_safety::validate_required_text(settings.appearance.language.trim(), 256).map_err(
        |error| AppSettingsValidationError::InvalidText {
            field: "appearance.language",
            label: "UI language",
            reason: input_safety_reason(error),
        },
    )?;
    for (source, value) in [
        (
            updates::SourceUrlField::GEO,
            settings.sources.geo.as_deref(),
        ),
        (
            updates::SourceUrlField::SINGBOX_RULESET,
            settings.sources.singbox_ruleset.as_deref(),
        ),
        (
            updates::SourceUrlField::ROUTING_TEMPLATE,
            settings.sources.routing_template.as_deref(),
        ),
        (
            updates::SourceUrlField::SUBSCRIPTION_CONVERTER,
            settings.sources.subscription_converter.as_deref(),
        ),
    ] {
        input_safety::validate_optional_text(value, 2048).map_err(|error| {
            AppSettingsValidationError::InvalidText {
                field: source.field,
                label: source.label,
                reason: input_safety_reason(error),
            }
        })?;
    }
    // The subscription converter is routinely a local helper (for example
    // http://localhost:25500/sub), so plain HTTP stays allowed there. Geo,
    // ruleset and routing-template sources decide which traffic bypasses the
    // proxy, so they must be authenticated transports.
    updates::validate_optional_source_url(
        updates::SourceUrlField::SUBSCRIPTION_CONVERTER,
        settings.sources.subscription_converter.as_deref(),
    )?;
    updates::validate_asset_source_urls(&updates::ConfigSourceSettings {
        geo_source_url: settings.sources.geo.clone(),
        srs_source_url: settings.sources.singbox_ruleset.clone(),
        route_rules_template_source_url: settings.sources.routing_template.clone(),
    })?;
    // Validated on the save path only: the mapping in `app_config_from_settings`
    // also runs when a stored configuration is loaded, so rejecting there would
    // block startup on a value that is already persisted. It has to be rejected
    // somewhere, because `parse_udp_test_target` fills in the named kind's
    // default host — a half-typed `dns:` would silently probe a server the user
    // never named.
    voya_udptest::validate_udp_test_target(&settings.speed_test.udp_target).map_err(|_| {
        AppSettingsValidationError::InvalidText {
            field: "speedTest.udpTarget",
            label: "UDP test target",
            reason: "value must be host:port, optionally prefixed with a test kind",
        }
    })?;
    if !(576..=65_535).contains(&settings.network.tun.mtu) {
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
    if settings.hysteria.hop_interval_seconds < 5 {
        return Err(AppSettingsValidationError::InvalidHysteriaHopInterval);
    }
    Ok(())
}

/// Whether the saved configuration changed something the running core reads
/// from its generated config. Only generation inputs belong here: fields the
/// UI alone consumes (node sorting, appearance) must not interrupt traffic.
#[must_use]
pub fn saved_config_requires_runtime_restart(original: &AppConfig, updated: &AppConfig) -> bool {
    original.index_id != updated.index_id
        || original.core_basic_item != updated.core_basic_item
        || original.tun_mode_item != updated.tun_mode_item
        || original.grpc_item != updated.grpc_item
        || original.routing_basic_item != updated.routing_basic_item
        || original.mux4_sbox_item != updated.mux4_sbox_item
        || original.hysteria_item != updated.hysteria_item
        // `traffic_mode` selects the generated outbound; `node_sorting` only
        // orders the proxy-groups snapshot the UI renders.
        || original.proxy_ui_item.traffic_mode != updated.proxy_ui_item.traffic_mode
        // The SRS source is embedded in every remote `route.rule_set[].url`.
        || original.const_item.srs_source_url != updated.const_item.srs_source_url
        || original.inbound != updated.inbound
        || original.simple_dns_item != updated.simple_dns_item
}

const fn input_safety_reason(error: input_safety::InputSafetyError) -> &'static str {
    match error {
        input_safety::InputSafetyError::EmptyValue => "value is required",
        input_safety::InputSafetyError::TooLong => "value is too long",
        input_safety::InputSafetyError::ControlCharacters => "control characters are not allowed",
        input_safety::InputSafetyError::TooManyItems => "too many items",
    }
}

#[must_use]
pub fn settings_from_app_config(config: &AppConfig) -> contracts::AppSettingsV1 {
    let show_window_shortcut = config.show_window_shortcut.as_ref().and_then(|item| {
        item.key_code.map(|key_code| contracts::ShortcutChord {
            alt: item.alt,
            control: item.control,
            shift: item.shift,
            key_code,
        })
    });

    contracts::AppSettingsV1 {
        schema_version: contracts::CURRENT_SCHEMA_VERSION,
        appearance: contracts::AppearanceSettings {
            language: config.ui_item.current_language.clone(),
            theme: theme_from_config(config.ui_item.current_theme.as_deref()),
        },
        behavior: contracts::BehaviorSettings {
            autostart: config.gui_item.auto_run,
            statistics: config.gui_item.enable_statistics,
            realtime_speed: config.gui_item.display_real_time_speed,
            auto_create_subscription_group: Some(config.gui_item.auto_create_subscription_group),
        },
        core: contracts::CoreSettings {
            log_enabled: config.core_basic_item.log_enabled,
            log_level: config.core_basic_item.loglevel.clone(),
            mux_enabled: config.core_basic_item.mux_enabled,
            default_allow_insecure: config.core_basic_item.def_allow_insecure,
            default_fingerprint: config.core_basic_item.def_fingerprint.clone(),
            default_user_agent: config.core_basic_item.def_user_agent.clone(),
            send_through: config.core_basic_item.send_through.clone(),
            bind_interface: config.core_basic_item.bind_interface.clone(),
            fragment_enabled: config.core_basic_item.enable_fragment,
            cache_file_enabled: config.core_basic_item.enable_cache_file4_sbox,
        },
        network: contracts::NetworkSettings {
            tun: contracts::TunSettings {
                enabled: config.tun_mode_item.enable_tun,
                auto_route: config.tun_mode_item.auto_route,
                strict_route: config.tun_mode_item.strict_route,
                stack: config.tun_mode_item.stack.clone(),
                mtu: config.tun_mode_item.mtu,
                ipv6_enabled: config.tun_mode_item.enable_ipv6_address,
                icmp_routing: config.tun_mode_item.icmp_routing.clone(),
            },
            system_proxy: contracts::SystemProxySettings {
                mode: system_proxy_mode_to_str(config.system_proxy_item.sys_proxy_type).to_string(),
                exceptions: config.system_proxy_item.system_proxy_exceptions.clone(),
                bypass_local: config.system_proxy_item.not_proxy_local_address,
                advanced_protocol: config
                    .system_proxy_item
                    .system_proxy_advanced_protocol
                    .clone(),
                custom_pac_path: config
                    .system_proxy_item
                    .custom_system_proxy_pac_path
                    .clone(),
                custom_script_path: config
                    .system_proxy_item
                    .custom_system_proxy_script_path
                    .clone(),
            },
            inbounds: config
                .inbound
                .iter()
                .map(|item| contracts::InboundSettings {
                    local_port: item.local_port,
                    protocol: item.protocol.clone(),
                    sniffing_enabled: item.sniffing_enabled,
                    lan_connections_allowed: item.allow_lan_conn,
                    separate_lan_port: item.new_port4_lan,
                    username: item.user.clone(),
                    password: item.pass.clone(),
                    secondary_port_enabled: item.second_local_port_enabled,
                })
                .collect(),
        },
        routing: contracts::RoutingSettings {
            domain_strategy: config.routing_basic_item.domain_strategy.clone(),
            singbox_domain_strategy: config.routing_basic_item.domain_strategy4_singbox.clone(),
        },
        dns: contracts::AppDnsSettings {
            use_system_hosts: config.simple_dns_item.use_system_hosts,
            add_common_hosts: config.simple_dns_item.add_common_hosts,
            fake_ip: config.simple_dns_item.fake_ip,
            global_fake_ip: config.simple_dns_item.global_fake_ip,
            block_binding_query: config.simple_dns_item.block_binding_query,
            direct: config.simple_dns_item.direct_dns.clone(),
            remote: config.simple_dns_item.remote_dns.clone(),
            bootstrap: config.simple_dns_item.bootstrap_dns.clone(),
            direct_strategy: config.simple_dns_item.strategy4_freedom.clone(),
            proxy_strategy: config.simple_dns_item.strategy4_proxy.clone(),
            serve_stale: config.simple_dns_item.serve_stale,
            parallel_query: config.simple_dns_item.parallel_query,
            hosts: config.simple_dns_item.hosts.clone(),
            direct_expected_ips: config.simple_dns_item.direct_expected_ips.clone(),
        },
        sources: contracts::SourceSettings {
            subscription_converter: config.const_item.sub_convert_url.clone(),
            geo: config.const_item.geo_source_url.clone(),
            singbox_ruleset: config.const_item.srs_source_url.clone(),
            routing_template: config.const_item.route_rules_template_source_url.clone(),
        },
        speed_test: contracts::SpeedTestSettings {
            timeout_seconds: config.speed_test_item.speed_test_timeout,
            download_url: config.speed_test_item.speed_test_url.clone(),
            latency_url: config.speed_test_item.speed_ping_test_url.clone(),
            mixed_concurrency: config.speed_test_item.mixed_concurrency_count,
            ip_lookup_url: config.speed_test_item.ipapi_url.clone(),
            udp_target: config.speed_test_item.udp_test_target.clone(),
            page_size: config.speed_test_item.speed_test_page_size,
            delay_interval_ms: config.speed_test_item.speed_test_delay_interval,
        },
        multiplexing: contracts::MultiplexingSettings {
            protocol: config.mux4_sbox_item.protocol.clone(),
            max_connections: config.mux4_sbox_item.max_connections,
            padding: config.mux4_sbox_item.padding,
        },
        grpc: contracts::GrpcSettings {
            idle_timeout_seconds: config.grpc_item.idle_timeout,
            health_check_timeout_seconds: config.grpc_item.health_check_timeout,
            permit_without_stream: config.grpc_item.permit_without_stream,
        },
        hysteria: contracts::HysteriaSettings {
            upload_mbps: config.hysteria_item.up_mbps,
            download_mbps: config.hysteria_item.down_mbps,
            hop_interval_seconds: config.hysteria_item.hop_interval,
        },
        proxy: contracts::ProxySettings {
            traffic_mode: traffic_mode_to_str(config.proxy_ui_item.traffic_mode).to_string(),
            node_sorting: config.proxy_ui_item.node_sorting,
        },
        shortcuts: contracts::ShortcutSettings {
            show_window_shortcut,
        },
    }
}

/// Map the settings contract onto a full `AppConfig`.
///
/// The active profile and routing ids are not part of the settings contract, so
/// they are carried over from the configuration being replaced.
pub fn config_from_settings(
    settings: &contracts::AppSettingsV1,
    current: &AppConfig,
) -> Result<AppConfig, SettingsContractError> {
    let state = AppStateRecord {
        active_profile_id: (!current.index_id.is_empty()).then(|| current.index_id.clone()),
        active_routing_id: (!current.routing_basic_item.routing_index_id.is_empty())
            .then(|| current.routing_basic_item.routing_index_id.clone()),
    };

    app_config_from_settings(settings, &state)
}

pub fn app_config_from_settings(
    settings: &contracts::AppSettingsV1,
    state: &AppStateRecord,
) -> Result<AppConfig, SettingsContractError> {
    let system_proxy_type = system_proxy_mode_from_str(&settings.network.system_proxy.mode)?;
    let traffic_mode = traffic_mode_from_str(&settings.proxy.traffic_mode)?;
    let show_window_shortcut = settings
        .shortcuts
        .show_window_shortcut
        .as_ref()
        .map(|shortcut| KeyEventItem {
            alt: shortcut.alt,
            control: shortcut.control,
            shift: shortcut.shift,
            key_code: Some(shortcut.key_code),
        });

    Ok(AppConfig {
        index_id: state.active_profile_id.clone().unwrap_or_default(),
        core_basic_item: CoreBasicItem {
            log_enabled: settings.core.log_enabled,
            loglevel: settings.core.log_level.clone(),
            mux_enabled: settings.core.mux_enabled,
            def_allow_insecure: settings.core.default_allow_insecure,
            def_fingerprint: settings.core.default_fingerprint.clone(),
            def_user_agent: settings.core.default_user_agent.clone(),
            send_through: settings.core.send_through.clone(),
            bind_interface: settings.core.bind_interface.clone(),
            enable_fragment: settings.core.fragment_enabled,
            enable_cache_file4_sbox: settings.core.cache_file_enabled,
        },
        tun_mode_item: TunModeItem {
            enable_tun: settings.network.tun.enabled,
            auto_route: settings.network.tun.auto_route,
            strict_route: settings.network.tun.strict_route,
            stack: settings.network.tun.stack.clone(),
            mtu: settings.network.tun.mtu,
            enable_ipv6_address: settings.network.tun.ipv6_enabled,
            icmp_routing: settings.network.tun.icmp_routing.clone(),
        },
        grpc_item: GrpcItem {
            idle_timeout: settings.grpc.idle_timeout_seconds,
            health_check_timeout: settings.grpc.health_check_timeout_seconds,
            permit_without_stream: settings.grpc.permit_without_stream,
        },
        routing_basic_item: RoutingBasicItem {
            domain_strategy: settings.routing.domain_strategy.clone(),
            domain_strategy4_singbox: settings.routing.singbox_domain_strategy.clone(),
            routing_index_id: state.active_routing_id.clone().unwrap_or_default(),
        },
        gui_item: GuiItem {
            auto_run: settings.behavior.autostart,
            enable_statistics: settings.behavior.statistics,
            display_real_time_speed: settings.behavior.realtime_speed,
            auto_create_subscription_group: settings
                .behavior
                .auto_create_subscription_group
                .unwrap_or(true),
        },
        ui_item: UiItem {
            current_theme: theme_to_config(settings.appearance.theme).map(str::to_string),
            current_language: settings.appearance.language.clone(),
        },
        const_item: voya_core::ConstItem {
            sub_convert_url: settings.sources.subscription_converter.clone(),
            geo_source_url: settings.sources.geo.clone(),
            srs_source_url: settings.sources.singbox_ruleset.clone(),
            route_rules_template_source_url: settings.sources.routing_template.clone(),
        },
        speed_test_item: SpeedTestItem {
            speed_test_timeout: settings.speed_test.timeout_seconds,
            speed_test_url: settings.speed_test.download_url.clone(),
            speed_ping_test_url: settings.speed_test.latency_url.clone(),
            mixed_concurrency_count: settings.speed_test.mixed_concurrency,
            ipapi_url: settings.speed_test.ip_lookup_url.clone(),
            udp_test_target: settings.speed_test.udp_target.clone(),
            speed_test_page_size: settings.speed_test.page_size,
            speed_test_delay_interval: settings.speed_test.delay_interval_ms,
        },
        mux4_sbox_item: Mux4SboxItem {
            protocol: settings.multiplexing.protocol.clone(),
            max_connections: settings.multiplexing.max_connections,
            padding: settings.multiplexing.padding,
        },
        hysteria_item: HysteriaItem {
            up_mbps: settings.hysteria.upload_mbps,
            down_mbps: settings.hysteria.download_mbps,
            hop_interval: settings.hysteria.hop_interval_seconds,
        },
        proxy_ui_item: ProxyUiItem {
            traffic_mode,
            node_sorting: settings.proxy.node_sorting,
        },
        system_proxy_item: SystemProxyItem {
            sys_proxy_type: system_proxy_type,
            system_proxy_exceptions: settings.network.system_proxy.exceptions.clone(),
            not_proxy_local_address: settings.network.system_proxy.bypass_local,
            system_proxy_advanced_protocol: settings.network.system_proxy.advanced_protocol.clone(),
            custom_system_proxy_pac_path: settings.network.system_proxy.custom_pac_path.clone(),
            custom_system_proxy_script_path: settings
                .network
                .system_proxy
                .custom_script_path
                .clone(),
        },
        inbound: settings
            .network
            .inbounds
            .iter()
            .map(|item| InItem {
                local_port: item.local_port,
                protocol: item.protocol.clone(),
                sniffing_enabled: item.sniffing_enabled,
                allow_lan_conn: item.lan_connections_allowed,
                new_port4_lan: item.separate_lan_port,
                user: item.username.clone(),
                pass: item.password.clone(),
                second_local_port_enabled: item.secondary_port_enabled,
            })
            .collect(),
        show_window_shortcut,
        simple_dns_item: SimpleDnsItem {
            use_system_hosts: settings.dns.use_system_hosts,
            add_common_hosts: settings.dns.add_common_hosts,
            fake_ip: settings.dns.fake_ip,
            global_fake_ip: settings.dns.global_fake_ip,
            block_binding_query: settings.dns.block_binding_query,
            direct_dns: settings.dns.direct.clone(),
            remote_dns: settings.dns.remote.clone(),
            bootstrap_dns: settings.dns.bootstrap.clone(),
            strategy4_freedom: settings.dns.direct_strategy.clone(),
            strategy4_proxy: settings.dns.proxy_strategy.clone(),
            serve_stale: settings.dns.serve_stale,
            parallel_query: settings.dns.parallel_query,
            hosts: settings.dns.hosts.clone(),
            direct_expected_ips: settings.dns.direct_expected_ips.clone(),
        },
    })
}

/// `None` is the stored shape of "follow the system theme", matching
/// `UiItem::default()` so contract defaults map onto domain defaults exactly.
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

const fn system_proxy_mode_to_str(value: SysProxyType) -> &'static str {
    match value {
        SysProxyType::ForcedClear => "forcedClear",
        SysProxyType::ForcedChange => "forcedChange",
        SysProxyType::Unchanged => "unchanged",
        SysProxyType::Pac => "pac",
    }
}

fn system_proxy_mode_from_str(value: &str) -> Result<SysProxyType, SettingsContractError> {
    match value {
        "forcedClear" => Ok(SysProxyType::ForcedClear),
        "forcedChange" => Ok(SysProxyType::ForcedChange),
        "unchanged" => Ok(SysProxyType::Unchanged),
        "pac" => Ok(SysProxyType::Pac),
        _ => Err(SettingsContractError::InvalidValue {
            field: "network.systemProxy.mode",
            value: value.to_string(),
        }),
    }
}

const fn traffic_mode_to_str(value: TrafficMode) -> &'static str {
    match value {
        TrafficMode::Rule => "rule",
        TrafficMode::Global => "global",
        TrafficMode::Direct => "direct",
        TrafficMode::Unchanged => "unchanged",
    }
}

fn traffic_mode_from_str(value: &str) -> Result<TrafficMode, SettingsContractError> {
    match value {
        "rule" => Ok(TrafficMode::Rule),
        "global" => Ok(TrafficMode::Global),
        "direct" => Ok(TrafficMode::Direct),
        "unchanged" => Ok(TrafficMode::Unchanged),
        _ => Err(SettingsContractError::InvalidValue {
            field: "proxy.trafficMode",
            value: value.to_string(),
        }),
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppliedSettingsSideEffects {
    autostart_touched: bool,
    hotkeys_touched: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSideEffectStage {
    Autostart,
    Hotkeys,
}

#[derive(Debug)]
pub struct SettingsSideEffectFailure<E> {
    pub stage: SettingsSideEffectStage,
    pub source: E,
    pub compensation_errors: Vec<E>,
}

pub trait SettingsSideEffectAdapter {
    type Error;

    fn apply_autostart(&self, config: &AppConfig) -> Result<(), Self::Error>;
    fn apply_hotkeys(&self, config: &AppConfig) -> Result<(), Self::Error>;
}

pub fn apply_settings_side_effects<A>(
    adapter: &A,
    original: &AppConfig,
    target: &AppConfig,
) -> Result<AppliedSettingsSideEffects, SettingsSideEffectFailure<A::Error>>
where
    A: SettingsSideEffectAdapter,
{
    let mut applied = AppliedSettingsSideEffects::default();
    if original.gui_item.auto_run != target.gui_item.auto_run {
        applied.autostart_touched = true;
        if let Err(source) = adapter.apply_autostart(target) {
            return Err(SettingsSideEffectFailure {
                stage: SettingsSideEffectStage::Autostart,
                source,
                compensation_errors: compensate_settings_side_effects(adapter, original, applied),
            });
        }
    }
    if original.show_window_shortcut != target.show_window_shortcut {
        applied.hotkeys_touched = true;
        if let Err(source) = adapter.apply_hotkeys(target) {
            return Err(SettingsSideEffectFailure {
                stage: SettingsSideEffectStage::Hotkeys,
                source,
                compensation_errors: compensate_settings_side_effects(adapter, original, applied),
            });
        }
    }
    Ok(applied)
}

pub fn compensate_settings_side_effects<A>(
    adapter: &A,
    original: &AppConfig,
    applied: AppliedSettingsSideEffects,
) -> Vec<A::Error>
where
    A: SettingsSideEffectAdapter,
{
    let mut errors = Vec::new();
    if applied.hotkeys_touched {
        if let Err(error) = adapter.apply_hotkeys(original) {
            errors.push(error);
        }
    }
    if applied.autostart_touched {
        if let Err(error) = adapter.apply_autostart(original) {
            errors.push(error);
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Every scalar and string in `AppConfig` gets a distinct value, so a
    /// same-typed neighbour swap in either mapper (`up_mbps`/`down_mbps`,
    /// `idle_timeout`/`health_check_timeout`, `direct`/`remote`/`bootstrap`
    /// DNS, `custom_pac_path`/`custom_script_path`, …) fails here instead of
    /// silently shipping. Struct literals guarantee that every field is
    /// assigned; nothing but distinct values proves it is assigned correctly.
    fn distinctly_valued_config() -> AppConfig {
        AppConfig {
            index_id: "active-profile-id".to_string(),
            core_basic_item: CoreBasicItem {
                log_enabled: true,
                loglevel: "debug".to_string(),
                mux_enabled: true,
                def_allow_insecure: true,
                def_fingerprint: "fingerprint-value".to_string(),
                def_user_agent: "user-agent-value".to_string(),
                send_through: Some("send-through-value".to_string()),
                bind_interface: Some("bind-interface-value".to_string()),
                enable_fragment: true,
                enable_cache_file4_sbox: false,
            },
            tun_mode_item: TunModeItem {
                enable_tun: true,
                auto_route: false,
                strict_route: true,
                stack: "gvisor".to_string(),
                mtu: 1301,
                enable_ipv6_address: true,
                icmp_routing: "icmp-routing-value".to_string(),
            },
            grpc_item: GrpcItem {
                idle_timeout: Some(11),
                health_check_timeout: Some(12),
                permit_without_stream: Some(true),
            },
            routing_basic_item: RoutingBasicItem {
                domain_strategy: "domain-strategy-value".to_string(),
                domain_strategy4_singbox: "singbox-domain-strategy-value".to_string(),
                routing_index_id: "active-routing-id".to_string(),
            },
            gui_item: GuiItem {
                auto_run: true,
                enable_statistics: false,
                display_real_time_speed: true,
                auto_create_subscription_group: false,
            },
            ui_item: UiItem {
                current_theme: Some("Dark".to_string()),
                current_language: "zh-Hans".to_string(),
            },
            const_item: voya_core::ConstItem {
                sub_convert_url: Some("https://convert.test/sub".to_string()),
                geo_source_url: Some("https://geo.test/{0}.dat".to_string()),
                srs_source_url: Some("https://srs.test/{0}.srs".to_string()),
                route_rules_template_source_url: Some("https://template.test/routing".to_string()),
            },
            speed_test_item: SpeedTestItem {
                speed_test_timeout: 21,
                speed_test_url: "https://speed.test/download".to_string(),
                speed_ping_test_url: "https://speed.test/latency".to_string(),
                mixed_concurrency_count: 22,
                ipapi_url: "https://speed.test/ip".to_string(),
                udp_test_target: "udp.test:5353".to_string(),
                speed_test_page_size: Some(23),
                speed_test_delay_interval: Some(24),
            },
            mux4_sbox_item: Mux4SboxItem {
                protocol: "h2mux".to_string(),
                max_connections: 31,
                padding: Some(true),
            },
            hysteria_item: HysteriaItem {
                up_mbps: 41,
                down_mbps: 42,
                hop_interval: 43,
            },
            proxy_ui_item: ProxyUiItem {
                traffic_mode: TrafficMode::Global,
                node_sorting: 51,
            },
            system_proxy_item: SystemProxyItem {
                sys_proxy_type: SysProxyType::Pac,
                system_proxy_exceptions: "exceptions-value".to_string(),
                not_proxy_local_address: false,
                system_proxy_advanced_protocol: "advanced-protocol-value".to_string(),
                custom_system_proxy_pac_path: Some("/tmp/custom-pac-path".to_string()),
                custom_system_proxy_script_path: Some("/tmp/custom-script-path".to_string()),
            },
            inbound: vec![InItem {
                local_port: 61,
                protocol: "inbound-protocol".to_string(),
                sniffing_enabled: false,
                allow_lan_conn: true,
                new_port4_lan: false,
                user: "inbound-user".to_string(),
                pass: "inbound-pass".to_string(),
                second_local_port_enabled: true,
            }],
            show_window_shortcut: Some(KeyEventItem {
                alt: true,
                control: false,
                shift: true,
                key_code: Some(71),
            }),
            simple_dns_item: SimpleDnsItem {
                use_system_hosts: Some(true),
                add_common_hosts: Some(false),
                fake_ip: Some(true),
                global_fake_ip: Some(false),
                block_binding_query: Some(true),
                direct_dns: Some("direct-dns-value".to_string()),
                remote_dns: Some("remote-dns-value".to_string()),
                bootstrap_dns: Some("bootstrap-dns-value".to_string()),
                strategy4_freedom: Some("direct-strategy-value".to_string()),
                strategy4_proxy: Some("proxy-strategy-value".to_string()),
                serve_stale: Some(true),
                parallel_query: Some(false),
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
            active_routing_id: Some(config.routing_basic_item.routing_index_id.clone()),
        };

        let settings = settings_from_app_config(&config);
        let restored = app_config_from_settings(&settings, &state).expect("round trip");

        assert_eq!(restored, config);
    }

    /// The state record, not the settings contract, owns the active ids, so an
    /// empty selection must survive the trip as an empty string rather than
    /// being resurrected from the previous configuration.
    #[test]
    fn settings_mapping_carries_the_active_ids_from_the_state_record() {
        let config = distinctly_valued_config();
        let settings = settings_from_app_config(&config);

        let restored = app_config_from_settings(&settings, &AppStateRecord::default())
            .expect("round trip without an active selection");

        assert!(restored.index_id.is_empty());
        assert!(restored.routing_basic_item.routing_index_id.is_empty());
        assert_eq!(
            restored.hysteria_item.up_mbps, config.hysteria_item.up_mbps,
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
        assert_eq!(
            restored.routing_basic_item.routing_index_id,
            config.routing_basic_item.routing_index_id
        );
        assert_eq!(restored.ui_item, AppConfig::default().ui_item);
    }

    use voya_core::KeyEventItem;

    #[derive(Default)]
    struct FakeSideEffects {
        calls: Mutex<Vec<String>>,
        fail_autostart_for: Mutex<Option<bool>>,
        fail_hotkeys_for_key: Mutex<Option<i32>>,
    }

    impl SettingsSideEffectAdapter for FakeSideEffects {
        type Error = String;

        fn apply_autostart(&self, config: &AppConfig) -> Result<(), Self::Error> {
            let enabled = config.gui_item.auto_run;
            self.calls
                .lock()
                .expect("calls lock")
                .push(format!("autostart:{enabled}"));
            if *self.fail_autostart_for.lock().expect("autostart lock") == Some(enabled) {
                return Err(format!("autostart failed for {enabled}"));
            }
            Ok(())
        }

        fn apply_hotkeys(&self, config: &AppConfig) -> Result<(), Self::Error> {
            let key = config
                .show_window_shortcut
                .as_ref()
                .and_then(|item| item.key_code)
                .unwrap_or_default();
            self.calls
                .lock()
                .expect("calls lock")
                .push(format!("hotkeys:{key}"));
            if *self.fail_hotkeys_for_key.lock().expect("hotkey lock") == Some(key) {
                return Err(format!("hotkeys failed for {key}"));
            }
            Ok(())
        }
    }

    fn config(autostart: bool, key_code: i32) -> AppConfig {
        let mut config = AppConfig::default();
        config.gui_item.auto_run = autostart;
        config.show_window_shortcut = Some(KeyEventItem {
            control: true,
            key_code: Some(key_code),
            ..KeyEventItem::default()
        });
        config
    }

    #[test]
    fn failed_hotkey_application_restores_every_touched_side_effect() {
        let original = config(false, 65);
        let target = config(true, 66);
        let effects = FakeSideEffects::default();
        *effects.fail_hotkeys_for_key.lock().expect("hotkey lock") = Some(66);

        let failure = apply_settings_side_effects(&effects, &original, &target)
            .expect_err("hotkey application should fail");

        assert_eq!(failure.stage, SettingsSideEffectStage::Hotkeys);
        assert_eq!(failure.source, "hotkeys failed for 66");
        assert!(failure.compensation_errors.is_empty());
        assert_eq!(
            *effects.calls.lock().expect("calls lock"),
            [
                "autostart:true",
                "hotkeys:66",
                "hotkeys:65",
                "autostart:false"
            ]
        );
    }

    #[test]
    fn failed_autostart_application_attempts_authoritative_restore() {
        let original = config(false, 65);
        let target = config(true, 65);
        let effects = FakeSideEffects::default();
        *effects.fail_autostart_for.lock().expect("autostart lock") = Some(true);

        let failure = apply_settings_side_effects(&effects, &original, &target)
            .expect_err("autostart application should fail");

        assert_eq!(failure.stage, SettingsSideEffectStage::Autostart);
        assert!(failure.compensation_errors.is_empty());
        assert_eq!(
            *effects.calls.lock().expect("calls lock"),
            ["autostart:true", "autostart:false"]
        );
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
    fn settings_validation_rejects_versions_urls_and_runtime_limits() {
        let mut settings = contracts::AppSettingsV1 {
            schema_version: 2,
            ..contracts::AppSettingsV1::default()
        };
        assert!(matches!(
            validate_app_settings(&settings),
            Err(AppSettingsValidationError::UnsupportedSchema { .. })
        ));

        settings.schema_version = contracts::CURRENT_SCHEMA_VERSION;
        settings.sources.geo = Some("ftp://example.test/{0}.dat".to_string());
        assert!(matches!(
            validate_app_settings(&settings),
            Err(AppSettingsValidationError::InvalidSource(_))
        ));

        settings.sources.geo = None;
        settings.sources.routing_template = Some("http://example.test/routing.json".to_string());
        assert!(matches!(
            validate_app_settings(&settings),
            Err(AppSettingsValidationError::InvalidSource(_))
        ));

        settings.sources.routing_template = None;
        settings.network.tun.mtu = 575;
        assert_eq!(
            validate_app_settings(&settings),
            Err(AppSettingsValidationError::InvalidTunMtu)
        );
    }

    /// A stored target is resolved with `parse_udp_test_target`, which fills in
    /// the named kind's default host, so a half-typed entry that reached the
    /// database would probe a server the user never named.
    #[test]
    fn settings_validation_rejects_a_malformed_udp_test_target() {
        let mut settings = contracts::AppSettingsV1::default();
        validate_app_settings(&settings).expect("the default UDP target is well formed");

        for target in ["dns:", "unknown:example.com", "2001:db8::1"] {
            settings.speed_test.udp_target = target.to_string();
            assert_eq!(
                validate_app_settings(&settings),
                Err(AppSettingsValidationError::InvalidText {
                    field: "speedTest.udpTarget",
                    label: "UDP test target",
                    reason: "value must be host:port, optionally prefixed with a test kind",
                }),
                "{target} should be rejected"
            );
        }

        // An empty target is the "use the built-in default" shape, not a typo.
        for target in ["1.1.1.1:53", "dns:1.1.1.1:53", "ntp:pool.ntp.org", ""] {
            settings.speed_test.udp_target = target.to_string();
            assert!(
                validate_app_settings(&settings).is_ok(),
                "{target} should be accepted"
            );
        }
    }

    #[test]
    fn runtime_restart_policy_ignores_appearance_only_changes() {
        let original = AppConfig::default();
        let mut appearance = original.clone();
        appearance.ui_item.current_language = "zh-Hans".to_string();
        assert!(!saved_config_requires_runtime_restart(
            &original,
            &appearance
        ));

        let mut network = original.clone();
        network.inbound[0].local_port += 1;
        assert!(saved_config_requires_runtime_restart(&original, &network));
    }

    #[test]
    fn runtime_restart_policy_tracks_srs_source_but_not_node_sorting() {
        let original = AppConfig::default();

        let mut sorted = original.clone();
        sorted.proxy_ui_item.node_sorting += 1;
        assert!(
            !saved_config_requires_runtime_restart(&original, &sorted),
            "node sorting only orders the proxy-groups snapshot the UI renders"
        );

        let mut mode = original.clone();
        mode.proxy_ui_item.traffic_mode = TrafficMode::Global;
        assert!(saved_config_requires_runtime_restart(&original, &mode));

        let mut srs = original.clone();
        srs.const_item.srs_source_url = Some("https://example.test/rules/{0}.srs".to_string());
        assert!(
            saved_config_requires_runtime_restart(&original, &srs),
            "the SRS source is embedded in every generated rule_set URL"
        );
    }

    /// Round-trip through the contract the way `save_app_settings` does, so a
    /// field that lives only in `AppConfig` cannot silently force a restart.
    fn config_from_settings(settings: &contracts::AppSettingsV1, current: &AppConfig) -> AppConfig {
        let state = AppStateRecord {
            active_profile_id: (!current.index_id.is_empty()).then(|| current.index_id.clone()),
            active_routing_id: (!current.routing_basic_item.routing_index_id.is_empty())
                .then(|| current.routing_basic_item.routing_index_id.clone()),
        };
        app_config_from_settings(settings, &state).expect("settings should map back to a config")
    }

    #[test]
    fn saving_unchanged_settings_never_requires_a_runtime_restart() {
        let mut original = AppConfig {
            index_id: "profile-a".to_string(),
            ..AppConfig::default()
        };
        original.routing_basic_item.routing_index_id = "routing-a".to_string();
        original.ui_item.current_language = "zh-Hans".to_string();
        original.const_item.srs_source_url = Some("https://example.test/{0}.srs".to_string());
        original.proxy_ui_item.node_sorting = 3;

        let target = config_from_settings(&settings_from_app_config(&original), &original);

        assert_eq!(target, original);
        assert!(!saved_config_requires_runtime_restart(&original, &target));
    }

    #[test]
    fn contract_defaults_match_domain_defaults() {
        let mut mapped = app_config_from_settings(
            &contracts::AppSettingsV1::default(),
            &AppStateRecord::default(),
        )
        .expect("default settings should map to a config");
        mapped.simple_dns_item = crate::dns::normalize_simple_dns(mapped.simple_dns_item);

        assert_eq!(
            mapped,
            AppConfig::default(),
            "a fresh install is configured from AppSettingsV1::default(), so it must \
             produce exactly AppConfig::default()"
        );
    }

    #[test]
    fn settings_validation_requires_https_for_geo_and_ruleset_sources() {
        let mut settings = contracts::AppSettingsV1::default();
        settings.sources.subscription_converter = Some("http://localhost:25500/sub".to_string());
        validate_app_settings(&settings).expect("a local converter may use plain HTTP");

        settings.sources.geo = Some("http://example.test/{0}.dat".to_string());
        assert!(matches!(
            validate_app_settings(&settings),
            Err(AppSettingsValidationError::InvalidSource(_))
        ));

        settings.sources.geo = Some("https://example.test/{0}.dat".to_string());
        settings.sources.singbox_ruleset = Some("http://example.test/{0}.srs".to_string());
        assert!(matches!(
            validate_app_settings(&settings),
            Err(AppSettingsValidationError::InvalidSource(_))
        ));

        settings.sources.singbox_ruleset = Some("https://example.test/{0}.srs".to_string());
        validate_app_settings(&settings).expect("HTTPS asset sources should be accepted");
    }

    /// The settings surface marks the input a rejection is about, so every
    /// rejection has to name an `AppSettingsV1` path — not the human label the
    /// message uses, which no form can key off.
    #[test]
    fn every_settings_rejection_names_the_contract_path_it_is_about() {
        let cases: [(contracts::AppSettingsV1, &str); 7] = [
            (
                contracts::AppSettingsV1 {
                    schema_version: contracts::CURRENT_SCHEMA_VERSION + 1,
                    ..contracts::AppSettingsV1::default()
                },
                "schemaVersion",
            ),
            (
                settings_with(|settings| settings.appearance.language = "  ".to_string()),
                "appearance.language",
            ),
            (
                settings_with(|settings| {
                    settings.sources.geo = Some("http://example.test/{0}.dat".to_string());
                }),
                "sources.geo",
            ),
            (
                settings_with(|settings| {
                    settings.sources.subscription_converter = Some("::".to_string());
                }),
                "sources.subscriptionConverter",
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
        ];

        for (settings, field) in cases {
            let error =
                validate_app_settings(&settings).expect_err("the case should be rejected: {field}");
            assert_eq!(error.field(), field, "{error}");
        }
    }

    fn settings_with(
        patch: impl FnOnce(&mut contracts::AppSettingsV1),
    ) -> contracts::AppSettingsV1 {
        let mut settings = contracts::AppSettingsV1::default();
        patch(&mut settings);
        settings
    }
}
