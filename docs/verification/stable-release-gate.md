# Stable Release Gate Verification

Batch: `05-01-final-local-sweep`

## Scope

This gate closes the production stable release documentation path for Phase `05-final-regression`. It verifies that the stable runbook, OS smoke matrix, rollback playbooks, external evidence checklist, and final local automated sweep describe the complete stable publication path without performing external publication.

The generated runner does not execute external publication. It does not upload artifacts to the CDN, mutate stable pointers, purge caches, access signing secrets, notarize apps, approve legal notices, approve diagnostics, or run real Windows/macOS/Linux smoke machines.

## Evidence Path

Stable gate doc path: `docs/verification/stable-release-gate.md`

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
- Update subsystem evidence: [updates.md](updates.md)

## Stable Gate Checklist

Every external checkpoint must attach owner, system, verification, rollback or stop condition, and artifact/hash evidence before stable pointer promotion. The fillable release-owner packet is [../release/external-evidence-checklist.md](../release/external-evidence-checklist.md).

| Gate | Owner | System | Verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| Automated regression gate | Release engineer | Local workstation and GitHub Actions `Release` workflow | `pnpm run verify:ci` covers Rust formatting, strict Clippy, Rust workspace tests, frontend typecheck, Vitest, ESLint, and generated binding drift; `pnpm run build`, local debug packaging, six-target package jobs, `SHA256SUMS`, artifact manifests, CDN release-index evidence, updater metadata evidence, and core manifest evidence are recorded. | Stop release. Fix the failing automated check and rerun from the frozen commit. |
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
- Stable Tauri updater config is generated with `pnpm tauri:stable-updater-config` into `target/release-config/tauri.updater.stable.generated.json`; the generated overlay enables `bundle.createUpdaterArtifacts`, while the committed `src-tauri/tauri.conf.json` keeps `createUpdaterArtifacts` false and remains credential-free.
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

## Automated Risk Closure

Batch `05-04-flaky-tests-and-build-budget` closes the named local release blockers before final regression.

Profiles table Vitest stability:

- Owner: frontend release engineer.
- Outcome: fixed through per-test QueryClient isolation, no query cache retention across profile table tests, awaited policy-group child-picker interactions, and an explicit wait for selected children before preview/save.
- Focused timeout rationale: the policy-group test exercises the full profile table, profile dialog, child picker query, React Hook Form state, generator preview, and group save path. It now has a focused `10_000` ms timeout to absorb local and CI variance without masking missing async state, because every transition still has a concrete DOM or IPC assertion.
- Evidence: `pnpm exec vitest --run src/features/profiles/server-table.test.tsx --reporter verbose` passed 1 file and 8 tests in 8.33s. The policy-group test completed in 2050ms.

Vite chunk budget:

- Owner: frontend release engineer.
- Outcome: resolved by `build.rolldownOptions.output.codeSplitting` groups for React, editor/UI libraries, data/form libraries, profile features, operational features, and remaining vendor code.
- Stable budget: keep the default Vite large-chunk threshold of 500 kB minified JS. Any future chunk over that threshold must either be split or recorded here with owner, reason, and rollback impact before stable sign-off.
- Evidence: `pnpm run build` passed with no Vite large-chunk warning. Largest emitted JS chunks were `vendor-editor` 419.33 kB, `feature-ops` 391.66 kB, `vendor-data` 246.55 kB, `vendor-react` 189.64 kB, and `feature-profiles` 111.25 kB.

## Verification Commands

Required local docs check from batch `04-01-external-evidence-checklist`:

```sh
rg -n "CDN staging|pointer promotion|notarization|Authenticator|Authenticode|updater smoke|manual download smoke|core smoke|diagnostics smoke|legal|privacy|rollback|monitoring" docs/release docs/verification
```

This command proves that the release docs expose the named external gates. It does not prove that human CDN, signing, notarization, smoke, legal, privacy, rollback, or monitoring gates have passed.

Required local docs check from batch `04-02-diagnostics-and-legal-signoff-evidence`:

```sh
rg -n "retention|opt-out|forbidden|source availability|GPL|byte|sha256|legal|privacy" docs/release docs/verification
```

This command proves that the release docs expose diagnostics privacy evidence fields and legal core redistribution evidence fields. It does not prove that privacy/security or legal owners have approved stable publication.

Required local docs checks from batch `05-03-stable-runbooks-and-smoke`:

```sh
test -f docs/verification/stable-release-gate.md
rg -n "CDN|x64|arm64|updater smoke|core smoke|diagnostics|rollback" docs/release docs/verification/stable-release-gate.md
```

These commands prove the documentation gate exists and references the required stable release concepts. They do not prove that external CDN publication, signing, smoke testing, diagnostics approval, or rollback operations have passed.

Required local docs check from batch `05-02-stable-env-preflight-doc`:

```sh
rg -n "VOYAVPN_CDN_BASE_URL|VOYAVPN_UPDATES_BASE_URL|VOYAVPN_UPDATER_PUBLIC_KEY|TAURI_SIGNING_PRIVATE_KEY|check:release:stable" docs/release docs/verification/stable-release-gate.md
```

This command proves that release docs expose the stable prepared-environment command sequence, required stable env names, expected unprepared-shell failures, and the requirement that `pnpm check:release:stable` pass before stable pointer promotion. It does not prove that external production variables, secrets, artifacts, or stable pointer promotion have been provisioned or completed.

Evidence captured on 2026-06-06:

- PASS. The scan found the prepared stable env names, `pnpm tauri:stable-updater-config`, `pnpm check:release:stable`, unprepared-shell failure language, and prepared-environment pass criteria in [../release/runbook.md](../release/runbook.md), [../release/ci-secrets.md](../release/ci-secrets.md), and this gate doc.

Required local automated checks for batch `05-04-flaky-tests-and-build-budget`:

```sh
pnpm test --run
pnpm run build
```

Evidence captured on 2026-06-06:

- `pnpm test --run`: PASS. Vitest completed 7 test files and 41 tests in 14.75s.
- `pnpm run build`: PASS. TypeScript build and Vite production build completed; no Vite large-chunk warning was emitted. The Tailwind plugin timing notice is informational and is not a release-blocking chunk budget finding.

Required local automated checks for batch `05-01-final-local-sweep`:

```sh
pnpm run verify:ci
pnpm run build
pnpm run smoke:frontend
pnpm run check:release:dry-run
```

Final local sweep evidence captured on 2026-06-06 at 12:58-13:00 CST (Asia/Shanghai):

- `pnpm run verify:ci`: PASS. The verifier completed Rust formatting, strict Rust Clippy, Rust workspace tests, frontend typecheck, Vitest, ESLint, and generated binding drift. The Clippy step invoked `cargo clippy --workspace --all-targets -- -D warnings` and finished successfully. Rust tests passed across 236 unit tests. Vitest passed 8 files and 48 tests in 10.89s. ESLint reported 0 errors and the existing 3 warnings in `src/components/ui/badge.tsx`, `src/components/ui/button.tsx`, and `src/features/clash/clash-connections-screen.tsx`. `pnpm bindings:check` compiled `export-bindings` and reported generated IPC bindings are up to date.
- `pnpm run build`: PASS. TypeScript build and Vite production build completed, transforming 2069 modules and building in 4.06s. No Vite large-chunk warning was emitted. Largest emitted JS chunks were `vendor-editor` 419.33 kB, `feature-ops` 391.66 kB, `vendor-data` 246.55 kB, `vendor-react` 189.64 kB, `vendor-radix` 144.23 kB, and `feature-profiles` 111.25 kB. The Tailwind plugin timing notice remains informational.
- `pnpm run smoke:frontend`: PASS. Playwright started the Vite web server on `127.0.0.1:1420` and passed 3 Chromium smoke tests in 16.7s: app shell and dialogs, profile import/activation/fake runtime connect, and routing/DNS settings. The `NO_COLOR`/`FORCE_COLOR` and React SWC performance notices were informational.
- `pnpm run check:release:dry-run`: PASS before and after this gate doc update. Dry-run readiness finished with 11 passes, 0 warnings, and 0 failures. The production blocker scan passed with no forbidden production URL fields, updater placeholders, or GitHub production download templates found. The generated stable release index contained 6 artifacts, updater metadata contained 6 platforms, and the core asset manifest contained 18 assets.
- `pnpm tauri:build --debug`: PASS as supplemental local packaging evidence. Local macOS debug packaging completed without signing credentials. The run produced `/Users/afu/Dev/VoyaVPN/target/debug/bundle/macos/VoyaVPN.app` and `/Users/afu/Dev/VoyaVPN/target/debug/bundle/dmg/VoyaVPN_0.1.0_x64.dmg`. These are unsigned local debug artifacts and are not stable release, notarized, or CDN-publishable artifacts.

Stable overlay and blocker-warning status after the final local sweep:

- The committed default Tauri config does not include production updater config and keeps `bundle.createUpdaterArtifacts` disabled. This is an intentional repository posture, not a remaining repository blocker. Stable non-dry-run release must generate `target/release-config/tauri.updater.stable.generated.json` with `pnpm tauri:stable-updater-config` and use the approved updater public key plus updater signing input.
- Dry-run readiness passes with zero stable-blocker warnings. Repository-controlled blockers for forbidden production URL fields, updater placeholders, and GitHub production download templates are resolved. CDN staging, signing, notarization, external smoke machines, legal approval, privacy approval, stable pointer promotion, and monitoring remain external stable gates.

External gates not completed by this local batch:

- CDN staging, stable pointer promotion, cache purge, and CDN rollback verification were not run.
- Production signing, Windows Authenticode signing, macOS notarization/stapling, Linux package repository signing, and six-target release workflow jobs were not run.
- Windows, macOS, and Linux x64/arm64 updater smoke, manual download smoke, core smoke, diagnostics smoke, and rollback drills were not run on external smoke machines.
- Legal redistribution approval and privacy/security diagnostics approval were not granted by this automated evidence run.
