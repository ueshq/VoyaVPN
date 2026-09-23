# macOS Native Tunnel

VoyaVPN's macOS transparent tunnel is designed to match the system VPN model
used by clients such as V2Box.

The provider's Swift sources are not here: they are shared with the iOS app at
[`native/apple/`](../../../../../native/apple/README.md). What stays in this
directory is what is macOS's alone — the extension's `Info.plist` (including
the `VoyaAppGroupIdentifier` the shared sources read) and the universal macOS
`Libbox.framework`.

Runtime shape:

- Containing app bundle id: `app.voyavpn.desktop`
- PacketTunnel extension bundle id: `app.voyavpn.desktop.PacketTunnel`
- App Group: `group.app.voyavpn.desktop`
- Runtime config file: `Library/Application Support/VoyaVPN/packet-tunnel-runtime.json`
- libbox base dir: `PT/` at the App Group container root. libbox binds
  `<base>/command.sock`, and macOS caps unix-socket paths (`sun_path`) at 104
  bytes, so this directory must stay short — a base under
  `Library/Application Support/...` already exceeds the limit and makes
  `bind()` fail with `invalid argument`.

The Tauri app writes the generated sing-box JSON into the App Group container
and starts the VPN profile in-process through NetworkExtension. The extension
then owns the packet tunnel and runs the sing-box Apple/libbox runtime linked
into or embedded with the PacketTunnel extension.

Build commands:

```sh
pnpm native:macos:libbox
pnpm native:macos:tunnel
pnpm native:macos:tunnel:verify
pnpm native:macos:dmg
```

`pnpm native:macos:libbox` clones the pinned sing-box source tag, builds the
Apple XCFramework, validates its universal macOS slice, and stages only
`src-tauri/native/macos/Frameworks/Libbox.framework`. iOS and tvOS slices are
discarded. Override the source or destination with:

- `VOYAVPN_SING_BOX_REF`: sing-box git ref, defaults to the app's pinned
  sing-box version.
- `VOYAVPN_SING_BOX_SOURCE_DIR`: local sing-box source checkout.
- `VOYAVPN_LIBBOX_FRAMEWORK`: existing or target universal macOS `Libbox.framework` path.

`pnpm native:macos:tunnel` stages one PacketTunnel provider shape:

- Developer ID direct distribution:
  `VoyaVPN.app/Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension`
- App Store/TestFlight or unsigned development:
  `VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex`
- `Contents/Frameworks/Libbox.framework` under the selected provider bundle
  only when the staged framework is dynamic.

When the selected `Libbox.framework` slice is static, the script links Libbox
symbols into `VoyaPacketTunnel` and intentionally does not embed a framework in
the extension bundle.

By default the staged app bundle is
`target/native/macos/VoyaVPN.app`. To inject the native tunnel into a real Tauri
bundle, set:

```sh
export VOYAVPN_MACOS_APP_BUNDLE="$PWD/target/release/bundle/macos/VoyaVPN.app"
pnpm native:macos:tunnel
```

For release or local TUN-test packaging, build the Tauri macOS bundle with
`pnpm tauri:build --bundles app`, then run the tunnel staging, signing, and
`pnpm native:macos:dmg`. The DMG helper copies the final signed `.app` into the
image and mounts it to verify that the selected PacketTunnel provider bundle
and `VoyaPacketTunnel` are present before the DMG is accepted.

Local TUN testing: `pnpm build:mac:local` runs the whole chain without Apple
notarization — Apple Development signing, development provisioning profiles,
`.appex` packaging, install into `/Applications`, and PlugInKit repair. Run
`pnpm native:macos:preflight` first; the full runbook is
[docs/release/macos-local-tun-testing.md](../../../../../docs/release/macos-local-tun-testing.md).

Set `VOYAVPN_CODESIGN_IDENTITY` to codesign the staged dynamic Libbox framework
when present and the extension. The final App Store/TestFlight lane must sign
the containing app with `src-tauri/entitlements/macos-app.plist` and the
extension with `src-tauri/entitlements/packet-tunnel.plist`.

Provisioning profiles are discovered from `VOYAVPN_PROVISIONING_PROFILE_DIR`
when set, otherwise from `../docs/certs` when that directory exists. Override
individual profiles with:

- `VOYAVPN_MACOS_APP_PROVISIONING_PROFILE`
- `VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE`

When profiles are present, `pnpm native:macos:tunnel` embeds them as
`Contents/embedded.provisionprofile` in the containing app and PacketTunnel
extension and generates signing entitlements from the profile app identifiers.
Set `VOYAVPN_REQUIRE_PROVISIONING=1` in App Store/TestFlight lanes so missing or
mismatched profiles fail the build.

Release signing and notarization helpers:

```sh
pnpm native:macos:app:sign
pnpm native:macos:app:notarize
```

`native:macos:app:notarize` uses `VOYAVPN_NOTARY_KEYCHAIN_PROFILE` when set.
Set `VOYAVPN_NOTARY_KEYCHAIN` too when the profile lives in a specific keychain.
Otherwise it uses `VOYAVPN_NOTARY_APPLE_ID`, `VOYAVPN_NOTARY_TEAM_ID`, and
`VOYAVPN_NOTARY_PASSWORD`. Do not commit these values.

Use a `Developer ID Application` identity plus notarization and stapling for a
`.app` or DMG that users can copy directly to `/Applications`. Developer ID
Network Extension builds must package PacketTunnel as a System Extension and use
`packet-tunnel-provider-systemextension`; signing that entitlement into an
`.appex` causes macOS to reject the provider before `startTunnel` runs. `3rd
Party Mac Developer` or Apple Distribution identities are for App
Store/TestFlight submission; those artifacts keep the `.appex` shape and should
be installed through App Store Connect/TestFlight instead of launched directly
from Finder. Because VoyaVPN uses Network Extension and App Group entitlements,
Developer ID direct builds also need Developer ID provisioning profiles for the
containing app and PacketTunnel provider.

Provisioning requirements:

- App ID: `app.voyavpn.desktop`
- PacketTunnel App ID: `app.voyavpn.desktop.PacketTunnel`
- App Group: `group.app.voyavpn.desktop`
- Network Extension capability with `packet-tunnel-provider-systemextension`
  for Developer ID direct distribution, or `packet-tunnel-provider` for App
  Store/TestFlight and unsigned development.

`pnpm native:macos:tunnel:verify` checks the staged app and PacketTunnel
extension, either static Libbox symbols or an embedded dynamic
`Libbox.framework`, embedded provisioning profiles when present, code
signatures, and required entitlement strings. Set `VOYAVPN_REQUIRE_LIBBOX=1`,
`VOYAVPN_REQUIRE_CODESIGN=1`, or
`VOYAVPN_REQUIRE_PROVISIONING=1` to make those checks hard failures in release
lanes.

If `Libbox.framework` is absent, the PacketTunnel provider still builds but
fails closed at runtime with a clear "requires the sing-box Apple/libbox
runtime" error. It does not report a connected VPN without an active sing-box
runtime.

## Provider Registration Hygiene

macOS elects app-extension PacketTunnel providers globally by bundle id through
PlugInKit. Developer ID System Extension builds are tracked through
`systemextensionsctl` and must be approved by the user the first time they are
activated.

Check the elected provider with:

```sh
pnpm native:macos:ne:doctor
```

After launching local app bundles with the PacketTunnel provider, quit VoyaVPN
and repair registration state with:

```sh
pnpm native:macos:ne:doctor --fix
```

For repo release-bundle tests:

```sh
pnpm native:macos:ne:doctor --fix --app "$PWD/target/release/bundle/macos/VoyaVPN.app" --dev
```

Fixtures that do not exercise NetworkExtension should remove both
`Contents/PlugIns` and `Contents/Library/SystemExtensions` before launch. See
`docs/release/macos-networkextension-troubleshooting.md` for symptoms, raw
registration commands, and manual VPN-profile cleanup steps.
