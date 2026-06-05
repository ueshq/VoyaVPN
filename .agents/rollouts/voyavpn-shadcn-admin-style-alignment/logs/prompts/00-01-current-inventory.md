# Batch 00-01-current-inventory: Current UI Inventory

You are implementing the rollout `voyavpn-shadcn-admin-style-alignment` in the repository rooted at `/Users/afu/Dev/VoyaVPN`.

## Phase
- `00-baseline` — Baseline Evidence
- Goal: Capture the current UI/style inventory and the reference contract before code migration starts.
- Context: This phase gives later batches concrete evidence for scope, hotspots, and final guard scans.

## Phase Entry Criteria
- VoyaVPN repo is available at /Users/afu/Dev/VoyaVPN.
- shadcn-admin reference repo is available read-only at /Users/afu/Dev/refs/shadcn-admin.

## Phase Exit Criteria
- Baseline counts and reference component list are captured under the rollout directory.
- The scope of required primitives and feature surfaces is explicit.

## Phase Risks
- Skipping inventory can leave raw controls or accent remnants outside the final sweep.

## Batch Shape
- Kind: `analysis`
- Execution: `codex`

## Batch Goal
Create an inventory document with current primitive files, feature surfaces, raw control counts, accent remnants, and special-surface notes.

## Depends On
- None

## Deliverables
- .agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md with current counts and hotspot file list.
- Inventory includes ui primitive files, 16 feature surfaces, raw input/select/checkbox hits, accent hits, forwardRef hits, and ad hoc rounded/card-like hits.

## Acceptance
- The inventory cites actual rg or find commands and their results.
- The inventory separates normal feature migrations from special surfaces such as CodeMirror, server-table, Clash dense views, QR, and logs.

## Evidence To Capture
- Baseline inventory file path and command output summary.

## Verification Commands (must pass before declaring success)
- `test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md`
- `rg -n "feature surfaces|data-accent|forwardRef|special surfaces" .agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md`

## Likely Files
- `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md`

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
- Use rg or rg --files for inventory. Do not change app code in this batch.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
