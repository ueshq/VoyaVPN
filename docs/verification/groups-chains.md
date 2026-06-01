# Policy Groups And Proxy Chains Verification

Batch: `05-04-policy-groups-chains-ui`

Implemented scope:

- Added typed group commands for child candidates, validation, preview, and save.
- Added cycle detection before group or chain persistence.
- Added a nested child picker in the profile dialog for mixed server, group, and chain selection.
- Added generator-backed previews for Xray `dialerProxy`/balancer/observatory and sing-box `selector`/`urltest`/`detour`.
- Added golden fixtures for a mixed-child policy group and 2-hop plus 3-hop proxy chains.

Deterministic checks:

- `cargo test -p voya-core proxy_chain --all-targets`
- `cargo test -p voya-core policy_group --all-targets`
- `pnpm typecheck`
- `pnpm test -- --run`
- `test -f docs/verification/groups-chains.md`

External checks:

- No external Xray or sing-box binaries were required for this batch. The preview and golden checks assert generated config structure; core binary acceptance remains covered by the broader config-generation gates where binaries are installed.
