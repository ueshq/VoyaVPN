//! The handshake the tunnel provider is started with.
//!
//! The provider runs in a process of its own with a sandbox of its own, so it
//! cannot read the app container: the payload carries the whole sing-box
//! configuration inline, and any rule set the config points at by absolute path
//! is copied into the shared container first and its path rewritten.
//!
//! It is pure JSON and file handling with no framework in it, so it is written
//! once for every host that runs the core inside a provider: the macOS bridge
//! and the mobile host each say where their container keeps things
//! ([`HandoffPaths`]) and pass the text on.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::{json, Map, Value};

use super::NativeTunStartRequest;

/// Where the provider's files live in the container both processes can reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffPaths {
    /// The status file the provider writes.
    pub status: PathBuf,
    /// The provider's own log.
    pub log: PathBuf,
    /// The directory local rule sets are copied into.
    pub staging: PathBuf,
}

/// The payload version. The provider refuses a version it does not know, so
/// this changes only when a field's meaning does.
const HANDOFF_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum HandoffError {
    #[error("could not read the generated config at {path}: {source}")]
    ReadConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the generated config at {path} is not a JSON object")]
    InvalidConfig { path: PathBuf },
    #[error("could not prepare the shared directory {path}: {source}")]
    Staging {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not stage the rule set {path}: {source}")]
    StageRuleSet {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not encode the handshake: {0}")]
    Encode(#[from] serde_json::Error),
}

/// Builds the handshake for a start request, staging local rule sets into
/// `paths.staging` on the way.
///
/// `singboxConfigJson` is the config *text*, not an object: the provider hands
/// it to Libbox verbatim, and re-encoding it here would be one more chance to
/// change it.
pub fn build_handoff(
    request: &NativeTunStartRequest,
    paths: &HandoffPaths,
) -> Result<String, HandoffError> {
    let config_text = fs::read_to_string(&request.main_config_path).map_err(|source| {
        HandoffError::ReadConfig {
            path: request.main_config_path.clone(),
            source,
        }
    })?;
    let staged = stage_local_rule_sets(&config_text, &request.main_config_path, &paths.staging)?;

    Ok(serde_json::to_string(&json!({
        "version": HANDOFF_VERSION,
        "activeProfileId": request.active_profile_id,
        "mainConfigPath": request.main_config_path.display().to_string(),
        "statusPath": paths.status.display().to_string(),
        "logPath": paths.log.display().to_string(),
        "singboxConfigJson": staged,
    }))?)
}

/// Copies every `type: "local"` rule set into `staging` and rewrites its path.
///
/// Returns the configuration text unchanged when there is nothing to rewrite,
/// which is the common case: a config whose rule sets are all remote, or none.
fn stage_local_rule_sets(
    config_text: &str,
    config_path: &Path,
    staging: &Path,
) -> Result<String, HandoffError> {
    let mut config: Value =
        serde_json::from_str(config_text).map_err(|_| HandoffError::InvalidConfig {
            path: config_path.to_path_buf(),
        })?;
    let Some(rule_sets) = config
        .get_mut("route")
        .and_then(|route| route.get_mut("rule_set"))
        .and_then(Value::as_array_mut)
    else {
        return Ok(config_text.to_string());
    };

    let mut rewritten = false;
    for entry in rule_sets.iter_mut() {
        let Some(entry) = entry.as_object_mut() else {
            continue;
        };
        let Some(source) = local_rule_set_path(entry) else {
            continue;
        };
        let Some(file_name) = source.file_name() else {
            continue;
        };

        if !rewritten {
            fs::create_dir_all(staging).map_err(|source| HandoffError::Staging {
                path: staging.to_path_buf(),
                source,
            })?;
        }
        let destination = staging.join(file_name);
        fs::copy(&source, &destination).map_err(|error| HandoffError::StageRuleSet {
            path: source.clone(),
            source: error,
        })?;
        entry.insert(
            "path".to_string(),
            Value::String(destination.display().to_string()),
        );
        rewritten = true;
    }

    if rewritten {
        Ok(serde_json::to_string(&config)?)
    } else {
        Ok(config_text.to_string())
    }
}

/// The absolute path of a `type: "local"` rule set, if this entry is one.
///
/// A relative path is left alone: sing-box resolves it against its own working
/// directory, which is the provider's, so rewriting it would break it.
fn local_rule_set_path(entry: &Map<String, Value>) -> Option<PathBuf> {
    if entry.get("type").and_then(Value::as_str) != Some("local") {
        return None;
    }
    let path = entry.get("path").and_then(Value::as_str)?;

    Path::new(path).is_absolute().then(|| PathBuf::from(path))
}

#[cfg(test)]
mod tests;
