# Batch 02-01-font-config-store: Font Config And Preference Store

You are implementing the rollout `voyavpn-shadcn-admin-style-alignment` in the repository rooted at `/Users/afu/Dev/VoyaVPN`.

## Phase
- `02-font-accent` — Font Preferences And Accent Removal
- Goal: Replace brand accent preferences with strict font choices while preserving persisted theme, language, and font-size behavior.
- Context: This phase removes the old brand accent system from state, shell, settings, i18n, and tests.

## Phase Entry Criteria
- Theme no longer provides data-accent token blocks.
- Required primitives exist.

## Phase Exit Criteria
- No app code depends on accent state.
- Font selection works through store, shell, settings, and document classes.

## Phase Risks
- Older configs may still carry ColorPrimaryName and must not crash hydration.
- Changing app-shell and modal-host together can create merge or dependency churn.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Create strict font config and update preferences-store from free-form accent/font family state to strict font choices.

## Depends On
- None

## Deliverables
- src/config/fonts.ts with inter, manrope, and system definitions plus conversion helpers as needed.
- preferences-store.ts removes Accent, accent, setAccent, accentFromConfig, and accentToConfig.
- preferences-store.ts exposes font, setFont, fontToCss, fontFromFamilyString or equivalent strict helpers.
- Old ColorPrimaryName is ignored defensively during config hydration.

## Acceptance
- CurrentFontFamily maps to a strict font and falls back to inter.
- CurrentFontSize still normalizes through existing min/max logic.
- themeMode conversion behavior is unchanged.

## Evidence To Capture
- Typecheck output and store API summary.

## Verification Commands (must pass before declaring success)
- `pnpm typecheck`
- `test -f src/config/fonts.ts`
- `bash -lc 'if rg -n "type Accent|setAccent|accentFromConfig|accentToConfig" src/stores/preferences-store.ts; then exit 1; fi'`
- `rg -n "fontToCss|fontFrom|setFont|DEFAULT_FONT" src/stores/preferences-store.ts src/config/fonts.ts`

## Likely Files
- `src/config/fonts.ts`
- `src/stores/preferences-store.ts`

## Sources Of Truth
- `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/spec.md`
- `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/plan.md`
- `/Users/afu/.claude/plans/refs-shadcn-admin-shimmying-dusk.md`
- `/Users/afu/Dev/refs/shadcn-admin/src/styles/theme.css`
- `/Users/afu/Dev/refs/shadcn-admin/src/styles/index.css`
- `/Users/afu/Dev/refs/shadcn-admin/src/components/ui`
- `components.json`

## Planning Notes
- This rollout is a style-system convergence, not a product workflow redesign.
- The reference repo is read-only. Copy style patterns into VoyaVPN, but never edit shadcn-admin.
- The VoyaVPN shell keeps its top tabs and bottom status bar.
- Remove only the teal, blue, and rose brand accent selector system. Keep standard shadcn --accent tokens.
- The runner does not auto-commit because the current workspace may contain user-owned dirty files.

## Success Metrics
- All required Radix dependencies and tw-animate-css are installed and locked.
- src/styles/globals.css carries the shadcn-admin neutral OKLch token system while preserving RTL and dynamic font size.
- The existing 6 UI primitives are rewritten and the required missing primitives exist.
- All 16 feature surfaces and the app shell use aligned primitives or documented special-surface token styling.
- Guard scans find no data-accent, accent-primary, setAccent, teal, sky, or rose brand accent remnants in src.
- pnpm typecheck, pnpm lint, pnpm build, and pnpm test -- --run pass.

## Global Context
- Stack: Tauri 2, React 19, TypeScript, Tailwind v4, CVA, Radix, lucide-react, Zustand, i18next, Vitest, Playwright.
- Current UI primitives live in src/components/ui and are old shadcn-style forwardRef components.
- Feature pages currently use raw Tailwind controls; migrate visible controls to shared UI primitives where practical.
- Use @/lib/utils cn() and existing path aliases.
- Inter, Manrope, and system are the only supported font choices after this rollout.
- Keep body font-size driven by --app-font-size.

## Hard Rules
- Do not edit /Users/afu/Dev/refs/shadcn-admin.
- Do not change the VoyaVPN tab shell into a sidebar shell.
- Do not remove @custom-variant rtl or the dynamic --app-font-size mechanism.
- Do not delete shadcn --accent tokens; delete only the brand accent selector system.
- Do not migrate Radix toast to sonner.
- Do not replace TanStack Virtual role grids with semantic tables.
- Do not replace CodeMirror, QR canvas, logs list semantics, or hidden file inputs unless behavior is preserved.
- Keep each batch focused and update tests or i18n when the touched surface requires it.
- Run the listed verification commands before declaring a batch complete.

## Batch Context
- Keep localStorage persistence tolerant of old persisted accent fields by simply not reading them.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
