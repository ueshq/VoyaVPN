use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use thiserror::Error;

use crate::{
    coreinfo::TargetOs,
    filesystem,
    process::{
        reg_add_arguments, ProcessError, ProcessRole, ProcessRunner, ProcessSpawn, HELPER_TIMEOUT,
    },
};

pub const AUTOSTART_APP_NAME: &str = "VoyaVPN";
/// Passed by every login entry, so a launch can tell it was not the user.
pub const AUTOSTART_ARG: &str = "--autostart";
pub const WINDOWS_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
pub const LINUX_AUTOSTART_DIR: &str = ".config/autostart";
/// Where builds before the SMAppService login item wrote their LaunchAgent,
/// relative to the home directory. Only cleaned up now.
pub const MACOS_LAUNCH_AGENTS_DIR: &str = "Library/LaunchAgents";
/// The launchd agent bundled in `Contents/Library/LaunchAgents` and registered
/// with `SMAppService`. Its file is
/// `apps/desktop/src-tauri/native/macos/LaunchAgents/<this name>`.
pub const MACOS_LOGIN_ITEM_PLIST_NAME: &str = "app.voyavpn.desktop.autostart.plist";
pub const MACOS_LOGIN_ITEM_LABEL: &str = "app.voyavpn.desktop.autostart";

mod macos_login_item;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartRequest {
    pub target_os: TargetOs,
    pub enabled: bool,
    pub app_name: String,
    pub executable: PathBuf,
    pub home_dir: PathBuf,
}

/// Whether this process was started by a login entry. `args` is the full
/// argument list, program name first, as `std::env::args_os` yields it.
#[must_use]
pub fn launched_by_autostart<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    args.into_iter()
        .skip(1)
        .any(|argument| argument.as_ref() == AUTOSTART_ARG)
}

/// `SMAppServiceStatus` of the macOS login item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginItemState {
    NotRegistered,
    Enabled,
    /// Registered, but the user has to allow it in System Settings.
    RequiresApproval,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutostartAction {
    SetWindowsRunRegistry {
        key_path: String,
        value_name: String,
        value: String,
    },
    DeleteWindowsRunRegistry {
        key_path: String,
        value_name: String,
    },
    WriteFile {
        path: PathBuf,
        contents: String,
    },
    RemoveFile {
        path: PathBuf,
    },
    RemoveFileBestEffort {
        path: PathBuf,
    },
    SetLoginItem {
        plist_name: String,
        enabled: bool,
    },
    Noop,
}

pub trait AutostartAdapter: Send + Sync {
    fn write_file(&self, path: &Path, contents: &str) -> Result<(), AutostartError>;
    fn remove_file(&self, path: &Path) -> Result<(), AutostartError>;
    fn set_windows_run_registry(
        &self,
        key_path: &str,
        value_name: &str,
        value: &str,
    ) -> Result<(), AutostartError>;
    fn delete_windows_run_registry(
        &self,
        key_path: &str,
        value_name: &str,
    ) -> Result<(), AutostartError>;
    /// Registers or unregisters the bundled macOS login item.
    fn set_login_item(
        &self,
        plist_name: &str,
        enabled: bool,
    ) -> Result<LoginItemState, AutostartError>;
}

/// Plans the login entry for `request` and carries it out through `adapter`.
pub fn apply_autostart(
    adapter: &dyn AutostartAdapter,
    request: &AutostartRequest,
) -> Result<(), AutostartError> {
    for action in &plan_autostart(request) {
        match action {
            AutostartAction::SetWindowsRunRegistry {
                key_path,
                value_name,
                value,
            } => adapter.set_windows_run_registry(key_path, value_name, value)?,
            AutostartAction::DeleteWindowsRunRegistry {
                key_path,
                value_name,
            } => adapter.delete_windows_run_registry(key_path, value_name)?,
            AutostartAction::WriteFile { path, contents } => {
                adapter.write_file(path, contents)?;
            }
            AutostartAction::RemoveFile { path } => {
                adapter.remove_file(path)?;
            }
            AutostartAction::RemoveFileBestEffort { path } => {
                if let Err(error) = adapter.remove_file(path) {
                    tracing::debug!(%error, "ignored autostart cleanup failure");
                }
            }
            AutostartAction::SetLoginItem {
                plist_name,
                enabled,
            } => {
                let state = adapter.set_login_item(plist_name, *enabled)?;
                if state == LoginItemState::RequiresApproval {
                    tracing::info!(
                        "the login item waits for approval in System Settings > General > \
                         Login Items & Extensions"
                    );
                }
            }
            AutostartAction::Noop => {}
        }
    }

    Ok(())
}

pub struct StdAutostartAdapter {
    runner: Arc<dyn ProcessRunner>,
}

impl StdAutostartAdapter {
    #[must_use]
    pub fn new(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }
}

impl AutostartAdapter for StdAutostartAdapter {
    fn write_file(&self, path: &Path, contents: &str) -> Result<(), AutostartError> {
        filesystem::write_file_with_parent(path, contents).map_err(|source| AutostartError::Io {
            operation: "write autostart file",
            path: path.to_path_buf(),
            source,
        })
    }

    fn remove_file(&self, path: &Path) -> Result<(), AutostartError> {
        filesystem::remove_file_if_exists(path).map_err(|source| AutostartError::Io {
            operation: "remove autostart file",
            path: path.to_path_buf(),
            source,
        })
    }

    fn set_windows_run_registry(
        &self,
        key_path: &str,
        value_name: &str,
        value: &str,
    ) -> Result<(), AutostartError> {
        run_checked(
            &*self.runner,
            Path::new("reg"),
            &reg_add_arguments(key_path, value_name, "REG_SZ", value),
        )
    }

    fn delete_windows_run_registry(
        &self,
        key_path: &str,
        value_name: &str,
    ) -> Result<(), AutostartError> {
        let arguments = vec![
            "delete".to_string(),
            key_path.to_string(),
            "/v".to_string(),
            value_name.to_string(),
            "/f".to_string(),
        ];
        run_checked(&*self.runner, Path::new("reg"), &arguments)
    }

    fn set_login_item(
        &self,
        plist_name: &str,
        enabled: bool,
    ) -> Result<LoginItemState, AutostartError> {
        macos_login_item::set_enabled(plist_name, enabled)
    }
}

#[must_use]
pub(crate) fn plan_autostart(request: &AutostartRequest) -> Vec<AutostartAction> {
    match request.target_os {
        TargetOs::Windows => windows_actions(request),
        TargetOs::Linux => linux_actions(request),
        TargetOs::Macos => macos_actions(request),
        // Nothing launches at login on a phone: the OS owns the lifecycle,
        // and an always-on VPN is a system setting, not an app one.
        TargetOs::Ios | TargetOs::Android | TargetOs::Other => vec![AutostartAction::Noop],
    }
}

#[must_use]
fn windows_value_name(app_name: &str, executable: &Path) -> String {
    format!(
        "{app_name}_{}",
        fnv1a_hex(executable.to_string_lossy().as_bytes())
    )
}

#[must_use]
pub(crate) fn linux_desktop_entry(app_name: &str, executable: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nExec={} {}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\nName[en_US]={app_name}\nName={app_name}\nComment[en_US]={app_name}\nComment={app_name}\n",
        desktop_entry_exec_argument(executable),
        AUTOSTART_ARG
    )
}

/// Quotes one `Exec=` argument per the Desktop Entry specification. The value
/// is always double-quoted, because a path such as `/home/u/My Apps/VoyaVPN`
/// would otherwise launch `/home/u/My`. Inside the quotes `"`, `` ` ``, `$`
/// and `\` are backslash-escaped, `%` is doubled so it is not read as a field
/// code, and every escaping backslash is doubled again because `Exec` is first
/// unescaped as a desktop-file string.
fn desktop_entry_exec_argument(executable: &Path) -> String {
    let mut quoted = String::from("\"");
    for character in executable.to_string_lossy().chars() {
        match character {
            '\\' => quoted.push_str(r"\\\\"),
            '"' | '`' | '$' => {
                quoted.push_str(r"\\");
                quoted.push(character);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

fn windows_actions(request: &AutostartRequest) -> Vec<AutostartAction> {
    let value_name = windows_value_name(&request.app_name, &request.executable);
    if request.enabled {
        vec![AutostartAction::SetWindowsRunRegistry {
            key_path: WINDOWS_RUN_KEY.to_string(),
            value_name,
            value: windows_run_value(&request.executable),
        }]
    } else {
        vec![AutostartAction::DeleteWindowsRunRegistry {
            key_path: WINDOWS_RUN_KEY.to_string(),
            value_name,
        }]
    }
}

fn linux_actions(request: &AutostartRequest) -> Vec<AutostartAction> {
    let path = linux_autostart_path(&request.home_dir, &request.app_name);
    if request.enabled {
        vec![AutostartAction::WriteFile {
            path,
            contents: linux_desktop_entry(&request.app_name, &request.executable),
        }]
    } else {
        vec![AutostartAction::RemoveFile { path }]
    }
}

/// Registers or unregisters the bundled login item, then deletes the
/// LaunchAgent file older builds wrote into the home directory. Inside the App
/// Sandbox the home directory is the container, so that finds nothing; outside
/// it, launchd no longer loads the old agent at the next login. The app runs
/// no `launchctl`, which a sandboxed store app has no business doing.
fn macos_actions(request: &AutostartRequest) -> Vec<AutostartAction> {
    vec![
        AutostartAction::SetLoginItem {
            plist_name: MACOS_LOGIN_ITEM_PLIST_NAME.to_string(),
            enabled: request.enabled,
        },
        AutostartAction::RemoveFileBestEffort {
            path: legacy_macos_launch_agent_path(&request.home_dir, &request.app_name),
        },
    ]
}

fn linux_autostart_path(home_dir: &Path, app_name: &str) -> PathBuf {
    home_dir
        .join(LINUX_AUTOSTART_DIR)
        .join(format!("{app_name}.desktop"))
}

fn legacy_macos_launch_agent_path(home_dir: &Path, app_name: &str) -> PathBuf {
    home_dir
        .join(MACOS_LAUNCH_AGENTS_DIR)
        .join(format!("{app_name}-LaunchAgent.plist"))
}

fn quote_windows_path(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn windows_run_value(executable: &Path) -> String {
    format!("{} {AUTOSTART_ARG}", quote_windows_path(executable))
}

fn run_checked(
    runner: &dyn ProcessRunner,
    executable: &Path,
    arguments: &[String],
) -> Result<(), AutostartError> {
    let output = runner.run_oneshot(
        ProcessSpawn::new(ProcessRole::Autostart, executable.to_path_buf())
            .with_arguments(arguments.to_vec())
            .with_display_log(false)
            .with_timeout(HELPER_TIMEOUT),
    )?;
    if output.success() {
        Ok(())
    } else {
        Err(AutostartError::CommandFailed {
            executable: executable.to_path_buf(),
            arguments: arguments.to_vec(),
            exit: output.status_code,
            stderr: output.stderr,
        })
    }
}

fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[derive(Debug, Error)]
pub enum AutostartError {
    #[error("{operation} failed for {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error(
        "autostart command failed: {executable:?} {arguments:?}: exit={exit:?}, stderr={stderr}"
    )]
    CommandFailed {
        executable: PathBuf,
        arguments: Vec<String>,
        exit: Option<i32>,
        stderr: String,
    },
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("login item {operation} failed: {message}")]
    LoginItem {
        operation: &'static str,
        message: String,
    },
    #[error("launch at login is unavailable: {reason}")]
    LoginItemUnavailable { reason: String },
}

#[cfg(test)]
mod autostart_tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct RecordingAutostartAdapter {
        writes: Mutex<Vec<(PathBuf, String)>>,
        removes: Mutex<Vec<PathBuf>>,
        registry_sets: Mutex<Vec<(String, String, String)>>,
        registry_deletes: Mutex<Vec<(String, String)>>,
        login_items: Mutex<Vec<(String, bool)>>,
    }

    impl AutostartAdapter for RecordingAutostartAdapter {
        fn write_file(&self, path: &Path, contents: &str) -> Result<(), AutostartError> {
            self.writes
                .lock()
                .expect("writes")
                .push((path.to_path_buf(), contents.to_string()));
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> Result<(), AutostartError> {
            self.removes
                .lock()
                .expect("removes")
                .push(path.to_path_buf());
            Ok(())
        }

        fn set_windows_run_registry(
            &self,
            key_path: &str,
            value_name: &str,
            value: &str,
        ) -> Result<(), AutostartError> {
            self.registry_sets.lock().expect("registry_sets").push((
                key_path.to_string(),
                value_name.to_string(),
                value.to_string(),
            ));
            Ok(())
        }

        fn delete_windows_run_registry(
            &self,
            key_path: &str,
            value_name: &str,
        ) -> Result<(), AutostartError> {
            self.registry_deletes
                .lock()
                .expect("registry_deletes")
                .push((key_path.to_string(), value_name.to_string()));
            Ok(())
        }

        fn set_login_item(
            &self,
            plist_name: &str,
            enabled: bool,
        ) -> Result<LoginItemState, AutostartError> {
            self.login_items
                .lock()
                .expect("login_items")
                .push((plist_name.to_string(), enabled));
            Ok(if enabled {
                LoginItemState::Enabled
            } else {
                LoginItemState::NotRegistered
            })
        }
    }

    fn request(target_os: TargetOs, enabled: bool) -> AutostartRequest {
        AutostartRequest {
            target_os,
            enabled,
            app_name: AUTOSTART_APP_NAME.to_string(),
            executable: PathBuf::from("/opt/VoyaVPN/voyavpn"),
            home_dir: PathBuf::from("/home/alice"),
        }
    }

    #[test]
    fn only_the_login_flag_after_the_program_name_marks_an_autostart_launch() {
        assert!(launched_by_autostart(["voyavpn", "--autostart"]));
        assert!(launched_by_autostart([
            "voyavpn",
            "--verbose",
            "--autostart"
        ]));
        assert!(!launched_by_autostart(["voyavpn"]));
        assert!(!launched_by_autostart(["--autostart"]));
        assert!(!launched_by_autostart(["voyavpn", "--autostart=1"]));
    }

    #[test]
    fn autostart_linux_plan_writes_desktop_file() {
        let request = request(TargetOs::Linux, true);
        let actions = plan_autostart(&request);

        assert!(matches!(
            &actions[..],
            [AutostartAction::WriteFile { path, contents }]
            if path == Path::new("/home/alice/.config/autostart/VoyaVPN.desktop")
                && contents.contains("Exec=\"/opt/VoyaVPN/voyavpn\" --autostart\n")
        ));
    }

    fn legacy_cleanup(request: &AutostartRequest) -> AutostartAction {
        AutostartAction::RemoveFileBestEffort {
            path: legacy_macos_launch_agent_path(&request.home_dir, &request.app_name),
        }
    }

    #[test]
    fn autostart_macos_plan_registers_login_item_then_retires_legacy_agent() {
        let request = request(TargetOs::Macos, true);
        let actions = plan_autostart(&request);

        assert_eq!(
            actions,
            vec![
                AutostartAction::SetLoginItem {
                    plist_name: MACOS_LOGIN_ITEM_PLIST_NAME.to_string(),
                    enabled: true,
                },
                legacy_cleanup(&request),
            ]
        );
    }

    #[test]
    fn autostart_macos_disable_plan_unregisters_login_item() {
        let request = request(TargetOs::Macos, false);
        let actions = plan_autostart(&request);

        assert_eq!(
            actions.first(),
            Some(&AutostartAction::SetLoginItem {
                plist_name: MACOS_LOGIN_ITEM_PLIST_NAME.to_string(),
                enabled: false,
            })
        );
        assert_eq!(&actions[1..], &[legacy_cleanup(&request)]);
    }

    /// The store build may write nothing outside its bundle or run system
    /// tools, and the old LaunchAgent embedded the executable path in a shell
    /// command.
    #[test]
    fn autostart_macos_plan_writes_nothing_and_never_names_the_executable() {
        for enabled in [true, false] {
            let request = AutostartRequest {
                executable: PathBuf::from("/Applications/VoyaVPN $(touch owned).app/voyavpn"),
                ..request(TargetOs::Macos, enabled)
            };
            let actions = plan_autostart(&request);

            for action in &actions {
                assert!(
                    !matches!(
                        action,
                        AutostartAction::WriteFile { .. } | AutostartAction::RemoveFile { .. }
                    ),
                    "unexpected write or command {action:?}"
                );
                assert!(
                    !format!("{action:?}").contains("touch owned"),
                    "executable path leaked into {action:?}"
                );
            }
        }
    }

    #[test]
    fn autostart_macos_apply_registers_through_the_adapter() {
        let adapter = RecordingAutostartAdapter::default();
        apply_autostart(&adapter, &request(TargetOs::Macos, true)).expect("enable");
        apply_autostart(&adapter, &request(TargetOs::Macos, false)).expect("disable");

        assert_eq!(
            adapter.login_items.lock().expect("login_items").as_slice(),
            &[
                (MACOS_LOGIN_ITEM_PLIST_NAME.to_string(), true),
                (MACOS_LOGIN_ITEM_PLIST_NAME.to_string(), false),
            ]
        );
        assert!(adapter.writes.lock().expect("writes").is_empty());
    }

    /// Ties the bundled agent plist to the constants the app registers it by
    /// and to the flag that marks a login launch.
    #[test]
    fn bundled_macos_login_item_plist_matches_the_constants() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/desktop/src-tauri/native/macos/LaunchAgents")
            .join(MACOS_LOGIN_ITEM_PLIST_NAME);
        let plist = std::fs::read_to_string(&path).expect("bundled login item plist");

        assert_eq!(
            MACOS_LOGIN_ITEM_PLIST_NAME,
            format!("{MACOS_LOGIN_ITEM_LABEL}.plist")
        );
        assert!(plist.contains(&format!("<string>{MACOS_LOGIN_ITEM_LABEL}</string>")));
        assert!(plist.contains(&format!("<string>{AUTOSTART_ARG}</string>")));
    }

    #[test]
    fn autostart_windows_plan_sets_run_registry() {
        let request = AutostartRequest {
            target_os: TargetOs::Windows,
            enabled: true,
            app_name: AUTOSTART_APP_NAME.to_string(),
            executable: PathBuf::from(r"C:\Program Files\VoyaVPN\voyavpn.exe"),
            home_dir: PathBuf::from(r"C:\Users\Alice"),
        };
        let actions = plan_autostart(&request);

        assert!(matches!(
            &actions[..],
            [AutostartAction::SetWindowsRunRegistry {
                key_path,
                value_name,
                value
            }] if key_path == WINDOWS_RUN_KEY
                && value_name.starts_with("VoyaVPN_")
                && value == "\"C:\\Program Files\\VoyaVPN\\voyavpn.exe\" --autostart"
        ));
    }

    #[test]
    fn autostart_linux_desktop_entry_quotes_paths_with_reserved_characters() {
        let entry = linux_desktop_entry(
            AUTOSTART_APP_NAME,
            Path::new("/home/alice/My Apps/VoyaVPN 100% $edge.AppImage"),
        );

        assert!(
            entry.contains("Exec=\"/home/alice/My Apps/VoyaVPN 100%% \\\\$edge.AppImage\""),
            "unexpected Exec line: {entry}"
        );
    }

    #[test]
    fn autostart_linux_desktop_entry_escapes_quotes_backticks_and_backslashes() {
        let entry = linux_desktop_entry(AUTOSTART_APP_NAME, Path::new("/opt/a\"b`c\\d/voyavpn"));

        assert!(
            entry.contains("Exec=\"/opt/a\\\\\"b\\\\`c\\\\\\\\d/voyavpn\""),
            "unexpected Exec line: {entry}"
        );
    }

    #[test]
    fn autostart_uses_fake_adapter_for_linux_clear() {
        let adapter = Arc::new(RecordingAutostartAdapter::default());
        apply_autostart(adapter.as_ref(), &request(TargetOs::Linux, false))
            .expect("autostart apply");

        assert_eq!(
            adapter.removes.lock().expect("removes").as_slice(),
            &[PathBuf::from(
                "/home/alice/.config/autostart/VoyaVPN.desktop"
            )]
        );
    }
}
