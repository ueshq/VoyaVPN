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
- `MARKETING_VERSION = 0.1.0` and `CURRENT_PROJECT_VERSION = 1`, matching the
  app — `pnpm run check:architecture` requires one release version across the
  whole workspace.
- Embedded in the app through a PlugIns copy-files phase.

### The VoyaVPNUITests target

A UI-testing bundle, `app.voyavpn.mobile.uitests`, with
`TEST_TARGET_NAME = VoyaVPN` and a generated `Info.plist`. It deliberately sets
*no* `MARKETING_VERSION` or `CURRENT_PROJECT_VERSION`: a test bundle's version
is meaningless and setting one trips the version-alignment gate above. The
script also repoints the shared scheme's `TestAction`, which the React Native
template left pointing at a `VoyaVPNTests.xctest` that has never existed.

### Signing

Signing is still done in Xcode, once: select the team on both targets, and add
**Network Extensions** and **App Groups** (`group.app.voyavpn.mobile`) to each
under *Signing & Capabilities*.

### Memory

Set the extension's memory ceiling early in `startTunnel`. iOS caps a
NetworkExtension appex at roughly 50 MB, and sing-box with a large rule set is
the classic way to exceed it. This is the first thing to measure on a device.


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

For a device, open `apps/mobile/ios/VoyaVPN.xcworkspace`, select the device and
run. The first connection raises the system's "VoyaVPN would like to add VPN
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

TestFlight and the App Store take the `.ipa` Xcode's Archive produces; both
bundles are signed with distribution profiles. Set the version from the repo's
release version — `pnpm run check:architecture` fails when the mobile
`Info.plist` and the root `package.json` disagree.

Libbox is GPL-3.0-or-later, so any build handed to a third party carries the
obligations in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
