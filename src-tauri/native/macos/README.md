# macOS Native Tunnel

VoyaVPN's macOS transparent tunnel is designed to match the system VPN model
used by clients such as V2Box.

Runtime shape:

- Containing app bundle id: `app.voyavpn.desktop`
- PacketTunnel extension bundle id: `app.voyavpn.desktop.PacketTunnel`
- App Group: `group.app.voyavpn.desktop`
- Runtime config file: `Library/Application Support/VoyaVPN/packet-tunnel-runtime.json`

The Tauri app writes the generated sing-box JSON into the App Group container
and starts the VPN profile through NetworkExtension. The extension then owns the
packet tunnel and runs the embedded sing-box Apple/libbox runtime.

Build helper:

```sh
pnpm native:macos:libbox
pnpm native:macos:tunnel
pnpm native:macos:tunnel:verify
```

`pnpm native:macos:libbox` clones the pinned sing-box source tag and builds the
Apple `Libbox.xcframework`. By default it stages the framework at
`src-tauri/native/macos/Frameworks/Libbox.xcframework`. Override the source or
destination with:

- `VOYAVPN_SING_BOX_REF`: sing-box git ref, defaults to the app's pinned
  sing-box version.
- `VOYAVPN_SING_BOX_SOURCE_DIR`: local sing-box source checkout.
- `VOYAVPN_LIBBOX_XCFRAMEWORK`: existing or target `Libbox.xcframework` path.

`pnpm native:macos:tunnel` stages:

- `VoyaVPN.app/Contents/MacOS/voyavpn-macos-tunnelctl`
- `VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex`
- `VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex/Contents/Frameworks/Libbox.framework`
  when `Libbox.xcframework` is present.

By default the staged app bundle is
`target/native/macos/VoyaVPN.app`. To inject the native tunnel into a real Tauri
bundle, set:

```sh
export VOYAVPN_MACOS_APP_BUNDLE="$PWD/target/release/bundle/macos/VoyaVPN.app"
pnpm native:macos:tunnel
```

Set `VOYAVPN_CODESIGN_IDENTITY` to codesign the staged Libbox framework, helper,
and extension. The final App Store/TestFlight lane must sign the containing app
with `src-tauri/entitlements/macos-app.plist` and the extension with
`src-tauri/entitlements/packet-tunnel.plist`.

Release signing and notarization helpers:

```sh
pnpm native:macos:app:sign
pnpm native:macos:app:notarize
```

`native:macos:app:notarize` uses `VOYAVPN_NOTARY_KEYCHAIN_PROFILE` when set, or
`VOYAVPN_NOTARY_APPLE_ID`, `VOYAVPN_NOTARY_TEAM_ID`, and
`VOYAVPN_NOTARY_PASSWORD`. Do not commit these values.

Provisioning requirements:

- App ID: `app.voyavpn.desktop`
- Extension App ID: `app.voyavpn.desktop.PacketTunnel`
- App Group: `group.app.voyavpn.desktop`
- Network Extension capability with `packet-tunnel-provider` for the containing
  app and the PacketTunnel extension.

`pnpm native:macos:tunnel:verify` checks the staged files, embedded
`Libbox.framework`, code signatures, and required entitlement strings. Set
`VOYAVPN_REQUIRE_LIBBOX=1` or `VOYAVPN_REQUIRE_CODESIGN=1` to make those checks
hard failures in release lanes.

If `Libbox.xcframework` is absent, the PacketTunnel provider still builds but
fails closed at runtime with a clear "requires Libbox.xcframework" error. It
does not report a connected VPN without an active sing-box runtime.
