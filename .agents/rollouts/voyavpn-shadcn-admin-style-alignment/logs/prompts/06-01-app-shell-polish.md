# Batch 06-01-app-shell-polish: App Shell Polish

You are implementing the rollout `voyavpn-shadcn-admin-style-alignment` in the repository rooted at `/Users/afu/Dev/VoyaVPN`.

## Phase
- `06-final-polish` — Shell Polish And Final Verification
- Goal: Polish the app shell and run global guard, test, build, and smoke verification.
- Context: This phase finishes shared shell styling and proves the rollout is complete.

## Phase Entry Criteria
- All feature and dense-surface batches have passed typecheck.

## Phase Exit Criteria
- All automated verification gates pass or have exact environment-only evidence.
- Final guard scans find no deleted accent-system remnants.

## Phase Risks
- Final lint/build/smoke can reveal issues hidden by batch-local typecheck.
- Visual fit can regress after multiple batches touch related layout tokens.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Align app-shell header, tabs, content background, menu polish, and modal host sizing to the neutral shadcn-admin style while keeping the tab architecture.

## Depends On
- None

## Deliverables
- app-shell.tsx uses neutral bg-card/bg-background/border-border tokens and aligned tab list styling.
- modal-host.tsx has final settings layout polish after primitive migration.
- No sidebar architecture is introduced.

## Acceptance
- Top-level layout remains header tabs plus content plus status bar.
- Tabs and menu controls have stable compact dimensions.
- No visible text overlaps in compact shell controls.

## Evidence To Capture
- Typecheck output and targeted shell guard output.

## Verification Commands (must pass before declaring success)
- `pnpm typecheck`
- `bash -lc 'if rg -n "data-accent|setAccent|accentMenuOptions|bg-teal|bg-sky|bg-rose" src/components/app-shell; then exit 1; fi'`

## Likely Files
- `src/components/app-shell/app-shell.tsx`
- `src/components/app-shell/modal-host.tsx`
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
- Do not alter shell navigation architecture. This is final styling polish only.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
