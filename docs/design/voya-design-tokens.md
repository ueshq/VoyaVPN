# Voya design tokens

All three surfaces use `packages/design-tokens/tokens.json`. The package has no React,
DOM or native dependency. `vp run generate design-tokens` emits browser class-theme
CSS, system-theme CSS for the static website, nested Uniwind theme CSS and typed native/SVG values.
Do not edit generated outputs. `vp run check design-tokens` is read-only and verifies
both themes, deterministic output and semantic WCAG contrast (4.5:1 text, 3:1 controls).
It runs in `verify local` and CI’s baseline-fast job.

## Roles and adapters

| Role | Light | Dark |
| --- | --- | --- |
| Canvas | #faf9f6 | #141816 |
| Card | #ffffff | #1e2420 |
| Text / ordinary primary | #081827 | #f1f5f1 |
| Secondary text | #626760 | #adb6ae |
| Selected wash | #f1eee7 | #30382f |
| Necessary control border | #858a81 | #7e8c80 |
| Connected fill | #178044 | #178044 |

Ordinary actions invert foreground/background. Selection adds a check, border or
weight as well as the warm-gray fill. Green is connection/success only. Red and amber
remain danger and warning. Links are neutral and underlined; focus uses a neutral
2 px ring. Decorative dividers and halos do not communicate control boundaries.
The connected fill is darker than the visual reference to support white button text.

Desktop aliases live in `packages/ui/src/styles/globals.css`; all shadcn primitives,
page shells, tables and overlays consume them. Mobile imports the generated adapter
after HeroUI. Native navigation and SVG use `usePalette`; no Home-only navigation
branch exists. Uniwind requires `@layer theme` → `:root` → `@variant light/dark`.
The website uses system CSS without a script and preserves static output and URLs.

## Geometry

Controls use radius 12, cards 22, overlays 24 and pills 9999. Minimum control heights:
mobile 48, desktop 36 (compact 32), website main actions 44. Mobile page inset is 14,
card padding 16, section gap 24; desktop inset is 16/24. Website max width is 1152,
inset 16/24 and section gap 64/96. Font roles are system sans, monospace for technical
data, and serif tabular numbers for the Home statistics. Mobile typography must use
standard Tailwind size names so HeroUI’s tailwind-merge retains the size.
At accessibility text sizes, segmented choices stack inside the scrolling content
column, including the capped iPad column, so every option remains reachable.
Native tab pages opt into automatic scroll insets so the last content clears
the iOS floating tab bar without a hardcoded bar height.

Home owns the connection illustration and restrained warm decoration. Both clients
show the circular control, current node and latency/duration cards. Desktop secondary
connection details hold the map, mode pointer, exit IP and reconnect; traffic remains
reachable in the persistent sidebar and Activity. Data pages retain compact toolbars,
virtualization, grouping and dense rows. Settings routes/identifiers and form save/apply
semantics remain stable; mobile subscriptions are reached from Nodes.

Motion is 150/200/250 ms. Respect reduced motion and native accessibility settings.
Platform-native navigation material is allowed to differ. QA evidence belongs in
ignored `.agents/docs/`, and website screenshots use sanitized renderer fixtures.
