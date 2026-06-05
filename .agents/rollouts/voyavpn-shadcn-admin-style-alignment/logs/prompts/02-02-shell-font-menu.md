# Batch 02-02-shell-font-menu: Shell Font Menu And Theme Effects

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
Update app-shell to remove accent UI/persistence and apply strict font classes plus CSS variables.

## Depends On
- None

## Deliverables
- app-shell.tsx removes accent menu options, Accent imports, setAccent, root.dataset.accent, and accent persistence.
- app-shell.tsx adds a font submenu or equivalent font radio controls using the strict font options.
- useThemeEffects removes previous font-* classes, adds font-${font}, sets --app-font-family, sets --app-font-size, and preserves colorScheme.
- index.html loads Inter and Manrope.

## Acceptance
- Persisted config no longer writes ColorPrimaryName.
- Theme/light/dark behavior remains unchanged.
- The shell continues using top tabs and bottom status bar.

## Evidence To Capture
- Typecheck output and rg guard output.

## Verification Commands (must pass before declaring success)
- `pnpm typecheck`
- `bash -lc 'if rg -n "data-accent|setAccent|accentMenuOptions|ColorPrimaryName|type Accent|accentToConfig" src/components/app-shell/app-shell.tsx; then exit 1; fi'`
- `rg -n "font-|setFont|fontToCss|Manrope|fonts" src/components/app-shell/app-shell.tsx index.html`

## Likely Files
- `src/components/app-shell/app-shell.tsx`
- `index.html`

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
- This batch owns app-shell font application. Avoid feature-page migrations here.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
