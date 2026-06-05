# VoyaVPN shadcn-admin Style Alignment Implementation Plan

This plan turns the style-alignment specification into resumable Codex batches. Prose explains the delivery shape. The YAML block is the executable source for `rollout.py`; if prose and YAML disagree, the YAML block wins.

The rollout is intentionally scoped to the VoyaVPN repo. `/Users/afu/Dev/refs/shadcn-admin` is read-only reference material. The generated runner is configured with `allow_dirty: true` and `commit_per_batch: false` because this workspace may contain user-owned uncommitted files.

## Milestones

- M0: baseline inventory and reference contract.
- M1: dependency, theme, and primitive foundation.
- M2: font preferences and accent removal.
- M3: dialog and form-heavy feature migration.
- M4: full-screen feature migration.
- M5: dense and special surface migration.
- M6: shell polish, guard sweep, and final verification evidence.

## Manual Or External Checkpoints

- Do not encode visual review as a runner batch. After automated checks pass, run `pnpm tauri:dev` manually and review light/dark, fonts, RTL, dialogs, tabs, focus rings, server table, routing, DNS, groups, Clash views, QR, updates, and toasts.
- If `pnpm smoke:frontend` fails because Playwright browsers or a local display are unavailable, capture the exact environment failure in the final batch log and rerun once the environment is available.

<!-- rollout-plan:start -->

```yaml
rollout:
  name: 'voyavpn-shadcn-admin-style-alignment'
  repo_root: '/Users/afu/Dev/VoyaVPN'
  workdir: '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/logs'
  codex_cmd: null
  model: null
  max_fix_attempts: 1
  allow_dirty: true
  commit_per_batch: false
  sources_of_truth:
    - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/spec.md'
    - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/plan.md'
    - '/Users/afu/.claude/plans/refs-shadcn-admin-shimmying-dusk.md'
    - '/Users/afu/Dev/refs/shadcn-admin/src/styles/theme.css'
    - '/Users/afu/Dev/refs/shadcn-admin/src/styles/index.css'
    - '/Users/afu/Dev/refs/shadcn-admin/src/components/ui'
    - 'components.json'
  planning_notes:
    - 'This rollout is a style-system convergence, not a product workflow redesign.'
    - 'The reference repo is read-only. Copy style patterns into VoyaVPN, but never edit shadcn-admin.'
    - 'The VoyaVPN shell keeps its top tabs and bottom status bar.'
    - 'Remove only the teal, blue, and rose brand accent selector system. Keep standard shadcn --accent tokens.'
    - 'The runner does not auto-commit because the current workspace may contain user-owned dirty files.'
  success_metrics:
    - 'All required Radix dependencies and tw-animate-css are installed and locked.'
    - 'src/styles/globals.css carries the shadcn-admin neutral OKLch token system while preserving RTL and dynamic font size.'
    - 'The existing 6 UI primitives are rewritten and the required missing primitives exist.'
    - 'All 16 feature surfaces and the app shell use aligned primitives or documented special-surface token styling.'
    - 'Guard scans find no data-accent, accent-primary, setAccent, teal, sky, or rose brand accent remnants in src.'
    - 'pnpm typecheck, pnpm lint, pnpm build, and pnpm test --run pass.'
  global_context:
    - 'Stack: Tauri 2, React 19, TypeScript, Tailwind v4, CVA, Radix, lucide-react, Zustand, i18next, Vitest, Playwright.'
    - 'Current UI primitives live in src/components/ui and are old shadcn-style forwardRef components.'
    - 'Feature pages currently use raw Tailwind controls; migrate visible controls to shared UI primitives where practical.'
    - 'Use @/lib/utils cn() and existing path aliases.'
    - 'Inter, Manrope, and system are the only supported font choices after this rollout.'
    - 'Keep body font-size driven by --app-font-size.'
  hard_rules:
    - 'Do not edit /Users/afu/Dev/refs/shadcn-admin.'
    - 'Do not change the VoyaVPN tab shell into a sidebar shell.'
    - 'Do not remove @custom-variant rtl or the dynamic --app-font-size mechanism.'
    - 'Do not delete shadcn --accent tokens; delete only the brand accent selector system.'
    - 'Do not migrate Radix toast to sonner.'
    - 'Do not replace TanStack Virtual role grids with semantic tables.'
    - 'Do not replace CodeMirror, QR canvas, logs list semantics, or hidden file inputs unless behavior is preserved.'
    - 'Keep each batch focused and update tests or i18n when the touched surface requires it.'
    - 'Run the listed verification commands before declaring a batch complete.'
  batch_prompt_suffix:
    - 'Finish only this batch and the minimum supporting changes required for its verification commands.'
    - 'Prefer the shadcn-admin reference component/style over local invention.'
    - 'When migrating a feature, preserve behavior and accessibility before polishing classes.'
    - 'If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.'
phases:
  - id: '00-baseline'
    title: 'Baseline Evidence'
    goal: 'Capture the current UI/style inventory and the reference contract before code migration starts.'
    depends_on: []
    summary: 'This phase gives later batches concrete evidence for scope, hotspots, and final guard scans.'
    entry_criteria:
      - 'VoyaVPN repo is available at /Users/afu/Dev/VoyaVPN.'
      - 'shadcn-admin reference repo is available read-only at /Users/afu/Dev/refs/shadcn-admin.'
    exit_criteria:
      - 'Baseline counts and reference component list are captured under the rollout directory.'
      - 'The scope of required primitives and feature surfaces is explicit.'
    risks:
      - 'Skipping inventory can leave raw controls or accent remnants outside the final sweep.'
    batches:
      - id: '00-01-current-inventory'
        title: 'Current UI Inventory'
        kind: 'analysis'
        execution: 'codex'
        goal: 'Create an inventory document with current primitive files, feature surfaces, raw control counts, accent remnants, and special-surface notes.'
        depends_on: []
        deliverables:
          - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md with current counts and hotspot file list.'
          - 'Inventory includes ui primitive files, 16 feature surfaces, raw input/select/checkbox hits, accent hits, forwardRef hits, and ad hoc rounded/card-like hits.'
        acceptance:
          - 'The inventory cites actual rg or find commands and their results.'
          - 'The inventory separates normal feature migrations from special surfaces such as CodeMirror, server-table, Clash dense views, QR, and logs.'
        evidence_to_capture:
          - 'Baseline inventory file path and command output summary.'
        verify_commands:
          - 'test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md'
          - 'rg -n "feature surfaces|data-accent|forwardRef|special surfaces" .agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md'
        files_to_touch:
          - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md'
        prompt_context:
          - 'Use rg or rg --files for inventory. Do not change app code in this batch.'
      - id: '00-02-reference-contract'
        title: 'Reference Contract'
        kind: 'analysis'
        execution: 'codex'
        goal: 'Create a concise reference contract mapping shadcn-admin theme and component sources to VoyaVPN target files.'
        depends_on: []
        deliverables:
          - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md.'
          - 'Lists theme token source, index utility source, required primitives, optional primitives, and VoyaVPN-specific exceptions.'
        acceptance:
          - 'The contract says menubar must be restyled in place.'
          - 'The contract says toast remains Radix toast and does not migrate to sonner.'
          - 'The contract says --accent token remains but data-accent selector system is removed.'
        evidence_to_capture:
          - 'Reference contract file path and cited source paths.'
        verify_commands:
          - 'test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md'
          - 'rg -n "menubar|toast|--accent|theme.css|components/ui" .agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md'
        files_to_touch:
          - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md'
        prompt_context:
          - 'Read the shadcn-admin reference files directly, but keep the reference repo read-only.'
  - id: '01-foundation'
    title: 'Theme And Primitive Foundation'
    goal: 'Install style dependencies, align global theme tokens, and create the shared UI primitives needed by feature migration.'
    depends_on:
      - '00-baseline'
    summary: 'This phase unlocks the rest of the rollout by making shared components and CSS tokens available.'
    entry_criteria:
      - 'Baseline inventory and reference contract exist.'
    exit_criteria:
      - 'Required dependencies are installed.'
      - 'Global theme matches the neutral shadcn-admin system.'
      - 'Existing primitives are rewritten and missing primitives are available.'
    risks:
      - 'Typecheck can fail if components are added before their Radix packages are installed.'
      - 'Theme drift can break later component styling in subtle ways.'
    batches:
      - id: '01-01-style-dependencies'
        title: 'Install Style Dependencies'
        kind: 'code'
        execution: 'codex'
        goal: 'Add the Radix packages required by the new primitives and add tw-animate-css as a dev dependency.'
        depends_on: []
        deliverables:
          - 'package.json includes required Radix packages and tw-animate-css.'
          - 'pnpm-lock.yaml is updated consistently.'
        acceptance:
          - '@radix-ui/react-label, checkbox, select, switch, tooltip, scroll-area, dropdown-menu, and popover are dependencies.'
          - 'tw-animate-css is a devDependency.'
          - 'No app behavior changes are introduced in this batch.'
        evidence_to_capture:
          - 'Package dependency diff and package-manager output in runner log.'
        verify_commands:
          - >-
            node -e "const p=require('./package.json'); const deps=['@radix-ui/react-label','@radix-ui/react-checkbox','@radix-ui/react-select','@radix-ui/react-switch','@radix-ui/react-tooltip','@radix-ui/react-scroll-area','@radix-ui/react-dropdown-menu','@radix-ui/react-popover']; for (const d of deps) if (!p.dependencies || !p.dependencies[d]) throw new Error('missing '+d); if (!p.devDependencies || !p.devDependencies['tw-animate-css']) throw new Error('missing tw-animate-css');"
          - 'pnpm typecheck'
        files_to_touch:
          - 'package.json'
          - 'pnpm-lock.yaml'
        prompt_context:
          - 'Use pnpm to add dependencies so the lockfile stays valid.'
      - id: '01-02-global-theme'
        title: 'Global Theme Alignment'
        kind: 'code'
        execution: 'codex'
        goal: 'Rewrite src/styles/globals.css to the shadcn-admin neutral token system while preserving VoyaVPN-specific RTL and dynamic font behavior.'
        depends_on: []
        deliverables:
          - 'src/styles/globals.css imports tailwindcss and tw-animate-css.'
          - 'Neutral OKLch :root and .dark tokens match shadcn-admin, including popover, chart, and sidebar tokens.'
          - '@theme inline includes fonts, radius-xl, popover, chart, and sidebar mappings.'
          - 'Base layer includes outline-ring/50, scrollbars, button cursor, scroll lock override, no-scrollbar, and faded-bottom.'
        acceptance:
          - 'All data-accent theme blocks are removed.'
          - '@custom-variant rtl remains.'
          - 'body still uses var(--app-font-family) and var(--app-font-size).'
        evidence_to_capture:
          - 'Theme file diff and successful build output.'
        verify_commands:
          - 'pnpm build'
          - >-
            bash -lc 'if rg -n ":root\[data-accent|\.dark\[data-accent" src/styles/globals.css; then exit 1; fi'
          - 'rg -n "@custom-variant rtl|--app-font-family|--app-font-size|--font-manrope|--color-sidebar|faded-bottom" src/styles/globals.css'
        files_to_touch:
          - 'src/styles/globals.css'
        prompt_context:
          - 'Keep globals.css as a single file. Do not split theme.css and index.css.'
      - id: '01-03-existing-primitives'
        title: 'Rewrite Existing Primitives'
        kind: 'code'
        execution: 'codex'
        goal: 'Rewrite button, dialog, tabs, separator, menubar, and toast to aligned new-york style while preserving public APIs.'
        depends_on: []
        deliverables:
          - 'button.tsx, dialog.tsx, tabs.tsx, separator.tsx align to shadcn-admin equivalents.'
          - 'menubar.tsx keeps @radix-ui/react-menubar and aligns focus, SVG, transition, and data-slot styling.'
          - 'toast.tsx keeps Radix toast API and aligns visual styling.'
        acceptance:
          - 'Migrated primitives use function component style where appropriate and add data-slot.'
          - 'Focus rings use focus-visible:ring-[3px] and ring-ring/50 patterns where applicable.'
          - 'Dialog animations rely on tw-animate-css classes.'
          - 'TabsList uses bg-muted p-[3px] rounded-lg h-9 w-fit and active triggers use bg-background shadow-sm.'
        evidence_to_capture:
          - 'Typecheck output and component diff summary.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "forwardRef" src/components/ui/button.tsx src/components/ui/dialog.tsx src/components/ui/tabs.tsx src/components/ui/separator.tsx; then exit 1; fi'
          - 'rg -n "data-slot|ring-\[3px\]|tw-animate|bg-muted p-\[3px\]" src/components/ui/button.tsx src/components/ui/dialog.tsx src/components/ui/tabs.tsx src/components/ui/separator.tsx src/components/ui/menubar.tsx src/components/ui/toast.tsx'
        files_to_touch:
          - 'src/components/ui/button.tsx'
          - 'src/components/ui/dialog.tsx'
          - 'src/components/ui/tabs.tsx'
          - 'src/components/ui/separator.tsx'
          - 'src/components/ui/menubar.tsx'
          - 'src/components/ui/toast.tsx'
        prompt_context:
          - 'Use shadcn-admin files as references for exact style patterns. Menubar has no reference equivalent, so restyle it in place.'
      - id: '01-04-required-primitives'
        title: 'Add Required Form Primitives'
        kind: 'code'
        execution: 'codex'
        goal: 'Add the shared primitives required by visible feature form migration.'
        depends_on: []
        deliverables:
          - 'input.tsx, textarea.tsx, label.tsx, checkbox.tsx, select.tsx, switch.tsx exist under src/components/ui.'
          - 'card.tsx, badge.tsx, alert.tsx, table.tsx exist under src/components/ui.'
        acceptance:
          - 'Imports use @/lib/utils cn() and project aliases.'
          - 'Components follow the shadcn-admin new-york class patterns.'
          - 'TypeScript exports are compatible with expected shadcn usage.'
        evidence_to_capture:
          - 'Typecheck output and file list.'
        verify_commands:
          - 'pnpm typecheck'
          - 'test -f src/components/ui/input.tsx'
          - 'test -f src/components/ui/textarea.tsx'
          - 'test -f src/components/ui/label.tsx'
          - 'test -f src/components/ui/checkbox.tsx'
          - 'test -f src/components/ui/select.tsx'
          - 'test -f src/components/ui/switch.tsx'
          - 'test -f src/components/ui/card.tsx'
          - 'test -f src/components/ui/badge.tsx'
          - 'test -f src/components/ui/alert.tsx'
          - 'test -f src/components/ui/table.tsx'
        files_to_touch:
          - 'src/components/ui/input.tsx'
          - 'src/components/ui/textarea.tsx'
          - 'src/components/ui/label.tsx'
          - 'src/components/ui/checkbox.tsx'
          - 'src/components/ui/select.tsx'
          - 'src/components/ui/switch.tsx'
          - 'src/components/ui/card.tsx'
          - 'src/components/ui/badge.tsx'
          - 'src/components/ui/alert.tsx'
          - 'src/components/ui/table.tsx'
        prompt_context:
          - 'Copy or adapt only the required primitives from shadcn-admin. Do not add unused large primitives.'
      - id: '01-05-support-primitives'
        title: 'Add Support Primitives'
        kind: 'code'
        execution: 'codex'
        goal: 'Add support primitives for scroll containers, tooltips, skeletons, menus, and popovers used during polish.'
        depends_on: []
        deliverables:
          - 'scroll-area.tsx, tooltip.tsx, skeleton.tsx, dropdown-menu.tsx, and popover.tsx exist under src/components/ui.'
        acceptance:
          - 'Components follow shadcn-admin new-york style.'
          - 'No feature migration is required in this batch except fixing imports needed by the new files.'
        evidence_to_capture:
          - 'Typecheck output and file list.'
        verify_commands:
          - 'pnpm typecheck'
          - 'test -f src/components/ui/scroll-area.tsx'
          - 'test -f src/components/ui/tooltip.tsx'
          - 'test -f src/components/ui/skeleton.tsx'
          - 'test -f src/components/ui/dropdown-menu.tsx'
          - 'test -f src/components/ui/popover.tsx'
        files_to_touch:
          - 'src/components/ui/scroll-area.tsx'
          - 'src/components/ui/tooltip.tsx'
          - 'src/components/ui/skeleton.tsx'
          - 'src/components/ui/dropdown-menu.tsx'
          - 'src/components/ui/popover.tsx'
        prompt_context:
          - 'These support primitives unblock later dense-view and shell polish batches.'
  - id: '02-font-accent'
    title: 'Font Preferences And Accent Removal'
    goal: 'Replace brand accent preferences with strict font choices while preserving persisted theme, language, and font-size behavior.'
    depends_on:
      - '01-foundation'
    summary: 'This phase removes the old brand accent system from state, shell, settings, i18n, and tests.'
    entry_criteria:
      - 'Theme no longer provides data-accent token blocks.'
      - 'Required primitives exist.'
    exit_criteria:
      - 'No app code depends on accent state.'
      - 'Font selection works through store, shell, settings, and document classes.'
    risks:
      - 'Older configs may still carry ColorPrimaryName and must not crash hydration.'
      - 'Changing app-shell and modal-host together can create merge or dependency churn.'
    batches:
      - id: '02-01-font-config-store'
        title: 'Font Config And Preference Store'
        kind: 'code'
        execution: 'codex'
        goal: 'Create strict font config and update preferences-store from free-form accent/font family state to strict font choices.'
        depends_on: []
        deliverables:
          - 'src/config/fonts.ts with inter, manrope, and system definitions plus conversion helpers as needed.'
          - 'preferences-store.ts removes Accent, accent, setAccent, accentFromConfig, and accentToConfig.'
          - 'preferences-store.ts exposes font, setFont, fontToCss, fontFromFamilyString or equivalent strict helpers.'
          - 'Old ColorPrimaryName is ignored defensively during config hydration.'
        acceptance:
          - 'CurrentFontFamily maps to a strict font and falls back to inter.'
          - 'CurrentFontSize still normalizes through existing min/max logic.'
          - 'themeMode conversion behavior is unchanged.'
        evidence_to_capture:
          - 'Typecheck output and store API summary.'
        verify_commands:
          - 'pnpm typecheck'
          - 'test -f src/config/fonts.ts'
          - >-
            bash -lc 'if rg -n "type Accent|setAccent|accentFromConfig|accentToConfig" src/stores/preferences-store.ts; then exit 1; fi'
          - 'rg -n "fontToCss|fontFrom|setFont|DEFAULT_FONT" src/stores/preferences-store.ts src/config/fonts.ts'
        files_to_touch:
          - 'src/config/fonts.ts'
          - 'src/stores/preferences-store.ts'
        prompt_context:
          - 'Keep localStorage persistence tolerant of old persisted accent fields by simply not reading them.'
      - id: '02-02-shell-font-menu'
        title: 'Shell Font Menu And Theme Effects'
        kind: 'code'
        execution: 'codex'
        goal: 'Update app-shell to remove accent UI/persistence and apply strict font classes plus CSS variables.'
        depends_on: []
        deliverables:
          - 'app-shell.tsx removes accent menu options, Accent imports, setAccent, root.dataset.accent, and accent persistence.'
          - 'app-shell.tsx adds a font submenu or equivalent font radio controls using the strict font options.'
          - 'useThemeEffects removes previous font-* classes, adds font-${font}, sets --app-font-family, sets --app-font-size, and preserves colorScheme.'
          - 'index.html loads Inter and Manrope.'
        acceptance:
          - 'Persisted config no longer writes ColorPrimaryName.'
          - 'Theme/light/dark behavior remains unchanged.'
          - 'The shell continues using top tabs and bottom status bar.'
        evidence_to_capture:
          - 'Typecheck output and rg guard output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "data-accent|setAccent|accentMenuOptions|ColorPrimaryName|type Accent|accentToConfig" src/components/app-shell/app-shell.tsx; then exit 1; fi'
          - 'rg -n "font-|setFont|fontToCss|Manrope|fonts" src/components/app-shell/app-shell.tsx index.html'
        files_to_touch:
          - 'src/components/app-shell/app-shell.tsx'
          - 'index.html'
        prompt_context:
          - 'This batch owns app-shell font application. Avoid feature-page migrations here.'
      - id: '02-03-settings-i18n-tests'
        title: 'Settings Dialog, I18n, And Tests'
        kind: 'code'
        execution: 'codex'
        goal: 'Replace settings accent controls with font controls, update locale keys, and update tests from accent assertions to font assertions.'
        depends_on: []
        deliverables:
          - 'modal-host.tsx removes accent options and uses font controls plus existing font-size controls.'
          - 'Locale files remove or stop using menu/modal accent labels and add font labels.'
          - 'src/App.test.tsx no longer asserts data-accent and instead asserts font class behavior.'
        acceptance:
          - 'Settings dialog exposes theme, font, font size, source settings, integration settings, and language without accent color swatches.'
          - 'No teal/sky/rose swatch classes remain in src.'
          - 'Tests pass after assertion updates.'
        evidence_to_capture:
          - 'Vitest output and guard scan.'
        verify_commands:
          - 'pnpm test --run'
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "data-accent|accent-primary|setAccent|menu\.accent|modal\.accent|bg-teal-600|bg-sky-600|bg-rose-600" src; then exit 1; fi'
        files_to_touch:
          - 'src/components/app-shell/modal-host.tsx'
          - 'src/i18n/locales/*.json'
          - 'src/App.test.tsx'
        prompt_context:
          - 'Use Button variants or Select primitives for font choices. Keep dynamic font size controls.'
  - id: '03-dialog-features'
    title: 'Dialog And Form Feature Migration'
    goal: 'Migrate form-heavy dialogs to the aligned primitives before larger full-screen surfaces.'
    depends_on:
      - '02-font-accent'
    summary: 'These batches convert high-value feature dialogs where raw fields and alerts are concentrated.'
    entry_criteria:
      - 'Required UI primitives are available.'
      - 'Accent state has been removed from shared shell/settings code.'
    exit_criteria:
      - 'Profile, subscription, import, backup, QR, and update dialogs use aligned primitives where appropriate.'
    risks:
      - 'Form helper changes can affect many fields at once.'
      - 'Hidden file inputs and canvas behavior must remain intact.'
    batches:
      - id: '03-01-profile-dialog'
        title: 'Profile Dialog Migration'
        kind: 'code'
        execution: 'codex'
        goal: 'Migrate profile-dialog helpers and controls to Input, Select, Checkbox, Switch, Card, Badge, Label, and aligned focus states.'
        depends_on: []
        deliverables:
          - 'profile-dialog.tsx visible fields use shared primitives.'
          - 'Repeated field helpers render aligned primitives internally.'
          - 'Ad hoc protocol option cards and checkbox labels use shared tokens or Card/Badge where appropriate.'
        acceptance:
          - 'Profile dialog behavior and form schema remain unchanged.'
          - 'No visible profile-dialog checkbox uses accent-primary.'
          - 'No visible profile-dialog input/select keeps old ring-offset focus styling.'
        evidence_to_capture:
          - 'Typecheck output and targeted rg output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "accent-primary|ring-offset-background|<select\b|<input\b" src/features/profiles/profile-dialog.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/features/profiles/profile-dialog.tsx'
        prompt_context:
          - 'Do not change validation rules or protocol-specific behavior.'
      - id: '03-02-subscription-dialogs'
        title: 'Subscription And Import Dialogs'
        kind: 'code'
        execution: 'codex'
        goal: 'Migrate subscriptions-dialog and import-profiles-dialog to shared primitives and aligned alert/card/scroll patterns.'
        depends_on: []
        deliverables:
          - 'subscriptions-dialog.tsx uses Checkbox, Input, Label, Alert, Badge or Card where appropriate.'
          - 'import-profiles-dialog.tsx uses Select, Textarea, Checkbox, Label, Alert, and aligned controls.'
        acceptance:
          - 'Subscription selection, edit, import, and error behavior remain unchanged.'
          - 'Hidden or semantic-only inputs are preserved only when necessary.'
          - 'No accent-primary remains in these files.'
        evidence_to_capture:
          - 'Typecheck output and targeted rg output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "accent-primary|ring-offset-background|<select\b" src/features/subscriptions/subscriptions-dialog.tsx src/features/subscriptions/import-profiles-dialog.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/features/subscriptions/subscriptions-dialog.tsx'
          - 'src/features/subscriptions/import-profiles-dialog.tsx'
        prompt_context:
          - 'Preserve any textarea content handling and import parsing behavior.'
      - id: '03-03-backup-qr-updates'
        title: 'Backup, QR, And Updates Dialogs'
        kind: 'code'
        execution: 'codex'
        goal: 'Migrate backup, QR, and update dialogs to aligned primitives while preserving QR canvas and hidden file input behavior.'
        depends_on: []
        deliverables:
          - 'backup-dialog.tsx uses Input, Alert, Label, Card/Badge where appropriate.'
          - 'qr-dialog.tsx uses Textarea/Input styling where visible and preserves QR canvas and hidden file input.'
          - 'check-update-dialog.tsx uses Checkbox, Alert, Badge, Table, ScrollArea where appropriate.'
        acceptance:
          - 'Update table remains readable and checkbox behavior is unchanged.'
          - 'Backup success/error messages use Alert.'
          - 'QR import/export behavior is unchanged.'
          - 'No accent-primary remains in these files.'
        evidence_to_capture:
          - 'Typecheck output and targeted rg output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "accent-primary|ring-offset-background|<select\b" src/features/backup/backup-dialog.tsx src/features/qr/qr-dialog.tsx src/features/updates/check-update-dialog.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/features/backup/backup-dialog.tsx'
          - 'src/features/qr/qr-dialog.tsx'
          - 'src/features/updates/check-update-dialog.tsx'
        prompt_context:
          - 'Do not replace the QR canvas or hidden file input. Only visible controls should migrate.'
  - id: '04-screen-features'
    title: 'Full-Screen Feature Migration'
    goal: 'Migrate routing, DNS, groups, options, and logs screens to shared primitives and aligned token styling.'
    depends_on:
      - '03-dialog-features'
    summary: 'These batches handle larger screens with repeated helpers, lists, badges, alerts, and special editors.'
    entry_criteria:
      - 'Dialog-heavy migrations have passed typecheck.'
    exit_criteria:
      - 'Routing, DNS, group builder, options, and logs no longer use old visible raw field styling.'
    risks:
      - 'Routing and DNS screens have many repeated controls and third-party editors.'
      - 'Group builder has dense selectable lists where checkbox labels must remain accessible.'
    batches:
      - id: '04-01-routing-screen'
        title: 'Routing Screen Migration'
        kind: 'code'
        execution: 'codex'
        goal: 'Migrate routing-screen controls, helper fields, badges, alerts, scroll containers, and tables to aligned primitives.'
        depends_on: []
        deliverables:
          - 'Visible routing inputs use Input or Textarea.'
          - 'Native visible selects use Select primitives.'
          - 'Checkboxes use Checkbox plus Label.'
          - 'Rule badges, empty states, and alerts use aligned primitives or tokens.'
        acceptance:
          - 'Routing CRUD, search, enable toggles, and rule editing behavior remain unchanged.'
          - 'No old ring-offset focus styling or accent-primary remains.'
        evidence_to_capture:
          - 'Typecheck output and targeted rg output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "accent-primary|ring-offset-background|<select\b" src/features/routing/routing-screen.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/features/routing/routing-screen.tsx'
        prompt_context:
          - 'Prefer changing shared helper components inside routing-screen so repeated controls migrate together.'
      - id: '04-02-dns-screen'
        title: 'DNS Screen Migration'
        kind: 'code'
        execution: 'codex'
        goal: 'Migrate dns-screen controls and cards to aligned primitives while preserving CodeMirror JSON editor behavior.'
        depends_on: []
        deliverables:
          - 'Visible fields use Input, Textarea, Select, Checkbox, Label, Alert, Badge, Card, and ScrollArea where appropriate.'
          - 'CodeMirror remains in place with aligned outer border/tokens only.'
        acceptance:
          - 'DNS server and rule editing behavior remains unchanged.'
          - 'CodeMirror still renders and edits JSON.'
          - 'No visible old input/select focus styling remains.'
        evidence_to_capture:
          - 'Typecheck output and targeted rg output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "accent-primary|ring-offset-background|<select\b" src/features/dns/dns-screen.tsx; then exit 1; fi'
          - 'rg -n "CodeMirror" src/features/dns/dns-screen.tsx'
        files_to_touch:
          - 'src/features/dns/dns-screen.tsx'
        prompt_context:
          - 'Do not replace CodeMirror. Preserve current JSON editor props.'
      - id: '04-03-group-builder'
        title: 'Group Builder Migration'
        kind: 'code'
        execution: 'codex'
        goal: 'Migrate group-builder selects, checkboxes, search, badges, cards, and scroll containers to aligned primitives.'
        depends_on: []
        deliverables:
          - 'Group type select and search controls use shared primitives.'
          - 'Node selection checkboxes use Checkbox plus accessible labels.'
          - 'Route/proxy detail pills use Badge.'
          - 'Panel-like sections use Card or aligned tokens.'
        acceptance:
          - 'Group building, node selection, and route details remain unchanged.'
          - 'No accent-primary or old native select styling remains.'
        evidence_to_capture:
          - 'Typecheck output and targeted rg output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "accent-primary|ring-offset-background|<select\b" src/features/groups/group-builder.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/features/groups/group-builder.tsx'
        prompt_context:
          - 'Keep dense list ergonomics. Do not make the builder card-heavy.'
      - id: '04-04-options-and-logs'
        title: 'Options And Logs Migration'
        kind: 'code'
        execution: 'codex'
        goal: 'Migrate options screens and logs surface to aligned primitives and tokens.'
        depends_on: []
        deliverables:
          - 'source-settings.tsx uses Input and Label patterns.'
          - 'integration-settings.tsx uses Input, Card or aligned action blocks where appropriate.'
          - 'logs-screen.tsx uses aligned icon panel, badges, and scroll/list tokens without changing list semantics.'
        acceptance:
          - 'Options behavior and log rendering remain unchanged.'
          - 'No old ring-offset focus styling remains in options files.'
        evidence_to_capture:
          - 'Typecheck output and targeted rg output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "ring-offset-background|<select\b" src/features/options/source-settings.tsx src/features/options/integration-settings.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/features/options/source-settings.tsx'
          - 'src/features/options/integration-settings.tsx'
          - 'src/features/logs/logs-screen.tsx'
        prompt_context:
          - 'Logs should stay a scan-friendly list, not become a marketing-style card layout.'
  - id: '05-dense-special'
    title: 'Dense And Special Surfaces'
    goal: 'Align server table, Clash dense views, status bar controls, and remaining specialized containers without breaking virtualization or dense workflows.'
    depends_on:
      - '04-screen-features'
    summary: 'This phase handles surfaces where blind primitive replacement could harm behavior.'
    entry_criteria:
      - 'Core feature screens have migrated their normal visible controls.'
    exit_criteria:
      - 'Dense views use aligned inputs, checkboxes, badges, alerts, and tokens while preserving their structure.'
    risks:
      - 'Virtualized lists can regress if DOM structure or sizing changes unexpectedly.'
      - 'Status bar controls are compact and can overflow if typography or padding changes.'
    batches:
      - id: '05-01-server-table'
        title: 'Server Table Alignment'
        kind: 'code'
        execution: 'codex'
        goal: 'Align profiles server-table search, counts, header checkbox, row checkbox, error blocks, and row/header tokens while preserving TanStack Virtual role-grid structure.'
        depends_on: []
        deliverables:
          - 'server-table.tsx search uses Input.'
          - 'Counts and protocol/status pills use Badge or aligned badge tokens.'
          - 'Header and row checkboxes use Checkbox.'
          - 'Errors use Alert.'
          - 'Selected or active rows use neutral bg-muted style rather than brand accent.'
        acceptance:
          - 'TanStack Virtual structure and row measurement behavior remain intact.'
          - 'server-table tests pass.'
          - 'No accent-primary remains in server-table.'
        evidence_to_capture:
          - 'server-table test output and typecheck output.'
        verify_commands:
          - 'pnpm test --run src/features/profiles/server-table.test.tsx'
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "accent-primary|bg-accent/70|<select\b" src/features/profiles/server-table.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/features/profiles/server-table.tsx'
          - 'src/features/profiles/server-table.test.tsx'
        prompt_context:
          - 'Do not convert this file to semantic Table components because virtualization and role grid behavior are intentional.'
      - id: '05-02-clash-views'
        title: 'Clash View Alignment'
        kind: 'code'
        execution: 'codex'
        goal: 'Align Clash proxies and connections dense views to shared inputs, badges, alerts, scroll areas, and neutral tokens while preserving their specialized list/grid structure.'
        depends_on: []
        deliverables:
          - 'clash-connections-screen.tsx search/filter controls and badges use aligned primitives or tokens.'
          - 'clash-proxies-screen.tsx mode controls, group cards/list rows, active markers, and scroll areas use neutral aligned styling.'
        acceptance:
          - 'Clash group switching, search, close/clear actions, and connection rendering remain unchanged.'
          - 'Dense layout remains scan-friendly.'
          - 'No old raw visible search field styling remains.'
        evidence_to_capture:
          - 'Typecheck output and targeted rg output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "ring-offset-background|accent-primary|<select\b" src/features/clash/clash-connections-screen.tsx src/features/clash/clash-proxies-screen.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/features/clash/clash-connections-screen.tsx'
          - 'src/features/clash/clash-proxies-screen.tsx'
        prompt_context:
          - 'Avoid oversized cards or marketing-like layout. These are operational dense screens.'
      - id: '05-03-status-bar'
        title: 'Status Bar Alignment'
        kind: 'code'
        execution: 'codex'
        goal: 'Align status-bar compact controls and tokens to the neutral component system without changing runtime controls.'
        depends_on: []
        deliverables:
          - 'status-bar.tsx uses aligned Button/Badge/Separator/Tooltip patterns where useful.'
          - 'System proxy mode controls keep compact stable dimensions and no text overflow.'
        acceptance:
          - 'Status bar runtime state display is unchanged.'
          - 'Compact controls fit without layout shift.'
          - 'No brand accent styling remains.'
        evidence_to_capture:
          - 'Typecheck output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "accent-primary|bg-teal|bg-sky|bg-rose|ring-offset-background" src/components/app-shell/status-bar.tsx; then exit 1; fi'
        files_to_touch:
          - 'src/components/app-shell/status-bar.tsx'
        prompt_context:
          - 'Stable dimensions matter in the bottom bar. Avoid text growth that shifts layout.'
  - id: '06-final-polish'
    title: 'Shell Polish And Final Verification'
    goal: 'Polish the app shell and run global guard, test, build, and smoke verification.'
    depends_on:
      - '05-dense-special'
    summary: 'This phase finishes shared shell styling and proves the rollout is complete.'
    entry_criteria:
      - 'All feature and dense-surface batches have passed typecheck.'
    exit_criteria:
      - 'All automated verification gates pass or have exact environment-only evidence.'
      - 'Final guard scans find no deleted accent-system remnants.'
    risks:
      - 'Final lint/build/smoke can reveal issues hidden by batch-local typecheck.'
      - 'Visual fit can regress after multiple batches touch related layout tokens.'
    batches:
      - id: '06-01-app-shell-polish'
        title: 'App Shell Polish'
        kind: 'code'
        execution: 'codex'
        goal: 'Align app-shell header, tabs, content background, menu polish, and modal host sizing to the neutral shadcn-admin style while keeping the tab architecture.'
        depends_on: []
        deliverables:
          - 'app-shell.tsx uses neutral bg-card/bg-background/border-border tokens and aligned tab list styling.'
          - 'modal-host.tsx has final settings layout polish after primitive migration.'
          - 'No sidebar architecture is introduced.'
        acceptance:
          - 'Top-level layout remains header tabs plus content plus status bar.'
          - 'Tabs and menu controls have stable compact dimensions.'
          - 'No visible text overlaps in compact shell controls.'
        evidence_to_capture:
          - 'Typecheck output and targeted shell guard output.'
        verify_commands:
          - 'pnpm typecheck'
          - >-
            bash -lc 'if rg -n "data-accent|setAccent|accentMenuOptions|bg-teal|bg-sky|bg-rose" src/components/app-shell; then exit 1; fi'
        files_to_touch:
          - 'src/components/app-shell/app-shell.tsx'
          - 'src/components/app-shell/modal-host.tsx'
          - 'src/components/app-shell/status-bar.tsx'
        prompt_context:
          - 'Do not alter shell navigation architecture. This is final styling polish only.'
      - id: '06-02-global-guard-sweep'
        title: 'Global Guard Sweep'
        kind: 'verification'
        execution: 'codex'
        goal: 'Run repo-wide scans for old accent, old primitive, and raw-control remnants; fix remaining in-scope issues.'
        depends_on: []
        deliverables:
          - 'No old brand accent selectors, setters, classes, or tests remain in src.'
          - 'No forwardRef remains in src/components/ui.'
          - 'Any remaining raw input/select/checkbox occurrences are either hidden implementation details or explicitly justified in .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-sweep.md.'
        acceptance:
          - 'Guard commands pass.'
          - 'final-sweep.md documents any intentional raw controls such as hidden file input.'
        evidence_to_capture:
          - 'final-sweep.md and guard output in runner log.'
        verify_commands:
          - 'test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-sweep.md'
          - >-
            bash -lc 'if rg -n "data-accent|accent-primary|:root\[data-accent|setAccent|type Accent|bg-teal-600|bg-sky-600|bg-rose-600" src; then exit 1; fi'
          - >-
            bash -lc 'if rg -n "forwardRef" src/components/ui; then exit 1; fi'
          - 'pnpm typecheck'
        files_to_touch:
          - 'src/**'
          - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-sweep.md'
        prompt_context:
          - 'When raw controls remain intentionally, document why. Hidden file inputs are allowed; visible ad hoc fields should be migrated.'
      - id: '06-03-final-frontend-gates'
        title: 'Final Frontend Gates'
        kind: 'verification'
        execution: 'codex'
        goal: 'Run lint, build, tests, and frontend smoke; fix in-scope failures and capture final evidence.'
        depends_on: []
        deliverables:
          - 'All frontend gates pass, or environment-only smoke failure is documented with exact command and error.'
          - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md records commands and outcomes.'
        acceptance:
          - 'pnpm lint passes.'
          - 'pnpm build passes.'
          - 'pnpm test --run passes.'
          - 'pnpm smoke:frontend passes unless blocked by a documented environment-only issue.'
        evidence_to_capture:
          - 'final-verification.md and runner logs for lint/build/test/smoke.'
        verify_commands:
          - 'pnpm lint'
          - 'pnpm build'
          - 'pnpm test --run'
          - 'pnpm smoke:frontend'
          - 'test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md'
        files_to_touch:
          - '.agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md'
          - 'src/**'
        prompt_context:
          - 'If smoke fails for missing browsers or display only, write precise evidence to final-verification.md and still fix all code-caused failures.'
```

<!-- rollout-plan:end -->

## Phase Notes

### 00-baseline

This phase is evidence-only. It should not change app behavior. It exists so later batches can compare their work against explicit current-state counts and reference contracts.

### 01-foundation

Install dependencies before copying primitives. The theme batch intentionally comes before feature migration so all later components share the same token system.

### 02-font-accent

Accent removal is separated from page migration because it changes shared state and shell persistence. App shell edits are kept in a dedicated batch so later feature batches do not fight the same file.

### 03-dialog-features

Dialogs are migrated before larger screens because they concentrate repeated form helper patterns and make primitive issues visible early.

### 04-screen-features

Routing, DNS, groups, options, and logs carry the broadest raw Tailwind surface. Each screen is isolated so typecheck or behavior regressions can be addressed locally.

### 05-dense-special

Server table and Clash views need careful styling rather than structural replacement. The goal is visual alignment without breaking virtualization or dense operational scanning.

### 06-final-polish

The final phase finishes shell styling and runs repo-wide guards. Manual Tauri visual review remains outside the runner, but the automated gates should be green first.
