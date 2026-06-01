# Updates Verification

Batch: `06-03-downloads-updates`

## Automated Coverage

- `voya-net` has fixture-driven GitHub release parsing, pre-release selection, semantic version parsing, OS/arch asset template resolution, release-asset fallback to v2rayN-style templated URLs, and proxy-to-direct binary download fallback.
- `voya-app` has update target selection, legacy selected-core storage normalization, app/core update checks, geo and SRS target planning, GPL/AGPL download-on-first-run policy checks, and staged directory swap tests.
- Frontend IPC bindings were regenerated after adding update commands.

## Operational Notes

- Unit tests do not require live GitHub access. Release data is supplied as JSON fixtures and binary downloads use a local HTTP fixture.
- GPL or AGPL cores are not bundled by default. `mihomo`, `sing_box`, and `juicity` remain marked as download-on-first-run with installer redistribution disabled.
- Live app package application and signed Tauri updater metadata remain release-phase work because they require package artifacts and signing/update keys.

## Local Evidence

Commands for this batch:

```sh
cargo test -p voya-net update --all-targets
cargo test -p voya-app update --all-targets
pnpm typecheck
pnpm test -- --run
test -f docs/verification/updates.md
```
