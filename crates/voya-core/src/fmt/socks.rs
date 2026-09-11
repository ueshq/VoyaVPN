use super::*;

pub(super) fn export(item: &ProfileItem) -> Result<String, ShareError> {
    ensure_address_port("socks", item)?;
    // Anonymous SOCKS proxies export without any userinfo at all; encoding
    // an empty credential pair would emit a meaningless `Og` blob.
    let user_info = if item.username().is_empty() && item.password().is_empty() {
        String::new()
    } else {
        base64_encode(&format!("{}:{}", item.username(), item.password()), true)
    };
    Ok(to_uri(
        ConfigType::SOCKS,
        item.address(),
        item.port(),
        &user_info,
        &[],
        &item.remarks,
    ))
}

pub(super) fn parse(input: &str) -> Result<ProfileItem, ShareError> {
    let parsed = parse_uri_with_schemes(input, "socks", &["socks", "socks5", "socks4"])?;
    let mut item = profile_from_uri(ConfigType::SOCKS, &parsed);
    let mut parsed_username = String::new();
    let mut parsed_password = String::new();
    if !parsed.user_info.is_empty() {
        if let Some((username, password)) = parsed.user_info.split_once(':') {
            parsed_username = username.to_string();
            parsed_password = password.to_string();
        } else if let Some((username, password)) = decode_socks_userinfo(&parsed.user_info) {
            parsed_username = username;
            parsed_password = password;
        } else {
            // Not a base64 `user:pass` blob, so this is a plain username such as
            // `socks://user@proxy.example:1080`.
            parsed_username = parsed.user_info.clone();
        }
    }
    if let ProfileProtocol::Socks {
        username, password, ..
    } = &mut item.protocol
    {
        *username = parsed_username;
        *password = parsed_password;
    }
    ensure_address_port("socks", &item)?;
    Ok(item)
}

fn decode_socks_userinfo(user_info: &str) -> Option<(String, String)> {
    let decoded = base64_decode(user_info, "socks").ok()?;
    let (username, password) = decoded.split_once(':')?;
    Some((username.to_string(), password.to_string()))
}
