# Mobile Libbox Pinning

The phones do not ship the packaged sing-box executable. A phone may not spawn
a child process at all, so the core runs as a library inside the app: Libbox,
sing-box's own mobile binding, built from source at the same tag the desktop's
seed is pinned to.

That makes three artifacts from one checkout:

| Platform | Artifact | Built by | Staged into |
| --- | --- | --- | --- |
| macOS | `Libbox.framework` (universal) | `pnpm native:macos:libbox` | `apps/desktop/src-tauri/native/macos/Frameworks/` |
| iOS | `Libbox.xcframework` (device + simulator) | `pnpm native:mobile:libbox:ios` | `apps/mobile/ios/Frameworks/` |
| Android | `libbox.aar` | `pnpm native:mobile:libbox:android` | `apps/mobile/android/app/libs/` |

None of them is committed. They are build outputs of a pinned source tag, so a
committed copy could only ever be a stale one — see `.gitignore`.

## Where the pin lives

`scripts/native/sing-box-source.mjs` is the single checkout helper all three
build scripts use. It resolves the ref in this order:

1. `VOYAVPN_SING_BOX_REF`
2. `SING_BOX_VERSION`
3. `DEFAULT_SING_BOX_VERSION` from `scripts/core/sing-box-installer.mjs`

The third is the pin: **the same constant the desktop's downloaded seed is
pinned to.** Bumping it in
[sing-box-seed-pinning.md](sing-box-seed-pinning.md) moves the phones' core
with it, which is the point — a phone measuring a node against one sing-box
version while the desktop runs another would produce results neither of them
could reproduce.

The checkout itself lives at `target/native/sing-box` and is refused when it
has local changes: the artifact would then be whatever is in the working tree
rather than the pinned tag, and nothing downstream could tell. Override with
`VOYAVPN_SING_BOX_SOURCE_DIR` to build from an existing checkout, and
`VOYAVPN_SING_BOX_ALLOW_DIRTY=1` to accept a dirty one on purpose.

Unlike the desktop seed there is **no archive digest to pin**: nothing is
downloaded pre-built. What is verified instead is the shape of the output —
the iOS build refuses an xcframework that cannot serve both a device and the
simulator, and the macOS build refuses a framework that is not universal.

## Building

Both mobile builds need the Go toolchain sing-box's `make lib_install`
installs (gomobile), and the iOS one needs Xcode command line tools and runs
only on macOS.

Android's pinned Libbox build requires JDK 17, SDK platform 23 (gomobile's
binding target), and NDK r29 (`29.0.14206865`); the pinned Cronet archive uses
relocations the older r27 linker cannot read. Set `ANDROID_HOME` and
`ANDROID_NDK_HOME` to these installations. The React Native Gradle build uses
Android Studio's bundled JDK and the NDK version in `android/build.gradle`.
Only arm64-v8a and x86_64 are packaged, matching the Rust host slices.

After staging both native libraries, build and run the Android UI smoke on a
connected test device or emulator (this writes only the app's test data):

```sh
cd apps/mobile/android
./gradlew :app:assembleRelease :app:assembleReleaseAndroidTest
./gradlew :app:connectedReleaseAndroidTest
```

The release test target bundles JavaScript and the ML Kit barcode model;
Metro and a first-run model download are not required. Physical camera,
photo-picker, share-sheet and VPN data-plane checks remain part of the
release device matrix.

```sh
pnpm native:mobile:libbox:ios       # Libbox.xcframework  → apps/mobile/ios/Frameworks/
pnpm native:mobile:libbox:android   # libbox.aar          → apps/mobile/android/app/libs/
```

Run the Rust host build beside it — the two are independent, and an Xcode or
Gradle build needs both:

```sh
pnpm native:mobile:rust:ios         # VoyaMobile.xcframework + Swift bindings
pnpm native:mobile:rust:android     # jniLibs + Kotlin bindings
```

Overrides, when the artifact belongs somewhere else:

- `VOYAVPN_LIBBOX_IOS_XCFRAMEWORK`
- `VOYAVPN_LIBBOX_ANDROID_AAR`

## What each build keeps

`make lib_apple` produces every Apple platform sing-box can build for —
macOS, iOS, tvOS, and their simulators. Shipping all of them would add
megabytes of slices nothing links, so each script keeps only what its app
loads:

- **iOS** keeps one device slice (`ios-arm64`) and one simulator slice
  (`ios-arm64_x86_64-simulator`, or `ios-arm64-simulator` on an
  Apple-silicon-only toolchain). Both are needed, and they cannot be merged
  into a single framework: both are arm64, and no framework holds two slices
  of one architecture. That is what an xcframework is for. The staged
  `Info.plist` is pruned to list exactly the slices that were kept, because
  Xcode reads it to choose one and a plist promising a slice that is not there
  fails the build.
- **Android** keeps the archive whole: gomobile packages every ABI into one
  `.aar`, and Gradle's `abiFilters` decides which survive into the APK.

## After a bump

1. Bump `DEFAULT_SING_BOX_VERSION` per
   [sing-box-seed-pinning.md](sing-box-seed-pinning.md).
2. Rebuild all three Libbox artifacts.
3. Run `pnpm check:native:macos:bridge` — it typechecks the shared PacketTunnel
   sources against the staged framework, which is where an API change in Libbox
   surfaces first.
4. Run the device checks in [os-smoke-matrix.md](os-smoke-matrix.md). A core
   version change is exactly the kind of thing only a real tunnel proves.

## Licensing

sing-box is GPL-3.0-or-later, and Libbox is built from it. Any package handed
to a third party — TestFlight, an App Store submission, an APK — carries the
same redistribution obligations the desktop bundle does. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
