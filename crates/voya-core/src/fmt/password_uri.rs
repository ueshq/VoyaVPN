//! `trojan://` and `anytls://` links: the userinfo is the password, and the two
//! differ only in which query parameters they read and their default security.

use super::*;

pub(super) struct PasswordUri {
    protocol: &'static str,
    config_type: ConfigType,
    resolve_query: fn(&Query, &mut ProfileItem) -> Result<(), ShareError>,
    security_default: Option<&'static str>,
}

pub(super) const TROJAN: PasswordUri = PasswordUri {
    protocol: "trojan",
    config_type: ConfigType::Trojan,
    resolve_query: resolve_uri_query,
    security_default: None,
};

/// AnyTLS is TLS by definition, so its links keep TLS unless they ask for
/// REALITY and export without a `security=` default.
pub(super) const ANYTLS: PasswordUri = PasswordUri {
    protocol: "anytls",
    config_type: ConfigType::Anytls,
    resolve_query: resolve_uri_query_tls_only,
    security_default: Some(NONE),
};

impl PasswordUri {
    pub(super) fn parse(&self, input: &str) -> Result<ProfileItem, ShareError> {
        let parsed = parse_uri(input, self.protocol)?;
        let mut item = profile_from_uri(self.config_type, &parsed);
        if let ProfileProtocol::Trojan { password, .. } | ProfileProtocol::Anytls { password, .. } =
            &mut item.protocol
        {
            *password = parsed.user_info;
        }
        (self.resolve_query)(&parsed.query, &mut item)?;
        self.ensure_complete(&item)?;
        Ok(item)
    }

    pub(super) fn export(&self, item: &ProfileItem) -> Result<String, ShareError> {
        self.ensure_complete(item)?;
        let mut query = Vec::new();
        to_uri_query(item, self.security_default, &mut query);
        Ok(to_uri(
            self.config_type,
            item.address(),
            item.port(),
            item.password(),
            &query,
            &item.remarks,
        ))
    }

    fn ensure_complete(&self, item: &ProfileItem) -> Result<(), ShareError> {
        ensure_address_port(self.protocol, item)?;
        ensure_nonempty(self.protocol, "password", item.password())
    }
}
