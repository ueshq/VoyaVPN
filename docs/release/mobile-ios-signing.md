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

For a device, open `apps/mobile/ios/VoyaVPN.xcworkspace`, select the device and
run. The first connection raises the system's "VoyaVPN would like to add VPN
configurations" prompt; declining it surfaces as a permission failure on the
Home screen rather than as an error.

## Acceptance

Split in two, because the two halves cost very different amounts of attention.
The simulator half runs itself; the device half is the one a person has to sit
through, and it is the one that says the milestone is done.

### Simulator: automated

```sh
cd apps/mobile/ios
xcrun simctl uninstall booted app.voyavpn.mobile        # the database is the state under test
xcodebuild test -workspace VoyaVPN.xcworkspace -scheme VoyaVPN \
  -configuration Release -destination 'platform=iOS Simulator,name=iPhone 17' \
  -derivedDataPath build/DerivedData CODE_SIGNING_ALLOWED=NO
```

Release, not Debug: a Debug build needs Metro running beside it and is not
sealed. The uninstall matters — the thirteen cases in
`apps/mobile/ios/VoyaVPNUITests/VoyaVPNUITests.swift` run in order and share one
install, because a node imported by one case is what the next selects, shares
and finally deletes. About four minutes end to end.

What they prove, which the Jest suite cannot, is that the screens are driving
the **real** backend — `crates/voya-mobile-ffi` through the `VoyaNative`
module — and not the in-memory mock `transport.ts` falls back to when the
module is missing. Case 00 is the one that decides it: the mock seeds three
nodes, the real backend opens an empty database.

| | Case | What a pass means |
| --- | --- | --- |
| 00 | Launches on the real backend | The node list is empty, not the mock's three nodes |
| 01 | Imports from the clipboard | A share link parsed in Rust and written to SQLite |
| 02 | Selects the node | The backend recorded the active profile |
| 03 | Switches traffic mode | `proxySetTrafficMode` round trip, and the global banner |
| 04 | Toggles a seeded rule | The default rule set seeded, named, and the toggle persisted |
| 05 | Runs a latency test | Every node comes back with an outcome from the probe core |
| 06 | Activity asks for a connection | The disconnected empty state |
| 07 | Saves a DNS resolver | `saveDnsSettings`, verified by leaving and returning |
| 08 | Switches theme | The chosen theme is marked (see the gap below) |
| 09 | Refreshes the rule library | The command was dispatched and answered |
| 10 | Shows a share QR | `generate_qr_code` in Rust, drawn by `react-native-svg` |
| 11 | Survives a relaunch | The node really went to the app's own SQLite file |
| 12 | Deletes the node | The list is empty again |

Two things worth knowing before changing these:

- React Navigation spells a tab's label out in full — `Home, tab, 1 of 5` — so
  tab queries match a prefix, not the whole label.
- A node row is one `Button`, not the three texts it draws. React Native's
  `Pressable` is an accessibility element itself and folds its children's text
  into a single label, which is also why the actions sheet's wrappers are
  explicitly `accessible={false}`.

### Simulator: by hand

- **Dark mode.** `xcrun simctl ui booted appearance dark` and look. Uniwind
  reports the scheme correctly but keeps resolving the `:root` block, so the
  colours do not currently change — a known gap, recorded in
  `apps/mobile/global.css`. The theme buttons in Settings do track the choice,
  which is what case 08 asserts.
- **VoiceOver** reading the node row and the long-press actions sheet.

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
9. Refresh a real subscription over the network, and use a policy group — the
   phone has no screen for creating one, so build it on the desktop first.

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
