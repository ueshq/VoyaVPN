use super::*;

pub(super) fn gen_experimental(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let mut experimental = config.experimental.clone().unwrap_or_default();
    experimental.clash_api = Some(SingboxClashApi {
        // `clash_api_port` is the single source of truth for this process's
        // controller port: with the Linux pre-socks split the main config keeps
        // the base api2 port while only the TUN config takes api2 + 1.
        external_controller: Some(format!("{LOOPBACK}:{}", context.clash_api_port())),
        secret: nonempty_string(context.clash_api_secret.as_deref()),
        store_selected: None,
    });

    if context.app_config.core_basic_item.enable_cache_file4_sbox {
        experimental.cache_file = Some(SingboxCacheFile {
            enabled: true,
            path: Some("cache.db".to_string()),
            cache_id: None,
            store_fakeip: (context.simple_dns_item.fake_ip == Some(true)).then_some(true),
        });
    }

    config.experimental = Some(experimental);
}

pub(super) fn convert_geo_to_ruleset(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
) -> Result<(), SingboxConfigError> {
    let mut rule_sets = Vec::new();
    for rule in &mut config.route.rules {
        convert_rule_geo_to_ruleset(rule, &mut rule_sets);
    }
    if let Some(dns) = &mut config.dns {
        for rule in &mut dns.rules {
            convert_rule_geo_to_ruleset(rule, &mut rule_sets);
        }
    }

    let unique_rule_sets = rule_sets
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<BTreeSet<_>>();
    if unique_rule_sets.is_empty() {
        return Ok(());
    }

    let custom_rulesets = parse_inline_custom_rulesets(
        context
            .routing_item
            .as_ref()
            .map(|routing| routing.custom_ruleset_path4_singbox.as_str()),
    )?;
    let source_url = SINGBOX_RULESET_URL;
    config.route.rule_set = Some(
        unique_rule_sets
            .into_iter()
            .map(|tag| {
                custom_rulesets
                    .iter()
                    .find(|ruleset| ruleset.tag.as_deref() == Some(tag.as_str()))
                    .cloned()
                    .unwrap_or_else(|| ruleset_for_tag(&tag, source_url, context))
            })
            .collect(),
    );
    Ok(())
}

fn convert_rule_geo_to_ruleset(rule: &mut SingboxRule, rule_sets: &mut Vec<String>) {
    let mut converted = Vec::new();
    if rule.geosite.as_ref().is_some_and(|items| !items.is_empty()) {
        if let Some(geosite) = rule.geosite.take() {
            converted.extend(geosite.into_iter().map(|item| format!("geosite-{item}")));
        }
    }
    if rule.geoip.as_ref().is_some_and(|items| !items.is_empty()) {
        if let Some(geoip) = rule.geoip.take() {
            converted.extend(geoip.into_iter().map(|item| format!("geoip-{item}")));
        }
    }
    if !converted.is_empty() {
        rule.rule_set.get_or_insert_with(Vec::new).extend(converted);
    }
    if let Some(rule_set) = &rule.rule_set {
        rule_sets.extend(rule_set.clone());
    }
    if let Some(nested_rules) = &mut rule.rules {
        for nested_rule in nested_rules {
            convert_rule_geo_to_ruleset(nested_rule, rule_sets);
        }
    }
}

fn parse_inline_custom_rulesets(
    value: Option<&str>,
) -> Result<Vec<SingboxRuleset>, SingboxConfigError> {
    let Some(value) = value.map(str::trim).filter(|value| value.starts_with('[')) else {
        return Ok(Vec::new());
    };
    let rulesets = serde_json::from_str::<Vec<SingboxRuleset>>(value)
        .map_err(SingboxConfigError::CustomRulesetJson)?;
    for (index, ruleset) in rulesets.iter().enumerate() {
        if ruleset
            .tag
            .as_deref()
            .and_then(|value| nonempty_str(Some(value)))
            .is_none()
            || ruleset
                .r#type
                .as_deref()
                .and_then(|value| nonempty_str(Some(value)))
                .is_none()
            || ruleset
                .format
                .as_deref()
                .and_then(|value| nonempty_str(Some(value)))
                .is_none()
        {
            return Err(SingboxConfigError::CustomRulesetMissingRequiredFields { index });
        }
    }
    Ok(rulesets)
}

fn ruleset_for_tag(tag: &str, source_url: &str, context: &CoreConfigContext) -> SingboxRuleset {
    if let Some(path) = context.singbox_ruleset_paths.get(tag) {
        return SingboxRuleset {
            tag: Some(tag.to_string()),
            r#type: Some("local".to_string()),
            format: Some("binary".to_string()),
            path: Some(path.clone()),
            ..SingboxRuleset::default()
        };
    }

    remote_ruleset(tag, source_url)
}

fn remote_ruleset(tag: &str, source_url: &str) -> SingboxRuleset {
    let kind = if tag.starts_with("geosite") {
        "geosite"
    } else {
        "geoip"
    };
    SingboxRuleset {
        tag: Some(tag.to_string()),
        r#type: Some("remote".to_string()),
        format: Some("binary".to_string()),
        url: Some(
            source_url
                .replace("{0}", kind)
                .replace("{1}", tag)
                .to_string(),
        ),
        download_detour: Some(PROXY_TAG.to_string()),
        ..SingboxRuleset::default()
    }
}
