# VoyaVPN shadcn-admin Style Alignment Specification

## 1. Executive Summary

- Initiative type: design-system convergence and UI style migration.
- Primary repo: `/Users/afu/Dev/VoyaVPN`.
- Read-only reference repo: `/Users/afu/Dev/refs/shadcn-admin`.
- Primary planning source: `/Users/afu/.claude/plans/refs-shadcn-admin-shimmying-dusk.md`.
- Primary decision: align VoyaVPN completely to the shadcn-admin `new-york` component style, neutral OKLch theme, and font system while preserving VoyaVPN's existing Tauri, React, tabbed shell, IPC, feature workflows, and dynamic font-size behavior.
- Definition of success: VoyaVPN no longer carries its teal, blue, or rose brand-accent system or old shadcn primitive style. The app uses the shadcn-admin neutral token set, modern function-component primitives with `data-slot`, the expected missing UI primitives, Inter/Manrope/system font selection, and feature screens migrated away from ad hoc Tailwind controls to shared UI components. All frontend checks and guard scans pass.

## 2. Problem And Current State

### 2.1 Problem Statement

VoyaVPN's UI foundation is close to shadcn, but it has drifted from the reference style in two directions:

- The shared primitives under `src/components/ui` are old-style shadcn implementations using `forwardRef`, old focus rings, and no `data-slot`.
- Most feature screens use raw Tailwind markup for inputs, selects, checkboxes, cards, badges, alerts, tables, and scroll containers.

That makes the UI harder to standardize and prevents a full visual match with the reference `shadcn-admin` application. The migration must converge the design system without changing the product architecture or feature behavior.

### 2.2 Current State

- Existing VoyaVPN UI primitives:
  - `src/components/ui/button.tsx`
  - `src/components/ui/dialog.tsx`
  - `src/components/ui/menubar.tsx`
  - `src/components/ui/separator.tsx`
  - `src/components/ui/tabs.tsx`
  - `src/components/ui/toast.tsx`
- Existing app shell:
  - `src/components/app-shell/app-shell.tsx`
  - `src/components/app-shell/modal-host.tsx`
  - `src/components/app-shell/status-bar.tsx`
  - `src/components/app-shell/toaster.tsx`
- Existing theme entry:
  - `src/styles/globals.css`
- Existing preferences store:
  - `src/stores/preferences-store.ts`
- Existing feature screens:
  - `src/features/backup/backup-dialog.tsx`
  - `src/features/clash/clash-connections-screen.tsx`
  - `src/features/clash/clash-proxies-screen.tsx`
  - `src/features/dns/dns-screen.tsx`
  - `src/features/groups/group-builder.tsx`
  - `src/features/logs/logs-screen.tsx`
  - `src/features/options/integration-settings.tsx`
  - `src/features/options/source-settings.tsx`
  - `src/features/profiles/profile-dialog.tsx`
  - `src/features/profiles/server-table.tsx`
  - `src/features/qr/qr-dialog.tsx`
  - `src/features/routing/routing-screen.tsx`
  - `src/features/subscriptions/import-profiles-dialog.tsx`
  - `src/features/subscriptions/subscriptions-dialog.tsx`
  - `src/features/updates/check-update-dialog.tsx`
- Existing reference project:
  - `/Users/afu/Dev/refs/shadcn-admin/src/styles/theme.css`
  - `/Users/afu/Dev/refs/shadcn-admin/src/styles/index.css`
  - `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/*.tsx`

### 2.3 Baseline Evidence

The initial scan from this repo shows:

- `src/components/ui` has 6 primitive files.
- `src/features` has 16 `.tsx` feature surfaces.
- Feature and shell code contains 30 `<input>` occurrences.
- Feature and shell code contains 6 `<select>` occurrences.
- Feature, shell, and tests contain 17 checkbox or `accent-primary` hits.
- Frontend code contains 19 `data-accent`, `setAccent`, or `Accent` type references.
- UI primitives contain 23 `forwardRef` hits.
- Feature, shell, and UI code contain 93 `rounded-*` hits, many of which are ad hoc cards, badges, fields, or containers.
- `src/styles/globals.css` still has six `:root[data-accent=...]` and `.dark[data-accent=...]` blocks.
- `src/App.test.tsx` still asserts `data-accent="rose"`.

These counts are not success criteria by themselves; they are evidence that the work must be phased rather than handled as one edit.

## 3. Goals, Non-Goals, And Success Metrics

### 3.1 Goals

- Replace VoyaVPN's theme tokens with the shadcn-admin neutral OKLch token set.
- Remove the teal, blue, and rose brand-accent switching system while preserving standard shadcn `--accent` tokens.
- Preserve `@custom-variant rtl`, `@custom-variant dark`, `--app-font-family`, `--app-font-size`, and the body font-size mechanism.
- Add `tw-animate-css` and the Radix dependencies needed by the new primitives.
- Rewrite the existing 6 primitives to modern shadcn `new-york` style.
- Add the missing UI primitives required by the feature migration.
- Introduce strict font selection for `inter`, `manrope`, and `system`.
- Migrate all 16 feature surfaces and the app shell to use aligned primitives or aligned token patterns.
- Preserve behavior, IPC calls, modal stack behavior, virtualization, CodeMirror, QR canvas, and tabbed shell architecture.
- Update tests and i18n so accent removal is intentional and covered.

### 3.2 Non-Goals

- No sidebar migration. VoyaVPN keeps the top tab bar and bottom status bar shell.
- No product workflow redesign.
- No backend, IPC, Rust, DB, or generated binding changes unless a frontend type reference requires a defensive compatibility adjustment.
- No migration from Radix toast to sonner. The existing toast API remains stable.
- No replacement of virtualized grids with semantic tables where that would break virtualization.
- No edits to `/Users/afu/Dev/refs/shadcn-admin`.
- No broad copy of unused shadcn-admin primitives such as sidebar, command, calendar, sheet, form, avatar, alert-dialog, collapsible, radio-group, input-otp, or sonner unless a later feature batch proves they are required.

### 3.3 Success Metrics

- `pnpm typecheck` passes.
- `pnpm lint` passes.
- `pnpm build` passes.
- `pnpm test --run` passes.
- `pnpm smoke:frontend` passes or any environment-only failure is captured with exact evidence.
- Guard scan passes for no remaining `data-accent`, `accent-primary`, `:root[data-accent`, `setAccent`, `bg-teal-600`, `bg-sky-600`, or `bg-rose-600` in `src`.
- `src/components/ui` no longer uses `forwardRef` for the migrated primitives.
- All imported UI primitives resolve through `@/components/ui/...`.
- The app visually uses the neutral slate-like OKLch palette rather than teal branding.

## 4. Principles And Target State

### 4.1 Design Principles

- Match shadcn-admin before inventing local styling.
- Prefer shared primitives over repeated Tailwind field/card/badge markup.
- Keep feature behavior intact while changing presentation.
- Keep batches small enough that typecheck or a guard failure points to a local change.
- Treat `--accent` as a neutral shadcn hover token, not as a brand accent.
- Preserve VoyaVPN-specific runtime behavior: Tauri shell, Zustand preferences, i18n/RTL, dynamic font size, modal stack, and status bar.
- Use the existing `cn()` helper and project alias conventions.

### 4.2 Target State

- `src/styles/globals.css` is a single-file theme entry that imports Tailwind and `tw-animate-css`, carries the shadcn-admin token set, adds the missing `@theme inline` mappings, includes the reference base utilities, and keeps VoyaVPN-specific RTL and font-size behavior.
- `src/components/ui` contains the existing primitives rewritten to modern function components plus the required added primitives:
  - Required: `input`, `textarea`, `label`, `checkbox`, `select`, `switch`, `card`, `badge`, `alert`, `table`.
  - Polishing and support: `scroll-area`, `tooltip`, `skeleton`, `dropdown-menu`, `popover`.
- `src/stores/preferences-store.ts` exposes strict font preferences instead of free-form accent preferences.
- `src/components/app-shell/app-shell.tsx` and `modal-host.tsx` expose font selection and no brand accent selection.
- All feature screens use aligned primitives where appropriate.
- Special surfaces preserve their underlying structure:
  - CodeMirror JSON editor keeps CodeMirror.
  - `server-table.tsx` keeps TanStack Virtual and role-grid structure.
  - Clash connection and proxy dense lists keep their specialized structure.
  - QR canvas and hidden file input remain implementation details.
  - Logs remain list-based.

## 5. Capability Slices

### 5.1 Theme And Dependency Foundation

- Trigger: the style rollout starts.
- Happy path: dependencies are installed, global tokens align to shadcn-admin, fonts are loaded, and base utilities are available.
- Edge cases: `tw-animate-css` missing, `@custom-variant rtl` accidentally removed, or `--app-font-size` lost.
- Acceptance notes: typecheck/build can resolve the new CSS and packages.

### 5.2 UI Primitive Convergence

- Trigger: feature migration needs shared primitives.
- Happy path: existing primitives are rewritten and missing primitives are added from the reference style.
- Edge cases: menubar has no reference equivalent and must be restyled in place; toast must keep its current public API.
- Acceptance notes: focus rings, `data-slot`, `aria-invalid`, SVG sizing, and new-york class patterns are present.

### 5.3 Font Preference And Accent Removal

- Trigger: the theme no longer supports brand accent switching.
- Happy path: preferences hydrate from existing config without throwing, ignore old `ColorPrimaryName`, persist font family and font size, and apply `font-inter`, `font-manrope`, or `font-system`.
- Edge cases: old persisted localStorage contains `accent`; backend config still contains `ColorPrimaryName`; tests still expect `data-accent`.
- Acceptance notes: tests assert font class behavior instead of accent data attributes.

### 5.4 Feature Migration

- Trigger: the component foundation is available.
- Happy path: raw inputs, selects, checkboxes, cards, badges, alerts, tables, scroll areas, and repeated field helpers migrate to shared primitives.
- Edge cases: inputs hidden for file upload, CodeMirror, virtualized grids, and canvas remain special.
- Acceptance notes: all feature surfaces keep behavior while using aligned visual tokens.

### 5.5 Shell Polish And Final Verification

- Trigger: feature migration is complete enough for global sweep.
- Happy path: shell, status bar, tabs, dense views, guard scans, and test gates pass.
- Edge cases: Playwright smoke may expose visual or interaction regressions that typecheck misses.
- Acceptance notes: final evidence is captured in the rollout logs and any manual visual review notes are explicit.

## 6. Functional Requirements

### 6.1 Theme Tokens

- `globals.css` must keep one file rather than splitting theme and index CSS.
- It must import `tailwindcss` and `tw-animate-css`.
- It must keep `@custom-variant rtl (&:where([dir="rtl"], [dir="rtl"] *));`.
- It must keep `@custom-variant dark (&:is(.dark *));`.
- It must set `--radius: 0.625rem`.
- It must include shadcn-admin `:root` and `.dark` OKLch tokens, including popover, chart, and sidebar tokens.
- It must include `--font-inter`, `--font-manrope`, `--radius-xl`, popover, chart, and sidebar `@theme inline` mappings.
- It must remove all `data-accent` token blocks.
- It must apply `outline-ring/50`, thin scrollbar defaults, button cursor defaults, Radix scroll lock override, `no-scrollbar`, and `faded-bottom`.
- It must preserve `body` font family and font size driven by VoyaVPN CSS variables.

### 6.2 Dependencies

- Runtime dependencies to add:
  - `@radix-ui/react-label`
  - `@radix-ui/react-checkbox`
  - `@radix-ui/react-select`
  - `@radix-ui/react-switch`
  - `@radix-ui/react-tooltip`
  - `@radix-ui/react-scroll-area`
  - `@radix-ui/react-dropdown-menu`
  - `@radix-ui/react-popover`
- Dev dependency to add:
  - `tw-animate-css`
- `@radix-ui/react-slot` already exists and should not be duplicated.

### 6.3 UI Primitives

- Existing primitives must become function components where the reference uses function components.
- Existing primitives must add `data-slot` attributes.
- Focus styles must use `focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]` where applicable.
- Invalid states must use `aria-invalid:*` styles where applicable.
- Button-like components must use the reference SVG sizing pattern:
  - `[&_svg:not([class*='size-'])]:size-4`
  - `[&_svg]:pointer-events-none`
  - `[&_svg]:shrink-0`
- `dialog.tsx` must use the reference animation classes that rely on `tw-animate-css`.
- `tabs.tsx` must use the reference `bg-muted p-[3px] rounded-lg h-9 w-fit` list and active trigger style.
- `menubar.tsx` must stay based on `@radix-ui/react-menubar`.
- `toast.tsx` must keep the current Radix toast API.

### 6.4 Font And Preference Behavior

- `index.html` must load Inter and Manrope.
- `src/config/fonts.ts` must define strict font options: `inter`, `manrope`, `system`.
- `preferences-store.ts` must replace accent state with strict font state and conversion helpers.
- Old `ColorPrimaryName` config fields must be ignored defensively.
- `useThemeEffects` must remove old `font-*` classes before adding `font-${font}`.
- `useThemeEffects` must continue setting `--app-font-family` and `--app-font-size`.
- Persisted config must save `CurrentFontFamily`, `CurrentFontSize`, and `CurrentTheme`.

### 6.5 Feature Migration Patterns

- Raw visible `<input>` fields become `<Input>`.
- Textareas become `<Textarea>`.
- Native selects become `<Select>`, `SelectTrigger`, `SelectValue`, `SelectContent`, and `SelectItem`.
- Checkbox inputs become `<Checkbox>` plus `<Label>` when visible.
- Toggle buttons that represent binary state become `<Switch>` plus `<Label>`.
- Ad hoc panel divs become `Card` family components when they are card-like repeated items or framed forms.
- Status and count pills become `<Badge>`.
- Error and success blocks become `<Alert>`.
- Semantic tables become the `Table` family.
- Scroll containers become `<ScrollArea>` when replacing a simple `overflow-y-auto` wrapper.
- Repeated helper components such as `TextField`, `SelectField`, or `LabeledField` should be migrated internally so their callers benefit at once.

### 6.6 Special Surface Rules

- Do not replace the CodeMirror JSON editor in `dns-screen.tsx`; only align the wrapper border and tokens.
- Do not replace `server-table.tsx` with semantic `<Table>` because it uses TanStack Virtual and role-grid semantics.
- Do not replace Clash dense grids/lists with semantic tables unless the existing behavior is preserved.
- Do not remove the hidden QR file input.
- Do not convert logs into card-heavy layouts.

## 7. Delivery Strategy

### 7.1 Proposed Phase Shape

- Phase 00: Baseline evidence and reference contract.
- Phase 01: Dependencies, global theme, and primitive foundation.
- Phase 02: Font preferences and accent removal.
- Phase 03: Dialog and form-heavy feature migration.
- Phase 04: Full-screen feature migration.
- Phase 05: Dense and special surfaces.
- Phase 06: Shell polish and final verification.

### 7.2 Rollout And Rollback Notes

- The automated runner is generated from `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/plan.md`.
- Runtime prompts, logs, and state live under `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/logs`.
- The runner does not auto-commit batches because the current workspace may contain user-owned uncommitted files.
- Rollback is normal git containment: revert the specific batch changes or restore from the previous commit.
- The reference repo is read-only, so rollback never requires touching external sources.
- Manual Tauri desktop visual review remains outside the runner and is documented as a final checkpoint.

## 8. Technical Boundaries

### 8.1 Likely Repo Areas Touched

- `package.json`
- `pnpm-lock.yaml`
- `index.html`
- `src/styles/globals.css`
- `src/config/fonts.ts`
- `src/components/ui/**`
- `src/stores/preferences-store.ts`
- `src/components/app-shell/**`
- `src/features/**`
- `src/i18n/locales/*.json`
- `src/App.test.tsx`

### 8.2 Interfaces, Data, Or Contracts

- IPC DTOs and Rust bindings remain unchanged.
- App config persistence keeps `CurrentFontFamily`, `CurrentFontSize`, `CurrentTheme`, and `CurrentLanguage`.
- Old `ColorPrimaryName` is tolerated on input and omitted or ignored on output.
- Toast API remains unchanged.
- Modal stack API remains unchanged.
- Feature tests may change only when assertions describe removed accent behavior.

### 8.3 Runtime And Environment Assumptions

- Node, pnpm 11.5.0, TypeScript, Vite, Vitest, ESLint, and Playwright are available.
- Network is available for `pnpm add` if dependencies are not already cached.
- Playwright browsers are installed or can be installed separately.
- The sibling reference path `/Users/afu/Dev/refs/shadcn-admin` exists.

## 9. External Dependencies And Coordination

- Read-only reference sources:
  - `/Users/afu/Dev/refs/shadcn-admin/src/styles/theme.css`
  - `/Users/afu/Dev/refs/shadcn-admin/src/styles/index.css`
  - `/Users/afu/Dev/refs/shadcn-admin/src/components/ui`
- Manual checkpoint:
  - Run `pnpm tauri:dev` after automated checks and visually review light/dark, Inter/Manrope/system, RTL, dialogs, tabs, focus rings, server table, routing, DNS, groups, Clash views, QR, updates, and toasts.
- No cloud, vendor, signing, or release coordination is part of this rollout.

## 10. Hard Rules

- Do not edit `/Users/afu/Dev/refs/shadcn-admin`.
- Do not change VoyaVPN's tab shell into a sidebar shell.
- Do not remove `@custom-variant rtl`.
- Do not remove VoyaVPN's dynamic `--app-font-size` behavior.
- Do not treat shadcn `--accent` as deleted; only delete the brand accent selector system.
- Do not migrate Radix toast to sonner.
- Do not replace virtualized grids with semantic tables.
- Do not replace CodeMirror, QR canvas, or hidden file-input implementation details.
- Do not hand-roll new style systems when a shadcn-admin primitive exists.
- Do not leave old accent tokens, setters, tests, or teal/blue/rose classes in `src`.
- Keep diffs focused on the current batch.

## 11. Verification And Evidence

### 11.1 Global Verification Commands

- `pnpm typecheck`
- `pnpm lint`
- `pnpm build`
- `pnpm test --run`
- `pnpm smoke:frontend`
- `bash -lc 'if rg -n "data-accent|accent-primary|:root\\[data-accent|setAccent|bg-teal-600|bg-sky-600|bg-rose-600" src; then exit 1; fi'`
- `bash -lc 'if rg -n "forwardRef" src/components/ui; then exit 1; fi'`

### 11.2 Evidence To Capture

- Runner logs for each batch.
- Baseline inventory document under the rollout directory.
- Final guard-scan output in rollout logs.
- Final frontend verification output.
- Manual visual review notes, if performed, under the rollout directory.

### 11.3 Batch-Level Verification Guidance

- Early batches should use targeted package and file-existence checks.
- Code batches should run at least `pnpm typecheck`.
- Feature-heavy batches should run targeted tests where they exist plus typecheck.
- Final sweep must run lint, build, tests, smoke, and guard scans.

## 12. Risks, Assumptions, And Open Questions

- Risk: copying primitives before installing Radix dependencies causes typecheck failures.
- Risk: removing accent state without defensive config handling breaks older persisted app config.
- Risk: replacing native selects or checkbox inputs can subtly change labels, disabled states, or keyboard behavior.
- Risk: server table virtualization can regress if converted too aggressively.
- Risk: Playwright smoke may require browser installation in the local environment.
- Assumption: shadcn-admin remains available at `/Users/afu/Dev/refs/shadcn-admin`.
- Assumption: all feature behavior can be preserved with local frontend changes only.
- Open question: whether final manual visual evidence should be tracked in a permanent docs file or only in rollout logs. The runner keeps it in rollout logs by default.

## 13. Definition Of Done

- Dependencies and lockfile include the required Radix packages and `tw-animate-css`.
- Global theme matches the shadcn-admin neutral OKLch system and preserves VoyaVPN RTL/font-size behavior.
- Existing primitives are rewritten and missing primitives are added.
- Accent selection is removed from store, shell, settings, i18n, tests, and CSS.
- Font selection supports Inter, Manrope, and system fonts.
- All 16 feature surfaces and the app shell are migrated or explicitly treated as special surfaces with aligned token styling.
- Guard scans find no deleted accent system remnants.
- `pnpm typecheck`, `pnpm lint`, `pnpm build`, and `pnpm test --run` pass.
- `pnpm smoke:frontend` passes or has a precise environment-only follow-up note.
- The generated rollout runner can list all phases and batches.
