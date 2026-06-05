#!/usr/bin/env python3
# 用法:
#   python3 rollout.py --list
#   python3 rollout.py [--from-phase PHASE_ID | --from-batch BATCH_ID | --only-phase PHASE_ID [PHASE_ID ...] | --only-batch BATCH_ID [BATCH_ID ...]]
#                      [--force] [--dry-run] [--commit-per-batch | --no-commit-per-batch] [--codex-cmd CMD] [--model MODEL]
#                      [--reset-batch BATCH_ID] [--max-fix-attempts N] [--allow-dirty]
# 参数说明:
#   --list                  列出所有 phase 和 batch 的当前状态，不执行 rollout。
#   --from-phase PHASE_ID   从指定 phase 开始执行，并包含其后的所有 phase。
#   --from-batch BATCH_ID   从指定 batch 开始执行，并包含其后的所有 batch。
#   --only-phase ...        只执行这些 phase，并自动补齐它们依赖的 phase。
#   --only-batch ...        只执行这些 batch。
#   --force                 即使 batch 已经完成，也强制重新执行。
#   --dry-run               只生成 prompt 和日志路径，不调用 Codex CLI。
#   --commit-per-batch      每个 batch 成功后自动提交一次 git commit（默认）。
#   --no-commit-per-batch   禁用每个 batch 成功后的自动 git commit。
#   --codex-cmd CMD         覆盖默认的 Codex CLI 命令模板。
#   --model MODEL           覆盖 rollout 计划里的模型配置。
#   --reset-batch BATCH_ID  将指定 batch 的状态重置为 pending。
#   --max-fix-attempts N    覆盖计划里的最大自动修复重试次数。
#   --allow-dirty           允许在 git 脏工作区里执行。
from __future__ import annotations

import argparse
import dataclasses
import json
import os
import shlex
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


PLAN_JSON = "{\"rollout\": {\"name\": \"voyavpn-shadcn-admin-style-alignment\", \"repo_root\": \"/Users/afu/Dev/VoyaVPN\", \"workdir\": \".agents/rollouts/voyavpn-shadcn-admin-style-alignment/logs\", \"codex_cmd\": null, \"model\": null, \"max_fix_attempts\": 1, \"allow_dirty\": true, \"commit_per_batch\": false, \"sources_of_truth\": [\".agents/rollouts/voyavpn-shadcn-admin-style-alignment/spec.md\", \".agents/rollouts/voyavpn-shadcn-admin-style-alignment/plan.md\", \"/Users/afu/.claude/plans/refs-shadcn-admin-shimmying-dusk.md\", \"/Users/afu/Dev/refs/shadcn-admin/src/styles/theme.css\", \"/Users/afu/Dev/refs/shadcn-admin/src/styles/index.css\", \"/Users/afu/Dev/refs/shadcn-admin/src/components/ui\", \"components.json\"], \"planning_notes\": [\"This rollout is a style-system convergence, not a product workflow redesign.\", \"The reference repo is read-only. Copy style patterns into VoyaVPN, but never edit shadcn-admin.\", \"The VoyaVPN shell keeps its top tabs and bottom status bar.\", \"Remove only the teal, blue, and rose brand accent selector system. Keep standard shadcn --accent tokens.\", \"The runner does not auto-commit because the current workspace may contain user-owned dirty files.\"], \"success_metrics\": [\"All required Radix dependencies and tw-animate-css are installed and locked.\", \"src/styles/globals.css carries the shadcn-admin neutral OKLch token system while preserving RTL and dynamic font size.\", \"The existing 6 UI primitives are rewritten and the required missing primitives exist.\", \"All 16 feature surfaces and the app shell use aligned primitives or documented special-surface token styling.\", \"Guard scans find no data-accent, accent-primary, setAccent, teal, sky, or rose brand accent remnants in src.\", \"pnpm typecheck, pnpm lint, pnpm build, and pnpm test --run pass.\"], \"global_context\": [\"Stack: Tauri 2, React 19, TypeScript, Tailwind v4, CVA, Radix, lucide-react, Zustand, i18next, Vitest, Playwright.\", \"Current UI primitives live in src/components/ui and are old shadcn-style forwardRef components.\", \"Feature pages currently use raw Tailwind controls; migrate visible controls to shared UI primitives where practical.\", \"Use @/lib/utils cn() and existing path aliases.\", \"Inter, Manrope, and system are the only supported font choices after this rollout.\", \"Keep body font-size driven by --app-font-size.\"], \"hard_rules\": [\"Do not edit /Users/afu/Dev/refs/shadcn-admin.\", \"Do not change the VoyaVPN tab shell into a sidebar shell.\", \"Do not remove @custom-variant rtl or the dynamic --app-font-size mechanism.\", \"Do not delete shadcn --accent tokens; delete only the brand accent selector system.\", \"Do not migrate Radix toast to sonner.\", \"Do not replace TanStack Virtual role grids with semantic tables.\", \"Do not replace CodeMirror, QR canvas, logs list semantics, or hidden file inputs unless behavior is preserved.\", \"Keep each batch focused and update tests or i18n when the touched surface requires it.\", \"Run the listed verification commands before declaring a batch complete.\"], \"batch_prompt_suffix\": [\"Finish only this batch and the minimum supporting changes required for its verification commands.\", \"Prefer the shadcn-admin reference component/style over local invention.\", \"When migrating a feature, preserve behavior and accessibility before polishing classes.\", \"If an environment-only verification cannot run, capture the exact command, error, and follow-up in the rollout log or evidence file.\"]}, \"phases\": [{\"id\": \"00-baseline\", \"title\": \"Baseline Evidence\", \"goal\": \"Capture the current UI/style inventory and the reference contract before code migration starts.\", \"depends_on\": [], \"summary\": \"This phase gives later batches concrete evidence for scope, hotspots, and final guard scans.\", \"entry_criteria\": [\"VoyaVPN repo is available at /Users/afu/Dev/VoyaVPN.\", \"shadcn-admin reference repo is available read-only at /Users/afu/Dev/refs/shadcn-admin.\"], \"exit_criteria\": [\"Baseline counts and reference component list are captured under the rollout directory.\", \"The scope of required primitives and feature surfaces is explicit.\"], \"risks\": [\"Skipping inventory can leave raw controls or accent remnants outside the final sweep.\"], \"batches\": [{\"id\": \"00-01-current-inventory\", \"title\": \"Current UI Inventory\", \"kind\": \"analysis\", \"execution\": \"codex\", \"goal\": \"Create an inventory document with current primitive files, feature surfaces, raw control counts, accent remnants, and special-surface notes.\", \"depends_on\": [], \"deliverables\": [\".agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md with current counts and hotspot file list.\", \"Inventory includes ui primitive files, 16 feature surfaces, raw input/select/checkbox hits, accent hits, forwardRef hits, and ad hoc rounded/card-like hits.\"], \"acceptance\": [\"The inventory cites actual rg or find commands and their results.\", \"The inventory separates normal feature migrations from special surfaces such as CodeMirror, server-table, Clash dense views, QR, and logs.\"], \"evidence_to_capture\": [\"Baseline inventory file path and command output summary.\"], \"verify_commands\": [\"test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md\", \"rg -n \\\"feature surfaces|data-accent|forwardRef|special surfaces\\\" .agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md\"], \"files_to_touch\": [\".agents/rollouts/voyavpn-shadcn-admin-style-alignment/baseline.md\"], \"prompt_context\": [\"Use rg or rg --files for inventory. Do not change app code in this batch.\"]}, {\"id\": \"00-02-reference-contract\", \"title\": \"Reference Contract\", \"kind\": \"analysis\", \"execution\": \"codex\", \"goal\": \"Create a concise reference contract mapping shadcn-admin theme and component sources to VoyaVPN target files.\", \"depends_on\": [], \"deliverables\": [\".agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md.\", \"Lists theme token source, index utility source, required primitives, optional primitives, and VoyaVPN-specific exceptions.\"], \"acceptance\": [\"The contract says menubar must be restyled in place.\", \"The contract says toast remains Radix toast and does not migrate to sonner.\", \"The contract says --accent token remains but data-accent selector system is removed.\"], \"evidence_to_capture\": [\"Reference contract file path and cited source paths.\"], \"verify_commands\": [\"test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md\", \"rg -n \\\"menubar|toast|--accent|theme.css|components/ui\\\" .agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md\"], \"files_to_touch\": [\".agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md\"], \"prompt_context\": [\"Read the shadcn-admin reference files directly, but keep the reference repo read-only.\"]}]}, {\"id\": \"01-foundation\", \"title\": \"Theme And Primitive Foundation\", \"goal\": \"Install style dependencies, align global theme tokens, and create the shared UI primitives needed by feature migration.\", \"depends_on\": [\"00-baseline\"], \"summary\": \"This phase unlocks the rest of the rollout by making shared components and CSS tokens available.\", \"entry_criteria\": [\"Baseline inventory and reference contract exist.\"], \"exit_criteria\": [\"Required dependencies are installed.\", \"Global theme matches the neutral shadcn-admin system.\", \"Existing primitives are rewritten and missing primitives are available.\"], \"risks\": [\"Typecheck can fail if components are added before their Radix packages are installed.\", \"Theme drift can break later component styling in subtle ways.\"], \"batches\": [{\"id\": \"01-01-style-dependencies\", \"title\": \"Install Style Dependencies\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Add the Radix packages required by the new primitives and add tw-animate-css as a dev dependency.\", \"depends_on\": [], \"deliverables\": [\"package.json includes required Radix packages and tw-animate-css.\", \"pnpm-lock.yaml is updated consistently.\"], \"acceptance\": [\"@radix-ui/react-label, checkbox, select, switch, tooltip, scroll-area, dropdown-menu, and popover are dependencies.\", \"tw-animate-css is a devDependency.\", \"No app behavior changes are introduced in this batch.\"], \"evidence_to_capture\": [\"Package dependency diff and package-manager output in runner log.\"], \"verify_commands\": [\"node -e \\\"const p=require('./package.json'); const deps=['@radix-ui/react-label','@radix-ui/react-checkbox','@radix-ui/react-select','@radix-ui/react-switch','@radix-ui/react-tooltip','@radix-ui/react-scroll-area','@radix-ui/react-dropdown-menu','@radix-ui/react-popover']; for (const d of deps) if (!p.dependencies || !p.dependencies[d]) throw new Error('missing '+d); if (!p.devDependencies || !p.devDependencies['tw-animate-css']) throw new Error('missing tw-animate-css');\\\"\", \"pnpm typecheck\"], \"files_to_touch\": [\"package.json\", \"pnpm-lock.yaml\"], \"prompt_context\": [\"Use pnpm to add dependencies so the lockfile stays valid.\"]}, {\"id\": \"01-02-global-theme\", \"title\": \"Global Theme Alignment\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Rewrite src/styles/globals.css to the shadcn-admin neutral token system while preserving VoyaVPN-specific RTL and dynamic font behavior.\", \"depends_on\": [], \"deliverables\": [\"src/styles/globals.css imports tailwindcss and tw-animate-css.\", \"Neutral OKLch :root and .dark tokens match shadcn-admin, including popover, chart, and sidebar tokens.\", \"@theme inline includes fonts, radius-xl, popover, chart, and sidebar mappings.\", \"Base layer includes outline-ring/50, scrollbars, button cursor, scroll lock override, no-scrollbar, and faded-bottom.\"], \"acceptance\": [\"All data-accent theme blocks are removed.\", \"@custom-variant rtl remains.\", \"body still uses var(--app-font-family) and var(--app-font-size).\"], \"evidence_to_capture\": [\"Theme file diff and successful build output.\"], \"verify_commands\": [\"pnpm build\", \"bash -lc 'if rg -n \\\":root\\\\[data-accent|\\\\.dark\\\\[data-accent\\\" src/styles/globals.css; then exit 1; fi'\", \"rg -n \\\"@custom-variant rtl|--app-font-family|--app-font-size|--font-manrope|--color-sidebar|faded-bottom\\\" src/styles/globals.css\"], \"files_to_touch\": [\"src/styles/globals.css\"], \"prompt_context\": [\"Keep globals.css as a single file. Do not split theme.css and index.css.\"]}, {\"id\": \"01-03-existing-primitives\", \"title\": \"Rewrite Existing Primitives\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Rewrite button, dialog, tabs, separator, menubar, and toast to aligned new-york style while preserving public APIs.\", \"depends_on\": [], \"deliverables\": [\"button.tsx, dialog.tsx, tabs.tsx, separator.tsx align to shadcn-admin equivalents.\", \"menubar.tsx keeps @radix-ui/react-menubar and aligns focus, SVG, transition, and data-slot styling.\", \"toast.tsx keeps Radix toast API and aligns visual styling.\"], \"acceptance\": [\"Migrated primitives use function component style where appropriate and add data-slot.\", \"Focus rings use focus-visible:ring-[3px] and ring-ring/50 patterns where applicable.\", \"Dialog animations rely on tw-animate-css classes.\", \"TabsList uses bg-muted p-[3px] rounded-lg h-9 w-fit and active triggers use bg-background shadow-sm.\"], \"evidence_to_capture\": [\"Typecheck output and component diff summary.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"forwardRef\\\" src/components/ui/button.tsx src/components/ui/dialog.tsx src/components/ui/tabs.tsx src/components/ui/separator.tsx; then exit 1; fi'\", \"rg -n \\\"data-slot|ring-\\\\[3px\\\\]|tw-animate|bg-muted p-\\\\[3px\\\\]\\\" src/components/ui/button.tsx src/components/ui/dialog.tsx src/components/ui/tabs.tsx src/components/ui/separator.tsx src/components/ui/menubar.tsx src/components/ui/toast.tsx\"], \"files_to_touch\": [\"src/components/ui/button.tsx\", \"src/components/ui/dialog.tsx\", \"src/components/ui/tabs.tsx\", \"src/components/ui/separator.tsx\", \"src/components/ui/menubar.tsx\", \"src/components/ui/toast.tsx\"], \"prompt_context\": [\"Use shadcn-admin files as references for exact style patterns. Menubar has no reference equivalent, so restyle it in place.\"]}, {\"id\": \"01-04-required-primitives\", \"title\": \"Add Required Form Primitives\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Add the shared primitives required by visible feature form migration.\", \"depends_on\": [], \"deliverables\": [\"input.tsx, textarea.tsx, label.tsx, checkbox.tsx, select.tsx, switch.tsx exist under src/components/ui.\", \"card.tsx, badge.tsx, alert.tsx, table.tsx exist under src/components/ui.\"], \"acceptance\": [\"Imports use @/lib/utils cn() and project aliases.\", \"Components follow the shadcn-admin new-york class patterns.\", \"TypeScript exports are compatible with expected shadcn usage.\"], \"evidence_to_capture\": [\"Typecheck output and file list.\"], \"verify_commands\": [\"pnpm typecheck\", \"test -f src/components/ui/input.tsx\", \"test -f src/components/ui/textarea.tsx\", \"test -f src/components/ui/label.tsx\", \"test -f src/components/ui/checkbox.tsx\", \"test -f src/components/ui/select.tsx\", \"test -f src/components/ui/switch.tsx\", \"test -f src/components/ui/card.tsx\", \"test -f src/components/ui/badge.tsx\", \"test -f src/components/ui/alert.tsx\", \"test -f src/components/ui/table.tsx\"], \"files_to_touch\": [\"src/components/ui/input.tsx\", \"src/components/ui/textarea.tsx\", \"src/components/ui/label.tsx\", \"src/components/ui/checkbox.tsx\", \"src/components/ui/select.tsx\", \"src/components/ui/switch.tsx\", \"src/components/ui/card.tsx\", \"src/components/ui/badge.tsx\", \"src/components/ui/alert.tsx\", \"src/components/ui/table.tsx\"], \"prompt_context\": [\"Copy or adapt only the required primitives from shadcn-admin. Do not add unused large primitives.\"]}, {\"id\": \"01-05-support-primitives\", \"title\": \"Add Support Primitives\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Add support primitives for scroll containers, tooltips, skeletons, menus, and popovers used during polish.\", \"depends_on\": [], \"deliverables\": [\"scroll-area.tsx, tooltip.tsx, skeleton.tsx, dropdown-menu.tsx, and popover.tsx exist under src/components/ui.\"], \"acceptance\": [\"Components follow shadcn-admin new-york style.\", \"No feature migration is required in this batch except fixing imports needed by the new files.\"], \"evidence_to_capture\": [\"Typecheck output and file list.\"], \"verify_commands\": [\"pnpm typecheck\", \"test -f src/components/ui/scroll-area.tsx\", \"test -f src/components/ui/tooltip.tsx\", \"test -f src/components/ui/skeleton.tsx\", \"test -f src/components/ui/dropdown-menu.tsx\", \"test -f src/components/ui/popover.tsx\"], \"files_to_touch\": [\"src/components/ui/scroll-area.tsx\", \"src/components/ui/tooltip.tsx\", \"src/components/ui/skeleton.tsx\", \"src/components/ui/dropdown-menu.tsx\", \"src/components/ui/popover.tsx\"], \"prompt_context\": [\"These support primitives unblock later dense-view and shell polish batches.\"]}]}, {\"id\": \"02-font-accent\", \"title\": \"Font Preferences And Accent Removal\", \"goal\": \"Replace brand accent preferences with strict font choices while preserving persisted theme, language, and font-size behavior.\", \"depends_on\": [\"01-foundation\"], \"summary\": \"This phase removes the old brand accent system from state, shell, settings, i18n, and tests.\", \"entry_criteria\": [\"Theme no longer provides data-accent token blocks.\", \"Required primitives exist.\"], \"exit_criteria\": [\"No app code depends on accent state.\", \"Font selection works through store, shell, settings, and document classes.\"], \"risks\": [\"Older configs may still carry ColorPrimaryName and must not crash hydration.\", \"Changing app-shell and modal-host together can create merge or dependency churn.\"], \"batches\": [{\"id\": \"02-01-font-config-store\", \"title\": \"Font Config And Preference Store\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Create strict font config and update preferences-store from free-form accent/font family state to strict font choices.\", \"depends_on\": [], \"deliverables\": [\"src/config/fonts.ts with inter, manrope, and system definitions plus conversion helpers as needed.\", \"preferences-store.ts removes Accent, accent, setAccent, accentFromConfig, and accentToConfig.\", \"preferences-store.ts exposes font, setFont, fontToCss, fontFromFamilyString or equivalent strict helpers.\", \"Old ColorPrimaryName is ignored defensively during config hydration.\"], \"acceptance\": [\"CurrentFontFamily maps to a strict font and falls back to inter.\", \"CurrentFontSize still normalizes through existing min/max logic.\", \"themeMode conversion behavior is unchanged.\"], \"evidence_to_capture\": [\"Typecheck output and store API summary.\"], \"verify_commands\": [\"pnpm typecheck\", \"test -f src/config/fonts.ts\", \"bash -lc 'if rg -n \\\"type Accent|setAccent|accentFromConfig|accentToConfig\\\" src/stores/preferences-store.ts; then exit 1; fi'\", \"rg -n \\\"fontToCss|fontFrom|setFont|DEFAULT_FONT\\\" src/stores/preferences-store.ts src/config/fonts.ts\"], \"files_to_touch\": [\"src/config/fonts.ts\", \"src/stores/preferences-store.ts\"], \"prompt_context\": [\"Keep localStorage persistence tolerant of old persisted accent fields by simply not reading them.\"]}, {\"id\": \"02-02-shell-font-menu\", \"title\": \"Shell Font Menu And Theme Effects\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Update app-shell to remove accent UI/persistence and apply strict font classes plus CSS variables.\", \"depends_on\": [], \"deliverables\": [\"app-shell.tsx removes accent menu options, Accent imports, setAccent, root.dataset.accent, and accent persistence.\", \"app-shell.tsx adds a font submenu or equivalent font radio controls using the strict font options.\", \"useThemeEffects removes previous font-* classes, adds font-${font}, sets --app-font-family, sets --app-font-size, and preserves colorScheme.\", \"index.html loads Inter and Manrope.\"], \"acceptance\": [\"Persisted config no longer writes ColorPrimaryName.\", \"Theme/light/dark behavior remains unchanged.\", \"The shell continues using top tabs and bottom status bar.\"], \"evidence_to_capture\": [\"Typecheck output and rg guard output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"data-accent|setAccent|accentMenuOptions|ColorPrimaryName|type Accent|accentToConfig\\\" src/components/app-shell/app-shell.tsx; then exit 1; fi'\", \"rg -n \\\"font-|setFont|fontToCss|Manrope|fonts\\\" src/components/app-shell/app-shell.tsx index.html\"], \"files_to_touch\": [\"src/components/app-shell/app-shell.tsx\", \"index.html\"], \"prompt_context\": [\"This batch owns app-shell font application. Avoid feature-page migrations here.\"]}, {\"id\": \"02-03-settings-i18n-tests\", \"title\": \"Settings Dialog, I18n, And Tests\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Replace settings accent controls with font controls, update locale keys, and update tests from accent assertions to font assertions.\", \"depends_on\": [], \"deliverables\": [\"modal-host.tsx removes accent options and uses font controls plus existing font-size controls.\", \"Locale files remove or stop using menu/modal accent labels and add font labels.\", \"src/App.test.tsx no longer asserts data-accent and instead asserts font class behavior.\"], \"acceptance\": [\"Settings dialog exposes theme, font, font size, source settings, integration settings, and language without accent color swatches.\", \"No teal/sky/rose swatch classes remain in src.\", \"Tests pass after assertion updates.\"], \"evidence_to_capture\": [\"Vitest output and guard scan.\"], \"verify_commands\": [\"pnpm test --run\", \"pnpm typecheck\", \"bash -lc 'if rg -n \\\"data-accent|accent-primary|setAccent|menu\\\\.accent|modal\\\\.accent|bg-teal-600|bg-sky-600|bg-rose-600\\\" src; then exit 1; fi'\"], \"files_to_touch\": [\"src/components/app-shell/modal-host.tsx\", \"src/i18n/locales/*.json\", \"src/App.test.tsx\"], \"prompt_context\": [\"Use Button variants or Select primitives for font choices. Keep dynamic font size controls.\"]}]}, {\"id\": \"03-dialog-features\", \"title\": \"Dialog And Form Feature Migration\", \"goal\": \"Migrate form-heavy dialogs to the aligned primitives before larger full-screen surfaces.\", \"depends_on\": [\"02-font-accent\"], \"summary\": \"These batches convert high-value feature dialogs where raw fields and alerts are concentrated.\", \"entry_criteria\": [\"Required UI primitives are available.\", \"Accent state has been removed from shared shell/settings code.\"], \"exit_criteria\": [\"Profile, subscription, import, backup, QR, and update dialogs use aligned primitives where appropriate.\"], \"risks\": [\"Form helper changes can affect many fields at once.\", \"Hidden file inputs and canvas behavior must remain intact.\"], \"batches\": [{\"id\": \"03-01-profile-dialog\", \"title\": \"Profile Dialog Migration\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Migrate profile-dialog helpers and controls to Input, Select, Checkbox, Switch, Card, Badge, Label, and aligned focus states.\", \"depends_on\": [], \"deliverables\": [\"profile-dialog.tsx visible fields use shared primitives.\", \"Repeated field helpers render aligned primitives internally.\", \"Ad hoc protocol option cards and checkbox labels use shared tokens or Card/Badge where appropriate.\"], \"acceptance\": [\"Profile dialog behavior and form schema remain unchanged.\", \"No visible profile-dialog checkbox uses accent-primary.\", \"No visible profile-dialog input/select keeps old ring-offset focus styling.\"], \"evidence_to_capture\": [\"Typecheck output and targeted rg output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"accent-primary|ring-offset-background|<select\\\\b|<input\\\\b\\\" src/features/profiles/profile-dialog.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/features/profiles/profile-dialog.tsx\"], \"prompt_context\": [\"Do not change validation rules or protocol-specific behavior.\"]}, {\"id\": \"03-02-subscription-dialogs\", \"title\": \"Subscription And Import Dialogs\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Migrate subscriptions-dialog and import-profiles-dialog to shared primitives and aligned alert/card/scroll patterns.\", \"depends_on\": [], \"deliverables\": [\"subscriptions-dialog.tsx uses Checkbox, Input, Label, Alert, Badge or Card where appropriate.\", \"import-profiles-dialog.tsx uses Select, Textarea, Checkbox, Label, Alert, and aligned controls.\"], \"acceptance\": [\"Subscription selection, edit, import, and error behavior remain unchanged.\", \"Hidden or semantic-only inputs are preserved only when necessary.\", \"No accent-primary remains in these files.\"], \"evidence_to_capture\": [\"Typecheck output and targeted rg output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"accent-primary|ring-offset-background|<select\\\\b\\\" src/features/subscriptions/subscriptions-dialog.tsx src/features/subscriptions/import-profiles-dialog.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/features/subscriptions/subscriptions-dialog.tsx\", \"src/features/subscriptions/import-profiles-dialog.tsx\"], \"prompt_context\": [\"Preserve any textarea content handling and import parsing behavior.\"]}, {\"id\": \"03-03-backup-qr-updates\", \"title\": \"Backup, QR, And Updates Dialogs\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Migrate backup, QR, and update dialogs to aligned primitives while preserving QR canvas and hidden file input behavior.\", \"depends_on\": [], \"deliverables\": [\"backup-dialog.tsx uses Input, Alert, Label, Card/Badge where appropriate.\", \"qr-dialog.tsx uses Textarea/Input styling where visible and preserves QR canvas and hidden file input.\", \"check-update-dialog.tsx uses Checkbox, Alert, Badge, Table, ScrollArea where appropriate.\"], \"acceptance\": [\"Update table remains readable and checkbox behavior is unchanged.\", \"Backup success/error messages use Alert.\", \"QR import/export behavior is unchanged.\", \"No accent-primary remains in these files.\"], \"evidence_to_capture\": [\"Typecheck output and targeted rg output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"accent-primary|ring-offset-background|<select\\\\b\\\" src/features/backup/backup-dialog.tsx src/features/qr/qr-dialog.tsx src/features/updates/check-update-dialog.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/features/backup/backup-dialog.tsx\", \"src/features/qr/qr-dialog.tsx\", \"src/features/updates/check-update-dialog.tsx\"], \"prompt_context\": [\"Do not replace the QR canvas or hidden file input. Only visible controls should migrate.\"]}]}, {\"id\": \"04-screen-features\", \"title\": \"Full-Screen Feature Migration\", \"goal\": \"Migrate routing, DNS, groups, options, and logs screens to shared primitives and aligned token styling.\", \"depends_on\": [\"03-dialog-features\"], \"summary\": \"These batches handle larger screens with repeated helpers, lists, badges, alerts, and special editors.\", \"entry_criteria\": [\"Dialog-heavy migrations have passed typecheck.\"], \"exit_criteria\": [\"Routing, DNS, group builder, options, and logs no longer use old visible raw field styling.\"], \"risks\": [\"Routing and DNS screens have many repeated controls and third-party editors.\", \"Group builder has dense selectable lists where checkbox labels must remain accessible.\"], \"batches\": [{\"id\": \"04-01-routing-screen\", \"title\": \"Routing Screen Migration\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Migrate routing-screen controls, helper fields, badges, alerts, scroll containers, and tables to aligned primitives.\", \"depends_on\": [], \"deliverables\": [\"Visible routing inputs use Input or Textarea.\", \"Native visible selects use Select primitives.\", \"Checkboxes use Checkbox plus Label.\", \"Rule badges, empty states, and alerts use aligned primitives or tokens.\"], \"acceptance\": [\"Routing CRUD, search, enable toggles, and rule editing behavior remain unchanged.\", \"No old ring-offset focus styling or accent-primary remains.\"], \"evidence_to_capture\": [\"Typecheck output and targeted rg output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"accent-primary|ring-offset-background|<select\\\\b\\\" src/features/routing/routing-screen.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/features/routing/routing-screen.tsx\"], \"prompt_context\": [\"Prefer changing shared helper components inside routing-screen so repeated controls migrate together.\"]}, {\"id\": \"04-02-dns-screen\", \"title\": \"DNS Screen Migration\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Migrate dns-screen controls and cards to aligned primitives while preserving CodeMirror JSON editor behavior.\", \"depends_on\": [], \"deliverables\": [\"Visible fields use Input, Textarea, Select, Checkbox, Label, Alert, Badge, Card, and ScrollArea where appropriate.\", \"CodeMirror remains in place with aligned outer border/tokens only.\"], \"acceptance\": [\"DNS server and rule editing behavior remains unchanged.\", \"CodeMirror still renders and edits JSON.\", \"No visible old input/select focus styling remains.\"], \"evidence_to_capture\": [\"Typecheck output and targeted rg output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"accent-primary|ring-offset-background|<select\\\\b\\\" src/features/dns/dns-screen.tsx; then exit 1; fi'\", \"rg -n \\\"CodeMirror\\\" src/features/dns/dns-screen.tsx\"], \"files_to_touch\": [\"src/features/dns/dns-screen.tsx\"], \"prompt_context\": [\"Do not replace CodeMirror. Preserve current JSON editor props.\"]}, {\"id\": \"04-03-group-builder\", \"title\": \"Group Builder Migration\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Migrate group-builder selects, checkboxes, search, badges, cards, and scroll containers to aligned primitives.\", \"depends_on\": [], \"deliverables\": [\"Group type select and search controls use shared primitives.\", \"Node selection checkboxes use Checkbox plus accessible labels.\", \"Route/proxy detail pills use Badge.\", \"Panel-like sections use Card or aligned tokens.\"], \"acceptance\": [\"Group building, node selection, and route details remain unchanged.\", \"No accent-primary or old native select styling remains.\"], \"evidence_to_capture\": [\"Typecheck output and targeted rg output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"accent-primary|ring-offset-background|<select\\\\b\\\" src/features/groups/group-builder.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/features/groups/group-builder.tsx\"], \"prompt_context\": [\"Keep dense list ergonomics. Do not make the builder card-heavy.\"]}, {\"id\": \"04-04-options-and-logs\", \"title\": \"Options And Logs Migration\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Migrate options screens and logs surface to aligned primitives and tokens.\", \"depends_on\": [], \"deliverables\": [\"source-settings.tsx uses Input and Label patterns.\", \"integration-settings.tsx uses Input, Card or aligned action blocks where appropriate.\", \"logs-screen.tsx uses aligned icon panel, badges, and scroll/list tokens without changing list semantics.\"], \"acceptance\": [\"Options behavior and log rendering remain unchanged.\", \"No old ring-offset focus styling remains in options files.\"], \"evidence_to_capture\": [\"Typecheck output and targeted rg output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"ring-offset-background|<select\\\\b\\\" src/features/options/source-settings.tsx src/features/options/integration-settings.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/features/options/source-settings.tsx\", \"src/features/options/integration-settings.tsx\", \"src/features/logs/logs-screen.tsx\"], \"prompt_context\": [\"Logs should stay a scan-friendly list, not become a marketing-style card layout.\"]}]}, {\"id\": \"05-dense-special\", \"title\": \"Dense And Special Surfaces\", \"goal\": \"Align server table, Clash dense views, status bar controls, and remaining specialized containers without breaking virtualization or dense workflows.\", \"depends_on\": [\"04-screen-features\"], \"summary\": \"This phase handles surfaces where blind primitive replacement could harm behavior.\", \"entry_criteria\": [\"Core feature screens have migrated their normal visible controls.\"], \"exit_criteria\": [\"Dense views use aligned inputs, checkboxes, badges, alerts, and tokens while preserving their structure.\"], \"risks\": [\"Virtualized lists can regress if DOM structure or sizing changes unexpectedly.\", \"Status bar controls are compact and can overflow if typography or padding changes.\"], \"batches\": [{\"id\": \"05-01-server-table\", \"title\": \"Server Table Alignment\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Align profiles server-table search, counts, header checkbox, row checkbox, error blocks, and row/header tokens while preserving TanStack Virtual role-grid structure.\", \"depends_on\": [], \"deliverables\": [\"server-table.tsx search uses Input.\", \"Counts and protocol/status pills use Badge or aligned badge tokens.\", \"Header and row checkboxes use Checkbox.\", \"Errors use Alert.\", \"Selected or active rows use neutral bg-muted style rather than brand accent.\"], \"acceptance\": [\"TanStack Virtual structure and row measurement behavior remain intact.\", \"server-table tests pass.\", \"No accent-primary remains in server-table.\"], \"evidence_to_capture\": [\"server-table test output and typecheck output.\"], \"verify_commands\": [\"pnpm test --run src/features/profiles/server-table.test.tsx\", \"pnpm typecheck\", \"bash -lc 'if rg -n \\\"accent-primary|bg-accent/70|<select\\\\b\\\" src/features/profiles/server-table.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/features/profiles/server-table.tsx\", \"src/features/profiles/server-table.test.tsx\"], \"prompt_context\": [\"Do not convert this file to semantic Table components because virtualization and role grid behavior are intentional.\"]}, {\"id\": \"05-02-clash-views\", \"title\": \"Clash View Alignment\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Align Clash proxies and connections dense views to shared inputs, badges, alerts, scroll areas, and neutral tokens while preserving their specialized list/grid structure.\", \"depends_on\": [], \"deliverables\": [\"clash-connections-screen.tsx search/filter controls and badges use aligned primitives or tokens.\", \"clash-proxies-screen.tsx mode controls, group cards/list rows, active markers, and scroll areas use neutral aligned styling.\"], \"acceptance\": [\"Clash group switching, search, close/clear actions, and connection rendering remain unchanged.\", \"Dense layout remains scan-friendly.\", \"No old raw visible search field styling remains.\"], \"evidence_to_capture\": [\"Typecheck output and targeted rg output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"ring-offset-background|accent-primary|<select\\\\b\\\" src/features/clash/clash-connections-screen.tsx src/features/clash/clash-proxies-screen.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/features/clash/clash-connections-screen.tsx\", \"src/features/clash/clash-proxies-screen.tsx\"], \"prompt_context\": [\"Avoid oversized cards or marketing-like layout. These are operational dense screens.\"]}, {\"id\": \"05-03-status-bar\", \"title\": \"Status Bar Alignment\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Align status-bar compact controls and tokens to the neutral component system without changing runtime controls.\", \"depends_on\": [], \"deliverables\": [\"status-bar.tsx uses aligned Button/Badge/Separator/Tooltip patterns where useful.\", \"System proxy mode controls keep compact stable dimensions and no text overflow.\"], \"acceptance\": [\"Status bar runtime state display is unchanged.\", \"Compact controls fit without layout shift.\", \"No brand accent styling remains.\"], \"evidence_to_capture\": [\"Typecheck output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"accent-primary|bg-teal|bg-sky|bg-rose|ring-offset-background\\\" src/components/app-shell/status-bar.tsx; then exit 1; fi'\"], \"files_to_touch\": [\"src/components/app-shell/status-bar.tsx\"], \"prompt_context\": [\"Stable dimensions matter in the bottom bar. Avoid text growth that shifts layout.\"]}]}, {\"id\": \"06-final-polish\", \"title\": \"Shell Polish And Final Verification\", \"goal\": \"Polish the app shell and run global guard, test, build, and smoke verification.\", \"depends_on\": [\"05-dense-special\"], \"summary\": \"This phase finishes shared shell styling and proves the rollout is complete.\", \"entry_criteria\": [\"All feature and dense-surface batches have passed typecheck.\"], \"exit_criteria\": [\"All automated verification gates pass or have exact environment-only evidence.\", \"Final guard scans find no deleted accent-system remnants.\"], \"risks\": [\"Final lint/build/smoke can reveal issues hidden by batch-local typecheck.\", \"Visual fit can regress after multiple batches touch related layout tokens.\"], \"batches\": [{\"id\": \"06-01-app-shell-polish\", \"title\": \"App Shell Polish\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Align app-shell header, tabs, content background, menu polish, and modal host sizing to the neutral shadcn-admin style while keeping the tab architecture.\", \"depends_on\": [], \"deliverables\": [\"app-shell.tsx uses neutral bg-card/bg-background/border-border tokens and aligned tab list styling.\", \"modal-host.tsx has final settings layout polish after primitive migration.\", \"No sidebar architecture is introduced.\"], \"acceptance\": [\"Top-level layout remains header tabs plus content plus status bar.\", \"Tabs and menu controls have stable compact dimensions.\", \"No visible text overlaps in compact shell controls.\"], \"evidence_to_capture\": [\"Typecheck output and targeted shell guard output.\"], \"verify_commands\": [\"pnpm typecheck\", \"bash -lc 'if rg -n \\\"data-accent|setAccent|accentMenuOptions|bg-teal|bg-sky|bg-rose\\\" src/components/app-shell; then exit 1; fi'\"], \"files_to_touch\": [\"src/components/app-shell/app-shell.tsx\", \"src/components/app-shell/modal-host.tsx\", \"src/components/app-shell/status-bar.tsx\"], \"prompt_context\": [\"Do not alter shell navigation architecture. This is final styling polish only.\"]}, {\"id\": \"06-02-global-guard-sweep\", \"title\": \"Global Guard Sweep\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Run repo-wide scans for old accent, old primitive, and raw-control remnants; fix remaining in-scope issues.\", \"depends_on\": [], \"deliverables\": [\"No old brand accent selectors, setters, classes, or tests remain in src.\", \"No forwardRef remains in src/components/ui.\", \"Any remaining raw input/select/checkbox occurrences are either hidden implementation details or explicitly justified in .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-sweep.md.\"], \"acceptance\": [\"Guard commands pass.\", \"final-sweep.md documents any intentional raw controls such as hidden file input.\"], \"evidence_to_capture\": [\"final-sweep.md and guard output in runner log.\"], \"verify_commands\": [\"test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-sweep.md\", \"bash -lc 'if rg -n \\\"data-accent|accent-primary|:root\\\\[data-accent|setAccent|type Accent|bg-teal-600|bg-sky-600|bg-rose-600\\\" src; then exit 1; fi'\", \"bash -lc 'if rg -n \\\"forwardRef\\\" src/components/ui; then exit 1; fi'\", \"pnpm typecheck\"], \"files_to_touch\": [\"src/**\", \".agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-sweep.md\"], \"prompt_context\": [\"When raw controls remain intentionally, document why. Hidden file inputs are allowed; visible ad hoc fields should be migrated.\"]}, {\"id\": \"06-03-final-frontend-gates\", \"title\": \"Final Frontend Gates\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Run lint, build, tests, and frontend smoke; fix in-scope failures and capture final evidence.\", \"depends_on\": [], \"deliverables\": [\"All frontend gates pass, or environment-only smoke failure is documented with exact command and error.\", \".agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md records commands and outcomes.\"], \"acceptance\": [\"pnpm lint passes.\", \"pnpm build passes.\", \"pnpm test --run passes.\", \"pnpm smoke:frontend passes unless blocked by a documented environment-only issue.\"], \"evidence_to_capture\": [\"final-verification.md and runner logs for lint/build/test/smoke.\"], \"verify_commands\": [\"pnpm lint\", \"pnpm build\", \"pnpm test --run\", \"pnpm smoke:frontend\", \"test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md\"], \"files_to_touch\": [\".agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-verification.md\", \"src/**\"], \"prompt_context\": [\"If smoke fails for missing browsers or display only, write precise evidence to final-verification.md and still fix all code-caused failures.\"]}]}]}"
PLAN = json.loads(PLAN_JSON)
MAX_VERIFY_OUTPUT_CHARS = 12000
DEFAULT_CODEX_CMD = "codex exec --dangerously-bypass-approvals-and-sandbox --cd {repo} -"


@dataclasses.dataclass
class Batch:
    id: str
    title: str
    kind: str
    execution: str
    goal: str
    depends_on: list[str]
    deliverables: list[str]
    acceptance: list[str]
    evidence_to_capture: list[str]
    verify_commands: list[str]
    files_to_touch: list[str]
    prompt_context: list[str]


@dataclasses.dataclass
class Phase:
    id: str
    title: str
    goal: str
    summary: str
    depends_on: list[str]
    entry_criteria: list[str]
    exit_criteria: list[str]
    risks: list[str]
    batches: list[Batch]


@dataclasses.dataclass
class VerifyFailure:
    cmd: str
    exit_code: int
    output: str


@dataclasses.dataclass
class CodexFailure:
    exit_code: int
    output: str


@dataclasses.dataclass
class VerifyResult:
    ok: bool
    failures: list[VerifyFailure] = dataclasses.field(default_factory=list)


class Colors:
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    CYAN = "\033[36m"


def c(text: str, *styles: str) -> str:
    if not sys.stdout.isatty():
        return text
    return "".join(styles) + text + Colors.RESET


def require(condition: bool, message: str) -> None:
    if condition:
        return
    print(c(f"! {message}", Colors.RED))
    sys.exit(2)


def build_phase_graph() -> list[Phase]:
    raw_phases = PLAN["phases"]
    phases: list[Phase] = []
    seen_phase_ids: set[str] = set()
    seen_batch_ids: set[str] = set()

    for index, raw_phase in enumerate(raw_phases):
        phase_id = raw_phase["id"]
        require(phase_id not in seen_phase_ids, f"Duplicate phase id: {phase_id}")
        seen_phase_ids.add(phase_id)

        depends_on = list(raw_phase.get("depends_on") or ([] if index == 0 else [raw_phases[index - 1]["id"]]))
        batches: list[Batch] = []
        for raw_batch in raw_phase["batches"]:
            batch_id = raw_batch["id"]
            require(batch_id not in seen_batch_ids, f"Duplicate batch id: {batch_id}")
            seen_batch_ids.add(batch_id)
            batches.append(
                Batch(
                    id=batch_id,
                    title=raw_batch["title"],
                    kind=raw_batch.get("kind") or "code",
                    execution=raw_batch.get("execution") or "codex",
                    goal=raw_batch["goal"],
                    depends_on=list(raw_batch.get("depends_on") or []),
                    deliverables=list(raw_batch.get("deliverables") or []),
                    acceptance=list(raw_batch.get("acceptance") or []),
                    evidence_to_capture=list(raw_batch.get("evidence_to_capture") or []),
                    verify_commands=list(raw_batch.get("verify_commands") or []),
                    files_to_touch=list(raw_batch.get("files_to_touch") or []),
                    prompt_context=list(raw_batch.get("prompt_context") or []),
                )
            )

        phases.append(
            Phase(
                id=phase_id,
                title=raw_phase["title"],
                goal=raw_phase["goal"],
                summary=raw_phase.get("summary") or "",
                depends_on=depends_on,
                entry_criteria=list(raw_phase.get("entry_criteria") or []),
                exit_criteria=list(raw_phase.get("exit_criteria") or []),
                risks=list(raw_phase.get("risks") or []),
                batches=batches,
            )
        )

    phase_ids = {phase.id for phase in phases}
    missing = sorted(
        dependency
        for phase in phases
        for dependency in phase.depends_on
        if dependency not in phase_ids
    )
    require(not missing, f"Unknown phase dependencies: {', '.join(missing)}")
    return phases


ROLLOUT = PLAN["rollout"]
REPO = Path(ROLLOUT["repo_root"]).resolve()
RAW_WORKDIR = Path(ROLLOUT.get("workdir") or f".AGENTS/rollouts/{ROLLOUT['name']}/logs")
WORKDIR = RAW_WORKDIR if RAW_WORKDIR.is_absolute() else REPO / RAW_WORKDIR
STATE = WORKDIR / "state.json"
PROMPTS_DIR = WORKDIR / "prompts"
LOGS_DIR = WORKDIR / "logs"

PHASES = build_phase_graph()
PHASE_BY_ID = {phase.id: phase for phase in PHASES}
BATCH_BY_ID = {batch.id: batch for phase in PHASES for batch in phase.batches}
PHASE_BY_BATCH_ID = {batch.id: phase for phase in PHASES for batch in phase.batches}
ALL_BATCH_IDS = [batch.id for phase in PHASES for batch in phase.batches]


def validate_batch_dependencies() -> None:
    missing = sorted(
        dependency
        for batch in BATCH_BY_ID.values()
        for dependency in batch.depends_on
        if dependency not in BATCH_BY_ID
    )
    require(not missing, f"Unknown batch dependencies: {', '.join(missing)}")

    self_refs = sorted(batch.id for batch in BATCH_BY_ID.values() if batch.id in batch.depends_on)
    require(not self_refs, f"Batch cannot depend on itself: {', '.join(self_refs)}")


validate_batch_dependencies()


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(REPO))
    except ValueError:
        return str(path)


def truncate_output(text: str, limit: int = MAX_VERIFY_OUTPUT_CHARS) -> str:
    text = text.strip()
    if len(text) <= limit:
        return text
    return text[: limit - 16].rstrip() + "\n...[truncated]"


def load_state() -> dict:
    if not STATE.exists():
        return {"batches": {}}
    return json.loads(STATE.read_text())


def save_state(state: dict) -> None:
    STATE.parent.mkdir(parents=True, exist_ok=True)
    STATE.write_text(json.dumps(state, indent=2, ensure_ascii=False))


def mark_batch(state: dict, batch_id: str, status: str, **extra) -> None:
    state["batches"][batch_id] = {
        "status": status,
        "ts": datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z"),
        **extra,
    }
    save_state(state)


def ensure_dirs() -> None:
    for directory in (WORKDIR, PROMPTS_DIR, LOGS_DIR):
        directory.mkdir(parents=True, exist_ok=True)


def append_log(log_path: Path, text: str) -> None:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with log_path.open("ab") as handle:
        handle.write(text.encode())


def render_bullets(values: list[str], formatter) -> list[str]:
    if not values:
        return ["- None"]
    return [formatter(value) for value in values]


def render_prompt(phase: Phase, batch: Batch, extra_notes: str | None = None) -> str:
    sources = list(ROLLOUT.get("sources_of_truth") or [])
    planning_notes = list(ROLLOUT.get("planning_notes") or [])
    success_metrics = list(ROLLOUT.get("success_metrics") or [])
    global_context = list(ROLLOUT.get("global_context") or [])
    hard_rules = list(ROLLOUT.get("hard_rules") or [])
    suffix = list(ROLLOUT.get("batch_prompt_suffix") or [])

    parts = [
        f"# Batch {batch.id}: {batch.title}",
        "",
        f"You are implementing the rollout `{ROLLOUT['name']}` in the repository rooted at `{REPO}`.",
        "",
        "## Phase",
        f"- `{phase.id}` — {phase.title}",
        f"- Goal: {phase.goal}",
    ]
    if phase.summary:
        parts.append(f"- Context: {phase.summary}")
    if phase.entry_criteria:
        parts.extend(
            [
                "",
                "## Phase Entry Criteria",
                *render_bullets(phase.entry_criteria, lambda value: f"- {value}"),
            ]
        )
    if phase.exit_criteria:
        parts.extend(
            [
                "",
                "## Phase Exit Criteria",
                *render_bullets(phase.exit_criteria, lambda value: f"- {value}"),
            ]
        )
    if phase.risks:
        parts.extend(
            [
                "",
                "## Phase Risks",
                *render_bullets(phase.risks, lambda value: f"- {value}"),
            ]
        )

    parts.extend(
        [
            "",
            "## Batch Shape",
            f"- Kind: `{batch.kind}`",
            f"- Execution: `{batch.execution}`",
            "",
            "## Batch Goal",
            batch.goal,
            "",
            "## Depends On",
            *render_bullets(batch.depends_on, lambda value: f"- `{value}`"),
            "",
            "## Deliverables",
            *render_bullets(batch.deliverables, lambda value: f"- {value}"),
            "",
            "## Acceptance",
            *render_bullets(batch.acceptance, lambda value: f"- {value}"),
            "",
            "## Evidence To Capture",
            *render_bullets(batch.evidence_to_capture, lambda value: f"- {value}"),
            "",
            "## Verification Commands (must pass before declaring success)",
            *render_bullets(batch.verify_commands, lambda value: f"- `{value}`"),
        ]
    )

    if batch.files_to_touch:
        parts.extend(
            [
                "",
                "## Likely Files",
                *[f"- `{value}`" for value in batch.files_to_touch],
            ]
        )

    parts.extend(
        [
            "",
            "## Sources Of Truth",
            *render_bullets(sources, lambda value: f"- `{value}`"),
            "",
            "## Planning Notes",
            *render_bullets(planning_notes, lambda value: f"- {value}"),
            "",
            "## Success Metrics",
            *render_bullets(success_metrics, lambda value: f"- {value}"),
            "",
            "## Global Context",
            *render_bullets(global_context, lambda value: f"- {value}"),
            "",
            "## Hard Rules",
            *render_bullets(hard_rules, lambda value: f"- {value}"),
        ]
    )

    if batch.prompt_context:
        parts.extend(
            [
                "",
                "## Batch Context",
                *[f"- {value}" for value in batch.prompt_context],
            ]
        )

    if suffix:
        parts.extend(
            [
                "",
                "## Working Agreement",
                *[f"- {value}" for value in suffix],
            ]
        )

    if extra_notes:
        parts.extend(
            [
                "",
                "## Retry Context",
                extra_notes.rstrip(),
            ]
        )

    parts.append("")
    return "\n".join(parts)


def write_prompt(phase: Phase, batch: Batch, attempt: int, extra_notes: str | None) -> Path:
    suffix = "" if attempt == 0 else f".retry{attempt}"
    path = PROMPTS_DIR / f"{batch.id}{suffix}.md"
    path.write_text(render_prompt(phase, batch, extra_notes=extra_notes))
    return path


def run_shell(cmd: str, cwd: Path = REPO, check: bool = True, *, capture_output: bool = False) -> subprocess.CompletedProcess:
    print(c(f"$ {cmd}", Colors.DIM))
    return subprocess.run(
        cmd,
        shell=True,
        cwd=cwd,
        check=check,
        capture_output=capture_output,
        text=capture_output,
    )


def invoke_codex(
    phase: Phase,
    batch: Batch,
    codex_cmd: list[str],
    log_path: Path,
    dry_run: bool,
    *,
    attempt: int = 0,
    extra_notes: str | None = None,
) -> tuple[int, Path, str]:
    prompt_path = write_prompt(phase, batch, attempt=attempt, extra_notes=extra_notes)
    print(c(f"→ prompt: {display_path(prompt_path)}", Colors.DIM))
    print(c(f"→ log:    {display_path(log_path)}", Colors.DIM))

    if dry_run:
        print(c("  (dry-run, skipping codex invocation)", Colors.YELLOW))
        return 0, prompt_path, ""

    mode = "wb" if attempt == 0 else "ab"
    with prompt_path.open("rb") as stdin, log_path.open(mode) as log:
        if attempt > 0:
            log.write(b"\n")
        log.write(f"# codex invocation {attempt + 1} for {batch.id}\n".encode())
        log.write(f"# cmd: {shlex.join(codex_cmd)}\n".encode())
        log.write(f"# ts:  {datetime.now(timezone.utc).isoformat()}\n\n".encode())
        log.flush()
        proc = subprocess.Popen(
            codex_cmd,
            cwd=REPO,
            stdin=stdin,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        assert proc.stdout is not None
        output = bytearray()
        for line in proc.stdout:
            sys.stdout.buffer.write(line)
            sys.stdout.buffer.flush()
            log.write(line)
            output.extend(line)
        return proc.wait(), prompt_path, output.decode("utf-8", errors="replace").rstrip()


def verify_batch(batch: Batch, log_path: Path) -> VerifyResult:
    if not batch.verify_commands:
        return VerifyResult(ok=True)

    print(c(f"▶ verifying {batch.id}", Colors.CYAN))
    append_log(log_path, f"\n# verification for {batch.id}\n")

    for cmd in batch.verify_commands:
        append_log(log_path, f"\n$ {cmd}\n")
        proc = run_shell(cmd, check=False, capture_output=True)
        output = ((proc.stdout or "") + (proc.stderr or "")).rstrip()
        if output:
            print(output)
            append_log(log_path, output + "\n")
        append_log(log_path, f"[exit {proc.returncode}]\n")
        if proc.returncode != 0:
            print(c(f"✗ verify failed: {cmd} (exit {proc.returncode})", Colors.RED))
            return VerifyResult(
                ok=False,
                failures=[
                    VerifyFailure(
                        cmd=cmd,
                        exit_code=proc.returncode,
                        output=truncate_output(output or "(no output)"),
                    )
                ],
            )
    return VerifyResult(ok=True)


def git_is_clean() -> bool:
    result = subprocess.run(
        "git status --porcelain",
        shell=True,
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() == ""


def build_codex_retry_notes(batch: Batch, codex_failure: CodexFailure, retry_number: int) -> str:
    return "\n".join(
        [
            f"The previous Codex CLI attempt for batch `{batch.id}` exited with a non-zero status.",
            f"Retry number: {retry_number}",
            "",
            "Inspect the error output below, keep any useful in-progress changes, and continue fixing the batch.",
            "Before you finish, rerun the verification commands yourself and confirm they are green.",
            "",
            "### Codex CLI Failure",
            f"Exit code: `{codex_failure.exit_code}`",
            "Output:",
            "```text",
            codex_failure.output,
            "```",
            "",
        ]
    )


def build_verify_retry_notes(batch: Batch, verify_result: VerifyResult, retry_number: int) -> str:
    parts = [
        f"The previous attempt for batch `{batch.id}` failed verification.",
        f"Retry number: {retry_number}",
        "",
        "Fix the implementation so that every verification command passes.",
        "Before you finish, rerun the verification commands yourself and confirm they are green.",
        "",
    ]
    for index, failure in enumerate(verify_result.failures, start=1):
        parts.extend(
            [
                f"### Failed Check {index}",
                f"Command: `{failure.cmd}`",
                f"Exit code: `{failure.exit_code}`",
                "Output:",
                "```text",
                failure.output,
                "```",
                "",
            ]
        )
    return "\n".join(parts)


def git_commit_batch(batch: Batch) -> None:
    run_shell("git add -A", check=False)
    if git_is_clean():
        print(c("  (no changes to commit)", Colors.DIM))
        return
    message = f"rollout({batch.id}): {batch.title}\n\nAutomated commit by generated rollout.py"
    run_shell(f"git commit -m {shlex.quote(message)}")


def strip_outer_quotes(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def split_command_line(command: str) -> list[str]:
    if sys.platform == "win32":
        return [strip_outer_quotes(part) for part in shlex.split(command, posix=False)]
    return shlex.split(command)


def find_executable(command: str) -> str | None:
    resolved = shutil.which(command)
    if resolved:
        return resolved

    candidate = Path(command)
    if candidate.exists():
        return str(candidate)

    if sys.platform != "win32":
        return None

    suffixes = [""] if candidate.suffix else [".cmd", ".bat", ".exe", ".ps1"]
    search_dirs = os.environ.get("PATH", "").split(os.pathsep)
    for directory in search_dirs:
        if not directory:
            continue
        for suffix in suffixes:
            executable = Path(directory) / f"{command}{suffix}"
            if executable.exists():
                return str(executable)
    return None


def resolve_executable_command(command: str) -> list[str]:
    executable = find_executable(command)
    if executable is None:
        print(c(f"! 未找到命令 `{command}`。请安装 Codex CLI，或使用 --codex-cmd 覆盖。", Colors.RED))
        sys.exit(2)

    if sys.platform == "win32" and Path(executable).suffix.lower() == ".ps1":
        launcher = shutil.which("pwsh") or shutil.which("powershell")
        if launcher is None:
            print(c(f"! `{command}` 解析为 PowerShell 脚本，但未找到 pwsh/powershell。", Colors.RED))
            sys.exit(2)
        script_path = executable.replace("'", "''")
        return [launcher, "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", f"$input | & '{script_path}' @args"]

    return [executable]


def resolve_codex_cmd(user_cmd: str | None, model: str | None) -> list[str]:
    template = user_cmd or ROLLOUT.get("codex_cmd") or DEFAULT_CODEX_CMD
    rendered = template.format(repo=str(REPO))
    cmd = split_command_line(rendered)
    require(bool(cmd), "Codex command is empty.")
    cmd = [*resolve_executable_command(cmd[0]), *cmd[1:]]
    if model and "--model" not in cmd:
        if "-" in cmd:
            index = cmd.index("-")
            cmd[index:index] = ["--model", model]
        else:
            cmd.extend(["--model", model])
    if "-" not in cmd:
        cmd.append("-")
    return cmd


def ordered_unique(items: list[str]) -> list[str]:
    seen: set[str] = set()
    ordered: list[str] = []
    for item in items:
        if item in seen:
            continue
        seen.add(item)
        ordered.append(item)
    return ordered


def phase_dependency_ids(phase_id: str) -> list[str]:
    ordered: list[str] = []
    visited: set[str] = set()

    def visit(target_id: str) -> None:
        for dependency in PHASE_BY_ID[target_id].depends_on:
            if dependency in visited:
                continue
            visit(dependency)
            visited.add(dependency)
            ordered.append(dependency)

    visit(phase_id)
    return ordered


def batch_prerequisites(batch_id: str) -> list[str]:
    batch = BATCH_BY_ID[batch_id]
    phase = PHASE_BY_BATCH_ID[batch_id]
    phase_dependency_set = set(phase_dependency_ids(phase.id))
    prerequisites: list[str] = []

    for candidate_phase in PHASES:
        if candidate_phase.id in phase_dependency_set:
            prerequisites.extend(batch.id for batch in candidate_phase.batches)

    for candidate_batch in phase.batches:
        if candidate_batch.id == batch_id:
            break
        prerequisites.append(candidate_batch.id)

    prerequisites.extend(batch.depends_on)
    return ordered_unique(prerequisites)


def require_known_phase_ids(flag: str, phase_ids: list[str]) -> None:
    unknown = [phase_id for phase_id in phase_ids if phase_id not in PHASE_BY_ID]
    require(not unknown, f"{flag} contains unknown phase ids: {', '.join(unknown)}")


def require_known_batch_ids(flag: str, batch_ids: list[str]) -> None:
    unknown = [batch_id for batch_id in batch_ids if batch_id not in BATCH_BY_ID]
    require(not unknown, f"{flag} contains unknown batch ids: {', '.join(unknown)}")


def expand_phase_ids_with_dependencies(phase_ids: list[str]) -> list[str]:
    ordered: list[str] = []
    visited: set[str] = set()
    visiting: set[str] = set()

    def visit(phase_id: str) -> None:
        if phase_id in visited:
            return
        require(phase_id not in visiting, f"Cyclic phase dependency detected at {phase_id}")
        visiting.add(phase_id)
        for dependency in PHASE_BY_ID[phase_id].depends_on:
            visit(dependency)
        visiting.remove(phase_id)
        visited.add(phase_id)
        ordered.append(phase_id)

    for phase_id in phase_ids:
        visit(phase_id)
    return ordered


def batch_ids_for_phases(phase_ids: list[str]) -> list[str]:
    phase_set = set(phase_ids)
    return [batch.id for phase in PHASES if phase.id in phase_set for batch in phase.batches]


def select_batch_ids(args, state: dict) -> list[str]:
    if args.only_phase:
        require_known_phase_ids("--only-phase", args.only_phase)
        phase_ids = expand_phase_ids_with_dependencies(ordered_unique(args.only_phase))
        selected = batch_ids_for_phases(phase_ids)
    elif args.only_batch:
        require_known_batch_ids("--only-batch", args.only_batch)
        target_set = set(args.only_batch)
        selected = [batch_id for batch_id in ALL_BATCH_IDS if batch_id in target_set]
    elif args.from_phase:
        require_known_phase_ids("--from-phase", [args.from_phase])
        start_index = next(index for index, phase in enumerate(PHASES) if phase.id == args.from_phase)
        selected = [batch.id for phase in PHASES[start_index:] for batch in phase.batches]
    elif args.from_batch:
        require_known_batch_ids("--from-batch", [args.from_batch])
        start_index = ALL_BATCH_IDS.index(args.from_batch)
        selected = ALL_BATCH_IDS[start_index:]
    else:
        selected = list(ALL_BATCH_IDS)

    if args.force:
        return selected

    done = {
        batch_id
        for batch_id, info in state.get("batches", {}).items()
        if info.get("status") == "done"
    }
    return [batch_id for batch_id in selected if batch_id not in done]


def ensure_selection_ready(selected_batch_ids: list[str], state: dict) -> None:
    completed = {
        batch_id
        for batch_id, info in state.get("batches", {}).items()
        if info.get("status") == "done"
    }
    planned_now: set[str] = set()

    for batch_id in selected_batch_ids:
        missing = [
            dependency
            for dependency in batch_prerequisites(batch_id)
            if dependency not in completed and dependency not in planned_now
        ]
        require(
            not missing,
            f"Batch `{batch_id}` is blocked by unfinished prerequisites: {', '.join(missing)}. "
            "Run an earlier phase or batch first, or rerun with a broader selection.",
        )
        planned_now.add(batch_id)


def batch_status(state: dict, batch_id: str) -> str:
    return state.get("batches", {}).get(batch_id, {}).get("status", "pending")


def phase_status(phase: Phase, state: dict) -> tuple[str, int, int]:
    statuses = [batch_status(state, batch.id) for batch in phase.batches]
    done_count = sum(status == "done" for status in statuses)
    total = len(statuses)
    if done_count == total:
        return "done", done_count, total
    if "failed" in statuses:
        return "failed", done_count, total
    if "running" in statuses:
        return "running", done_count, total
    if done_count:
        return "partial", done_count, total
    return "pending", done_count, total


def list_plan(state: dict) -> None:
    print(c(f"Rollout: {ROLLOUT['name']}", Colors.BOLD))
    for phase in PHASES:
        status, done_count, total = phase_status(phase, state)
        print(f"  {phase.id}  {phase.title}  [{status} {done_count}/{total}]")
        for batch in phase.batches:
            print(
                f"    - {batch.id}  {batch.title}  "
                f"[{batch_status(state, batch.id)}; {batch.execution}/{batch.kind}]"
            )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=f"Run rollout plan: {ROLLOUT['name']}",
    )
    parser.add_argument("--list", action="store_true", help="List phases and batch status")

    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--from-phase", dest="from_phase", metavar="PHASE_ID", help="Start from this phase")
    selection.add_argument("--from-batch", dest="from_batch", metavar="BATCH_ID", help="Start from this batch")
    selection.add_argument("--only-phase", nargs="+", metavar="PHASE_ID", help="Run only these phases")
    selection.add_argument("--only-batch", nargs="+", metavar="BATCH_ID", help="Run only these batches")

    parser.add_argument("--force", action="store_true", help="Rerun selected batches even if already done")
    parser.add_argument("--dry-run", action="store_true", help="Write prompts only, do not invoke Codex")
    commit_group = parser.add_mutually_exclusive_group()
    commit_group.add_argument(
        "--commit-per-batch",
        dest="commit_per_batch",
        action="store_true",
        default=None,
        help="Commit after each successful batch (default)",
    )
    commit_group.add_argument(
        "--no-commit-per-batch",
        dest="commit_per_batch",
        action="store_false",
        help="Do not commit after each successful batch",
    )
    parser.add_argument("--codex-cmd", help="Override the Codex command template")
    parser.add_argument("--model", help="Override the Codex model")
    parser.add_argument("--reset-batch", metavar="BATCH_ID", help="Reset one batch to pending state")
    parser.add_argument(
        "--max-fix-attempts",
        type=int,
        default=None,
        help="Retries after Codex or verification failures; defaults to the plan value",
    )
    parser.add_argument("--allow-dirty", action="store_true", help="Allow a dirty git worktree")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    state = load_state()

    if args.list:
        list_plan(state)
        return 0

    if args.reset_batch:
        require_known_batch_ids("--reset-batch", [args.reset_batch])
        state.setdefault("batches", {}).pop(args.reset_batch, None)
        save_state(state)
        print(c(f"Reset batch `{args.reset_batch}` to pending.", Colors.GREEN))
        return 0

    require(REPO.exists(), f"Repository root does not exist: {REPO}")

    max_fix_attempts = (
        ROLLOUT.get("max_fix_attempts", 1)
        if args.max_fix_attempts is None
        else args.max_fix_attempts
    )
    require(max_fix_attempts >= 0, "--max-fix-attempts cannot be negative.")

    allow_dirty = bool(ROLLOUT.get("allow_dirty", False) or args.allow_dirty)
    commit_per_batch = bool(
        ROLLOUT.get("commit_per_batch", True)
        if args.commit_per_batch is None
        else args.commit_per_batch
    )
    require(
        not (commit_per_batch and allow_dirty),
        "`commit_per_batch` cannot be combined with `--allow-dirty`; pass `--no-commit-per-batch` or set `rollout.commit_per_batch: false`.",
    )

    if not allow_dirty and not git_is_clean():
        print(c("! Working tree is dirty. Commit first or pass --allow-dirty.", Colors.RED))
        return 2

    ensure_dirs()
    model = args.model or ROLLOUT.get("model")
    if args.dry_run:
        codex_cmd = ["codex", "exec", "-"]
    else:
        codex_cmd = resolve_codex_cmd(args.codex_cmd, model)
        print(c(f"codex cmd: {shlex.join(codex_cmd)}", Colors.DIM))

    selected_batch_ids = select_batch_ids(args, state)
    if not selected_batch_ids:
        print(c("All selected batches are already complete.", Colors.GREEN))
        return 0

    ensure_selection_ready(selected_batch_ids, state)

    print(c(f"Running {len(selected_batch_ids)} batch(es):", Colors.BOLD))
    for batch_id in selected_batch_ids:
        phase = PHASE_BY_BATCH_ID[batch_id]
        batch = BATCH_BY_ID[batch_id]
        print(f"  - {batch.id}  {batch.title}  ({phase.id})")

    for batch_id in selected_batch_ids:
        phase = PHASE_BY_BATCH_ID[batch_id]
        batch = BATCH_BY_ID[batch_id]
        banner = f"═══ {phase.id} / {batch.id} · {batch.title} ═══"
        print("\n" + c(banner, Colors.BOLD, Colors.BLUE))

        log_path = LOGS_DIR / f"{batch.id}.log"
        t0 = time.time()
        extra_notes: str | None = None
        attempt = 0

        if not args.dry_run:
            mark_batch(state, batch.id, "running")

        while True:
            rc, prompt_path, codex_output = invoke_codex(
                phase,
                batch,
                codex_cmd,
                log_path,
                args.dry_run,
                attempt=attempt,
                extra_notes=extra_notes,
            )
            elapsed = time.time() - t0

            if rc != 0:
                codex_failure = CodexFailure(
                    exit_code=rc,
                    output=truncate_output(codex_output or "(no output)"),
                )
                if attempt < max_fix_attempts:
                    attempt += 1
                    extra_notes = build_codex_retry_notes(batch, codex_failure, attempt)
                    print(c(f"↺ {batch.id} codex exited with {rc}, retrying ({attempt})", Colors.YELLOW))
                    continue
                if not args.dry_run:
                    mark_batch(
                        state,
                        batch.id,
                        "failed",
                        exit_code=rc,
                        reason="codex_failed",
                        log=display_path(log_path),
                        prompt=display_path(prompt_path),
                        codex_failure={
                            "exit_code": codex_failure.exit_code,
                            "output": codex_failure.output,
                        },
                    )
                print(c(f"✗ {batch.id} codex exited with {rc} ({elapsed:.0f}s)", Colors.RED))
                return rc

            if args.dry_run:
                print(c(f"◌ {batch.id} prompt generated ({elapsed:.0f}s)", Colors.CYAN))
                break

            verify_result = verify_batch(batch, log_path)
            if verify_result.ok:
                mark_batch(
                    state,
                    batch.id,
                    "done",
                    duration_sec=round(elapsed, 1),
                    log=display_path(log_path),
                    prompt=display_path(prompt_path),
                )
                print(c(f"✔ {batch.id} complete ({elapsed:.0f}s)", Colors.GREEN))
                if commit_per_batch:
                    git_commit_batch(batch)
                break

            if attempt >= max_fix_attempts:
                mark_batch(
                    state,
                    batch.id,
                    "failed",
                    reason="verify_failed",
                    log=display_path(log_path),
                    prompt=display_path(prompt_path),
                    verify_failures=[
                        {
                            "cmd": failure.cmd,
                            "exit_code": failure.exit_code,
                            "output": failure.output,
                        }
                        for failure in verify_result.failures
                    ],
                )
                print(c(f"✗ {batch.id} failed verification", Colors.RED))
                return 1

            attempt += 1
            extra_notes = build_verify_retry_notes(batch, verify_result, attempt)
            print(c(f"↺ {batch.id} verification failed, retrying ({attempt})", Colors.YELLOW))

    print("\n" + c("All selected batches completed.", Colors.BOLD, Colors.GREEN))
    return 0


if __name__ == "__main__":
    sys.exit(main())
