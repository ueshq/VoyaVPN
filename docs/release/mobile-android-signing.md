# Android Build, Signing and Device Acceptance

VoyaVPN on Android is one process. Unlike iOS there is no extension: the
sing-box core runs inside the app through a foreground `VpnService`, which is
what puts the Clash API on plain loopback with nothing in between (ADR 0013).

> **Status.** The Kotlin host under
> `apps/mobile/android/app/src/main/java/app/voyavpn/mobile/host/` has never
> been compiled: it is written against the `libbox.aar` that
> `vp run native:mobile:libbox:android` stages, and there is no host-side build
> that typechecks it. Expect to reconcile the `PlatformInterface` members with
> the pinned archive on the first Gradle build; the macOS implementation in
> `native/apple/PacketTunnel/PacketTunnelPlatform.swift` is the reference for
> what each one is expected to do.

## Identifiers

| | Identifier |
| --- | --- |
| Application id | `app.voyavpn.mobile` |
| VPN service | `app.voyavpn.mobile.host.VoyaVpnService` |

There is no App Group equivalent and no second bundle to keep in step. The
service is declared in `AndroidManifest.xml` with
`android:permission="android.permission.BIND_VPN_SERVICE"` and the
`android.net.VpnService` intent filter, which is what makes the system offer it
as a VPN at all.

## Prerequisites

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
```

Plus a JDK 17+, the Android SDK, and the NDK version `build.gradle` pins. Go
and gomobile come from sing-box's own `make lib_install`, which
`vp run native:mobile:libbox:android` runs.

## Build the native artifacts

Both are gitignored build outputs, and a Gradle build needs both:

```sh
vp run native:mobile:libbox:android   # libbox.aar  → apps/mobile/android/app/libs/
vp run native:mobile:rust:android     # .so files   → apps/mobile/android/app/src/main/jniLibs/
                                    # + Kotlin bindings → app/src/main/java/uniffi/
```

`native:mobile:rust:*` builds the `release` cargo profile; set
`VOYAVPN_RUST_PROFILE=debug` for a faster unoptimised library while iterating.

`abiFilters` in `app/build.gradle` is `arm64-v8a` and `x86_64` — a device and
the emulator. A build for any other ABI would link a library that was never
staged, so add the Rust target and rebuild before widening it.

## Running

```sh
vp run dev:android                  # add --rebuild-rust after a Rust change
```

It builds the native artifacts above when they are missing (the Rust backend
with the faster `mobile-smoke` profile), then runs `react-native run-android`
with any other arguments.

The emulator *can* run a `VpnService`, which makes Android the cheaper platform
to develop the tunnel on. The first connection raises the system's VPN
authorization dialog; `VpnService.prepare` is what detects that it has not been
granted, and a declined prompt reaches the Home screen as a permission failure
rather than as an error.

## Device acceptance

1. Import a share link, select the node, connect. The Home screen shows an exit
   IP and live up/down rates.
2. Disconnect. The tunnel goes down and the state settles on disconnected.
3. Kill the app and reopen it. The state is the same one the system has.
4. Run a latency test while disconnected, then while connected. Both report
   measurements — the first through the in-app probe core, the second through
   the running core's Clash API.
5. Turn on the system's **Always-on VPN** for VoyaVPN and reboot. The app comes
   back consistent with what the system did.
6. Disconnect from the notification shade rather than from the app. The app's
   state follows; it must not keep claiming a connection.
7. Let another VPN app take over. `onRevoke` fires, and the app reports the
   tunnel as stopped rather than as running.

Steps 5 to 7 are Android's own: they are the ways a tunnel can end without the
app being asked, and each one is a state the UI has to land on correctly.

## Production APK

`vp run build:android` builds the APK distributed outside Google Play:

```sh
vp run build:android                  # Rust (release profile), Libbox, Gradle, checks
vp run build:android --reuse-libbox   # keep the staged libbox.aar
vp run build:android --skip-native    # link the staged Rust and Libbox as they are
```

It signs with the release key, read from four environment variables that
`app/build.gradle` passes to Gradle's `release` signing config:

| Variable | |
| --- | --- |
| `VOYAVPN_ANDROID_KEYSTORE` | Path to the keystore, kept out of the repository |
| `VOYAVPN_ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `VOYAVPN_ANDROID_KEY_ALIAS` | Alias of the signing key |
| `VOYAVPN_ANDROID_KEY_PASSWORD` | Key password |

Without them the `release` build type falls back to the checked-in debug
keystore, which is only for local runs, so the lane refuses to start. Every
user's installed copy only accepts updates signed by the same key: lose it and
the app cannot be updated in place. Generate it once:

```sh
keytool -genkeypair -v -keystore voyavpn-release.jks -alias voyavpn -keyalg RSA -keysize 4096 -validity 10000
```

`versionName` is the repo's release version — `vp run check:architecture`
fails when `build.gradle` and the root `package.json` disagree. `versionCode`
is `VOYAVPN_ANDROID_VERSION_CODE`, or the commit count when unset, so a build
from a later commit always installs over an earlier one.

Before it copies the APK out, the lane checks with `aapt2` and `apksigner`
that the package id, `versionName` and `versionCode` are the expected ones, the
build is not debuggable, the signer is the release key's certificate, and both
ABIs carry the Rust host and Libbox. The result is
`target/release/bundle/android/VoyaVPN_<version>_<versionCode>.apk`.

Libbox is GPL-3.0-or-later, so any APK or AAB handed to a third party carries
the obligations in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
