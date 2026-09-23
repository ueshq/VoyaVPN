# Mac App Store Package

`pnpm build:mac:appstore` produces the signed installer package that is
uploaded to App Store Connect for TestFlight and Mac App Store review:

```text
target/release/bundle/pkg/VoyaVPN_<version>_<build>_aarch64.pkg
```

It is the third macOS lane beside `pnpm build:mac` (notarized Developer ID
DMG, see [signing-notarization.md](signing-notarization.md)) and
`pnpm build:mac:local` (this-Mac-only TUN testing, see
[macos-local-tun-testing.md](macos-local-tun-testing.md)). The package is
**arm64 only** and requires **macOS 26** or later. App Store Connect needs at
least 12.0 for an arm64-only package (ITMS-90869), and the pinned upstream
sing-box seed is itself built for macOS 26.

## What differs from the other lanes

| | App Store package | Local / Developer ID |
| --- | --- | --- |
| Rust feature | `mac-app-store`: the Tauri updater plugin is not compiled in, and `app_update_status` reports `unsupported` so Settings hides the app-update panel | self-updater present |
| Tauri config | `target/release-config/tauri.mac-app-store.generated.json`: app bundle only, `LSMinimumSystemVersion` 26.0, `CFBundleVersion` = build number | `tauri.conf.json` |
| PacketTunnel | `Contents/PlugIns/VoyaPacketTunnel.appex` | appex (local) / System Extension (Developer ID) |
| Signing identity | `3rd Party Mac Developer Application` (or `Apple Distribution`) | Apple Development / Developer ID Application |
| Profiles | Mac App Store distribution profiles, no device list | development / Developer ID |
| Signed NetworkExtension values | `packet-tunnel-provider` only; no team wildcards, no keychain groups | everything the profile grants |
| Output | `.pkg` signed by `3rd Party Mac Developer Installer` | `.dmg` |
| Notarization | none (App Store Connect does not use notarytool) | Developer ID only |

In every lane the PacketTunnel is compiled for its container's
`LSMinimumSystemVersion` (raised to 11.0 on Apple Silicon) and declares the
same value in its own `Info.plist` (ITMS-90360). Its folder is named after its
executable, `VoyaPacketTunnel` (ITMS-90362).

Every lane now signs the bundled sing-box seed with
`apps/desktop/src-tauri/entitlements/macos-inherit.plist` (App Sandbox +
`inherit`) and removes the Windows-only `voyavpn-tunnel-service` from
`Contents/MacOS`. App Store validation rejects any nested executable that does
not enable the sandbox. The app entitlements also carry
`com.apple.security.files.user-selected.read-write` for the log-export save
panel.

## One-time setup

1. **Certificates.** Both must be in the login keychain:
   - `3rd Party Mac Developer Application: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)`
   - `3rd Party Mac Developer Installer: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)`

   Check them with:

   ```sh
   security find-identity -v | grep "3rd Party Mac Developer"
   ```

2. **Identifiers.** `app.voyavpn.desktop` and `app.voyavpn.desktop.PacketTunnel`
   both need App Groups (`group.app.voyavpn.desktop`) and Network Extensions.
3. **Profiles.** Create one **Mac App Store Connect** distribution profile per
   bundle id with the `3rd Party Mac Developer Application` certificate.
   Downloading them from Xcode puts them in
   `~/Library/MobileDevice/Provisioning Profiles`. Point the build at that
   folder, or at any folder that holds them:

   ```sh
   export VOYAVPN_PROVISIONING_PROFILE_DIR="$HOME/Library/MobileDevice/Provisioning Profiles"
   ```

   You can also name each file with `VOYAVPN_MACOS_APP_PROVISIONING_PROFILE`
   and `VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE`. Development and
   Developer ID profiles in the same folder are ignored. The store lane does
   not accept development provisioning.
4. **App Store Connect.** The app record for bundle id `app.voyavpn.desktop`
   must exist before the first upload.
5. **Libbox.** `apps/desktop/src-tauri/native/macos/Frameworks/Libbox.framework`
   must exist; build it with `pnpm native:macos:libbox` if not.

## Build

Run on an Apple Silicon Mac. The script refuses to run on Intel.

```sh
export VOYAVPN_PROVISIONING_PROFILE_DIR="$HOME/Library/MobileDevice/Provisioning Profiles"
pnpm build:mac:appstore
```

The script runs these steps:

1. `tauri:build --bundles app` with the store overlay and the
   `mac-app-store` feature.
2. `native:macos:tunnel` to build and sign the PacketTunnel appex.
3. `native:macos:app:sign`.
4. `native:macos:tunnel:verify`.
5. `native:macos:pkg`.

Before it writes the `.pkg`, `native:macos:pkg` checks the following:

- Both embedded profiles are store distribution profiles.
- The app is signed by a store application identity.
- `codesign --verify --deep --strict` passes.
- Every executable in the bundle enables App Sandbox and has an arm64 slice.
- No executable targets a newer macOS than the app's `LSMinimumSystemVersion`,
  and an arm64-only app declares at least 12.0 (ITMS-90869).
- The PacketTunnel declares the app's `LSMinimumSystemVersion` (ITMS-90360),
  and its folder name equals its `CFBundleExecutable` (ITMS-90362).
- No System Extension, `export-bindings` or tunnel service is bundled, and no
  appex is left under the old bundle-id folder name.
- No file carries `com.apple.quarantine` (ITMS-91109). This is checked on the
  bundle, and again on the finished `.pkg` after expanding it, because
  `productbuild` keeps extended attributes in the payload.

Provisioning profiles saved from a browser are quarantined, and build 352
shipped both embedded profiles with that attribute. The lane now writes
profiles into the bundle as plain bytes. `native:macos:app:sign` also removes
any remaining quarantine before signing, and lists the files it cleaned.

Afterwards the script removes `Contents/PlugIns` from the `target/` app copy.
That keeps the copy from winning PlugInKit election for the production bundle
id. A store-signed app cannot launch outside the store anyway, and the `.pkg`
is the artifact.

The script refuses `VOYAVPN_RELEASE_CHANNEL=stable` together with the store
build, because the stable channel enables the updater.

### Build number

App Store Connect rejects a second upload with the same `CFBundleVersion` for
one marketing version. The build number is `VOYAVPN_MACOS_BUILD_NUMBER`. When
that is unset, it is the commit count of `HEAD`. Set the variable explicitly
to upload the same commit twice, or to build from a branch whose commit count
is lower than an earlier upload's:

```sh
VOYAVPN_MACOS_BUILD_NUMBER=412 pnpm build:mac:appstore
```

It takes one to three period-separated integers. The PacketTunnel copies both
version fields from the app, which avoids ITMS-90473.

## Upload

1. Open **Transporter**, sign in with the team's Apple ID, and add the `.pkg`.
2. Choose **Verify**. This runs the same ITMS checks as delivery, including
   sandbox, nested code and entitlements.
3. Choose **Deliver**.
4. In App Store Connect, answer the export-compliance questions for the build.
   VoyaVPN uses encryption beyond the exempt categories, and `Info.plist` does
   not declare `ITSAppUsesNonExemptEncryption`, so the question is asked
   per build.
5. Install the build from TestFlight and run the acceptance in
   [macos-vpn-manual-proxy-acceptance.md](macos-vpn-manual-proxy-acceptance.md).
   Cover VPN authorization, TUN traffic, the latency test while disconnected
   (it runs the sandboxed seed), and log export to a user-chosen folder.

## Known gaps for review

- **Licensing.** The bundled sing-box seed and the Libbox statically linked
  into the PacketTunnel are both GPL-3.0-or-later. The redistribution approval
  in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and ADR 0004 is still
  open. GPL terms and the App Store terms are widely considered incompatible,
  so settle this before a public release. TestFlight distribution raises the
  same question.
- **Launch at login.** Autostart writes a LaunchAgent under `~/Library`. Inside
  the sandbox that path is redirected into the app container, so the setting
  has no effect in the store build. A store-safe version needs `SMAppService`.
- **Architecture and OS floor.** The package is arm64 only and macOS 26 only.
  Lowering the floor needs a sing-box seed built from source for that release;
  the upstream darwin-arm64 archive is built for macOS 26. A universal package
  would also need an x86_64 seed merged with `lipo`, an x86_64 appex slice, and
  a universal Rust build.
