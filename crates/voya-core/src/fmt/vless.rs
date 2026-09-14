use super::*;

pub(super) fn parse(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri(input, "vless")?;
    let mut item = profile_from_uri(ConfigType::VLESS, &parsed);
    if let ProfileProtocol::Vless {
        uuid,
        encryption,
        flow,
        ..
    } = &mut item.protocol
    {
        *uuid = parsed.user_info;
        *encryption = nonempty(parsed.query.value_or("encryption", NONE));
        *flow = nonempty(parsed.query.value_or("flow", ""));
    }
    resolve_uri_query(&parsed.query, &mut item)?;
    ensure_address_port("vless", &item)?;
    ensure_nonempty("vless", "password", item.password())?;
    Ok(item)
}

pub(super) fn export(item: &ProfileItem) -> Result<String, ShareError> {
    ensure_address_port("vless", item)?;
    ensure_nonempty("vless", "password", item.password())?;
    let ProfileProtocol::Vless {
        uuid,
        encryption,
        flow,
        ..
    } = &item.protocol
    else {
        return Err(ShareError::WrongConfigType {
            protocol: "vless",
            actual: item.config_type(),
        });
    };
    let mut query = Vec::new();
    query.push((
        "encryption".to_string(),
        nonempty_str(encryption.as_deref())
            .unwrap_or(NONE)
            .to_string(),
    ));
    if let Some(flow) = nonempty_str(flow.as_deref()) {
        query.push(("flow".to_string(), flow.to_string()));
    }
    to_uri_query(item, Some(NONE), &mut query);
    Ok(to_uri(
        ConfigType::VLESS,
        item.address(),
        item.port(),
        uuid,
        &query,
        &item.remarks,
    ))
}
