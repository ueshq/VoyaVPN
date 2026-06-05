# Batch 01-01-style-dependencies: Install Style Dependencies

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
Add the Radix packages required by the new primitives and add tw-animate-css as a dev dependency.

## Depends On
- None

## Deliverables
- package.json includes required Radix packages and tw-animate-css.
- pnpm-lock.yaml is updated consistently.

## Acceptance
- @radix-ui/react-label, checkbox, select, switch, tooltip, scroll-area, dropdown-menu, and popover are dependencies.
- tw-animate-css is a devDependency.
- No app behavior changes are introduced in this batch.

## Evidence To Capture
- Package dependency diff and package-manager output in runner log.

## Verification Commands (must pass before declaring success)
- `node -e "const p=require('./package.json'); const deps=['@radix-ui/react-label','@radix-ui/react-checkbox','@radix-ui/react-select','@radix-ui/react-switch','@radix-ui/react-tooltip','@radix-ui/react-scroll-area','@radix-ui/react-dropdown-menu','@radix-ui/react-popover']; for (const d of deps) if (!p.dependencies || !p.dependencies[d]) throw new Error('missing '+d); if (!p.devDependencies || !p.devDependencies['tw-animate-css']) throw new Error('missing tw-animate-css');"`
- `pnpm typecheck`

## Likely Files
- `package.json`
- `pnpm-lock.yaml`

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
- Use pnpm to add dependencies so the lockfile stays valid.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.
