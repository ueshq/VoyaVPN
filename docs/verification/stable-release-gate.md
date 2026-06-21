# Stable Release Gate Verification

This gate defines the current production-stable release contract. It records what must be true before stable publication, without preserving historical batch logs or one-off command output in the repository.

The release runner, local scripts, and GitHub Actions workflow produce evidence only. They do not upload artifacts to the CDN, mutate stable pointers, purge caches, access signing secrets, notarize apps, approve legal notices, approve diagnostics, or run real Windows/macOS/Linux smoke machines.

## Release Evidence Inputs

- Stable runbook: [../release/runbook.md](../release/runbook.md)
- Stable external evidence checklist: [../release/external-evidence-checklist.md](../release/external-evidence-checklist.md)
- OS smoke matrix: [../release/os-smoke-matrix.md](../release/os-smoke-matrix.md)
- Rollback playbooks: [../release/rollback.md](../release/rollback.md)
- Packaging and CDN manifest model: [../release/packaging.md](../release/packaging.md)
- CI secrets and workflow boundary: [../release/ci-secrets.md](../release/ci-secrets.md)
- Signing, notarization, and updater signing: [../release/signing-notarization.md](../release/signing-notarization.md)
- Third-party notices and core redistribution: [../release/THIRD_PARTY_NOTICES.md](../release/THIRD_PARTY_NOTICES.md)
- Diagnostics privacy contract: [../release/diagnostics-privacy.md](../release/diagnostics-privacy.md)

## Stable Gate Checklist

Every external checkpoint must attach owner, system, verification, rollback or stop condition, and artifact/hash evidence before stable pointer promotion. The fillable release-owner packet is [../release/external-evidence-checklist.md](../release/external-evidence-checklist.md).

| Gate | Owner | System | Verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| Automated regression gate | Release engineer | Local workstation and GitHub Actions `Release` workflow | `pnpm run verify:ci`, `pnpm run build`, package jobs, `SHA256SUMS`, artifact manifests, CDN release-index evidence, updater metadata evidence, and core manifest evidence are recorded for the frozen commit. | Stop release, fix the failing check, and rerun from the frozen commit. |
| CDN staging | CDN owner | VoyaVPN CDN immutable versioned paths | App artifacts, updater payloads, `latest.json`, manual release index, core manifest, geo/SRS manifests, checksums, signatures, notices, and evidence resolve from the approved CDN and match SHA-256 evidence. | Stop pointer promotion, purge accidental public cache, and quarantine bad staged artifacts with hashes. |
| Stable pointer promotion | Release owner and CDN owner | VoyaVPN CDN stable pointers | Manual release-index pointer, app updater `latest.json` pointer, core manifest pointer, geo/SRS pointers, checksum pointers, and notices are promoted only after all gates pass; before/after pointer hashes are recorded. | Roll back pointers to the previous known-good release index, `latest.json`, core manifest, and geo/SRS manifests. |
| Signing and notarization | Security owner, macOS owner, Windows owner, Linux owner | Approved signing systems, Apple Developer ID, Authenticode signer, package repository signing when used | macOS x64/arm64 signatures, notarization, and stapling pass; Windows x64/arm64 signatures pass; Linux package metadata and optional repository signatures pass. | Hold affected platform assets, rebuild from clean artifacts, re-sign, re-notarize, and rerun smoke. |
| Updater smoke | Release engineer and platform owners | Older signed app build, stable updater CDN metadata, signed updater payloads | Each supported x64 and arm64 target detects the stable version, validates signature, downloads from CDN, applies the update, relaunches, and reports the expected version. | Restore previous `latest.json` pointer or remove updater metadata for the affected target. |
| Manual download smoke | Platform owners | Stable CDN release index and platform packages | Windows, macOS, and Linux x64/arm64 packages download from the CDN release index, match checksums, validate signatures/notarization where applicable, install or launch, and uninstall or remove cleanly. | Restore previous release-index pointer or remove affected OS/arch entries; quarantine bad packages. |
| Core smoke | Platform owners and release engineer | In-app update manager, app data `bin/`, Xray/mihomo/sing-box core manifest, geo/SRS manifests | Seed copy, manifest check, download, checksum verification, staged extraction, Unix chmod, safe swap, rollback-on-failure behavior, runtime restart, and geo/SRS separation pass for x64 and arm64 targets. | Restore previous core manifest pointer, restore geo/SRS pointers if affected, quarantine bad core assets, and preserve app-data backups. |
| Diagnostics smoke | Privacy/security owner and platform owners | Stable diagnostics settings, event envelope, endpoint or approved disablement control | Default-on state, visible opt-out, queue clearing, redacted event delivery or approved disabled state, retention assumptions, and forbidden-field exclusion are verified. | Disable diagnostics delivery through the approved control path and stop release if forbidden data can be emitted. |
| Legal redistribution approval | Legal/release owner | Third-party notices, source references, core asset manifest, CDN staging evidence | Exact Xray, mihomo, and sing-box versions, licenses, source URLs, OS/arch entries, SHA-256 values, byte sizes, source availability, and GPL obligations are approved. | Remove unapproved seed or CDN core assets, publish corrected notices/source evidence, and rerun package and CDN metadata generation. |
| Privacy diagnostics approval | Privacy/security owner | Diagnostics endpoint, event schema, settings surface, retention policy, redaction tests, endpoint disable control | Default-on diagnostics, visible opt-out, anonymous install ID storage, queue bounds, retention, endpoint ownership, forbidden-field exclusion, and disabled-state fallback are approved. | Keep diagnostics disabled and stop stable publication if privacy approval is missing or rejects any diagnostics field or endpoint assumption. |
| Rollback readiness | Release owner, CDN owner, security owner, platform owners, legal owner, privacy/security owner | Previous stable pointers, rollback runbook, cache purge workflow, diagnostics disable control, quarantine storage, incident tracker | Previous known-good release-index, `latest.json`, core manifest, geo/SRS manifests, checksum pointers, notices, diagnostics disable control, cache purge path, incident owner, and fixed-build path are ready. | Stop pointer promotion if previous pointers, rollback owner, cache purge route, diagnostics disable control, or incident path are missing. |
| Bad artifact quarantine readiness | Release engineer and CDN owner | Private evidence storage and incident tracker | Quarantine location, access owner, artifact hash format, incident id format, and non-sensitive evidence rules are ready before publication. | Stop release if bad artifacts cannot be withdrawn and preserved without losing audit evidence. |
| Release monitoring | Release owner | Issue tracker, support channel, crash/log intake, update/CDN metrics, diagnostics aggregate health | Monitoring owner, window, severity thresholds, and rollback decision path are recorded before stable exposure. | Execute rollback if thresholds are exceeded or if monitoring ownership is unavailable. |

## Stable Exit Criteria

Production stable may be exposed only when:

- Release workflow and docs define CDN staging, pointer promotion, updater smoke, manual download smoke, core smoke, diagnostics smoke, legal approval, privacy approval, rollback, and monitoring.
- Windows, macOS, and Linux coverage includes x64 and arm64.
- Stable Tauri updater config is generated with `pnpm tauri:stable-updater-config` into `target/release-config/tauri.updater.stable.generated.json`; the generated overlay enables `bundle.createUpdaterArtifacts`, while the committed `src-tauri/tauri.conf.json` keeps `createUpdaterArtifacts` false with an empty credential-free updater config.
- Rollback docs cover app updater pointer rollback, manual index rollback, core manifest rollback, diagnostics disablement, and bad artifact quarantine.
- The stable external evidence checklist has matching owner, system, required evidence, stop or rollback condition, and artifact/hash fields for each stable gate entry.
- Generated evidence contains no `voyavpn.example`, placeholder updater signatures, placeholder public keys, or production GitHub download URLs.
- All external checkpoint evidence is attached or the release is stopped with an owner and follow-up.

## Stable Environment Preflight

Prepared stable readiness is a release-owner gate that runs only after external variables, signing secrets, diagnostics configuration, and real artifact inputs have been provisioned. The release shell must expose the required names without printing or committing their values.

Required prepared names include `VOYAVPN_CDN_BASE_URL`, `VOYAVPN_UPDATES_BASE_URL`, `VOYAVPN_UPDATER_PUBLIC_KEY`, `VOYAVPN_DIAGNOSTICS_ENDPOINT`, `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`, Apple signing/notarization inputs, Windows signing inputs, and real stable artifact paths when defaults are not used.

Prepared release shell sequence:

```sh
export VOYAVPN_RELEASE_CHANNEL=stable
pnpm tauri:stable-updater-config
pnpm check:release:stable
```

Expected unprepared-shell failures are missing external inputs, not repository blockers. A normal local shell may fail immediately on missing `VOYAVPN_CDN_BASE_URL`, missing `VOYAVPN_UPDATES_BASE_URL`, missing `VOYAVPN_UPDATER_PUBLIC_KEY`, missing `VOYAVPN_DIAGNOSTICS_ENDPOINT`, missing `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`, missing platform signing inputs, missing real stable artifacts, fixture artifact paths in stable mode, placeholder updater signatures, or forbidden production URLs.

Expected prepared-environment pass criteria: `pnpm tauri:stable-updater-config` generates `target/release-config/tauri.updater.stable.generated.json`, the overlay enables `bundle.createUpdaterArtifacts`, updater metadata is signed and CDN-derived, app/core metadata use approved CDN production URLs, and `pnpm check:release:stable` exits successfully with zero failures. Stable pointer promotion must not start until the prepared environment passes `pnpm check:release:stable`.

## Repository-Owned Checks

Before handing a frozen commit to external release owners, run:

```sh
pnpm run verify:ci
pnpm run build
pnpm run smoke:frontend
pnpm run check:release:dry-run
```

These checks prove local regression, packaging metadata shape, and dry-run release readiness. They do not prove CDN publication, signing, notarization, external smoke, legal approval, privacy approval, stable pointer promotion, rollback drills, or monitoring readiness.
