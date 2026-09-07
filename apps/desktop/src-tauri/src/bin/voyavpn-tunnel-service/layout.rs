//! Trusted locations for the Windows tunnel service.
//!
//! The service runs as LocalSystem, so nothing it executes or writes may be
//! selected by the caller. Every path here is derived from the service's own
//! installed location or from machine-wide known folders; a start argument only
//! selects *which* already-trusted runtime config is used.

use std::{
    env, fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use crate::ServiceError;

pub(crate) const PRODUCT_DIR_NAME: &str = "VoyaVPN";
pub(crate) const APP_IDENTIFIER: &str = "app.voyavpn.desktop";
pub(crate) const SING_BOX_CORE_DIR: &str = "sing_box";
pub(crate) const BIN_CONFIG_DIR: &str = "binConfigs";
pub(crate) const RUNTIME_STAGING_DIR: &str = "runtime";
/// Generated runtime configs are tens of kilobytes. The cap stops a hostile or
/// corrupt caller from making the service buffer an arbitrary file as SYSTEM.
pub(crate) const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServiceLayout {
    /// Administrator-owned directory holding the sing-box the service launches.
    pub(crate) core_dir: PathBuf,
    /// Service-owned directory the accepted config is copied into, and the
    /// working directory the core runs in.
    pub(crate) staging_dir: PathBuf,
    /// Directories a caller-supplied config file may live in.
    pub(crate) config_dirs: Vec<PathBuf>,
    /// Roots that files referenced by an accepted config may live under.
    pub(crate) reference_roots: Vec<PathBuf>,
}

impl ServiceLayout {
    pub(crate) fn from_environment() -> Result<Self, ServiceError> {
        let service_dir = service_directory()?;
        let staging_dir = program_data_dir()?
            .join(PRODUCT_DIR_NAME)
            .join(RUNTIME_STAGING_DIR);
        let app_data_roots = user_app_data_roots();

        let mut config_dirs = vec![staging_dir.clone()];
        config_dirs.extend(app_data_roots.iter().map(|root| root.join(BIN_CONFIG_DIR)));
        let mut reference_roots = vec![staging_dir.clone()];
        reference_roots.extend(app_data_roots);

        Ok(Self {
            core_dir: service_dir.join(SING_BOX_CORE_DIR),
            staging_dir,
            config_dirs,
            reference_roots,
        })
    }
}

fn service_directory() -> Result<PathBuf, ServiceError> {
    let executable = env::current_exe().map_err(ServiceError::ServiceLocation)?;
    executable.parent().map(Path::to_path_buf).ok_or_else(|| {
        ServiceError::ServiceLocation(io::Error::new(
            io::ErrorKind::NotFound,
            "service executable has no parent directory",
        ))
    })
}

fn program_data_dir() -> Result<PathBuf, ServiceError> {
    if let Some(value) = env::var_os("ProgramData").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(value));
    }
    if let Some(drive) = system_drive() {
        return Ok(drive.join("ProgramData"));
    }

    Err(ServiceError::Unsupported(
        "the tunnel service runtime root requires %ProgramData%, so it only runs on Windows"
            .to_string(),
    ))
}

fn system_drive() -> Option<PathBuf> {
    let drive = env::var_os("SystemDrive")?;
    let drive = drive.to_str()?.trim().to_string();
    // `%SystemDrive%` is `C:`; joining that without a separator would produce a
    // drive-relative path instead of the drive root.
    (!drive.is_empty()).then(|| PathBuf::from(format!("{drive}\\")))
}

/// Every local profile's VoyaVPN app-data root. The desktop app is a per-user
/// install, so its runtime config lives under `%APPDATA%\app.voyavpn.desktop`;
/// the service resolves those roots from the machine's profile directory rather
/// than trusting the directory chain of the path it was handed.
fn user_app_data_roots() -> Vec<PathBuf> {
    let Some(profiles_root) = user_profiles_root() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&profiles_root) else {
        return Vec::new();
    };

    let mut roots = entries
        .flatten()
        .map(|entry| {
            entry
                .path()
                .join("AppData")
                .join("Roaming")
                .join(APP_IDENTIFIER)
        })
        .filter(|root| root.is_dir())
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

fn user_profiles_root() -> Option<PathBuf> {
    if let Some(public) = env::var_os("PUBLIC").filter(|value| !value.is_empty()) {
        if let Some(parent) = Path::new(&public).parent() {
            return Some(parent.to_path_buf());
        }
    }
    Some(system_drive()?.join("Users"))
}

pub(crate) fn read_config_within_limit(path: &Path) -> Result<String, ServiceError> {
    let file = fs::File::open(path).map_err(|source| ServiceError::Staging {
        path: path.to_path_buf(),
        source,
    })?;
    let mut contents = String::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_string(&mut contents)
        .map_err(|source| ServiceError::Staging {
            path: path.to_path_buf(),
            source,
        })?;
    if contents.len() as u64 > MAX_CONFIG_BYTES {
        return Err(ServiceError::ConfigTooLarge {
            path: path.to_path_buf(),
            limit: MAX_CONFIG_BYTES,
        });
    }

    Ok(contents)
}

/// Rejects configs that would make the core read or write outside the
/// service-owned staging directory and the VoyaVPN app-data roots. The config
/// stays user-generated, so these options are the remaining way a caller could
/// reach the rest of the filesystem with the service account's rights.
pub(crate) fn ensure_config_files_are_contained(
    config: &serde_json::Value,
    layout: &ServiceLayout,
) -> Result<(), ServiceError> {
    let writable_roots = std::slice::from_ref(&layout.staging_dir);
    for (field, value) in [
        ("log.output", config.pointer("/log/output")),
        (
            "experimental.cache_file.path",
            config.pointer("/experimental/cache_file/path"),
        ),
    ] {
        if let Some(path) = value.and_then(serde_json::Value::as_str) {
            ensure_path_is_contained(field, path, writable_roots)?;
        }
    }

    let rule_sets = config
        .pointer("/route/rule_set")
        .and_then(serde_json::Value::as_array);
    for rule_set in rule_sets.into_iter().flatten() {
        if rule_set.get("type").and_then(serde_json::Value::as_str) != Some("local") {
            continue;
        }
        let Some(path) = rule_set.get("path").and_then(serde_json::Value::as_str) else {
            continue;
        };
        ensure_path_is_contained("route.rule_set.path", path, &layout.reference_roots)?;
    }

    Ok(())
}

fn ensure_path_is_contained(
    field: &'static str,
    value: &str,
    roots: &[PathBuf],
) -> Result<(), ServiceError> {
    let candidate = Path::new(value);
    if candidate.is_absolute() {
        let contained = canonicalize_existing_or_parent(candidate)
            .is_ok_and(|canonical| is_inside_any(&canonical, roots));
        if contained {
            return Ok(());
        }
    } else if !candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        // Relative paths resolve against the staging working directory.
        return Ok(());
    }

    Err(ServiceError::InvalidConfigOption {
        field,
        value: value.to_string(),
        reason: format!("path must stay inside {}", describe_paths(roots)),
    })
}

pub(crate) fn describe_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn is_inside_any(path: &Path, roots: &[PathBuf]) -> bool {
    roots
        .iter()
        .filter_map(|root| fs::canonicalize(root).ok())
        .any(|root| path.starts_with(root))
}

pub(crate) fn canonicalize_existing_or_parent(path: &Path) -> Result<PathBuf, ServiceError> {
    match fs::canonicalize(path) {
        Ok(canonical) => Ok(canonical),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let Some(parent) = path.parent() else {
                return Err(ServiceError::Canonicalize {
                    path: path.to_path_buf(),
                    source: error,
                });
            };
            fs::canonicalize(parent).map_err(|source| ServiceError::Canonicalize {
                path: parent.to_path_buf(),
                source,
            })
        }
        Err(source) => Err(ServiceError::Canonicalize {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(root: &Path) -> ServiceLayout {
        ServiceLayout {
            core_dir: root.join("ProgramFiles").join(SING_BOX_CORE_DIR),
            staging_dir: root.join("staging"),
            config_dirs: vec![root.join("staging")],
            reference_roots: vec![root.join("staging"), root.join("appdata")],
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "voyavpn-tunnel-layout-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        fs::create_dir_all(root.join("staging")).expect("create staging directory");
        fs::create_dir_all(root.join("appdata")).expect("create app data directory");
        root
    }

    #[test]
    fn relative_paths_without_parent_components_stay_in_the_staging_directory() {
        let root = temp_root("relative-ok");
        let config = serde_json::json!({
            "experimental": {"cache_file": {"enabled": true, "path": "cache.db"}}
        });

        ensure_config_files_are_contained(&config, &layout(&root)).expect("relative path is safe");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn relative_parent_components_are_rejected() {
        let root = temp_root("relative-escape");
        let config = serde_json::json!({"log": {"output": "../voya.log"}});

        let error = ensure_config_files_are_contained(&config, &layout(&root))
            .expect_err("parent components must be rejected");

        assert!(
            matches!(error, ServiceError::InvalidConfigOption { field, .. }
            if field == "log.output")
        );

        let _ = fs::remove_dir_all(root);
    }

    fn parse(text: &str) -> serde_json::Value {
        serde_json::from_str(text).expect("test config is valid JSON")
    }

    fn json_path(path: &Path) -> String {
        path.display().to_string().replace('\\', "\\\\")
    }

    #[test]
    fn read_only_options_may_point_into_the_app_data_reference_roots() {
        let root = temp_root("reference-root");
        let ruleset = root.join("appdata").join("geosite-cn.srs");
        fs::write(&ruleset, "srs").expect("write ruleset");
        let config = parse(&format!(
            r#"{{"route":{{"rule_set":[{{"tag":"geosite-cn","type":"local","path":"{}"}}]}}}}"#,
            json_path(&ruleset)
        ));

        ensure_config_files_are_contained(&config, &layout(&root))
            .expect("app-data rule sets are readable");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writable_options_may_not_point_into_the_reference_roots() {
        let root = temp_root("writable-root");
        let log = root.join("appdata").join("core.log");
        let config = parse(&format!(r#"{{"log":{{"output":"{}"}}}}"#, json_path(&log)));

        let error = ensure_config_files_are_contained(&config, &layout(&root))
            .expect_err("writes outside the staging directory must be rejected");

        assert!(
            matches!(error, ServiceError::InvalidConfigOption { field, .. }
            if field == "log.output")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn remote_rule_sets_carry_no_local_path_to_validate() {
        let root = temp_root("remote-ruleset");
        let config = serde_json::json!({
            "route": {"rule_set": [{"tag": "geosite-cn", "type": "remote", "url": "https://x/y"}]}
        });

        ensure_config_files_are_contained(&config, &layout(&root)).expect("remote rule sets pass");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn oversized_configs_are_rejected_before_they_are_parsed() {
        let root = temp_root("oversized");
        let path = root.join("staging").join("huge.json");
        let payload = "a".repeat(usize::try_from(MAX_CONFIG_BYTES).unwrap_or(usize::MAX) + 1);
        fs::write(&path, payload).expect("write oversized config");

        let error = read_config_within_limit(&path).expect_err("oversized config must be rejected");

        assert!(matches!(error, ServiceError::ConfigTooLarge { .. }));

        let _ = fs::remove_dir_all(root);
    }
}
