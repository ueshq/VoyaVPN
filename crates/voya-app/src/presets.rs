use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use voya_core::{AppConfig, CoreType, DnsItem, PresetType, SimpleDnsDefaults, SimpleDnsItem};
use voya_db::{Database, DbError};
use voya_net::{
    PresetDnsTemplateClient, PresetDnsTemplateFetchOptions, RegionalPreset, RegionalPresetCatalog,
    RegionalPresetSources,
};

pub type Result<T> = std::result::Result<T, PresetManagerError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetApplyOptions {
    pub prefer_proxy: bool,
    pub proxy_url: Option<String>,
}

impl Default for PresetApplyOptions {
    fn default() -> Self {
        Self {
            prefer_proxy: true,
            proxy_url: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetApplyResult {
    pub preset_type: PresetType,
    pub geo_source_url: Option<String>,
    pub srs_source_url: Option<String>,
    pub route_rules_template_source_url: Option<String>,
    pub singbox_dns_fetched: bool,
    pub simple_dns_fetched: bool,
    pub fallback_custom_dns_enabled: bool,
}

#[derive(Debug, Error)]
pub enum PresetManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("regional preset {0:?} does not have network sources")]
    UnsupportedPreset(PresetType),
}

#[derive(Debug, Clone)]
pub struct PresetManager<'db> {
    database: &'db Database,
    sources: RegionalPresetCatalog,
}

impl<'db> PresetManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self {
            database,
            sources: RegionalPresetCatalog::default(),
        }
    }

    #[must_use]
    pub fn with_sources(database: &'db Database, sources: RegionalPresetCatalog) -> Self {
        Self { database, sources }
    }

    pub async fn apply(
        &self,
        config: &mut AppConfig,
        preset_type: PresetType,
        options: PresetApplyOptions,
    ) -> Result<PresetApplyResult> {
        match preset_type {
            PresetType::Default => self.apply_default(config).await,
            PresetType::Russia => {
                self.apply_region(
                    config,
                    PresetType::Russia,
                    self.sources.sources(RegionalPreset::Russia).clone(),
                    options,
                )
                .await
            }
            PresetType::Iran => {
                self.apply_region(
                    config,
                    PresetType::Iran,
                    self.sources.sources(RegionalPreset::Iran).clone(),
                    options,
                )
                .await
            }
        }
    }

    async fn apply_default(&self, config: &mut AppConfig) -> Result<PresetApplyResult> {
        config.const_item.geo_source_url = None;
        config.const_item.srs_source_url = None;
        config.const_item.route_rules_template_source_url = None;
        config.simple_dns_item = SimpleDnsDefaults::builtin();

        let mut singbox = self.current_dns_item(CoreType::sing_box).await?;
        reset_dns_item(&mut singbox, CoreType::sing_box);
        self.database.dns().upsert(&singbox).await?;

        Ok(PresetApplyResult {
            preset_type: PresetType::Default,
            geo_source_url: None,
            srs_source_url: None,
            route_rules_template_source_url: None,
            singbox_dns_fetched: false,
            simple_dns_fetched: false,
            fallback_custom_dns_enabled: false,
        })
    }

    async fn apply_region(
        &self,
        config: &mut AppConfig,
        preset_type: PresetType,
        sources: RegionalPresetSources,
        options: PresetApplyOptions,
    ) -> Result<PresetApplyResult> {
        config.const_item.geo_source_url = nonempty(sources.geo_source_url.clone());
        config.const_item.srs_source_url = nonempty(sources.srs_source_url.clone());
        config.const_item.route_rules_template_source_url =
            nonempty(sources.route_rules_template_source_url.clone());

        let fetch_options = PresetDnsTemplateFetchOptions {
            prefer_proxy: options.prefer_proxy,
            proxy_url: options.proxy_url,
        };
        let client = PresetDnsTemplateClient::new();
        let templates = client
            .fetch(&sources.dns_template_source_url, &fetch_options)
            .await;

        let current_singbox = self.current_dns_item(CoreType::sing_box).await?;
        let (mut singbox, singbox_dns_fetched) = external_dns_item(
            CoreType::sing_box,
            current_singbox,
            templates.singbox_template.as_deref(),
            &client,
            &fetch_options,
        )
        .await;
        let simple_dns = external_simple_dns_item(templates.simple_template.as_deref());
        let simple_dns_fetched = simple_dns.is_some();
        let fallback_custom_dns_enabled = simple_dns.is_none();

        if let Some(simple_dns) = simple_dns {
            config.simple_dns_item = simple_dns;
        } else {
            singbox.enabled = true;
            config.simple_dns_item = SimpleDnsDefaults::builtin();
        }

        self.database.dns().upsert(&singbox).await?;

        Ok(PresetApplyResult {
            preset_type,
            geo_source_url: config.const_item.geo_source_url.clone(),
            srs_source_url: config.const_item.srs_source_url.clone(),
            route_rules_template_source_url: config
                .const_item
                .route_rules_template_source_url
                .clone(),
            singbox_dns_fetched,
            simple_dns_fetched,
            fallback_custom_dns_enabled,
        })
    }

    async fn current_dns_item(&self, core_type: CoreType) -> Result<DnsItem> {
        Ok(self
            .database
            .dns()
            .get_by_core_type(core_type)
            .await?
            .unwrap_or_else(|| default_dns_item(core_type)))
    }
}

async fn external_dns_item(
    core_type: CoreType,
    current: DnsItem,
    template_content: Option<&str>,
    client: &PresetDnsTemplateClient,
    fetch_options: &PresetDnsTemplateFetchOptions,
) -> (DnsItem, bool) {
    let Some(mut template) = template_content.and_then(parse_optional_json::<DnsItem>) else {
        return (current, false);
    };

    template.normal_dns = resolve_dns_body(template.normal_dns.as_deref(), client, fetch_options)
        .await
        .or(template.normal_dns);
    template.tun_dns = resolve_dns_body(template.tun_dns.as_deref(), client, fetch_options)
        .await
        .or(template.tun_dns);
    template.id = current.id;
    template.enabled = current.enabled;
    template.remarks = current.remarks;
    template.core_type = core_type;

    (template, true)
}

async fn resolve_dns_body(
    value: Option<&str>,
    client: &PresetDnsTemplateClient,
    fetch_options: &PresetDnsTemplateFetchOptions,
) -> Option<String> {
    let value = value.map(str::trim).filter(|value| !value.is_empty())?;
    if is_http_url(value) {
        client.fetch_optional(value, fetch_options).await
    } else {
        Some(value.to_string())
    }
}

fn external_simple_dns_item(template_content: Option<&str>) -> Option<SimpleDnsItem> {
    template_content.and_then(parse_optional_json::<SimpleDnsItem>)
}

fn parse_optional_json<T>(content: &str) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str::<Option<T>>(content.trim())
        .ok()
        .flatten()
}

fn reset_dns_item(item: &mut DnsItem, core_type: CoreType) {
    let id = std::mem::take(&mut item.id);
    let remarks = if item.remarks.trim().is_empty() {
        default_dns_item(core_type).remarks
    } else {
        item.remarks.trim().to_string()
    };

    *item = default_dns_item(core_type);
    item.id = id;
    item.remarks = remarks;
}

fn default_dns_item(core_type: CoreType) -> DnsItem {
    crate::dns::default_dns_item(core_type)
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn nonempty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };
    use voya_db::Database;
    use voya_net::{RegionalPresetCatalog, RegionalPresetSources};

    use super::*;

    #[tokio::test]
    async fn preset_apply_successful_fetch_writes_routing_dns_and_simple_dns() {
        let seen_paths = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([
                (
                    "/dns/sing_box.json".to_string(),
                    format!(
                        r#"{{"NormalDNS":"{base}/sing-normal.json"}}"#,
                        base = "__BASE__"
                    ),
                ),
                (
                    "/dns/simple_dns.json".to_string(),
                    r#"{"DirectDNS":"8.8.8.8","RemoteDNS":"https://dns.google/dns-query","FakeIP":true}"#
                        .to_string(),
                ),
                (
                    "/sing-normal.json".to_string(),
                    r#"{"servers":[{"tag":"remote","type":"https","server":"dns.google"}]}"#
                        .to_string(),
                ),
            ]),
            3,
            Arc::clone(&seen_paths),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("preset manager test operation should succeed");
        let manager = PresetManager::with_sources(&database, test_catalog(&base));
        let mut config = AppConfig::default();

        let result = manager
            .apply(
                &mut config,
                PresetType::Russia,
                PresetApplyOptions {
                    prefer_proxy: false,
                    proxy_url: None,
                },
            )
            .await
            .expect("preset manager test operation should succeed");

        let singbox = database
            .dns()
            .get_by_core_type(CoreType::sing_box)
            .await
            .expect("preset manager test operation should succeed")
            .expect("preset manager test operation should succeed");

        assert_eq!(
            result.route_rules_template_source_url.as_deref(),
            Some("https://example.test/routing-russia.json")
        );
        assert!(result.singbox_dns_fetched);
        assert!(result.simple_dns_fetched);
        assert!(!result.fallback_custom_dns_enabled);
        assert_eq!(
            config.const_item.geo_source_url.as_deref(),
            Some("https://example.test/geo-russia/{0}.dat")
        );
        assert_eq!(
            config.simple_dns_item.direct_dns.as_deref(),
            Some("8.8.8.8")
        );
        assert_eq!(config.simple_dns_item.fake_ip, Some(true));
        assert_eq!(
            singbox.normal_dns.as_deref(),
            Some(r#"{"servers":[{"tag":"remote","type":"https","server":"dns.google"}]}"#)
        );
        assert_eq!(seen_paths.lock().await.len(), 3);
    }

    #[tokio::test]
    async fn preset_apply_null_simple_dns_fallback_enables_custom_dns() {
        let seen_paths = Arc::new(Mutex::new(Vec::new()));
        let base = spawn_http_fixture(
            HashMap::from([
                (
                    "/dns/sing_box.json".to_string(),
                    r#"{"NormalDNS":"{\"servers\":[{\"tag\":\"remote\",\"type\":\"udp\",\"server\":\"9.9.9.9\"}]}"}"#
                        .to_string(),
                ),
                ("/dns/simple_dns.json".to_string(), "null".to_string()),
            ]),
            2,
            Arc::clone(&seen_paths),
        )
        .await;
        let database = Database::connect_in_memory()
            .await
            .expect("preset manager test operation should succeed");
        let manager = PresetManager::with_sources(&database, test_catalog(&base));
        let mut config = AppConfig::default();

        let result = manager
            .apply(
                &mut config,
                PresetType::Iran,
                PresetApplyOptions {
                    prefer_proxy: false,
                    proxy_url: None,
                },
            )
            .await
            .expect("preset manager test operation should succeed");

        let singbox = database
            .dns()
            .get_by_core_type(CoreType::sing_box)
            .await
            .expect("preset manager test operation should succeed")
            .expect("preset manager test operation should succeed");

        assert_eq!(
            result.route_rules_template_source_url.as_deref(),
            Some("https://example.test/routing-iran.json")
        );
        assert!(result.fallback_custom_dns_enabled);
        assert!(!result.simple_dns_fetched);
        assert!(singbox.enabled);
        assert_eq!(
            config.simple_dns_item.direct_dns.as_deref(),
            Some(voya_core::DEFAULT_DIRECT_DNS)
        );
        assert_eq!(
            config.const_item.srs_source_url.as_deref(),
            Some("https://example.test/srs-iran/{1}.srs")
        );
        assert_eq!(seen_paths.lock().await.len(), 2);
    }

    fn test_catalog(base: &str) -> RegionalPresetCatalog {
        RegionalPresetCatalog {
            russia: RegionalPresetSources {
                geo_source_url: "https://example.test/geo-russia/{0}.dat".to_string(),
                srs_source_url: "https://example.test/srs-russia/{1}.srs".to_string(),
                route_rules_template_source_url: "https://example.test/routing-russia.json"
                    .to_string(),
                dns_template_source_url: format!("{base}/dns/"),
            },
            iran: RegionalPresetSources {
                geo_source_url: "https://example.test/geo-iran/{0}.dat".to_string(),
                srs_source_url: "https://example.test/srs-iran/{1}.srs".to_string(),
                route_rules_template_source_url: "https://example.test/routing-iran.json"
                    .to_string(),
                dns_template_source_url: format!("{base}/dns/"),
            },
        }
    }

    async fn spawn_http_fixture(
        routes: HashMap<String, String>,
        max_requests: usize,
        seen_paths: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("preset manager test operation should succeed");
        let address = listener
            .local_addr()
            .expect("preset manager test operation should succeed");
        let routes = Arc::new(routes);

        tokio::spawn(async move {
            for _ in 0..max_requests {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let routes = Arc::clone(&routes);
                let seen_paths = Arc::clone(&seen_paths);
                tokio::spawn(async move {
                    let mut buffer = vec![0; 8192];
                    let bytes_read = socket.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .and_then(|target| target.split('?').next())
                        .unwrap_or("/")
                        .to_string();
                    seen_paths.lock().await.push(path.clone());
                    let body = routes
                        .get(&path)
                        .map(|body| body.replace("__BASE__", &format!("http://{address}")))
                        .unwrap_or_default();
                    let status = if routes.contains_key(&path) {
                        "200 OK"
                    } else {
                        "404 Not Found"
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        format!("http://{address}")
    }
}
