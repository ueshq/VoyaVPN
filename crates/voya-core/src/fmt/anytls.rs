use super::*;

pub(super) fn parse(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri(input, "anytls")?;
    let mut item = profile_from_uri(ConfigType::Anytls, &parsed);
    if let ProfileProtocol::Anytls { password, .. } = &mut item.protocol {
        *password = parsed.user_info;
    }
    resolve_uri_query_tls_only(&parsed.query, &mut item)?;
    ensure_address_port("anytls", &item)?;
    ensure_nonempty("anytls", "password", item.password())?;
    Ok(item)
}

pub(super) fn export(item: &ProfileItem) -> Result<String, ShareError> {
    ensure_address_port("anytls", item)?;
    ensure_nonempty("anytls", "password", item.password())?;
    let mut query = Vec::new();
    to_uri_query(item, Some(NONE), &mut query);
    Ok(to_uri(
        ConfigType::Anytls,
        item.address(),
        item.port(),
        item.password(),
        &query,
        &item.remarks,
    ))
}
