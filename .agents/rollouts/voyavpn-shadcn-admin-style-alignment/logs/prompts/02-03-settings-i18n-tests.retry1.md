# Batch 02-03-settings-i18n-tests: Settings Dialog, I18n, And Tests

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
Replace settings accent controls with font controls, update locale keys, and update tests from accent assertions to font assertions.

## Depends On
- None

## Deliverables
- modal-host.tsx removes accent options and uses font controls plus existing font-size controls.
- Locale files remove or stop using menu/modal accent labels and add font labels.
- src/App.test.tsx no longer asserts data-accent and instead asserts font class behavior.

## Acceptance
- Settings dialog exposes theme, font, font size, source settings, integration settings, and language without accent color swatches.
- No teal/sky/rose swatch classes remain in src.
- Tests pass after assertion updates.

## Evidence To Capture
- Vitest output and guard scan.

## Verification Commands (must pass before declaring success)
- `pnpm test -- --run`
- `pnpm typecheck`
- `bash -lc 'if rg -n "data-accent|accent-primary|setAccent|menu\.accent|modal\.accent|bg-teal-600|bg-sky-600|bg-rose-600" src; then exit 1; fi'`

## Likely Files
- `src/components/app-shell/modal-host.tsx`
- `src/i18n/locales/*.json`
- `src/App.test.tsx`

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
- Use Button variants or Select primitives for font choices. Keep dynamic font size controls.

## Working Agreement
- Finish only this batch and the minimum supporting changes required for its verification commands.
- Prefer the shadcn-admin reference component/style over local invention.
- When migrating a feature, preserve behavior and accessibility before polishing classes.
- If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.

## Retry Context
The previous attempt for batch `02-03-settings-i18n-tests` failed verification.
Retry number: 1

Fix the implementation so that every verification command passes.
Before you finish, rerun the verification commands yourself and confirm they are green.

### Failed Check 1
Command: `pnpm test -- --run`
Exit code: `130`
Output:
```text
[1m[30m[44m DEV [49m[39m[22m [34mv4.1.7 [39m[90m/Users/afu/Dev/VoyaVPN[39m

 [32m✓[39m src/ipc/runtime-event-store.test.ts [2m([22m[2m2 tests[22m[2m)[22m[32m 7[2mms[22m[39m
 [32m✓[39m src/i18n/locales.test.ts [2m([22m[2m5 tests[22m[2m)[22m[32m 224[2mms[22m[39m
 [32m✓[39m src/App.test.tsx [2m([22m[2m3 tests[22m[2m)[22m[33m 1972[2mms[22m[39m
     [33m[2m✓[22m[39m renders the app shell tabs and status bar [33m 518[2mms[22m[39m
     [33m[2m✓[22m[39m switches document direction through the RTL locale [33m 325[2mms[22m[39m
     [33m[2m✓[22m[39m hydrates and persists theme and strict font settings through app config [33m 1125[2mms[22m[39m
 [32m✓[39m src/features/profiles/server-table.test.tsx [2m([22m[2m8 tests[22m[2m)[22m[33m 4706[2mms[22m[39m
     [33m[2m✓[22m[39m keeps a 5k row profile list virtualized [33m 744[2mms[22m[39m
     [33m[2m✓[22m[39m runs table operations through profile IPC wrappers [33m 485[2mms[22m[39m
     [33m[2m✓[22m[39m runs import and subscription update actions through subscription IPC wrappers [33m 335[2mms[22m[39m
     [33m[2m✓[22m[39m submits every protocol through the zod-backed profile dialog path [33m 1407[2mms[22m[39m
     [33m[2m✓[22m[39m builds a policy group with child picker and generator preview [33m 1417[2mms[22m[39m

[2m Test Files [22m [1m[32m4 passed[39m[22m[90m (4)[39m
[2m      Tests [22m [1m[32m18 passed[39m[22m[90m (18)[39m
[2m   Start at [22m 11:58:41
[2m   Duration [22m 8.79s[2m (transform 2.64s, setup 1.30s, import 4.20s, tests 6.91s, environment 8.92s)[22m

[1m[30m[42m PASS [49m[39m[22m [32mWaiting for file changes...[39m
       [2mpress [22m[1mh[22m[2m to show help[22m[2m, [22m[2mpress [22m[1mq[22m[2m to quit[22m
[31mCancelling test run. Press CTRL+c again to exit forcefully.
[39m
[ELIFECYCLE] Test failed. See above for more details.
$ vitest -- --run
[vite:react-swc] We recommend switching to `@vitejs/plugin-react` for improved performance as no swc plugins are used. More information at https://vite.dev/rolldown
[90mstderr[2m | src/App.test.tsx[2m > [22m[2mApp[2m > [22m[2mrenders the app shell tabs and status bar
[22m[39mAn update to AppShell inside a test was not wrapped in act(...).

When testing, code that causes React state updates should be wrapped into act(...):

act(() => {
  /* fire events that update state */
});
/* assert on the output */

This ensures that you're testing the behavior the user would see in the browser. Learn more at https://react.dev/link/wrap-tests-with-act

[90mstderr[2m | src/App.test.tsx[2m > [22m[2mApp[2m > [22m[2mrenders the app shell tabs and status bar
[22m[39mAn update to ToastProvider inside a test was not wrapped in act(...).

When testing, code that causes React state updates should be wrapped into act(...):

act(() => {
  /* fire events that update state */
});
/* assert on the output */

This ensures that you're testing the behavior the user would see in the browser. Learn more at https://react.dev/link/wrap-tests-with-act
An update to ToastProvider inside a test was not wrapped in act(...).

When testing, code that causes React state updates should be wrapped into act(...):

act(() => {
  /* fire events that update state */
});
/* assert on the output */

This ensures that you're testing the behavior the user would see in the browser. Learn more at https://react.dev/link/wrap-tests-with-act
An update to ProfilesScreen inside a test was not wrapped in act(...).

When testing, code that causes React state updates should be wrapped into act(...):

act(() => {
  /* fire events that update state */
});
/* assert on the output */

This ensures that you're testing the behavior the user would see in the browser. Learn more at https://react.dev/link/wrap-tests-with-act
```
