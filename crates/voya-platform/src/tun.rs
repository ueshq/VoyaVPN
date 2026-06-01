use std::io;

#[cfg(windows)]
use std::process::Command;

use thiserror::Error;

use crate::coreinfo::TargetOs;

pub const WINDOWS_TUN_DEVICES: &[WindowsTunDevice] = &[
    WindowsTunDevice {
        name: "wintunsingbox_tun",
        guid: "b738a021-9842-444c-10b0-a4e3f65ab5b6",
    },
    WindowsTunDevice {
        name: "xray_tun",
        guid: "7d7d9015-6c82-d838-2430-f41b93b28148",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsTunDevice {
    pub name: &'static str,
    pub guid: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunPreflightState {
    Ready,
    NeedsSudoPassword,
    ManualCheck,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunPreflightReport {
    pub os: TargetOs,
    pub state: TunPreflightState,
    pub allow_enable_tun: bool,
    pub requires_sudo_password: bool,
    pub sudo_password_present: bool,
    pub notes: Vec<String>,
    pub route_restore_note: String,
    pub windows_cleanup_devices: Vec<WindowsTunDevice>,
}

#[must_use]
pub const fn allow_enable_tun(os: TargetOs, sudo_password_present: bool) -> bool {
    match os {
        TargetOs::Windows => true,
        TargetOs::Linux | TargetOs::Macos => sudo_password_present,
        TargetOs::Other => false,
    }
}

#[must_use]
pub fn tun_preflight(os: TargetOs, sudo_password_present: bool) -> TunPreflightReport {
    let requires_sudo_password = matches!(os, TargetOs::Linux | TargetOs::Macos);
    let allow_enable_tun = allow_enable_tun(os, sudo_password_present);
    let state = match os {
        TargetOs::Windows => TunPreflightState::ManualCheck,
        TargetOs::Linux | TargetOs::Macos if sudo_password_present => TunPreflightState::Ready,
        TargetOs::Linux | TargetOs::Macos => TunPreflightState::NeedsSudoPassword,
        TargetOs::Other => TunPreflightState::Unsupported,
    };

    TunPreflightReport {
        os,
        state,
        allow_enable_tun,
        requires_sudo_password,
        sudo_password_present,
        notes: tun_preflight_notes(os, sudo_password_present),
        route_restore_note: route_restore_note(os).to_string(),
        windows_cleanup_devices: if os == TargetOs::Windows {
            WINDOWS_TUN_DEVICES.to_vec()
        } else {
            Vec::new()
        },
    }
}

fn tun_preflight_notes(os: TargetOs, sudo_password_present: bool) -> Vec<String> {
    match os {
        TargetOs::Windows => vec![
            "Windows TUN start runs stale Wintun device cleanup before launching the core."
                .to_string(),
            "UAC and Windows job containment are runtime-managed; driver install still needs a manual OS smoke check."
                .to_string(),
        ],
        TargetOs::Linux | TargetOs::Macos if sudo_password_present => vec![
            "Unix TUN start will reuse the in-memory sudo password collected at enable time."
                .to_string(),
            "The elevated process is killed first during disconnect before regular process teardown."
                .to_string(),
        ],
        TargetOs::Linux | TargetOs::Macos => vec![
            "Unix TUN start requires a non-empty sudo password collected before enabling TUN."
                .to_string(),
        ],
        TargetOs::Other => vec!["TUN mode is not supported on this platform yet.".to_string()],
    }
}

fn route_restore_note(os: TargetOs) -> &'static str {
    match os {
        TargetOs::Windows => {
            "Disconnect stops the job-owned core processes; Windows TUN device and route restoration must be confirmed in manual OS smoke."
        }
        TargetOs::Linux | TargetOs::Macos => {
            "Disconnect runs sudo kill for elevated TUN cores before normal teardown so core-owned routes can be restored by process exit."
        }
        TargetOs::Other => "No route mutation is attempted on unsupported platforms.",
    }
}

pub trait TunCleaner: Send + Sync {
    fn cleanup_before_start(&self) -> Result<(), TunCleanupError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTunCleaner;

impl TunCleaner for NoopTunCleaner {
    fn cleanup_before_start(&self) -> Result<(), TunCleanupError> {
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformTunCleaner;

impl TunCleaner for PlatformTunCleaner {
    fn cleanup_before_start(&self) -> Result<(), TunCleanupError> {
        platform_cleanup_before_start()
    }
}

#[cfg(windows)]
fn platform_cleanup_before_start() -> Result<(), TunCleanupError> {
    for device in WINDOWS_TUN_DEVICES {
        let output = Command::new(r"C:\Windows\System32\pnputil.exe")
            .args([
                "/remove-device",
                &format!(r"SWD\Wintun\{{{}}}", device.guid),
            ])
            .output()
            .map_err(TunCleanupError::Command)?;

        if !output.status.success() {
            tracing::warn!(
                device = device.name,
                status = ?output.status.code(),
                stderr = %String::from_utf8_lossy(&output.stderr),
                "failed to remove stale Windows TUN device"
            );
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn platform_cleanup_before_start() -> Result<(), TunCleanupError> {
    Ok(())
}

#[derive(Debug, Error)]
pub enum TunCleanupError {
    #[error("failed to run Windows TUN cleanup command: {0}")]
    Command(io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_windows_tun_cleanup_abstraction_names_reference_devices() {
        assert_eq!(WINDOWS_TUN_DEVICES.len(), 2);
        assert_eq!(WINDOWS_TUN_DEVICES[0].name, "wintunsingbox_tun");
        assert_eq!(WINDOWS_TUN_DEVICES[1].name, "xray_tun");
        assert!(WINDOWS_TUN_DEVICES
            .iter()
            .all(|device| device.guid.len() == 36));
    }

    #[test]
    fn process_noop_tun_cleaner_is_deterministic_for_tests() {
        NoopTunCleaner.cleanup_before_start().expect("noop cleanup");
    }

    #[test]
    fn tun_allow_enable_on_unix_requires_stored_sudo_password() {
        assert!(!allow_enable_tun(TargetOs::Linux, false));
        assert!(allow_enable_tun(TargetOs::Linux, true));
        assert!(!allow_enable_tun(TargetOs::Macos, false));
        assert!(allow_enable_tun(TargetOs::Macos, true));
        assert!(allow_enable_tun(TargetOs::Windows, false));
    }

    #[test]
    fn tun_preflight_reports_driver_and_restore_notes_by_platform() {
        let windows = tun_preflight(TargetOs::Windows, false);
        assert_eq!(windows.state, TunPreflightState::ManualCheck);
        assert_eq!(windows.windows_cleanup_devices, WINDOWS_TUN_DEVICES);
        assert!(windows.route_restore_note.contains("manual OS smoke"));

        let linux_missing = tun_preflight(TargetOs::Linux, false);
        assert_eq!(linux_missing.state, TunPreflightState::NeedsSudoPassword);
        assert!(!linux_missing.allow_enable_tun);

        let linux_ready = tun_preflight(TargetOs::Linux, true);
        assert_eq!(linux_ready.state, TunPreflightState::Ready);
        assert!(linux_ready.allow_enable_tun);
        assert!(linux_ready.route_restore_note.contains("sudo kill"));
    }
}
