use super::*;

use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

mod bridge;
use bridge::{
    macos_packet_tunnel_bridge_container_path, macos_packet_tunnel_bridge_last_error,
    macos_packet_tunnel_bridge_status,
};
#[cfg(target_os = "macos")]
use bridge::{macos_packet_tunnel_bridge_start, macos_packet_tunnel_bridge_stop};

/// `systemextensionsctl list` is a process spawn and the supervisor health
/// watcher asks for TUN status every few seconds; activation only changes when
/// the user approves or removes the extension, so a short cache is enough.
const SYSTEM_EXTENSION_STATE_TTL: Duration = Duration::from_secs(30);

pub(super) fn macos_packet_tunnel_status() -> NativeTunStatus {
    if let Err(status) = require_bundled_component(
        macos_packet_tunnel_component_path(),
        "PacketTunnel extension is not bundled in this build",
    ) {
        return status;
    }

    if let Some(message) = macos_packet_tunnel_packaging_error() {
        return NativeTunStatus {
            backend: TunBackend::MacosPacketTunnel,
            provider_state: NativeTunProviderState::Error,
            component_ready: false,
            message: Some(message),
        };
    }

    if macos_packet_tunnel_packaging_mode() == Some("systemExtension")
        && !macos_system_extension_is_activated()
    {
        return NativeTunStatus {
            backend: TunBackend::MacosPacketTunnel,
            provider_state: NativeTunProviderState::PermissionRequired,
            component_ready: true,
            message: Some(
                "Approve the VoyaVPN PacketTunnel system extension in System Settings, then enable TUN again."
                    .to_string(),
            ),
        };
    }

    match macos_packet_tunnel_bridge_status() {
        Ok(output) if output.starts_with("error:") => NativeTunStatus {
            backend: TunBackend::MacosPacketTunnel,
            provider_state: NativeTunProviderState::Error,
            component_ready: true,
            message: Some(macos_packet_tunnel_status_message(
                Some(output.trim_start_matches("error:").to_string()),
                NativeTunProviderState::Error,
            )),
        },
        Ok(output) => {
            let provider_state = parse_macos_provider_state(&output);
            NativeTunStatus {
                backend: TunBackend::MacosPacketTunnel,
                provider_state,
                component_ready: true,
                message: macos_packet_tunnel_terminal_message(provider_state),
            }
        }
        Err(error) => NativeTunStatus {
            backend: TunBackend::MacosPacketTunnel,
            provider_state: NativeTunProviderState::Error,
            component_ready: true,
            message: Some(macos_packet_tunnel_status_message(
                Some(error.to_string()),
                NativeTunProviderState::Error,
            )),
        },
    }
}

fn macos_packet_tunnel_terminal_message(provider_state: NativeTunProviderState) -> Option<String> {
    if matches!(
        provider_state,
        NativeTunProviderState::Stopped | NativeTunProviderState::Error
    ) {
        let message = macos_packet_tunnel_status_message(None, provider_state);
        if !message.is_empty() {
            return Some(message);
        }
    }
    None
}

/// Status must stay cheap: it runs on every `tun_status` IPC, on every TUN
/// enable, and on every tick of the supervisor health watcher. Only the bridge
/// last-error and the provider's own status file are consulted here — both are
/// local reads. Registration evidence, the host log tail and `codesign` output
/// belong to `diagnostics()` behind the `tun_provider_diagnostics` IPC, which
/// the UI calls on demand when it has to explain a Stopped or Error state.
fn macos_packet_tunnel_status_message(
    base: Option<String>,
    provider_state: NativeTunProviderState,
) -> String {
    let mut messages = Vec::new();
    push_unique_message(&mut messages, base);
    if matches!(
        provider_state,
        NativeTunProviderState::Stopped | NativeTunProviderState::Error
    ) {
        push_unique_message(&mut messages, macos_packet_tunnel_last_error());
        if let Some(status) = macos_packet_tunnel_status_file() {
            push_unique_message(&mut messages, status.last_error);
        }
    }

    messages.join("; ")
}

/// Reads the provider-written status JSON from the App Group container. This is
/// a container-path lookup plus one small file read, so it is safe on the
/// status path, unlike the process spawns in `macos_packet_tunnel_diagnostics`.
fn macos_packet_tunnel_status_file() -> Option<NativeTunProviderStatusFile> {
    let status_path =
        macos_packet_tunnel_container_path()?.join(MACOS_PROVIDER_STATUS_RELATIVE_PATH);
    let status_text = fs::read_to_string(status_path).ok()?;
    parse_provider_status_json(&status_text).ok()
}

fn push_unique_message(messages: &mut Vec<String>, message: Option<String>) {
    let Some(message) = message.map(|value| value.trim().to_string()) else {
        return;
    };
    if message.is_empty() || messages.iter().any(|existing| existing == &message) {
        return;
    }
    messages.push(message);
}

pub fn parse_provider_status_json(
    input: &str,
) -> Result<NativeTunProviderStatusFile, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(input)?;
    let breadcrumbs = value
        .get("breadcrumbs")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Ok(NativeTunProviderStatusFile {
        state: json_string_field(&value, "state"),
        last_error: json_string_field(&value, "lastError"),
        provider_bundle_path: json_string_field(&value, "providerBundlePath"),
        breadcrumbs,
    })
}

fn json_string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn macos_packet_tunnel_diagnostics() -> NativeTunDiagnostics {
    let mut diagnostics = NativeTunDiagnostics::empty(TunBackend::MacosPacketTunnel);
    diagnostics.packaging_mode = macos_packet_tunnel_packaging_mode().map(str::to_string);
    diagnostics.expected_provider_path = macos_packet_tunnel_component_path();
    diagnostics.system_extension_state = macos_system_extension_state();
    diagnostics.registration_paths = macos_packet_tunnel_registration_evidence();
    diagnostics.host_log_tail = macos_packet_tunnel_host_log_tail();

    if let Some(message) = macos_packet_tunnel_packaging_error() {
        diagnostics.message = Some(message);
    } else if diagnostics.packaging_mode.as_deref() == Some("systemExtension")
        && !macos_system_extension_is_activated()
    {
        diagnostics.message = Some(
            "VoyaVPN PacketTunnel system extension is not activated; approve it in System Settings, then enable TUN again."
                .to_string(),
        );
    }

    let Some(container_path) = macos_packet_tunnel_container_path() else {
        return diagnostics;
    };

    let status_path = container_path.join(MACOS_PROVIDER_STATUS_RELATIVE_PATH);
    let log_path = container_path.join(MACOS_PROVIDER_LOG_RELATIVE_PATH);
    diagnostics.container_path = Some(container_path);
    diagnostics.status_path = Some(status_path.clone());
    diagnostics.log_path = Some(log_path.clone());

    if let Ok(status_text) = fs::read_to_string(&status_path) {
        match parse_provider_status_json(&status_text) {
            Ok(status) => {
                diagnostics.status = Some(status);
            }
            Err(error) => {
                diagnostics.message = Some(format!(
                    "failed to parse PacketTunnel provider status {}: {error}",
                    status_path.display()
                ));
            }
        }
    }

    if let Ok(log_text) = fs::read_to_string(&log_path) {
        diagnostics.provider_log_tail = tail_lines(&log_text, PROVIDER_LOG_TAIL_LINES);
    }

    diagnostics
}

fn macos_packet_tunnel_registration_evidence() -> Vec<String> {
    let mut evidence = Vec::new();
    if let Ok(paths) = platform_provider_registration_paths(MACOS_PACKET_TUNNEL_BUNDLE_ID) {
        evidence.extend(paths.into_iter().map(|path| path.display().to_string()));
    }
    evidence.extend(macos_system_extension_registration_lines());
    evidence
}

#[cfg(target_os = "macos")]
fn macos_packet_tunnel_host_log_tail() -> Vec<String> {
    let output = Command::new("/usr/bin/log")
        .args([
            "show",
            "--last",
            "15m",
            "--style",
            "compact",
            "--predicate",
            "process == \"VoyaPacketTunnel\" OR processImagePath CONTAINS \"VoyaPacketTunnel\" OR (process == \"nesessionmanager\" AND (eventMessage CONTAINS \"app.voyavpn.desktop.PacketTunnel\" OR eventMessage CONTAINS \"VoyaVPN\" OR eventMessage CONTAINS \"Validation failed\" OR eventMessage CONTAINS \"Signature check failed\"))",
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    // The sandboxed host app cannot run the `log` CLI ("Cannot run while
    // sandboxed" on stderr); surface nothing instead of the denial text.
    if !output.status.success() {
        return Vec::new();
    }
    tail_lines(
        &command_output_text(&output.stdout, &output.stderr),
        PROVIDER_LOG_TAIL_LINES,
    )
}

#[cfg(not(target_os = "macos"))]
fn macos_packet_tunnel_host_log_tail() -> Vec<String> {
    Vec::new()
}

fn tail_lines(text: &str, limit: usize) -> Vec<String> {
    let mut lines = text
        .lines()
        .rev()
        .take(limit)
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.reverse();
    lines
}

fn macos_packet_tunnel_last_error() -> Option<String> {
    macos_packet_tunnel_bridge_last_error()
        .ok()
        .and_then(normalize_bridge_optional_output)
}

fn macos_packet_tunnel_container_path() -> Option<PathBuf> {
    macos_packet_tunnel_bridge_container_path()
        .ok()
        .and_then(normalize_bridge_optional_output)
        .map(PathBuf::from)
}

fn normalize_bridge_optional_output(output: String) -> Option<String> {
    let value = output.trim();
    if value.is_empty() || value.starts_with("error:") {
        return None;
    }
    Some(value.to_string())
}

/// Resolve a bundled macOS PacketTunnel component path, returning a
/// `missing_component` status when it is absent from the build.
fn require_bundled_component(
    path: Option<PathBuf>,
    missing_message: &'static str,
) -> Result<PathBuf, NativeTunStatus> {
    match path {
        Some(path) if path.exists() => Ok(path),
        _ => Err(NativeTunStatus::missing_component(
            TunBackend::MacosPacketTunnel,
            missing_message,
        )),
    }
}

pub(super) fn parse_macos_provider_state(output: &str) -> NativeTunProviderState {
    match output.trim() {
        "running" => NativeTunProviderState::Running,
        "starting" => NativeTunProviderState::Starting,
        "stopped" => NativeTunProviderState::Stopped,
        "permissionRequired" => NativeTunProviderState::PermissionRequired,
        "missingComponent" => NativeTunProviderState::MissingComponent,
        "notApplicable" => NativeTunProviderState::NotApplicable,
        _ => NativeTunProviderState::Error,
    }
}

#[cfg(target_os = "macos")]
fn macos_app_contents_dir() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    executable
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "Contents"))
        .map(Path::to_path_buf)
}

#[cfg(target_os = "macos")]
pub(super) fn macos_packet_tunnel_appex_path() -> Option<PathBuf> {
    Some(
        macos_app_contents_dir()?
            .join("PlugIns")
            .join(MACOS_PACKET_TUNNEL_APPEX_NAME),
    )
}

#[cfg(not(target_os = "macos"))]
pub(super) fn macos_packet_tunnel_appex_path() -> Option<PathBuf> {
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

fn macos_packet_tunnel_component_path() -> Option<PathBuf> {
    if let Some(path) = macos_packet_tunnel_sysex_path().filter(|path| path.exists()) {
        return Some(path);
    }
    macos_packet_tunnel_appex_path()
}

/// The app bundle cannot change while the process runs, so the packaging shape
/// is resolved once instead of on every status query.
fn macos_packet_tunnel_packaging_mode() -> Option<&'static str> {
    static PACKAGING_MODE: OnceLock<Option<&'static str>> = OnceLock::new();

    *PACKAGING_MODE.get_or_init(|| {
        if macos_packet_tunnel_sysex_path().is_some_and(|path| path.exists()) {
            return Some("systemExtension");
        }
        if macos_packet_tunnel_appex_path().is_some_and(|path| path.exists()) {
            return Some("appExtension");
        }
        None
    })
}

/// `codesign -d --entitlements` is a process spawn against a bundle that cannot
/// change while the process runs, so its verdict is cached for the process.
fn macos_packet_tunnel_packaging_error() -> Option<String> {
    static PACKAGING_ERROR: OnceLock<Option<String>> = OnceLock::new();

    PACKAGING_ERROR
        .get_or_init(probe_macos_packet_tunnel_packaging_error)
        .clone()
}

#[cfg(target_os = "macos")]
fn probe_macos_packet_tunnel_packaging_error() -> Option<String> {
    if macos_packet_tunnel_packaging_mode() != Some("appExtension") {
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
    let needle = format!("{MACOS_PACKET_TUNNEL_BUNDLE_ID}.appex");
    let mut paths = Vec::new();

    for line in output.lines() {
        let mut search_start = 0;
        while let Some(relative_index) = line[search_start..].find(&needle) {
            let needle_start = search_start + relative_index;
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

fn macos_system_extension_state() -> Option<String> {
    parse_systemextensionsctl_state(
        &macos_systemextensionsctl_output()?,
        MACOS_PACKET_TUNNEL_BUNDLE_ID,
    )
}

fn macos_system_extension_is_activated() -> bool {
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

fn macos_system_extension_registration_lines() -> Vec<String> {
    let Some(output) = macos_systemextensionsctl_output() else {
        return Vec::new();
    };
    parse_systemextensionsctl_matches(&output, MACOS_PACKET_TUNNEL_BUNDLE_ID)
}

#[cfg(target_os = "macos")]
pub(super) fn platform_provider_registration_paths(
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
pub(super) fn platform_provider_registration_paths(
    _bundle_id: &str,
) -> Result<Vec<PathBuf>, NativeTunError> {
    Ok(Vec::new())
}

pub(super) fn start_macos_packet_tunnel(
    request: &NativeTunStartRequest,
) -> Result<(), NativeTunError> {
    // Starting only needs the component to be present and correctly packaged.
    // The full status probe would repeat a provider round trip whose result is
    // discarded, and the bridge activates the system extension itself.
    let component_missing = |message: String| NativeTunError::ComponentMissing {
        backend: TunBackend::MacosPacketTunnel,
        message,
    };
    require_bundled_component(
        macos_packet_tunnel_component_path(),
        "PacketTunnel extension is not bundled in this build",
    )
    .map_err(|status| {
        component_missing(
            status
                .message
                .unwrap_or_else(|| "PacketTunnel extension is missing".to_string()),
        )
    })?;
    if let Some(message) = macos_packet_tunnel_packaging_error() {
        return Err(component_missing(message));
    }

    ensure_macos_provider_path_matches(&PlatformProviderRegistrationResolver)?;
    start_macos_packet_tunnel_with_bridge(request)
}

pub fn ensure_macos_provider_path_matches(
    resolver: &dyn ProviderRegistrationResolver,
) -> Result<(), NativeTunError> {
    let Some(expected) = resolver.expected_provider_path(MACOS_PACKET_TUNNEL_BUNDLE_ID) else {
        return Ok(());
    };

    let resolved = match resolver.resolved_provider_paths(MACOS_PACKET_TUNNEL_BUNDLE_ID) {
        Ok(resolved) => resolved,
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

#[cfg(target_os = "macos")]
fn start_macos_packet_tunnel_with_bridge(
    request: &NativeTunStartRequest,
) -> Result<(), NativeTunError> {
    let config_path =
        request
            .main_config_path
            .to_str()
            .ok_or_else(|| NativeTunError::InvalidRequest {
                backend: TunBackend::MacosPacketTunnel,
                message: "main config path is not valid UTF-8".to_string(),
            })?;
    let output = macos_packet_tunnel_bridge_start(
        config_path,
        request.active_profile_id.as_deref(),
        MACOS_PACKET_TUNNEL_START_TIMEOUT_MS,
    )?;
    if output == "ok" {
        return Ok(());
    }
    if let Some(message) = output.strip_prefix("permissionRequired:") {
        return Err(NativeTunError::PermissionRequired {
            backend: TunBackend::MacosPacketTunnel,
            message: message.to_string(),
        });
    }

    Err(NativeTunError::CommandFailed {
        action: "start macOS PacketTunnel",
        status_code: None,
        output: output.trim_start_matches("error:").to_string(),
    })
}

#[cfg(not(target_os = "macos"))]
fn start_macos_packet_tunnel_with_bridge(
    _request: &NativeTunStartRequest,
) -> Result<(), NativeTunError> {
    Err(NativeTunError::ComponentMissing {
        backend: TunBackend::MacosPacketTunnel,
        message: "PacketTunnel bridge is not available on this platform".to_string(),
    })
}

pub(super) fn stop_macos_packet_tunnel() -> Result<(), NativeTunError> {
    stop_macos_packet_tunnel_with_bridge()
}

#[cfg(target_os = "macos")]
fn stop_macos_packet_tunnel_with_bridge() -> Result<(), NativeTunError> {
    let output = macos_packet_tunnel_bridge_stop()?;
    if output == "ok" {
        return Ok(());
    }

    Err(NativeTunError::CommandFailed {
        action: "stop macOS PacketTunnel",
        status_code: None,
        output: output.trim_start_matches("error:").to_string(),
    })
}

#[cfg(not(target_os = "macos"))]
fn stop_macos_packet_tunnel_with_bridge() -> Result<(), NativeTunError> {
    Ok(())
}
