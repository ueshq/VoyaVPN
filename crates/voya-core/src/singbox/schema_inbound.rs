use super::*;

/// One sing-box inbound. Local proxy and TUN inbounds use the listener and TUN
/// fields; the server inbounds of a self-hosted node add `users`, `method`,
/// `password`, and `tls`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxInbound {
    pub r#type: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_route: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_route: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_independent_nat: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<SingboxTunPlatform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<SingboxUser>>,
    /// Shadowsocks server cipher.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Shadowsocks server key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<SingboxInboundTls>,
}

impl Default for SingboxInbound {
    fn default() -> Self {
        Self {
            r#type: "mixed".to_string(),
            tag: "socks".to_string(),
            listen: Some(LOOPBACK.to_string()),
            listen_port: None,
            interface_name: None,
            address: None,
            mtu: None,
            auto_route: None,
            strict_route: None,
            endpoint_independent_nat: None,
            stack: None,
            platform: None,
            users: None,
            method: None,
            password: None,
            tls: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxTunPlatform {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_proxy: Option<SingboxTunHttpProxy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxTunHttpProxy {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_port: Option<i32>,
}

/// An inbound user: `username`/`password` for the mixed inbound, and
/// `name`/`uuid`/`flow` for a VLESS server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
}

/// Server-side TLS of an inbound. Only REALITY is generated: a self-hosted
/// node borrows the handshake of a public site instead of holding a
/// certificate.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxInboundTls {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality: Option<SingboxRealityServer>,
}

impl Default for SingboxInboundTls {
    fn default() -> Self {
        Self {
            enabled: true,
            server_name: None,
            reality: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxRealityServer {
    pub enabled: bool,
    pub handshake: SingboxRealityHandshake,
    pub private_key: String,
    pub short_id: Vec<String>,
}

impl Default for SingboxRealityServer {
    fn default() -> Self {
        Self {
            enabled: true,
            handshake: SingboxRealityHandshake::default(),
            private_key: String::new(),
            short_id: Vec::new(),
        }
    }
}

/// The site whose TLS handshake a REALITY server forwards unauthenticated
/// clients to.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SingboxRealityHandshake {
    pub server: String,
    pub server_port: i32,
}
