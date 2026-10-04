//! The contract-typed node use cases both hosts share: each checks its
//! arguments, runs the read or the mutation and returns the public DTO.

use voya_contracts::{AppError, AppErrorSubsystem, MoveAction, Profile, ProfileDetails};
use voya_core::AppConfig;

use super::ProfileManager;
use crate::{
    config_mutation::{CommittedMutation, ConfigMutationCoordinator},
    contract_map::{profile_details_to_contract, profile_from_contract},
    input_safety::{require_id, require_ids},
    services::AppServices,
};

/// One node in full, for the editor and the details view.
pub async fn get_profile_use_case(
    services: &AppServices,
    config: &AppConfig,
    index_id: &str,
) -> Result<ProfileDetails, AppError> {
    require_id(index_id, "node id", AppErrorSubsystem::Profile)?;

    Ok(profile_details_to_contract(
        services.profiles().get_profile(config, index_id).await?,
    ))
}

/// Saves a manual node.
pub async fn save_profile_use_case(
    mutations: &ConfigMutationCoordinator,
    profile: Profile,
) -> Result<CommittedMutation<ProfileDetails>, AppError> {
    mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(profile_details_to_contract(
                ProfileManager::new_in(unit_of_work)
                    .save_profile(config, profile_from_contract(profile))
                    .await?,
            ))
        })
        .await
}

/// Deletes manual nodes, which may include the running one.
pub async fn delete_profiles_use_case(
    mutations: &ConfigMutationCoordinator,
    index_ids: Vec<String>,
) -> Result<CommittedMutation<u32>, AppError> {
    require_ids(&index_ids, "node id", AppErrorSubsystem::Profile)?;
    mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            let deleted = ProfileManager::new_in(unit_of_work)
                .delete_profiles(config, &index_ids)
                .await?;
            Ok(u32::try_from(deleted).unwrap_or(u32::MAX))
        })
        .await
}

/// Makes a node the one connecting uses.
pub async fn set_active_profile_use_case(
    mutations: &ConfigMutationCoordinator,
    index_id: String,
) -> Result<CommittedMutation<ProfileDetails>, AppError> {
    require_id(&index_id, "node id", AppErrorSubsystem::Profile)?;
    mutations
        .mutate(async |unit_of_work, config| -> Result<_, AppError> {
            Ok(profile_details_to_contract(
                ProfileManager::new_in(unit_of_work)
                    .set_active_profile(config, &index_id)
                    .await?,
            ))
        })
        .await
}

/// Moves a manual node among the manual nodes.
pub async fn move_profile_use_case(
    mutations: &ConfigMutationCoordinator,
    index_id: String,
    action: MoveAction,
    position: Option<i32>,
) -> Result<(), AppError> {
    require_id(&index_id, "node id", AppErrorSubsystem::Profile)?;
    mutations
        .mutate(async |unit_of_work, _config| -> Result<_, AppError> {
            Ok(ProfileManager::new_in(unit_of_work)
                .move_profile(&index_id, action, position)
                .await?)
        })
        .await?;
    Ok(())
}
