# Ruleset And Geo Verification

Batch: `06-04-ruleset-geo`

Implemented:

- Added `voya-net::ruleset` for geo `.dat` and sing-box `.srs` asset planning, manifest parsing, validation, local fixture downloads, and proxy-to-direct fallback through the shared download client.
- Wired `voya-app` updates to acquire geo and SRS files through the new client and to discover acquired local rulesets from `bin/srss`.
- Added typed source settings commands for `GeoSourceUrl` and `SrsSourceUrl`, with settings UI controls under Settings.
- Added `CoreConfigContext::singbox_ruleset_paths` so sing-box config generation consumes resolved local SRS paths and falls back to remote rule-set URLs when an asset is not present.

Verification run:

- `cargo test -p voya-net ruleset --all-targets`
- `cargo test -p voya-app ruleset --all-targets`
- `cargo test -p voya-core ruleset --all-targets`
- `pnpm bindings`
- `pnpm bindings:check`
- `pnpm typecheck`
- `pnpm test -- --run`

Notes:

- Network-dependent tests use local HTTP fixtures and a deliberately invalid proxy URL to verify proxy-to-direct fallback without live internet access.
- No external core binary checks were run in this batch; the config generator assertion is pure and limited to resolved local SRS references.
