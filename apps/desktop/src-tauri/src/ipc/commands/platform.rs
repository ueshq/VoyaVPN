use super::*;

/// Enumerates running processes (and installed applications where the OS
/// makes that cheap) for the per-app proxy picker. Blocking OS enumeration
/// runs off the async runtime.
#[tauri::command]
#[specta::specta]
pub async fn list_process_candidates() -> Result<Vec<voya_contracts::ProcessCandidate>, AppError> {
    tauri::async_runtime::spawn_blocking(voya_platform::apps::list_process_candidates)
        .await
        .map(|candidates| {
            candidates
                .into_iter()
                .map(voya_app::contract_map::process_candidate_to_contract)
                .collect()
        })
        .map_err(|error| AppError::State(format!("process enumeration task failed: {error}")))
}
