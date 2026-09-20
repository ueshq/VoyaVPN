# iOS Build, Signing and Device Acceptance

VoyaVPN on iOS is two bundles: the app and a `NEPacketTunnelProvider` app
extension that runs the sing-box core. Both need Apple developer identities,
matched provisioning profiles, and the Network Extension and App Group
capabilities. This is the procedure for getting from a clean checkout to a
build on a device.

> **Status.** The Xcode targets described under [Xcode target
> setup](#xcode-target-setup) are not yet committed to
> `apps/mobile/ios/VoyaVPN.xcodeproj`: the project still has the single app
> target the React Native template generates. Everything the targets need —
> the sources, the `Info.plist` files, the entitlements, the two build scripts
> — is in the repository, and that section is the remaining wiring. It is
> written out rather than scripted because an `.xcodeproj` edited by hand is
> how project files get corrupted.

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

Done once, in Xcode, committed as a change to `project.pbxproj`.

### The app target

1. Signing & Capabilities → set the team, bundle id `app.voyavpn.mobile`, and
   the code-signing entitlements file to `VoyaVPN/VoyaVPN.entitlements`.
2. Add **Network Extensions** and **App Groups**; the group is
   `group.app.voyavpn.mobile`.
3. Add `VoyaVPN/Native/*.swift` and `VoyaVPN/Native/VoyaNative.m` to
   *Compile Sources*, and `VoyaVPN/Generated/voya_mobile_ffi.swift` with them.
4. *Link Binary With Libraries*: `VoyaMobile.xcframework` **and**
   `Libbox.xcframework`. The app links Libbox for the disconnected latency test
   (ADR 0013); this is the change that grows the app binary, and its size is
   worth watching.
5. Add `VoyaVPN/Generated` to *Header Search Paths* so
   `voya_mobile_ffiFFI.modulemap` is found.

### The PacketTunnel target

1. New target → **Network Extension** → Packet Tunnel Provider, named
   `PacketTunnel`, bundle id `app.voyavpn.mobile.PacketTunnel`.
2. Replace the generated `Info.plist` and entitlements with
   `PacketTunnel/Info.plist` and `PacketTunnel/PacketTunnel.entitlements`, and
   delete the generated provider source — the real one is shared.
3. *Compile Sources*: the four files in `native/apple/PacketTunnel/`. Add them
   as references to the existing files; do **not** copy them into the iOS
   directory, or macOS and iOS will drift.
4. *Link Binary With Libraries*: `Libbox.xcframework` only. The extension does
   not link the Rust host — it runs the core, and the host runs in the app.
5. Set *Deployment Target* to match the app's.
6. Embed the extension in the app target under *Frameworks, Libraries, and
   Embedded Content* as **Embed Without Signing** (Xcode signs it separately).

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
reports `missingComponent` and connecting fails cleanly.

For a device, open `apps/mobile/ios/VoyaVPN.xcworkspace`, select the device and
run. The first connection raises the system's "VoyaVPN would like to add VPN
configurations" prompt; declining it surfaces as a permission failure on the
Home screen rather than as an error.

## Device acceptance

The list that says this milestone is done. A simulator proves none of it.

1. Import a share link, select the node, connect. The Home screen shows an exit
   IP and live up/down rates — which proves the app process reached the Clash
   API *inside the extension process* over loopback.
2. Disconnect. The tunnel goes down and the state settles on disconnected.
3. Kill the app and reopen it. The state is the same one the system has.
4. Run a latency test while disconnected. Nodes report measurements, which
   proves the in-app probe core started (ADR 0013).
5. Run a latency test while connected. Measurements again, this time through
   the running core's Clash API.
6. Connect with a large rule set (a subscription with a full ruleset, not two
   manual nodes) and leave it up. The extension must not be killed for memory.

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
