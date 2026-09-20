# @voya/mobile

The VoyaVPN bare React Native app (RN 0.87, no Expo).

## Boundaries

- **No `@voya/ui`.** That package is Radix and DOM specific. Shared logic comes
  from `@voya/client` (query keys, backend-code→text maps, the runtime event
  store, the persisted stores), `@voya/contracts` (generated backend DTOs) and
  `@voya/i18n/core` + `@voya/i18n/native`.
- **No `@tauri-apps/*`.** ADR-0002 confines Tauri APIs to
  `apps/desktop/src/ipc`; lint enforces it here too.
- The `@/*` alias is desktop-private. This app uses `~/*` for `./src/*`,
  declared in both `tsconfig.json` and `babel.config.js`.

## Platform wiring

`src/native/platform-boot.ts` registers the platform behind the shared
packages — MMKV storage, the system colour scheme, and the i18n host — and is
imported first from `index.js`, before any store module is evaluated. The
desktop equivalent is `apps/desktop/src/platform-boot.ts`.

`src/ipc/transport.ts` is the only place that chooses a backend. It registers
the `VoyaNative` TurboModule when the build has one, and the shared in-memory
`createMockBackend` when it does not — which is what makes a development build
a rehearsal for a device build rather than a separate app. Nothing downstream
of `setVoyaCommands` can tell which one answered.

`src/ipc/native-transport.ts` builds the whole `VoyaCommands` surface from the
generated `VOYA_COMMAND_WIRE` table: positional arguments become the same named
object Tauri receives, the answer is parsed, and a rejection's serialized
`AppError` becomes an `IpcCommandError` with the same typed `kind` the desktop
gets. The three event channels arrive as one native event carrying its channel.

## Metro and pnpm

`metro.config.js` carries the reasoning in full. In short: `watchFolders` covers
the source-only workspace packages, `unstable_enablePackageExports` makes their
`exports` maps resolve, `extraNodeModules` points Babel's injected
`@babel/runtime` helpers at this app's copy, and hierarchical resolution must
stay **on** — pnpm stores each package with its dependencies in a sibling
`node_modules`, so disabling it breaks `react-native`'s own imports.

## Native hosts

The Rust backend runs in-process through `crates/voya-mobile-ffi` (ADR 0012),
and the tunnel runs sing-box inside a platform provider (ADR 0013):

- **iOS** — `ios/VoyaVPN/Native/` holds the RN module, the
  `NETunnelProviderManager` driver and the in-app probe core. The provider
  itself is the Swift in `native/apple/PacketTunnel/`, shared with the macOS
  app.
- **Android** — `android/app/src/main/java/app/voyavpn/mobile/host/` holds the
  RN module, the foreground `VpnService` and its Libbox platform interface. The
  core runs in the app's own process, so the Clash API is plain loopback.

Neither host is built from this directory. The artifacts they need are
gitignored build outputs:

```sh
pnpm native:mobile:libbox:ios       # Libbox.xcframework
pnpm native:mobile:libbox:android   # libbox.aar
pnpm native:mobile:rust:ios         # VoyaMobile.xcframework + Swift bindings
pnpm native:mobile:rust:android     # jniLibs + Kotlin bindings
```

The runbooks are [`docs/release/mobile-ios-signing.md`](../../docs/release/mobile-ios-signing.md),
[`mobile-android-signing.md`](../../docs/release/mobile-android-signing.md) and
[`mobile-libbox-pinning.md`](../../docs/release/mobile-libbox-pinning.md). The
iOS one also carries the Xcode target setup, which is not yet committed to the
project file.

## Commands

```sh
pnpm --filter @voya/mobile start          # Metro
pnpm --filter @voya/mobile ios            # run on the iOS simulator
pnpm --filter @voya/mobile android        # run on an Android device/emulator
pnpm --filter @voya/mobile typecheck      # also covered by check:frontend:typecheck
pnpm run check:mobile:test                # Jest + @testing-library/react-native
pnpm run check:mobile:bundle              # Metro bundle for both platforms
pnpm run check:mobile:swift               # parse the iOS app Swift (macOS only)
```

`typecheck` and lint run inside the repo-wide gates. The three `check:mobile:*`
gates are not part of `verify:local`; CI runs the first two in the `mobile` job
and the third on the `mobile-rust` iOS leg, which already has a macOS runner.

iOS needs CocoaPods (`cd ios && pod install`). Android needs an Android SDK;
`ANDROID_HOME` must be set.
