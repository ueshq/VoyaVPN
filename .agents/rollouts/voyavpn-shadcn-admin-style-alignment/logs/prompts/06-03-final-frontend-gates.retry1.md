# Batch 06-03-final-frontend-gates: Final Frontend Gates

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
- Kind: `verification`
- Execution: `codex`

## Batch Goal
Run lint, build, tests, and frontend smoke; fix in-scope failures and capture final evidence.

## Depends On
- None

## Deliverables
- All frontend gates pass, or environment-only smoke failure is documented with exact command and error.
- .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md records commands and outcomes.

## Acceptance
- pnpm lint passes.
- pnpm build passes.
- pnpm test --run passes.
- pnpm smoke:frontend passes unless blocked by a documented environment-only issue.

## Evidence To Capture
- final-verification.md and runner logs for lint/build/test/smoke.

## Verification Commands (must pass before declaring success)
- `pnpm lint`
- `pnpm build`
- `pnpm test --run`
- `pnpm smoke:frontend`
- `test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md`

## Likely Files
- `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md`
- `src/**`

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
- If smoke fails for missing browsers or display only, write precise evidence to final-verification.md and still fix all code-caused failures.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.

## Retry Context
The previous attempt for batch `06-03-final-frontend-gates` failed verification.
Retry number: 1

Fix the implementation so that every verification command passes.
Before you finish, rerun the verification commands yourself and confirm they are green.

### Failed Check 1
Command: `pnpm smoke:frontend`
Exit code: `1`
Output:
```text
Running 3 tests using 1 worker

  ✘  1 [chromium] › e2e/smoke.spec.ts:13:1 › loads the app shell and key dialogs (30.1s)
  ✓  2 [chromium] › e2e/smoke.spec.ts:49:1 › adds and imports profiles, activates one, and connects through the fake runtime (4.5s)
  ✓  3 [chromium] › e2e/smoke.spec.ts:80:1 › edits routing and DNS settings without network or OS side effects (3.2s)


  1) [chromium] › e2e/smoke.spec.ts:13:1 › loads the app shell and key dialogs ─────────────────────

    [31mTest timeout of 30000ms exceeded.[39m

    Error: locator.click: Test timeout of 30000ms exceeded.
    Call log:
    [2m  - waiting for getByRole('menuitem', { name: 'QR' })[22m


      33 |
      34 |   await page.getByRole("menuitem", { exact: true, name: "Tools" }).click();
    > 35 |   await page.getByRole("menuitem", { name: "QR" }).click();
         |                                                    ^
      36 |   const qrDialog = page.getByRole("dialog", { name: "QR" });
      37 |   await expect(qrDialog).toBeVisible();
      38 |   await page.getByLabel("Content").fill(importFixture);
        at /Users/afu/Dev/VoyaVPN/e2e/smoke.spec.ts:35:52

    Error Context: test-results/smoke-loads-the-app-shell-and-key-dialogs-chromium/error-context.md

    attachment #2: trace (application/zip) ─────────────────────────────────────────────────────────
    test-results/smoke-loads-the-app-shell-and-key-dialogs-chromium/trace.zip
    Usage:

        pnpm exec playwright show-trace test-results/smoke-loads-the-app-shell-and-key-dialogs-chromium/trace.zip

    ────────────────────────────────────────────────────────────────────────────────────────────────

  1 failed
    [chromium] › e2e/smoke.spec.ts:13:1 › loads the app shell and key dialogs ──────────────────────
  2 passed (41.9s)
[ELIFECYCLE] Command failed with exit code 1.
$ playwright test
[2m[WebServer] [22m[2m$ vite --host 127.0.0.1 --port 1420[22m
[2m[WebServer] [22m[vite:react-swc] We recommend switching to `@vitejs/plugin-react` for improved performance as no swc plugins are used. More information at https://vite.dev/rolldown
```
