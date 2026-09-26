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

6. Pin the source build of the same tag (see
   [From-source seed](#from-source-seed-mac-app-store-lane)):

   ```sh
   git -C target/native/sing-box fetch --tags --force
   git -C target/native/sing-box rev-list -n1 <tag>
   git -C target/native/sing-box show <tag>:release/DEFAULT_BUILD_TAGS_OTHERS
   ```

   Record the commit in `SING_BOX_SOURCE_COMMITS`. Compare the tag list with
   `SING_BOX_SOURCE_BUILD_TAGS`; if upstream changed it, review the change
   before copying it, and never add `with_naive_outbound`. Then run
   `pnpm core:sing-box:build` on a Mac and check that it passes its own
   public-API scan.

## From-source seed (Mac App Store lane)

The Mac App Store package cannot ship the upstream macOS binary. Upstream
builds its release with `with_naive_outbound`, which links Chromium's Cronet
(`github.com/sagernet/cronet-go`). Cronet's `info_plist_data.o` imports the
non-public `__kCFBundleNumericVersionKey`, and Cronet links the private
`/usr/lib/libpmenergy.dylib` and `/usr/lib/libpmsample.dylib`. App Review
rejected the 2026-09 upload for the first of these (Guideline 2.5.1).

So the store lane builds the seed from source instead:

- **Switch.** `VOYAVPN_SING_BOX_SEED_ORIGIN` is `upstream` (default) or
  `source`. `VOYAVPN_MAC_APP_STORE=1` implies `source`, and asking that build
  for `upstream` is refused. `pnpm install` stages the upstream seed and never
  needs Go.
- **Pin.** `SING_BOX_SOURCE_COMMITS` maps each tag to the commit it must
  resolve to, since a tag can move. `SING_BOX_SOURCE_BUILD_TAGS` is upstream's
  own `make build` list (`release/DEFAULT_BUILD_TAGS_OTHERS`), written out so a
  bump cannot change it silently. `SING_BOX_SOURCE_EXCLUDED_TAGS` names what
  may never appear.
- **Build.** `scripts/core/sing-box-source-seed.mjs` checks out the pin in
  `target/native/sing-box` (the checkout the Libbox builds share) and runs
  `go build -trimpath` with upstream's `release/LDFLAGS`. It then checks the
  tags and revision the binary reports and, on macOS, runs the public-API scan
  from `scripts/native/macos/macho-imports.mjs`. `pnpm core:sing-box:build`
  runs it by hand; the Tauri build wrapper runs it when the staged seed does
  not verify.
- **Manifest.** `sing-box.seed.json` records `origin: "source"`, `version`,
  `commit`, `target`, `tags`, `excludedTags`, `goVersion`, `ldflags` and
  `executableSha256`. The same build with the same Go version is
  byte-for-byte reproducible; a different Go version gives different bytes, so
  the pin is the commit and tags, not the binary digest.
- **Verification.** A seed staged for one origin never verifies for the other
  (`origin-mismatch`), so the next build re-stages instead of bundling it. That
  keeps a source seed out of a Developer ID package and the upstream binary out
  of the store package. A source seed must also match the pinned commit, the
  exact tag list, the target, and its recorded executable digest. `pnpm release
  -- readiness` fails on a wrong commit or tag list.
- **Runtime.** The app reads `tags` from the bundled manifest. A node whose
  outbound type needs a tag the seed lacks is reported as `protocolUnsupported`
  and left out of the probe core's config, because sing-box refuses a whole
  config that names an unknown outbound type. The upstream manifest has no
  `tags`, so nothing changes for the other lanes.

`VOYAVPN_ALLOW_UNPINNED_SING_BOX=1` applies to the source build too: it permits
a tag with no pinned commit for a local experiment, and never accepts a commit
that disagrees with the pin.

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

## Default rule sets

Packages also bundle the three rule sets the default routing profile names
(`geosite-cn`, `geoip-cn`, `geosite-private`), so a fresh install routes China
and LAN destinations directly before any download. The pins live in
`RULE_SET_PINS` in `scripts/core/rule-sets-installer.mjs`: one upstream commit
and one SHA-256 per file. Upstream (`2dust/sing-box-rules`) publishes them on
the `rule-set-geosite` and `rule-set-geoip` branches and has no release tags,
so a branch name alone would change under the build every day.

`pnpm install` stages them into
`apps/desktop/src-tauri/resources/core-seeds/rule_sets/` (skipped on CI unless
`VOYAVPN_FETCH_RULE_SETS_ON_INSTALL=1`, and whenever
`VOYAVPN_SKIP_RULE_SETS_POSTINSTALL=1`), and every `tauri build` re-stages any
file that is missing or does not match its pin. `pnpm core:rule-sets:install`
forces a fresh download. At startup the desktop app copies any bundled file
that app data `bin/srss/` lacks; it never replaces one, because a rule-library
update may have put a newer file there.

To bump them:

1. `git ls-remote https://github.com/2dust/sing-box-rules.git` and note the
   `refs/heads/rule-set-geosite` and `refs/heads/rule-set-geoip` commits.
2. Download each file from
   `https://raw.githubusercontent.com/2dust/sing-box-rules/<commit>/<tag>.srs`,
   check that it starts with `SRS`, and record its SHA-256.
3. Update `RULE_SET_PINS` in one commit, then run
   `pnpm core:rule-sets:install` and `pnpm release -- readiness --dry-run`.

