# Batch 04-01-routing-screen: Routing Screen Migration

You are implementing the rollout `voyavpn-shadcn-admin-style-alignment` in the repository rooted at `/Users/afu/Dev/VoyaVPN`.

## Phase
- `04-screen-features` — Full-Screen Feature Migration
- Goal: Migrate routing, DNS, groups, options, and logs screens to shared primitives and aligned token styling.
- Context: These batches handle larger screens with repeated helpers, lists, badges, alerts, and special editors.

## Phase Entry Criteria
- Dialog-heavy migrations have passed typecheck.

## Phase Exit Criteria
- Routing, DNS, group builder, options, and logs no longer use old visible raw field styling.

## Phase Risks
- Routing and DNS screens have many repeated controls and third-party editors.
- Group builder has dense selectable lists where checkbox labels must remain accessible.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Migrate routing-screen controls, helper fields, badges, alerts, scroll containers, and tables to aligned primitives.

## Depends On
- None

## Deliverables
- Visible routing inputs use Input or Textarea.
- Native visible selects use Select primitives.
- Checkboxes use Checkbox plus Label.
- Rule badges, empty states, and alerts use aligned primitives or tokens.

## Acceptance
- Routing CRUD, search, enable toggles, and rule editing behavior remain unchanged.
- No old ring-offset focus styling or accent-primary remains.

## Evidence To Capture
- Typecheck output and targeted rg output.

## Verification Commands (must pass before declaring success)
- `pnpm typecheck`
- `bash -lc 'if rg -n "accent-primary|ring-offset-background|<select\b" src/features/routing/routing-screen.tsx; then exit 1; fi'`

## Likely Files
- `src/features/routing/routing-screen.tsx`

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
- pnpm typecheck, pnpm lint, pnpm build, and pnpm test --run pass.

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
- Prefer changing shared helper components inside routing-screen so repeated controls migrate together.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
