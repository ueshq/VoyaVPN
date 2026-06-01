use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

/// Stable application directory name used for non-portable storage.
pub const APP_DIR_NAME: &str = "VoyaVPN";
/// Voya namespaced equivalent of v2rayN's local-application-data switch.
pub const LOCAL_APP_DATA_ENV: &str = "VOYAVPN_LOCAL_APPLICATION_DATA";
/// Compatibility marker from v2rayN: when present beside the app, portable
/// storage must not be used.
pub const PORTABLE_BLOCK_FILE: &str = "NotStoreConfigHere.txt";

pub const CONFIG_DIR_NAME: &str = "guiConfigs";
pub const BIN_DIR_NAME: &str = "bin";
pub const BIN_CONFIG_DIR_NAME: &str = "binConfigs";
pub const BACKUP_DIR_NAME: &str = "guiBackups";
pub const LOG_DIR_NAME: &str = "guiLogs";
pub const TEMP_DIR_NAME: &str = "guiTemps";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Portable,
    UserData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    mode: StorageMode,
    app_dir: PathBuf,
    config_dir: PathBuf,
    bin_dir: PathBuf,
    bin_config_dir: PathBuf,
    backup_dir: PathBuf,
    log_dir: PathBuf,
    temp_dir: PathBuf,
}

impl AppPaths {
    #[must_use]
    pub fn new(app_dir: impl Into<PathBuf>, mode: StorageMode) -> Self {
        let app_dir = app_dir.into();
        Self {
            mode,
            config_dir: app_dir.join(CONFIG_DIR_NAME),
            bin_dir: app_dir.join(BIN_DIR_NAME),
            bin_config_dir: app_dir.join(BIN_CONFIG_DIR_NAME),
            backup_dir: app_dir.join(BACKUP_DIR_NAME),
            log_dir: app_dir.join(LOG_DIR_NAME),
            temp_dir: app_dir.join(TEMP_DIR_NAME),
            app_dir,
        }
    }

    #[must_use]
    pub const fn mode(&self) -> StorageMode {
        self.mode
    }

    #[must_use]
    pub fn is_portable(&self) -> bool {
        self.mode == StorageMode::Portable
    }

    #[must_use]
    pub fn app_dir(&self) -> &Path {
        &self.app_dir
    }

    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    #[must_use]
    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    #[must_use]
    pub fn bin_config_dir(&self) -> &Path {
        &self.bin_config_dir
    }

    #[must_use]
    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    #[must_use]
    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    #[must_use]
    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    #[must_use]
    pub fn config_file(&self, file_name: impl AsRef<Path>) -> PathBuf {
        self.config_dir.join(file_name)
    }

    #[must_use]
    pub fn bin_config_file(&self, file_name: impl AsRef<Path>) -> PathBuf {
        self.bin_config_dir.join(file_name)
    }

    #[must_use]
    pub fn log_file(&self, file_name: impl AsRef<Path>) -> PathBuf {
        self.log_dir.join(file_name)
    }

    #[must_use]
    pub fn temp_file(&self, file_name: impl AsRef<Path>) -> PathBuf {
        self.temp_dir.join(file_name)
    }

    #[must_use]
    pub fn core_bin_dir(&self, core_type_dir: impl AsRef<Path>) -> PathBuf {
        self.bin_dir.join(core_type_dir)
    }

    #[must_use]
    pub fn core_bin_file(
        &self,
        core_type_dir: impl AsRef<Path>,
        file_name: impl AsRef<Path>,
    ) -> PathBuf {
        self.core_bin_dir(core_type_dir).join(file_name)
    }

    pub fn ensure_dirs(&self) -> Result<(), PathError> {
        for dir in [
            &self.app_dir,
            &self.config_dir,
            &self.bin_dir,
            &self.bin_config_dir,
            &self.backup_dir,
            &self.log_dir,
            &self.temp_dir,
        ] {
            create_dir(dir)?;
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PathError {
    #[error("failed to create directory {path}: {source}")]
    CreateDir { path: PathBuf, source: io::Error },
    #[error("failed to determine current executable path: {0}")]
    CurrentExe(io::Error),
    #[error("current executable has no parent directory: {0}")]
    CurrentExeParent(PathBuf),
    #[error("failed to determine a local data directory")]
    LocalDataDir,
}

#[must_use]
pub fn resolve_app_paths_with_roots(
    base_dir: impl AsRef<Path>,
    local_data_root: impl AsRef<Path>,
    force_user_data: bool,
) -> AppPaths {
    let base_dir = base_dir.as_ref();
    let mode = if force_user_data || !can_use_portable_mode(base_dir) {
        StorageMode::UserData
    } else {
        StorageMode::Portable
    };

    let app_dir = match mode {
        StorageMode::Portable => base_dir.to_path_buf(),
        StorageMode::UserData => local_data_root.as_ref().join(APP_DIR_NAME),
    };

    AppPaths::new(app_dir, mode)
}

pub fn resolve_app_paths() -> Result<AppPaths, PathError> {
    let base_dir = current_exe_dir()?;
    let local_data_root = default_local_data_root().ok_or(PathError::LocalDataDir)?;
    let force_user_data = env::var(LOCAL_APP_DATA_ENV).is_ok_and(|value| value == "1");

    Ok(resolve_app_paths_with_roots(
        base_dir,
        local_data_root,
        force_user_data,
    ))
}

#[must_use]
pub fn can_use_portable_mode(base_dir: impl AsRef<Path>) -> bool {
    let base_dir = base_dir.as_ref();
    if base_dir.join(PORTABLE_BLOCK_FILE).exists() {
        return false;
    }

    let temp_dir = base_dir.join(TEMP_DIR_NAME);
    if fs::create_dir_all(&temp_dir).is_err() {
        return false;
    }

    let probe = temp_dir.join(format!(".voyavpn-write-probe-{}", std::process::id()));
    if fs::write(&probe, b"probe").is_err() {
        return false;
    }
    let _ = fs::remove_file(probe);
    true
}

fn current_exe_dir() -> Result<PathBuf, PathError> {
    let exe = env::current_exe().map_err(PathError::CurrentExe)?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or(PathError::CurrentExeParent(exe))
}

fn default_local_data_root() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| env::var_os("APPDATA").map(PathBuf::from))
            .or_else(|| {
                env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .map(|home| home.join("AppData").join("Local"))
            })
    }

    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local").join("share"))
        })
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
}

fn create_dir(path: &Path) -> Result<(), PathError> {
    fs::create_dir_all(path).map_err(|source| PathError::CreateDir {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coreinfo_paths_keep_reference_directory_names() {
        let paths = AppPaths::new("/tmp/VoyaVPN", StorageMode::Portable);

        assert_eq!(paths.app_dir(), Path::new("/tmp/VoyaVPN"));
        assert_eq!(paths.config_dir(), Path::new("/tmp/VoyaVPN/guiConfigs"));
        assert_eq!(paths.bin_dir(), Path::new("/tmp/VoyaVPN/bin"));
        assert_eq!(paths.bin_config_dir(), Path::new("/tmp/VoyaVPN/binConfigs"));
        assert_eq!(paths.log_dir(), Path::new("/tmp/VoyaVPN/guiLogs"));
        assert_eq!(paths.temp_dir(), Path::new("/tmp/VoyaVPN/guiTemps"));
    }

    #[test]
    fn coreinfo_paths_detect_portable_and_forced_user_data_modes() {
        let root = unique_temp_root("paths-mode");
        let base = root.join("app");
        let data = root.join("data");
        fs::create_dir_all(&base).expect("create base dir");

        let portable = resolve_app_paths_with_roots(&base, &data, false);
        assert_eq!(portable.mode(), StorageMode::Portable);
        assert_eq!(portable.app_dir(), base.as_path());

        let forced = resolve_app_paths_with_roots(&base, &data, true);
        assert_eq!(forced.mode(), StorageMode::UserData);
        assert_eq!(forced.app_dir(), data.join(APP_DIR_NAME).as_path());

        fs::write(base.join(PORTABLE_BLOCK_FILE), b"blocked").expect("write portable block file");
        let blocked = resolve_app_paths_with_roots(&base, &data, false);
        assert_eq!(blocked.mode(), StorageMode::UserData);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coreinfo_paths_ensure_required_directories() {
        let root = unique_temp_root("paths-ensure");
        let paths = AppPaths::new(root.join("VoyaVPN"), StorageMode::Portable);

        paths.ensure_dirs().expect("create app directories");

        assert!(paths.config_dir().is_dir());
        assert!(paths.bin_dir().is_dir());
        assert!(paths.bin_config_dir().is_dir());
        assert!(paths.log_dir().is_dir());
        assert!(paths.temp_dir().is_dir());

        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_root(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "voyavpn-{name}-{}-{}",
            std::process::id(),
            monotonic_nanos()
        ))
    }

    fn monotonic_nanos() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    }
}
