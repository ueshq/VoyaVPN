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

/// The untyped string protocol the ObjC bridge answers with. Mirrored from
/// `crates/voya-platform/native/macos_packet_tunnel_bridge.m`; the tests below
/// assert the two sides still agree, because nothing else does.
const BRIDGE_OK: &str = "ok";
const BRIDGE_ERROR_PREFIX: &str = "error:";
const BRIDGE_PERMISSION_REQUIRED_PREFIX: &str = "permissionRequired:";

const MISSING_COMPONENT_MESSAGE: &str = "PacketTunnel extension is not bundled in this build";
const SYSTEM_EXTENSION_APPROVAL_MESSAGE: &str =
    "Approve the VoyaVPN PacketTunnel system extension in System Settings, then enable TUN again.";
const PACKAGING_MODE_SYSTEM_EXTENSION: &str = "systemExtension";

/// The OS-observable facts the macOS status decision table reads.
///
/// Each probe spawns a process or crosses the ObjC bridge, so they stay behind
/// a seam and behind method calls rather than an eager snapshot: the table must
/// keep short-circuiting them, and the fail-closed ordering ADR-0005 depends on
/// is then table-testable on every CI platform instead of only on macOS.
trait PacketTunnelProbe {
    fn component_present(&self) -> bool;
    fn packaging_error(&self) -> Option<String>;
    fn packaging_mode(&self) -> Option<&'static str>;
    fn system_extension_activated(&self) -> bool;
    fn bridge_status(&self) -> Result<String, NativeTunError>;
    /// Bridge last-error and provider status-file context, joined onto `base`
    /// for the terminal (Stopped/Error) states.
    fn status_message(&self, base: Option<String>, state: NativeTunProviderState) -> String;
}

struct PlatformPacketTunnelProbe;

impl PacketTunnelProbe for PlatformPacketTunnelProbe {
    fn component_present(&self) -> bool {
        macos_packet_tunnel_component_path().is_some_and(|path| path.exists())
    }

    fn packaging_error(&self) -> Option<String> {
        macos_packet_tunnel_packaging_error()
    }

    fn packaging_mode(&self) -> Option<&'static str> {
        macos_packet_tunnel_packaging_mode()
    }

    fn system_extension_activated(&self) -> bool {
        macos_system_extension_is_activated()
    }

    fn bridge_status(&self) -> Result<String, NativeTunError> {
        macos_packet_tunnel_bridge_status()
    }

    fn status_message(&self, base: Option<String>, state: NativeTunProviderState) -> String {
        macos_packet_tunnel_status_message(base, state)
    }
}

pub(super) fn macos_packet_tunnel_status() -> NativeTunStatus {
    macos_packet_tunnel_status_from(&PlatformPacketTunnelProbe)
}

/// Decides the macOS provider state. The branch order is the fail-closed
/// contract: a missing or mispackaged component and an unapproved system
/// extension are reported before the bridge is consulted, so the UI explains
/// what to fix instead of showing a generic error.
fn macos_packet_tunnel_status_from(probe: &dyn PacketTunnelProbe) -> NativeTunStatus {
    if !probe.component_present() {
        return NativeTunStatus::missing_component(
            TunBackend::MacosPacketTunnel,
            MISSING_COMPONENT_MESSAGE,
        );
    }

    if let Some(message) = probe.packaging_error() {
        return NativeTunStatus {
            backend: TunBackend::MacosPacketTunnel,
            provider_state: NativeTunProviderState::Error,
            component_ready: false,
            message: Some(message),
        };
    }

    if probe.packaging_mode() == Some(PACKAGING_MODE_SYSTEM_EXTENSION)
        && !probe.system_extension_activated()
    {
        return NativeTunStatus {
            backend: TunBackend::MacosPacketTunnel,
            provider_state: NativeTunProviderState::PermissionRequired,
            component_ready: true,
            message: Some(SYSTEM_EXTENSION_APPROVAL_MESSAGE.to_string()),
        };
    }

    let (provider_state, base) = match probe.bridge_status() {
        Ok(output) if output.starts_with(BRIDGE_ERROR_PREFIX) => (
            NativeTunProviderState::Error,
            Some(strip_bridge_error_prefix(&output).to_string()),
        ),
        Ok(output) => (parse_macos_provider_state(&output), None),
        Err(error) => (NativeTunProviderState::Error, Some(error.to_string())),
    };

    NativeTunStatus {
        backend: TunBackend::MacosPacketTunnel,
        provider_state,
        component_ready: true,
        message: macos_packet_tunnel_terminal_message(probe, provider_state, base),
    }
}

/// Stopped and Error carry every message the probe can offer; the running and
/// starting states stay quiet unless the bridge itself reported something.
fn macos_packet_tunnel_terminal_message(
    probe: &dyn PacketTunnelProbe,
    provider_state: NativeTunProviderState,
    base: Option<String>,
) -> Option<String> {
    if base.is_none()
        && !matches!(
            provider_state,
            NativeTunProviderState::Stopped | NativeTunProviderState::Error
        )
    {
        return None;
    }

    let message = probe.status_message(base, provider_state);
    (!message.is_empty()).then_some(message)
}

fn strip_bridge_error_prefix(output: &str) -> &str {
    output.strip_prefix(BRIDGE_ERROR_PREFIX).unwrap_or(output)
}

/// Parses the bridge's `start` reply.
///
/// Kept free of `cfg` because the only caller is macOS-only: without this the
/// contract with the ObjC side never compiles, let alone runs, in Linux CI.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_bridge_start_output(output: &str) -> Result<(), NativeTunError> {
    if output == BRIDGE_OK {
        return Ok(());
    }
    if let Some(message) = output.strip_prefix(BRIDGE_PERMISSION_REQUIRED_PREFIX) {
        return Err(NativeTunError::PermissionRequired {
            backend: TunBackend::MacosPacketTunnel,
            message: message.to_string(),
        });
    }

    if let Some(json) = output.strip_prefix("startFailed:") {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(start_error) = value.get("error").and_then(serde_json::Value::as_str) {
                return Err(
                    match value
                        .get("cleanupError")
                        .and_then(serde_json::Value::as_str)
                    {
                        Some(cleanup_error) => NativeTunError::StartCleanupFailed {
                            start_error: start_error.to_string(),
                            cleanup_error: cleanup_error.to_string(),
                        },
                        None => NativeTunError::StartFailed {
                            message: start_error.to_string(),
                        },
                    },
                );
            }
        }
    }
    Err(bridge_command_failed("start macOS PacketTunnel", output))
}

/// Parses the bridge's `stop` reply; see [`parse_bridge_start_output`].
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_bridge_stop_output(output: &str) -> Result<(), NativeTunError> {
    if output == BRIDGE_OK {
        return Ok(());
    }

    Err(bridge_command_failed("stop macOS PacketTunnel", output))
}

fn bridge_command_failed(action: &'static str, output: &str) -> NativeTunError {
    NativeTunError::Bridge {
        action,
        message: strip_bridge_error_prefix(output).to_string(),
    }
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
    let mut messages = vec![base];
    if matches!(
        provider_state,
        NativeTunProviderState::Stopped | NativeTunProviderState::Error
    ) {
        messages.push(macos_packet_tunnel_last_error());
        messages.push(macos_packet_tunnel_status_file().and_then(|status| status.last_error));
    }

    join_unique_messages(messages)
}

/// Joins the message sources with `; `, dropping blanks and repeats: the bridge
/// last error and the provider status file usually carry the same text.
fn join_unique_messages(messages: impl IntoIterator<Item = Option<String>>) -> String {
    let mut unique = Vec::new();
    for message in messages {
        push_unique_message(&mut unique, message);
    }
    unique.join("; ")
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
    let registration = platform_provider_registration_paths(MACOS_PACKET_TUNNEL_BUNDLE_ID);
    if matches!(registration, Err(NativeTunError::RegistrationUnavailable)) {
        diagnostics.message = Some("PacketTunnel registration cannot be queried inside the app sandbox; use the external NetworkExtension doctor.".to_string());
    }
    diagnostics.registration_paths = registration
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.display().to_string())
        .collect();
    diagnostics
        .registration_paths
        .extend(macos_system_extension_registration_lines());
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
    if value.is_empty() || value.starts_with(BRIDGE_ERROR_PREFIX) {
        return None;
    }
    Some(value.to_string())
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
    if !PlatformPacketTunnelProbe.component_present() {
        return Err(component_missing(MISSING_COMPONENT_MESSAGE.to_string()));
    }
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
        request.kill_switch,
    )?;
    parse_bridge_start_output(&output)
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
    parse_bridge_stop_output(&macos_packet_tunnel_bridge_stop()?)
}

#[cfg(not(target_os = "macos"))]
fn stop_macos_packet_tunnel_with_bridge() -> Result<(), NativeTunError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// The ObjC bridge source, so a rename on either side of the untyped string
    /// protocol fails a test instead of silently degrading the UI.
    const BRIDGE_SOURCE: &str = include_str!("../../native/macos_packet_tunnel_bridge.m");

    #[derive(Clone, Copy)]
    enum FakeBridgeStatus {
        Output(&'static str),
        Failure(&'static str),
    }

    struct FakeProbe {
        component_present: bool,
        packaging_error: Option<&'static str>,
        packaging_mode: Option<&'static str>,
        system_extension_activated: bool,
        bridge_status: FakeBridgeStatus,
        terminal_context: Option<&'static str>,
        calls: RefCell<Vec<&'static str>>,
    }

    impl FakeProbe {
        fn ready(bridge_status: FakeBridgeStatus) -> Self {
            Self {
                component_present: true,
                packaging_error: None,
                packaging_mode: Some("appExtension"),
                system_extension_activated: false,
                bridge_status,
                terminal_context: None,
                calls: RefCell::new(Vec::new()),
            }
        }

        fn record(&self, call: &'static str) {
            self.calls.borrow_mut().push(call);
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.borrow().clone()
        }
    }

    impl PacketTunnelProbe for FakeProbe {
        fn component_present(&self) -> bool {
            self.record("component_present");
            self.component_present
        }

        fn packaging_error(&self) -> Option<String> {
            self.record("packaging_error");
            self.packaging_error.map(str::to_string)
        }

        fn packaging_mode(&self) -> Option<&'static str> {
            self.record("packaging_mode");
            self.packaging_mode
        }

        fn system_extension_activated(&self) -> bool {
            self.record("system_extension_activated");
            self.system_extension_activated
        }

        fn bridge_status(&self) -> Result<String, NativeTunError> {
            self.record("bridge_status");
            match self.bridge_status {
                FakeBridgeStatus::Output(output) => Ok(output.to_string()),
                FakeBridgeStatus::Failure(message) => Err(NativeTunError::CommandFailed {
                    action: "query macOS PacketTunnel status",
                    status_code: None,
                    output: message.to_string(),
                }),
            }
        }

        fn status_message(&self, base: Option<String>, state: NativeTunProviderState) -> String {
            self.record("status_message");
            let mut messages = vec![base];
            if matches!(
                state,
                NativeTunProviderState::Stopped | NativeTunProviderState::Error
            ) {
                messages.push(self.terminal_context.map(str::to_string));
            }
            join_unique_messages(messages)
        }
    }

    #[test]
    fn macos_status_reports_a_missing_component_before_probing_anything_else() {
        let probe = FakeProbe {
            component_present: false,
            ..FakeProbe::ready(FakeBridgeStatus::Output("running"))
        };

        let status = macos_packet_tunnel_status_from(&probe);

        assert_eq!(
            status.provider_state,
            NativeTunProviderState::MissingComponent
        );
        assert!(!status.component_ready);
        assert_eq!(status.message.as_deref(), Some(MISSING_COMPONENT_MESSAGE));
        assert_eq!(
            probe.calls(),
            ["component_present"],
            "a missing component must not reach the bridge"
        );
    }

    #[test]
    fn macos_status_reports_a_packaging_error_before_probing_the_bridge() {
        let probe = FakeProbe {
            packaging_error: Some("re-run pnpm native:macos:tunnel"),
            ..FakeProbe::ready(FakeBridgeStatus::Output("running"))
        };

        let status = macos_packet_tunnel_status_from(&probe);

        assert_eq!(status.provider_state, NativeTunProviderState::Error);
        assert!(
            !status.component_ready,
            "a mispackaged extension is not usable"
        );
        assert_eq!(
            status.message.as_deref(),
            Some("re-run pnpm native:macos:tunnel")
        );
        assert_eq!(probe.calls(), ["component_present", "packaging_error"]);
    }

    #[test]
    fn macos_status_asks_for_approval_before_probing_the_bridge() {
        let probe = FakeProbe {
            packaging_mode: Some(PACKAGING_MODE_SYSTEM_EXTENSION),
            system_extension_activated: false,
            ..FakeProbe::ready(FakeBridgeStatus::Output("running"))
        };

        let status = macos_packet_tunnel_status_from(&probe);

        assert_eq!(
            status.provider_state,
            NativeTunProviderState::PermissionRequired,
            "an unapproved system extension must not be reported as a generic error"
        );
        assert!(status.component_ready);
        assert_eq!(
            status.message.as_deref(),
            Some(SYSTEM_EXTENSION_APPROVAL_MESSAGE)
        );
        assert!(
            !probe.calls().contains(&"bridge_status"),
            "an unapproved system extension must not reach the bridge"
        );
    }

    #[test]
    fn macos_status_consults_the_bridge_once_the_system_extension_is_approved() {
        let probe = FakeProbe {
            packaging_mode: Some(PACKAGING_MODE_SYSTEM_EXTENSION),
            system_extension_activated: true,
            ..FakeProbe::ready(FakeBridgeStatus::Output("running"))
        };

        let status = macos_packet_tunnel_status_from(&probe);

        assert_eq!(status.provider_state, NativeTunProviderState::Running);
        assert_eq!(status.message, None, "a healthy tunnel needs no message");
        assert!(probe.calls().contains(&"system_extension_activated"));
        assert!(probe.calls().contains(&"bridge_status"));
    }

    #[test]
    fn macos_status_maps_bridge_output_to_provider_states() {
        for (output, expected) in [
            ("running", NativeTunProviderState::Running),
            ("starting", NativeTunProviderState::Starting),
            ("stopped", NativeTunProviderState::Stopped),
            (
                "permissionRequired",
                NativeTunProviderState::PermissionRequired,
            ),
            ("gibberish", NativeTunProviderState::Error),
        ] {
            let status = macos_packet_tunnel_status_from(&FakeProbe::ready(
                FakeBridgeStatus::Output(output),
            ));

            assert_eq!(
                status.provider_state, expected,
                "unexpected state for bridge output {output:?}"
            );
            assert!(status.component_ready);
        }
    }

    #[test]
    fn macos_status_unwraps_an_error_reply_and_appends_terminal_context() {
        let probe = FakeProbe {
            terminal_context: Some("provider stopped: sing-box runtime unavailable"),
            ..FakeProbe::ready(FakeBridgeStatus::Output(
                "error:VoyaVPN PacketTunnel session is unavailable.",
            ))
        };

        let status = macos_packet_tunnel_status_from(&probe);

        assert_eq!(status.provider_state, NativeTunProviderState::Error);
        assert_eq!(
            status.message.as_deref(),
            Some(
                "VoyaVPN PacketTunnel session is unavailable.; provider stopped: sing-box runtime unavailable"
            ),
            "the error: marker is stripped and the provider context is joined on"
        );
    }

    #[test]
    fn macos_status_surfaces_a_failed_bridge_call_as_an_error() {
        let probe = FakeProbe::ready(FakeBridgeStatus::Failure("bridge returned a null response"));

        let status = macos_packet_tunnel_status_from(&probe);

        assert_eq!(status.provider_state, NativeTunProviderState::Error);
        assert!(status
            .message
            .as_deref()
            .is_some_and(|message| message.contains("bridge returned a null response")));
    }

    #[test]
    fn macos_status_explains_a_stopped_tunnel_with_provider_context() {
        let probe = FakeProbe {
            terminal_context: Some("failed: sing-box runtime unavailable"),
            ..FakeProbe::ready(FakeBridgeStatus::Output("stopped"))
        };

        let status = macos_packet_tunnel_status_from(&probe);

        assert_eq!(status.provider_state, NativeTunProviderState::Stopped);
        assert_eq!(
            status.message.as_deref(),
            Some("failed: sing-box runtime unavailable")
        );
    }

    #[test]
    fn macos_status_leaves_a_stopped_tunnel_without_a_message_when_nothing_explains_it() {
        let status =
            macos_packet_tunnel_status_from(&FakeProbe::ready(FakeBridgeStatus::Output("stopped")));

        assert_eq!(status.provider_state, NativeTunProviderState::Stopped);
        assert_eq!(status.message, None);
    }

    #[test]
    fn macos_bridge_start_reply_is_parsed_into_typed_outcomes() {
        parse_bridge_start_output("ok").expect("ok must succeed");

        let permission = parse_bridge_start_output(
            "permissionRequired:Approve the VoyaVPN PacketTunnel system extension in System Settings, then enable TUN again.",
        )
        .expect_err("permissionRequired must fail");
        assert!(
            matches!(
                &permission,
                NativeTunError::PermissionRequired {
                    backend: TunBackend::MacosPacketTunnel,
                    message,
                } if message.starts_with("Approve the VoyaVPN PacketTunnel")
            ),
            "unexpected error: {permission}"
        );

        let failure =
            parse_bridge_start_output("error:VoyaVPN PacketTunnel manager is unavailable.")
                .expect_err("error must fail");
        assert!(
            matches!(
                &failure,
                NativeTunError::Bridge {
                    action: "start macOS PacketTunnel",
                    message,
                } if message == "VoyaVPN PacketTunnel manager is unavailable."
            ),
            "unexpected error: {failure}"
        );

        let unexpected =
            parse_bridge_start_output("running").expect_err("unexpected output must fail closed");
        assert!(
            matches!(
                &unexpected,
                NativeTunError::Bridge { message, .. } if message == "running"
            ),
            "unexpected error: {unexpected}"
        );
    }

    #[test]
    fn bridge_start_failure_preserves_original_and_cleanup_errors() {
        let clean = parse_bridge_start_output(
            r#"startFailed:{"error":"start timed out","cleanupError":null}"#,
        )
        .expect_err("failure");
        assert!(matches!(clean, NativeTunError::StartFailed { .. }));
        let pending = parse_bridge_start_output(
            r#"startFailed:{"error":"start timed out","cleanupError":"stop timed out"}"#,
        )
        .expect_err("pending cleanup");
        assert!(
            matches!(&pending, NativeTunError::StartCleanupFailed { start_error, cleanup_error } if start_error == "start timed out" && cleanup_error == "stop timed out")
        );
        assert!(!pending.to_string().contains("status None"));
        assert!(parse_bridge_start_output("startFailed:garbage").is_err());
    }

    #[test]
    fn macos_bridge_stop_reply_is_parsed_into_typed_outcomes() {
        parse_bridge_stop_output("ok").expect("ok must succeed");

        let failure = parse_bridge_stop_output("error:no manager").expect_err("error must fail");
        assert!(
            matches!(
                &failure,
                NativeTunError::Bridge {
                    action: "stop macOS PacketTunnel",
                    message,
                    ..
                } if message == "no manager"
            ),
            "unexpected error: {failure}"
        );
    }

    #[test]
    fn macos_bridge_string_protocol_matches_the_objective_c_source() {
        for literal in [
            BRIDGE_OK,
            BRIDGE_ERROR_PREFIX,
            BRIDGE_PERMISSION_REQUIRED_PREFIX,
        ] {
            assert!(
                BRIDGE_SOURCE.contains(&format!("@\"{literal}")),
                "{literal:?} is not produced by macos_packet_tunnel_bridge.m any more"
            );
        }

        for state in ["running", "starting", "stopped", "permissionRequired"] {
            assert_ne!(
                parse_macos_provider_state(state),
                NativeTunProviderState::Error,
                "the bridge status vocabulary must stay parseable"
            );
            assert!(
                BRIDGE_SOURCE.contains(&format!("@\"{state}\"")),
                "{state:?} is not produced by macos_packet_tunnel_bridge.m any more"
            );
        }
    }

    #[test]
    fn macos_bridge_error_prefix_is_stripped_exactly_once() {
        assert_eq!(strip_bridge_error_prefix("error:boom"), "boom");
        assert_eq!(strip_bridge_error_prefix("boom"), "boom");
        assert_eq!(
            strip_bridge_error_prefix("error:error:boom"),
            "error:boom",
            "a message that legitimately mentions error: must survive"
        );
    }

    #[test]
    fn macos_status_messages_are_trimmed_deduplicated_and_joined() {
        assert_eq!(
            join_unique_messages([
                Some("  boom  ".to_string()),
                Some("boom".to_string()),
                None,
                Some("   ".to_string()),
                Some("later".to_string()),
            ]),
            "boom; later"
        );
        assert_eq!(join_unique_messages([None, Some(String::new())]), "");
    }

    #[test]
    fn macos_push_unique_message_keeps_the_first_occurrence() {
        let mut messages = vec!["first".to_string()];
        push_unique_message(&mut messages, Some("first".to_string()));
        push_unique_message(&mut messages, Some("\tfirst\n".to_string()));
        push_unique_message(&mut messages, None);
        push_unique_message(&mut messages, Some(" second ".to_string()));

        assert_eq!(messages, ["first", "second"]);
    }

    #[test]
    fn macos_log_tail_keeps_the_last_lines_in_order() {
        assert_eq!(tail_lines("a\nb\nc\nd", 2), ["c", "d"]);
        assert_eq!(tail_lines("a\nb", 10), ["a", "b"]);
        assert!(tail_lines("", 10).is_empty());
        assert!(tail_lines("a\nb", 0).is_empty());
    }

    #[test]
    fn macos_optional_bridge_output_drops_blanks_and_error_replies() {
        assert_eq!(normalize_bridge_optional_output(String::new()), None);
        assert_eq!(normalize_bridge_optional_output("  \n".to_string()), None);
        assert_eq!(
            normalize_bridge_optional_output("error:no container".to_string()),
            None,
            "an error reply is not a container path or a last error"
        );
        assert_eq!(
            normalize_bridge_optional_output("  /Users/afu/Library  ".to_string()),
            Some("/Users/afu/Library".to_string())
        );
    }
}
