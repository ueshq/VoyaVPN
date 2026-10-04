# Apple PacketTunnel sources

The `NEPacketTunnelProvider` that runs sing-box, shared by the macOS desktop
app and the iOS app. They ship the same provider; what differs is packaging,
not behaviour, so there is one copy here rather than one per app.

| | macOS | iOS |
| --- | --- | --- |
| App bundle id | `app.voyavpn.desktop` | `app.voyavpn.mobile` |
| Extension bundle id | `app.voyavpn.desktop.PacketTunnel` | `app.voyavpn.mobile.PacketTunnel` |
| App Group | `group.app.voyavpn.desktop` | `group.app.voyavpn.mobile` |
| Staged by | `scripts/native/macos/build-tunnel.mjs` | the Xcode appex target in `apps/mobile/ios` |
| Libbox slice | universal macOS | `ios-arm64` + simulator |

**The identifiers must never be shared.** macOS elects app-extension providers
globally by bundle id through PlugInKit, so a second bundle claiming
`app.voyavpn.desktop.PacketTunnel` can become the elected provider for the
desktop app — see the NetworkExtension hygiene section in `AGENTS.md`.

Because the identifiers differ, nothing here hardcodes one. The App Group comes
from the extension's own `Info.plist` under `VoyaAppGroupIdentifier`, which each
platform declares beside the matching `com.apple.security.application-groups`
entitlement. An extension that declares neither fails closed: there is no
container, so `runtimePaths` throws `missingAppGroupContainer` rather than
reaching into another app's group.

## Files

- `PacketTunnel/PacketTunnelProvider.swift` — the provider itself: start and
  stop, serialized on one queue.
- `PacketTunnel/PacketTunnelRuntime.swift` — the host handshake payload, its
  validation, and the libbox working paths. The base is `PT/` at the container
  root: short because libbox once bound a command socket under it, and kept
  there because installed tunnels already have their files in it. The provider
  does not open that socket — the app reads the core through its Clash API.
- `PacketTunnel/PacketTunnelDiagnostics.swift` — the provider's status file and
  rotating log, both confined to the container.
- `PacketTunnel/PacketTunnelPlatform.swift` — the libbox `PlatformInterface`,
  including the tunnel network settings built from the options libbox hands it.
- `DefaultInterfaceMonitor.swift` — the `NWPathMonitor` behind libbox's
  default-interface callbacks, shared with the iOS app's probe core.
- `PacketTunnelTests.swift` — a `@main` binary, not XCTest: it runs the runtime
  and diagnostics in a temporary directory without loading a NetworkExtension.
  `pnpm check:native:macos:bridge` compiles and runs it, and `check:rust:test`
  runs that on macOS.
