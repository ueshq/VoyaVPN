# Batch 05-03-status-bar: Status Bar Alignment

You are implementing the rollout `voyavpn-shadcn-admin-style-alignment` in the repository rooted at `/Users/afu/Dev/VoyaVPN`.

## Phase
- `05-dense-special` — Dense And Special Surfaces
- Goal: Align server table, Clash dense views, status bar controls, and remaining specialized containers without breaking virtualization or dense workflows.
- Context: This phase handles surfaces where blind primitive replacement could harm behavior.

## Phase Entry Criteria
- Core feature screens have migrated their normal visible controls.

## Phase Exit Criteria
- Dense views use aligned inputs, checkboxes, badges, alerts, and tokens while preserving their structure.

## Phase Risks
- Virtualized lists can regress if DOM structure or sizing changes unexpectedly.
- Status bar controls are compact and can overflow if typography or padding changes.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Align status-bar compact controls and tokens to the neutral component system without changing runtime controls.

## Depends On
- None

## Deliverables
- status-bar.tsx uses aligned Button/Badge/Separator/Tooltip patterns where useful.
- System proxy mode controls keep compact stable dimensions and no text overflow.

## Acceptance
- Status bar runtime state display is unchanged.
- Compact controls fit without layout shift.
- No brand accent styling remains.

## Evidence To Capture
- Typecheck output.

## Verification Commands (must pass before declaring success)
- `pnpm typecheck`
- `bash -lc 'if rg -n "accent-primary|bg-teal|bg-sky|bg-rose|ring-offset-background" src/components/app-shell/status-bar.tsx; then exit 1; fi'`

## Likely Files
- `src/components/app-shell/status-bar.tsx`

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
- Stable dimensions matter in the bottom bar. Avoid text growth that shifts layout.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
