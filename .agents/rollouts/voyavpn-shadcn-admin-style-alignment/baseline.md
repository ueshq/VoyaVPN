# VoyaVPN shadcn-admin Style Alignment Baseline

Batch: `00-01-current-inventory`
Generated from the current checkout on 2026-06-05.

## Inventory Method

`rg` was not available at the start of this batch, so the inventory uses `find` and `grep` fallback scans. A Homebrew install attempt started compiling large dependencies on this macOS target, so it was stopped; the official prebuilt `ripgrep` 15.1.0 binary was then installed to `/Users/afu/.local/bin/rg` only so the batch's required `rg -n ... baseline.md` verification command can run exactly.

Command evidence:

```sh
$ command -v rg
# exit 1, no output at batch start

$ find src/components/ui -maxdepth 1 -type f -name '*.tsx' -print | sort
src/components/ui/button.tsx
src/components/ui/dialog.tsx
src/components/ui/menubar.tsx
src/components/ui/separator.tsx
src/components/ui/tabs.tsx
src/components/ui/toast.tsx

$ find src/components/ui -maxdepth 1 -type f -print | wc -l
6
```

## Current UI Primitives

Current VoyaVPN primitive files:

- `src/components/ui/button.tsx`
- `src/components/ui/dialog.tsx`
- `src/components/ui/menubar.tsx`
- `src/components/ui/separator.tsx`
- `src/components/ui/tabs.tsx`
- `src/components/ui/toast.tsx`

Required missing primitives from the rollout scope:

```sh
$ zsh -lc 'for f in input textarea label checkbox select switch card badge alert table scroll-area tooltip skeleton dropdown-menu popover; do test -f src/components/ui/$f.tsx && printf "%s: present\n" $f || printf "%s: missing\n" $f; done'
input: missing
textarea: missing
label: missing
checkbox: missing
select: missing
switch: missing
card: missing
badge: missing
alert: missing
table: missing
scroll-area: missing
tooltip: missing
skeleton: missing
dropdown-menu: missing
popover: missing
```

Current package readiness:

```sh
$ node -e "const p=require('./package.json'); const names=['@radix-ui/react-label','@radix-ui/react-checkbox','@radix-ui/react-select','@radix-ui/react-switch','@radix-ui/react-tooltip','@radix-ui/react-scroll-area','@radix-ui/react-dropdown-menu','@radix-ui/react-popover','@radix-ui/react-slot','tw-animate-css']; for (const name of names) { const version=(p.dependencies&&p.dependencies[name])||(p.devDependencies&&p.devDependencies[name])||'MISSING'; console.log(name+': '+version); }"
@radix-ui/react-label: MISSING
@radix-ui/react-checkbox: MISSING
@radix-ui/react-select: MISSING
@radix-ui/react-switch: MISSING
@radix-ui/react-tooltip: MISSING
@radix-ui/react-scroll-area: MISSING
@radix-ui/react-dropdown-menu: MISSING
@radix-ui/react-popover: MISSING
@radix-ui/react-slot: ^1.2.4
tw-animate-css: MISSING
```

## Reference Contract Snapshot

Reference UI command:

```sh
$ find /Users/afu/Dev/refs/shadcn-admin/src/components/ui -maxdepth 1 -type f -name '*.tsx' -print | wc -l
30
```

Reference component files:

- `alert-dialog.tsx`
- `alert.tsx`
- `avatar.tsx`
- `badge.tsx`
- `button.tsx`
- `calendar.tsx`
- `card.tsx`
- `checkbox.tsx`
- `collapsible.tsx`
- `command.tsx`
- `dialog.tsx`
- `dropdown-menu.tsx`
- `form.tsx`
- `input-otp.tsx`
- `input.tsx`
- `label.tsx`
- `popover.tsx`
- `radio-group.tsx`
- `scroll-area.tsx`
- `select.tsx`
- `separator.tsx`
- `sheet.tsx`
- `sidebar.tsx`
- `skeleton.tsx`
- `sonner.tsx`
- `switch.tsx`
- `table.tsx`
- `tabs.tsx`
- `textarea.tsx`
- `tooltip.tsx`

Reference style/config facts from `components.json`, `theme.css`, and `index.css`:

- `components.json` is `style: "new-york"`, `baseColor: "neutral"`, CSS target `src/styles/globals.css`, alias `ui: "@/components/ui"`, and `iconLibrary: "lucide"`.
- `/Users/afu/Dev/refs/shadcn-admin/src/styles/theme.css` uses neutral OKLch tokens, `--radius: 0.625rem`, popover/chart/sidebar tokens, `--font-inter`, `--font-manrope`, and `--radius-xl`.
- `/Users/afu/Dev/refs/shadcn-admin/src/styles/index.css` imports `tailwindcss` and `tw-animate-css`, applies `border-border outline-ring/50`, thin scrollbars, Radix scroll-lock override, button cursor rules, `no-scrollbar`, and `faded-bottom`.
- Reference component checks: `grep -RIn 'forwardRef' /Users/afu/Dev/refs/shadcn-admin/src/components/ui | wc -l` returned `0`; `grep -RIn 'data-slot' /Users/afu/Dev/refs/shadcn-admin/src/components/ui | wc -l` returned `158`.

## feature surfaces

Command evidence:

```sh
$ find src/features -type f -name '*.tsx' -print | sort
src/features/backup/backup-dialog.tsx
src/features/clash/clash-connections-screen.tsx
src/features/clash/clash-proxies-screen.tsx
src/features/dns/dns-screen.tsx
src/features/groups/group-builder.tsx
src/features/logs/logs-screen.tsx
src/features/options/integration-settings.tsx
src/features/options/source-settings.tsx
src/features/profiles/profile-dialog.tsx
src/features/profiles/server-table.test.tsx
src/features/profiles/server-table.tsx
src/features/qr/qr-dialog.tsx
src/features/routing/routing-screen.tsx
src/features/subscriptions/import-profiles-dialog.tsx
src/features/subscriptions/subscriptions-dialog.tsx
src/features/updates/check-update-dialog.tsx

$ find src/features -type f -name '*.tsx' -print | wc -l
16
```

Runtime feature migration surfaces from the current tree:

- Normal primitive-heavy surfaces: `backup-dialog.tsx`, `group-builder.tsx`, `integration-settings.tsx`, `source-settings.tsx`, `profile-dialog.tsx`, `routing-screen.tsx`, `import-profiles-dialog.tsx`, `subscriptions-dialog.tsx`, `check-update-dialog.tsx`.
- Mixed normal/special surfaces: `dns-screen.tsx`, `server-table.tsx`, `qr-dialog.tsx`, `clash-connections-screen.tsx`, `clash-proxies-screen.tsx`, `logs-screen.tsx`.
- Test support surface in the 16-file inventory: `server-table.test.tsx`.

Shell surfaces scanned with features:

- `src/components/app-shell/app-shell.tsx`
- `src/components/app-shell/modal-host.tsx`
- `src/components/app-shell/status-bar.tsx`
- `src/components/app-shell/toaster.tsx`

## Raw Control Inventory

Raw visible and implementation-detail controls are still spread across features and shell.

```sh
$ grep -RInE '<input\b' src/features src/components/app-shell | wc -l
30

$ grep -RInE '<select\b' src/features src/components/app-shell | wc -l
6

$ grep -RIn -e 'type="checkbox"' -e "type='checkbox'" -e 'accent-primary' src/features src/components/app-shell src/App.test.tsx | wc -l
17
```

Raw `<input>` hotspots:

```text
4 src/features/routing/routing-screen.tsx
3 src/features/profiles/server-table.tsx
3 src/features/groups/group-builder.tsx
3 src/features/dns/dns-screen.tsx
3 src/features/backup/backup-dialog.tsx
2 src/features/updates/check-update-dialog.tsx
2 src/features/subscriptions/subscriptions-dialog.tsx
2 src/features/profiles/profile-dialog.tsx
2 src/features/options/source-settings.tsx
2 src/components/app-shell/modal-host.tsx
1 src/features/subscriptions/import-profiles-dialog.tsx
1 src/features/qr/qr-dialog.tsx
1 src/features/options/integration-settings.tsx
1 src/features/clash/clash-connections-screen.tsx
```

Raw `<select>` hotspots:

```text
1 src/features/subscriptions/import-profiles-dialog.tsx
1 src/features/routing/routing-screen.tsx
1 src/features/profiles/profile-dialog.tsx
1 src/features/groups/group-builder.tsx
1 src/features/dns/dns-screen.tsx
1 src/components/app-shell/modal-host.tsx
```

Checkbox and `accent-primary` hotspots:

```text
4 src/features/updates/check-update-dialog.tsx
4 src/features/profiles/server-table.tsx
2 src/features/subscriptions/subscriptions-dialog.tsx
2 src/features/routing/routing-screen.tsx
2 src/features/groups/group-builder.tsx
2 src/features/dns/dns-screen.tsx
1 src/features/profiles/profile-dialog.tsx
```

Notes:

- Hidden file inputs appear in `qr-dialog.tsx` and `import-profiles-dialog.tsx`; keep behavior and only restyle surrounding visible controls.
- `server-table.tsx` checkboxes should migrate visually, but the role-grid/virtualized structure should stay.

## Accent And Brand Remnant Inventory

Primary accent-system scan:

```sh
$ grep -RInE 'data-accent|setAccent|Accent' src/components src/stores src/styles src/App.test.tsx | wc -l
26
```

Hotspots:

```text
7 src/stores/preferences-store.ts
7 src/components/app-shell/app-shell.tsx
6 src/styles/globals.css
4 src/components/app-shell/modal-host.tsx
2 src/App.test.tsx
```

Broader brand-accent scan including `accent-primary`, config field names, generated bindings, and locale labels:

```sh
$ grep -RInE 'data-accent|setAccent|Accent|accent-primary|ColorPrimaryName' src | wc -l
55
```

Hotspots:

```text
8 src/stores/preferences-store.ts
8 src/components/app-shell/app-shell.tsx
6 src/styles/globals.css
6 src/App.test.tsx
4 src/components/app-shell/modal-host.tsx
2 src/ipc/bindings.ts
2 each src/i18n/locales/{de,en,fr,hu,ru,zh-Hans,zh-Hant}.json
2 src/features/updates/check-update-dialog.tsx
2 src/features/profiles/server-table.tsx
1 src/features/subscriptions/subscriptions-dialog.tsx
1 src/features/profiles/profile-dialog.tsx
1 src/features/groups/group-builder.tsx
```

Brand color scan:

```sh
$ grep -RInE '(teal|sky|rose)' src | wc -l
19

$ grep -RInE 'bg-(teal|sky|rose)-|text-(teal|sky|rose)-|border-(teal|sky|rose)-|ring-(teal|sky|rose)-|from-(teal|sky|rose)-|to-(teal|sky|rose)-|via-(teal|sky|rose)-' src | wc -l
3
```

The three Tailwind brand swatches are in `src/components/app-shell/modal-host.tsx`:

- `bg-teal-600`
- `bg-sky-600`
- `bg-rose-600`

Specific `data-accent` notes:

- `src/styles/globals.css` contains six brand accent token blocks: three `:root[data-accent=...]` and three `.dark[data-accent=...]`.
- `src/App.test.tsx` removes `data-accent` during setup and still asserts `data-accent="rose"`.
- `src/stores/preferences-store.ts`, `app-shell.tsx`, and `modal-host.tsx` still carry `Accent` / `setAccent` state and UI.
- `src/ipc/bindings.ts` still exposes generated `ColorPrimaryName`; later code should ignore that field defensively rather than edit generated bindings unless the owning generator is updated.

## forwardRef Inventory

Command evidence:

```sh
$ grep -RIn 'forwardRef' src/components/ui | wc -l
23
```

Per-file counts:

```text
9 src/components/ui/menubar.tsx
5 src/components/ui/toast.tsx
4 src/components/ui/dialog.tsx
3 src/components/ui/tabs.tsx
1 src/components/ui/button.tsx
1 src/components/ui/separator.tsx
```

All current `forwardRef` hits are inside the six existing primitives.

## Rounded And Card-Like Inventory

Broad ad hoc rounded scan:

```sh
$ grep -RIn 'rounded-' src/features src/components/app-shell src/components/ui | wc -l
93
```

Broad hotspots:

```text
12 src/features/groups/group-builder.tsx
7 src/features/profiles/server-table.tsx
7 src/components/ui/menubar.tsx
6 src/features/routing/routing-screen.tsx
6 src/features/dns/dns-screen.tsx
5 src/features/profiles/profile-dialog.tsx
5 src/features/clash/clash-proxies-screen.tsx
5 src/features/backup/backup-dialog.tsx
4 src/features/subscriptions/subscriptions-dialog.tsx
4 src/features/subscriptions/import-profiles-dialog.tsx
4 src/components/app-shell/modal-host.tsx
4 src/components/app-shell/app-shell.tsx
3 src/features/updates/check-update-dialog.tsx
3 src/features/qr/qr-dialog.tsx
3 src/features/clash/clash-connections-screen.tsx
3 src/components/ui/button.tsx
2 src/features/options/source-settings.tsx
2 src/features/options/integration-settings.tsx
2 src/features/logs/logs-screen.tsx
2 src/components/ui/toast.tsx
2 src/components/ui/dialog.tsx
1 src/components/ui/tabs.tsx
1 src/components/app-shell/status-bar.tsx
```

Narrow card-like scan for `rounded` plus `border`, `bg-card`, or `shadow`:

```sh
$ grep -RInE 'rounded-(md|lg|xl|2xl).*(border|bg-|shadow)|border.*rounded-(md|lg|xl|2xl)|bg-card|shadow' src/features src/components/app-shell src/components/ui | wc -l
80
```

Narrow hotspots:

```text
11 src/features/groups/group-builder.tsx
6 src/features/routing/routing-screen.tsx
6 src/features/profiles/profile-dialog.tsx
6 src/features/dns/dns-screen.tsx
5 src/features/profiles/server-table.tsx
5 src/features/backup/backup-dialog.tsx
5 src/components/app-shell/app-shell.tsx
4 src/features/updates/check-update-dialog.tsx
4 src/features/subscriptions/import-profiles-dialog.tsx
3 src/features/subscriptions/subscriptions-dialog.tsx
3 src/features/qr/qr-dialog.tsx
3 src/components/app-shell/modal-host.tsx
2 src/features/options/source-settings.tsx
2 src/features/options/integration-settings.tsx
2 src/features/clash/clash-proxies-screen.tsx
2 src/features/clash/clash-connections-screen.tsx
2 src/components/ui/toast.tsx
2 src/components/ui/menubar.tsx
2 src/components/ui/dialog.tsx
2 src/components/app-shell/status-bar.tsx
1 src/features/logs/logs-screen.tsx
```

Migration implication: many of these are field shells, badges, alerts, cards, table-like wrappers, or scroll containers and should migrate to shared primitives where behavior permits.

## special surfaces

These are not plain primitive swaps. Preserve structure and behavior, then align tokens and visible controls.

### CodeMirror JSON editor

Command:

```sh
$ grep -RInE 'CodeMirror|@codemirror|EditorView|json\(' src/features src/components/app-shell
```

Result summary:

- `src/features/dns/dns-screen.tsx:2` imports `@uiw/react-codemirror`.
- `src/features/dns/dns-screen.tsx:3` imports `@codemirror/lang-json`.
- `src/features/dns/dns-screen.tsx:14` defines `editorExtensions = [json()]`.
- `src/features/dns/dns-screen.tsx:395` renders `<CodeMirror ... />`.

Keep CodeMirror; only align the outer border/container and surrounding labels/errors.

### Server table virtual role grid

Command:

```sh
$ grep -RInE 'useVirtualizer|TanStack|virtual|role="grid"|role="row"|role="columnheader"|role="gridcell"' src/features/profiles/server-table.tsx
```

Result summary:

- `server-table.tsx:6` imports `useVirtualizer` from `@tanstack/react-virtual`.
- `server-table.tsx:226` documents TanStack Table ownership of row helpers.
- `server-table.tsx:235` creates `rowVirtualizer`.
- `server-table.tsx:472`, `478`, `494`, `594`, and related lines use role-based rows/column headers/cells.
- `server-table.tsx:556` currently highlights active rows with `bg-accent/70`.

Do not replace this with semantic `<Table>` if it breaks virtualization. Migrate visible search/checkbox/badge/alert affordances and retokenize row/header styling.

### Clash dense views

Command:

```sh
$ grep -RInE 'grid|overflow|Connection|Proxy|connections|proxies' src/features/clash/clash-connections-screen.tsx src/features/clash/clash-proxies-screen.tsx
```

Result summary:

- `clash-connections-screen.tsx` uses dense min-width CSS grids for connection headers and rows around lines `122-139`.
- `clash-proxies-screen.tsx` uses a two-column dense layout around line `161`, group scrolling around line `167`, and proxy-node grids around lines `240-258`.

Keep the dense grid/list structures. Use shared visible controls where practical and token-style the containers.

### QR import/generate surface

Command:

```sh
$ grep -RInE 'canvas|QRCode|qr|type="file"|accept=' src/features/qr/qr-dialog.tsx src/features/subscriptions/import-profiles-dialog.tsx src/features/backup/backup-dialog.tsx src/components/app-shell/modal-host.tsx
```

Result summary:

- `qr-dialog.tsx` currently renders generated QR output as an SVG data URL in an `<img>`.
- `qr-dialog.tsx:133-138` contains the hidden image file input used by the visible scan button.
- `qr-dialog.tsx:174` uses `BarcodeDetector` for QR scanning.
- `import-profiles-dialog.tsx:115` also has a file input.

Preserve hidden file input behavior and QR generation/scanning behavior. Migrate textareas/buttons/alerts around it as normal.

### Logs list

Command:

```sh
$ grep -RInE '<ol|<li|log|Log' src/features/logs/logs-screen.tsx
```

Result summary:

- `logs-screen.tsx:38` renders `<ol data-testid="log-lines">`.
- `logs-screen.tsx:40` renders each line as `<li>`.

Keep logs list semantics. Token-style the scroll area/list rows; do not convert to a table.

## Hotspot Summary For Later Batches

Highest priority primitive migration hotspots:

- `src/features/groups/group-builder.tsx`: raw input/select/checkbox, rounded cards/badges, scroll boxes.
- `src/features/routing/routing-screen.tsx`: most raw inputs, one raw select, badges/table-like rule editing, textarea.
- `src/features/profiles/server-table.tsx`: virtualized special surface plus search input, checkboxes, badges, context menu/card-like styling.
- `src/features/dns/dns-screen.tsx`: raw controls plus CodeMirror exception.
- `src/features/backup/backup-dialog.tsx`: repeated raw inputs and success/error alert blocks.
- `src/components/app-shell/app-shell.tsx` and `modal-host.tsx`: accent removal, shell token polish, raw setting controls.
- `src/styles/globals.css`: six `data-accent` blocks and old token system.
- `src/stores/preferences-store.ts`: `Accent` state, `setAccent`, and config conversion helpers.
- `src/App.test.tsx`: `data-accent` setup/assertions.
