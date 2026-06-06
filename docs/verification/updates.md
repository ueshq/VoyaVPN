# Updates Verification

Batches:

- `02-01-production-url-model`
- `02-02-cdn-backed-update-client`

## Automated Coverage

- `voya-net` has CDN release-index and core-asset manifest parsing, OS/arch app/core asset resolution, checksum field propagation, unsupported-target coverage, production URL rejection for GitHub/placeholder update URLs, fixture-only GitHub release parsing, and proxy-to-direct binary download fallback.
- `voya-app` has CDN base URL/config resolution for production app/core update checks, update target selection, legacy selected-core storage normalization, app/core checksum results, geo and SRS target planning, GPL/AGPL download-on-first-run policy checks, and staged directory swap tests.
- Frontend IPC bindings must be regenerated after Rust update/config type changes.
- `ReleasePackage` production metadata carries package identity, target, prerelease policy, and acquisition policy only. Legacy GitHub release parsing requires explicit `UpstreamReleaseEvidence`, keeping GitHub release/download templates out of stable app/core delivery metadata.

## Operational Notes

- Production app/core checks load `release-index.json` and `core-assets.json` from `ConstItem.CdnBaseUrl`, or from explicit `ConstItem.CdnReleaseIndexUrl` / `ConstItem.CdnCoreManifestUrl` overrides. They do not use hard-coded GitHub release download URLs.
- GitHub release parsing remains available for fixtures and migration compatibility only through explicit upstream/source evidence. Production CDN manifest URLs and production app/core asset URLs reject GitHub, example-host, and placeholder values.
- Unit tests do not require live GitHub access. Release data is supplied as JSON fixtures or local HTTP fixtures.
- GPL or AGPL cores are not bundled by default. `mihomo`, `sing_box`, and `juicity` remain marked as download-on-first-run with installer redistribution disabled.
- Live app package application and signed Tauri updater metadata remain release-phase work because they require package artifacts and signing/update keys.

## Local Evidence

Commands for this batch:

```sh
cargo test -p voya-net --all-targets update
cargo test -p voya-app --all-targets updates
test -f docs/verification/updates.md
```

Result for batch `02-01-production-url-model`: both focused Rust commands passed locally. `voya-net` ran 9 update tests; `voya-app` ran 28 update-filtered tests.

## Diagnostics Evidence Stub

Batch: `04-01-diagnostics-contract`

- Contract path: `docs/release/diagnostics-privacy.md`
- Later diagnostics batches should append evidence for default-on settings, persisted opt-out, allowlisted serialization, forbidden field exclusion, bounded queue behavior, endpoint failure handling, and UI toggle coverage.
- Required privacy assertion: diagnostics must not serialize node URLs, subscription URLs, credentials, IPs, full logs, generated configs, or traffic destinations.

## Diagnostics Core Evidence

Batch: `04-02-diagnostics-core`

- `voya-core` adds `DiagnosticsItem` to `AppConfig` with default-on `Enabled`, persisted `AnonymousInstallId`, and optional `EndpointUrl`.
- `voya-app` adds typed diagnostics event constructors, an allowlisted JSON envelope, anonymous install id generation, opt-out queue clearing, bounded in-memory queue limits, endpoint validation, best-effort JSON POST delivery, and nonblocking failure retention.
- Privacy coverage includes forbidden fixture values for node URLs, subscription URLs, credentials, IP addresses, logs, generated configs, paths, and traffic destinations.

Verification:

```sh
cargo test -p voya-app --all-targets diagnostics
cargo test -p voya-core --all-targets diagnostics
```

Result: both commands passed locally for this batch.
