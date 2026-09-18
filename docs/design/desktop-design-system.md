# Desktop design and interaction specification

Scope: the desktop app's pages, settings categories and dialogs. Colors are
vendored from Primer 11.9 (see [Primer mapping](primer-token-mapping.md)); add a
new color only as a role mapped to an existing Primer value.

Where things live:

- `packages/ui/src/styles/globals.css` — tokens, type, control sizes, radii, motion.
- `packages/ui/src/components/` — shared primitives.
- `apps/desktop/src/styles/home.css` (Home, node card surface, sidebar),
  `nodes.css` (node list, groups, policy groups), `shell.css` (window chrome).

## Foundations

| Role | Token / specification |
| --- | --- |
| Page canvas | `surface-canvas` — light `#f6f8fa`, dark same as `background` |
| Panels and cards | `card` / `surface-raised`, no border or shadow; they read against the canvas |
| Component fill | `background` — switch thumb, active tab or segment, toasts, chips |
| Overlays | `surface-overlay` with `shadow-overlay` (menus, dialogs, toasts, dragged rows) |
| Text | `foreground`, `muted-foreground` |
| Feedback | text on its own background: `success`/`success-bg`, `warning`/`warning-bg`, `danger`/`danger-bg` |
| Accent | `primary` is the only idle and interactive accent; `connected` only marks an achieved connection |
| Map land | `map-land` — light Primer neutral-4, dark neutral-3 |
| Page / dialog / section title | `text-page` 24/32, `text-dialog` 20/28, `text-section` 16/24 |
| Body / caption | 14/20, 12/16 |
| Control height | 36 default; 32 compact (toolbars, settings rows, list actions); 24 `xs` |
| Radius | 6 px controls, menus, toasts; 12 px panels and dialogs |
| Focus | primitives: `ring-2 ring-ring`; page CSS on plain buttons: 2 px `--ring` outline, 2 px offset |
| Motion | 150 ms feedback, 200 ms layout; reduced motion collapses animations and stops pulses |

Status is never color alone: every colored state also has text or an icon.
Normal and secondary text reach 4.5:1 on canvas, card and sidebar.

The saved theme is applied by `apps/desktop/public/theme-boot.js` before the
first paint. It is a file rather than an inline script because the app's CSP
allows scripts only from its own origin.

## Components

- **SegmentedControl** — a few exclusive choices that apply at once. It is a
  `group` of `aria-pressed` buttons that looks exactly like `TabsList` /
  `TabsTrigger`. Used for traffic mode, per-app mode, theme and language.
- **SwitchField** (`SettingsSwitch` in Settings) — on/off settings: label and
  description on the left, switch at the row's end. Settings save immediately;
  the subscription editor submits its automatic-update switch with the form.
- **FieldLayout `addon`** — controls under a field that fill it in (DNS presets).
- **Disclosure** — borderless summary and content, the same in panels and dialogs.
- **EmptyState** — icon, title, optional description; buttons go in `actions`;
  `iconClassName` spins a loader for waiting states.
- **Badge** — `default`, `secondary`, `outline`, `destructive`, `connected`,
  plus the quiet tints `warning`, `success`, `danger`.
- **Menubar `bare`** — a menu with one trigger button, without the bar's frame.
- **BrandMark** — the app icon glyph on a `primary` tile.
- **Dialogs** — fixed header, scrolling body, fixed footer. Cancel is outline,
  Save has no icon, title icons are `size-4`. Read-only details dialogs put
  Close in the footer instead of the corner button.

## Shell

- **Sidebar** — no brand at the top, only the collapse toggle: a 28 px button
  with a 16 px icon in half-strength foreground. On macOS the traffic lights sit
  16 px from the window's left and top edges, and the toggle starts at 88 px in
  the same 46 px row; the collapsed rail moves it just below that row. On
  Windows the toolbar starts below the 40 px caption band. Navigation: Home,
  Nodes (server icon), Rules, Network activity, Self-hosted node (radio tower
  icon), Settings. The selected item
  keeps its fill on hover. The footer shows the connection state; rates appear
  only while connected.
- **Page title** — the single `h1` on the left; actions on the right with the
  page's primary action last (Nodes "Add", Rules "Add rule"). In-page views use
  Tabs in the same slot (Network activity, Settings). The title row has equal
  16 px padding above and below, so it sits as far from the top (below the
  Windows caption band) as from the content. On macOS the row drags the window;
  its buttons and tabs stay clickable.

## Screens

**Self-hosted node**
- The title row is the `h1` alone: every control of this page lives in its
  content.
- The hosting card comes first and answers the page's two questions: is the
  node on, and can other devices reach it. Its static title labels the Host a
  node switch at the card's end; below it a dot and the state, with one
  sentence on what that means for other devices. A problem replaces that
  sentence in danger text, with an Open node settings button when the settings
  can fix it. Live connections and traffic appear under a hairline only while
  the node runs.
- The network check closes the card, under another hairline: the overall
  verdict (the better of the two address families) with its tint dot, the time
  of the check and a Check network / Check again button, then one line per
  family with its public address and reachability badge. Everything else shows
  only when it matters: the steps to take (each finding once, good news left
  out) and the local address to forward to when a peer cannot connect, a failed
  self-test, the Windows Firewall row with Allow, and the Cloudflare caveat when
  the verdict is reachable. The backend re-checks after start and every ten
  minutes, so the button is rarely needed.
- Two tiles follow in one row — Share links and Node settings. Each is a
  button on a raised surface with an icon, its name and a one-line summary of
  where things stand (whether links are ready, the enabled protocols), and
  opens a 40 rem dialog that ends in Done. An empty Share links dialog offers
  Check network in place, and the links appear as soon as an address is found.
- Share links lists one row per protocol and address: a header that selects
  it, then the link in a read-only field with Copy at its end. The selected
  link's QR code sits alongside; the first is selected on open, and using a
  row's field or Copy selects it too, so no second dialog is stacked. Switching
  keeps the previous code up, dimmed, until the next is drawn, so the dialog
  never changes size. The "only share with people you trust" warning sits here,
  where the decision is made. Reset keys is the footer's only extra action.
- Node settings saves on change. It shows Name, the two protocol switches and
  Fixed address; ports, the disguise site (`host` or `host:port` in one field)
  and the network switches sit under Advanced, which opens by itself when one
  of its fields is rejected or a port is in use. An empty field means "use the
  default", and its placeholder names that default. Restore defaults in the
  footer asks first and leaves hosting and the keys alone.
- Reachability badges and dots use the quiet status tints: success for tested
  reachable, warning for probably reachable or needs forwarding, danger for
  unreachable, secondary when unknown.

**Home**
- The disc is the action: an idle `primary` ring on a card, green fill with glow
  when connected, a warning ring while cleanup is pending, and a plus-shaped
  Add node action when there are no nodes. Empty Home has this single entry;
  it opens Nodes with the Add menu expanded and its first option focused.
- A status line under the disc (`role="status"`) says where the connection
  stands; with no nodes it shows how to add one instead.
- The centered reading order is connection state, capture/routing summary,
  current node, then metrics, with 24 px between the main sections.
- Capture names the runtime backend or effective system proxy when connected.
  Routing is labeled as a saved setting because existing IPC has no live read;
  pending settings remain explicit. Both summaries link to their editing page.
- Last tested latency, connection time and exit IP sit in one strip. Exit IP
  distinguishes Not checked, Checking and Failed with a Check/Retry action;
  checking is still opt-in.
- View logs on a failed connection opens Advanced, scrolls the runtime log
  heading into view, focuses it and briefly highlights the section.
- The world map is a theme-colored mask. It marks the exit country: a green
  pulse while connected (checked exit IP, then the measured country, then the
  node name's flag) and a hollow ring for the node that would be used.

**Nodes**
- Subscription and manual groups are continuous panels; their header actions
  are ghost buttons that become icons in a container narrower than 700 px.
- Rows are about 56 px: a 32 px flag or globe, the name with its state badge
  ("In use" green, "Selected" neutral), then address and protocol.
- Latency carries a dot: under 150 ms good, slower reachable nodes warning,
  failed tests danger. Untested has its own text.
- Each row always offers Connect while disconnected, Switch during a
  connection, or In use for the running node. The name opens details and has
  a stable detail tooltip; other operations remain in More.
- Search matches node names, addresses and source names, shows result counts,
  and temporarily expands matching groups. Clearing restores saved folds.
- A flag emoji in a node name is a provisional country; a measured one wins.

**Rules**
- The title holds the traffic mode, More and Add rule. More contains the full
  Restore default rules label with its confirmation; empty rules offer a text
  entry too. There is one active rule set and no configuration list.
- Rule rows have switches and drag handles. A missing target shows a warning
  badge, a blocking outbound a danger badge. Clicking a rule name edits it.
- Global mode locks every rule control.

**Network activity**
- Live connections and Policy groups tabs.
- A route column colors where traffic went: proxy with the node name (blue),
  direct (neutral), blocked (red).
- Connection details are grouped (what, route, addresses, traffic) with
  monospaced addresses.

**Settings**
- Tabs: General, Connection (with DNS), Advanced (with the runtime log), Updates.
- Connection starts with Traffic capture, then Kill switch, Local proxy and
  DNS. VPN-only platforms explain the available mode without an inert choice.
  Tunnel stack, MTU and other specialist parameters stay in Advanced.
- Groups divide one setting per row; theme and language are segmented controls;
  DNS presets sit under their field. Short number fields are 144 px wide and
  common icon actions have a 32 px target.
- The save status aligns with the content, and loading shows skeleton groups.
- Updates lead with the current version and show a state badge only when the
  updater is not ready.

## Product behavior

Home's Switch node goes to Nodes, where Connect or Switch connects. The node page shows
subscription groups, manual groups, then unassigned nodes. Only
source-maintained node parameters are read-only; see
[ADR 0008](../adr/0008-manual-node-groups.md).

Subscription creation uses one source editor: name, URL, automatic update, then
advanced source options. Add and update persists the source first; a failed
fetch leaves the source and draft available for retry. Text, file, clipboard
and QR imports create manual nodes and do not connect.
New subscriptions default to automatic updates off; turning it on starts with
one hour. Existing sources reflect `enabled && interval > 0`; turning it off
hides the interval and saves `enabled: false` with a null interval. Required
name/URL and invalid intervals show feedback on blur or submit, and submit
focuses the first invalid field. Import and subscription results remain on
Nodes, where the user explicitly connects after adding a node.

Settings retain their tab across navigation. Switches and choices save
immediately; text saves after validation on blur or Enter. Saving, saved and
failure feedback stays visible, and a failed draft survives leaving the page.
Connection settings and DNS save without restarting: `SettingsApplication` in
`voya-app` records applied snapshots, `get_settings_apply_status` returns none,
reconnect or reapplyProxy, and `apply_pending_settings` applies a captured
configuration. Disconnected settings apply on the next connection.

## Review evidence

Playwright specs write current screenshots to `apps/desktop/test-results/`:
`design-unification` (every page at 960, 1180 and 1440, light and dark, three
languages), `home-design`, `profile-cards`, `unified-nodes`, `settings-layout`,
`network-activity` and `window-chrome`. Pre-redesign "before" screenshots are
kept out of the repository, under the untracked `.agents/docs/design/`.

## Verification

2026-09-14 visual design round, six batches (3bed5c8, 2fdd1d9, e19cf4e,
c981d82, 2cb4ea9 and the batch 6 commit). Each batch passed typecheck, lint,
the unit suite with the coverage policy, the dead-code and i18n checks, and the
Playwright renderer smoke run; batch 6 also passed the bundle budgets.

Final `pnpm run verify:local` on 4f11a13 passed every gate: architecture,
lockfile, Rust formatting, Clippy and tests, frontend typecheck, 1047 unit tests
with the coverage policy, lint, bundle budgets, 84 Playwright renderer tests,
dead code, sing-box config acceptance, IPC binding drift and i18n.
