//! Where the PacketTunnel component sits in the app bundle, whether it is
//! packaged the way its signing identity requires, and how macOS has
//! registered it (PlugInKit for the app extension, `systemextensionsctl` for
//! the system extension).

use super::*;

use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

/// `systemextensionsctl list` is a process spawn and the supervisor health
/// watcher asks for TUN status every few seconds; activation only changes when
/// the user approves or removes the extension, so a short cache is enough.
const SYSTEM_EXTENSION_STATE_TTL: Duration = Duration::from_secs(30);

#[cfg(target_os = "macos")]
fn macos_app_contents_dir() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    executable
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "Contents"))
        .map(Path::to_path_buf)
}

#[cfg(target_os = "macos")]
pub(in crate::tun) fn macos_packet_tunnel_appex_path() -> Option<PathBuf> {
    Some(
        macos_app_contents_dir()?
            .join("PlugIns")
            .join(MACOS_PACKET_TUNNEL_APPEX_NAME),
    )
}

#[cfg(not(target_os = "macos"))]
pub(in crate::tun) fn macos_packet_tunnel_appex_path() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn macos_packet_tunnel_sysex_path() -> Option<PathBuf> {
    Some(
        macos_app_contents_dir()?
            .join("Library")
            .join("SystemExtensions")
            .join(MACOS_PACKET_TUNNEL_SYSEX_NAME),
    )
}

#[cfg(not(target_os = "macos"))]
fn macos_packet_tunnel_sysex_path() -> Option<PathBuf> {
    None
}

pub(super) fn macos_packet_tunnel_component_path() -> Option<PathBuf> {
    if let Some(path) = macos_packet_tunnel_sysex_path().filter(|path| path.exists()) {
        return Some(path);
    }
    macos_packet_tunnel_appex_path()
}

/// The app bundle cannot change while the process runs, so the packaging shape
/// is resolved once instead of on every status query.
pub(super) fn macos_packet_tunnel_packaging_mode() -> Option<&'static str> {
    static PACKAGING_MODE: OnceLock<Option<&'static str>> = OnceLock::new();

    *PACKAGING_MODE.get_or_init(|| {
        if macos_packet_tunnel_sysex_path().is_some_and(|path| path.exists()) {
            return Some(PACKAGING_MODE_SYSTEM_EXTENSION);
        }
        if macos_packet_tunnel_appex_path().is_some_and(|path| path.exists()) {
            return Some(PACKAGING_MODE_APP_EXTENSION);
        }
        None
    })
}

/// `codesign -d --entitlements` is a process spawn against a bundle that cannot
/// change while the process runs, so its verdict is cached for the process.
pub(super) fn macos_packet_tunnel_packaging_error() -> Option<String> {
    static PACKAGING_ERROR: OnceLock<Option<String>> = OnceLock::new();

    PACKAGING_ERROR
        .get_or_init(probe_macos_packet_tunnel_packaging_error)
        .clone()
}

#[cfg(target_os = "macos")]
fn probe_macos_packet_tunnel_packaging_error() -> Option<String> {
    if macos_packet_tunnel_packaging_mode() != Some(PACKAGING_MODE_APP_EXTENSION) {
        return None;
    }
    let appex = macos_packet_tunnel_appex_path()?;
    let output = Command::new("/usr/bin/codesign")
        .args(["-d", "--entitlements", ":-"])
        .arg(&appex)
        .output()
        .ok()?;
    let text = command_output_text(&output.stdout, &output.stderr);
    if text.contains("packet-tunnel-provider-systemextension") {
        return Some(
            "Developer ID PacketTunnel builds must be packaged as Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension; re-run pnpm native:macos:tunnel with a Developer ID identity."
                .to_string(),
        );
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn probe_macos_packet_tunnel_packaging_error() -> Option<String> {
    None
}

pub fn parse_pluginkit_matches(output: &str) -> Vec<PathBuf> {
    // `pluginkit -i` already filters to this provider. Both folder names count:
    // a registration left by a build from before the rename must still surface,
    // so the provider-path precheck reports it instead of seeing nothing.
    let needles = [
        MACOS_PACKET_TUNNEL_APPEX_NAME,
        MACOS_PACKET_TUNNEL_LEGACY_APPEX_NAME,
    ];
    let mut paths = Vec::new();

    for line in output.lines() {
        let mut search_start = 0;
        while let Some((needle_start, needle)) = needles
            .iter()
            .filter_map(|needle| {
                line[search_start..]
                    .find(needle)
                    .map(|index| (search_start + index, *needle))
            })
            .min_by_key(|(start, _)| *start)
        {
            let needle_end = needle_start + needle.len();
            let prefix = &line[..needle_start];
            let Some(path_start) = prefix.find("file:/").or_else(|| prefix.find('/')) else {
                search_start = needle_end;
                continue;
            };
            let raw = line[path_start..needle_end]
                .trim()
                .trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ')' | ';'));
            let normalized = raw.strip_prefix("file://").unwrap_or(raw);
            let path = PathBuf::from(normalized);
            if !paths.iter().any(|existing| existing == &path) {
                paths.push(path);
            }
            search_start = needle_end;
        }
    }

    paths
}

pub fn parse_systemextensionsctl_matches(output: &str, bundle_id: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| line.contains(bundle_id))
        .map(str::to_string)
        .collect()
}

pub fn parse_systemextensionsctl_state(output: &str, bundle_id: &str) -> Option<String> {
    parse_systemextensionsctl_matches(output, bundle_id)
        .into_iter()
        .find_map(|line| {
            let start = line.rfind('[')?;
            let end = line[start + 1..].find(']')?;
            Some(line[start + 1..start + 1 + end].trim().to_string())
        })
}

#[cfg(target_os = "macos")]
fn macos_systemextensionsctl_output() -> Option<String> {
    let output = Command::new("/usr/bin/systemextensionsctl")
        .arg("list")
        .output()
        .ok()?;
    Some(command_output_text(&output.stdout, &output.stderr))
}

#[cfg(not(target_os = "macos"))]
fn macos_systemextensionsctl_output() -> Option<String> {
    None
}

pub(super) fn macos_system_extension_state() -> Option<String> {
    parse_systemextensionsctl_state(
        &macos_systemextensionsctl_output()?,
        MACOS_PACKET_TUNNEL_BUNDLE_ID,
    )
}

pub(super) fn macos_system_extension_is_activated() -> bool {
    static ACTIVATED: OnceLock<Mutex<Option<(Instant, bool)>>> = OnceLock::new();

    let probe = || {
        macos_system_extension_state()
            .as_deref()
            .is_some_and(|state| state.contains("activated") && state.contains("enabled"))
    };
    let Ok(mut cached) = ACTIVATED.get_or_init(|| Mutex::new(None)).lock() else {
        return probe();
    };
    if let Some((checked_at, activated)) = *cached {
        if checked_at.elapsed() < SYSTEM_EXTENSION_STATE_TTL {
            return activated;
        }
    }

    let activated = probe();
    *cached = Some((Instant::now(), activated));
    activated
}

pub(super) fn macos_system_extension_registration_lines() -> Vec<String> {
    let Some(output) = macos_systemextensionsctl_output() else {
        return Vec::new();
    };
    parse_systemextensionsctl_matches(&output, MACOS_PACKET_TUNNEL_BUNDLE_ID)
}

#[cfg(target_os = "macos")]
pub(in crate::tun) fn platform_provider_registration_paths(
    bundle_id: &str,
) -> Result<Vec<PathBuf>, NativeTunError> {
    let output = Command::new("/usr/bin/pluginkit")
        .args(["-mAvvv", "-i", bundle_id])
        .output()
        .map_err(|source| NativeTunError::Command {
            action: "query macOS PacketTunnel provider registration",
            source,
        })?;

    if !output.status.success() {
        if String::from_utf8_lossy(&output.stderr).contains("unauthorized discovery flag") {
            return Err(NativeTunError::RegistrationUnavailable);
        }
        return Err(NativeTunError::CommandFailed {
            action: "query macOS PacketTunnel provider registration",
            status_code: output.status.code(),
            output: command_output_text(&output.stdout, &output.stderr),
        });
    }

    Ok(parse_pluginkit_matches(&command_output_text(
        &output.stdout,
        &output.stderr,
    )))
}

#[cfg(not(target_os = "macos"))]
pub(in crate::tun) fn platform_provider_registration_paths(
    _bundle_id: &str,
) -> Result<Vec<PathBuf>, NativeTunError> {
    Ok(Vec::new())
}

pub fn ensure_macos_provider_path_matches(
    resolver: &dyn ProviderRegistrationResolver,
) -> Result<(), NativeTunError> {
    let Some(expected) = resolver.expected_provider_path(MACOS_PACKET_TUNNEL_BUNDLE_ID) else {
        return Ok(());
    };

    let resolved = match resolver.resolved_provider_paths(MACOS_PACKET_TUNNEL_BUNDLE_ID) {
        Ok(resolved) => resolved,
        Err(NativeTunError::RegistrationUnavailable) => return Ok(()),
        Err(error) => {
            tracing::warn!(
                ?error,
                "failed to query macOS PacketTunnel provider registration; continuing"
            );
            return Ok(());
        }
    };
    if resolved.is_empty() {
        return Ok(());
    }

    if resolved
        .iter()
        .any(|candidate| paths_equivalent(candidate, &expected))
    {
        return Ok(());
    }

    let resolved = resolved
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("<unknown>"));
    Err(NativeTunError::ProviderPathMismatch { expected, resolved })
}

fn paths_equivalent(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}
