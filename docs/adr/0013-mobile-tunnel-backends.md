# ADR 0013: Mobile Tunnel Backends

Status: Accepted

Date: 2026-09-21

## Context

ADR 0005 established the transparent-tunnel backends the desktop runs: a
child sing-box with a TUN on Windows and Linux, and on macOS a
`NEPacketTunnelProvider` that runs the core inside a NetworkExtension, reached
through the `NativeTunController` seam so the supervisor never learns which one
it has.

ADR 0012 then established the mobile host. It left one thing open: what runs
the core on a phone.

Neither phone can answer that the way Windows does. Both refuse to let an app
spawn a child process at all, so "launch sing-box and hand it a TUN" is not
available on either. What they offer instead is the same idea macOS already
uses — a system-owned tunnel the app asks for — with two different shapes.

## Decision

### The core is a library on both phones, and `NativeTunController` is still the seam

`Libbox` — sing-box's own mobile binding — is linked into the tunnel on both
platforms, built from the same pinned source tag the desktop's seed comes from
(`docs/release/mobile-libbox-pinning.md`). `voya-app` is unchanged: the mobile
host registers a `NativeTunController`, `SupervisorActor::plan_start` takes the
native branch, and nothing above it knows whether the provider is a macOS
appex, an iOS appex or an Android service.

`TargetOs::runs_core_in_tunnel_provider()` is the one fact that follows from
this, and all three platforms answer yes to it. Two things turn on it: the
runtime config carries an unrouted probe outbound per node, and a latency test
while connected goes through that core's Clash API rather than launching one of
its own.

### iOS: a PacketTunnel extension, in a process of its own

The provider is `app.voyavpn.mobile.PacketTunnel`, an appex, and it is the same
Swift the macOS provider is — the sources moved to `native/apple/` and are
shared. What differs is packaging, not behaviour.

Three consequences follow from the extension being a *separate process*:

- **The handshake travels inline.** `startVPNTunnel(options:)` carries the
  generated config as `runtimeConfigJson`, because the provider cannot read the
  app's own container. The App Group container is for rule sets and status, not
  for the config a tunnel is being started with.
- **The App Group is a bundle fact.** macOS and iOS declare different groups
  (`group.app.voyavpn.desktop` and `group.app.voyavpn.mobile`) in each
  extension's own `Info.plist` under `VoyaAppGroupIdentifier`. They must never
  be shared, and neither must the bundle ids: macOS elects app-extension
  providers globally by bundle id through PlugInKit, so a second bundle
  claiming the desktop's identifier can become the elected provider for the
  desktop app.
- **A disconnected latency test needs a second Libbox.** The extension is not
  running while disconnected, so the app process links Libbox too and runs a
  TUN-less probe instance under `PTest/` — kept apart from the provider's `PT/`
  for independent per-run files. The probe calls `StartOrReloadService`
  in-process without starting libbox’s optional command listener, so it does
  not create a Unix socket (simulator container paths exceed its 104-byte
  limit). This is what `ProbeCoreLauncher` exists for.

### Android: a `VpnService` in the app's own process

`VoyaVpnService` is a foreground `VpnService`. The core runs in the app
process, which removes the whole class of problem iOS has: the Clash API is on
plain loopback with no container socket, the probe core is the same Libbox in
the same process, and there is no handshake file at all.

What Android adds instead is an authorization dance. `VpnService.prepare`
returns an intent when the user has not granted the VPN, and only an Activity
can present it — so `TunnelHost::start` reports it as `PermissionDenied`, which
reaches the frontend through the same `setElevationHandler` seam macOS's
"add a VPN configuration" prompt does.

### The probe core is a seam, not a special case

`voya-app`'s `ProbeCoreLauncher` splits a probe run along the one line that is
not portable. `LauncherCoreBackend` generates the config, waits for the SOCKS
ports and tears the core down; a launcher starts it. The desktop's spawns a
child process. The mobile host's calls back into the app, which runs Libbox.

Everything above that line — the port reservation, the readiness budget, the
per-node outcomes — is the same code on every platform.

## Alternatives considered

**A `sing-box` binary shipped and exec'd.** Not available: neither phone lets
an app execute a binary it did not have signed into its own bundle, and iOS
forbids it outright.

**Libbox in the app process on iOS, with the extension as a dumb packet pipe.**
The extension would forward packets over an XPC or socket channel to a core in
the app. It halves the memory pressure in the 50 MB extension budget, and it
was rejected: the app is suspended aggressively in the background, and a tunnel
whose core dies when the user switches apps is not a tunnel.

**One Libbox instance on iOS, shared between the tunnel and the probe.** There
is nothing to share while disconnected — the extension is not running — and
while connected the probe goes through the running core's Clash API anyway. The
second instance exists for exactly the case the first one cannot cover.

**Android per-app rules now.** Deferred, not rejected. Android matches by
package name rather than by process name, so `list_process_candidates` does not
apply and the desktop's per-app UI does not transfer. `addDisallowedApplication`
currently excludes only VoyaVPN itself, so its own Clash API traffic never
re-enters the tunnel.

## Consequences

- Three platforms now run the core inside a tunnel provider and two do not.
  `runs_core_in_tunnel_provider()` is where that split is stated; a fourth
  platform answers it rather than adding a branch.
- The iOS extension's memory ceiling is the risk this ADR does not remove.
  A large rule set inside a 50 MB appex is the classic sing-box-on-iOS failure,
  and it is the first thing the device acceptance in
  `docs/release/mobile-ios-signing.md` checks.
- Libbox is GPL-3.0-or-later. Every mobile artifact handed to a third party
  carries the same redistribution obligations the desktop bundle does.
