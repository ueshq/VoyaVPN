//! Fetch outside the configuration lock, then apply only unchanged source snapshots.
use super::super::update_flow::{
    persist_subscription_metadata, prepare_subscription_snapshot, PreparedSubscriptionUpdate,
};
use super::{Result, SubscriptionManager, SubscriptionManagerError};
use voya_core::{AppConfig, SubscriptionUpdateResult};

impl SubscriptionManager<'_> {
    pub async fn update_subscriptions(
        &self,
        config: &mut AppConfig,
        subscription_id: Option<&str>,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
    ) -> Result<SubscriptionUpdateResult> {
        let prepared = self
            .prepare_subscription_update(subscription_id, prefer_proxy, proxy_url)
            .await?;
        self.apply_prepared_subscription_update(config, prepared)
            .await
    }

    pub async fn prepare_subscription_update(
        &self,
        subscription_id: Option<&str>,
        prefer_proxy: bool,
        proxy_url: Option<&str>,
    ) -> Result<PreparedSubscriptionUpdate> {
        let subscriptions = self.database.subscriptions().list().await?;
        prepare_subscription_snapshot(subscriptions, subscription_id, prefer_proxy, proxy_url).await
    }

    pub async fn apply_prepared_subscription_update(
        &self,
        config: &mut AppConfig,
        prepared: PreparedSubscriptionUpdate,
    ) -> Result<SubscriptionUpdateResult> {
        let mut result = prepared.result;
        for prepared_import in prepared.imports {
            let current = self
                .database
                .subscriptions()
                .get(&prepared_import.item.id)
                .await?;
            if current.as_ref() != Some(&prepared_import.item) {
                result.skipped = result.skipped.saturating_add(1);
                result.messages.push(format!(
                    "{}->subscription changed while the update was downloading; prepared content was discarded",
                    prepared_import.item.remarks
                ));
                continue;
            }
            // The import runs first so the recorded `last_update_at` only
            // advances for a subscription that actually produced profiles.
            let import = self
                .import_subscription_content(
                    config,
                    &prepared_import.content,
                    Some(&prepared_import.item.id),
                )
                .await;
            match import {
                Ok(import) if import.imported > 0 => {
                    persist_subscription_metadata(self.database, &prepared_import, true).await?;
                    result.updated = result.updated.saturating_add(1);
                    result.imported = result.imported.saturating_add(import.imported);
                    result.removed_existing = result
                        .removed_existing
                        .saturating_add(import.removed_existing);
                    result.messages.push(format!(
                        "{}->imported {} nodes",
                        prepared_import.item.remarks, import.imported
                    ));
                }
                Ok(_) => {
                    persist_subscription_metadata(self.database, &prepared_import, false).await?;
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!(
                        "{}->no nodes were imported",
                        prepared_import.item.remarks
                    ));
                }
                Err(SubscriptionManagerError::NoImportableProfiles) => {
                    persist_subscription_metadata(self.database, &prepared_import, false).await?;
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!(
                        "{}->no importable nodes were found",
                        prepared_import.item.remarks
                    ));
                }
                // A bad filter belongs to one subscription; failing the whole
                // batch would roll back every sibling's successful import.
                Err(SubscriptionManagerError::InvalidFilter(reason)) => {
                    persist_subscription_metadata(self.database, &prepared_import, false).await?;
                    result.skipped = result.skipped.saturating_add(1);
                    result.messages.push(format!(
                        "{}->subscription filter is invalid: {reason}",
                        prepared_import.item.remarks
                    ));
                }
                Err(error) => return Err(error),
            }
        }

        Ok(result)
    }
}
