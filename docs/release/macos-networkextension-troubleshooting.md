# macOS NetworkExtension Troubleshooting

VoyaVPN's macOS TUN mode is a NetworkExtension PacketTunnel provider with this
bundle id:

```sh
app.voyavpn.desktop.PacketTunnel
```

macOS registers and elects app extensions globally through PlugInKit. The
selected provider is not necessarily the appex inside the app bundle you just
opened. If an old test bundle with the same PacketTunnel id was launched once,
macOS can keep electing that stale copy.

Developer ID direct-distribution builds use a System Extension instead:

```sh
VoyaVPN.app/Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension
```

Those builds must be signed with `packet-tunnel-provider-systemextension` and
approved by the user through System Settings. App Store/TestFlight and unsigned
development builds keep the `.appex` shape and use `packet-tunnel-provider`.

## Symptoms

- **"PacketTunnel extension is not bundled in this build"** (or the localized
  missing VPN extension message) means the running copy has no PacketTunnel
  provider. Quit VoyaVPN and open `/Applications/VoyaVPN.app`. Local builds
  intentionally remove the extension from leftover `target/` app copies after
  installation to prevent them from winning PlugInKit election. `pnpm dev` also
  runs without a bundled provider and cannot be used for macOS VPN testing.
  If the installed copy is incomplete, reinstall a complete VPN-capable package;
  developers can rebuild and install it with `pnpm build:mac:local`. Verify the
  installed copy with `pnpm native:macos:ne:doctor --app /Applications/VoyaVPN.app`.
- Enabling TUN disconnects browsers and apps.
- Disabling TUN and using system proxy works.
- The UI may report connected while traffic does not pass.
- `scutil --nc list` shows a VoyaVPN VPN profile, but the elected appex is not
  `/Applications/VoyaVPN.app`.
- Starting TUN fails with "The VPN session failed because an internal error
  occurred", while provider status/log files are absent.
- System logs contain `Signature check failed` or `Validation failed - no audit
  tokens`. This usually means a Developer ID `*-systemextension` entitlement was
  signed into an `.appex` instead of packaging PacketTunnel as a System
  Extension.
- Provider status/log shows `listen unix ... command.sock: bind: invalid
  argument`. The libbox command-socket path exceeds the macOS 104-byte
  `sun_path` limit; providers built before the base dir moved to `PT/` at the
  App Group container root do this. Rebuild and reinstall the app.
- Provider status/log shows `listen tcp 127.0.0.1:<port>: bind: operation not
  permitted`. The sandboxed process lacks the
  `com.apple.security.network.server` entitlement, so sing-box cannot open its
  local mixed/socks inbound; bundles signed before that key was added to the
  entitlement plists do this. Rebuild and reinstall the app.

## Check Registration Health

Run:

```sh
pnpm native:macos:ne:doctor
```

For raw evidence:

```sh
pluginkit -mDvvv -i app.voyavpn.desktop.PacketTunnel
pluginkit -mAvvv -i app.voyavpn.desktop.PacketTunnel
systemextensionsctl list | grep app.voyavpn.desktop.PacketTunnel
scutil --nc list
```

The healthy Developer ID/direct-install shape is exactly one active legal
provider under:

```sh
/Applications/VoyaVPN.app/Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension
```

The healthy App Store/TestFlight or unsigned development shape is exactly one
active legal PlugInKit provider under:

```sh
/Applications/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex
```

Builds from before 2026-09-24 named the folder
`app.voyavpn.desktop.PacketTunnel.appex`. App Store validation requires an
appex folder to match its executable (ITMS-90362), so it was renamed. The
doctor still lists registrations under the old name and `--fix` removes them
as stale.

For local release-bundle testing, pass the app path and allow the repo bundle:

```sh
pnpm native:macos:ne:doctor --app "$PWD/target/release/bundle/macos/VoyaVPN.app" --dev
```

## Repair

Quit VoyaVPN first, then run:

```sh
pnpm native:macos:ne:doctor --fix
```

For a non-`/Applications` app:

```sh
pnpm native:macos:ne:doctor --fix --app "$PWD/target/release/bundle/macos/VoyaVPN.app" --dev
```

The doctor unregisters stale app-extension registrations, refreshes
LaunchServices, reports System Extension activation state, and re-checks the
result. It does not remove VPN profiles. If a broken profile was created while a
stale provider was elected, remove it manually from System Settings > VPN.

## Runtime Evidence

After repair and a successful TUN start:

```sh
scutil --nc list
ifconfig | grep -B1 -A3 utun
env -u http_proxy -u https_proxy -u all_proxy curl -4 -m 10 https://api.ip.sb/ip
log stream --predicate 'processImagePath CONTAINS "VoyaPacketTunnel"' --info
```

The PacketTunnel provider also writes diagnostic state in the App Group
container under `Library/Application Support/VoyaVPN/packet-tunnel-status.json`
and `provider.log` when the extension starts.
