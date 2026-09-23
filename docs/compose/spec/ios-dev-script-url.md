---
feature: ios-dev-script-url
status: in-progress
updated: 2026-09-22
branch: feat/mobile-heroui-native
commits: 
---

# iOS Dev Script URL

## Report

## [S1] Problem

A Debug build of the iOS app redboxes at launch with
`No script URL provided … unsanitizedScriptURLString = (null)`.

Reproduced on the iPhone 17 simulator (iOS 26.5) while Metro is running on
`localhost:8081` and `GET /status` answers `packager-status:running` from the
Mac. The simulator’s `NSURLSession` still fails that same request.

Root cause, confirmed from simulator unified logs and `RCTBundleURLProvider.mm`:

1. macOS system HTTP proxy is on (`127.0.0.1:10808`) with **no** localhost /
   `127.0.0.1` exception. The iOS Simulator inherits CFNetwork’s proxy settings.
2. `RCTBundleURLProvider.packagerServerHostPort` calls `isPackagerRunning`,
   which does `GET http://localhost:8081/status` on `NSURLSession.sharedSession`.
   Via the proxy that call is reset or 502, so the check returns false.
3. With no `RCT_jsLocation` override and `guessPackagerHost` failing the same
   way, `packagerServerHostPort` is nil.
4. `jsBundleURL(forBundleRoot:)` then falls back to an embedded `main.jsbundle`.
   A Debug product does not embed one, so `bundleURL()` is nil and
   `RCTJavaScriptLoader` raises the redbox above.

Release is unaffected when `main.jsbundle` is present (the
`Bundle React Native code and images` phase embeds it). The failure is specific
to Debug + packager + a system HTTP proxy that does not except localhost.

## [S2] Design

Two cooperating changes, both Debug-only:

1. **Always mint an explicit Metro URL.** `ReactNativeDelegate.bundleURL()` in
   `#if DEBUG` calls
   `RCTBundleURLProvider.jsBundleURL(forBundleRoot:packagerHost:enableDev:enableMinification:inlineSourceMap:)`
   with `packagerHost = "localhost:8081"` instead of
   `sharedSettings().jsBundleURL(forBundleRoot:)`. That API builds the URL and
   does **not** probe `/status`, so a proxy-broken status check can no longer
   collapse the script URL to nil. Release keeps
   `Bundle.main.url(forResource: "main", withExtension: "jsbundle")`.

2. **Force direct connections for Debug URLSession.** Before
   `RCTReactNativeFactory.startReactNative`, swizzle
   `URLSessionConfiguration.defaultSessionConfiguration` and
   `ephemeralSessionConfiguration` so every configuration created afterwards
   (including `NSURLSession.sharedSession` used by the status probe and
   `RCTMultipartDataTask` used to download `index.bundle`) has
   `connectionProxyDictionary = [:]`, which CFNetwork documents as “disable
   proxy lookups”. Release does not swizzle.

Error behaviour: if Metro is actually down, the redbox becomes a load failure
against a real `http://localhost:8081/index.bundle…` URL rather than a nil
script URL. Release with a missing `main.jsbundle` still returns nil (unchanged).

Testing boundary: simulator Debug launch with the Mac HTTP proxy enabled is the
regression. No Android change. No change to Release bundle embedding.

## [S3] Out of Scope

- Android packager URL resolution.
- Release bundling, `main.jsbundle` embedding, or `react-native-xcode.sh`.
- Changing system proxy settings from the app or install script.
- PacketTunnel / Libbox linking (already handled separately in
  `scripts/native/mobile/ios-project.rb`).
- Making `guessPackagerHost` / `RCT_jsLocation` smarter for remote devices.

## Tasks

- [x] T1: DEBUG `bundleURL()` mints an explicit Metro URL — acceptance: the
  method never consults `isPackagerRunning`; source shows the class
  `jsBundleURL(forBundleRoot:packagerHost:…)` call (covers: S2)
- [x] T2: DEBUG URLSession swizzle clears `connectionProxyDictionary` before RN
  starts — acceptance: swizzle runs first in `didFinishLaunchingWithOptions`;
  Release path is `#if DEBUG`-gated (covers: S2)
- [x] T3: Document the proxy/localhost constraint — acceptance:
  `docs/release/mobile-ios-signing.md` Running section states that a Mac HTTP
  proxy must except `127.0.0.1`/`localhost` **or** rely on the Debug direct
  path, and that Release still needs `main.jsbundle` (covers: S2)
- [ ] T4: Simulator regression — acceptance: Debug build installed on the
  booted iPhone 17 simulator launches to Home (empty node list) with the Mac
  HTTP proxy still enabled; `pnpm run check:mobile:swift` passes (covers: S2;
  depends: T1, T2)
