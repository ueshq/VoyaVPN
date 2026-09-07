use super::*;

pub(crate) fn parse_direct_expected_ips(value: Option<&str>) -> (Vec<String>, Vec<String>, String) {
    let mut ip_cidr = Vec::new();
    let mut regions = Vec::new();
    let mut region_name = String::new();
    for item in value
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        if let Some(region) = item.strip_prefix(GEOIP_PREFIX) {
            if !region.is_empty() {
                regions.push(region.to_string());
                region_name = region.to_string();
            }
        } else {
            ip_cidr.push(item.to_string());
        }
    }
    (ip_cidr, regions, region_name)
}

pub(crate) fn parse_dns_address_or_default(
    address: &str,
    default_address: &str,
) -> SingboxDnsServer {
    parse_dns_address(address)
        .or_else(|| parse_dns_address(default_address))
        .unwrap_or_default()
}

pub(crate) fn parse_dns_address(address: &str) -> Option<SingboxDnsServer> {
    let address_first = first_dns_address(address)?;
    if matches!(address_first.as_str(), "local" | "localhost") {
        return Some(SingboxDnsServer {
            r#type: "local".to_string(),
            ..SingboxDnsServer::default()
        });
    }

    let (domain, scheme, port, path) = parse_url_parts(&address_first)?;
    if scheme.eq_ignore_ascii_case("dhcp") {
        // sing-box's DHCP server option is `interface`, not `server`.
        return Some(SingboxDnsServer {
            r#type: "dhcp".to_string(),
            interface_name: (!domain.is_empty() && domain != "auto").then_some(domain),
            ..SingboxDnsServer::default()
        });
    }

    let server_type = if scheme.is_empty() {
        "udp".to_string()
    } else {
        scheme.replace("+local", "").to_lowercase()
    };
    Some(SingboxDnsServer {
        r#type: server_type.clone(),
        server: (!domain.is_empty()).then_some(domain),
        server_port: port.map(i32::from),
        path: matches!(server_type.as_str(), "https" | "h3")
            .then(|| path)
            .filter(|path| !path.is_empty() && path != "/"),
        ..SingboxDnsServer::default()
    })
}

fn first_dns_address(address: &str) -> Option<String> {
    let delimiter = if address.contains(',') { ',' } else { ';' };
    address
        .split(delimiter)
        .map(str::trim)
        .find(|item| !item.is_empty())
        .map(str::to_string)
}

fn parse_url_parts(input: &str) -> Option<(String, String, Option<u16>, String)> {
    if let Ok(url) = url::Url::parse(input) {
        if let Some(host) = url.host_str() {
            let mut path = url.path().to_string();
            if let Some(query) = url.query() {
                path.push('?');
                path.push_str(query);
            }
            let port = match url.port() {
                Some(0) => return None,
                Some(port) => Some(port),
                None => None,
            };
            return Some((
                strip_host_brackets(host),
                url.scheme().to_string(),
                port,
                path,
            ));
        }
    }
    // Anything with a scheme has already been handled by `Url::parse`; if that
    // failed the input is malformed, not a bare authority.
    if input.contains("://") {
        return None;
    }

    // Schemeless fallback: a bare `host`, `host:port` or `[v6]:port`, with an
    // optional path that only the DoH/H3 branch keeps.
    let authority_end = input.find(['/', '?', '#']).unwrap_or(input.len());
    let authority = &input[..authority_end];
    let path = if authority_end < input.len() && input[authority_end..].starts_with('/') {
        input[authority_end..]
            .split('#')
            .next()
            .unwrap_or_default()
            .to_string()
    } else {
        String::new()
    };
    let (domain, port) = parse_authority(authority)?;
    if domain.is_empty() {
        Some((input.to_string(), String::new(), None, String::new()))
    } else {
        Some((domain, String::new(), port, path))
    }
}

fn parse_authority(authority: &str) -> Option<(String, Option<u16>)> {
    if authority.is_empty() {
        return Some((String::new(), None));
    }
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, authority)| authority);
    if authority.starts_with('[') {
        if let Some(closing_bracket_index) = authority.rfind(']') {
            let domain = strip_host_brackets(&authority[..=closing_bracket_index]);
            let rest = authority
                .get(closing_bracket_index + 1..)
                .unwrap_or_default();
            if rest.is_empty() {
                return Some((domain, None));
            }
            let port = parse_authority_port(rest.strip_prefix(':')?)?;
            return Some((domain, Some(port)));
        }
    }
    if let Some((domain, port)) = authority.rsplit_once(':') {
        if !domain.contains(':') {
            let port = parse_authority_port(port)?;
            return Some((domain.to_string(), Some(port)));
        }
    }
    Some((authority.to_string(), None))
}

fn parse_authority_port(port: &str) -> Option<u16> {
    port.parse::<u16>().ok().filter(|port| *port > 0)
}

/// sing-box parses `dns.servers[].server` with `netip.ParseAddr`, which rejects
/// the bracketed IPv6 authority form that URL parsing hands back.
fn strip_host_brackets(host: &str) -> String {
    host.trim_matches(['[', ']']).to_string()
}

pub(crate) fn domain_strategy4_sbox(strategy: Option<&str>) -> Option<String> {
    let strategy = strategy?;
    if strategy.starts_with("UseIPv4") {
        Some("prefer_ipv4".to_string())
    } else if strategy.starts_with("UseIPv6") {
        Some("prefer_ipv6".to_string())
    } else if strategy.starts_with("ForceIPv4") {
        Some("ipv4_only".to_string())
    } else if strategy.starts_with("ForceIPv6") {
        Some("ipv6_only".to_string())
    } else {
        None
    }
}

pub(crate) fn dns_rcode(value: i32) -> &'static str {
    match value {
        1 => "FORMERR",
        2 => "SERVFAIL",
        3 => "NXDOMAIN",
        4 => "NOTIMP",
        5 => "REFUSED",
        _ => "NOERROR",
    }
}

pub(crate) fn is_domain_name(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !is_ip_address(value)
        && value.contains('.')
        && value
            .rsplit('.')
            .next()
            .is_some_and(|tld| tld.chars().all(|ch| ch.is_ascii_alphanumeric()))
}

pub(crate) fn is_ip_address(value: &str) -> bool {
    value.trim().parse::<IpAddr>().is_ok()
}

pub(crate) fn normalize_bare_host_domain(host: &str, rule: &mut SingboxRule) {
    if host.contains(':') {
        return;
    }
    if let Some(domain_keyword) = rule.domain_keyword.take() {
        if !domain_keyword.is_empty() {
            rule.domain = Some(domain_keyword);
        }
    }
}

pub(crate) fn parse_hosts_to_dictionary(
    hosts_content: Option<&str>,
) -> BTreeMap<String, Vec<String>> {
    let mut result = BTreeMap::new();
    for line in hosts_content
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty() && !line.starts_with('#') && line.contains(char::is_whitespace)
        })
    {
        let parts = line
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        result
            .entry(parts[0].clone())
            .or_insert_with(Vec::new)
            .extend(parts.into_iter().skip(1));
    }
    result
}

pub(crate) fn predefined_hosts() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        (
            "dns.google",
            &[
                "8.8.8.8",
                "8.8.4.4",
                "2001:4860:4860::8888",
                "2001:4860:4860::8844",
            ],
        ),
        (
            "dns.alidns.com",
            &[
                "223.5.5.5",
                "223.6.6.6",
                "2400:3200::1",
                "2400:3200:baba::1",
            ],
        ),
        (
            "one.one.one.one",
            &[
                "1.1.1.1",
                "1.0.0.1",
                "2606:4700:4700::1111",
                "2606:4700:4700::1001",
            ],
        ),
        (
            "1dot1dot1dot1.cloudflare-dns.com",
            &[
                "1.1.1.1",
                "1.0.0.1",
                "2606:4700:4700::1111",
                "2606:4700:4700::1001",
            ],
        ),
        (
            "cloudflare-dns.com",
            &[
                "104.16.249.249",
                "104.16.248.249",
                "2606:4700::6810:f8f9",
                "2606:4700::6810:f9f9",
            ],
        ),
        (
            "dns.cloudflare.com",
            &[
                "104.16.132.229",
                "104.16.133.229",
                "2606:4700::6810:84e5",
                "2606:4700::6810:85e5",
            ],
        ),
        ("dot.pub", &["1.12.12.12", "120.53.53.53"]),
        ("doh.pub", &["1.12.12.12", "120.53.53.53"]),
        (
            "dns.quad9.net",
            &["9.9.9.9", "149.112.112.112", "2620:fe::fe", "2620:fe::9"],
        ),
        (
            "dns.yandex.net",
            &[
                "77.88.8.8",
                "77.88.8.1",
                "2a02:6b8::feed:0ff",
                "2a02:6b8:0:1::feed:0ff",
            ],
        ),
        ("dns.sb", &["185.222.222.222", "2a09::"]),
        (
            "dns.umbrella.com",
            &[
                "208.67.220.220",
                "208.67.222.222",
                "2620:119:35::35",
                "2620:119:53::53",
            ],
        ),
        (
            "dns.sse.cisco.com",
            &[
                "208.67.220.220",
                "208.67.222.222",
                "2620:119:35::35",
                "2620:119:53::53",
            ],
        ),
        ("engage.cloudflareclient.com", &["162.159.192.1"]),
    ]
}
