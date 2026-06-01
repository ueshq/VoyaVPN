# Regional Presets Verification

Batch: `05-05-regional-presets`

## Coverage

- `voya-net` owns the default Russia and Iran preset source URLs and fetches
  `v2ray.json`, `sing_box.json`, and `simple_dns.json` through the shared
  download client.
- `voya-app` applies preset sources to `ConstItem.GeoSourceUrl`,
  `ConstItem.SrsSourceUrl`, and `ConstItem.RouteRulesTemplateSourceUrl`.
- External raw DNS templates preserve existing DNS item IDs and enabled state.
- When `simple_dns.json` is unavailable or `null`, the preset falls back to
  built-in simple DNS and enables both Xray and sing-box custom DNS items.
- The UI exposes Default, Russia, and Iran preset actions through a confirmation
  dialog under Tools -> Regional presets.

## Deterministic Checks

- `cargo test -p voya-app preset --all-targets`
- `cargo test -p voya-net --all-targets`
- `pnpm typecheck`
- `test -f docs/verification/regional-presets.md`

## External Checks

Real upstream GitHub template availability was not used as a gate for this
batch. The fetch path is covered by local HTTP fixture tests so failures in
external services do not make local verification nondeterministic. A release
smoke check should apply Russia and Iran presets against the live upstream URLs
and confirm the fallback toast is not shown when all template files are present.
