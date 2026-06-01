# M6 Polish Gate Verification

Batch: `07-06-polish-phase-gate`

Run date: 2026-06-01

## Scope

This gate stabilizes Phase 07: backup, WebDAV, autostart, global hotkeys, QR, i18n, theming, accessibility, performance, and smoke automation before packaging starts.

Covered subsystem evidence:

- `docs/verification/backup-webdav.md`
- `docs/verification/autostart-hotkeys-qr.md`
- `docs/verification/i18n.md`
- `docs/verification/ui-polish.md`
- `docs/verification/cross-platform-smoke.md`
- `docs/verification/manual-os-smoke.md`

## Gate Findings

- Local backup and restore are covered by deterministic zip round-trip tests with unsafe path rejection and clean-state restore.
- WebDAV behavior is fixture-tested for PROPFIND parsing, upload, download, and delete without live credentials.
- Autostart planning covers Windows Run registry, Linux desktop autostart, and macOS LaunchAgent artifacts through adapter-backed tests.
- Global hotkeys cover the five v2rayN actions and are wired to Tauri registration through generated IPC commands.
- QR generation is backend-owned and returns SVG over typed IPC; QR scanning remains a frontend/platform path.
- I18n covers 8 registered locales, ResX import drift, missing-key parity, and RTL document direction for `fa`.
- Theme, accent, font, compact layout, dialog sizing, table accessibility, and large-table live-update performance are covered by frontend tests and docs.
- Frontend Playwright smoke is available through `pnpm smoke:frontend` with a browser-side Tauri IPC mock and no real OS mutation.

## Fixes Applied

No code fixes were required during this gate. The required workspace checks passed before the report was added.

## Automated Evidence

Final local results:

```sh
cargo test --workspace --all-targets
```

PASS. Workspace tests completed successfully across Rust crates and `src-tauri`.

```sh
pnpm typecheck
```

PASS. TypeScript project references compiled with `tsc -b --pretty false`.

```sh
pnpm test -- --run
```

PASS. Vitest completed 4 test files and 18 tests.

```sh
pnpm lint
```

PASS. ESLint completed without findings.

```sh
pnpm bindings:check
```

PASS. `scripts/bindings.mjs --check` regenerated bindings in a temporary path and reported that `src/ipc/bindings.ts` is up to date.

```sh
test -f docs/verification/m6-polish-gate.md
```

PASS. This report is present.

## Manual OS Smoke Matrix

The release-candidate manual OS smoke matrix is ready in `docs/verification/manual-os-smoke.md` and is summarized by `docs/verification/cross-platform-smoke.md`.

Minimum RC coverage:

| OS | Required RC smoke |
| --- | --- |
| Windows 11 x64 | install or dev launch, add/import profile, real connect, system proxy set/clear, PAC, TUN/UAC cleanup, autostart artifact, global hotkeys, uninstall or cleanup |
| Windows 10 x64 | package launch, connect, system proxy restore, updater smoke where supported |
| macOS Apple Silicon | signed or dev launch, real connect, proxy restore, sudo TUN cleanup, LaunchAgent autostart, global hotkeys |
| macOS Intel, if supported | package launch, connect, proxy restore |
| Linux x64 | AppImage or package launch, real connect, proxy shell restore, sudo TUN cleanup, desktop autostart file, global hotkeys |

Each run must record date, operator, build identifier, OS version and architecture, core binary versions and paths, redacted server or subscription source, before/after OS proxy and route state, visible logs or screenshots, and skipped checks with owner and follow-up.

## Deferred External Checks

- Live WebDAV push and pull were not executed because they require credentials and a reachable remote. Follow-up: run the WebDAV section of the manual smoke matrix with test credentials before release sign-off.
- Real autostart mutation, global desktop hotkeys, system proxy changes, and TUN route changes were not executed in this local gate because they require real Windows, macOS, and Linux desktop sessions with permission to mutate OS state. Follow-up: capture one manual evidence record per supported OS before packaging sign-off.
- Real connect, logs, stats, and traffic flow were not executed here because they require private server credentials and locally installed or first-run-downloaded third-party core binaries. Follow-up: execute the Core Discovery And Connection flow from `docs/verification/manual-os-smoke.md`.
- Packaged install, signing, notarization, updater trust, and uninstall checks are deferred to Phase 08 packaging because this batch does not produce release installers.
