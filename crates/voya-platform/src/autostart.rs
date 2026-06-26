use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use thiserror::Error;

use crate::{
    coreinfo::TargetOs,
    process::{ProcessError, ProcessRole, ProcessRunner, ProcessSpawn},
};

pub const AUTOSTART_APP_NAME: &str = "VoyaVPN";
pub const WINDOWS_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
pub const LINUX_AUTOSTART_DIR: &str = ".config/autostart";
pub const MACOS_LAUNCH_AGENTS_DIR: &str = "Library/LaunchAgents";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartRequest {
    pub target_os: TargetOs,
    pub enabled: bool,
    pub app_name: String,
    pub executable: PathBuf,
    pub home_dir: PathBuf,
}

impl AutostartRequest {
    #[must_use]
    pub fn artifact(&self) -> Option<AutostartArtifact> {
        autostart_artifact(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutostartArtifact {
    WindowsRunRegistry {
        key_path: String,
        value_name: String,
        value: String,
    },
    LinuxDesktopFile {
        path: PathBuf,
    },
    MacosLaunchAgent {
        path: PathBuf,
        label: String,
    },
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
    RunCommand {
        executable: PathBuf,
        arguments: Vec<String>,
    },
    RunCommandBestEffort {
        executable: PathBuf,
        arguments: Vec<String>,
    },
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutostartPlan {
    pub target_os: TargetOs,
    pub enabled: bool,
    pub artifact: Option<AutostartArtifact>,
    pub actions: Vec<AutostartAction>,
}

pub trait AutostartAdapter: Send + Sync {
    fn write_file(&self, path: &Path, contents: &str) -> Result<(), AutostartError>;
    fn remove_file(&self, path: &Path) -> Result<(), AutostartError>;
    fn run_command(&self, executable: &Path, arguments: &[String]) -> Result<(), AutostartError>;
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
}

#[derive(Clone)]
pub struct AutostartService {
    adapter: Arc<dyn AutostartAdapter>,
}

impl AutostartService {
    #[must_use]
    pub fn new(adapter: Arc<dyn AutostartAdapter>) -> Self {
        Self { adapter }
    }

    pub fn apply(&self, request: &AutostartRequest) -> Result<AutostartPlan, AutostartError> {
        let plan = plan_autostart(request);
        for action in &plan.actions {
            match action {
                AutostartAction::SetWindowsRunRegistry {
                    key_path,
                    value_name,
                    value,
                } => self
                    .adapter
                    .set_windows_run_registry(key_path, value_name, value)?,
                AutostartAction::DeleteWindowsRunRegistry {
                    key_path,
                    value_name,
                } => self
                    .adapter
                    .delete_windows_run_registry(key_path, value_name)?,
                AutostartAction::WriteFile { path, contents } => {
                    self.adapter.write_file(path, contents)?;
                }
                AutostartAction::RemoveFile { path } => {
                    self.adapter.remove_file(path)?;
                }
                AutostartAction::RunCommand {
                    executable,
                    arguments,
                } => self.adapter.run_command(executable, arguments)?,
                AutostartAction::RunCommandBestEffort {
                    executable,
                    arguments,
                } => {
                    if let Err(error) = self.adapter.run_command(executable, arguments) {
                        tracing::debug!(
                            %error,
                            executable = %executable.display(),
                            "ignored autostart cleanup command failure"
                        );
                    }
                }
                AutostartAction::Noop => {}
            }
        }

        Ok(plan)
    }
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AutostartError::Io {
                operation: "create autostart directory",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(path, contents).map_err(|source| AutostartError::Io {
            operation: "write autostart file",
            path: path.to_path_buf(),
            source,
        })
    }

    fn remove_file(&self, path: &Path) -> Result<(), AutostartError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(AutostartError::Io {
                operation: "remove autostart file",
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    fn run_command(&self, executable: &Path, arguments: &[String]) -> Result<(), AutostartError> {
        run_checked(&*self.runner, executable, arguments)
    }

    fn set_windows_run_registry(
        &self,
        key_path: &str,
        value_name: &str,
        value: &str,
    ) -> Result<(), AutostartError> {
        let arguments = vec![
            "add".to_string(),
            key_path.to_string(),
            "/v".to_string(),
            value_name.to_string(),
            "/t".to_string(),
            "REG_SZ".to_string(),
            "/d".to_string(),
            value.to_string(),
            "/f".to_string(),
        ];
        run_checked(&*self.runner, Path::new("reg"), &arguments)
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
}

#[must_use]
pub fn plan_autostart(request: &AutostartRequest) -> AutostartPlan {
    let artifact = autostart_artifact(request);
    let actions = match request.target_os {
        TargetOs::Windows => windows_actions(request),
        TargetOs::Linux => linux_actions(request),
        TargetOs::Macos => macos_actions(request),
        TargetOs::Other => vec![AutostartAction::Noop],
    };

    AutostartPlan {
        target_os: request.target_os,
        enabled: request.enabled,
        artifact,
        actions,
    }
}

#[must_use]
pub fn windows_value_name(app_name: &str, executable: &Path) -> String {
    format!(
        "{app_name}_{}",
        fnv1a_hex(executable.to_string_lossy().as_bytes())
    )
}

#[must_use]
pub fn linux_desktop_entry(app_name: &str, executable: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nExec={}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\nName[en_US]={app_name}\nName={app_name}\nComment[en_US]={app_name}\nComment={app_name}\n",
        executable.display()
    )
}

#[must_use]
pub fn macos_launch_agent_plist(app_name: &str, executable: &Path) -> String {
    let label = macos_label(app_name);
    let process_name = executable
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(app_name);
    let executable = shell_quoted_xml(&executable.to_string_lossy());
    let process_name = shell_quoted_xml(process_name);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/sh</string>
        <string>-c</string>
        <string>if ! pgrep -x {process_name} &gt; /dev/null; then {executable}; fi</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#
    )
}

fn autostart_artifact(request: &AutostartRequest) -> Option<AutostartArtifact> {
    match request.target_os {
        TargetOs::Windows => Some(AutostartArtifact::WindowsRunRegistry {
            key_path: WINDOWS_RUN_KEY.to_string(),
            value_name: windows_value_name(&request.app_name, &request.executable),
            value: quote_windows_path(&request.executable),
        }),
        TargetOs::Linux => Some(AutostartArtifact::LinuxDesktopFile {
            path: linux_autostart_path(&request.home_dir, &request.app_name),
        }),
        TargetOs::Macos => Some(AutostartArtifact::MacosLaunchAgent {
            path: macos_launch_agent_path(&request.home_dir, &request.app_name),
            label: macos_label(&request.app_name),
        }),
        TargetOs::Other => None,
    }
}

fn windows_actions(request: &AutostartRequest) -> Vec<AutostartAction> {
    let value_name = windows_value_name(&request.app_name, &request.executable);
    if request.enabled {
        vec![AutostartAction::SetWindowsRunRegistry {
            key_path: WINDOWS_RUN_KEY.to_string(),
            value_name,
            value: quote_windows_path(&request.executable),
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

fn macos_actions(request: &AutostartRequest) -> Vec<AutostartAction> {
    let path = macos_launch_agent_path(&request.home_dir, &request.app_name);
    if request.enabled {
        vec![
            launchctl_best_effort_action("unload", &path),
            AutostartAction::WriteFile {
                path: path.clone(),
                contents: macos_launch_agent_plist(&request.app_name, &request.executable),
            },
            launchctl_action("load", &path),
        ]
    } else {
        vec![
            launchctl_best_effort_action("unload", &path),
            AutostartAction::RemoveFile { path },
        ]
    }
}

fn launchctl_action(command: &str, path: &Path) -> AutostartAction {
    AutostartAction::RunCommand {
        executable: PathBuf::from("launchctl"),
        arguments: launchctl_arguments(command, path),
    }
}

fn launchctl_best_effort_action(command: &str, path: &Path) -> AutostartAction {
    AutostartAction::RunCommandBestEffort {
        executable: PathBuf::from("launchctl"),
        arguments: launchctl_arguments(command, path),
    }
}

fn launchctl_arguments(command: &str, path: &Path) -> Vec<String> {
    vec![
        command.to_string(),
        "-w".to_string(),
        path.to_string_lossy().into_owned(),
    ]
}

fn linux_autostart_path(home_dir: &Path, app_name: &str) -> PathBuf {
    home_dir
        .join(LINUX_AUTOSTART_DIR)
        .join(format!("{app_name}.desktop"))
}

fn macos_launch_agent_path(home_dir: &Path, app_name: &str) -> PathBuf {
    home_dir
        .join(MACOS_LAUNCH_AGENTS_DIR)
        .join(format!("{app_name}-LaunchAgent.plist"))
}

fn macos_label(app_name: &str) -> String {
    format!("{app_name}-LaunchAgent")
}

fn quote_windows_path(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

fn run_checked(
    runner: &dyn ProcessRunner,
    executable: &Path,
    arguments: &[String],
) -> Result<(), AutostartError> {
    let output = runner.run_oneshot(
        ProcessSpawn::new(ProcessRole::Autostart, executable.to_path_buf())
            .with_arguments(arguments.to_vec())
            .with_display_log(false),
    )?;
    if output.status_code == Some(0) {
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

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn shell_quoted_xml(value: &str) -> String {
    xml_escape(&shell_single_quote(value))
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

#[derive(Debug, Error)]
pub enum AutostartError {
    #[error("unsupported autostart target OS {0:?}")]
    Unsupported(TargetOs),
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
}

#[cfg(test)]
mod autostart_tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct RecordingAutostartAdapter {
        writes: Mutex<Vec<(PathBuf, String)>>,
        removes: Mutex<Vec<PathBuf>>,
        commands: Mutex<Vec<(PathBuf, Vec<String>)>>,
        registry_sets: Mutex<Vec<(String, String, String)>>,
        registry_deletes: Mutex<Vec<(String, String)>>,
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

        fn run_command(
            &self,
            executable: &Path,
            arguments: &[String],
        ) -> Result<(), AutostartError> {
            self.commands
                .lock()
                .expect("commands")
                .push((executable.to_path_buf(), arguments.to_vec()));
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

    fn string_args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn autostart_linux_plan_writes_desktop_file() {
        let request = request(TargetOs::Linux, true);
        let plan = plan_autostart(&request);

        assert_eq!(
            plan.artifact,
            Some(AutostartArtifact::LinuxDesktopFile {
                path: PathBuf::from("/home/alice/.config/autostart/VoyaVPN.desktop")
            })
        );
        assert!(matches!(
            &plan.actions[..],
            [AutostartAction::WriteFile { path, contents }]
            if path.ends_with("VoyaVPN.desktop") && contents.contains("Exec=/opt/VoyaVPN/voyavpn")
        ));
    }

    #[test]
    fn autostart_macos_plan_loads_launch_agent() {
        let request = request(TargetOs::Macos, true);
        let plan = plan_autostart(&request);
        // Derive the plist path with the same helper production uses so the
        // expected launchctl arguments stay portable across path separators.
        let plist_arg = macos_launch_agent_path(&request.home_dir, &request.app_name)
            .to_string_lossy()
            .into_owned();

        assert_eq!(plan.actions.len(), 3);
        assert!(matches!(
            &plan.actions[0],
            AutostartAction::RunCommandBestEffort {
                executable,
                arguments,
            } if executable == Path::new("launchctl")
                && arguments == &string_args(&["unload", "-w", plist_arg.as_str()])
        ));
        assert!(matches!(
            &plan.actions[1],
            AutostartAction::WriteFile { path, contents }
            if path.ends_with("VoyaVPN-LaunchAgent.plist")
                && contents.contains("<string>VoyaVPN-LaunchAgent</string>")
        ));
        assert!(matches!(
            &plan.actions[2],
            AutostartAction::RunCommand { executable, arguments }
            if executable == Path::new("launchctl")
                && arguments == &string_args(&["load", "-w", plist_arg.as_str()])
        ));
    }

    #[test]
    fn autostart_macos_shell_metacharacters_do_not_escape_path_arguments() {
        let executable =
            PathBuf::from("/Applications/VoyaVPN $(touch owned) \"quote\" 'apostrophe'/voyavpn");
        let request = AutostartRequest {
            target_os: TargetOs::Macos,
            enabled: true,
            app_name: AUTOSTART_APP_NAME.to_string(),
            executable: executable.clone(),
            home_dir: PathBuf::from("/Users/alice; touch owned"),
        };
        // Derive the plist path the same way production does so the expected
        // launchctl arguments stay portable across path separators.
        let path = macos_launch_agent_path(&request.home_dir, &request.app_name)
            .to_string_lossy()
            .into_owned();
        let plan = plan_autostart(&request);
        let executable_text = executable.to_string_lossy();
        let quoted_executable = shell_quoted_xml(&executable_text);
        let contents = match &plan.actions[1] {
            AutostartAction::WriteFile { contents, .. } => contents,
            action => panic!("expected plist write action, got {action:?}"),
        };

        assert!(matches!(
            &plan.actions[0],
            AutostartAction::RunCommandBestEffort {
                executable,
                arguments,
            } if executable == Path::new("launchctl")
                && arguments == &string_args(&["unload", "-w", path.as_str()])
        ));
        assert!(
            contents.contains(&format!("then {quoted_executable}; fi</string>")),
            "plist did not include shell-quoted executable path:\n{contents}"
        );
        assert!(
            !contents.contains(&format!(
                "then &quot;{}&quot;;",
                xml_escape(&executable_text)
            )),
            "plist still used a double-quoted shell path:\n{contents}"
        );
        assert!(matches!(
            &plan.actions[2],
            AutostartAction::RunCommand {
                executable,
                arguments,
            } if executable == Path::new("launchctl")
                && arguments == &string_args(&["load", "-w", path.as_str()])
        ));
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
        let plan = plan_autostart(&request);

        assert!(matches!(
            &plan.actions[..],
            [AutostartAction::SetWindowsRunRegistry {
                key_path,
                value_name,
                value
            }] if key_path == WINDOWS_RUN_KEY
                && value_name.starts_with("VoyaVPN_")
                && value == "\"C:\\Program Files\\VoyaVPN\\voyavpn.exe\""
        ));
    }

    #[test]
    fn autostart_service_uses_fake_adapter_for_linux_clear() {
        let adapter = Arc::new(RecordingAutostartAdapter::default());
        let service = AutostartService::new(adapter.clone());

        let plan = service
            .apply(&request(TargetOs::Linux, false))
            .expect("autostart apply");

        assert!(!plan.enabled);
        assert_eq!(
            adapter.removes.lock().expect("removes").as_slice(),
            &[PathBuf::from(
                "/home/alice/.config/autostart/VoyaVPN.desktop"
            )]
        );
    }
}
