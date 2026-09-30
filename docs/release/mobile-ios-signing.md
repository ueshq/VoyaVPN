# iOS Build, Signing and Device Acceptance

VoyaVPN on iOS is two bundles: the app and a `NEPacketTunnelProvider` app
extension that runs the sing-box core. Both need Apple developer identities,
matched provisioning profiles, and the Network Extension and App Group
capabilities. This is the procedure for getting from a clean checkout to a
build on a device.

## Identifiers

These are fixed. **None of them may be shared with the desktop app**: macOS
elects app-extension PacketTunnel providers globally by bundle id through
PlugInKit, so a bundle claiming `app.voyavpn.desktop.PacketTunnel` can become
the elected provider for the desktop build on the same machine. See the
NetworkExtension hygiene section in `AGENTS.md`.

| | Identifier |
| --- | --- |
| App | `app.voyavpn.mobile` |
| PacketTunnel extension | `app.voyavpn.mobile.PacketTunnel` |
| App Group | `group.app.voyavpn.mobile` |

The App Group appears in four places and all four must agree:

- `apps/mobile/ios/VoyaVPN/Info.plist` (`VoyaAppGroupIdentifier`)
- `apps/mobile/ios/VoyaVPN/VoyaVPN.entitlements`
- `apps/mobile/ios/PacketTunnel/Info.plist` (`VoyaAppGroupIdentifier`)
- `apps/mobile/ios/PacketTunnel/PacketTunnel.entitlements`

The shared provider sources read the identifier from the extension's
`Info.plist` rather than hardcoding it, which is what lets macOS and iOS ship
the same Swift. An extension that declares neither the key nor the entitlement
fails closed with `missingAppGroupContainer` rather than reaching into another
app's container.

## Apple Developer portal

1. Register both App IDs, with **App Groups** and **Network Extensions**
   enabled on each.
2. Register the App Group `group.app.voyavpn.mobile` and assign it to both.
3. Create development and distribution provisioning profiles for both App IDs.
   Four profiles in total; a profile for the app does not cover the extension.

## Prerequisites on the build machine

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
xcode-select --install          # or a full Xcode
```

Go and gomobile come from sing-box's own `make lib_install`, which
`pnpm native:mobile:libbox:ios` runs.

## Build the native artifacts

Both are gitignored build outputs, and an Xcode build needs both:

```sh
pnpm native:mobile:libbox:ios    # Libbox.xcframework      → apps/mobile/ios/Frameworks/
pnpm native:mobile:rust:ios      # VoyaMobile.xcframework  → apps/mobile/ios/Frameworks/
                                 # + Swift bindings        → apps/mobile/ios/VoyaVPN/Generated/
```

`native:mobile:rust:*` builds the `release` cargo profile; set
`VOYAVPN_RUST_PROFILE=debug` for a faster unoptimised library while iterating.

Then the CocoaPods dependencies React Native itself needs:

```sh
cd apps/mobile/ios && pod install
```

## Xcode target setup

```sh
pnpm run native:mobile:ios:project
```

`scripts/native/mobile/ios-project.rb` does the whole of it, through the
`xcodeproj` gem, and is idempotent — run it again after `pod install` or a
React Native upgrade, both of which rewrite parts of the project and drop our
half. The Node wrapper beside it only finds a Ruby that can load the gem;
CocoaPods bundles one, so no extra install is needed.

Editing the `.pbxproj` by hand instead is how project files get corrupted, and
the script is also the record of what the project contains. What it sets up:

### The app target

- *Compile Sources* gains `VoyaVPN/Native/*.swift`, `VoyaVPN/Native/VoyaNative.m`
  and `VoyaVPN/Generated/voya_mobile_ffi.swift`.
- *Link Binary With Libraries* gains `VoyaMobile.xcframework`,
  `Libbox.xcframework` and `NetworkExtension.framework`. The app links Libbox
  for the disconnected latency test (ADR 0013); this is the change that grows
  the app binary, and its size is worth watching.
- `SWIFT_OBJC_BRIDGING_HEADER = VoyaVPN/Native/VoyaVPN-Bridging-Header.h`, which
  imports `voya_mobile_ffiFFI.h`. The generated modulemap is not named
  `module.modulemap`, so Xcode never treats it as a module and
  `canImport(voya_mobile_ffiFFI)` is always false — the bridging header is the
  only route in, and the two mechanisms must not both be active or the same C
  declarations arrive twice.
- `CODE_SIGN_ENTITLEMENTS = VoyaVPN/VoyaVPN.entitlements`,
  `FRAMEWORK_SEARCH_PATHS` and `HEADER_SEARCH_PATHS` for the two generated
  directories, and two linker flags the Go runtime inside Libbox needs:
  `-lresolv` (`res_9_ninit` and friends) and `-Wl,-no_compact_unwind` (Go has
  more personality routines than compact unwind can encode).
- `EXCLUDED_ARCHS[sdk=iphonesimulator*] = x86_64` at the project level: a
  Release build otherwise builds both simulator architectures and the Rust
  slices are arm64-only.

### The PacketTunnel target

- An app extension, bundle id `app.voyavpn.mobile.PacketTunnel`, with
  `PRODUCT_MODULE_NAME = PacketTunnel` — `Info.plist` names the principal class
  `$(PRODUCT_MODULE_NAME).PacketTunnelProvider`, so the module name is load
  bearing.
- *Compile Sources* references the four files in `native/apple/PacketTunnel/`
  **in place**. They are not copied, or macOS and iOS drift.
- Links `Libbox.xcframework` and `NetworkExtension.framework` only. It does not
  link the Rust host: it runs the core, and the host runs in the app.
- `MARKETING_VERSION` is the root `package.json` version, which the script
  writes to the app and the extension alike; `pnpm run check:architecture`
  requires one release version across the whole workspace. A version bump is
  "edit `package.json`, run `pnpm run native:mobile:ios:project`".
  `CURRENT_PROJECT_VERSION` stays `1` in the project. The store build passes
  the real build number on the command line, which reaches both targets.
- Embedded in the app through a PlugIns copy-files phase.

### The VoyaVPNUITests target

A UI-testing bundle, `app.voyavpn.mobile.uitests`, with
`TEST_TARGET_NAME = VoyaVPN` and a generated `Info.plist`. It deliberately sets
*no* `MARKETING_VERSION` or `CURRENT_PROJECT_VERSION`: a test bundle's version
is meaningless and setting one trips the version-alignment gate above. The
script also repoints the shared scheme's `TestAction`, which the React Native
template left pointing at a `VoyaVPNTests.xctest` that has never existed.

### Signing

For development, signing is done in Xcode, once: select the team on both
targets, and add **Network Extensions** and **App Groups**
(`group.app.voyavpn.mobile`) to each under *Signing & Capabilities*. Xcode then
manages the development profiles.

The App Store build does not use those settings. `pnpm build:ios:appstore`
passes its own identity and profiles on the command line; see
[Distribution](#distribution). Nothing about store signing is written into the
project.

### Memory

iOS kills a NetworkExtension provider at roughly 50 MB, and sing-box with a
large rule set is the classic way to exceed it. On iOS the provider calls
`LibboxSetMemoryLimit(true)` between `LibboxSetup` and
`LibboxNewCommandServer`. That makes libbox collect at 10% heap growth, cap
the Go heap at 45 MiB, and enable its OOM killer. The order matters, because
the command server reads the flag when it is created. macOS does not make the
call. The limit bounds the Go heap, not the whole process, so memory under a
large rule set is still the first thing to measure on a device.


## Running

```sh
pnpm --filter @voya/mobile ios          # simulator: UI and every command
```

The simulator cannot start a NetworkExtension tunnel. Everything else works:
the database, config generation, imports, routing, DNS, settings. The tunnel
reports `missingComponent` and connecting fails cleanly. That is the line the
acceptance list below is split along.

**Debug and a Mac HTTP proxy.** The iOS Simulator inherits CFNetwork’s system
proxy settings. A proxy on `127.0.0.1` (Clash, V2Ray, …) without a
`127.0.0.1`/`localhost` exception makes `RCTBundleURLProvider`’s packager
probe fail and a Debug launch redbox with `No script URL provided`. Debug
builds therefore mint `http://localhost:8081` directly and force URLSession to
skip proxy lookup, so Metro is reachable with the proxy on. Release still embeds
`main.jsbundle` and never talks to Metro; if that file is missing from the
product you get the same nil script URL. Keep `127.0.0.1, localhost` in the
system proxy bypass list if you also need the host browser or other tools to
reach Metro.

For a device, start Metro (`pnpm --filter @voya/mobile start`), open
`apps/mobile/ios/VoyaVPN.xcworkspace`, select the device and run. On a device
`localhost` is the phone, so a Debug build reads the Mac's LAN address from
`ip.txt`, which `react-native-xcode.sh` writes into every Debug device build.
The phone and the Mac must be on the same network, the Mac's firewall must let
`node` accept incoming connections, and the app's first launch raises the
"Local Network" prompt, which must be allowed. If `ip.txt` picked the wrong
interface, set the right `IP:8081` under Dev Menu → Configure Bundler; that
value wins over `ip.txt` until it is reset. The first connection raises the system's "VoyaVPN would like to add VPN
configurations" prompt; declining it surfaces as a permission failure on the
Home screen rather than as an error.

## Acceptance

Split in two, because the two halves cost very different amounts of attention.
The simulator half runs itself; the device half is the one a person has to sit
through, and it is the one that says the milestone is done.

### Simulator: automated

Run from the repository root on an Apple silicon Mac with Xcode, an installed
stable iOS runtime, CocoaPods, Go, Rust, Python 3 and pnpm:

```sh
pnpm check:mobile:ios:smoke
pnpm check:mobile:ios:full # release matrix
pnpm check:mobile:ios:full --matrix-only # layout iteration; excludes business flows
```

The runner creates and deletes its own simulator; it never erases a developer's
existing device. It builds Release from the real Rust host and pinned Libbox,
reuses CocoaPods only when dependency/Podfile fingerprints and lockfiles match,
starts a local VLESS server and a private-LAN HTTP subscription fixture, and
cleans up the services on completion. A private LAN IPv4 interface is required:
loopback subscription URLs remain rejected by the product's security policy.
If Simulator is open, turn off **Edit > Automatically Sync Pasteboard** first;
the preflight rejects host clipboard synchronization instead of letting unrelated
clipboard changes alter a test. The runner never changes that global preference.
For local iteration with an unchanged Libbox pin, `--reuse-libbox` explicitly
reuses the already staged framework; CI always builds the pinned source.

Each XCTest business flow imports its own data. Only latency target URLs and
the timeout are configured in the test device's database; no nodes or
subscriptions are preseeded. The smoke covers:

- The first-launch "Before you start" notice: every case accepts it after
  launching, and the first launch on each simulator attaches a screenshot of it.
- Real backend launch, all five tabs and foreground restoration.
- Clipboard validation, node import, duplicate import, search, selection,
  semantic sheet buttons, exported link, QR image decoding and deletion after
  a process restart.
- Subscription download immediately after import, automatic policy group,
  individual/all refresh and read-only subscription node actions.
- A successful real VLESS latency measurement, cancellation, timeout and retry;
  a native host test also checks long paths, failed-start cleanup, overlapping
  starts, idempotent stop and port reuse.
- Rule modes/toggles, DNS validation and settings persistence across restarts.

The full run adds rule library download and the five pages in English,
Simplified/Traditional Chinese, light/dark, default/maximum accessibility text,
and approximately 375/402/440pt widths. Inspect the resulting screenshots for
layout clipping and contrast; automated navigation alone is not visual approval.
VoiceOver focus announcements and return focus also need a manual accessibility
pass. `.github/workflows/ios-simulator.yml` runs the smoke for relevant PRs and
accepts a full-matrix manual dispatch; it is deliberately separate from
`verify:local`.

Every run writes `.agents/docs/ios-repair-<timestamp>/` (override with
`VOYA_IOS_QA_OUTPUT`): environment, fixtures, build logs, XCTest result bundles,
screenshots/accessibility attachments, native logs and a classified summary.
Environment/build failures are reported separately from product assertions.
A UI assertion failure still allows the remaining independent size/text cases
to collect evidence; the command exits unsuccessfully if any case failed.
A simulator VPN rejection is expected and is never counted as tunnel acceptance.

### Device only

A simulator proves none of this.

1. Import a share link, select the node, connect. The Home screen shows an exit
   IP and live up/down rates — which proves the app process reached the Clash
   API *inside the extension process* over loopback.
2. The system's "VoyaVPN would like to add VPN configurations" prompt appears on
   that first connection, and declining it surfaces as a permission failure on
   the Home screen rather than as an error.
3. Disconnect. The tunnel goes down and the state settles on disconnected.
4. Kill the app and reopen it. The state is the same one the system has.
5. Background the app and come back. The log stream and the connection monitor
   stop while it is away and resume on return, and the core state is re-read —
   `AppState` drives the visibility seam, and `useRuntimeStatusSeed` the re-read.
6. Run a latency test while disconnected. Nodes report measurements, which
   proves the in-app probe core started (ADR 0013).
7. Run a latency test while connected. Measurements again, this time through
   the running core's Clash API.
8. Connect with a large rule set (a subscription with a full ruleset, not two
   manual nodes) and leave it up. The extension must not be killed for memory.
   Attach Xcode to the `PacketTunnel` process and watch the memory gauge in
   the Debug navigator; it has to stay under the 50 MB ceiling.
9. Refresh a real subscription over the network and use the policy group
   automatically created by its import.
10. Exercise VPN authorization allow, deny and revoke in system Settings.
11. Confirm exit IP, traffic counters, connection list and close-connection
    controls against the actual tunnel; verify routing and DNS using controlled
    domains and destinations.
12. Switch Wi-Fi/cellular, background/restore, and exercise kill switch behavior
    during connection loss. Verify recovery and that blocked traffic does not
    escape through the physical interface.

If step 1 fails only when the kill switch is on, the cause is loopback under
`includeAllNetworks`. The fallback is libbox's `CommandClient` over
`command.sock` in the App Group container: `ClashApiEndpoint.host` is already a
field, so it is a change to `loopback()` and its two consumers rather than a
redesign.


## Distribution

`pnpm build:ios:appstore` produces the package Transporter uploads to App
Store Connect for TestFlight and App Review:

```text
target/release/bundle/ios/VoyaVPN_<version>_<build>.ipa
target/release/bundle/ios/VoyaVPN.xcarchive        # keeps the dSYMs
```

Libbox is GPL-3.0-or-later, so any build handed to a third party carries the
obligations in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). GPL terms and
the App Store terms are widely considered incompatible. Settle that before a
public release; TestFlight distribution raises the same question.

### One-time setup

1. **Certificate.** An **Apple Distribution** certificate for the team, with
   its private key, in the login keychain. The Mac App Store certificate
   (`3rd Party Mac Developer Application`) cannot sign an iOS app.

   ```sh
   security find-identity -v -p codesigning | grep "Apple Distribution"
   ```

2. **Profiles.** One **App Store Connect** distribution profile for each of
   `app.voyavpn.mobile` and `app.voyavpn.mobile.PacketTunnel`, made with that
   certificate. Put the two `.mobileprovision` files in `../docs/certs`, or
   double-click them, which installs them into
   `~/Library/MobileDevice/Provisioning Profiles`. The build looks in both and
   stops before it compiles anything when either is missing. To search one
   folder only, set `VOYAVPN_PROVISIONING_PROFILE_DIR`. To name the files, set
   `VOYAVPN_IOS_APP_PROVISIONING_PROFILE` and
   `VOYAVPN_IOS_PACKET_TUNNEL_PROVISIONING_PROFILE`. The development profiles
   Xcode manages are not accepted.
3. **App record.** The App Store Connect record for `app.voyavpn.mobile` must
   exist before the first upload.

### Build

```sh
pnpm build:ios:appstore                 # full build, signed, exported
pnpm build:ios:appstore --reuse-libbox  # keep the staged Libbox.xcframework
pnpm build:ios:appstore --skip-native   # skip Rust, Libbox, pods and the project step
pnpm build:ios:appstore --unsigned      # archive and check, no signature, no .ipa
```

`--unsigned` needs no certificate. It runs every check that does not depend on
a signature, which makes it the way to try the lane on a machine that cannot
sign.

The script runs these steps and writes one log per step into
`target/release/bundle/ios/logs/`:

1. Preflight, before anything is built: the distribution identity, both
   profiles, and the checked-in inputs that `pnpm check:mobile:ios:assets`
   also checks.
2. The native artifacts: the Rust host, Libbox, CocoaPods when the lockfiles
   changed, and `native:mobile:ios:project`.
3. `xcodebuild archive` for `generic/platform=iOS`, arm64, with manual signing.
4. Checks on the archived app.
5. `xcodebuild -exportArchive` with a generated `ExportOptions.plist`
   (`app-store-connect`, manual signing, both profiles).

What it checks:

- Each profile is a store profile for its bundle id, grants the App Group
  `group.app.voyavpn.mobile` and `packet-tunnel-provider`, and does not allow
  debugging.
- Every icon in the set is present, the right size, and opaque. App Store
  Connect rejects a 1024px icon with an alpha channel (ITMS-90717).
- Every purpose string in `Info.plist` is non-empty and translated in all
  three languages.
- The app and the extension name the same App Group in both `Info.plist`s and
  both entitlements files, and claim only `packet-tunnel-provider`.
- The built app and extension carry the release version and the same build
  number (ITMS-90473), and target iOS 15.1.
- No Mach-O imports a symbol App Review has named as non-public or links a
  private library (Guideline 2.5.1, the check that stopped the 2026-09 Mac
  submission), and every one is arm64 only. `pnpm native:mobile:ios:verify-imports <VoyaVPN.app>`
  runs the import scan on its own.
- The signed entitlements are exactly the application identifier, the team,
  `packet-tunnel-provider` and the App Group, with no `get-task-allow`.
- Each bundle embeds the profile that was selected.

Signing is manual on purpose. The profiles are files that can be read and
checked before a twenty-minute build, and the same ones every time. A
command-line build setting applies to every target, CocoaPods ones included,
so the script selects the profile through a macro keyed by product name,
`PROVISIONING_PROFILE_SPECIFIER=$(VOYA_PROFILE_$(PRODUCT_NAME))`, which
resolves to nothing for a pod. The settings it used are also written to
`target/release/bundle/ios/signing.xcconfig`.

### Build number

App Store Connect rejects a second upload with the same `CFBundleVersion` for
one marketing version. The build number is `VOYAVPN_IOS_BUILD_NUMBER`, or the
commit count of `HEAD` when that is unset, the same rule as the Mac package.
Set the variable to upload the same commit twice.

### Icons

The icon set is checked in as opaque RGB PNGs. To regenerate it from a new
source image:

```sh
apps/desktop/node_modules/.bin/tauri icon apps/desktop/src-tauri/app-icon.svg \
  --ios-color "#1A58F2" -o <scratch>
cp <scratch>/ios/*.png apps/mobile/ios/VoyaVPN/Images.xcassets/AppIcon.appiconset/
pnpm native:mobile:ios:icons
```

The last command removes the alpha channel the generator writes, then checks
the set.

### Upload

1. Open **Transporter**, add the `.ipa`, choose **Verify**, then **Deliver**.
2. Answer the export-compliance question for the build. VoyaVPN uses
   encryption beyond the exempt categories, and `Info.plist` does not declare
   `ITSAppUsesNonExemptEncryption`, so the question is asked per build.
3. Paste the iOS block from
   [app-store-review-notes.md](app-store-review-notes.md#ios-and-ipados) into
   App Review Information → Notes and fill in the review test configuration.
   The reviewer connects from Apple's network in the United States, so the
   node must be reachable from there and stay valid for the whole review. Keep
   the credential out of git.
4. Set App Privacy to "Data Not Collected" and the Privacy Policy and Support
   URLs from the same page.
5. In Pricing and Availability, leave out China mainland
   ([Sales and territories](app-store-review-notes.md#sales-and-territories)).
6. Upload screenshots for iPhone 6.9" and iPad 13". The app ships for both,
   and App Store Connect requires a set for each.
7. Install the build from TestFlight on an iPhone and an iPad and run
   [Device only](#device-only) before submitting.

### Known gaps for review

- **Licensing.** The GPL question above is open.
- **Device acceptance.** The device list has not been run and recorded for a
  store build. Step 8 there, a large rule set left connected, is what shows
  whether the memory limit holds.
- **iPad.** The layout is the iPhone layout in a centred column. Review it on
  an iPad with `pnpm check:mobile:ios:full --matrix-only`, which captures all
  four orientations, before the first submission.
