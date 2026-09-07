use std::collections::BTreeMap;

use thiserror::Error;
pub use voya_contracts::{ConfigSourceSettings, ResourceUpdateFile};
use voya_core::{AppConfig, RoutingItem};
use voya_db::{Database, DbError};
use voya_net::ruleset::{
    collect_singbox_ruleset_assets, discover_local_singbox_ruleset_paths, geo_assets,
    AcquiredRulesetGeoAsset, AssetAcquisitionOptions, RulesetGeoClient, RulesetGeoError, SrsAsset,
};
use voya_platform::paths::AppPaths;

pub type Result<T> = std::result::Result<T, UpdateManagerError>;

#[derive(Debug, Error)]
pub enum UpdateManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    RulesetGeo(#[from] RulesetGeoError),
    #[error("invalid {label}: {reason}")]
    InvalidSourceUrl {
        label: &'static str,
        reason: &'static str,
    },
}

#[derive(Debug, Clone)]
pub struct UpdateManager<'db> {
    database: &'db Database,
    paths: AppPaths,
    ruleset_geo: RulesetGeoClient,
}

impl<'db> UpdateManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database, paths: AppPaths) -> Self {
        Self {
            database,
            paths,
            ruleset_geo: RulesetGeoClient::new(),
        }
    }

    pub async fn update_geo_assets(
        &self,
        config: &AppConfig,
        proxy_url: Option<String>,
    ) -> Result<Vec<ResourceUpdateFile>> {
        let assets = geo_assets(config.const_item.geo_source_url.as_deref());
        let acquired = self
            .ruleset_geo
            .acquire_geo_assets(
                &assets,
                self.paths.bin_dir(),
                &asset_acquisition_options(proxy_url),
            )
            .await?;

        Ok(acquired.into_iter().map(resource_update_file).collect())
    }

    pub async fn update_srs_assets(
        &self,
        config: &AppConfig,
        proxy_url: Option<String>,
    ) -> Result<Vec<ResourceUpdateFile>> {
        let routings = self.database.routings().list().await?;
        let assets = collect_srs_assets(config, &routings);
        let srs_dir = self.paths.bin_dir().join("srss");
        let acquired = self
            .ruleset_geo
            .acquire_srs_assets(&assets, srs_dir, &asset_acquisition_options(proxy_url))
            .await?;

        Ok(acquired.into_iter().map(resource_update_file).collect())
    }
}

#[must_use]
pub fn source_settings(config: &AppConfig) -> ConfigSourceSettings {
    ConfigSourceSettings {
        geo_source_url: config.const_item.geo_source_url.clone(),
        srs_source_url: config.const_item.srs_source_url.clone(),
        route_rules_template_source_url: config.const_item.route_rules_template_source_url.clone(),
    }
}

pub fn apply_source_settings(
    config: &mut AppConfig,
    settings: ConfigSourceSettings,
) -> ConfigSourceSettings {
    config.const_item.geo_source_url = normalize_optional_url(settings.geo_source_url);
    config.const_item.srs_source_url = normalize_optional_url(settings.srs_source_url);
    config.const_item.route_rules_template_source_url =
        normalize_optional_url(settings.route_rules_template_source_url);
    source_settings(config)
}

pub fn validate_optional_source_url(label: &'static str, value: Option<&str>) -> Result<()> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    voya_net::validate_absolute_http_url(value).map_err(|error| invalid_source_url(label, error))
}

pub fn validate_optional_https_source_url(label: &'static str, value: Option<&str>) -> Result<()> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    voya_net::validate_absolute_https_url(value).map_err(|error| invalid_source_url(label, error))
}

/// Geo databases, sing-box rule sets and routing templates all decide which
/// traffic bypasses the proxy, and the downloaded bytes are consumed by the
/// core without a signature check. A network attacker able to rewrite a plain
/// HTTP response could therefore redirect or leak arbitrary traffic, so every
/// asset source must be HTTPS.
pub fn validate_asset_source_urls(sources: &ConfigSourceSettings) -> Result<()> {
    validate_optional_https_source_url("Geo source URL", sources.geo_source_url.as_deref())?;
    validate_optional_https_source_url("SRS source URL", sources.srs_source_url.as_deref())?;
    validate_optional_https_source_url(
        "routing template source URL",
        sources.route_rules_template_source_url.as_deref(),
    )
}

const fn invalid_source_url(
    label: &'static str,
    error: voya_net::UrlValidationError,
) -> UpdateManagerError {
    UpdateManagerError::InvalidSourceUrl {
        label,
        reason: match error {
            voya_net::UrlValidationError::InvalidHttpUrl => {
                "expected an absolute HTTP or HTTPS URL"
            }
            voya_net::UrlValidationError::InvalidHttpsUrl => "expected an absolute HTTPS URL",
            voya_net::UrlValidationError::EmbeddedCredentials => {
                "embedded credentials are not allowed"
            }
        },
    }
}

#[must_use]
pub fn local_singbox_ruleset_paths(paths: &AppPaths) -> BTreeMap<String, String> {
    discover_local_singbox_ruleset_paths(paths.bin_dir().join("srss"))
}

#[must_use]
pub fn collect_srs_assets(config: &AppConfig, routings: &[RoutingItem]) -> Vec<SrsAsset> {
    collect_singbox_ruleset_assets(config.const_item.srs_source_url.as_deref(), routings)
}

fn asset_acquisition_options(proxy_url: Option<String>) -> AssetAcquisitionOptions {
    AssetAcquisitionOptions {
        prefer_proxy: true,
        proxy_url,
    }
}

fn resource_update_file(asset: AcquiredRulesetGeoAsset) -> ResourceUpdateFile {
    ResourceUpdateFile {
        name: asset.file_name,
        bytes: u32::try_from(asset.bytes).unwrap_or(u32::MAX),
        used_proxy: asset.used_proxy,
    }
}

fn normalize_optional_url(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_settings_trim_optional_urls() {
        let mut config = AppConfig::default();
        let saved = apply_source_settings(
            &mut config,
            ConfigSourceSettings {
                geo_source_url: Some(" https://example.com/geo.zip ".to_string()),
                srs_source_url: Some("  ".to_string()),
                route_rules_template_source_url: Some(
                    "https://example.com/routing.json".to_string(),
                ),
            },
        );

        assert_eq!(
            saved.geo_source_url.as_deref(),
            Some("https://example.com/geo.zip")
        );
        assert_eq!(saved.srs_source_url, None);
        assert_eq!(
            saved.route_rules_template_source_url.as_deref(),
            Some("https://example.com/routing.json")
        );
    }

    #[test]
    fn source_url_validation_accepts_templates_and_rejects_unsafe_shapes() {
        validate_optional_source_url("Geo source URL", Some("https://example.com/geo/{0}.dat"))
            .expect("template URL should be valid");
        validate_optional_source_url(
            "subscription converter URL",
            Some("http://localhost:25500/sub"),
        )
        .expect("local converter URL should be valid");

        assert!(validate_optional_source_url("source URL", Some("../rules.json")).is_err());
        assert!(
            validate_optional_source_url("source URL", Some("ftp://example.com/rules")).is_err()
        );
        assert!(validate_optional_source_url(
            "source URL",
            Some("https://user:secret@example.com/rules")
        )
        .is_err());

        validate_optional_https_source_url(
            "routing template source URL",
            Some("https://example.com/routing.json"),
        )
        .expect("HTTPS routing source should be valid");
        assert!(validate_optional_https_source_url(
            "routing template source URL",
            Some("http://example.com/routing.json")
        )
        .is_err());
    }

    #[test]
    fn asset_source_validation_requires_https_for_every_source() {
        validate_asset_source_urls(&ConfigSourceSettings {
            geo_source_url: Some("https://example.com/geo/{0}.dat".to_string()),
            srs_source_url: Some("https://example.com/rules/{0}.srs".to_string()),
            route_rules_template_source_url: Some("https://example.com/routing.json".to_string()),
        })
        .expect("HTTPS asset sources should be accepted");

        for sources in [
            ConfigSourceSettings {
                geo_source_url: Some("http://example.com/geo/{0}.dat".to_string()),
                ..ConfigSourceSettings::default()
            },
            ConfigSourceSettings {
                srs_source_url: Some("http://example.com/rules/{0}.srs".to_string()),
                ..ConfigSourceSettings::default()
            },
            ConfigSourceSettings {
                route_rules_template_source_url: Some(
                    "http://example.com/routing.json".to_string(),
                ),
                ..ConfigSourceSettings::default()
            },
        ] {
            assert!(
                matches!(
                    validate_asset_source_urls(&sources),
                    Err(UpdateManagerError::InvalidSourceUrl { .. })
                ),
                "plain HTTP asset sources must be rejected: {sources:?}"
            );
        }

        validate_asset_source_urls(&ConfigSourceSettings::default())
            .expect("unset asset sources fall back to the built-in defaults");
    }

    #[test]
    fn resource_updates_always_prefer_the_runtime_proxy() {
        let options = asset_acquisition_options(Some("http://127.0.0.1:10808".to_string()));

        assert!(options.prefer_proxy);
        assert_eq!(options.proxy_url.as_deref(), Some("http://127.0.0.1:10808"));
    }

    #[test]
    fn resource_update_result_keeps_file_and_proxy_metadata() {
        let result = resource_update_file(AcquiredRulesetGeoAsset {
            kind: voya_net::ruleset::AcquiredAssetKind::Geo,
            name: "geoip".to_string(),
            file_name: "geoip.dat".to_string(),
            url: "https://example.com/geoip.dat".to_string(),
            path: "bin/geoip.dat".into(),
            bytes: u64::MAX,
            used_proxy: true,
            attempts: Vec::new(),
        });

        assert_eq!(result.name, "geoip.dat");
        assert_eq!(result.bytes, u32::MAX);
        assert!(result.used_proxy);
    }
}
