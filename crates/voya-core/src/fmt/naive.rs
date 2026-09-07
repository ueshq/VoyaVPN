use super::*;

#[derive(Debug, Clone, Copy)]
pub struct NaiveFmt;

impl ShareFmt for NaiveFmt {
    fn config_type(&self) -> ConfigType {
        ConfigType::Naive
    }

    fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed =
            parse_uri_with_schemes(input, "naive", &["naive", "naive+https", "naive+quic"])?;
        let mut item = profile_from_uri(ConfigType::Naive, &parsed);
        resolve_uri_query_tls_only(&parsed.query, &mut item);
        if let ProfileProtocol::Naive {
            username,
            password,
            quic,
            insecure_concurrency,
            ..
        } = &mut item.protocol
        {
            *quic = parsed.scheme.contains("quic");
            if let Some((parsed_username, parsed_password)) = parsed.user_info.split_once(':') {
                *username = parsed_username.to_string();
                *password = parsed_password.to_string();
            } else {
                *password = parsed.user_info;
            }
            *insecure_concurrency =
                parse_positive_i32(&parsed.query.value_or("insecure-concurrency", ""));
        }
        ensure_address_port("naive", &item)?;
        ensure_nonempty("naive", "password", item.password())?;
        Ok(item)
    }

    fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        ensure_type("naive", item, ConfigType::Naive)?;
        ensure_address_port("naive", item)?;
        ensure_nonempty("naive", "password", item.password())?;
        let ProfileProtocol::Naive {
            username,
            password,
            quic,
            insecure_concurrency,
            ..
        } = &item.protocol
        else {
            return Err(ShareError::WrongConfigType {
                protocol: "naive",
                actual: item.config_type(),
            });
        };
        let mut query = Vec::new();
        to_uri_query(item, Some(NONE), &mut query);
        if let Some(concurrency) = insecure_concurrency.filter(|value| *value > 0) {
            query.push(("insecure-concurrency".to_string(), concurrency.to_string()));
        }
        let user_info = if username.is_empty() {
            url_encode(password)
        } else {
            format!("{}:{}", url_encode(username), url_encode(password))
        };
        let scheme = if *quic {
            NAIVE_QUIC_SCHEME
        } else {
            NAIVE_HTTPS_SCHEME
        };
        Ok(format!(
            "{scheme}{}",
            to_uri_without_scheme_preencoded_userinfo(
                item.address(),
                item.port(),
                &user_info,
                &query,
                &item.remarks
            )
        ))
    }
}
