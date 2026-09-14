use super::*;

pub(super) fn gen_dns(config: &mut SingboxConfig, context: &CoreConfigContext) {
    gen_dns_servers(config, context);
    gen_dns_rules(config, context);

    let use_direct_dns = final_dns_uses_direct(context);
    let dns = config.dns.get_or_insert_with(SingboxDns::default);
    dns.independent_cache = Some(true);
    dns.final_server = Some(
        if use_direct_dns {
            SINGBOX_DIRECT_DNS_TAG
        } else {
            SINGBOX_REMOTE_DNS_TAG
        }
        .to_string(),
    );
    apply_tun_dns_reverse_mapping(dns, context);

    let simple_dns = &context.simple_dns_item;
    if !use_direct_dns
        && simple_dns.fake_ip == Some(true)
        && simple_dns.global_fake_ip == Some(false)
    {
        dns.rules.push(SingboxRule {
            server: Some(SINGBOX_FAKE_DNS_TAG.to_string()),
            query_type: Some(vec![1, 28]),
            rewrite_ttl: Some(1),
            ..SingboxRule::default()
        });
    }
}

fn gen_dns_servers(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let simple_dns = &context.simple_dns_item;
    let mut bootstrap_dns = parse_dns_address_or_default(
        simple_dns
            .bootstrap_dns
            .as_deref()
            .unwrap_or(DEFAULT_BOOTSTRAP_DNS),
        DEFAULT_BOOTSTRAP_DNS,
    );
    bootstrap_dns.tag = SINGBOX_LOCAL_DNS_TAG.to_string();

    let mut direct_dns = parse_dns_address_or_default(
        simple_dns
            .direct_dns
            .as_deref()
            .unwrap_or(DEFAULT_DIRECT_DNS),
        DEFAULT_DIRECT_DNS,
    );
    direct_dns.tag = SINGBOX_DIRECT_DNS_TAG.to_string();
    direct_dns.domain_resolver = Some(SINGBOX_LOCAL_DNS_TAG.to_string());

    let mut remote_dns = parse_dns_address_or_default(
        simple_dns
            .remote_dns
            .as_deref()
            .unwrap_or(DEFAULT_REMOTE_DNS),
        DEFAULT_REMOTE_DNS,
    );
    remote_dns.tag = SINGBOX_REMOTE_DNS_TAG.to_string();
    remote_dns.detour = Some(PROXY_TAG.to_string());
    remote_dns.domain_resolver = Some(SINGBOX_LOCAL_DNS_TAG.to_string());

    let mut predefined = BTreeMap::new();
    if simple_dns.add_common_hosts == Some(true) {
        for (host, addresses) in predefined_hosts() {
            predefined.insert(
                host.to_string(),
                addresses
                    .iter()
                    .map(|address| (*address).to_string())
                    .collect(),
            );
        }
    }
    for (host, addresses) in parse_hosts_to_dictionary(simple_dns.hosts.as_deref()) {
        let mut test_rule = SingboxRule::default();
        if !parse_v2_domain(&host, &mut test_rule) {
            continue;
        }
        normalize_bare_host_domain(&host, &mut test_rule);
        if let Some(domain) = test_rule.domain.as_ref().and_then(|items| items.first()) {
            let ips = addresses
                .into_iter()
                .filter(|address| is_ip_address(address))
                .collect::<Vec<_>>();
            predefined.insert(domain.clone(), ips);
        }
    }

    for host in predefined.keys() {
        if bootstrap_dns.server.as_deref() == Some(host.as_str()) {
            bootstrap_dns.domain_resolver = Some(SINGBOX_HOSTS_DNS_TAG.to_string());
        }
        if remote_dns.server.as_deref() == Some(host.as_str()) {
            remote_dns.domain_resolver = Some(SINGBOX_HOSTS_DNS_TAG.to_string());
        }
        if direct_dns.server.as_deref() == Some(host.as_str()) {
            direct_dns.domain_resolver = Some(SINGBOX_HOSTS_DNS_TAG.to_string());
        }
    }

    let mut servers = vec![
        bootstrap_dns,
        remote_dns,
        direct_dns,
        SingboxDnsServer {
            tag: SINGBOX_HOSTS_DNS_TAG.to_string(),
            r#type: "hosts".to_string(),
            predefined: Some(predefined),
            ..SingboxDnsServer::default()
        },
    ];
    if simple_dns.fake_ip == Some(true) {
        servers.push(SingboxDnsServer {
            tag: SINGBOX_FAKE_DNS_TAG.to_string(),
            r#type: "fakeip".to_string(),
            inet4_range: Some(SINGBOX_FAKEIP_INET4_RANGE.to_string()),
            inet6_range: Some(SINGBOX_FAKEIP_INET6_RANGE.to_string()),
            ..SingboxDnsServer::default()
        });
    }

    config.dns.get_or_insert_with(SingboxDns::default).servers = servers;
}

fn gen_dns_rules(config: &mut SingboxConfig, context: &CoreConfigContext) {
    let simple_dns = &context.simple_dns_item;
    let mut rules = vec![SingboxRule {
        ip_accept_any: Some(true),
        server: Some(SINGBOX_HOSTS_DNS_TAG.to_string()),
        ..SingboxRule::default()
    }];

    if !context.protect_domain_list.is_empty() {
        rules.push(SingboxRule {
            server: Some(SINGBOX_DIRECT_DNS_TAG.to_string()),
            strategy: domain_strategy4_sbox(simple_dns.strategy4_freedom.as_deref()),
            domain: Some(context.protect_domain_list.clone()),
            ..SingboxRule::default()
        });
    }

    // Mirror the route generator: Global mode precedes the user's rules.
    rules.push(SingboxRule {
        server: Some(SINGBOX_REMOTE_DNS_TAG.to_string()),
        strategy: domain_strategy4_sbox(simple_dns.strategy4_proxy.as_deref()),
        clash_mode: Some("Global".to_string()),
        ..SingboxRule::default()
    });

    for (host, addresses) in parse_hosts_to_dictionary(simple_dns.hosts.as_deref()) {
        let Some(predefined) = addresses.first() else {
            continue;
        };
        if predefined.is_empty() {
            continue;
        }
        let mut rule = SingboxRule {
            query_type: Some(vec![1, 5, 28]),
            action: Some("predefined".to_string()),
            rcode: Some("NOERROR".to_string()),
            ..SingboxRule::default()
        };
        if !parse_v2_domain(&host, &mut rule) {
            continue;
        }
        normalize_bare_host_domain(&host, &mut rule);
        if let Some(rcode) = predefined
            .strip_prefix('#')
            .and_then(|value| value.parse::<i32>().ok())
        {
            rule.rcode = Some(dns_rcode(rcode).to_string());
        } else if is_domain_name(predefined) {
            rule.answer = Some(vec![format!("*. IN CNAME {predefined}.")]);
        } else if is_ip_address(predefined) && rule.domain.as_ref().is_none_or(Vec::is_empty) {
            if predefined.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
                rule.answer = Some(vec![format!("*. IN AAAA {predefined}")]);
            } else {
                rule.answer = Some(vec![format!("*. IN A {predefined}")]);
            }
        } else {
            continue;
        }
        rules.push(rule);
    }

    if simple_dns.block_binding_query == Some(true) {
        rules.push(SingboxRule {
            query_type: Some(vec![64, 65]),
            action: Some("predefined".to_string()),
            rcode: Some("NOERROR".to_string()),
            ..SingboxRule::default()
        });
    }

    if simple_dns.fake_ip == Some(true) && simple_dns.global_fake_ip == Some(true) {
        let mut fakeip_filter_rule = fakeip_filter_rule();
        fakeip_filter_rule.invert = Some(true);
        rules.push(SingboxRule {
            server: Some(SINGBOX_FAKE_DNS_TAG.to_string()),
            r#type: Some("logical".to_string()),
            mode: Some("and".to_string()),
            rewrite_ttl: Some(1),
            rules: Some(vec![
                SingboxRule {
                    query_type: Some(vec![1, 28]),
                    ..SingboxRule::default()
                },
                fakeip_filter_rule,
            ]),
            ..SingboxRule::default()
        });
    }

    append_dns_routing_rules(&mut rules, context);
    config.dns.get_or_insert_with(SingboxDns::default).rules = rules;
}

fn apply_tun_dns_reverse_mapping(dns: &mut SingboxDns, context: &CoreConfigContext) {
    if context.is_tun_enabled {
        dns.reverse_mapping = Some(true);
    }
}

fn append_dns_routing_rules(rules: &mut Vec<SingboxRule>, context: &CoreConfigContext) {
    let Some(routing) = context.routing_item.as_ref() else {
        return;
    };
    let simple_dns = &context.simple_dns_item;
    let (expected_ip_cidr, expected_ip_regions, region_name) =
        parse_direct_expected_ips(simple_dns.direct_expected_ips.as_deref());

    for item in routing
        .rule_set
        .iter()
        .filter(|item| item.enabled && item.rule_type != Some(RuleType::Routing))
    {
        let Some(domains) = item.domain.as_ref().filter(|domains| !domains.is_empty()) else {
            continue;
        };
        let mut rule = SingboxRule::default();
        let valid_domains = domains
            .iter()
            .filter(|domain| parse_v2_domain(domain, &mut rule))
            .count();
        if valid_domains == 0 {
            continue;
        }

        match item.outbound_tag.as_deref() {
            Some(DIRECT_TAG) => {
                rule.server = Some(SINGBOX_DIRECT_DNS_TAG.to_string());
                rule.strategy = domain_strategy4_sbox(simple_dns.strategy4_freedom.as_deref());
                if !expected_ip_regions.is_empty() && !region_name.is_empty() {
                    if let Some(geosite) = &mut rule.geosite {
                        let matched_geosite = geosite
                            .iter()
                            .filter(|item| {
                                item.ends_with(&format!("-{region_name}"))
                                    || item.ends_with(&format!("@{region_name}"))
                                    || *item == &region_name
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        if !matched_geosite.is_empty() {
                            geosite.retain(|item| !matched_geosite.contains(item));
                            let mut expected_rule = rule.clone();
                            expected_rule.geosite = Some(matched_geosite);
                            expected_rule.geoip = Some(expected_ip_regions.clone());
                            if !expected_ip_cidr.is_empty() {
                                expected_rule.ip_cidr = Some(expected_ip_cidr.clone());
                            }
                            rules.push(expected_rule);
                        }
                    }
                }
            }
            Some(BLOCK_TAG) => {
                rule.action = Some("predefined".to_string());
                rule.rcode = Some("NXDOMAIN".to_string());
            }
            _ => {
                if simple_dns.fake_ip == Some(true) && simple_dns.global_fake_ip == Some(false) {
                    let mut fake_rule = rule.clone();
                    fake_rule.server = Some(SINGBOX_FAKE_DNS_TAG.to_string());
                    fake_rule.query_type = Some(vec![1, 28]);
                    fake_rule.rewrite_ttl = Some(1);
                    rules.push(fake_rule);
                }
                rule.server = Some(SINGBOX_REMOTE_DNS_TAG.to_string());
                rule.strategy = domain_strategy4_sbox(simple_dns.strategy4_proxy.as_deref());
            }
        }

        if dns_rule_has_matcher(&rule) {
            rules.push(rule);
        }
    }
}

fn dns_rule_has_matcher(rule: &SingboxRule) -> bool {
    rule.domain.as_ref().is_some_and(|items| !items.is_empty())
        || rule
            .domain_suffix
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || rule
            .domain_keyword
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || rule
            .domain_regex
            .as_ref()
            .is_some_and(|items| !items.is_empty())
        || rule.geosite.as_ref().is_some_and(|items| !items.is_empty())
        || rule.geoip.as_ref().is_some_and(|items| !items.is_empty())
        || rule.ip_cidr.as_ref().is_some_and(|items| !items.is_empty())
        || rule
            .rule_set
            .as_ref()
            .is_some_and(|items| !items.is_empty())
}

fn final_dns_uses_direct(context: &CoreConfigContext) -> bool {
    // Mirror the route generator's filter (routing.rs): a disabled or DNS-only
    // trailing rule never becomes `route.final`, so it must not flip `dns.final`
    // to the plaintext direct resolver either.
    let Some(last_rule) = context.routing_item.as_ref().and_then(|routing| {
        routing
            .rule_set
            .iter()
            .rev()
            .find(|item| item.enabled && item.rule_type != Some(RuleType::DNS))
    }) else {
        return false;
    };
    if last_rule.outbound_tag.as_deref() != Some(DIRECT_TAG) {
        return false;
    }

    let no_domain = last_rule.domain.as_ref().is_none_or(Vec::is_empty);
    let no_process = last_rule.process.as_ref().is_none_or(Vec::is_empty);
    let is_any_ip = last_rule
        .ip
        .as_ref()
        .is_none_or(|ips| ips.is_empty() || ips.iter().any(|ip| ip == "0.0.0.0/0"));
    let is_any_port = last_rule
        .port
        .as_deref()
        .is_none_or(|port| port.is_empty() || port == "0-65535");
    let is_any_network = last_rule
        .network
        .as_deref()
        .is_none_or(|network| network.is_empty() || network == "tcp,udp");

    no_domain && no_process && is_any_ip && is_any_port && is_any_network
}

mod address;
pub use address::first_dns_address;
pub(super) use address::*;

fn fakeip_filter_rule() -> SingboxRule {
    SingboxRule {
        domain: Some(vec![
            "amobile.music.tc.qq.com".to_string(),
            "api-jooxtt.sanook.com".to_string(),
            "api.joox.com".to_string(),
            "aqqmusic.tc.qq.com".to_string(),
            "dl.stream.qqmusic.qq.com".to_string(),
            "ff.dorado.sdo.com".to_string(),
            "heartbeat.belkin.com".to_string(),
            "isure.stream.qqmusic.qq.com".to_string(),
            "joox.com".to_string(),
            "lens.l.google.com".to_string(),
            "localhost.ptlogin2.qq.com".to_string(),
            "localhost.sec.qq.com".to_string(),
            "mesu.apple.com".to_string(),
            "mobileoc.music.tc.qq.com".to_string(),
            "music.taihe.com".to_string(),
            "musicapi.taihe.com".to_string(),
            "na.b.g-tun.com".to_string(),
            "proxy.golang.org".to_string(),
            "ps.res.netease.com".to_string(),
            "shark007.net".to_string(),
            "songsearch.kugou.com".to_string(),
            "static.adtidy.org".to_string(),
            "streamoc.music.tc.qq.com".to_string(),
            "swcdn.apple.com".to_string(),
            "swdist.apple.com".to_string(),
            "swdownload.apple.com".to_string(),
            "swquery.apple.com".to_string(),
            "swscan.apple.com".to_string(),
            "turn.cloudflare.com".to_string(),
            "trackercdn.kugou.com".to_string(),
            "xnotify.xboxlive.com".to_string(),
        ]),
        domain_keyword: Some(vec![
            "ntp".to_string(),
            "stun".to_string(),
            "time".to_string(),
        ]),
        domain_regex: Some(vec![
            "^[^.]+$".to_string(),
            r"^[^.]+\.[^.]+\.xboxlive\.com$".to_string(),
            r"^localhost\.[^.]+\.weixin\.qq\.com$".to_string(),
            r"^mijia\scloud$".to_string(),
            r"^xbox\.[^.]+\.microsoft\.com$".to_string(),
            r"^xbox\.[^.]+\.[^.]+\.microsoft\.com$".to_string(),
        ]),
        domain_suffix: Some(vec![
            "126.net".to_string(),
            "3gppnetwork.org".to_string(),
            "battle.net".to_string(),
            "battlenet.com.cn".to_string(),
            "cdn.nintendo.net".to_string(),
            "cmbchina.com".to_string(),
            "cmbimg.com".to_string(),
            "ff14.sdo.com".to_string(),
            "ffxiv.com".to_string(),
            "finalfantasyxiv.com".to_string(),
            "gcloudcs.com".to_string(),
            "home.arpa".to_string(),
            "invalid".to_string(),
            "kuwo.cn".to_string(),
            "lan".to_string(),
            "linksys.com".to_string(),
            "linksyssmartwifi.com".to_string(),
            "local".to_string(),
            "localdomain".to_string(),
            "localhost".to_string(),
            "market.xiaomi.com".to_string(),
            "mcdn.bilivideo.cn".to_string(),
            "media.dssott.com".to_string(),
            "msftconnecttest.com".to_string(),
            "msftncsi.com".to_string(),
            "music.163.com".to_string(),
            "music.migu.cn".to_string(),
            "n0808.com".to_string(),
            "nflxvideo.net".to_string(),
            "oray.com".to_string(),
            "orayimg.com".to_string(),
            "router.asus.com".to_string(),
            "sandai.net".to_string(),
            "square-enix.com".to_string(),
            "srv.nintendo.net".to_string(),
            "steamcontent.com".to_string(),
            "uu.163.com".to_string(),
            "wargaming.net".to_string(),
            "wggames.cn".to_string(),
            "wotgame.cn".to_string(),
            "wowsgame.cn".to_string(),
            "xiami.com".to_string(),
            "y.qq.com".to_string(),
        ]),
        ..SingboxRule::default()
    }
}
