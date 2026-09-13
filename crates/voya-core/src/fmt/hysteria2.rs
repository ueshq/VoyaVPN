use super::*;

pub(super) fn parse(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri_with_schemes(input, "hysteria2", &["hysteria2", "hy2"])?;
    let mut item = profile_from_uri(ConfigType::Hysteria2, &parsed);
    resolve_uri_query_tls_only(&parsed.query, &mut item)?;
    if let ProfileProtocol::Hysteria2 {
        password,
        port_hops,
        obfuscation_password,
        ..
    } = &mut item.protocol
    {
        *password = parsed.user_info;
        *port_hops = nonempty(parsed.query.value_or("mport", ""));
        *obfuscation_password = nonempty(parsed.query.value_or("obfs-password", ""));
    }
    ensure_address_port("hysteria2", &item)?;
    ensure_nonempty("hysteria2", "password", item.password())?;
    Ok(item)
}

pub(super) fn export(item: &ProfileItem) -> Result<String, ShareError> {
    ensure_address_port("hysteria2", item)?;
    ensure_nonempty("hysteria2", "password", item.password())?;
    let mut query = Vec::new();
    to_uri_query_lite(item, &mut query);
    let ProfileProtocol::Hysteria2 {
        port_hops,
        obfuscation_password,
        ..
    } = &item.protocol
    else {
        return Err(ShareError::WrongConfigType {
            protocol: "hysteria2",
            actual: item.config_type(),
        });
    };
    if let Some(pass) = nonempty_option(obfuscation_password) {
        query.push(("obfs".to_string(), "salamander".to_string()));
        query.push(("obfs-password".to_string(), url_encode(pass)));
    }
    if let Some(ports) = nonempty_option(port_hops) {
        query.push(("mport".to_string(), url_encode(&ports.replace(':', "-"))));
    }
    Ok(format!(
        "{}{}",
        HYSTERIA2_DEFAULT_SCHEME,
        to_uri_without_scheme(
            item.address(),
            item.port(),
            item.password(),
            &query,
            &item.remarks
        )
    ))
}
