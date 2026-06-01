# UI Polish Verification

Batch: `07-04-theme-a11y-perf`

## Scope

- Theme, accent, font family, and font size preferences now hydrate from generated `AppConfig.UIItem`, apply immediately through document theme/CSS variables, and persist back through `saveAppConfig`.
- Theme tokens were adjusted to neutral surfaces with teal, blue, and rose accent variants so the UI is not dominated by one hue family.
- Dialog, menu, tab, status bar, and table controls were tightened for compact widths with bounded dialog sizing, scrollable tab rails, hidden quick language controls below `md`, and truncation-safe compact controls.
- The profiles table now exposes column/row counts, `aria-sort`, row and column indices, status text for empty/loading states, and keyboard row actions.

## Performance Evidence

- `src/features/profiles/server-table.test.tsx` keeps the 5,000-row virtualization assertion.
- The same test file adds a 500-row live-stat harness that applies 60 one-second-equivalent stat batches and asserts the run stays below a 1,000 ms budget while preserving updated row data.

## Accessibility Evidence

- Dialog content is constrained to the viewport and remains scrollable for settings-heavy flows.
- Settings forms use native labels and immediate controls for theme, accent, font family, font size, source settings, autostart, and hotkeys.
- Status controls use icon buttons with labels/titles, proxy mode buttons expose pressed state, and the TUN toggle exposes pressed state.
- Profile table headers expose sort state and rows support keyboard activation/selection.

## External Checks

- No Playwright or Tauri-driver visual smoke was run in this batch because `07-05-playwright-tauri-smoke` is the dedicated smoke automation batch. This batch is covered by the required frontend static and unit checks.
