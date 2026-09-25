# macOS Local TUN Testing Without Notarization

`pnpm build:mac:local` produces a locally runnable `VoyaVPN.app` that can test
TUN mode on this Mac without Apple notarization. It signs with an **Apple
Development** certificate, packages the PacketTunnel provider as an
App-Store-shaped `.appex` (`Contents/PlugIns/`), embeds **macOS App
Development** provisioning profiles, installs the app into `/Applications`, and
repairs PlugInKit registrations. The artifact runs only on Macs listed in the
development profiles and must never be distributed; `pnpm build:mac` remains
the notarized Developer ID lane described in
[signing-notarization.md](signing-notarization.md).

## One-time Apple Developer portal setup

1. **Certificate** — Certificates → `+` → *Apple Development*. Upload the CSR
   from the profile directory (default `../docs/certs`,
   `CertificateSigningRequest.certSigningRequest`), download the `.cer`, and
   double-click to install it into the login keychain. Confirm with:

   ```sh
   security find-identity -v -p codesigning | grep "Apple Development"
   ```

   If the identity does not show as valid, install Apple's WWDR intermediate
   certificate (G3) and confirm the CSR's private key is in the keychain.
2. **Device** — Devices → `+` → register this Mac. Read the Provisioning UDID
   with:

   ```sh
   system_profiler SPHardwareDataType | grep "Provisioning UDID"
   ```

3. **Identifiers** — confirm `app.voyavpn.desktop` and
   `app.voyavpn.desktop.PacketTunnel` both have the App Groups
   (`group.app.voyavpn.desktop`) and Network Extensions capabilities enabled.
4. **Profiles** — create two *macOS App Development* profiles, one per bundle
   id, selecting the Apple Development certificate and this Mac. Download both
   into the profile directory (default `../docs/certs`, override with
   `VOYAVPN_PROVISIONING_PROFILE_DIR`). Development, App Store, and Developer
   ID profiles can coexist there; the build scripts select the profile whose
   `DeveloperCertificates` contains the active signing certificate and whose
   `ProvisionedDevices` covers this Mac.

No system-level changes are required: SIP stays on, and
`systemextensionsctl developer on` is not needed because the local lane uses a
PlugInKit app extension, not a system extension.

## Preflight

```sh
pnpm native:macos:preflight
```

| Check | Fix when it fails |
| --- | --- |
| Xcode developer dir / `swiftc` | Install Xcode or `xcode-select --install`. |
| Apple Development signing identity | Portal step 1 above; or set `VOYAVPN_CODESIGN_IDENTITY`. |
| Provisioning UDID readable | Set `VOYAVPN_PROVISIONING_UDID` if `system_profiler` cannot report it. |
| Development profile per bundle id | Portal step 4; the report lists why each candidate profile was rejected. |
| Universal macOS `Libbox.framework` with arm64 and x86_64 | `pnpm native:macos:libbox`. |

The profile checks mirror the exact selection criteria the build uses
(bundle id, certificate fingerprint, device coverage, expiry), so a green
preflight means the signed app will pass AMFI at launch.

## Build and install

```sh
pnpm build:mac:local
```

The script runs preflight, then: `tauri:build --bundles app` →
`native:macos:tunnel` → `native:macos:app:sign` → `native:macos:tunnel:verify`
→ `native:macos:dmg`. Before replacing the installed app, quit the VoyaVPN GUI.
If only the VoyaVPN PacketTunnel remains active, the script stops the connection
whose VPN provider is exactly `app.voyavpn.desktop`, then waits for the provider
to exit. It never kills the provider or removes the VPN profile. The script then
installs the app into `/Applications/VoyaVPN.app` → removes `Contents/PlugIns`
from the leftover `target/` build and DMG-staging copies so they cannot win
PlugInKit election → `native:macos:ne:doctor --fix --app
/Applications/VoyaVPN.app`.

Launch the installed copy, not the `target/` copies:

```sh
open -n /Applications/VoyaVPN.app
```

## First run and verifying the tunnel

1. Enable TUN mode in the app. macOS asks *“VoyaVPN” Would Like to Add VPN
   Configurations* — allow it. Recent macOS releases may also ask once about
   App Group container access.
2. Verify the tunnel:

   ```sh
   ifconfig | grep utun            # a new utun interface appears
   scutil --nc list | grep VoyaVPN # shows Connected
   curl https://api.ipify.org      # exit IP goes through the proxy
   ```

## Environment overrides

| Variable | Effect |
| --- | --- |
| `VOYAVPN_CODESIGN_IDENTITY` | Explicit signing identity (SHA-1 or name) instead of the first Apple Development match. |
| `VOYAVPN_PROVISIONING_PROFILE_DIR` | Profile directory instead of `../docs/certs`. |
| `VOYAVPN_MACOS_APP_PROVISIONING_PROFILE` / `VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE` | Explicit profile paths; they are still validated against the signing certificate and device. |
| `VOYAVPN_PROVISIONING_UDID` | Override the detected Provisioning UDID. |
| `VOYAVPN_MACOS_DMG_PATH` / `VOYAVPN_MACOS_DMG_ARCH` | DMG output path/arch suffix. |
| `VOYAVPN_MACOS_DMG_DIR` | Directory the DMG is written to when `VOYAVPN_MACOS_DMG_PATH` is unset (default `target/release/bundle/dmg`). |

## Troubleshooting and teardown

- **App is killed instantly at launch** — check the VoyaVPN report in
  `~/Library/Logs/DiagnosticReports/`. An AMFI/code-signing rejection points to
  a signature, profile, or device mismatch; re-run
  `pnpm native:macos:preflight` for the failing profile and reason.
- **Startup crashes with `SIGABRT` / Rust panic** — run
  `RUST_BACKTRACE=1 /Applications/VoyaVPN.app/Contents/MacOS/voyavpn` in a
  terminal to capture the original panic. `EventRegistry not found in Tauri
  state` identifies an older build that forwarded startup warnings to the log
  panel before mounting typed events. Rebuild with `pnpm build:mac:local`.
  The macOS “reopen windows” alert is a consequence of the crash, not its cause.
- **TUN fails with `ProviderPathMismatch`** — another copy of the appex is
  elected. Quit VoyaVPN and run
  `pnpm native:macos:ne:doctor --fix` (defaults to `/Applications/VoyaVPN.app`).
- **TUN fails with `command.sock: bind: invalid argument`** — the libbox
  command-socket path exceeds the macOS 104-byte `sun_path` limit; this is a
  stale provider built before the libbox base dir moved to `PT/` at the App
  Group container root. Rebuild with `pnpm build:mac:local`.
- **TUN fails with `listen tcp 127.0.0.1:<port>: bind: operation not
  permitted`** — the bundle was signed without the
  `com.apple.security.network.server` entitlement, so the sandboxed provider
  cannot open the local mixed/socks inbound. Rebuild with
  `pnpm build:mac:local`.
- Deeper NetworkExtension issues:
  [macos-networkextension-troubleshooting.md](macos-networkextension-troubleshooting.md).
- **Teardown** — disable TUN, quit the app, remove the VPN configuration from
  System Settings → VPN if desired, delete `/Applications/VoyaVPN.app`, and run
  `pnpm native:macos:ne:doctor --fix` to clear the registration.

## VPN and manual proxy release acceptance

All macOS channels keep App Sandbox. System proxy setup and restoration are
manual, including local development; VPN remains managed by NetworkExtension.
Follow the [VPN acceptance matrix](macos-vpn-manual-proxy-acceptance.md)
for ten real reconnects, failure cleanup and proxy restoration. Automated
waiter tests do not alter the current VPN or system proxy.
