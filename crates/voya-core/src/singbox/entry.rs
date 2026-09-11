use super::*;

pub fn generate_singbox_config(
    context: &CoreConfigContext,
) -> Result<SingboxConfig, SingboxConfigError> {
    validate_proxy_ports(context)?;
    validate_active_wireguard(context)?;
    let mut config = SingboxConfig::sample();
    gen_log(&mut config, context);
    gen_inbounds(&mut config, context);
    gen_outbounds(&mut config, context);
    gen_routing(&mut config, context);
    gen_dns(&mut config, context);
    gen_experimental(&mut config, context);
    convert_geo_to_ruleset(&mut config, context)?;
    apply_outbound_bind_interface(&mut config, context);
    apply_outbound_send_through(&mut config, context);
    Ok(config)
}

pub fn generate_singbox_config_value(
    context: &CoreConfigContext,
) -> Result<Value, SingboxConfigError> {
    let config = generate_singbox_config(context)?;
    Ok(value_from_config(&config))
}

pub fn generate_singbox_config_json(
    context: &CoreConfigContext,
) -> Result<String, SingboxConfigError> {
    let value = generate_singbox_config_value(context)?;
    serde_json::to_string_pretty(&value).map_err(SingboxConfigError::Serialize)
}

pub fn generate_singbox_speedtest_config_json(
    entries: &[SpeedtestConfigEntry],
) -> Result<String, SingboxConfigError> {
    serde_json::to_string_pretty(&generate_singbox_speedtest_config(entries))
        .map_err(SingboxConfigError::Serialize)
}

#[must_use]
pub fn generate_singbox_speedtest_config(entries: &[SpeedtestConfigEntry]) -> SingboxConfig {
    let mut config = SingboxConfig::sample();
    config.inbounds.clear();
    config.outbounds.clear();
    config.endpoints.clear();
    config.route.rules.clear();
    config.route.rule_set = None;
    config.route.final_outbound = None;
    config.route.default_domain_resolver = Some(SingboxRule {
        server: Some(SINGBOX_DIRECT_DNS_TAG.to_string()),
        ..SingboxRule::default()
    });
    config.dns = Some(SingboxDns {
        servers: vec![SingboxDnsServer {
            r#type: "udp".to_string(),
            tag: SINGBOX_DIRECT_DNS_TAG.to_string(),
            server: Some(DEFAULT_DIRECT_DNS.to_string()),
            ..SingboxDnsServer::default()
        }],
        final_server: Some(SINGBOX_DIRECT_DNS_TAG.to_string()),
        ..SingboxDns::default()
    });

    for entry in entries {
        let inbound_tag = speedtest_inbound_tag(entry.port);
        let proxy_tag = speedtest_proxy_tag(entry.port);
        config
            .inbounds
            .push(build_mixed_inbound_with(inbound_tag.as_str(), entry.port));

        append_servers(
            &mut config,
            build_proxy_servers(&entry.context, &entry.context.node, &proxy_tag),
        );
        config.route.rules.push(SingboxRule {
            inbound: Some(vec![inbound_tag]),
            outbound: Some(proxy_tag),
            ..SingboxRule::default()
        });
    }

    apply_common_config_settings(&mut config, entries);

    config
}

fn apply_common_config_settings(config: &mut SingboxConfig, entries: &[SpeedtestConfigEntry]) {
    let Some(entry) = entries.first() else {
        return;
    };
    gen_log(config, &entry.context);
    apply_outbound_bind_interface(config, &entry.context);
    apply_outbound_send_through(config, &entry.context);
}

fn speedtest_inbound_tag(port: i32) -> String {
    format!("mixed{port}")
}

fn speedtest_proxy_tag(port: i32) -> String {
    format!("{PROXY_TAG}{port}")
}

fn validate_active_wireguard(context: &CoreConfigContext) -> Result<(), SingboxConfigError> {
    if context.node.config_type() == ConfigType::WireGuard
        && wireguard_public_key(&context.node.protocol).is_none()
    {
        return Err(SingboxConfigError::MissingWireGuardPublicKey {
            remarks: context.node.remarks.clone(),
        });
    }
    Ok(())
}

fn validate_proxy_ports(context: &CoreConfigContext) -> Result<(), SingboxConfigError> {
    let mut pending = vec![context.node.clone()];
    let mut seen = BTreeSet::new();

    while let Some(node) = pending.pop() {
        if !node.index_id.is_empty() && !seen.insert(node.index_id.clone()) {
            continue;
        }
        if !(1..=65535).contains(&node.port()) {
            let port = node.port();
            return Err(SingboxConfigError::InvalidNodePort {
                remarks: node.remarks,
                port,
            });
        }
    }

    Ok(())
}

/// The `log.level` values sing-box accepts, per
/// <https://sing-box.sagernet.org/configuration/log/>. sing-box refuses the
/// whole config on any other value, so nothing outside this set may be
/// forwarded verbatim.
const SINGBOX_LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error", "fatal", "panic"];

/// Level emitted when the stored app level is empty, disabled, or unrecognised.
///
/// It is the sing-box spelling of `crate::DEFAULT_LOG_LEVEL`. Anything quieter
/// would hide connection failures, and anything louder would make a stale or
/// hand-edited setting silently opt the core into per-connection logging.
const SINGBOX_FALLBACK_LOG_LEVEL: &str = "warn";

/// Translates the app's log level onto sing-box's accepted `log.level` set.
///
/// The settings UI stores `none`/`trace`/`debug`/`info`/`warn`/`warning`/`error`,
/// so `warning` needs sing-box's spelling and `none` (which is expressed as
/// `log.disabled` instead) must not reach `log.level` at all.
fn singbox_log_level(configured: &str) -> &'static str {
    let lowercased = configured.trim().to_ascii_lowercase();
    let normalized = lowercased.as_str();
    // The app inherited v2rayN's `warning`; sing-box only accepts `warn`.
    if normalized == "warning" {
        return "warn";
    }
    SINGBOX_LOG_LEVELS
        .iter()
        .copied()
        .find(|level| *level == normalized)
        .unwrap_or(SINGBOX_FALLBACK_LOG_LEVEL)
}

fn gen_log(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let mut log = config.log.clone().unwrap_or_default();
    let configured = context.app_config.core_basic_item.loglevel.trim();
    // Always overwrite: the sample config ships `debug`, so leaving an
    // unmapped level in place would run the core at its noisiest setting.
    log.level = singbox_log_level(configured).to_string();
    if configured.eq_ignore_ascii_case("none") {
        log.disabled = Some(true);
    }
    if context.app_config.core_basic_item.log_enabled {
        log.output = Some("sbox.log".to_string());
    }
    config.log = Some(log);
}
