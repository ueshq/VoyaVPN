# shadcn-admin Reference Contract

Rollout: `voyavpn-shadcn-admin-style-alignment`  
Batch: `00-02-reference-contract`  
Purpose: define the shadcn-admin style and component sources that later batches should copy or adapt into VoyaVPN. The reference repository is read-only.

## Cited Sources

- Rollout spec: `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/spec.md`
- Rollout plan: `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/plan.md`
- Planning note: `/Users/afu/.claude/plans/refs-shadcn-admin-shimmying-dusk.md`
- Theme token source: `/Users/afu/Dev/refs/shadcn-admin/src/styles/theme.css`
- Index utility source: `/Users/afu/Dev/refs/shadcn-admin/src/styles/index.css`
- Component source directory: `/Users/afu/Dev/refs/shadcn-admin/src/components/ui`
- VoyaVPN shadcn config: `components.json`

## Theme Contract

Target file: `src/styles/globals.css`

- Use `/Users/afu/Dev/refs/shadcn-admin/src/styles/theme.css` as the token source for the neutral OKLch palette, `--radius: 0.625rem`, `:root`, `.dark`, popover tokens, chart tokens, sidebar tokens, `--font-inter`, `--font-manrope`, radius mappings, and `@theme inline` color mappings.
- Use `/Users/afu/Dev/refs/shadcn-admin/src/styles/index.css` as the index utility source for `@import 'tailwindcss'`, `@import 'tw-animate-css'`, `@custom-variant dark`, base border/outline/scrollbar defaults, Radix scroll-lock override, button cursor defaults, `no-scrollbar`, and `faded-bottom`.
- Keep VoyaVPN-specific CSS in the single target file instead of splitting `theme.css` and `index.css`.
- Preserve `@custom-variant rtl (&:where([dir="rtl"], [dir="rtl"] *));`.
- Preserve body font behavior driven by `--app-font-family` and `--app-font-size`.
- `--accent` and `--accent-foreground` remain as standard shadcn neutral hover tokens.
- The `data-accent` selector system is removed, including `:root[data-accent=...]`, `.dark[data-accent=...]`, and teal/blue/rose brand accent switching.

## Component Style Contract

Target directory: `src/components/ui`

`components.json` declares `style: "new-york"`, `baseColor: "neutral"`, `cssVariables: true`, aliases `@/components/ui` and `@/lib/utils`, and lucide icons. Later component work should keep those conventions.

Existing VoyaVPN primitives to rewrite or restyle:

| VoyaVPN target | Reference source | Contract |
| --- | --- | --- |
| `src/components/ui/button.tsx` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/button.tsx` | Copy/adapt new-york function-component style, `data-slot`, SVG sizing, and focus ring patterns. |
| `src/components/ui/dialog.tsx` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/dialog.tsx` | Copy/adapt new-york structure and `tw-animate-css` animation classes. |
| `src/components/ui/separator.tsx` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/separator.tsx` | Copy/adapt new-york function-component style and `data-slot`. |
| `src/components/ui/tabs.tsx` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/tabs.tsx` | Copy/adapt `TabsList` `bg-muted p-[3px] rounded-lg h-9 w-fit` and active trigger `bg-background shadow-sm`. |
| `src/components/ui/menubar.tsx` | No shadcn-admin menubar file | Menubar must be restyled in place, keep `@radix-ui/react-menubar`, and align focus, transition, SVG, and `data-slot` styling. Do not replace it with dropdown-menu. |
| `src/components/ui/toast.tsx` | No Radix toast reference; `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/sonner.tsx` is excluded | Toast remains Radix toast and does not migrate to sonner. Preserve the current toast public API and only align token styling. |

## Required Primitives

Add these primitives under `src/components/ui` using the matching source files from `/Users/afu/Dev/refs/shadcn-admin/src/components/ui`. These are required for visible feature migration.

| Primitive | Reference source | VoyaVPN target |
| --- | --- | --- |
| `input` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/input.tsx` | `src/components/ui/input.tsx` |
| `textarea` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/textarea.tsx` | `src/components/ui/textarea.tsx` |
| `label` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/label.tsx` | `src/components/ui/label.tsx` |
| `checkbox` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/checkbox.tsx` | `src/components/ui/checkbox.tsx` |
| `select` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/select.tsx` | `src/components/ui/select.tsx` |
| `switch` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/switch.tsx` | `src/components/ui/switch.tsx` |
| `card` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/card.tsx` | `src/components/ui/card.tsx` |
| `badge` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/badge.tsx` | `src/components/ui/badge.tsx` |
| `alert` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/alert.tsx` | `src/components/ui/alert.tsx` |
| `table` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/table.tsx` | `src/components/ui/table.tsx` |

## Optional Support Primitives

Add only when a later batch needs the support surface. Use the matching source files from `/Users/afu/Dev/refs/shadcn-admin/src/components/ui`.

| Primitive | Reference source | VoyaVPN target |
| --- | --- | --- |
| `scroll-area` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/scroll-area.tsx` | `src/components/ui/scroll-area.tsx` |
| `tooltip` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/tooltip.tsx` | `src/components/ui/tooltip.tsx` |
| `skeleton` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/skeleton.tsx` | `src/components/ui/skeleton.tsx` |
| `dropdown-menu` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/dropdown-menu.tsx` | `src/components/ui/dropdown-menu.tsx` |
| `popover` | `/Users/afu/Dev/refs/shadcn-admin/src/components/ui/popover.tsx` | `src/components/ui/popover.tsx` |

Excluded unless a later batch proves a need: `sidebar`, `command`, `calendar`, `sheet`, `form`, `avatar`, `alert-dialog`, `collapsible`, `radio-group`, `input-otp`, and `sonner`.

## Feature Surface Scope

Later migration batches cover these VoyaVPN feature surfaces while preserving behavior:

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

App shell targets:

- `src/components/app-shell/app-shell.tsx`
- `src/components/app-shell/modal-host.tsx`
- `src/components/app-shell/status-bar.tsx`
- `src/components/app-shell/toaster.tsx`

## VoyaVPN-Specific Exceptions

- Keep the VoyaVPN top tab shell and bottom status bar. Do not migrate to the shadcn-admin sidebar shell.
- Menubar must be restyled in place because shadcn-admin has no menubar primitive and VoyaVPN uses it for the tab shell.
- Toast remains Radix toast and does not migrate to sonner; `sonner.tsx` is not a source for VoyaVPN toast behavior.
- Keep TanStack Virtual role grids in `server-table.tsx`; do not replace them with semantic `Table`.
- Keep CodeMirror, QR canvas, logs list semantics, and hidden file inputs unless behavior is preserved.
- Remove only teal/blue/rose brand accent switching. Keep standard shadcn `--accent` tokens.
- Keep RTL support and dynamic `--app-font-size`.

## Evidence

- Contract file path: `.agents/rollouts/voyavpn-shadcn-admin-style-alignment/reference-contract.md`
- Source paths cited above define the theme token source, index utility source, `components/ui` reference set, VoyaVPN targets, and required exceptions for this rollout.
