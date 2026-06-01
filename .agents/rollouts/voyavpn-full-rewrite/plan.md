# VoyaVPN Full Rewrite Implementation Plan

This plan converts the v2rayN rewrite brief into resumable Codex batches. Prose explains the delivery shape; the YAML block is the executable source for `rollout.py`.

The runner is allowed to modify only the VoyaVPN target repo. The v2rayN repo is a read-only oracle for behavior, structure, tests, and UI parity.

## Milestones

- M0: baseline evidence and source inventory.
- M1: scaffold, typed IPC, DB, profiles, and a navigable shell.
- M2: imports, subscriptions, Xray and sing-box config generation, and golden parity gates.
- M3: first usable internal alpha with connect, logs, proxy, tray, and stats.
- M4: routing, DNS, TUN, policy groups, proxy chains, and regional presets.
- M5: Clash, speedtest, downloads, updates, geo, and rulesets.
- M6: backup, WebDAV, autostart, hotkeys, QR, i18n, theming, accessibility, and smoke tests.
- M7: packaging, release workflow, and final regression evidence.

## Manual Or External Checkpoints

- Signing identities, notarization credentials, updater private keys, and release publishing credentials are outside the automated runner.
- Real OS smoke testing on Windows, Linux, and macOS is documented by the runner but must be executed on actual machines.
- GPL and AGPL core redistribution decisions require human legal approval; the automated plan defaults to download-on-first-run.

<!-- rollout-plan:start -->

```yaml
rollout:
  name: 'voyavpn-full-rewrite'
  repo_root: '/Users/afu/Dev/refs/VoyaVPN'
  workdir: '.agents/rollouts/voyavpn-full-rewrite/logs'
  codex_cmd: null
  model: null
  max_fix_attempts: 1
  allow_dirty: true
  commit_per_batch: false
  sources_of_truth:
    - '.agents/rollouts/voyavpn-full-rewrite/spec.md'
    - '.agents/rollouts/voyavpn-full-rewrite/plan.md'
    - '/Users/afu/.claude/plans/typescript-shadcn-tauri-silly-marble.md'
    - '/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib'
    - '/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib.Tests'
    - '/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN'
    - '/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN.Desktop'
  planning_notes:
    - 'This is a greenfield rewrite in VoyaVPN; the v2rayN repo is read-only reference material.'
    - 'Deliver subsystem by subsystem with backend, frontend, tests, and IPC wiring in the same slice when feasible.'
    - 'Keep all three platforms in scope from the first scaffold.'
    - 'Fresh SQLite schema only; no migration from v2rayN data and no obsolete columns.'
  success_metrics:
    - 'Rust workspace tests pass with cargo test --workspace --all-targets.'
    - 'Frontend checks pass with pnpm typecheck, pnpm test -- --run, and pnpm lint.'
    - 'Generated bindings have no drift after regeneration.'
    - 'Xray and sing-box generated configs match v2rayN golden fixtures and pass core acceptance where binaries exist.'
    - 'A real server can connect through pnpm tauri dev with logs, stats, and traffic flow.'
  global_context:
    - 'Target stack: Tauri 2, Rust, React, TypeScript, Tailwind v4, shadcn/ui, Zustand, TanStack Query, TanStack Table, Radix, i18next, sqlx, specta, tauri-specta.'
    - 'Rust crate layout: voya-core, voya-db, voya-platform, voya-net, voya-udptest, voya-app, and src-tauri.'
    - 'Frontend IPC rule: only src/ipc may import @tauri-apps/api; all app code uses typed wrappers.'
    - 'Config generator correctness is judged by generated core JSON and core acceptance, not entity snapshots alone.'
  hard_rules:
    - 'Do not modify /Users/afu/Dev/refs/v2rayN/v2rayN or sibling reference sources.'
    - 'Do not add obsolete v2rayN columns or data migration code.'
    - 'Do not place OS-specific code in voya-core.'
    - 'Do not hand-write TypeScript IPC DTOs that should be generated from Rust.'
    - 'Do not import @tauri-apps/api outside src/ipc.'
    - 'Do not redistribute GPL or AGPL core binaries in installers by default.'
    - 'Keep diffs focused on the current batch and update tests or docs for the touched surface.'
  batch_prompt_suffix:
    - 'Finish only this batch and the minimum supporting work required for its verification commands.'
    - 'Read the source-of-truth files before changing behavior that must match v2rayN.'
    - 'Capture any skipped external checks in docs with a concrete reason and follow-up.'
phases:
  - id: '00-baseline'
    title: 'Baseline And Evidence'
    goal: 'Establish source inventory, architecture decisions, and verification scaffolding before implementation starts.'
    depends_on: []
    summary: 'This phase creates the human evidence needed to keep a full rewrite aligned with the reference app.'
    entry_criteria:
      - 'The target VoyaVPN repo exists and may be empty.'
      - 'The v2rayN reference repo is available read-only.'
    exit_criteria:
      - 'Reference source areas and high-risk parity points are documented.'
      - 'Architecture and verification decisions are captured in docs.'
    risks:
      - 'Missing source inventory can cause later batches to silently drift from v2rayN behavior.'
    batches:
      - id: '00-01-baseline-inventory'
        title: 'Baseline Source Inventory'
        kind: 'analysis'
        execution: 'codex'
        goal: 'Create a source inventory that maps v2rayN systems, UI screens, tests, and fidelity hot spots to VoyaVPN target modules.'
        depends_on: []
        deliverables:
          - 'docs/source-inventory.md with backend, frontend, model, parser, config-gen, runtime, platform, and test references.'
          - 'docs/fidelity-hotspots.md covering sudo lifecycle, finalmask, policy groups, proxy chains, DNS, stats, system proxy, TUN, Clash PATCH, and QR scope.'
        acceptance:
          - 'Every subsystem from S0 through S19 in the planning source is represented.'
          - 'Reference paths point to concrete v2rayN files or directories.'
        evidence_to_capture:
          - 'Inventory docs committed in the target repo tree.'
        verify_commands:
          - 'test -f docs/source-inventory.md'
          - 'test -f docs/fidelity-hotspots.md'
        files_to_touch:
          - 'docs/source-inventory.md'
          - 'docs/fidelity-hotspots.md'
        prompt_context:
          - 'Use rg and rg --files against /Users/afu/Dev/refs/v2rayN/v2rayN for evidence.'
      - id: '00-02-architecture-adrs'
        title: 'Architecture ADRs'
        kind: 'docs'
        execution: 'codex'
        goal: 'Record the core architecture decisions that all implementation batches must preserve.'
        depends_on:
          - '00-01-baseline-inventory'
        deliverables:
          - 'docs/adr/0001-target-architecture.md.'
          - 'docs/adr/0002-typed-ipc-and-events.md.'
          - 'docs/adr/0003-config-generation-parity.md.'
          - 'docs/adr/0004-platform-boundaries.md.'
        acceptance:
          - 'ADRs capture crate boundaries, generated bindings, event channels, DB stance, platform isolation, and golden testing.'
          - 'ADRs explicitly state no legacy migration and no obsolete columns.'
        evidence_to_capture:
          - 'ADR index or docs references are present.'
        verify_commands:
          - 'test -f docs/adr/0001-target-architecture.md'
          - 'test -f docs/adr/0002-typed-ipc-and-events.md'
          - 'test -f docs/adr/0003-config-generation-parity.md'
          - 'test -f docs/adr/0004-platform-boundaries.md'
        files_to_touch:
          - 'docs/adr/**'
        prompt_context:
          - 'Keep ADRs short but decisive so later Codex batches can rely on them.'
      - id: '00-03-verification-scaffold-plan'
        title: 'Verification Scaffold Plan'
        kind: 'docs'
        execution: 'codex'
        goal: 'Create the verification map for unit, golden, IPC drift, frontend, platform, and packaging checks.'
        depends_on:
          - '00-02-architecture-adrs'
        deliverables:
          - 'docs/verification/strategy.md.'
          - 'tests/golden/README.md explaining golden fixture shape and canonicalization.'
          - 'docs/verification/manual-os-smoke.md for checks that require real OS machines.'
        acceptance:
          - 'The strategy defines local deterministic checks and separate manual evidence.'
          - 'Golden tests assert on generated configs, not only entity snapshots.'
        evidence_to_capture:
          - 'Verification docs and initial tests/golden directory exist.'
        verify_commands:
          - 'test -f docs/verification/strategy.md'
          - 'test -f tests/golden/README.md'
          - 'test -f docs/verification/manual-os-smoke.md'
        files_to_touch:
          - 'docs/verification/**'
          - 'tests/golden/**'
        prompt_context:
          - 'Do not require external core binaries in this batch; document how later checks discover or skip them.'
  - id: '01-foundation'
    title: 'Workspace, Shell, IPC, DB'
    goal: 'Create the Rust, Tauri, React, typed IPC, data, event, and CI foundation.'
    depends_on:
      - '00-baseline'
    summary: 'This phase establishes the repo shape that all subsystem work builds on.'
    entry_criteria:
      - 'Baseline docs and ADRs exist.'
    exit_criteria:
      - 'Workspace compiles, frontend checks run, generated IPC exists, DB migrations exist, and CI covers baseline checks.'
    risks:
      - 'Bad early boundaries can force expensive refactors during config-gen or platform work.'
    batches:
      - id: '01-01-workspace-scaffold'
        title: 'Workspace Scaffold'
        kind: 'code'
        execution: 'codex'
        goal: 'Create the initial Rust workspace, Tauri app, frontend package, crate directories, formatting, linting, and baseline scripts.'
        depends_on: []
        deliverables:
          - 'Cargo workspace with src-tauri and crates/voya-* packages.'
          - 'Vite React TypeScript app with Tailwind v4 and shadcn/ui foundations.'
          - 'package.json, pnpm lockfile, tsconfig, rustfmt, clippy, and README baseline.'
        acceptance:
          - 'Cargo metadata resolves.'
          - 'Frontend package scripts exist for typecheck, test, lint, bindings, and tauri build.'
          - 'No app behavior beyond a minimal shell is required yet.'
        evidence_to_capture:
          - 'Workspace metadata and package scripts are present.'
        verify_commands:
          - 'test -f Cargo.toml'
          - 'test -f package.json'
          - 'test -f src-tauri/Cargo.toml'
          - 'test -f crates/voya-core/Cargo.toml'
          - 'cargo metadata --format-version 1 --no-deps'
        files_to_touch:
          - 'Cargo.toml'
          - 'crates/**'
          - 'src-tauri/**'
          - 'src/**'
          - 'package.json'
          - 'pnpm-lock.yaml'
        prompt_context:
          - 'Use workspace dependencies conservatively and keep the repo green after scaffold.'
      - id: '01-02-app-shell-design-system'
        title: 'App Shell And Design System'
        kind: 'code'
        execution: 'codex'
        goal: 'Build the navigable empty app shell with menubar, tabs, status bar, modal host, toaster, i18n, RTL-ready theme, and static tray.'
        depends_on:
          - '01-01-workspace-scaffold'
        deliverables:
          - 'React AppShell components and stores.'
          - 'shadcn/ui base components used by the shell.'
          - 'i18next setup with initial locale files.'
          - 'Theme and accent persistence stubs.'
          - 'Static Rust tray menu.'
        acceptance:
          - 'The first screen is the usable app shell, not a landing page.'
          - 'Tabs for Profiles, Clash Proxies, Clash Connections, and Logs exist even if empty.'
          - 'RTL locale plumbing is present.'
        evidence_to_capture:
          - 'Frontend smoke test or component test for AppShell.'
        verify_commands:
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
        files_to_touch:
          - 'src/**'
          - 'src-tauri/**'
        prompt_context:
          - 'Keep UI quiet and operational; avoid marketing-style hero content.'
      - id: '01-03-typed-ipc-events'
        title: 'Typed IPC And Event Bridge'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement specta and tauri-specta export, generated bindings, typed command wrappers, and the three-channel event bridge.'
        depends_on:
          - '01-02-app-shell-design-system'
        deliverables:
          - 'Rust command and event type scaffolding.'
          - 'Generated src/ipc/bindings.ts.'
          - 'src/ipc command wrappers and EventBridge.'
          - 'bindings drift check script.'
        acceptance:
          - 'A demo command round-trips through generated types.'
          - 'Only src/ipc imports @tauri-apps/api.'
          - 'EventBridge routes invalidate, transient stream, and imperative app events.'
        evidence_to_capture:
          - 'bindings drift command output documented in docs/verification/bindings.md.'
        verify_commands:
          - 'pnpm bindings:check'
          - 'pnpm typecheck'
          - 'rg "@tauri-apps/api" src | rg -v "^src/ipc/" -q && exit 1 || exit 0'
        files_to_touch:
          - 'src-tauri/**'
          - 'src/ipc/**'
          - 'src/**'
          - 'docs/verification/bindings.md'
        prompt_context:
          - 'The generated binding file is allowed to be checked in and must be regenerated by script.'
      - id: '01-04-data-models-db-config'
        title: 'Data Models, DB, Config'
        kind: 'code'
        execution: 'codex'
        goal: 'Port live model shapes, fresh SQLite schema, repositories, JSON config defaults, and typed blob boundary.'
        depends_on:
          - '01-03-typed-ipc-events'
        deliverables:
          - 'voya-core models and enums with serde and specta derives.'
          - 'voya-db migrations, repositories, and typed JSON blob helpers.'
          - 'AppConfig defaults and load/save commands.'
          - 'Unit and integration tests for DB defaults and persistence.'
        acceptance:
          - 'Obsolete columns are absent.'
          - 'Enum discriminants match the planning source.'
          - 'Settings persist across process restart in tests.'
        evidence_to_capture:
          - 'docs/verification/db-schema.md with schema notes.'
        verify_commands:
          - 'cargo test -p voya-core --all-targets'
          - 'cargo test -p voya-db --all-targets'
          - 'pnpm bindings:check'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'crates/voya-db/**'
          - 'src-tauri/**'
          - 'src/ipc/**'
          - 'docs/verification/db-schema.md'
        prompt_context:
          - 'ProtocolExtraItem and TransportExtraItem stay typed across IPC and become TEXT only inside voya-db blob helpers.'
      - id: '01-05-ci-baseline'
        title: 'CI Baseline'
        kind: 'code'
        execution: 'codex'
        goal: 'Add CI workflows for Rust, frontend, generated binding drift, and baseline formatting or lint checks.'
        depends_on:
          - '01-04-data-models-db-config'
        deliverables:
          - '.github/workflows/ci.yml.'
          - 'Scripts for local verification parity with CI.'
          - 'README instructions for local setup.'
        acceptance:
          - 'CI commands are non-interactive and mirror local scripts.'
          - 'Binding drift is a first-class check.'
        evidence_to_capture:
          - 'docs/verification/ci.md documenting the baseline.'
        verify_commands:
          - 'cargo test --workspace --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'pnpm bindings:check'
        files_to_touch:
          - '.github/workflows/**'
          - 'README.md'
          - 'docs/verification/ci.md'
          - 'package.json'
        prompt_context:
          - 'Keep CI broad enough to catch drift but avoid requiring signing credentials or real core binaries.'
  - id: '02-profiles-imports'
    title: 'Profiles, Parsers, Subscriptions'
    goal: 'Deliver persisted profiles, server table, protocol dialogs, share links, imports, and subscription flows.'
    depends_on:
      - '01-foundation'
    summary: 'This phase turns the empty shell into a profile manager with real data and import paths.'
    entry_criteria:
      - 'Typed IPC, data models, DB, and app shell are available.'
    exit_criteria:
      - 'Users can create, edit, import, view, sort, dedupe, and persist profiles and subscriptions.'
    risks:
      - 'Parser edge cases can corrupt later config generation if not tested early.'
    batches:
      - id: '02-01-profile-crud-managers'
        title: 'Profile CRUD Managers'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement profile CRUD, ordering, active selection, dedupe, copy, move, grouping, and ProfileEx manager behavior.'
        depends_on: []
        deliverables:
          - 'voya-app profile manager modules.'
          - 'DB repositories and IPC commands for profile operations.'
          - 'Invalidation events for profile changes.'
          - 'Rust tests for CRUD, reorder, defaulting, dedupe, and active selection.'
        acceptance:
          - 'All live profile types can be persisted through typed commands.'
          - 'Operations update state and emit expected invalidation events.'
        evidence_to_capture:
          - 'docs/verification/profile-crud.md.'
        verify_commands:
          - 'cargo test -p voya-app profile --all-targets'
          - 'cargo test -p voya-db profile --all-targets'
          - 'pnpm bindings:check'
        files_to_touch:
          - 'crates/voya-app/**'
          - 'crates/voya-db/**'
          - 'crates/voya-core/**'
          - 'src-tauri/**'
          - 'docs/verification/profile-crud.md'
        prompt_context:
          - 'Use ConfigHandler behavior as the reference but decompose it into cohesive Rust modules.'
      - id: '02-02-server-table-dialogs'
        title: 'Server Table And Profile Dialogs'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement the virtualized server table and per-protocol add/edit dialogs wired to real profile IPC.'
        depends_on:
          - '02-01-profile-crud-managers'
        deliverables:
          - 'ServerTable with virtualization, columns, multi-select, context menu, drag reorder, filter, and active marker.'
          - 'Add/Edit profile dialogs with react-hook-form and zod discriminated unions.'
          - 'Protocol, transport, security, and mux panels.'
          - 'Frontend tests for table and form behavior.'
        acceptance:
          - 'Every supported protocol has a form path.'
          - '5k row table scenario remains virtualized in tests or documented perf harness.'
          - 'Create, edit, delete, copy, move, sort, and activate use real IPC.'
        evidence_to_capture:
          - 'docs/verification/server-table.md.'
        verify_commands:
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
        files_to_touch:
          - 'src/features/profiles/**'
          - 'src/components/**'
          - 'src/stores/**'
          - 'docs/verification/server-table.md'
        prompt_context:
          - 'Use icons and compact operational controls; keep columns stable and responsive.'
      - id: '02-03-share-link-parsers'
        title: 'Share Link Parsers'
        kind: 'code'
        execution: 'codex'
        goal: 'Port share-link parse and export behavior for all supported protocols with round-trip and negative tests.'
        depends_on:
          - '02-01-profile-crud-managers'
        deliverables:
          - 'ShareFmt trait and protocol implementations in voya-core.'
          - 'Base query and stream codec handling transport, security, pqv, ech, pcs, and fm parameters.'
          - 'Inner v2rayn format and full JSON custom import helpers.'
          - 'Property, round-trip, and negative parser tests.'
        acceptance:
          - 'Protocols covered: vmess, vless, trojan, ss, hysteria2, tuic, wireguard, anytls, naive, socks, and inner format.'
          - 'Malformed inputs return typed errors and never panic.'
        evidence_to_capture:
          - 'docs/verification/share-links.md with parity notes against ServiceLib.Tests/Fmt.'
        verify_commands:
          - 'cargo test -p voya-core fmt --all-targets'
          - 'cargo test -p voya-core share --all-targets'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'docs/verification/share-links.md'
        prompt_context:
          - 'Read ServiceLib/Handler/Fmt and ServiceLib.Tests/Fmt before implementing each protocol.'
      - id: '02-04-import-subscriptions'
        title: 'Imports And Subscriptions'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement manual import flows, subscription management, update engine, filters, dedupe, conversion target, and scheduler.'
        depends_on:
          - '02-03-share-link-parsers'
          - '02-02-server-table-dialogs'
        deliverables:
          - 'voya-net download client with proxy-to-direct fallback.'
          - 'Subscription manager in voya-app.'
          - 'Subscription UI dialogs and update actions.'
          - 'Clipboard, file, and JSON import flows where locally testable.'
          - 'Tests for base64, multi-URL, filter, dedupe, UA, and conversion target behavior.'
        acceptance:
          - 'A real or fixture subscription imports, deduplicates, persists, and invalidates profiles.'
          - 'Auto-update scheduler can be started and stopped deterministically in tests.'
        evidence_to_capture:
          - 'docs/verification/subscriptions.md.'
        verify_commands:
          - 'cargo test -p voya-net --all-targets'
          - 'cargo test -p voya-app subscription --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
        files_to_touch:
          - 'crates/voya-net/**'
          - 'crates/voya-app/**'
          - 'src/features/subscriptions/**'
          - 'src-tauri/**'
          - 'docs/verification/subscriptions.md'
        prompt_context:
          - 'Regex filtering and dedupe belong in the manager decomposition, not only the raw subscription download layer.'
      - id: '02-05-profile-phase-gate'
        title: 'Profile Phase Gate'
        kind: 'verification'
        execution: 'codex'
        goal: 'Stabilize profile, parser, subscription, table, and IPC behavior before config generation starts.'
        depends_on:
          - '02-04-import-subscriptions'
        deliverables:
          - 'docs/verification/m1-profile-gate.md with command results and any deferred edge cases.'
          - 'Additional tests or fixes needed to make the phase green.'
        acceptance:
          - 'Workspace checks pass for the profile and import surface.'
          - 'Deferred items are explicit and do not block config generation.'
        evidence_to_capture:
          - 'M1 gate report.'
        verify_commands:
          - 'cargo test --workspace --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'pnpm bindings:check'
          - 'test -f docs/verification/m1-profile-gate.md'
        files_to_touch:
          - 'docs/verification/m1-profile-gate.md'
          - 'crates/**'
          - 'src/**'
          - 'src-tauri/**'
        prompt_context:
          - 'Fix only profile, parser, subscription, IPC, and table issues found by the gate.'
  - id: '03-config-generation'
    title: 'Xray And Sing-Box Config Generation'
    goal: 'Port deterministic config generation for Xray and sing-box with golden parity and core acceptance gates.'
    depends_on:
      - '02-profiles-imports'
    summary: 'This is the highest-fidelity phase and should move one resolver or generator area at a time.'
    entry_criteria:
      - 'Profiles, parser models, DB, and typed IPC are stable.'
    exit_criteria:
      - 'Xray and sing-box golden parity is established for the fixture matrix.'
    risks:
      - 'Small serialization differences can produce valid but behaviorally wrong configs.'
      - 'Policy group, proxy chain, DNS, finalmask, and TUN behavior are especially easy to drift.'
    batches:
      - id: '03-01-context-builder'
        title: 'Core Config Context Builder'
        kind: 'code'
        execution: 'codex'
        goal: 'Port cross-entity context resolution shared by Xray and sing-box generation.'
        depends_on: []
        deliverables:
          - 'CoreGenEnv trait and deterministic context builder in voya-core.'
          - 'Resolution for active node, pre-socks contexts, groups, chains, sub-level virtual proxy chains, per-rule outbounds, protect domains, and template inputs.'
          - 'Cycle detection, dedupe, and ECH SNI extraction tests.'
        acceptance:
          - 'Context builder is OS-free and deterministic.'
          - 'Main context disables TUN when building pre-socks as specified.'
        evidence_to_capture:
          - 'docs/verification/context-builder.md.'
        verify_commands:
          - 'cargo test -p voya-core context --all-targets'
          - 'test -f docs/verification/context-builder.md'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'docs/verification/context-builder.md'
        prompt_context:
          - 'Use Handler/Builder/CoreConfigContextBuilder.cs as the primary reference.'
      - id: '03-02-xray-coregen-outbounds'
        title: 'Xray Outbounds And Streams'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement Xray outbound generation for protocols, transports, security, mux, proxy chains, policy groups, and finalmask.'
        depends_on:
          - '03-01-context-builder'
        deliverables:
          - 'Xray serde config models.'
          - 'Outbound generation for live protocols and transports.'
          - 'TLS, reality, ECH, uTLS fingerprint, ALPN, pinned cert, and finalmask composition.'
          - 'PolicyGroup balancer and observatory behavior.'
          - 'ProxyChain dialerProxy behavior including xhttp rewrite.'
        acceptance:
          - 'Golden unit fixtures cover the highest-risk outbound combinations.'
          - 'serde casing and skip behavior match Xray expectations.'
        evidence_to_capture:
          - 'docs/verification/xray-outbounds.md.'
        verify_commands:
          - 'cargo test -p voya-core xray_outbound --all-targets'
          - 'cargo test -p voya-core policy_group --all-targets'
          - 'test -f docs/verification/xray-outbounds.md'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'tests/golden/**'
          - 'docs/verification/xray-outbounds.md'
        prompt_context:
          - 'Model finalmask as the merge target, including tcp and udp noise behavior and documented precedence.'
      - id: '03-03-xray-coregen-routing-dns'
        title: 'Xray Inbounds, Routing, DNS, Templates'
        kind: 'code'
        execution: 'codex'
        goal: 'Complete Xray generation for inbounds, DNS, routing, stats, TUN inbound, dokodemo API, and full config templates.'
        depends_on:
          - '03-02-xray-coregen-outbounds'
        deliverables:
          - 'Xray inbound, DNS, routing, stats, log, and template services.'
          - 'SOCKS variants, LAN auth, sniffing, second port, dokodemo API, and Xray TUN inbound.'
          - 'FullConfigTemplate support for proxy-only, proxy-detour, and TunConfig.'
          - 'Xray generator tests and canonical snapshots.'
        acceptance:
          - 'Minimal and advanced DNS/routing fixtures match the reference behavior.'
          - 'Xray TUN inbound is generated when TUN is enabled.'
        evidence_to_capture:
          - 'docs/verification/xray-configgen.md.'
        verify_commands:
          - 'cargo test -p voya-core xray --all-targets'
          - 'test -f docs/verification/xray-configgen.md'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'tests/golden/**'
          - 'docs/verification/xray-configgen.md'
        prompt_context:
          - 'DNS must include fakeip, expected IPs, hosts, strategies, custom DNS override, and final-DNS direct auto-detect behavior.'
      - id: '03-04-singbox-coregen-outbounds'
        title: 'Sing-Box Outbounds And Selectors'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement sing-box outbound generation for protocols, transports, selectors, urltest, mux, detour, and clash API basics.'
        depends_on:
          - '03-01-context-builder'
        deliverables:
          - 'sing-box serde config models with snake_case and strict skip rules.'
          - 'Outbound and transport generation for live protocol matrix.'
          - 'Selector and urltest policy group generation.'
          - 'ProxyChain detour behavior.'
          - 'Experimental clash_api and cache_file scaffolding.'
        acceptance:
          - 'Generated JSON avoids null or unknown fields rejected by sing-box.'
          - 'Selector ordering and dedupe match policy group expectations.'
        evidence_to_capture:
          - 'docs/verification/singbox-outbounds.md.'
        verify_commands:
          - 'cargo test -p voya-core singbox_outbound --all-targets'
          - 'cargo test -p voya-core singbox_selector --all-targets'
          - 'test -f docs/verification/singbox-outbounds.md'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'tests/golden/**'
          - 'docs/verification/singbox-outbounds.md'
        prompt_context:
          - 'Keep sing-box strict serialization in mind; unknown or null fields can fail check -c.'
      - id: '03-05-singbox-coregen-routing-dns'
        title: 'Sing-Box Inbounds, Routing, DNS, Rulesets'
        kind: 'code'
        execution: 'codex'
        goal: 'Complete sing-box generation for inbounds, typed DNS server schema, routing, fakeip, rulesets, templates, and TUN config.'
        depends_on:
          - '03-04-singbox-coregen-outbounds'
        deliverables:
          - 'Sing-box inbound, DNS, routing, ruleset, stats, log, and template services.'
          - 'Typed Server4Sbox and Rule4Sbox schema.'
          - 'Fakeip, predefined hosts, independent cache, rule_set, and TUN template support.'
          - 'Generator tests and canonical snapshots.'
        acceptance:
          - 'DNS uses the new typed-server schema with domain_resolver and predefined hosts as required.'
          - 'Fakeip and TUN fixtures are covered.'
        evidence_to_capture:
          - 'docs/verification/singbox-configgen.md.'
        verify_commands:
          - 'cargo test -p voya-core singbox --all-targets'
          - 'test -f docs/verification/singbox-configgen.md'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'tests/golden/**'
          - 'docs/verification/singbox-configgen.md'
        prompt_context:
          - 'ParseDnsAddress and final DNS behavior must follow the reference plan and source.'
      - id: '03-06-golden-export-harness'
        title: 'Golden Export Harness'
        kind: 'code'
        execution: 'codex'
        goal: 'Create the v2rayN golden export path, canonical diff tooling, and optional core acceptance checks.'
        depends_on:
          - '03-03-xray-coregen-routing-dns'
          - '03-05-singbox-coregen-routing-dns'
        deliverables:
          - 'Golden fixture loader and canonicalizer in Rust tests.'
          - 'Documented C# export harness path or helper scripts that do not modify the reference repo unless explicitly copied.'
          - 'Optional xray run -test and sing-box check -c checks that skip clearly when binaries are missing.'
          - 'Golden report document.'
        acceptance:
          - 'Fixture matrix is represented and can grow incrementally.'
          - 'Golden failures produce actionable diffs.'
          - 'Missing external binaries do not make local unit tests fail without a clear opt-in.'
        evidence_to_capture:
          - 'docs/verification/golden-report.md.'
        verify_commands:
          - 'cargo test -p voya-core golden --all-targets'
          - 'test -f docs/verification/golden-report.md'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'tests/golden/**'
          - 'docs/verification/golden-report.md'
          - 'scripts/**'
        prompt_context:
          - 'Do not write into /Users/afu/Dev/refs/v2rayN/v2rayN; if helper code is needed, copy or document it in VoyaVPN.'
      - id: '03-07-configgen-phase-gate'
        title: 'Config Generation Phase Gate'
        kind: 'verification'
        execution: 'codex'
        goal: 'Stabilize context builder, Xray, sing-box, templates, golden tests, and generated IPC after config generation.'
        depends_on:
          - '03-06-golden-export-harness'
        deliverables:
          - 'docs/verification/m2-configgen-gate.md.'
          - 'Fixes required for config generation tests and drift checks.'
        acceptance:
          - 'Config-generation relevant tests pass.'
          - 'Golden report lists coverage and known gaps.'
        evidence_to_capture:
          - 'M2 config generation gate report.'
        verify_commands:
          - 'cargo test -p voya-core --all-targets'
          - 'cargo test --workspace --all-targets'
          - 'pnpm bindings:check'
          - 'test -f docs/verification/m2-configgen-gate.md'
        files_to_touch:
          - 'docs/verification/m2-configgen-gate.md'
          - 'crates/voya-core/**'
          - 'tests/golden/**'
        prompt_context:
          - 'Prioritize behavioral parity fixes over expanding unrelated app features.'
  - id: '04-runtime-alpha'
    title: 'Runtime Alpha'
    goal: 'Deliver connect, disconnect, core process supervision, logs, system proxy, tray, and statistics.'
    depends_on:
      - '03-config-generation'
    summary: 'This phase turns generated configs into a usable internal alpha.'
    entry_criteria:
      - 'Xray and sing-box configs can be generated from persisted profiles.'
    exit_criteria:
      - 'A real server can connect, traffic flows, logs stream, proxy mode toggles, and speed is visible.'
    risks:
      - 'Privilege, process tree, and route cleanup bugs can leave the host in a bad state.'
    batches:
      - id: '04-01-coreinfo-process-model'
        title: 'Core Info And Process Model'
        kind: 'code'
        execution: 'codex'
        goal: 'Port the 15-core launch table, executable discovery, per-core arguments, env vars, and app/bin/log path resolution.'
        depends_on: []
        deliverables:
          - 'voya-platform app dir, bin dir, log dir, temp dir, and portable-mode detection.'
          - 'voya-app or voya-platform CoreInfo table for all supported cores.'
          - 'Executable discovery and chmod behavior on Unix.'
          - 'Tests for command templates and OS path behavior.'
        acceptance:
          - 'CoreInfo covers xray, v2fly variants, mihomo, hysteria, naiveproxy, tuic, sing-box, juicity, brook, overtls, shadowquic, mieru, and v2rayN core entries.'
          - 'Argument substitution and env vars match the reference plan.'
        evidence_to_capture:
          - 'docs/verification/coreinfo.md.'
        verify_commands:
          - 'cargo test -p voya-platform coreinfo --all-targets'
          - 'cargo test -p voya-app coreinfo --all-targets'
          - 'test -f docs/verification/coreinfo.md'
        files_to_touch:
          - 'crates/voya-platform/**'
          - 'crates/voya-app/**'
          - 'docs/verification/coreinfo.md'
        prompt_context:
          - 'Pay attention to mihomo executable probe order and env vars for v2fly, xray, and mieru.'
      - id: '04-02-supervisor-elevation'
        title: 'Supervisor And Elevation'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement the actor-owned core supervisor, dual-process lifecycle, sudo or UAC primitives, Windows job handling, and teardown order.'
        depends_on:
          - '04-01-coreinfo-process-model'
        deliverables:
          - 'Supervisor actor with serialized start, stop, restart, and crash handling.'
          - 'Dual-process main and pre core lifecycle.'
          - 'Unix sudo password storage using Zeroizing and request-response collection primitives.'
          - 'Windows job object and TUN cleanup abstractions.'
          - 'Tests with fake process runner.'
        acceptance:
          - 'Teardown order is sudo kill, main, pre.'
          - 'Sudo password is collected once when TUN is enabled and read synchronously at spawn.'
          - 'Linux and macOS share sudo -S behavior except kill script name.'
        evidence_to_capture:
          - 'docs/verification/supervisor.md.'
        verify_commands:
          - 'cargo test -p voya-platform process --all-targets'
          - 'cargo test -p voya-app supervisor --all-targets'
          - 'test -f docs/verification/supervisor.md'
        files_to_touch:
          - 'crates/voya-platform/**'
          - 'crates/voya-app/**'
          - 'src-tauri/**'
          - 'docs/verification/supervisor.md'
        prompt_context:
          - 'Use fake process runners for deterministic tests; leave real OS smoke for later docs.'
      - id: '04-03-connect-logs-ui'
        title: 'Connect, Disconnect, Logs UI'
        kind: 'code'
        execution: 'codex'
        goal: 'Wire connect and disconnect commands, config generation, supervisor start, status events, log streaming, and UI controls.'
        depends_on:
          - '04-02-supervisor-elevation'
        deliverables:
          - 'Tauri commands for connect, disconnect, restart, and status.'
          - 'Log event streaming and Logs tab.'
          - 'Status bar connect controls and core state display.'
          - 'Sudo prompt modal for the collection primitive.'
          - 'Integration tests using fake generated configs or fake process runner.'
        acceptance:
          - 'Connecting an active profile writes config files and starts the supervisor path.'
          - 'Logs stream to the Logs tab through transient events.'
          - 'Disconnect updates core state and cleans generated runtime state.'
        evidence_to_capture:
          - 'docs/verification/runtime-alpha.md.'
        verify_commands:
          - 'cargo test -p voya-app supervisor --all-targets'
          - 'cargo test --workspace --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/runtime-alpha.md'
        files_to_touch:
          - 'crates/voya-app/**'
          - 'src-tauri/**'
          - 'src/features/logs/**'
          - 'src/features/status/**'
          - 'src/ipc/**'
          - 'docs/verification/runtime-alpha.md'
        prompt_context:
          - 'Real network traffic can be a documented smoke step; automated tests should use fakes.'
      - id: '04-04-system-proxy-tray'
        title: 'System Proxy, PAC, Tray'
        kind: 'code'
        execution: 'codex'
        goal: 'Port per-OS system proxy behavior, Windows-only PAC server, proxy mode switching, and dynamic tray menu.'
        depends_on:
          - '04-03-connect-logs-ui'
        deliverables:
          - 'voya-platform sysproxy modules for Windows, Linux, and macOS.'
          - 'PAC manager gated to Windows.'
          - 'Proxy mode commands and status events.'
          - 'Dynamic tray menu with recent servers and mode submenu.'
          - 'UI status bar segmented control with PAC hidden off Windows.'
        acceptance:
          - 'Forced change, forced clear, unchanged, and PAC modes are represented.'
          - 'Advanced template substitutes both http_port and socks_port to the single SOCKS port and prepends local exceptions.'
          - 'Exit and disconnect restore behavior is documented and testable with fakes.'
        evidence_to_capture:
          - 'docs/verification/system-proxy.md.'
        verify_commands:
          - 'cargo test -p voya-platform sysproxy --all-targets'
          - 'cargo test -p voya-app sysproxy --all-targets'
          - 'pnpm typecheck'
          - 'test -f docs/verification/system-proxy.md'
        files_to_touch:
          - 'crates/voya-platform/**'
          - 'crates/voya-app/**'
          - 'src-tauri/**'
          - 'src/features/status/**'
          - 'docs/verification/system-proxy.md'
        prompt_context:
          - 'PAC HTTP server is Windows-only; do not expose PAC as enabled on Linux or macOS.'
      - id: '04-05-statistics-speed'
        title: 'Statistics And Speed Columns'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement Xray and sing-box stat services, coalesced speed events, persistent per-server traffic, and UI speed columns.'
        depends_on:
          - '04-03-connect-logs-ui'
        deliverables:
          - 'Xray debug vars polling service.'
          - 'sing-box traffic WebSocket service.'
          - 'ServerStatItem persistence with date rollover, orphan cleanup, and clone behavior.'
          - 'Status bar live speed and server table traffic columns.'
          - 'Tests for stat parsing, active-server keying, and rollover.'
        acceptance:
          - 'Both stat services run concurrently and no-op unless their core is active.'
          - 'Xray uses StatePort and sing-box uses StatePort2.'
          - 'Display speed can sum proxy and direct while persistent traffic keys to the active server.'
        evidence_to_capture:
          - 'docs/verification/statistics.md.'
        verify_commands:
          - 'cargo test -p voya-app statistics --all-targets'
          - 'cargo test -p voya-db statistics --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/statistics.md'
        files_to_touch:
          - 'crates/voya-app/**'
          - 'crates/voya-db/**'
          - 'src/features/profiles/**'
          - 'src/features/status/**'
          - 'docs/verification/statistics.md'
        prompt_context:
          - 'Coalesce UI events around 1 Hz and keep the stats hot path lightweight.'
      - id: '04-06-alpha-phase-gate'
        title: 'Runtime Alpha Phase Gate'
        kind: 'verification'
        execution: 'codex'
        goal: 'Stabilize first usable internal alpha behavior and document real-server smoke steps.'
        depends_on:
          - '04-04-system-proxy-tray'
          - '04-05-statistics-speed'
        deliverables:
          - 'docs/verification/m3-runtime-alpha-gate.md.'
          - 'Fixes needed for runtime, proxy, logs, stats, and frontend checks.'
        acceptance:
          - 'Automated workspace checks pass.'
          - 'Manual real-server smoke steps are precise enough to execute.'
        evidence_to_capture:
          - 'M3 runtime alpha gate report.'
        verify_commands:
          - 'cargo test --workspace --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'pnpm bindings:check'
          - 'test -f docs/verification/m3-runtime-alpha-gate.md'
        files_to_touch:
          - 'docs/verification/m3-runtime-alpha-gate.md'
          - 'crates/**'
          - 'src/**'
          - 'src-tauri/**'
        prompt_context:
          - 'Do not require actual network credentials in automated checks.'
  - id: '05-routing-dns-tun-groups'
    title: 'Routing, DNS, TUN, Groups'
    goal: 'Complete routing settings, DNS settings, TUN polish, policy group UI, proxy chain UI, and regional presets.'
    depends_on:
      - '04-runtime-alpha'
    summary: 'This phase deepens runtime control and exposes advanced generator features through the UI.'
    entry_criteria:
      - 'Runtime alpha can connect and show state through real IPC.'
    exit_criteria:
      - 'Routing, DNS, TUN, policy groups, proxy chains, and presets work in both generators and UI.'
    risks:
      - 'Advanced generator UI can diverge from backend structures without typed forms and tests.'
    batches:
      - id: '05-01-routing-settings-editor'
        title: 'Routing Settings And Rule Editor'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement routing config CRUD, rule list, rule editor, templates, domain strategies, and reconnect integration.'
        depends_on: []
        deliverables:
          - 'Routing repositories, managers, IPC commands, and invalidation events.'
          - 'Routing screen, rule list, and rule editor dialogs.'
          - 'Template fetch path from RouteRulesTemplateSourceUrl.'
          - 'Tests for active routing selection and rule serialization into both generators.'
        acceptance:
          - 'Users can create, edit, activate, and delete routing profiles and rules.'
          - 'Reconnect picks up active routing changes.'
        evidence_to_capture:
          - 'docs/verification/routing.md.'
        verify_commands:
          - 'cargo test -p voya-app routing --all-targets'
          - 'cargo test -p voya-core routing --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/routing.md'
        files_to_touch:
          - 'crates/voya-app/**'
          - 'crates/voya-core/**'
          - 'src/features/routing/**'
          - 'src-tauri/**'
          - 'docs/verification/routing.md'
        prompt_context:
          - 'Routing changes must feed both Xray and sing-box config generation.'
      - id: '05-02-dns-settings'
        title: 'DNS Settings'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement simple and per-core advanced DNS settings, custom DNS JSON editors, fakeip controls, and generator integration.'
        depends_on:
          - '05-01-routing-settings-editor'
        deliverables:
          - 'DNS repositories, managers, IPC commands, and UI.'
          - 'CodeMirror JSON editors for advanced per-core DNS.'
          - 'Validation and tests for fakeip, expected IPs, hosts, bootstrap, serve stale, strategy, and raw DNS override behavior.'
        acceptance:
          - 'DNS settings persist and regenerate configs for both Xray and sing-box.'
          - 'Invalid JSON returns typed errors mapped to UI forms.'
        evidence_to_capture:
          - 'docs/verification/dns.md.'
        verify_commands:
          - 'cargo test -p voya-app dns --all-targets'
          - 'cargo test -p voya-core dns --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/dns.md'
        files_to_touch:
          - 'crates/voya-app/**'
          - 'crates/voya-core/**'
          - 'src/features/dns/**'
          - 'docs/verification/dns.md'
        prompt_context:
          - 'Keep sing-box DNS typed schema and Xray DNS behavior distinct but driven by shared settings.'
      - id: '05-03-tun-mode-polish'
        title: 'TUN Mode Polish'
        kind: 'code'
        execution: 'codex'
        goal: 'Complete TUN lifecycle, elevation UX, driver preflight, route restoration notes, and UI controls across platforms.'
        depends_on:
          - '05-02-dns-settings'
        deliverables:
          - 'TUN manager and commands in voya-app and voya-platform.'
          - 'Status bar TUN toggle and sudo prompt UX.'
          - 'Preflight checks and restore-on-disconnect behavior.'
          - 'Tests with fake platform adapters and docs for manual OS smoke.'
        acceptance:
          - 'AllowEnableTun on Unix is tied to non-empty stored sudo password.'
          - 'sing-box and mihomo sudo wrapping and Xray TUN inbound remain distinct paths.'
          - 'No orphan elevated process expectation is documented and tested with fakes.'
        evidence_to_capture:
          - 'docs/verification/tun.md.'
        verify_commands:
          - 'cargo test -p voya-platform tun --all-targets'
          - 'cargo test -p voya-app tun --all-targets'
          - 'pnpm typecheck'
          - 'test -f docs/verification/tun.md'
        files_to_touch:
          - 'crates/voya-platform/**'
          - 'crates/voya-app/**'
          - 'src/features/status/**'
          - 'src/features/tun/**'
          - 'docs/verification/tun.md'
        prompt_context:
          - 'Do not prompt for sudo per spawn; collect once at TUN-enable.'
      - id: '05-04-policy-groups-chains-ui'
        title: 'Policy Groups And Proxy Chains UI'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement policy group and proxy chain builders, child pickers, previews, validation, and generator-backed tests.'
        depends_on:
          - '05-03-tun-mode-polish'
        deliverables:
          - 'Group and chain data commands and UI dialogs.'
          - 'Server picker modal flow for nested selection.'
          - 'Preview of selector, urltest, dialerProxy, and detour output.'
          - 'Golden tests for mixed-child groups and two-hop or three-hop chains.'
        acceptance:
          - 'Mixed-child group and 2 or 3 hop proxy chain route correctly in generated configs.'
          - 'Cycle detection prevents invalid chain or group structures.'
        evidence_to_capture:
          - 'docs/verification/groups-chains.md.'
        verify_commands:
          - 'cargo test -p voya-core proxy_chain --all-targets'
          - 'cargo test -p voya-core policy_group --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/groups-chains.md'
        files_to_touch:
          - 'crates/voya-core/**'
          - 'crates/voya-app/**'
          - 'src/features/groups/**'
          - 'docs/verification/groups-chains.md'
        prompt_context:
          - 'Config generation support already exists from phase 03; this batch exposes and validates it through UI.'
      - id: '05-05-regional-presets'
        title: 'Regional Presets'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement Russia and Iran regional preset application with external DNS template fetch, routing and DNS writes, and fallback behavior.'
        depends_on:
          - '05-02-dns-settings'
        deliverables:
          - 'Preset manager using voya-net.'
          - 'Preset UI actions and confirmation flow.'
          - 'Tests for successful fetch, null fallback, routing write, DNS write, and simple DNS behavior.'
        acceptance:
          - 'Preset apply fetches DNS templates through configured sources when available.'
          - 'Fallback enables custom DNS when network template data is unavailable.'
        evidence_to_capture:
          - 'docs/verification/regional-presets.md.'
        verify_commands:
          - 'cargo test -p voya-app preset --all-targets'
          - 'cargo test -p voya-net --all-targets'
          - 'pnpm typecheck'
          - 'test -f docs/verification/regional-presets.md'
        files_to_touch:
          - 'crates/voya-app/**'
          - 'crates/voya-net/**'
          - 'src/features/options/**'
          - 'docs/verification/regional-presets.md'
        prompt_context:
          - 'Regional presets depend on voya-net and are not a static local-only settings write.'
      - id: '05-06-advanced-routing-phase-gate'
        title: 'Advanced Routing Phase Gate'
        kind: 'verification'
        execution: 'codex'
        goal: 'Stabilize routing, DNS, TUN, groups, chains, and presets before service integrations.'
        depends_on:
          - '05-04-policy-groups-chains-ui'
          - '05-05-regional-presets'
        deliverables:
          - 'docs/verification/m4-routing-dns-tun-gate.md.'
          - 'Fixes needed for advanced routing checks.'
        acceptance:
          - 'Advanced generator and UI checks pass.'
          - 'Manual TUN OS smoke steps remain documented separately.'
        evidence_to_capture:
          - 'M4 advanced routing gate report.'
        verify_commands:
          - 'cargo test --workspace --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'pnpm bindings:check'
          - 'test -f docs/verification/m4-routing-dns-tun-gate.md'
        files_to_touch:
          - 'docs/verification/m4-routing-dns-tun-gate.md'
          - 'crates/**'
          - 'src/**'
        prompt_context:
          - 'Keep fixes scoped to this phase surface.'
  - id: '06-service-integrations'
    title: 'Clash, Speedtest, Updates'
    goal: 'Complete Clash API, speedtest, downloads, updates, ruleset, and geo acquisition workflows.'
    depends_on:
      - '05-routing-dns-tun-groups'
    summary: 'This phase adds operational services around a working proxy runtime.'
    entry_criteria:
      - 'Runtime, routing, DNS, TUN, and groups are functional.'
    exit_criteria:
      - 'Clash, speedtest, downloads, updates, rulesets, and geo acquisition are implemented and tested.'
    risks:
      - 'Network-dependent behavior can make tests flaky unless clients are injectable and fixture-driven.'
    batches:
      - id: '06-01-clash-api-ui'
        title: 'Clash API And UI'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement Clash REST and WebSocket clients plus proxies and connections screens.'
        depends_on: []
        deliverables:
          - 'voya-net Clash REST and WebSocket client.'
          - 'voya-app Clash manager and commands.'
          - 'Clash Proxies and Clash Connections tabs.'
          - 'Delay test, select active, connection monitor, close connection, rule-mode PATCH, and reload force behavior.'
        acceptance:
          - 'Rule-mode switch uses HTTP PATCH on /configs.'
          - 'Reload uses /configs?force=true.'
          - 'WebSocket traffic and connection events update UI stores.'
        evidence_to_capture:
          - 'docs/verification/clash.md.'
        verify_commands:
          - 'cargo test -p voya-net clash --all-targets'
          - 'cargo test -p voya-app clash --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/clash.md'
        files_to_touch:
          - 'crates/voya-net/**'
          - 'crates/voya-app/**'
          - 'src/features/clash/**'
          - 'docs/verification/clash.md'
        prompt_context:
          - 'Use mocked HTTP and WebSocket clients in automated tests.'
      - id: '06-02-speedtest-udptest'
        title: 'Speedtest And UDP Tests'
        kind: 'code'
        execution: 'codex'
        goal: 'Port all six speed actions, UDP associate channel, NTP, DNS, STUN, MCBE testers, cancellation, and UI result writing.'
        depends_on:
          - '06-01-clash-api-ui'
        deliverables:
          - 'voya-udptest crate implementation and tests.'
          - 'Speedtest manager and commands.'
          - 'UI actions and result display in server table.'
          - 'ProfileExItem delay, speed, message, and ipinfo updates.'
        acceptance:
          - 'ESpeedActionType covers Tcping, Realping, UdpTest, Speedtest, Mixedtest, and FastRealping.'
          - 'Mixedtest combines realping, speedtest, and UDP as expected.'
          - 'Cancel stops active jobs.'
        evidence_to_capture:
          - 'docs/verification/speedtest.md.'
        verify_commands:
          - 'cargo test -p voya-udptest --all-targets'
          - 'cargo test -p voya-app speedtest --all-targets'
          - 'pnpm typecheck'
          - 'test -f docs/verification/speedtest.md'
        files_to_touch:
          - 'crates/voya-udptest/**'
          - 'crates/voya-app/**'
          - 'src/features/profiles/**'
          - 'docs/verification/speedtest.md'
        prompt_context:
          - 'Network-heavy tests should use local fixtures or mocked sockets where possible.'
      - id: '06-03-downloads-updates'
        title: 'Downloads And Updates'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement download service, app and core update checks, asset templating, pre-release toggle, and safe binary swap workflows.'
        depends_on:
          - '06-02-speedtest-udptest'
        deliverables:
          - 'voya-net download and GitHub release clients.'
          - 'Update manager for app, cores, geo, and srs.'
          - 'Check Update UI.'
          - 'Tests for OS and arch asset selection, fallback, and version parsing.'
        acceptance:
          - 'Proxy-to-direct fallback is implemented.'
          - 'Core asset templating covers the supported core matrix and architectures.'
          - 'Download-on-first-run stance is preserved for GPL or AGPL cores.'
        evidence_to_capture:
          - 'docs/verification/updates.md.'
        verify_commands:
          - 'cargo test -p voya-net update --all-targets'
          - 'cargo test -p voya-app update --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/updates.md'
        files_to_touch:
          - 'crates/voya-net/**'
          - 'crates/voya-app/**'
          - 'src/features/updates/**'
          - 'docs/verification/updates.md'
        prompt_context:
          - 'Do not require live GitHub network access in unit tests; use fixture releases.'
      - id: '06-04-ruleset-geo'
        title: 'Ruleset And Geo Acquisition'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement acquisition, update, validation, and configuration integration for geo dat files and sing-box srs rulesets.'
        depends_on:
          - '06-03-downloads-updates'
        deliverables:
          - 'Ruleset and geo clients in voya-net.'
          - 'Manager commands and UI controls for sources.'
          - 'Integration with routing and DNS generation where needed.'
          - 'Tests using fixture archives and manifest data.'
        acceptance:
          - 'Geo and ruleset updates can run through proxy-to-direct fallback.'
          - 'Generated configs reference acquired assets consistently.'
        evidence_to_capture:
          - 'docs/verification/ruleset-geo.md.'
        verify_commands:
          - 'cargo test -p voya-net ruleset --all-targets'
          - 'cargo test -p voya-app ruleset --all-targets'
          - 'cargo test -p voya-core ruleset --all-targets'
          - 'test -f docs/verification/ruleset-geo.md'
        files_to_touch:
          - 'crates/voya-net/**'
          - 'crates/voya-app/**'
          - 'crates/voya-core/**'
          - 'src/features/options/**'
          - 'docs/verification/ruleset-geo.md'
        prompt_context:
          - 'Keep acquisition separate from config generation; generation consumes resolved local asset paths.'
      - id: '06-05-services-phase-gate'
        title: 'Service Integrations Phase Gate'
        kind: 'verification'
        execution: 'codex'
        goal: 'Stabilize Clash, speedtest, updates, ruleset, geo, and related UI workflows.'
        depends_on:
          - '06-04-ruleset-geo'
        deliverables:
          - 'docs/verification/m5-services-gate.md.'
          - 'Fixes required for service integration checks.'
        acceptance:
          - 'Automated service integration tests pass without live network dependency.'
          - 'Manual live-network smoke steps are documented.'
        evidence_to_capture:
          - 'M5 services gate report.'
        verify_commands:
          - 'cargo test --workspace --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'pnpm bindings:check'
          - 'test -f docs/verification/m5-services-gate.md'
        files_to_touch:
          - 'docs/verification/m5-services-gate.md'
          - 'crates/**'
          - 'src/**'
        prompt_context:
          - 'Do not hide real-network requirements in automated tests.'
  - id: '07-polish-backup-i18n'
    title: 'Backup, Integrations, I18n, Polish'
    goal: 'Complete backup, WebDAV, autostart, hotkeys, QR, i18n, theming, accessibility, performance, and smoke automation.'
    depends_on:
      - '06-service-integrations'
    summary: 'This phase closes user-facing breadth and quality gates before packaging.'
    entry_criteria:
      - 'Major runtime and service workflows are implemented.'
    exit_criteria:
      - 'The UI and integration surface is complete, localized, accessible, and smoke-tested where automatable.'
    risks:
      - 'Polish work can sprawl; each batch must stay tied to specific workflows and checks.'
    batches:
      - id: '07-01-backup-webdav'
        title: 'Backup, Restore, WebDAV'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement local backup, restore, WebDAV push and pull, zip handling, and Backup UI.'
        depends_on: []
        deliverables:
          - 'Backup manager and commands.'
          - 'WebDAV client using reqwest and quick-xml.'
          - 'Backup and Restore screen.'
          - 'Tests for local round trip and fixture WebDAV XML.'
        acceptance:
          - 'Local backup restores into a clean temp app state in tests.'
          - 'WebDAV PROPFIND, upload, download, and delete behavior is fixture-tested.'
        evidence_to_capture:
          - 'docs/verification/backup-webdav.md.'
        verify_commands:
          - 'cargo test -p voya-net webdav --all-targets'
          - 'cargo test -p voya-app backup --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/backup-webdav.md'
        files_to_touch:
          - 'crates/voya-net/**'
          - 'crates/voya-app/**'
          - 'src/features/backup/**'
          - 'docs/verification/backup-webdav.md'
        prompt_context:
          - 'Avoid live WebDAV in tests; fixture the XML and HTTP responses.'
      - id: '07-02-autostart-hotkeys-qr'
        title: 'Autostart, Hotkeys, QR'
        kind: 'code'
        execution: 'codex'
        goal: 'Implement per-OS autostart, global hotkeys, QR generation, QR scan frontend path, and related settings UI.'
        depends_on:
          - '07-01-backup-webdav'
        deliverables:
          - 'Autostart platform adapters and commands.'
          - 'Global hotkey registration for show window and four proxy mode actions.'
          - 'Backend QR generation command.'
          - 'Frontend QR generate and scan UI hooks.'
          - 'Tests with fake platform adapters.'
        acceptance:
          - 'QR generation is backend; scanning remains frontend or platform scoped.'
          - 'Hotkey actions are represented by the five EGlobalHotkey actions.'
          - 'Autostart artifacts are documented per OS.'
        evidence_to_capture:
          - 'docs/verification/autostart-hotkeys-qr.md.'
        verify_commands:
          - 'cargo test -p voya-platform autostart --all-targets'
          - 'cargo test -p voya-app hotkey --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'test -f docs/verification/autostart-hotkeys-qr.md'
        files_to_touch:
          - 'crates/voya-platform/**'
          - 'crates/voya-app/**'
          - 'src/features/options/**'
          - 'src/features/qr/**'
          - 'docs/verification/autostart-hotkeys-qr.md'
        prompt_context:
          - 'Prefer Tauri or platform plugins where faithful, but keep behavior aligned with the reference.'
      - id: '07-03-i18n-resx-import'
        title: 'I18n Resource Import'
        kind: 'code'
        execution: 'codex'
        goal: 'Convert reference resources into i18next locale files, wire missing-key checks, and verify RTL behavior.'
        depends_on:
          - '07-02-autostart-hotkeys-qr'
        deliverables:
          - 'Locale files for 8 languages including fa RTL.'
          - 'Resource conversion script or documented import process.'
          - 'Missing-key tests.'
          - 'UI language switch integration.'
        acceptance:
          - 'No missing i18n keys in tests.'
          - 'RTL layout can be toggled and is covered by tests or smoke docs.'
        evidence_to_capture:
          - 'docs/verification/i18n.md.'
        verify_commands:
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'test -f docs/verification/i18n.md'
        files_to_touch:
          - 'src/locales/**'
          - 'src/i18n/**'
          - 'scripts/**'
          - 'docs/verification/i18n.md'
        prompt_context:
          - 'Use v2rayN resource files as references but do not edit them.'
      - id: '07-04-theme-a11y-perf'
        title: 'Theme, Accessibility, Performance'
        kind: 'code'
        execution: 'codex'
        goal: 'Polish theme tokens, accent and font settings, accessibility, table performance, modal ergonomics, and UI consistency.'
        depends_on:
          - '07-03-i18n-resx-import'
        deliverables:
          - 'Theme and font settings fully persisted and applied.'
          - 'Accessibility pass for dialogs, menus, table, status controls, and forms.'
          - 'Measured or tested large-table performance path.'
          - 'Visual consistency fixes across screens.'
        acceptance:
          - 'No one-note palette dominates the UI.'
          - 'Text does not overflow compact controls at desktop and mobile-ish widths.'
          - '500 server rows with 1 Hz updates remain responsive in the perf harness or test.'
        evidence_to_capture:
          - 'docs/verification/ui-polish.md.'
        verify_commands:
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'test -f docs/verification/ui-polish.md'
        files_to_touch:
          - 'src/**'
          - 'docs/verification/ui-polish.md'
        prompt_context:
          - 'Follow the frontend design guidance from the global instructions and existing app shell conventions.'
      - id: '07-05-playwright-tauri-smoke'
        title: 'Playwright And Tauri Smoke'
        kind: 'code'
        execution: 'codex'
        goal: 'Add automated smoke coverage for key app flows through Playwright and tauri-driver where locally feasible.'
        depends_on:
          - '07-04-theme-a11y-perf'
        deliverables:
          - 'Playwright setup for frontend flows.'
          - 'Tauri driver smoke setup or documented platform limitations.'
          - 'Smoke tests for app shell, profile add, import fixture, connect fake, routing, DNS, and dialogs.'
          - 'Manual smoke matrix for OS-only flows.'
        acceptance:
          - 'Frontend smoke tests run non-interactively.'
          - 'Tauri-driver gaps are documented with exact manual checks.'
        evidence_to_capture:
          - 'docs/verification/cross-platform-smoke.md.'
        verify_commands:
          - 'pnpm test -- --run'
          - 'pnpm typecheck'
          - 'test -f docs/verification/cross-platform-smoke.md'
        files_to_touch:
          - 'tests/**'
          - 'e2e/**'
          - 'playwright.config.*'
          - 'docs/verification/cross-platform-smoke.md'
        prompt_context:
          - 'Do not require real proxies or real OS proxy changes in automated smoke tests.'
      - id: '07-06-polish-phase-gate'
        title: 'Polish Phase Gate'
        kind: 'verification'
        execution: 'codex'
        goal: 'Stabilize backup, integrations, i18n, theme, accessibility, and smoke checks before packaging.'
        depends_on:
          - '07-05-playwright-tauri-smoke'
        deliverables:
          - 'docs/verification/m6-polish-gate.md.'
          - 'Fixes required for UI and integration quality gates.'
        acceptance:
          - 'Workspace checks pass.'
          - 'Manual OS smoke matrix is ready for release candidates.'
        evidence_to_capture:
          - 'M6 polish gate report.'
        verify_commands:
          - 'cargo test --workspace --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'pnpm bindings:check'
          - 'test -f docs/verification/m6-polish-gate.md'
        files_to_touch:
          - 'docs/verification/m6-polish-gate.md'
          - 'crates/**'
          - 'src/**'
          - 'tests/**'
        prompt_context:
          - 'Keep fixes focused on release-readiness gaps from this phase.'
  - id: '08-packaging-release'
    title: 'Packaging And Release'
    goal: 'Prepare package builds, updater metadata, release CI, runbooks, and final evidence for public beta.'
    depends_on:
      - '07-polish-backup-i18n'
    summary: 'This phase makes the app shippable while keeping credentials and real publication outside the runner.'
    entry_criteria:
      - 'Feature-complete app and smoke checks are available.'
    exit_criteria:
      - 'Debug or unsigned packages build, release workflows are configured, and manual signing or publication steps are documented.'
    risks:
      - 'Packaging can depend on credentials or OS environments unavailable to the runner.'
    batches:
      - id: '08-01-tauri-packaging'
        title: 'Tauri Packaging Config'
        kind: 'code'
        execution: 'codex'
        goal: 'Configure Tauri bundle targets, updater settings, sidecar strategy, attribution, and first-run core download posture.'
        depends_on: []
        deliverables:
          - 'Tauri bundle configuration for macOS, Windows, and Linux targets.'
          - 'Updater configuration with placeholders for keys and channels.'
          - 'Attribution and licenses screen or document.'
          - 'First-run core download flow documentation.'
        acceptance:
          - 'Debug package build can run locally without signing credentials.'
          - 'GPL or AGPL cores are not bundled by default.'
        evidence_to_capture:
          - 'docs/release/packaging.md.'
        verify_commands:
          - 'pnpm tauri:build --debug'
          - 'test -f docs/release/packaging.md'
        files_to_touch:
          - 'src-tauri/**'
          - 'src/features/about/**'
          - 'docs/release/packaging.md'
          - 'package.json'
        prompt_context:
          - 'If platform prerequisites are missing, document the exact failure and keep config changes deterministic.'
      - id: '08-02-release-ci'
        title: 'Release CI Workflows'
        kind: 'code'
        execution: 'codex'
        goal: 'Add release workflows for tests, package builds, updater metadata, checksums, and artifact upload without embedding secrets.'
        depends_on:
          - '08-01-tauri-packaging'
        deliverables:
          - '.github/workflows/release.yml.'
          - 'Artifact naming and checksum scripts.'
          - 'Updater latest.json generation path with secret placeholders.'
          - 'Docs for required CI secrets.'
        acceptance:
          - 'Release workflow is triggerable manually and does not require secrets for dry-run validation.'
          - 'Secrets are referenced by name but never committed.'
        evidence_to_capture:
          - 'docs/release/ci-secrets.md.'
        verify_commands:
          - 'test -f .github/workflows/release.yml'
          - 'test -f docs/release/ci-secrets.md'
          - 'pnpm typecheck'
        files_to_touch:
          - '.github/workflows/release.yml'
          - 'scripts/**'
          - 'docs/release/ci-secrets.md'
        prompt_context:
          - 'Keep real publishing credentials outside the repo.'
      - id: '08-03-release-runbooks'
        title: 'Release Runbooks'
        kind: 'docs'
        execution: 'codex'
        goal: 'Write manual runbooks for signing, notarization, updater keys, OS smoke testing, rollback, and beta publication.'
        depends_on:
          - '08-02-release-ci'
        deliverables:
          - 'docs/release/runbook.md.'
          - 'docs/release/signing-notarization.md.'
          - 'docs/release/os-smoke-matrix.md.'
          - 'docs/release/rollback.md.'
        acceptance:
          - 'Every manual release checkpoint has owner, system, verification, and rollback notes.'
          - 'The runbook separates local debug packaging from public beta publication.'
        evidence_to_capture:
          - 'Release docs exist and link to verification evidence.'
        verify_commands:
          - 'test -f docs/release/runbook.md'
          - 'test -f docs/release/signing-notarization.md'
          - 'test -f docs/release/os-smoke-matrix.md'
          - 'test -f docs/release/rollback.md'
        files_to_touch:
          - 'docs/release/**'
        prompt_context:
          - 'Do not invent secret values; document the names and how owners supply them.'
      - id: '08-04-final-regression-evidence'
        title: 'Final Regression Evidence'
        kind: 'verification'
        execution: 'codex'
        goal: 'Run the final automated regression suite, collect evidence, and document remaining external release prerequisites.'
        depends_on:
          - '08-03-release-runbooks'
        deliverables:
          - 'docs/verification/m7-public-beta-gate.md.'
          - 'Updated README with build, test, dev, and release commands.'
          - 'Fixes required for final automated checks.'
        acceptance:
          - 'All global verification commands pass or documented external prerequisites explain any skipped package-only checks.'
          - 'Remaining work is only external credentials, OS machines, or publication actions.'
        evidence_to_capture:
          - 'M7 public beta gate report.'
        verify_commands:
          - 'cargo test --workspace --all-targets'
          - 'pnpm typecheck'
          - 'pnpm test -- --run'
          - 'pnpm lint'
          - 'pnpm bindings:check'
          - 'test -f docs/verification/m7-public-beta-gate.md'
        files_to_touch:
          - 'docs/verification/m7-public-beta-gate.md'
          - 'README.md'
          - 'crates/**'
          - 'src/**'
          - 'src-tauri/**'
          - '.github/workflows/**'
        prompt_context:
          - 'This is a closeout batch; do not add new feature scope unless required to satisfy an existing acceptance criterion.'
```

<!-- rollout-plan:end -->

## Phase Notes

### 00-baseline

This phase is intentionally documentation-heavy. The goal is to reduce ambiguity before any large code surface exists.

### 01-foundation

Scaffold decisions made here are expensive to reverse. Keep the crate boundaries and generated IPC rule strict.

### 02-profiles-imports

Profiles and parsers are the first meaningful product slice. They also feed the config generation oracle.

### 03-config-generation

This phase is the main correctness risk. Prefer many focused tests and fixture growth over broad unverified ports.

### 04-runtime-alpha

Runtime work must be testable with fake process and platform adapters. Real network and OS checks stay as documented smoke steps.

### 05-routing-dns-tun-groups

This phase exposes advanced config generation through user-facing screens, so backend and frontend should move together.

### 06-service-integrations

Network clients should be injectable and fixture-tested. Live service checks are smoke evidence, not unit test requirements.

### 07-polish-backup-i18n

This phase finishes product breadth and quality. Keep each batch grounded in a concrete workflow.

### 08-packaging-release

The runner can prepare packaging and release automation, but credentials and public publication remain manual.
