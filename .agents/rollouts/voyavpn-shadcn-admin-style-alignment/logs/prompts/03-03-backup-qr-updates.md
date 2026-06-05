# Batch 03-03-backup-qr-updates: Backup, QR, And Updates Dialogs

You are implementing the rollout `voyavpn-shadcn-admin-style-alignment` in the repository rooted at `/Users/afu/Dev/VoyaVPN`.

## Phase
- `03-dialog-features` — Dialog And Form Feature Migration
- Goal: Migrate form-heavy dialogs to the aligned primitives before larger full-screen surfaces.
- Context: These batches convert high-value feature dialogs where raw fields and alerts are concentrated.

## Phase Entry Criteria
- Required UI primitives are available.
- Accent state has been removed from shared shell/settings code.

## Phase Exit Criteria
- Profile, subscription, import, backup, QR, and update dialogs use aligned primitives where appropriate.

## Phase Risks
- Form helper changes can affect many fields at once.
- Hidden file inputs and canvas behavior must remain intact.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Migrate backup, QR, and update dialogs to aligned primitives while preserving QR canvas and hidden file input behavior.

## Depends On
- None

## Deliverables
- backup-dialog.tsx uses Input, Alert, Label, Card/Badge where appropriate.
- qr-dialog.tsx uses Textarea/Input styling where visible and preserves QR canvas and hidden file input.
- check-update-dialog.tsx uses Checkbox, Alert, Badge, Table, ScrollArea where appropriate.

## Acceptance
- Update table remains readable and checkbox behavior is unchanged.
- Backup success/error messages use Alert.
- QR import/export behavior is unchanged.
- No accent-primary remains in these files.

## Evidence To Capture
- Typecheck output and targeted rg output.

## Verification Commands (must pass before declaring success)
- `pnpm typecheck`
- `bash -lc 'if rg -n "accent-primary|ring-offset-background|<select\b" src/features/backup/backup-dialog.tsx src/features/qr/qr-dialog.tsx src/features/updates/check-update-dialog.tsx; then exit 1; fi'`

## Likely Files
- `src/features/backup/backup-dialog.tsx`
- `src/features/qr/qr-dialog.tsx`
- `src/features/updates/check-update-dialog.tsx`

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
- Do not replace the QR canvas or hidden file input. Only visible controls should migrate.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
