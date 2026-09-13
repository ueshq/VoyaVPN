//! Apply a parsed import within the manager's database session.
use super::parse::parse_import_text;
use super::{Result, SubscriptionManager, SubscriptionManagerError};
use crate::policy_groups::PolicyGroupManager;
use crate::profiles::{normalize_profile, ProfileManager, ProfileManagerError};
use regex::Regex;
use std::collections::BTreeSet;
use voya_core::{profile_items_match, AppConfig, ImportProfilesResult, ProfileExItem, ProfileItem};

impl SubscriptionManager<'_> {
    pub async fn import_profiles_from_text(
        &self,
        config: &mut AppConfig,
        text: &str,
        subscription_id: Option<&str>,
    ) -> Result<ImportProfilesResult> {
        if let Some(id) = subscription_id.filter(|id| !id.trim().is_empty()) {
            return Err(ProfileManagerError::SubscriptionReadOnly(id.to_string()).into());
        }
        self.import_subscription_content(config, text, None).await
    }

    pub(super) async fn import_subscription_content(
        &self,
        config: &mut AppConfig,
        text: &str,
        subscription_id: Option<&str>,
    ) -> Result<ImportProfilesResult> {
        let subscription_id = subscription_id
            .map(str::trim)
            .filter(|value| !value.is_empty());
        // Resolve the target up front: `profile_items.subscription_id` has a
        // foreign key, so continuing with an unknown id would fail the upsert
        // with a raw "FOREIGN KEY constraint failed" instead of naming the
        // missing subscription.
        let filter = match subscription_id {
            Some(id) => {
                self.database
                    .subscriptions()
                    .get(id)
                    .await?
                    .ok_or_else(|| SubscriptionManagerError::SubscriptionNotFound(id.to_string()))?
                    .filter
            }
            None => None,
        };
        let regex = compile_filter(filter.as_deref())?;
        let old_profiles = if subscription_id.is_some() {
            self.database
                .profiles()
                .list_by_subscription_id(subscription_id)
                .await?
        } else {
            Vec::new()
        };

        let parsed_import = parse_import_text(text, subscription_id.unwrap_or_default())?;
        let mut added_subscription_ids: Vec<String> = Vec::new();
        for url in &parsed_import.subscription_urls {
            let subscription = self.add_subscription_from_url(url).await?;
            if !added_subscription_ids.contains(&subscription.id) {
                added_subscription_ids.push(subscription.id);
            }
        }
        let mut profiles = parsed_import.profiles;
        let parsed = profiles.len();
        let before_filter = profiles.len();
        if let Some(regex) = &regex {
            profiles.retain(|profile| regex.is_match(&profile.remarks));
        }
        let filtered = before_filter.saturating_sub(profiles.len());

        for profile in &mut profiles {
            profile.subscription_id = subscription_id.map(str::to_string);
            normalize_profile(profile);
        }

        let before_dedupe = profiles.len();
        profiles = dedupe_profiles(profiles);
        let deduped = before_dedupe.saturating_sub(profiles.len());
        let skipped = filtered
            .saturating_add(deduped)
            .saturating_add(parsed_import.failed_lines);
        if profiles.is_empty() {
            ProfileManager::from_session(self.database)
                .ensure_active_profile(config)
                .await?;
            return Ok(ImportProfilesResult {
                imported: 0,
                updated: 0,
                skipped: u32::try_from(skipped).unwrap_or(u32::MAX),
                parsed: u32::try_from(parsed).unwrap_or(u32::MAX),
                filtered: u32::try_from(filtered).unwrap_or(u32::MAX),
                deduped: u32::try_from(deduped).unwrap_or(u32::MAX),
                failed: u32::try_from(parsed_import.failed_lines).unwrap_or(u32::MAX),
                removed_existing: 0,
                removed_duplicates: 0,
                discarded_node_overrides: u32::try_from(parsed_import.discarded_node_overrides)
                    .unwrap_or(u32::MAX),
                subscription_id: subscription_id.map(str::to_string),
                imported_index_ids: Vec::new(),
                updated_index_ids: Vec::new(),
                line_issues: parsed_import.line_issues,
                added_subscription_ids,
            });
        }

        let profile_manager = ProfileManager::from_session(self.database);
        let mut existing_profiles = self
            .database
            .profiles()
            .list_with_profile_ex(None)
            .await?
            .items;
        let mut imported_index_ids = Vec::new();
        let mut updated_index_ids = Vec::new();
        let mut duplicate_index_ids_to_remove = Vec::new();
        for mut profile in profiles {
            let match_indices = existing_profiles
                .iter()
                .enumerate()
                .filter_map(|(index, (existing, _))| {
                    (existing.subscription_id.as_deref() == subscription_id
                        && profile_items_match(existing, &profile, false))
                    .then_some(index)
                })
                .collect::<Vec<_>>();

            if let Some(canonical_index) = choose_canonical_match_index(
                &match_indices,
                &existing_profiles,
                &config.index_id,
                subscription_id,
            ) {
                let canonical_index_id = existing_profiles[canonical_index].0.index_id.clone();
                let duplicate_index_ids = match_indices
                    .iter()
                    .filter_map(|index| {
                        let index_id = &existing_profiles[*index].0.index_id;
                        (index_id != &canonical_index_id).then(|| index_id.clone())
                    })
                    .collect::<Vec<_>>();

                profile.index_id.clone_from(&canonical_index_id);
                let saved = profile_manager
                    .save_imported_profile(config, profile)
                    .await?;
                update_existing_profile_cache(
                    &mut existing_profiles,
                    saved.profile.clone(),
                    saved.profile_ex.clone(),
                    &duplicate_index_ids,
                );
                duplicate_index_ids_to_remove.extend(duplicate_index_ids);
                updated_index_ids.push(saved.profile.index_id.clone());
                imported_index_ids.push(saved.profile.index_id.clone());
            } else {
                // External bundle IDs cannot overwrite another source's node.
                profile.index_id.clear();
                let saved = profile_manager
                    .save_imported_profile(config, profile)
                    .await?;
                existing_profiles.push((saved.profile.clone(), saved.profile_ex.clone()));
                imported_index_ids.push(saved.profile.index_id.clone());
            }
        }

        let removed_duplicates = if duplicate_index_ids_to_remove.is_empty() {
            0
        } else {
            self.database
                .profiles()
                .delete_many(&duplicate_index_ids_to_remove)
                .await?
        };

        let removed_existing = if let Some(id) = subscription_id {
            let retained_current_sub_index_ids: BTreeSet<&str> =
                imported_index_ids.iter().map(String::as_str).collect();
            let stale_index_ids = old_profiles
                .iter()
                .filter(|profile| {
                    profile.subscription_id.as_deref() == Some(id)
                        && !retained_current_sub_index_ids.contains(profile.index_id.as_str())
                })
                .map(|profile| profile.index_id.clone())
                .collect::<Vec<_>>();
            self.database
                .profiles()
                .delete_many(&stale_index_ids)
                .await?
        } else {
            0
        };

        let line_issues = parsed_import.line_issues;
        profile_manager.ensure_active_profile(config).await?;
        if let Some(id) = subscription_id {
            // Only the first import that brings nodes offers the group, so a
            // group the user deleted stays deleted across later updates.
            let first_import = !imported_index_ids.is_empty()
                && !old_profiles
                    .iter()
                    .any(|profile| profile.subscription_id.as_deref() == Some(id));
            if first_import {
                PolicyGroupManager::from_session(self.database)
                    .ensure_subscription_auto_group(config, id)
                    .await?;
            }
        }

        Ok(ImportProfilesResult {
            imported: u32::try_from(imported_index_ids.len()).unwrap_or(u32::MAX),
            updated: u32::try_from(updated_index_ids.len()).unwrap_or(u32::MAX),
            skipped: u32::try_from(skipped).unwrap_or(u32::MAX),
            parsed: u32::try_from(parsed).unwrap_or(u32::MAX),
            filtered: u32::try_from(filtered).unwrap_or(u32::MAX),
            deduped: u32::try_from(deduped).unwrap_or(u32::MAX),
            failed: u32::try_from(parsed_import.failed_lines).unwrap_or(u32::MAX),
            removed_existing: u32::try_from(removed_existing).unwrap_or(u32::MAX),
            removed_duplicates: u32::try_from(removed_duplicates).unwrap_or(u32::MAX),
            discarded_node_overrides: u32::try_from(parsed_import.discarded_node_overrides)
                .unwrap_or(u32::MAX),
            subscription_id: subscription_id.map(str::to_string),
            imported_index_ids,
            updated_index_ids,
            line_issues,
            added_subscription_ids,
        })
    }
}

pub(super) fn compile_filter(filter: Option<&str>) -> Result<Option<Regex>> {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    Regex::new(filter)
        .map(Some)
        .map_err(|error| SubscriptionManagerError::InvalidFilter(error.to_string()))
}

fn dedupe_profiles(profiles: Vec<ProfileItem>) -> Vec<ProfileItem> {
    let mut kept = Vec::<ProfileItem>::new();
    for profile in profiles {
        if kept
            .iter()
            .any(|existing| profile_items_match(existing, &profile, false))
        {
            continue;
        }
        kept.push(profile);
    }
    kept
}

fn choose_canonical_match_index(
    match_indices: &[usize],
    existing_profiles: &[(ProfileItem, ProfileExItem)],
    active_index_id: &str,
    target_subscription_id: Option<&str>,
) -> Option<usize> {
    match_indices.iter().copied().min_by_key(|index| {
        let (profile, profile_ex) = &existing_profiles[*index];
        let active_rank = if !active_index_id.is_empty() && profile.index_id == active_index_id {
            0
        } else {
            1
        };
        let target_subscription_id_rank = if target_subscription_id.is_some_and(|subscription_id| {
            profile.subscription_id.as_deref() == Some(subscription_id)
        }) {
            0
        } else {
            1
        };

        (
            active_rank,
            target_subscription_id_rank,
            profile_ex.sort,
            *index,
        )
    })
}

fn update_existing_profile_cache(
    existing_profiles: &mut Vec<(ProfileItem, ProfileExItem)>,
    saved_profile: ProfileItem,
    saved_profile_ex: ProfileExItem,
    removed_index_ids: &[String],
) {
    let saved_index_id = saved_profile.index_id.clone();
    existing_profiles.retain(|(profile, _)| {
        profile.index_id == saved_index_id
            || !removed_index_ids
                .iter()
                .any(|index_id| index_id == &profile.index_id)
    });

    if let Some((profile, profile_ex)) = existing_profiles
        .iter_mut()
        .find(|(profile, _)| profile.index_id == saved_index_id)
    {
        *profile = saved_profile;
        *profile_ex = saved_profile_ex;
    } else {
        existing_profiles.push((saved_profile, saved_profile_ex));
    }
}
