use std::{
    env, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use thiserror::Error;
use voya_platform::{
    autostart::{
        apply_autostart, AutostartAdapter, AutostartError, AutostartRequest, StdAutostartAdapter,
        AUTOSTART_APP_NAME,
    },
    coreinfo::TargetOs,
    process::StdProcessRunner,
};

#[derive(Clone)]
pub struct AutostartManager {
    adapter: Arc<dyn AutostartAdapter>,
    target_os: TargetOs,
    app_name: String,
}

impl AutostartManager {
    #[must_use]
    pub fn new() -> Self {
        Self::with_adapter(
            Arc::new(StdAutostartAdapter::new(Arc::new(StdProcessRunner::new()))),
            TargetOs::current(),
            AUTOSTART_APP_NAME,
        )
    }

    #[must_use]
    pub fn with_adapter(
        adapter: Arc<dyn AutostartAdapter>,
        target_os: TargetOs,
        app_name: impl Into<String>,
    ) -> Self {
        Self {
            adapter,
            target_os,
            app_name: app_name.into(),
        }
    }

    /// Writes or removes the login entry.
    pub fn set_enabled(&self, enabled: bool) -> Result<(), AutostartManagerError> {
        let request = self.request(enabled)?;
        apply_autostart(self.adapter.as_ref(), &request)?;
        Ok(())
    }

    fn request(&self, enabled: bool) -> Result<AutostartRequest, AutostartManagerError> {
        Ok(AutostartRequest {
            target_os: self.target_os,
            enabled,
            app_name: self.app_name.clone(),
            executable: current_executable()?,
            home_dir: home_dir()?,
        })
    }
}

impl Default for AutostartManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error)]
pub enum AutostartManagerError {
    #[error(transparent)]
    Autostart(#[from] AutostartError),
    #[error("failed to determine current executable path: {0}")]
    CurrentExe(io::Error),
    #[error("failed to determine a home directory for autostart artifacts")]
    HomeDir,
}

/// Resolve the path a login entry should launch.
///
/// Inside an AppImage `current_exe()` points at the transient `/tmp/.mount_*`
/// FUSE mount, which no longer exists once the app quits, so an autostart entry
/// written from it silently never launches. The runtime exports `APPIMAGE` with
/// the real `.AppImage` path, so prefer it whenever it names an existing file.
fn current_executable() -> Result<PathBuf, AutostartManagerError> {
    let current_exe = env::current_exe().map_err(AutostartManagerError::CurrentExe)?;
    let app_image = env::var_os("APPIMAGE").map(PathBuf::from);

    Ok(resolve_autostart_executable(
        app_image,
        current_exe,
        Path::is_file,
    ))
}

/// Pure half of [`current_executable`], so the AppImage preference is testable
/// without mutating process-wide environment state.
fn resolve_autostart_executable(
    app_image: Option<PathBuf>,
    current_exe: PathBuf,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    match app_image {
        Some(path) if path.is_absolute() && exists(&path) => path,
        _ => current_exe,
    }
}

fn home_dir() -> Result<PathBuf, AutostartManagerError> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or(AutostartManagerError::HomeDir)
}

#[cfg(test)]
mod autostart_app_tests {
    use std::sync::Mutex;

    use voya_platform::autostart::{AutostartAdapter, LoginItemState};

    use super::*;

    #[test]
    fn autostart_prefers_the_appimage_path_over_the_transient_mount() {
        // Inside an AppImage current_exe() is /tmp/.mount_*/usr/bin/voyavpn, which
        // is gone by the time the login entry runs.
        let resolved = resolve_autostart_executable(
            Some(PathBuf::from("/home/u/Apps/VoyaVPN.AppImage")),
            PathBuf::from("/tmp/.mount_abc/usr/bin/voyavpn"),
            |_| true,
        );

        assert_eq!(resolved, PathBuf::from("/home/u/Apps/VoyaVPN.AppImage"));
    }

    #[test]
    fn autostart_ignores_an_appimage_path_that_is_relative_or_missing() {
        let missing = resolve_autostart_executable(
            Some(PathBuf::from("/home/u/Apps/Gone.AppImage")),
            PathBuf::from("/usr/bin/voyavpn"),
            |_| false,
        );
        assert_eq!(missing, PathBuf::from("/usr/bin/voyavpn"));

        let relative = resolve_autostart_executable(
            Some(PathBuf::from("relative/VoyaVPN.AppImage")),
            PathBuf::from("/usr/bin/voyavpn"),
            |_| true,
        );
        assert_eq!(relative, PathBuf::from("/usr/bin/voyavpn"));
    }

    #[test]
    fn autostart_falls_back_to_current_exe_outside_an_appimage() {
        let resolved =
            resolve_autostart_executable(None, PathBuf::from("/usr/bin/voyavpn"), |_| true);

        assert_eq!(resolved, PathBuf::from("/usr/bin/voyavpn"));
    }

    #[derive(Default)]
    struct FakeAutostartAdapter {
        writes: Mutex<u32>,
        registry_sets: Mutex<u32>,
        login_items: Mutex<Vec<bool>>,
    }

    impl AutostartAdapter for FakeAutostartAdapter {
        fn write_file(&self, _path: &Path, _contents: &str) -> Result<(), AutostartError> {
            *self.writes.lock().expect("writes") += 1;
            Ok(())
        }

        fn remove_file(&self, _path: &Path) -> Result<(), AutostartError> {
            Ok(())
        }

        fn set_windows_run_registry(
            &self,
            _key_path: &str,
            _value_name: &str,
            _value: &str,
        ) -> Result<(), AutostartError> {
            *self.registry_sets.lock().expect("registry_sets") += 1;
            Ok(())
        }

        fn delete_windows_run_registry(
            &self,
            _key_path: &str,
            _value_name: &str,
        ) -> Result<(), AutostartError> {
            Ok(())
        }

        fn set_login_item(
            &self,
            _plist_name: &str,
            enabled: bool,
        ) -> Result<LoginItemState, AutostartError> {
            self.login_items.lock().expect("login_items").push(enabled);
            Ok(LoginItemState::Enabled)
        }
    }

    #[test]
    fn autostart_manager_writes_the_linux_desktop_file() {
        let adapter = Arc::new(FakeAutostartAdapter::default());
        let manager =
            AutostartManager::with_adapter(adapter.clone(), TargetOs::Linux, AUTOSTART_APP_NAME);

        manager.set_enabled(true).expect("autostart set");

        assert_eq!(*adapter.writes.lock().expect("writes"), 1);
    }

    #[test]
    fn autostart_manager_registers_the_macos_login_item() {
        let adapter = Arc::new(FakeAutostartAdapter::default());
        let manager =
            AutostartManager::with_adapter(adapter.clone(), TargetOs::Macos, AUTOSTART_APP_NAME);

        manager.set_enabled(true).expect("autostart set");

        assert_eq!(*adapter.login_items.lock().expect("login_items"), [true]);
        assert_eq!(*adapter.writes.lock().expect("writes"), 0);
    }
}
