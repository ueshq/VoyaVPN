# Batch 01-03-existing-primitives: Rewrite Existing Primitives

You are implementing the rollout `voyavpn-shadcn-admin-style-alignment` in the repository rooted at `/Users/afu/Dev/VoyaVPN`.

## Phase
- `01-foundation` — Theme And Primitive Foundation
- Goal: Install style dependencies, align global theme tokens, and create the shared UI primitives needed by feature migration.
- Context: This phase unlocks the rest of the rollout by making shared components and CSS tokens available.

## Phase Entry Criteria
- Baseline inventory and reference contract exist.

## Phase Exit Criteria
- Required dependencies are installed.
- Global theme matches the neutral shadcn-admin system.
- Existing primitives are rewritten and missing primitives are available.

## Phase Risks
- Typecheck can fail if components are added before their Radix packages are installed.
- Theme drift can break later component styling in subtle ways.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Rewrite button, dialog, tabs, separator, menubar, and toast to aligned new-york style while preserving public APIs.

## Depends On
- None

## Deliverables
- button.tsx, dialog.tsx, tabs.tsx, separator.tsx align to shadcn-admin equivalents.
- menubar.tsx keeps @radix-ui/react-menubar and aligns focus, SVG, transition, and data-slot styling.
- toast.tsx keeps Radix toast API and aligns visual styling.

## Acceptance
- Migrated primitives use function component style where appropriate and add data-slot.
- Focus rings use focus-visible:ring-[3px] and ring-ring/50 patterns where applicable.
- Dialog animations rely on tw-animate-css classes.
- TabsList uses bg-muted p-[3px] rounded-lg h-9 w-fit and active triggers use bg-background shadow-sm.

## Evidence To Capture
- Typecheck output and component diff summary.

## Verification Commands (must pass before declaring success)
- `pnpm typecheck`
- `bash -lc 'if rg -n "forwardRef" src/components/ui/button.tsx src/components/ui/dialog.tsx src/components/ui/tabs.tsx src/components/ui/separator.tsx; then exit 1; fi'`
- `rg -n "data-slot|ring-\[3px\]|tw-animate|bg-muted p-\[3px\]" src/components/ui/button.tsx src/components/ui/dialog.tsx src/components/ui/tabs.tsx src/components/ui/separator.tsx src/components/ui/menubar.tsx src/components/ui/toast.tsx`

## Likely Files
- `src/components/ui/button.tsx`
- `src/components/ui/dialog.tsx`
- `src/components/ui/tabs.tsx`
- `src/components/ui/separator.tsx`
- `src/components/ui/menubar.tsx`
- `src/components/ui/toast.tsx`

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
- Use shadcn-admin files as references for exact style patterns. Menubar has no reference equivalent, so restyle it in place.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
