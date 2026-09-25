use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::paths::{core_seed_resource_dir, AppPaths};

/// Executable names probed for the sing-box core, in order.
pub const SING_BOX_EXECUTABLES: &[&str] = &["sing-box"];
/// The directory under `bin/`, and under the packaged core seeds, that holds sing-box.
pub const CORE_DIR_NAME: &str = "sing_box";
const SING_BOX_ARGUMENTS: &str = "run -c {0} --disable-color";
const SING_BOX_RELEASES_URL: &str = "https://github.com/SagerNet/sing-box/releases";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreLaunch {
    pub executable: PathBuf,
    pub arguments: String,
    pub working_dir: PathBuf,
}

/// How sing-box runs `config_file`, from the runtime config directory.
#[must_use]
pub fn core_launch(
    executable: impl Into<PathBuf>,
    paths: &AppPaths,
    config_file: impl AsRef<Path>,
) -> CoreLaunch {
    CoreLaunch {
        executable: executable.into(),
        arguments: SING_BOX_ARGUMENTS.replace("{0}", &config_file.as_ref().to_string_lossy()),
        working_dir: paths.bin_config_dir().to_path_buf(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSeedCopyStatus {
    SeedMissing,
    AlreadyInstalled,
    Copied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSeedCopyOutcome {
    pub seed_dir: PathBuf,
    pub target_dir: PathBuf,
    pub status: CoreSeedCopyStatus,
    pub copied_files: Vec<PathBuf>,
    pub chmod_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    Linux,
    Macos,
    Ios,
    Android,
    Other,
}

impl TargetOs {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "android") {
            // Checked before Linux: Android *is* Linux to `target_family`, and
            // `cfg!(target_os = "linux")` is false there, but the order states
            // the intent rather than relying on that.
            Self::Android
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "ios") {
            // Likewise before macOS: they share frameworks, not behaviour.
            Self::Ios
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Other
        }
    }

    /// Whether the core runs inside a tunnel provider rather than as a child
    /// of the app.
    ///
    /// macOS's PacketTunnel and both phones answer yes, and the same two
    /// things follow on all three: the runtime config carries an unrouted
    /// probe outbound per node, and a latency test while connected goes
    /// through that core's Clash API instead of launching one of its own.
    #[must_use]
    pub const fn runs_core_in_tunnel_provider(self) -> bool {
        matches!(self, Self::Macos | Self::Ios | Self::Android)
    }
}

#[derive(Debug, Error)]
pub enum CoreInfoError {
    #[error("core sing_box executable not found in {search_dir}; expected one of: {candidates}; download: {url}")]
    ExecutableNotFound {
        search_dir: PathDisplay,
        candidates: String,
        url: &'static str,
    },
    #[error("failed to create core bin directory {path}: {source}")]
    CreateCoreBinDir { path: PathBuf, source: io::Error },
    #[error("failed to inspect executable {path}: {source}")]
    InspectExecutable { path: PathBuf, source: io::Error },
    #[error("failed to inspect core seed resource {path}: {source}")]
    InspectCoreSeed { path: PathBuf, source: io::Error },
    #[error("core seed resource path is not a directory: {path}")]
    InvalidCoreSeedDir { path: PathBuf },
    #[error("failed to read core seed directory {path}: {source}")]
    ReadCoreSeedDir { path: PathBuf, source: io::Error },
    #[error("failed to copy core seed asset from {source_path} to {target_path}: {source}")]
    CopyCoreSeedAsset {
        source_path: PathBuf,
        target_path: PathBuf,
        source: io::Error,
    },
    #[error("failed to update executable permissions for {path}: {source}")]
    ChmodExecutable { path: PathBuf, source: io::Error },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathDisplay(PathBuf);

impl std::fmt::Display for PathDisplay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0.display())
    }
}

#[must_use]
pub fn executable_name_for_os(name: &str, os: TargetOs) -> String {
    if os == TargetOs::Windows && !name.to_ascii_lowercase().ends_with(".exe") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

#[must_use]
pub fn executable_name_for_current_os(name: &str) -> String {
    executable_name_for_os(name, TargetOs::current())
}

/// The sing-box executables `dir` may hold, spelled for `os`.
fn executable_candidates(dir: &Path, os: TargetOs) -> impl Iterator<Item = PathBuf> + '_ {
    SING_BOX_EXECUTABLES
        .iter()
        .map(move |name| dir.join(executable_name_for_os(name, os)))
}

/// The first candidate that exists as a regular file.
fn first_existing_file(
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Result<Option<PathBuf>, CoreInfoError> {
    for candidate in candidates {
        match candidate.try_exists() {
            Ok(true) if candidate.is_file() => return Ok(Some(candidate)),
            Ok(_) => {}
            Err(source) => {
                return Err(CoreInfoError::InspectExecutable {
                    path: candidate,
                    source,
                });
            }
        }
    }

    Ok(None)
}

pub fn discover_executable(paths: &AppPaths) -> Result<PathBuf, CoreInfoError> {
    let search_dir = paths.core_bin_dir(CORE_DIR_NAME);
    fs::create_dir_all(&search_dir).map_err(|source| CoreInfoError::CreateCoreBinDir {
        path: search_dir.clone(),
        source,
    })?;

    if let Some(executable) =
        first_existing_file(executable_candidates(&search_dir, TargetOs::current()))?
    {
        ensure_executable_permission(&executable)?;
        return Ok(executable);
    }

    let candidates = SING_BOX_EXECUTABLES
        .iter()
        .map(|name| executable_name_for_current_os(name))
        .collect::<Vec<_>>()
        .join(", ");
    Err(CoreInfoError::ExecutableNotFound {
        search_dir: PathDisplay(search_dir),
        candidates,
        url: SING_BOX_RELEASES_URL,
    })
}

/// Finds the sing-box seed inside the app bundle without copying it anywhere.
///
/// macOS launches the seed where it lies: a copy in app data would lose the
/// bundle's code-signing context and fail the notarized launch.
pub fn discover_packaged_seed_executable(
    seed_resources_dir: impl AsRef<Path>,
    target_os: TargetOs,
) -> Result<Option<PathBuf>, CoreInfoError> {
    let search_dir = core_seed_resource_dir(seed_resources_dir, CORE_DIR_NAME);

    match search_dir.try_exists() {
        Ok(false) => return Ok(None),
        Ok(true) => {}
        Err(source) => {
            return Err(CoreInfoError::InspectCoreSeed {
                path: search_dir,
                source,
            });
        }
    }

    if !search_dir.is_dir() {
        return Err(CoreInfoError::InvalidCoreSeedDir { path: search_dir });
    }

    first_existing_file(executable_candidates(&search_dir, target_os))
}

/// Copies the packaged sing-box seed into app data unless an executable is
/// already installed there.
pub fn copy_seed_core_asset(
    paths: &AppPaths,
    seed_resources_dir: impl AsRef<Path>,
) -> Result<CoreSeedCopyOutcome, CoreInfoError> {
    let seed_dir = core_seed_resource_dir(seed_resources_dir, CORE_DIR_NAME);
    let target_dir = paths.core_bin_dir(CORE_DIR_NAME);

    match seed_dir.try_exists() {
        Ok(false) => {
            return Ok(CoreSeedCopyOutcome {
                seed_dir,
                target_dir,
                status: CoreSeedCopyStatus::SeedMissing,
                copied_files: Vec::new(),
                chmod_paths: Vec::new(),
            });
        }
        Ok(true) => {}
        Err(source) => {
            return Err(CoreInfoError::InspectCoreSeed {
                path: seed_dir,
                source,
            });
        }
    }

    if !seed_dir.is_dir() {
        return Err(CoreInfoError::InvalidCoreSeedDir { path: seed_dir });
    }

    if first_existing_file(executable_candidates(&target_dir, TargetOs::current()))?.is_some() {
        let chmod_paths = apply_executable_permission_plan(paths)?;
        return Ok(CoreSeedCopyOutcome {
            seed_dir,
            target_dir,
            status: CoreSeedCopyStatus::AlreadyInstalled,
            copied_files: Vec::new(),
            chmod_paths,
        });
    }

    fs::create_dir_all(&target_dir).map_err(|source| CoreInfoError::CreateCoreBinDir {
        path: target_dir.clone(),
        source,
    })?;

    let mut copied_files = Vec::new();
    copy_seed_dir_contents(&seed_dir, &target_dir, &mut copied_files)?;
    let chmod_paths = apply_executable_permission_plan(paths)?;

    Ok(CoreSeedCopyOutcome {
        seed_dir,
        target_dir,
        status: CoreSeedCopyStatus::Copied,
        copied_files,
        chmod_paths,
    })
}

/// The executables that need the execute bit once a seed is in place.
fn executable_permission_plan(paths: &AppPaths) -> Vec<PathBuf> {
    #[cfg(unix)]
    {
        executable_candidates(&paths.core_bin_dir(CORE_DIR_NAME), TargetOs::current()).collect()
    }

    #[cfg(not(unix))]
    {
        let _ = paths;
        Vec::new()
    }
}

fn apply_executable_permission_plan(paths: &AppPaths) -> Result<Vec<PathBuf>, CoreInfoError> {
    let mut chmod_paths = Vec::new();
    for candidate in executable_permission_plan(paths) {
        if let Some(executable) = first_existing_file([candidate])? {
            ensure_executable_permission(&executable)?;
            chmod_paths.push(executable);
        }
    }

    Ok(chmod_paths)
}

fn copy_seed_dir_contents(
    source_dir: &Path,
    target_dir: &Path,
    copied_files: &mut Vec<PathBuf>,
) -> Result<(), CoreInfoError> {
    let entries = fs::read_dir(source_dir).map_err(|source| CoreInfoError::ReadCoreSeedDir {
        path: source_dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| CoreInfoError::ReadCoreSeedDir {
            path: source_dir.to_path_buf(),
            source,
        })?;
        let source_path = entry.path();
        let target_path = target_dir.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|source| CoreInfoError::InspectCoreSeed {
                path: source_path.clone(),
                source,
            })?;

        if file_type.is_dir() {
            fs::create_dir_all(&target_path).map_err(|source| CoreInfoError::CreateCoreBinDir {
                path: target_path.clone(),
                source,
            })?;
            copy_seed_dir_contents(&source_path, &target_path, copied_files)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        match target_path.try_exists() {
            Ok(true) => continue,
            Ok(false) => {}
            Err(source) => {
                return Err(CoreInfoError::InspectExecutable {
                    path: target_path,
                    source,
                });
            }
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| CoreInfoError::CreateCoreBinDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::copy(&source_path, &target_path).map_err(|source| {
            CoreInfoError::CopyCoreSeedAsset {
                source_path,
                target_path: target_path.clone(),
                source,
            }
        })?;
        copied_files.push(target_path);
    }

    Ok(())
}

pub fn ensure_executable_permission(path: impl AsRef<Path>) -> Result<(), CoreInfoError> {
    ensure_executable_permission_inner(path.as_ref())
}

#[cfg(unix)]
fn ensure_executable_permission_inner(path: &Path) -> Result<(), CoreInfoError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| CoreInfoError::InspectExecutable {
        path: path.to_path_buf(),
        source,
    })?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    let executable_mode = mode | 0o111;
    if executable_mode != mode {
        permissions.set_mode(executable_mode);
        fs::set_permissions(path, permissions).map_err(|source| {
            CoreInfoError::ChmodExecutable {
                path: path.to_path_buf(),
                source,
            }
        })?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_executable_permission_inner(path: &Path) -> Result<(), CoreInfoError> {
    let _ = fs::metadata(path).map_err(|source| CoreInfoError::InspectExecutable {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use crate::paths::{core_seed_resources_dir, AppPaths};

    use super::*;

    #[test]
    fn sing_box_runs_the_config_from_the_config_dir() {
        let paths = AppPaths::new("/tmp/VoyaVPN");
        let launch = core_launch("/tmp/VoyaVPN/bin/sing_box/sing-box", &paths, "config.json");

        assert_eq!(launch.arguments, "run -c config.json --disable-color");
        assert_eq!(launch.working_dir, paths.bin_config_dir());
    }

    #[test]
    fn coreinfo_executable_discovery_uses_core_subdir_and_probe_order() {
        let root = unique_temp_root("discover");
        let paths = AppPaths::new(root.join("VoyaVPN"));
        let exe = paths.core_bin_file(CORE_DIR_NAME, executable_name_for_current_os("sing-box"));
        fs::create_dir_all(exe.parent().expect("sing-box exe parent"))
            .expect("create sing-box dir");
        fs::write(&exe, b"").expect("write sing-box exe");

        let discovered = discover_executable(&paths).expect("discover sing-box");
        assert_eq!(discovered, exe);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coreinfo_discovers_packaged_seed_executable_without_copying() {
        let root = unique_temp_root("seed-discover");
        let seed_root = core_seed_resources_dir(root.join("resources"));
        let seed_exe = seed_root
            .join(CORE_DIR_NAME)
            .join(executable_name_for_current_os("sing-box"));
        fs::create_dir_all(seed_exe.parent().expect("seed exe parent")).expect("create seed dir");
        fs::write(&seed_exe, b"seed-sing-box").expect("write seed exe");

        let discovered = discover_packaged_seed_executable(&seed_root, TargetOs::current())
            .expect("discover packaged seed executable");

        assert_eq!(discovered, Some(seed_exe));
        assert_eq!(
            discover_packaged_seed_executable(root.join("missing"), TargetOs::current())
                .expect("a missing seed dir is not an error"),
            None
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coreinfo_seed_copy_missing_seed_is_noop() {
        let root = unique_temp_root("seed-missing");
        let paths = AppPaths::new(root.join("VoyaVPN"));
        let seed_root = core_seed_resources_dir(root.join("resources"));

        let outcome = copy_seed_core_asset(&paths, &seed_root).expect("missing seed noop");

        assert_eq!(outcome.status, CoreSeedCopyStatus::SeedMissing);
        assert!(outcome.copied_files.is_empty());
        assert!(!paths.core_bin_dir(CORE_DIR_NAME).exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coreinfo_seed_copy_copies_missing_core_into_app_data() {
        let root = unique_temp_root("seed-copy");
        let paths = AppPaths::new(root.join("VoyaVPN"));
        let seed_root = core_seed_resources_dir(root.join("resources"));
        let seed_exe = seed_root
            .join(CORE_DIR_NAME)
            .join(executable_name_for_current_os("sing-box"));
        fs::create_dir_all(seed_exe.parent().expect("seed exe parent")).expect("create seed dir");
        fs::write(&seed_exe, b"seed-sing-box").expect("write seed exe");

        let outcome = copy_seed_core_asset(&paths, &seed_root).expect("copy seed");
        let app_data_exe =
            paths.core_bin_file(CORE_DIR_NAME, executable_name_for_current_os("sing-box"));

        assert_eq!(outcome.status, CoreSeedCopyStatus::Copied);
        assert_eq!(outcome.copied_files, vec![app_data_exe.clone()]);
        assert_eq!(
            fs::read(&app_data_exe).expect("read copied exe"),
            b"seed-sing-box"
        );
        assert_eq!(
            discover_executable(&paths).expect("discover copied app data exe"),
            app_data_exe
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coreinfo_seed_copy_does_not_overwrite_existing_core() {
        let root = unique_temp_root("seed-existing");
        let paths = AppPaths::new(root.join("VoyaVPN"));
        let seed_root = core_seed_resources_dir(root.join("resources"));
        let executable_name = executable_name_for_current_os("sing-box");
        let seed_exe = seed_root.join(CORE_DIR_NAME).join(&executable_name);
        let app_data_exe = paths.core_bin_file(CORE_DIR_NAME, &executable_name);
        fs::create_dir_all(seed_exe.parent().expect("seed exe parent")).expect("create seed dir");
        fs::create_dir_all(app_data_exe.parent().expect("app data exe parent"))
            .expect("create app data dir");
        fs::write(&seed_exe, b"older-seed").expect("write seed exe");
        fs::write(&app_data_exe, b"newer-installed").expect("write installed exe");

        let outcome = copy_seed_core_asset(&paths, &seed_root).expect("skip existing");

        assert_eq!(outcome.status, CoreSeedCopyStatus::AlreadyInstalled);
        assert!(outcome.copied_files.is_empty());
        assert_eq!(
            fs::read(&app_data_exe).expect("read installed exe"),
            b"newer-installed"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn coreinfo_seed_copy_applies_unix_chmod_plan() {
        let root = unique_temp_root("seed-chmod");
        let paths = AppPaths::new(root.join("VoyaVPN"));
        let seed_root = core_seed_resources_dir(root.join("resources"));
        let seed_exe = seed_root.join(CORE_DIR_NAME).join("sing-box");
        fs::create_dir_all(seed_exe.parent().expect("seed exe parent")).expect("create seed dir");
        fs::write(&seed_exe, b"seed-sing-box").expect("write seed exe");
        fs::set_permissions(&seed_exe, fs::Permissions::from_mode(0o600)).expect("set seed mode");

        let plan = executable_permission_plan(&paths);
        let app_data_exe = paths.core_bin_file(CORE_DIR_NAME, "sing-box");
        assert!(plan.contains(&app_data_exe));

        let outcome = copy_seed_core_asset(&paths, &seed_root).expect("copy seed");
        let mode = fs::metadata(&app_data_exe)
            .expect("stat copied exe")
            .permissions()
            .mode();

        assert_eq!(outcome.chmod_paths, vec![app_data_exe]);
        assert_ne!(mode & 0o111, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn coreinfo_discovery_chmods_unix_executables() {
        let root = unique_temp_root("chmod");
        let paths = AppPaths::new(root.join("VoyaVPN"));
        let exe = paths.core_bin_file(CORE_DIR_NAME, "sing-box");
        fs::create_dir_all(exe.parent().expect("sing-box exe parent"))
            .expect("create sing-box dir");
        fs::write(&exe, b"").expect("write sing-box exe");
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o600)).expect("set initial mode");

        let discovered = discover_executable(&paths).expect("discover sing-box");
        let mode = fs::metadata(&discovered)
            .expect("stat discovered exe")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coreinfo_windows_exe_suffix_is_added_only_for_windows() {
        assert_eq!(
            executable_name_for_os("sing-box", TargetOs::Windows),
            "sing-box.exe"
        );
        assert_eq!(
            executable_name_for_os("sing-box.exe", TargetOs::Windows),
            "sing-box.exe"
        );
        assert_eq!(
            executable_name_for_os("sing-box.ExE", TargetOs::Windows),
            "sing-box.ExE"
        );
        assert_eq!(
            executable_name_for_os("sing-box", TargetOs::Linux),
            "sing-box"
        );
        assert_eq!(
            executable_name_for_os("sing-box", TargetOs::Macos),
            "sing-box"
        );
    }

    fn unique_temp_root(name: &str) -> PathBuf {
        tempfile::Builder::new()
            .prefix(&format!("voyavpn-coreinfo-{name}-"))
            .tempdir()
            .expect("coreinfo test temp dir")
            .keep()
    }
}
