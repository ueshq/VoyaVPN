# sing-box Seed Pinning

The sing-box core is not built by VoyaVPN. It is downloaded from an upstream
GitHub release, extracted into
`apps/desktop/src-tauri/resources/core-seeds/sing_box/`, executed on developer
machines by the `postinstall` probe, and bundled into every package that
`pnpm tauri:build` produces. Because a GitHub release tag is mutable, that
archive is pinned by SHA-256 in the repository.

## Where the pin lives

`scripts/core/sing-box-installer.mjs` holds two constants that are bumped
together in a single commit:

- `DEFAULT_SING_BOX_VERSION` — the upstream release tag.
- `SING_BOX_ARCHIVE_SHA256` — the SHA-256 of each upstream **release archive**
  (the exact bytes the installer downloads, before extraction), keyed by version
  and by the `<os>-<cpu>` pair used in the asset name.

The installer verifies the pin in three places:

1. Before downloading — an unpinned `{version, platform, arch}` is refused
   without reaching the network.
2. After downloading and before `extractArchive`/any execution — a mismatching
   archive throws and nothing is staged.
3. On the already-staged path (`ensureSingBoxSeedForBuild`, `installSingBoxCore`)
   — `sing-box.seed.json` must record the expected version, asset name and
   pinned archive digest, and the staged executable must still hash to the
   `executableSha256` the manifest recorded. Anything else is re-staged rather
   than bundled.

`SING_BOX_VERSION` is validated against `/^v\d+\.\d+\.\d+(-[\w.]+)?$/` before it
is interpolated into a URL or a PowerShell command line.

## Bumping the pinned version

1. Pick the new upstream tag and read the release asset digests:

   ```sh
   curl -sS "https://api.github.com/repos/SagerNet/sing-box/releases/tags/<tag>" \
     | node -e 'const r=JSON.parse(require("fs").readFileSync(0,"utf8"));
       for (const a of r.assets) if (/^sing-box-[\d.]+-(darwin|linux|windows)-(amd64|arm64)\.(tar\.gz|zip)$/.test(a.name))
         console.log(a.name, a.digest);'
   ```

   `assets[].digest` is `sha256:<hex>`; record only the hex half.

2. Cross-check at least one entry by downloading the archive yourself and
   running `shasum -a 256 <archive>`. Never copy a digest from a mirror,
   an issue comment, or a previous seed manifest of a different version.

3. Update `DEFAULT_SING_BOX_VERSION` and every `SING_BOX_ARCHIVE_SHA256` entry
   for the new tag in the same commit. All six supported combinations
   (`darwin`, `linux`, `windows` × `amd64`, `arm64`) must be present; an
   installer test fails when one is missing.

4. Re-stage locally and confirm the seed manifest reports the new version:

   ```sh
   pnpm core:sing-box:install --force-fetch
   cat apps/desktop/src-tauri/resources/core-seeds/sing_box/sing-box.seed.json
   ```

5. Record the new version, archive name, SHA-256 and byte size in the
   [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) approval evidence before a
   stable release ships the new binary.

## Escape hatch

`VOYAVPN_ALLOW_UNPINNED_SING_BOX=1` permits staging a `{version, platform, arch}`
with no pinned archive digest for a local experiment. The resulting seed must
still have a current manifest containing the expected version and asset name,
a valid archive SHA-256, and an executable SHA-256 matching the staged binary.
Missing, unreadable or incomplete historical manifests are never accepted,
including with this variable; they require re-staging.

It never accepts a digest that disagrees: a mismatching archive or a staged
binary that no longer matches its manifest is refused with or without the
variable. It must not be set in CI or for a release build — the resulting
`sing-box.seed.json` records `"pinned": false`, and `pnpm release -- readiness`
reports the seed as unverified (a stable blocker).

A missing or incomplete manifest requires re-staging and verification regardless
of the variable, just as after a version bump.

Do not invent a digest to silence the check. A `{version, platform, arch}` whose
genuine hash cannot be obtained stays absent from the table.

## Related opt-in

`VOYAVPN_ALLOW_SING_BOX_SEED_BACKFILL=1` re-enables copying the per-user
app-data binary back into the repository seed directory. It is off by default:
the app owns that directory and may replace the binary, so a back-fill would let
an unverified binary flow into the next package build.
