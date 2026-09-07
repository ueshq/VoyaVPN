use thiserror::Error;
pub use voya_contracts::{
    ConfigTemplateImportOptions, ConfigTemplateImportResult, ConfigTemplateSelection,
};
use voya_core::{AppConfig, SimpleDnsDefaults, SimpleDnsItem};
use voya_db::{Database, DatabaseSession, DbError, UnitOfWork};

use crate::{
    routing::{manager::PreparedRoutingTemplate, RoutingManager, RoutingManagerError},
    updates::{
        apply_source_settings, validate_asset_source_urls, ConfigSourceSettings, UpdateManagerError,
    },
};

pub type Result<T> = std::result::Result<T, PresetManagerError>;

#[derive(Debug, Error)]
pub enum PresetManagerError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error(transparent)]
    Routing(#[from] RoutingManagerError),
    #[error(transparent)]
    Source(#[from] UpdateManagerError),
    #[error("custom configuration template requires a routing template source URL")]
    MissingRoutingTemplateSource,
}

#[derive(Debug, Clone)]
pub struct PresetManager<'db> {
    database: DatabaseSession<'db>,
}

impl<'db> PresetManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self::from_session(DatabaseSession::from_database(database))
    }

    #[must_use]
    pub fn new_in(unit_of_work: &'db UnitOfWork) -> Self {
        Self::from_session(DatabaseSession::from_unit_of_work(unit_of_work))
    }

    #[must_use]
    const fn from_session(database: DatabaseSession<'db>) -> Self {
        Self { database }
    }

    pub async fn import_config_template(
        &self,
        config: &mut AppConfig,
        selection: ConfigTemplateSelection,
        options: ConfigTemplateImportOptions,
    ) -> Result<ConfigTemplateImportResult> {
        let prepared = self
            .prepare_config_template_import(selection, &options)
            .await?;
        self.apply_prepared_config_template_import(config, prepared)
            .await
    }

    pub async fn prepare_config_template_import(
        &self,
        selection: ConfigTemplateSelection,
        options: &ConfigTemplateImportOptions,
    ) -> Result<PreparedConfigTemplateImport> {
        let routing_manager = RoutingManager::from_session(self.database);
        self.prepare_import(&routing_manager, selection, options)
            .await
    }

    pub async fn apply_prepared_config_template_import(
        &self,
        config: &mut AppConfig,
        prepared: PreparedConfigTemplateImport,
    ) -> Result<ConfigTemplateImportResult> {
        let routing_manager = RoutingManager::from_session(self.database);
        let mut next_config = config.clone();
        let normalized_sources = apply_source_settings(&mut next_config, prepared.sources.clone());
        if let Some(simple_dns) = prepared.simple_dns {
            next_config.simple_dns_item = simple_dns;
        }

        let routing_result = routing_manager
            .apply_prepared_config_template(&mut next_config, prepared.routing)
            .await?;

        *config = next_config;
        Ok(ConfigTemplateImportResult {
            sources: normalized_sources,
            routing_ids: routing_result.routing_ids,
            active_routing_id: routing_result.active_routing_id,
            reused_existing_routing: routing_result.reused_existing_routing,
        })
    }

    async fn prepare_import(
        &self,
        routing: &RoutingManager<'_>,
        selection: ConfigTemplateSelection,
        options: &ConfigTemplateImportOptions,
    ) -> Result<PreparedConfigTemplateImport> {
        match selection {
            ConfigTemplateSelection::Default => Ok(PreparedConfigTemplateImport {
                sources: ConfigSourceSettings::default(),
                routing: routing.prepare_builtin_config_template(),
                simple_dns: Some(SimpleDnsDefaults::builtin()),
            }),
            ConfigTemplateSelection::Custom { sources } => {
                let mut normalized_config = AppConfig::default();
                let sources = apply_source_settings(&mut normalized_config, sources);
                validate_asset_source_urls(&sources).map_err(UpdateManagerError::from)?;
                let source_url = sources
                    .route_rules_template_source_url
                    .as_deref()
                    .ok_or(PresetManagerError::MissingRoutingTemplateSource)?;
                let routing = routing
                    .prepare_external_config_template(
                        source_url,
                        options.prefer_proxy,
                        options.proxy_url.as_deref(),
                    )
                    .await?;
                Ok(PreparedConfigTemplateImport {
                    sources,
                    routing,
                    simple_dns: None,
                })
            }
        }
    }
}

pub struct PreparedConfigTemplateImport {
    sources: ConfigSourceSettings,
    routing: PreparedRoutingTemplate,
    simple_dns: Option<SimpleDnsItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_template_applies_structured_dns_and_routing_only() {
        let database = Database::connect_in_memory()
            .await
            .expect("preset manager test operation should succeed");
        let mut config = AppConfig::default();
        config.simple_dns_item.direct_dns = Some("1.1.1.1".to_string());

        let result = PresetManager::new(&database)
            .import_config_template(
                &mut config,
                ConfigTemplateSelection::Default,
                ConfigTemplateImportOptions::default(),
            )
            .await
            .expect("preset manager test operation should succeed");

        assert_eq!(result.routing_ids.len(), 3);
        assert_eq!(
            config.simple_dns_item.direct_dns,
            SimpleDnsDefaults::builtin().direct_dns
        );
    }

    #[tokio::test]
    async fn failed_custom_template_preparation_does_not_modify_data() {
        let database = Database::connect_in_memory()
            .await
            .expect("preset database should connect");
        let manager = PresetManager::new(&database);
        let config = AppConfig::default();
        let original = config.clone();

        let result = manager
            .prepare_config_template_import(
                ConfigTemplateSelection::Custom {
                    sources: ConfigSourceSettings {
                        geo_source_url: Some("https://geo.example.test/{0}.dat".to_string()),
                        srs_source_url: None,
                        route_rules_template_source_url: None,
                    },
                },
                &ConfigTemplateImportOptions::default(),
            )
            .await;

        assert!(matches!(
            result,
            Err(PresetManagerError::MissingRoutingTemplateSource)
        ));
        assert_eq!(config, original);
        assert!(database
            .routings()
            .list()
            .await
            .expect("routings should load")
            .is_empty());
    }
}
