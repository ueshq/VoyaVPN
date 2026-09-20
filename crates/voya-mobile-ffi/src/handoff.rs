//! The handshake the tunnel provider is started with.
//!
//! The provider runs in a process of its own with a sandbox of its own, so it
//! cannot read the app container: the payload carries the whole sing-box
//! configuration inline, and any rule set the config points at by absolute path
//! is copied into the shared container first and its path rewritten.
//!
//! macOS does the same thing in Objective-C
//! (`crates/voya-platform/native/macos_packet_tunnel_bridge.m`). This is the
//! portable half — pure JSON handling with no framework in it — written once
//! for iOS and Android, which is why the shape below matches that file's
//! `runtimeConfig` dictionary field for field.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{Map, Value};
use voya_platform::tun::NativeTunStartRequest;

/// Where staged rule sets and the provider's own files live, relative to the
/// shared container the host passes in.
const STAGING_DIR: &str = "srss";
const STATUS_FILE: &str = "PT/status.json";
const LOG_FILE: &str = "PT/provider.log";

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

/// The dictionary the provider reads on start.
///
/// `singboxConfigJson` is the config *text*, not an object: the provider hands
/// it to Libbox verbatim, and re-encoding it here would be one more chance to
/// change it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Handoff {
    version: u32,
    active_profile_id: Option<String>,
    main_config_path: String,
    status_path: String,
    log_path: String,
    singbox_config_json: String,
}

/// Builds the handshake for a start request, staging local rule sets into
/// `shared_dir` on the way.
pub fn build_handoff(
    request: &NativeTunStartRequest,
    shared_dir: &Path,
) -> Result<String, HandoffError> {
    let config_text = fs::read_to_string(&request.main_config_path).map_err(|source| {
        HandoffError::ReadConfig {
            path: request.main_config_path.clone(),
            source,
        }
    })?;
    let staged = stage_local_rule_sets(&config_text, &request.main_config_path, shared_dir)?;

    Ok(serde_json::to_string(&Handoff {
        active_profile_id: request.active_profile_id.clone(),
        log_path: shared_dir.join(LOG_FILE).display().to_string(),
        main_config_path: request.main_config_path.display().to_string(),
        singbox_config_json: staged,
        status_path: shared_dir.join(STATUS_FILE).display().to_string(),
        version: HANDOFF_VERSION,
    })?)
}

/// Copies every `type: "local"` rule set into the shared container and rewrites
/// its path.
///
/// Returns the configuration text unchanged when there is nothing to rewrite,
/// which is the common case: a config whose rule sets are all remote, or none.
fn stage_local_rule_sets(
    config_text: &str,
    config_path: &Path,
    shared_dir: &Path,
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

    let staging = shared_dir.join(STAGING_DIR);
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
            fs::create_dir_all(&staging).map_err(|source| HandoffError::Staging {
                path: staging.clone(),
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
