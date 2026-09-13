use super::*;

pub(super) fn parse(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri(input, "tuic")?;
    let mut item = profile_from_uri(ConfigType::TUIC, &parsed);
    resolve_uri_query_tls_only(&parsed.query, &mut item)?;
    if let Some((username, password)) = parsed.user_info.split_once(':') {
        if let ProfileProtocol::Tuic {
            uuid,
            password: item_password,
            congestion_control,
            ..
        } = &mut item.protocol
        {
            *uuid = username.to_string();
            *item_password = password.to_string();
            *congestion_control = nonempty(parsed.query.value_or("congestion_control", ""));
        }
    }
    ensure_address_port("tuic", &item)?;
    ensure_nonempty("tuic", "username", item.username())?;
    ensure_nonempty("tuic", "password", item.password())?;
    Ok(item)
}

pub(super) fn export(item: &ProfileItem) -> Result<String, ShareError> {
    ensure_address_port("tuic", item)?;
    ensure_nonempty("tuic", "username", item.username())?;
    ensure_nonempty("tuic", "password", item.password())?;
    let ProfileProtocol::Tuic {
        uuid,
        password,
        congestion_control,
        ..
    } = &item.protocol
    else {
        return Err(ShareError::WrongConfigType {
            protocol: "tuic",
            actual: item.config_type(),
        });
    };
    let mut query = Vec::new();
    to_uri_query_lite(item, &mut query);
    if let Some(congestion) = nonempty_option(congestion_control) {
        query.push(("congestion_control".to_string(), congestion.to_string()));
    }
    Ok(to_uri(
        ConfigType::TUIC,
        item.address(),
        item.port(),
        &format!("{uuid}:{password}"),
        &query,
        &item.remarks,
    ))
}
