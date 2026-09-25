//! Apply a parsed import within the manager's database session.
use super::parse::parse_import_text;
use super::{Result, SubscriptionManager, SubscriptionManagerError};
use crate::policy_groups::PolicyGroupManager;
use crate::profiles::{normalize_profile, NewProfileSort, ProfileManager, ProfileManagerError};
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use voya_contracts::ImportProfilesResult;
use voya_core::{profile_items_match, AppConfig, ProfileExItem, ProfileItem};

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
                imported_profile_ids: Vec::new(),
                updated_profile_ids: Vec::new(),
                line_issues: parsed_import.line_issues,
                added_subscription_ids,
            });
        }

        let profile_manager = ProfileManager::from_session(self.database);
        let mut existing_profiles = ExistingProfiles::new(
            self.database
                .profiles()
                .list_with_profile_ex(None)
                .await?
                .items,
        );
        let mut imported_profile_ids = Vec::new();
        let mut updated_profile_ids = Vec::new();
        let mut duplicate_index_ids_to_remove = Vec::new();
        let mut new_sort = NewProfileSort::default();
        for mut profile in profiles {
            let match_indices = existing_profiles.matches(&profile, subscription_id);

            if let Some(canonical_index_id) = choose_canonical_match_index(
                &match_indices,
                &existing_profiles,
                &config.index_id,
                subscription_id,
            ) {
                let duplicate_index_ids = match_indices
                    .iter()
                    .filter_map(|index| {
                        let index_id = &existing_profiles.entry(*index)?.0.index_id;
                        (*index_id != canonical_index_id).then(|| index_id.clone())
                    })
                    .collect::<Vec<_>>();

                profile.index_id.clone_from(&canonical_index_id);
                let stored = existing_profiles
                    .get(&canonical_index_id)
                    .map(|(stored, stored_ex)| (stored, stored_ex));
                let (saved, saved_ex) = profile_manager
                    .write_imported_profile(profile, stored, &mut new_sort)
                    .await?;
                updated_profile_ids.push(saved.index_id.clone());
                imported_profile_ids.push(saved.index_id.clone());
                existing_profiles.store(saved, saved_ex, &duplicate_index_ids);
                duplicate_index_ids_to_remove.extend(duplicate_index_ids);
            } else {
                // External bundle IDs cannot overwrite another source's node.
                profile.index_id.clear();
                let (saved, saved_ex) = profile_manager
                    .write_imported_profile(profile, None, &mut new_sort)
                    .await?;
                imported_profile_ids.push(saved.index_id.clone());
                existing_profiles.store(saved, saved_ex, &[]);
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
                imported_profile_ids.iter().map(String::as_str).collect();
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
            let first_import = !imported_profile_ids.is_empty()
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
            imported: u32::try_from(imported_profile_ids.len()).unwrap_or(u32::MAX),
            updated: u32::try_from(updated_profile_ids.len()).unwrap_or(u32::MAX),
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
            imported_profile_ids,
            updated_profile_ids,
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

/// The part of a profile that `profile_items_match` implies: equal protocols
/// share a server address and port. Bucketing on it keeps matching linear in
/// the import size; the full predicate still decides within a bucket.
type EndpointKey = (String, i32);

fn endpoint_key(profile: &ProfileItem) -> EndpointKey {
    (profile.address().to_owned(), profile.port())
}

fn dedupe_profiles(profiles: Vec<ProfileItem>) -> Vec<ProfileItem> {
    let mut kept = Vec::<ProfileItem>::new();
    let mut kept_by_endpoint = HashMap::<EndpointKey, Vec<usize>>::new();
    for profile in profiles {
        let bucket = kept_by_endpoint.entry(endpoint_key(&profile)).or_default();
        if bucket
            .iter()
            .any(|index| profile_items_match(&kept[*index], &profile, false))
        {
            continue;
        }
        bucket.push(kept.len());
        kept.push(profile);
    }
    kept
}

/// Stored profiles an import matches against.
///
/// A linear scan with `profile_items_match` per parsed node made a large
/// subscription update quadratic, so entries are indexed by endpoint. A
/// replaced duplicate leaves a hole instead of shifting later entries: the
/// position is the final canonical tie-breaker, and holes keep its order.
struct ExistingProfiles {
    entries: Vec<Option<(ProfileItem, ProfileExItem)>>,
    by_endpoint: HashMap<EndpointKey, Vec<usize>>,
    by_index_id: HashMap<String, usize>,
}

impl ExistingProfiles {
    fn new(items: Vec<(ProfileItem, ProfileExItem)>) -> Self {
        let mut existing = Self {
            entries: Vec::with_capacity(items.len()),
            by_endpoint: HashMap::new(),
            by_index_id: HashMap::with_capacity(items.len()),
        };
        for (profile, profile_ex) in items {
            existing.push(profile, profile_ex);
        }
        existing
    }

    fn push(&mut self, profile: ProfileItem, profile_ex: ProfileExItem) {
        let index = self.entries.len();
        self.by_endpoint
            .entry(endpoint_key(&profile))
            .or_default()
            .push(index);
        self.by_index_id.insert(profile.index_id.clone(), index);
        self.entries.push(Some((profile, profile_ex)));
    }

    /// Positions of stored profiles in `subscription_id` matching `profile`,
    /// ascending.
    fn matches(&self, profile: &ProfileItem, subscription_id: Option<&str>) -> Vec<usize> {
        let Some(bucket) = self.by_endpoint.get(&endpoint_key(profile)) else {
            return Vec::new();
        };
        bucket
            .iter()
            .copied()
            .filter(|index| {
                self.entries[*index].as_ref().is_some_and(|(existing, _)| {
                    existing.subscription_id.as_deref() == subscription_id
                        && profile_items_match(existing, profile, false)
                })
            })
            .collect()
    }

    /// The live entry at `index`; `None` once it was removed.
    fn entry(&self, index: usize) -> Option<&(ProfileItem, ProfileExItem)> {
        self.entries.get(index)?.as_ref()
    }

    /// The live entry stored under `index_id`. It is what the database holds
    /// for that row inside the import's transaction, because every write goes
    /// through [`Self::store`] as well.
    fn get(&self, index_id: &str) -> Option<&(ProfileItem, ProfileExItem)> {
        self.entry(*self.by_index_id.get(index_id)?)
    }

    /// Records a saved profile, replacing its stored copy, and drops the
    /// duplicates it absorbed.
    fn store(
        &mut self,
        saved_profile: ProfileItem,
        saved_profile_ex: ProfileExItem,
        removed_index_ids: &[String],
    ) {
        for index_id in removed_index_ids {
            if *index_id == saved_profile.index_id {
                continue;
            }
            if let Some(index) = self.by_index_id.remove(index_id) {
                self.remove_from_bucket(index);
                self.entries[index] = None;
            }
        }

        let Some(&index) = self.by_index_id.get(&saved_profile.index_id) else {
            self.push(saved_profile, saved_profile_ex);
            return;
        };
        let new_key = endpoint_key(&saved_profile);
        let moved = self.entries[index]
            .as_ref()
            .is_some_and(|(stored, _)| endpoint_key(stored) != new_key);
        if moved {
            self.remove_from_bucket(index);
            let bucket = self.by_endpoint.entry(new_key).or_default();
            let position = bucket.partition_point(|existing| *existing < index);
            bucket.insert(position, index);
        }
        self.entries[index] = Some((saved_profile, saved_profile_ex));
    }

    fn remove_from_bucket(&mut self, index: usize) {
        let Some((stored, _)) = &self.entries[index] else {
            return;
        };
        if let Some(bucket) = self.by_endpoint.get_mut(&endpoint_key(stored)) {
            bucket.retain(|existing| *existing != index);
        }
    }
}

/// The index id of the stored match the import updates in place.
fn choose_canonical_match_index(
    match_indices: &[usize],
    existing_profiles: &ExistingProfiles,
    active_index_id: &str,
    target_subscription_id: Option<&str>,
) -> Option<String> {
    let entries = match_indices
        .iter()
        .filter_map(|index| Some((*index, existing_profiles.entry(*index)?)));
    let (_, (canonical, _)) = entries.min_by_key(|(index, (profile, profile_ex))| {
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
    })?;
    Some(canonical.index_id.clone())
}
