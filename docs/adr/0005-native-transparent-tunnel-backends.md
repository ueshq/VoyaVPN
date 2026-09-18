# ADR 0005: Native Transparent Tunnel Backends

Status: Accepted

Date: 2026-06-30

## Context

The original desktop TUN path launched the regular sing-box process directly
from the Tauri app and used sudo on Unix when TUN was enabled. That works for
some browser traffic, but it does not match macOS VPN clients such as V2Box:
terminal tools like Claude Code and Codex can bypass or fail that process-level
TUN path unless the user sets explicit HTTP proxy environment variables.

VoyaVPN needs transparent proxy behavior for terminal apps, browsers, and
developer tools without requiring per-process proxy variables.

## Decision

VoyaVPN uses platform-native tunnel backends:

- macOS TUN uses a Network Extension `NEPacketTunnelProvider` hosted in a
  PacketTunnel app extension. The Tauri app writes the generated sing-box config
  into the App Group container and starts or stops the VPN profile in-process
  through `NETunnelProviderManager`. The PacketTunnel provider links or embeds
  the universal macOS `Libbox.framework` staged from sing-box's Apple build, starts a libbox command server, maps
  libbox TUN settings onto `NEPacketTunnelNetworkSettings`, and hands the
  `packetFlow` file descriptor back to libbox. It does not run the TUN core
  directly with sudo; restricted NetworkExtension operations stay within the
  provisioned app and PacketTunnel provider.
- Windows TUN uses a Windows service named `VoyaVPNTunnelService`. The desktop
  app writes the generated sing-box config and asks the service to start or stop
  sing-box with Wintun. The service validates the config with `sing-box check`
  before launch. The UI process does not own driver, route, or DNS lifecycle.
- Linux keeps the existing process TUN backend with the root-owned elevation
  launcher.

The shared contract remains the generated sing-box JSON. The binary/runtime
shape differs by OS, but route, DNS, proxy-group, and outbound semantics stay
shared through `voya-core`.

## Consequences

- `voya-platform::tun::tun_backend` is the single platform selection point for
  TUN runtime shape.
- macOS and Windows no longer silently fall back to the regular process TUN
  path when transparent TUN is enabled.
- Native component absence is reported as an explicit provider state so users
  see "missing PacketTunnel extension" or "missing Windows service" instead of
  a misleading connected state.
- macOS PacketTunnel registration health must be checked when local `.app`
  bundles or test fixtures are launched, because PlugInKit elects providers by
  extension bundle id globally. Use `pnpm native:macos:ne:doctor`; see
  `docs/release/macos-networkextension-troubleshooting.md`.
- The temporary system-proxy fallback is only used for the process TUN backend.
  Native backends are expected to capture terminal and app traffic at the OS
  tunnel layer.
- App Store/TestFlight macOS builds must include App Group and Network
  Extension entitlements and a signed PacketTunnel extension with a linked
  static Libbox runtime or an embedded signed dynamic `Libbox.framework`.
  Developer builds without libbox still compile, but the PacketTunnel provider
  fails closed instead of reporting a connected tunnel. Windows installers must
  install and update the service before enabling service-backed TUN in release
  smoke tests.
