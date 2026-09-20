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

The backend command surface is not registered yet. `@voya/client` exposes
`setVoyaCommands`, and a native module will implement `VoyaCommands` in a later
phase; until then nothing in this app calls a backend command.

## Metro and pnpm

`metro.config.js` carries the reasoning in full. In short: `watchFolders` covers
the source-only workspace packages, `unstable_enablePackageExports` makes their
`exports` maps resolve, `extraNodeModules` points Babel's injected
`@babel/runtime` helpers at this app's copy, and hierarchical resolution must
stay **on** — pnpm stores each package with its dependencies in a sibling
`node_modules`, so disabling it breaks `react-native`'s own imports.

## Commands

```sh
pnpm --filter @voya/mobile start          # Metro
pnpm --filter @voya/mobile ios            # run on the iOS simulator
pnpm --filter @voya/mobile android        # run on an Android device/emulator
pnpm --filter @voya/mobile typecheck      # also covered by check:frontend:typecheck
pnpm run check:mobile:test                # Jest + @testing-library/react-native
pnpm run check:mobile:bundle              # Metro bundle for both platforms
```

`typecheck` and lint run inside the repo-wide gates. `check:mobile:test` and
`check:mobile:bundle` are not part of `verify:local`; CI runs them in the
`mobile` job.

iOS needs CocoaPods (`cd ios && pod install`). Android needs an Android SDK;
`ANDROID_HOME` must be set.
