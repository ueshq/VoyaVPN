use crate::{
    protocol_common::split_csv, text::nonempty_str, CoreConfigContext, RuleType, RulesItem,
    BLOCK_TAG, DIRECT_TAG, PROXY_TAG,
};

use super::{
    dns::{direct_dns_strategy, normalize_bare_host_domain, parse_hosts_to_dictionary},
    outbounds::{append_servers, build_policy_group_servers, build_proxy_server, rule_group_tag},
    support::exe_name,
    SingboxConfig, SingboxRule, GEOSITE_PREFIX, SINGBOX_DIRECT_DNS_TAG,
};

pub(super) fn gen_routing(config: &mut SingboxConfig, context: &CoreConfigContext) {
    config.route.final_outbound = Some(PROXY_TAG.to_string());
    let dns_config = &context.dns;
    config.route.default_domain_resolver = Some(SingboxRule {
        server: Some(SINGBOX_DIRECT_DNS_TAG.to_string()),
        strategy: direct_dns_strategy(context, dns_config.direct_strategy),
        ..SingboxRule::default()
    });

    if context.is_tun_enabled {
        config.route.auto_detect_interface = Some(true);
        config.route.rules.extend(tun_route_rules());

        // The core's own sockets go direct. The macOS NetworkExtension tunnel
        // cannot tell which process a packet came from (the PacketTunnel's
        // connection-owner lookup always fails), so the rule could never
        // match there and is left out.
        if !context.platform.is_macos() {
            config.route.rules.push(SingboxRule {
                outbound: Some(DIRECT_TAG.to_string()),
                process_name: Some(vec!["sing-box".to_string()]),
                ..SingboxRule::default()
            });
        }
        let icmp_routing = tun_icmp_routing(&context.app_config.tun.icmp_routing);
        match icmp_routing {
            "direct" => config.route.rules.push(SingboxRule {
                network: Some(vec!["icmp".to_string()]),
                outbound: Some(DIRECT_TAG.to_string()),
                ..SingboxRule::default()
            }),
            "unreachable" | "drop" | "reply" => {
                let method = match icmp_routing {
                    "unreachable" => "default",
                    "drop" => "drop",
                    _ => "reply",
                };
                config.route.rules.push(SingboxRule {
                    network: Some(vec!["icmp".to_string()]),
                    action: Some("reject".to_string()),
                    method: Some(method.to_string()),
                    ..SingboxRule::default()
                });
            }
            _ => {}
        }
    }

    if context
        .app_config
        .inbounds
        .first()
        .is_none_or(|inbound| inbound.sniffing_enabled)
    {
        config.route.rules.push(SingboxRule {
            action: Some("sniff".to_string()),
            ..SingboxRule::default()
        });
        config.route.rules.push(SingboxRule {
            r#type: Some("logical".to_string()),
            mode: Some("or".to_string()),
            action: Some("hijack-dns".to_string()),
            rules: Some(vec![
                SingboxRule {
                    port: Some(vec![53]),
                    ..SingboxRule::default()
                },
                SingboxRule {
                    protocol: Some(vec!["dns".to_string()]),
                    ..SingboxRule::default()
                },
            ]),
            ..SingboxRule::default()
        });
    } else {
        config.route.rules.push(SingboxRule {
            port: Some(vec![53]),
            action: Some("hijack-dns".to_string()),
            ..SingboxRule::default()
        });
    }

    if let Some(hosts_resolve_rule) = hosts_resolve_rule(dns_config) {
        config.route.rules.push(hosts_resolve_rule);
    }

    let rejects_proxied_ipv6 = context.ipv6_mode().rejects_proxied_ipv6();
    // Global mode sends everything to the node, so an IPv6 destination the
    // node cannot reach is refused here rather than dialled through it.
    if rejects_proxied_ipv6 {
        config.route.rules.push(SingboxRule {
            clash_mode: Some("Global".to_string()),
            ip_version: Some(6),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        });
    }

    // Global mode sends everything through the proxy ahead of the user's rules.
    config.route.rules.push(SingboxRule {
        outbound: Some(PROXY_TAG.to_string()),
        clash_mode: Some("Global".to_string()),
        ..SingboxRule::default()
    });

    if let Some(routing) = context.routing_item.clone() {
        for item in routing
            .rule_set
            .iter()
            .filter(|item| item.enabled && item.rule_type != Some(RuleType::DNS))
        {
            gen_routing_user_rule(config, context, item);
        }
    }

    // An IPv6 destination no rule sent elsewhere would reach `final`, the
    // node, which has no IPv6 egress (or IPv6 is switched off). Refuse it
    // here, after the user's rules, so a direct rule still claims its IPv6
    // destinations: apps such as WeChat receive CDN addresses from their own
    // servers rather than from DNS, and rejecting those before the rules ran
    // left transfers retrying a dead IPv6 connection. Under TUN the reject
    // cannot fail fast either way: the local stack completes the handshake
    // first, so rejecting only what would have failed at the node is the
    // one placement that loses nothing.
    if rejects_proxied_ipv6 {
        config.route.rules.push(SingboxRule {
            ip_version: Some(6),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        });
    }
}

fn tun_route_rules() -> Vec<SingboxRule> {
    vec![
        SingboxRule {
            network: Some(vec!["udp".to_string()]),
            port: Some(vec![135, 137, 138, 139, 5353]),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        },
        SingboxRule {
            ip_cidr: Some(vec!["224.0.0.0/3".to_string(), "ff00::/8".to_string()]),
            action: Some("reject".to_string()),
            ..SingboxRule::default()
        },
    ]
}

fn tun_icmp_routing(value: &str) -> &str {
    match value {
        "direct" | "unreachable" | "drop" | "reply" => value,
        _ => "rule",
    }
}

fn hosts_resolve_rule(dns_config: &crate::DnsConfig) -> Option<SingboxRule> {
    let host_keys = parse_hosts_to_dictionary(dns_config.hosts.as_deref())
        .into_keys()
        .collect::<Vec<_>>();
    if host_keys.is_empty() {
        return None;
    }

    let mut rule = SingboxRule {
        action: Some("resolve".to_string()),
        ..SingboxRule::default()
    };
    let mut count = 0;
    for host in host_keys {
        let mut domain_rule = SingboxRule::default();
        if !parse_v2_domain(&host, &mut domain_rule) {
            continue;
        }
        normalize_bare_host_domain(&host, &mut domain_rule);
        if let Some(items) = domain_rule.domain {
            rule.domain.get_or_insert_with(Vec::new).extend(items);
            count += 1;
        } else if let Some(items) = domain_rule.domain_keyword {
            rule.domain_keyword
                .get_or_insert_with(Vec::new)
                .extend(items);
            count += 1;
        } else if let Some(items) = domain_rule.domain_suffix {
            rule.domain_suffix
                .get_or_insert_with(Vec::new)
                .extend(items);
            count += 1;
        } else if let Some(items) = domain_rule.domain_regex {
            rule.domain_regex.get_or_insert_with(Vec::new).extend(items);
            count += 1;
        } else if let Some(items) = domain_rule.geosite {
            rule.geosite.get_or_insert_with(Vec::new).extend(items);
            count += 1;
        }
    }

    (count > 0).then_some(rule)
}

fn gen_routing_user_rule(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
    user_rule: &RulesItem,
) {
    let outbound_tag = gen_routing_user_rule_outbound(
        config,
        context,
        user_rule.outbound_tag.as_deref().unwrap_or(PROXY_TAG),
    );
    let mut rule = SingboxRule {
        outbound: Some(outbound_tag.clone()),
        ..SingboxRule::default()
    };
    if outbound_tag == BLOCK_TAG {
        rule.outbound = None;
        rule.action = Some("reject".to_string());
    }
    fill_rule_common(&mut rule, user_rule);

    let mut has_domain_ip_process = false;
    if let Some(domains) = &user_rule.domain {
        let mut domain_rule = rule.clone();
        let count = domains
            .iter()
            .filter(|domain| parse_v2_domain(domain, &mut domain_rule))
            .count();
        if count > 0 {
            config.route.rules.push(domain_rule);
            has_domain_ip_process = true;
        }
    }
    if let Some(ips) = &user_rule.ip {
        let mut ip_rule = rule.clone();
        let negative_ips = ips
            .iter()
            .filter_map(|ip| ip.strip_prefix('!').map(str::trim))
            .collect::<Vec<_>>();
        let count = if negative_ips.is_empty() {
            ips.iter()
                .filter(|ip| parse_v2_address(ip, &mut ip_rule))
                .count()
        } else {
            let mut positive_rule = rule.clone();
            positive_rule.outbound = None;
            positive_rule.action = None;
            let mut negative_rule = SingboxRule::default();
            let positive_count = ips
                .iter()
                .filter(|ip| !ip.starts_with('!'))
                .filter(|ip| parse_v2_address(ip, &mut positive_rule))
                .count();
            let negative_count = negative_ips
                .iter()
                .filter(|ip| parse_v2_address(ip, &mut negative_rule))
                .count();
            if positive_count > 0 && negative_count > 0 && route_ip_rule_has_matcher(&negative_rule)
            {
                negative_rule.invert = Some(true);
                ip_rule = SingboxRule {
                    outbound: rule.outbound.clone(),
                    action: rule.action.clone(),
                    r#type: Some("logical".to_string()),
                    mode: Some("and".to_string()),
                    rules: Some(vec![positive_rule, negative_rule]),
                    ..SingboxRule::default()
                };
                positive_count + negative_count
            } else if positive_count > 0 {
                ip_rule = positive_rule;
                positive_count
            } else {
                has_domain_ip_process = true;
                0
            }
        };
        if count > 0 {
            config.route.rules.push(ip_rule);
            has_domain_ip_process = true;
        }
    }
    // The macOS NetworkExtension tunnel cannot tell which app a packet came
    // from, so an app condition there would never match; leave it out rather
    // than emit a rule sing-box rejects or silently ignores.
    if let Some(processes) = user_rule
        .process
        .as_ref()
        .filter(|_| !context.platform.is_macos())
    {
        let mut process_name_rule = rule.clone();
        let mut process_path_rule = rule.clone();
        for process in processes {
            if process == "self/" {
                process_name_rule
                    .process_name
                    .get_or_insert_with(Vec::new)
                    .push("sing-box".to_string());
                continue;
            }
            if process.contains('/') || process.contains('\\') {
                process_path_rule
                    .process_path
                    .get_or_insert_with(Vec::new)
                    .push(process.clone());
            } else {
                process_name_rule
                    .process_name
                    .get_or_insert_with(Vec::new)
                    .push(exe_name(process));
            }
        }
        if process_name_rule
            .process_name
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        {
            config.route.rules.push(process_name_rule);
            has_domain_ip_process = true;
        }
        if process_path_rule
            .process_path
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        {
            config.route.rules.push(process_path_rule);
            has_domain_ip_process = true;
        }
    }

    if !has_domain_ip_process
        && (rule.port.is_some()
            || rule.port_range.is_some()
            || rule.protocol.is_some()
            || rule.inbound.is_some()
            || rule.network.is_some())
    {
        config.route.rules.push(rule);
    }
}

fn fill_rule_common(rule: &mut SingboxRule, user_rule: &RulesItem) {
    if let Some(port) = nonempty_str(user_rule.port.as_deref()) {
        let mut ports = Vec::new();
        let mut port_ranges = Vec::new();
        for item in port
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            if item.contains('-') {
                port_ranges.push(item.replace('-', ":"));
            } else if let Ok(port) = item.parse::<i32>() {
                ports.push(port);
            }
        }
        if !ports.is_empty() {
            rule.port = Some(ports);
        }
        if !port_ranges.is_empty() {
            rule.port_range = Some(port_ranges);
        }
    }
    if let Some(network) = nonempty_str(user_rule.network.as_deref()) {
        rule.network = Some(split_csv(network));
    }
    rule.protocol = user_rule.protocol.clone().filter(|items| !items.is_empty());
    rule.inbound = user_rule
        .inbound_tag
        .clone()
        .filter(|items| !items.is_empty());
}

fn route_ip_rule_has_matcher(rule: &SingboxRule) -> bool {
    rule.geoip.as_ref().is_some_and(|items| !items.is_empty())
        || rule.ip_cidr.as_ref().is_some_and(|items| !items.is_empty())
        || rule.ip_is_private == Some(true)
}

/// A rule that names a policy group. The active group already is the `proxy`
/// outbound; any other group gets its own outbound, added once.
fn gen_rule_group_outbound(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
    group_id: &str,
) -> String {
    if context
        .policy_group
        .as_ref()
        .is_some_and(|active| active.group.id == group_id)
    {
        return PROXY_TAG.to_string();
    }
    // Resolution already failed the build for a group that is not here.
    let Some(group) = context
        .rule_policy_groups
        .iter()
        .find(|group| group.group.id == group_id)
    else {
        return PROXY_TAG.to_string();
    };
    let tag = rule_group_tag(&group.group);
    if config.outbounds.iter().any(|outbound| outbound.tag == tag) {
        return tag;
    }
    append_servers(
        config,
        build_policy_group_servers(context, group, &tag, Some(&tag)),
    );
    tag
}

fn gen_routing_user_rule_outbound(
    config: &mut SingboxConfig,
    context: &CoreConfigContext,
    outbound_tag: &str,
) -> String {
    if [PROXY_TAG, DIRECT_TAG, BLOCK_TAG].contains(&outbound_tag) {
        return outbound_tag.to_string();
    }
    if let Some(group_id) = outbound_tag.strip_prefix(crate::GROUP_OUTBOUND_PREFIX) {
        return gen_rule_group_outbound(config, context, group_id);
    }

    let Some(node) = context
        .all_proxies_map
        .get(&format!("remark:{outbound_tag}"))
        .cloned()
    else {
        return PROXY_TAG.to_string();
    };

    let tag = format!("{}-{PROXY_TAG}-{}", node.index_id, node.remarks);
    if config
        .outbounds
        .iter()
        .any(|outbound| outbound.tag.starts_with(&tag))
        || config
            .endpoints
            .iter()
            .any(|endpoint| endpoint.tag.starts_with(&tag))
    {
        return tag;
    }

    let Some(server) = build_proxy_server(context, &node, &tag) else {
        return PROXY_TAG.to_string();
    };
    append_servers(config, [server]);
    tag
}

pub(super) fn parse_v2_domain(domain: &str, rule: &mut SingboxRule) -> bool {
    if domain.starts_with('#') || domain.starts_with("ext:") || domain.starts_with("ext-domain:") {
        return false;
    }
    if let Some(value) = domain.strip_prefix(GEOSITE_PREFIX) {
        rule.geosite
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else if let Some(value) = domain.strip_prefix("regexp:") {
        rule.domain_regex
            .get_or_insert_with(Vec::new)
            .push(value.replace("<COMMA>", ","));
    } else if let Some(value) = domain.strip_prefix("domain:") {
        rule.domain_suffix
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else if let Some(value) = domain.strip_prefix("full:") {
        rule.domain
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else if let Some(value) = domain.strip_prefix("keyword:") {
        rule.domain_keyword
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else if let Some(value) = domain.strip_prefix("dotless:") {
        rule.domain_keyword
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else {
        rule.domain_keyword
            .get_or_insert_with(Vec::new)
            .push(domain.to_string());
    }
    true
}

fn parse_v2_address(address: &str, rule: &mut SingboxRule) -> bool {
    if address.starts_with("ext:") || address.starts_with("ext-ip:") {
        return false;
    }
    if address == "geoip:private" {
        rule.ip_is_private = Some(true);
    } else if let Some(value) = address.strip_prefix("geoip:") {
        rule.geoip
            .get_or_insert_with(Vec::new)
            .push(value.to_string());
    } else {
        rule.ip_cidr
            .get_or_insert_with(Vec::new)
            .push(address.to_string());
    }
    true
}
