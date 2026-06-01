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


PLAN_JSON = "{\"rollout\": {\"name\": \"voyavpn-full-rewrite\", \"repo_root\": \"/Users/afu/Dev/refs/VoyaVPN\", \"workdir\": \".agents/rollouts/voyavpn-full-rewrite/logs\", \"codex_cmd\": null, \"model\": null, \"max_fix_attempts\": 1, \"allow_dirty\": true, \"commit_per_batch\": false, \"sources_of_truth\": [\".agents/rollouts/voyavpn-full-rewrite/spec.md\", \".agents/rollouts/voyavpn-full-rewrite/plan.md\", \"/Users/afu/.claude/plans/typescript-shadcn-tauri-silly-marble.md\", \"/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib\", \"/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib.Tests\", \"/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN\", \"/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN.Desktop\"], \"planning_notes\": [\"This is a greenfield rewrite in VoyaVPN; the v2rayN repo is read-only reference material.\", \"Deliver subsystem by subsystem with backend, frontend, tests, and IPC wiring in the same slice when feasible.\", \"Keep all three platforms in scope from the first scaffold.\", \"Fresh SQLite schema only; no migration from v2rayN data and no obsolete columns.\"], \"success_metrics\": [\"Rust workspace tests pass with cargo test --workspace --all-targets.\", \"Frontend checks pass with pnpm typecheck, pnpm test -- --run, and pnpm lint.\", \"Generated bindings have no drift after regeneration.\", \"Xray and sing-box generated configs match v2rayN golden fixtures and pass core acceptance where binaries exist.\", \"A real server can connect through pnpm tauri dev with logs, stats, and traffic flow.\"], \"global_context\": [\"Target stack: Tauri 2, Rust, React, TypeScript, Tailwind v4, shadcn/ui, Zustand, TanStack Query, TanStack Table, Radix, i18next, sqlx, specta, tauri-specta.\", \"Rust crate layout: voya-core, voya-db, voya-platform, voya-net, voya-udptest, voya-app, and src-tauri.\", \"Frontend IPC rule: only src/ipc may import @tauri-apps/api; all app code uses typed wrappers.\", \"Config generator correctness is judged by generated core JSON and core acceptance, not entity snapshots alone.\"], \"hard_rules\": [\"Do not modify /Users/afu/Dev/refs/v2rayN/v2rayN or sibling reference sources.\", \"Do not add obsolete v2rayN columns or data migration code.\", \"Do not place OS-specific code in voya-core.\", \"Do not hand-write TypeScript IPC DTOs that should be generated from Rust.\", \"Do not import @tauri-apps/api outside src/ipc.\", \"Do not redistribute GPL or AGPL core binaries in installers by default.\", \"Keep diffs focused on the current batch and update tests or docs for the touched surface.\"], \"batch_prompt_suffix\": [\"Finish only this batch and the minimum supporting work required for its verification commands.\", \"Read the source-of-truth files before changing behavior that must match v2rayN.\", \"Capture any skipped external checks in docs with a concrete reason and follow-up.\"]}, \"phases\": [{\"id\": \"00-baseline\", \"title\": \"Baseline And Evidence\", \"goal\": \"Establish source inventory, architecture decisions, and verification scaffolding before implementation starts.\", \"depends_on\": [], \"summary\": \"This phase creates the human evidence needed to keep a full rewrite aligned with the reference app.\", \"entry_criteria\": [\"The target VoyaVPN repo exists and may be empty.\", \"The v2rayN reference repo is available read-only.\"], \"exit_criteria\": [\"Reference source areas and high-risk parity points are documented.\", \"Architecture and verification decisions are captured in docs.\"], \"risks\": [\"Missing source inventory can cause later batches to silently drift from v2rayN behavior.\"], \"batches\": [{\"id\": \"00-01-baseline-inventory\", \"title\": \"Baseline Source Inventory\", \"kind\": \"analysis\", \"execution\": \"codex\", \"goal\": \"Create a source inventory that maps v2rayN systems, UI screens, tests, and fidelity hot spots to VoyaVPN target modules.\", \"depends_on\": [], \"deliverables\": [\"docs/source-inventory.md with backend, frontend, model, parser, config-gen, runtime, platform, and test references.\", \"docs/fidelity-hotspots.md covering sudo lifecycle, finalmask, policy groups, proxy chains, DNS, stats, system proxy, TUN, Clash PATCH, and QR scope.\"], \"acceptance\": [\"Every subsystem from S0 through S19 in the planning source is represented.\", \"Reference paths point to concrete v2rayN files or directories.\"], \"evidence_to_capture\": [\"Inventory docs committed in the target repo tree.\"], \"verify_commands\": [\"test -f docs/source-inventory.md\", \"test -f docs/fidelity-hotspots.md\"], \"files_to_touch\": [\"docs/source-inventory.md\", \"docs/fidelity-hotspots.md\"], \"prompt_context\": [\"Use rg and rg --files against /Users/afu/Dev/refs/v2rayN/v2rayN for evidence.\"]}, {\"id\": \"00-02-architecture-adrs\", \"title\": \"Architecture ADRs\", \"kind\": \"docs\", \"execution\": \"codex\", \"goal\": \"Record the core architecture decisions that all implementation batches must preserve.\", \"depends_on\": [\"00-01-baseline-inventory\"], \"deliverables\": [\"docs/adr/0001-target-architecture.md.\", \"docs/adr/0002-typed-ipc-and-events.md.\", \"docs/adr/0003-config-generation-parity.md.\", \"docs/adr/0004-platform-boundaries.md.\"], \"acceptance\": [\"ADRs capture crate boundaries, generated bindings, event channels, DB stance, platform isolation, and golden testing.\", \"ADRs explicitly state no legacy migration and no obsolete columns.\"], \"evidence_to_capture\": [\"ADR index or docs references are present.\"], \"verify_commands\": [\"test -f docs/adr/0001-target-architecture.md\", \"test -f docs/adr/0002-typed-ipc-and-events.md\", \"test -f docs/adr/0003-config-generation-parity.md\", \"test -f docs/adr/0004-platform-boundaries.md\"], \"files_to_touch\": [\"docs/adr/**\"], \"prompt_context\": [\"Keep ADRs short but decisive so later Codex batches can rely on them.\"]}, {\"id\": \"00-03-verification-scaffold-plan\", \"title\": \"Verification Scaffold Plan\", \"kind\": \"docs\", \"execution\": \"codex\", \"goal\": \"Create the verification map for unit, golden, IPC drift, frontend, platform, and packaging checks.\", \"depends_on\": [\"00-02-architecture-adrs\"], \"deliverables\": [\"docs/verification/strategy.md.\", \"tests/golden/README.md explaining golden fixture shape and canonicalization.\", \"docs/verification/manual-os-smoke.md for checks that require real OS machines.\"], \"acceptance\": [\"The strategy defines local deterministic checks and separate manual evidence.\", \"Golden tests assert on generated configs, not only entity snapshots.\"], \"evidence_to_capture\": [\"Verification docs and initial tests/golden directory exist.\"], \"verify_commands\": [\"test -f docs/verification/strategy.md\", \"test -f tests/golden/README.md\", \"test -f docs/verification/manual-os-smoke.md\"], \"files_to_touch\": [\"docs/verification/**\", \"tests/golden/**\"], \"prompt_context\": [\"Do not require external core binaries in this batch; document how later checks discover or skip them.\"]}]}, {\"id\": \"01-foundation\", \"title\": \"Workspace, Shell, IPC, DB\", \"goal\": \"Create the Rust, Tauri, React, typed IPC, data, event, and CI foundation.\", \"depends_on\": [\"00-baseline\"], \"summary\": \"This phase establishes the repo shape that all subsystem work builds on.\", \"entry_criteria\": [\"Baseline docs and ADRs exist.\"], \"exit_criteria\": [\"Workspace compiles, frontend checks run, generated IPC exists, DB migrations exist, and CI covers baseline checks.\"], \"risks\": [\"Bad early boundaries can force expensive refactors during config-gen or platform work.\"], \"batches\": [{\"id\": \"01-01-workspace-scaffold\", \"title\": \"Workspace Scaffold\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Create the initial Rust workspace, Tauri app, frontend package, crate directories, formatting, linting, and baseline scripts.\", \"depends_on\": [], \"deliverables\": [\"Cargo workspace with src-tauri and crates/voya-* packages.\", \"Vite React TypeScript app with Tailwind v4 and shadcn/ui foundations.\", \"package.json, pnpm lockfile, tsconfig, rustfmt, clippy, and README baseline.\"], \"acceptance\": [\"Cargo metadata resolves.\", \"Frontend package scripts exist for typecheck, test, lint, bindings, and tauri build.\", \"No app behavior beyond a minimal shell is required yet.\"], \"evidence_to_capture\": [\"Workspace metadata and package scripts are present.\"], \"verify_commands\": [\"test -f Cargo.toml\", \"test -f package.json\", \"test -f src-tauri/Cargo.toml\", \"test -f crates/voya-core/Cargo.toml\", \"cargo metadata --format-version 1 --no-deps\"], \"files_to_touch\": [\"Cargo.toml\", \"crates/**\", \"src-tauri/**\", \"src/**\", \"package.json\", \"pnpm-lock.yaml\"], \"prompt_context\": [\"Use workspace dependencies conservatively and keep the repo green after scaffold.\"]}, {\"id\": \"01-02-app-shell-design-system\", \"title\": \"App Shell And Design System\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Build the navigable empty app shell with menubar, tabs, status bar, modal host, toaster, i18n, RTL-ready theme, and static tray.\", \"depends_on\": [\"01-01-workspace-scaffold\"], \"deliverables\": [\"React AppShell components and stores.\", \"shadcn/ui base components used by the shell.\", \"i18next setup with initial locale files.\", \"Theme and accent persistence stubs.\", \"Static Rust tray menu.\"], \"acceptance\": [\"The first screen is the usable app shell, not a landing page.\", \"Tabs for Profiles, Clash Proxies, Clash Connections, and Logs exist even if empty.\", \"RTL locale plumbing is present.\"], \"evidence_to_capture\": [\"Frontend smoke test or component test for AppShell.\"], \"verify_commands\": [\"pnpm typecheck\", \"pnpm test -- --run\"], \"files_to_touch\": [\"src/**\", \"src-tauri/**\"], \"prompt_context\": [\"Keep UI quiet and operational; avoid marketing-style hero content.\"]}, {\"id\": \"01-03-typed-ipc-events\", \"title\": \"Typed IPC And Event Bridge\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement specta and tauri-specta export, generated bindings, typed command wrappers, and the three-channel event bridge.\", \"depends_on\": [\"01-02-app-shell-design-system\"], \"deliverables\": [\"Rust command and event type scaffolding.\", \"Generated src/ipc/bindings.ts.\", \"src/ipc command wrappers and EventBridge.\", \"bindings drift check script.\"], \"acceptance\": [\"A demo command round-trips through generated types.\", \"Only src/ipc imports @tauri-apps/api.\", \"EventBridge routes invalidate, transient stream, and imperative app events.\"], \"evidence_to_capture\": [\"bindings drift command output documented in docs/verification/bindings.md.\"], \"verify_commands\": [\"pnpm bindings:check\", \"pnpm typecheck\", \"rg \\\"@tauri-apps/api\\\" src | rg -v \\\"^src/ipc/\\\" -q && exit 1 || exit 0\"], \"files_to_touch\": [\"src-tauri/**\", \"src/ipc/**\", \"src/**\", \"docs/verification/bindings.md\"], \"prompt_context\": [\"The generated binding file is allowed to be checked in and must be regenerated by script.\"]}, {\"id\": \"01-04-data-models-db-config\", \"title\": \"Data Models, DB, Config\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Port live model shapes, fresh SQLite schema, repositories, JSON config defaults, and typed blob boundary.\", \"depends_on\": [\"01-03-typed-ipc-events\"], \"deliverables\": [\"voya-core models and enums with serde and specta derives.\", \"voya-db migrations, repositories, and typed JSON blob helpers.\", \"AppConfig defaults and load/save commands.\", \"Unit and integration tests for DB defaults and persistence.\"], \"acceptance\": [\"Obsolete columns are absent.\", \"Enum discriminants match the planning source.\", \"Settings persist across process restart in tests.\"], \"evidence_to_capture\": [\"docs/verification/db-schema.md with schema notes.\"], \"verify_commands\": [\"cargo test -p voya-core --all-targets\", \"cargo test -p voya-db --all-targets\", \"pnpm bindings:check\"], \"files_to_touch\": [\"crates/voya-core/**\", \"crates/voya-db/**\", \"src-tauri/**\", \"src/ipc/**\", \"docs/verification/db-schema.md\"], \"prompt_context\": [\"ProtocolExtraItem and TransportExtraItem stay typed across IPC and become TEXT only inside voya-db blob helpers.\"]}, {\"id\": \"01-05-ci-baseline\", \"title\": \"CI Baseline\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Add CI workflows for Rust, frontend, generated binding drift, and baseline formatting or lint checks.\", \"depends_on\": [\"01-04-data-models-db-config\"], \"deliverables\": [\".github/workflows/ci.yml.\", \"Scripts for local verification parity with CI.\", \"README instructions for local setup.\"], \"acceptance\": [\"CI commands are non-interactive and mirror local scripts.\", \"Binding drift is a first-class check.\"], \"evidence_to_capture\": [\"docs/verification/ci.md documenting the baseline.\"], \"verify_commands\": [\"cargo test --workspace --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"pnpm bindings:check\"], \"files_to_touch\": [\".github/workflows/**\", \"README.md\", \"docs/verification/ci.md\", \"package.json\"], \"prompt_context\": [\"Keep CI broad enough to catch drift but avoid requiring signing credentials or real core binaries.\"]}]}, {\"id\": \"02-profiles-imports\", \"title\": \"Profiles, Parsers, Subscriptions\", \"goal\": \"Deliver persisted profiles, server table, protocol dialogs, share links, imports, and subscription flows.\", \"depends_on\": [\"01-foundation\"], \"summary\": \"This phase turns the empty shell into a profile manager with real data and import paths.\", \"entry_criteria\": [\"Typed IPC, data models, DB, and app shell are available.\"], \"exit_criteria\": [\"Users can create, edit, import, view, sort, dedupe, and persist profiles and subscriptions.\"], \"risks\": [\"Parser edge cases can corrupt later config generation if not tested early.\"], \"batches\": [{\"id\": \"02-01-profile-crud-managers\", \"title\": \"Profile CRUD Managers\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement profile CRUD, ordering, active selection, dedupe, copy, move, grouping, and ProfileEx manager behavior.\", \"depends_on\": [], \"deliverables\": [\"voya-app profile manager modules.\", \"DB repositories and IPC commands for profile operations.\", \"Invalidation events for profile changes.\", \"Rust tests for CRUD, reorder, defaulting, dedupe, and active selection.\"], \"acceptance\": [\"All live profile types can be persisted through typed commands.\", \"Operations update state and emit expected invalidation events.\"], \"evidence_to_capture\": [\"docs/verification/profile-crud.md.\"], \"verify_commands\": [\"cargo test -p voya-app profile --all-targets\", \"cargo test -p voya-db profile --all-targets\", \"pnpm bindings:check\"], \"files_to_touch\": [\"crates/voya-app/**\", \"crates/voya-db/**\", \"crates/voya-core/**\", \"src-tauri/**\", \"docs/verification/profile-crud.md\"], \"prompt_context\": [\"Use ConfigHandler behavior as the reference but decompose it into cohesive Rust modules.\"]}, {\"id\": \"02-02-server-table-dialogs\", \"title\": \"Server Table And Profile Dialogs\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement the virtualized server table and per-protocol add/edit dialogs wired to real profile IPC.\", \"depends_on\": [\"02-01-profile-crud-managers\"], \"deliverables\": [\"ServerTable with virtualization, columns, multi-select, context menu, drag reorder, filter, and active marker.\", \"Add/Edit profile dialogs with react-hook-form and zod discriminated unions.\", \"Protocol, transport, security, and mux panels.\", \"Frontend tests for table and form behavior.\"], \"acceptance\": [\"Every supported protocol has a form path.\", \"5k row table scenario remains virtualized in tests or documented perf harness.\", \"Create, edit, delete, copy, move, sort, and activate use real IPC.\"], \"evidence_to_capture\": [\"docs/verification/server-table.md.\"], \"verify_commands\": [\"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\"], \"files_to_touch\": [\"src/features/profiles/**\", \"src/components/**\", \"src/stores/**\", \"docs/verification/server-table.md\"], \"prompt_context\": [\"Use icons and compact operational controls; keep columns stable and responsive.\"]}, {\"id\": \"02-03-share-link-parsers\", \"title\": \"Share Link Parsers\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Port share-link parse and export behavior for all supported protocols with round-trip and negative tests.\", \"depends_on\": [\"02-01-profile-crud-managers\"], \"deliverables\": [\"ShareFmt trait and protocol implementations in voya-core.\", \"Base query and stream codec handling transport, security, pqv, ech, pcs, and fm parameters.\", \"Inner v2rayn format and full JSON custom import helpers.\", \"Property, round-trip, and negative parser tests.\"], \"acceptance\": [\"Protocols covered: vmess, vless, trojan, ss, hysteria2, tuic, wireguard, anytls, naive, socks, and inner format.\", \"Malformed inputs return typed errors and never panic.\"], \"evidence_to_capture\": [\"docs/verification/share-links.md with parity notes against ServiceLib.Tests/Fmt.\"], \"verify_commands\": [\"cargo test -p voya-core fmt --all-targets\", \"cargo test -p voya-core share --all-targets\"], \"files_to_touch\": [\"crates/voya-core/**\", \"docs/verification/share-links.md\"], \"prompt_context\": [\"Read ServiceLib/Handler/Fmt and ServiceLib.Tests/Fmt before implementing each protocol.\"]}, {\"id\": \"02-04-import-subscriptions\", \"title\": \"Imports And Subscriptions\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement manual import flows, subscription management, update engine, filters, dedupe, conversion target, and scheduler.\", \"depends_on\": [\"02-03-share-link-parsers\", \"02-02-server-table-dialogs\"], \"deliverables\": [\"voya-net download client with proxy-to-direct fallback.\", \"Subscription manager in voya-app.\", \"Subscription UI dialogs and update actions.\", \"Clipboard, file, and JSON import flows where locally testable.\", \"Tests for base64, multi-URL, filter, dedupe, UA, and conversion target behavior.\"], \"acceptance\": [\"A real or fixture subscription imports, deduplicates, persists, and invalidates profiles.\", \"Auto-update scheduler can be started and stopped deterministically in tests.\"], \"evidence_to_capture\": [\"docs/verification/subscriptions.md.\"], \"verify_commands\": [\"cargo test -p voya-net --all-targets\", \"cargo test -p voya-app subscription --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\"], \"files_to_touch\": [\"crates/voya-net/**\", \"crates/voya-app/**\", \"src/features/subscriptions/**\", \"src-tauri/**\", \"docs/verification/subscriptions.md\"], \"prompt_context\": [\"Regex filtering and dedupe belong in the manager decomposition, not only the raw subscription download layer.\"]}, {\"id\": \"02-05-profile-phase-gate\", \"title\": \"Profile Phase Gate\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Stabilize profile, parser, subscription, table, and IPC behavior before config generation starts.\", \"depends_on\": [\"02-04-import-subscriptions\"], \"deliverables\": [\"docs/verification/m1-profile-gate.md with command results and any deferred edge cases.\", \"Additional tests or fixes needed to make the phase green.\"], \"acceptance\": [\"Workspace checks pass for the profile and import surface.\", \"Deferred items are explicit and do not block config generation.\"], \"evidence_to_capture\": [\"M1 gate report.\"], \"verify_commands\": [\"cargo test --workspace --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"pnpm bindings:check\", \"test -f docs/verification/m1-profile-gate.md\"], \"files_to_touch\": [\"docs/verification/m1-profile-gate.md\", \"crates/**\", \"src/**\", \"src-tauri/**\"], \"prompt_context\": [\"Fix only profile, parser, subscription, IPC, and table issues found by the gate.\"]}]}, {\"id\": \"03-config-generation\", \"title\": \"Xray And Sing-Box Config Generation\", \"goal\": \"Port deterministic config generation for Xray and sing-box with golden parity and core acceptance gates.\", \"depends_on\": [\"02-profiles-imports\"], \"summary\": \"This is the highest-fidelity phase and should move one resolver or generator area at a time.\", \"entry_criteria\": [\"Profiles, parser models, DB, and typed IPC are stable.\"], \"exit_criteria\": [\"Xray and sing-box golden parity is established for the fixture matrix.\"], \"risks\": [\"Small serialization differences can produce valid but behaviorally wrong configs.\", \"Policy group, proxy chain, DNS, finalmask, and TUN behavior are especially easy to drift.\"], \"batches\": [{\"id\": \"03-01-context-builder\", \"title\": \"Core Config Context Builder\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Port cross-entity context resolution shared by Xray and sing-box generation.\", \"depends_on\": [], \"deliverables\": [\"CoreGenEnv trait and deterministic context builder in voya-core.\", \"Resolution for active node, pre-socks contexts, groups, chains, sub-level virtual proxy chains, per-rule outbounds, protect domains, and template inputs.\", \"Cycle detection, dedupe, and ECH SNI extraction tests.\"], \"acceptance\": [\"Context builder is OS-free and deterministic.\", \"Main context disables TUN when building pre-socks as specified.\"], \"evidence_to_capture\": [\"docs/verification/context-builder.md.\"], \"verify_commands\": [\"cargo test -p voya-core context --all-targets\", \"test -f docs/verification/context-builder.md\"], \"files_to_touch\": [\"crates/voya-core/**\", \"docs/verification/context-builder.md\"], \"prompt_context\": [\"Use Handler/Builder/CoreConfigContextBuilder.cs as the primary reference.\"]}, {\"id\": \"03-02-xray-coregen-outbounds\", \"title\": \"Xray Outbounds And Streams\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement Xray outbound generation for protocols, transports, security, mux, proxy chains, policy groups, and finalmask.\", \"depends_on\": [\"03-01-context-builder\"], \"deliverables\": [\"Xray serde config models.\", \"Outbound generation for live protocols and transports.\", \"TLS, reality, ECH, uTLS fingerprint, ALPN, pinned cert, and finalmask composition.\", \"PolicyGroup balancer and observatory behavior.\", \"ProxyChain dialerProxy behavior including xhttp rewrite.\"], \"acceptance\": [\"Golden unit fixtures cover the highest-risk outbound combinations.\", \"serde casing and skip behavior match Xray expectations.\"], \"evidence_to_capture\": [\"docs/verification/xray-outbounds.md.\"], \"verify_commands\": [\"cargo test -p voya-core xray_outbound --all-targets\", \"cargo test -p voya-core policy_group --all-targets\", \"test -f docs/verification/xray-outbounds.md\"], \"files_to_touch\": [\"crates/voya-core/**\", \"tests/golden/**\", \"docs/verification/xray-outbounds.md\"], \"prompt_context\": [\"Model finalmask as the merge target, including tcp and udp noise behavior and documented precedence.\"]}, {\"id\": \"03-03-xray-coregen-routing-dns\", \"title\": \"Xray Inbounds, Routing, DNS, Templates\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Complete Xray generation for inbounds, DNS, routing, stats, TUN inbound, dokodemo API, and full config templates.\", \"depends_on\": [\"03-02-xray-coregen-outbounds\"], \"deliverables\": [\"Xray inbound, DNS, routing, stats, log, and template services.\", \"SOCKS variants, LAN auth, sniffing, second port, dokodemo API, and Xray TUN inbound.\", \"FullConfigTemplate support for proxy-only, proxy-detour, and TunConfig.\", \"Xray generator tests and canonical snapshots.\"], \"acceptance\": [\"Minimal and advanced DNS/routing fixtures match the reference behavior.\", \"Xray TUN inbound is generated when TUN is enabled.\"], \"evidence_to_capture\": [\"docs/verification/xray-configgen.md.\"], \"verify_commands\": [\"cargo test -p voya-core xray --all-targets\", \"test -f docs/verification/xray-configgen.md\"], \"files_to_touch\": [\"crates/voya-core/**\", \"tests/golden/**\", \"docs/verification/xray-configgen.md\"], \"prompt_context\": [\"DNS must include fakeip, expected IPs, hosts, strategies, custom DNS override, and final-DNS direct auto-detect behavior.\"]}, {\"id\": \"03-04-singbox-coregen-outbounds\", \"title\": \"Sing-Box Outbounds And Selectors\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement sing-box outbound generation for protocols, transports, selectors, urltest, mux, detour, and clash API basics.\", \"depends_on\": [\"03-01-context-builder\"], \"deliverables\": [\"sing-box serde config models with snake_case and strict skip rules.\", \"Outbound and transport generation for live protocol matrix.\", \"Selector and urltest policy group generation.\", \"ProxyChain detour behavior.\", \"Experimental clash_api and cache_file scaffolding.\"], \"acceptance\": [\"Generated JSON avoids null or unknown fields rejected by sing-box.\", \"Selector ordering and dedupe match policy group expectations.\"], \"evidence_to_capture\": [\"docs/verification/singbox-outbounds.md.\"], \"verify_commands\": [\"cargo test -p voya-core singbox_outbound --all-targets\", \"cargo test -p voya-core singbox_selector --all-targets\", \"test -f docs/verification/singbox-outbounds.md\"], \"files_to_touch\": [\"crates/voya-core/**\", \"tests/golden/**\", \"docs/verification/singbox-outbounds.md\"], \"prompt_context\": [\"Keep sing-box strict serialization in mind; unknown or null fields can fail check -c.\"]}, {\"id\": \"03-05-singbox-coregen-routing-dns\", \"title\": \"Sing-Box Inbounds, Routing, DNS, Rulesets\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Complete sing-box generation for inbounds, typed DNS server schema, routing, fakeip, rulesets, templates, and TUN config.\", \"depends_on\": [\"03-04-singbox-coregen-outbounds\"], \"deliverables\": [\"Sing-box inbound, DNS, routing, ruleset, stats, log, and template services.\", \"Typed Server4Sbox and Rule4Sbox schema.\", \"Fakeip, predefined hosts, independent cache, rule_set, and TUN template support.\", \"Generator tests and canonical snapshots.\"], \"acceptance\": [\"DNS uses the new typed-server schema with domain_resolver and predefined hosts as required.\", \"Fakeip and TUN fixtures are covered.\"], \"evidence_to_capture\": [\"docs/verification/singbox-configgen.md.\"], \"verify_commands\": [\"cargo test -p voya-core singbox --all-targets\", \"test -f docs/verification/singbox-configgen.md\"], \"files_to_touch\": [\"crates/voya-core/**\", \"tests/golden/**\", \"docs/verification/singbox-configgen.md\"], \"prompt_context\": [\"ParseDnsAddress and final DNS behavior must follow the reference plan and source.\"]}, {\"id\": \"03-06-golden-export-harness\", \"title\": \"Golden Export Harness\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Create the v2rayN golden export path, canonical diff tooling, and optional core acceptance checks.\", \"depends_on\": [\"03-03-xray-coregen-routing-dns\", \"03-05-singbox-coregen-routing-dns\"], \"deliverables\": [\"Golden fixture loader and canonicalizer in Rust tests.\", \"Documented C# export harness path or helper scripts that do not modify the reference repo unless explicitly copied.\", \"Optional xray run -test and sing-box check -c checks that skip clearly when binaries are missing.\", \"Golden report document.\"], \"acceptance\": [\"Fixture matrix is represented and can grow incrementally.\", \"Golden failures produce actionable diffs.\", \"Missing external binaries do not make local unit tests fail without a clear opt-in.\"], \"evidence_to_capture\": [\"docs/verification/golden-report.md.\"], \"verify_commands\": [\"cargo test -p voya-core golden --all-targets\", \"test -f docs/verification/golden-report.md\"], \"files_to_touch\": [\"crates/voya-core/**\", \"tests/golden/**\", \"docs/verification/golden-report.md\", \"scripts/**\"], \"prompt_context\": [\"Do not write into /Users/afu/Dev/refs/v2rayN/v2rayN; if helper code is needed, copy or document it in VoyaVPN.\"]}, {\"id\": \"03-07-configgen-phase-gate\", \"title\": \"Config Generation Phase Gate\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Stabilize context builder, Xray, sing-box, templates, golden tests, and generated IPC after config generation.\", \"depends_on\": [\"03-06-golden-export-harness\"], \"deliverables\": [\"docs/verification/m2-configgen-gate.md.\", \"Fixes required for config generation tests and drift checks.\"], \"acceptance\": [\"Config-generation relevant tests pass.\", \"Golden report lists coverage and known gaps.\"], \"evidence_to_capture\": [\"M2 config generation gate report.\"], \"verify_commands\": [\"cargo test -p voya-core --all-targets\", \"cargo test --workspace --all-targets\", \"pnpm bindings:check\", \"test -f docs/verification/m2-configgen-gate.md\"], \"files_to_touch\": [\"docs/verification/m2-configgen-gate.md\", \"crates/voya-core/**\", \"tests/golden/**\"], \"prompt_context\": [\"Prioritize behavioral parity fixes over expanding unrelated app features.\"]}]}, {\"id\": \"04-runtime-alpha\", \"title\": \"Runtime Alpha\", \"goal\": \"Deliver connect, disconnect, core process supervision, logs, system proxy, tray, and statistics.\", \"depends_on\": [\"03-config-generation\"], \"summary\": \"This phase turns generated configs into a usable internal alpha.\", \"entry_criteria\": [\"Xray and sing-box configs can be generated from persisted profiles.\"], \"exit_criteria\": [\"A real server can connect, traffic flows, logs stream, proxy mode toggles, and speed is visible.\"], \"risks\": [\"Privilege, process tree, and route cleanup bugs can leave the host in a bad state.\"], \"batches\": [{\"id\": \"04-01-coreinfo-process-model\", \"title\": \"Core Info And Process Model\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Port the 15-core launch table, executable discovery, per-core arguments, env vars, and app/bin/log path resolution.\", \"depends_on\": [], \"deliverables\": [\"voya-platform app dir, bin dir, log dir, temp dir, and portable-mode detection.\", \"voya-app or voya-platform CoreInfo table for all supported cores.\", \"Executable discovery and chmod behavior on Unix.\", \"Tests for command templates and OS path behavior.\"], \"acceptance\": [\"CoreInfo covers xray, v2fly variants, mihomo, hysteria, naiveproxy, tuic, sing-box, juicity, brook, overtls, shadowquic, mieru, and v2rayN core entries.\", \"Argument substitution and env vars match the reference plan.\"], \"evidence_to_capture\": [\"docs/verification/coreinfo.md.\"], \"verify_commands\": [\"cargo test -p voya-platform coreinfo --all-targets\", \"cargo test -p voya-app coreinfo --all-targets\", \"test -f docs/verification/coreinfo.md\"], \"files_to_touch\": [\"crates/voya-platform/**\", \"crates/voya-app/**\", \"docs/verification/coreinfo.md\"], \"prompt_context\": [\"Pay attention to mihomo executable probe order and env vars for v2fly, xray, and mieru.\"]}, {\"id\": \"04-02-supervisor-elevation\", \"title\": \"Supervisor And Elevation\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement the actor-owned core supervisor, dual-process lifecycle, sudo or UAC primitives, Windows job handling, and teardown order.\", \"depends_on\": [\"04-01-coreinfo-process-model\"], \"deliverables\": [\"Supervisor actor with serialized start, stop, restart, and crash handling.\", \"Dual-process main and pre core lifecycle.\", \"Unix sudo password storage using Zeroizing and request-response collection primitives.\", \"Windows job object and TUN cleanup abstractions.\", \"Tests with fake process runner.\"], \"acceptance\": [\"Teardown order is sudo kill, main, pre.\", \"Sudo password is collected once when TUN is enabled and read synchronously at spawn.\", \"Linux and macOS share sudo -S behavior except kill script name.\"], \"evidence_to_capture\": [\"docs/verification/supervisor.md.\"], \"verify_commands\": [\"cargo test -p voya-platform process --all-targets\", \"cargo test -p voya-app supervisor --all-targets\", \"test -f docs/verification/supervisor.md\"], \"files_to_touch\": [\"crates/voya-platform/**\", \"crates/voya-app/**\", \"src-tauri/**\", \"docs/verification/supervisor.md\"], \"prompt_context\": [\"Use fake process runners for deterministic tests; leave real OS smoke for later docs.\"]}, {\"id\": \"04-03-connect-logs-ui\", \"title\": \"Connect, Disconnect, Logs UI\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Wire connect and disconnect commands, config generation, supervisor start, status events, log streaming, and UI controls.\", \"depends_on\": [\"04-02-supervisor-elevation\"], \"deliverables\": [\"Tauri commands for connect, disconnect, restart, and status.\", \"Log event streaming and Logs tab.\", \"Status bar connect controls and core state display.\", \"Sudo prompt modal for the collection primitive.\", \"Integration tests using fake generated configs or fake process runner.\"], \"acceptance\": [\"Connecting an active profile writes config files and starts the supervisor path.\", \"Logs stream to the Logs tab through transient events.\", \"Disconnect updates core state and cleans generated runtime state.\"], \"evidence_to_capture\": [\"docs/verification/runtime-alpha.md.\"], \"verify_commands\": [\"cargo test -p voya-app supervisor --all-targets\", \"cargo test --workspace --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/runtime-alpha.md\"], \"files_to_touch\": [\"crates/voya-app/**\", \"src-tauri/**\", \"src/features/logs/**\", \"src/features/status/**\", \"src/ipc/**\", \"docs/verification/runtime-alpha.md\"], \"prompt_context\": [\"Real network traffic can be a documented smoke step; automated tests should use fakes.\"]}, {\"id\": \"04-04-system-proxy-tray\", \"title\": \"System Proxy, PAC, Tray\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Port per-OS system proxy behavior, Windows-only PAC server, proxy mode switching, and dynamic tray menu.\", \"depends_on\": [\"04-03-connect-logs-ui\"], \"deliverables\": [\"voya-platform sysproxy modules for Windows, Linux, and macOS.\", \"PAC manager gated to Windows.\", \"Proxy mode commands and status events.\", \"Dynamic tray menu with recent servers and mode submenu.\", \"UI status bar segmented control with PAC hidden off Windows.\"], \"acceptance\": [\"Forced change, forced clear, unchanged, and PAC modes are represented.\", \"Advanced template substitutes both http_port and socks_port to the single SOCKS port and prepends local exceptions.\", \"Exit and disconnect restore behavior is documented and testable with fakes.\"], \"evidence_to_capture\": [\"docs/verification/system-proxy.md.\"], \"verify_commands\": [\"cargo test -p voya-platform sysproxy --all-targets\", \"cargo test -p voya-app sysproxy --all-targets\", \"pnpm typecheck\", \"test -f docs/verification/system-proxy.md\"], \"files_to_touch\": [\"crates/voya-platform/**\", \"crates/voya-app/**\", \"src-tauri/**\", \"src/features/status/**\", \"docs/verification/system-proxy.md\"], \"prompt_context\": [\"PAC HTTP server is Windows-only; do not expose PAC as enabled on Linux or macOS.\"]}, {\"id\": \"04-05-statistics-speed\", \"title\": \"Statistics And Speed Columns\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement Xray and sing-box stat services, coalesced speed events, persistent per-server traffic, and UI speed columns.\", \"depends_on\": [\"04-03-connect-logs-ui\"], \"deliverables\": [\"Xray debug vars polling service.\", \"sing-box traffic WebSocket service.\", \"ServerStatItem persistence with date rollover, orphan cleanup, and clone behavior.\", \"Status bar live speed and server table traffic columns.\", \"Tests for stat parsing, active-server keying, and rollover.\"], \"acceptance\": [\"Both stat services run concurrently and no-op unless their core is active.\", \"Xray uses StatePort and sing-box uses StatePort2.\", \"Display speed can sum proxy and direct while persistent traffic keys to the active server.\"], \"evidence_to_capture\": [\"docs/verification/statistics.md.\"], \"verify_commands\": [\"cargo test -p voya-app statistics --all-targets\", \"cargo test -p voya-db statistics --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/statistics.md\"], \"files_to_touch\": [\"crates/voya-app/**\", \"crates/voya-db/**\", \"src/features/profiles/**\", \"src/features/status/**\", \"docs/verification/statistics.md\"], \"prompt_context\": [\"Coalesce UI events around 1 Hz and keep the stats hot path lightweight.\"]}, {\"id\": \"04-06-alpha-phase-gate\", \"title\": \"Runtime Alpha Phase Gate\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Stabilize first usable internal alpha behavior and document real-server smoke steps.\", \"depends_on\": [\"04-04-system-proxy-tray\", \"04-05-statistics-speed\"], \"deliverables\": [\"docs/verification/m3-runtime-alpha-gate.md.\", \"Fixes needed for runtime, proxy, logs, stats, and frontend checks.\"], \"acceptance\": [\"Automated workspace checks pass.\", \"Manual real-server smoke steps are precise enough to execute.\"], \"evidence_to_capture\": [\"M3 runtime alpha gate report.\"], \"verify_commands\": [\"cargo test --workspace --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"pnpm bindings:check\", \"test -f docs/verification/m3-runtime-alpha-gate.md\"], \"files_to_touch\": [\"docs/verification/m3-runtime-alpha-gate.md\", \"crates/**\", \"src/**\", \"src-tauri/**\"], \"prompt_context\": [\"Do not require actual network credentials in automated checks.\"]}]}, {\"id\": \"05-routing-dns-tun-groups\", \"title\": \"Routing, DNS, TUN, Groups\", \"goal\": \"Complete routing settings, DNS settings, TUN polish, policy group UI, proxy chain UI, and regional presets.\", \"depends_on\": [\"04-runtime-alpha\"], \"summary\": \"This phase deepens runtime control and exposes advanced generator features through the UI.\", \"entry_criteria\": [\"Runtime alpha can connect and show state through real IPC.\"], \"exit_criteria\": [\"Routing, DNS, TUN, policy groups, proxy chains, and presets work in both generators and UI.\"], \"risks\": [\"Advanced generator UI can diverge from backend structures without typed forms and tests.\"], \"batches\": [{\"id\": \"05-01-routing-settings-editor\", \"title\": \"Routing Settings And Rule Editor\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement routing config CRUD, rule list, rule editor, templates, domain strategies, and reconnect integration.\", \"depends_on\": [], \"deliverables\": [\"Routing repositories, managers, IPC commands, and invalidation events.\", \"Routing screen, rule list, and rule editor dialogs.\", \"Template fetch path from RouteRulesTemplateSourceUrl.\", \"Tests for active routing selection and rule serialization into both generators.\"], \"acceptance\": [\"Users can create, edit, activate, and delete routing profiles and rules.\", \"Reconnect picks up active routing changes.\"], \"evidence_to_capture\": [\"docs/verification/routing.md.\"], \"verify_commands\": [\"cargo test -p voya-app routing --all-targets\", \"cargo test -p voya-core routing --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/routing.md\"], \"files_to_touch\": [\"crates/voya-app/**\", \"crates/voya-core/**\", \"src/features/routing/**\", \"src-tauri/**\", \"docs/verification/routing.md\"], \"prompt_context\": [\"Routing changes must feed both Xray and sing-box config generation.\"]}, {\"id\": \"05-02-dns-settings\", \"title\": \"DNS Settings\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement simple and per-core advanced DNS settings, custom DNS JSON editors, fakeip controls, and generator integration.\", \"depends_on\": [\"05-01-routing-settings-editor\"], \"deliverables\": [\"DNS repositories, managers, IPC commands, and UI.\", \"CodeMirror JSON editors for advanced per-core DNS.\", \"Validation and tests for fakeip, expected IPs, hosts, bootstrap, serve stale, strategy, and raw DNS override behavior.\"], \"acceptance\": [\"DNS settings persist and regenerate configs for both Xray and sing-box.\", \"Invalid JSON returns typed errors mapped to UI forms.\"], \"evidence_to_capture\": [\"docs/verification/dns.md.\"], \"verify_commands\": [\"cargo test -p voya-app dns --all-targets\", \"cargo test -p voya-core dns --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/dns.md\"], \"files_to_touch\": [\"crates/voya-app/**\", \"crates/voya-core/**\", \"src/features/dns/**\", \"docs/verification/dns.md\"], \"prompt_context\": [\"Keep sing-box DNS typed schema and Xray DNS behavior distinct but driven by shared settings.\"]}, {\"id\": \"05-03-tun-mode-polish\", \"title\": \"TUN Mode Polish\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Complete TUN lifecycle, elevation UX, driver preflight, route restoration notes, and UI controls across platforms.\", \"depends_on\": [\"05-02-dns-settings\"], \"deliverables\": [\"TUN manager and commands in voya-app and voya-platform.\", \"Status bar TUN toggle and sudo prompt UX.\", \"Preflight checks and restore-on-disconnect behavior.\", \"Tests with fake platform adapters and docs for manual OS smoke.\"], \"acceptance\": [\"AllowEnableTun on Unix is tied to non-empty stored sudo password.\", \"sing-box and mihomo sudo wrapping and Xray TUN inbound remain distinct paths.\", \"No orphan elevated process expectation is documented and tested with fakes.\"], \"evidence_to_capture\": [\"docs/verification/tun.md.\"], \"verify_commands\": [\"cargo test -p voya-platform tun --all-targets\", \"cargo test -p voya-app tun --all-targets\", \"pnpm typecheck\", \"test -f docs/verification/tun.md\"], \"files_to_touch\": [\"crates/voya-platform/**\", \"crates/voya-app/**\", \"src/features/status/**\", \"src/features/tun/**\", \"docs/verification/tun.md\"], \"prompt_context\": [\"Do not prompt for sudo per spawn; collect once at TUN-enable.\"]}, {\"id\": \"05-04-policy-groups-chains-ui\", \"title\": \"Policy Groups And Proxy Chains UI\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement policy group and proxy chain builders, child pickers, previews, validation, and generator-backed tests.\", \"depends_on\": [\"05-03-tun-mode-polish\"], \"deliverables\": [\"Group and chain data commands and UI dialogs.\", \"Server picker modal flow for nested selection.\", \"Preview of selector, urltest, dialerProxy, and detour output.\", \"Golden tests for mixed-child groups and two-hop or three-hop chains.\"], \"acceptance\": [\"Mixed-child group and 2 or 3 hop proxy chain route correctly in generated configs.\", \"Cycle detection prevents invalid chain or group structures.\"], \"evidence_to_capture\": [\"docs/verification/groups-chains.md.\"], \"verify_commands\": [\"cargo test -p voya-core proxy_chain --all-targets\", \"cargo test -p voya-core policy_group --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/groups-chains.md\"], \"files_to_touch\": [\"crates/voya-core/**\", \"crates/voya-app/**\", \"src/features/groups/**\", \"docs/verification/groups-chains.md\"], \"prompt_context\": [\"Config generation support already exists from phase 03; this batch exposes and validates it through UI.\"]}, {\"id\": \"05-05-regional-presets\", \"title\": \"Regional Presets\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement Russia and Iran regional preset application with external DNS template fetch, routing and DNS writes, and fallback behavior.\", \"depends_on\": [\"05-02-dns-settings\"], \"deliverables\": [\"Preset manager using voya-net.\", \"Preset UI actions and confirmation flow.\", \"Tests for successful fetch, null fallback, routing write, DNS write, and simple DNS behavior.\"], \"acceptance\": [\"Preset apply fetches DNS templates through configured sources when available.\", \"Fallback enables custom DNS when network template data is unavailable.\"], \"evidence_to_capture\": [\"docs/verification/regional-presets.md.\"], \"verify_commands\": [\"cargo test -p voya-app preset --all-targets\", \"cargo test -p voya-net --all-targets\", \"pnpm typecheck\", \"test -f docs/verification/regional-presets.md\"], \"files_to_touch\": [\"crates/voya-app/**\", \"crates/voya-net/**\", \"src/features/options/**\", \"docs/verification/regional-presets.md\"], \"prompt_context\": [\"Regional presets depend on voya-net and are not a static local-only settings write.\"]}, {\"id\": \"05-06-advanced-routing-phase-gate\", \"title\": \"Advanced Routing Phase Gate\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Stabilize routing, DNS, TUN, groups, chains, and presets before service integrations.\", \"depends_on\": [\"05-04-policy-groups-chains-ui\", \"05-05-regional-presets\"], \"deliverables\": [\"docs/verification/m4-routing-dns-tun-gate.md.\", \"Fixes needed for advanced routing checks.\"], \"acceptance\": [\"Advanced generator and UI checks pass.\", \"Manual TUN OS smoke steps remain documented separately.\"], \"evidence_to_capture\": [\"M4 advanced routing gate report.\"], \"verify_commands\": [\"cargo test --workspace --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"pnpm bindings:check\", \"test -f docs/verification/m4-routing-dns-tun-gate.md\"], \"files_to_touch\": [\"docs/verification/m4-routing-dns-tun-gate.md\", \"crates/**\", \"src/**\"], \"prompt_context\": [\"Keep fixes scoped to this phase surface.\"]}]}, {\"id\": \"06-service-integrations\", \"title\": \"Clash, Speedtest, Updates\", \"goal\": \"Complete Clash API, speedtest, downloads, updates, ruleset, and geo acquisition workflows.\", \"depends_on\": [\"05-routing-dns-tun-groups\"], \"summary\": \"This phase adds operational services around a working proxy runtime.\", \"entry_criteria\": [\"Runtime, routing, DNS, TUN, and groups are functional.\"], \"exit_criteria\": [\"Clash, speedtest, downloads, updates, rulesets, and geo acquisition are implemented and tested.\"], \"risks\": [\"Network-dependent behavior can make tests flaky unless clients are injectable and fixture-driven.\"], \"batches\": [{\"id\": \"06-01-clash-api-ui\", \"title\": \"Clash API And UI\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement Clash REST and WebSocket clients plus proxies and connections screens.\", \"depends_on\": [], \"deliverables\": [\"voya-net Clash REST and WebSocket client.\", \"voya-app Clash manager and commands.\", \"Clash Proxies and Clash Connections tabs.\", \"Delay test, select active, connection monitor, close connection, rule-mode PATCH, and reload force behavior.\"], \"acceptance\": [\"Rule-mode switch uses HTTP PATCH on /configs.\", \"Reload uses /configs?force=true.\", \"WebSocket traffic and connection events update UI stores.\"], \"evidence_to_capture\": [\"docs/verification/clash.md.\"], \"verify_commands\": [\"cargo test -p voya-net clash --all-targets\", \"cargo test -p voya-app clash --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/clash.md\"], \"files_to_touch\": [\"crates/voya-net/**\", \"crates/voya-app/**\", \"src/features/clash/**\", \"docs/verification/clash.md\"], \"prompt_context\": [\"Use mocked HTTP and WebSocket clients in automated tests.\"]}, {\"id\": \"06-02-speedtest-udptest\", \"title\": \"Speedtest And UDP Tests\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Port all six speed actions, UDP associate channel, NTP, DNS, STUN, MCBE testers, cancellation, and UI result writing.\", \"depends_on\": [\"06-01-clash-api-ui\"], \"deliverables\": [\"voya-udptest crate implementation and tests.\", \"Speedtest manager and commands.\", \"UI actions and result display in server table.\", \"ProfileExItem delay, speed, message, and ipinfo updates.\"], \"acceptance\": [\"ESpeedActionType covers Tcping, Realping, UdpTest, Speedtest, Mixedtest, and FastRealping.\", \"Mixedtest combines realping, speedtest, and UDP as expected.\", \"Cancel stops active jobs.\"], \"evidence_to_capture\": [\"docs/verification/speedtest.md.\"], \"verify_commands\": [\"cargo test -p voya-udptest --all-targets\", \"cargo test -p voya-app speedtest --all-targets\", \"pnpm typecheck\", \"test -f docs/verification/speedtest.md\"], \"files_to_touch\": [\"crates/voya-udptest/**\", \"crates/voya-app/**\", \"src/features/profiles/**\", \"docs/verification/speedtest.md\"], \"prompt_context\": [\"Network-heavy tests should use local fixtures or mocked sockets where possible.\"]}, {\"id\": \"06-03-downloads-updates\", \"title\": \"Downloads And Updates\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement download service, app and core update checks, asset templating, pre-release toggle, and safe binary swap workflows.\", \"depends_on\": [\"06-02-speedtest-udptest\"], \"deliverables\": [\"voya-net download and GitHub release clients.\", \"Update manager for app, cores, geo, and srs.\", \"Check Update UI.\", \"Tests for OS and arch asset selection, fallback, and version parsing.\"], \"acceptance\": [\"Proxy-to-direct fallback is implemented.\", \"Core asset templating covers the supported core matrix and architectures.\", \"Download-on-first-run stance is preserved for GPL or AGPL cores.\"], \"evidence_to_capture\": [\"docs/verification/updates.md.\"], \"verify_commands\": [\"cargo test -p voya-net update --all-targets\", \"cargo test -p voya-app update --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/updates.md\"], \"files_to_touch\": [\"crates/voya-net/**\", \"crates/voya-app/**\", \"src/features/updates/**\", \"docs/verification/updates.md\"], \"prompt_context\": [\"Do not require live GitHub network access in unit tests; use fixture releases.\"]}, {\"id\": \"06-04-ruleset-geo\", \"title\": \"Ruleset And Geo Acquisition\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement acquisition, update, validation, and configuration integration for geo dat files and sing-box srs rulesets.\", \"depends_on\": [\"06-03-downloads-updates\"], \"deliverables\": [\"Ruleset and geo clients in voya-net.\", \"Manager commands and UI controls for sources.\", \"Integration with routing and DNS generation where needed.\", \"Tests using fixture archives and manifest data.\"], \"acceptance\": [\"Geo and ruleset updates can run through proxy-to-direct fallback.\", \"Generated configs reference acquired assets consistently.\"], \"evidence_to_capture\": [\"docs/verification/ruleset-geo.md.\"], \"verify_commands\": [\"cargo test -p voya-net ruleset --all-targets\", \"cargo test -p voya-app ruleset --all-targets\", \"cargo test -p voya-core ruleset --all-targets\", \"test -f docs/verification/ruleset-geo.md\"], \"files_to_touch\": [\"crates/voya-net/**\", \"crates/voya-app/**\", \"crates/voya-core/**\", \"src/features/options/**\", \"docs/verification/ruleset-geo.md\"], \"prompt_context\": [\"Keep acquisition separate from config generation; generation consumes resolved local asset paths.\"]}, {\"id\": \"06-05-services-phase-gate\", \"title\": \"Service Integrations Phase Gate\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Stabilize Clash, speedtest, updates, ruleset, geo, and related UI workflows.\", \"depends_on\": [\"06-04-ruleset-geo\"], \"deliverables\": [\"docs/verification/m5-services-gate.md.\", \"Fixes required for service integration checks.\"], \"acceptance\": [\"Automated service integration tests pass without live network dependency.\", \"Manual live-network smoke steps are documented.\"], \"evidence_to_capture\": [\"M5 services gate report.\"], \"verify_commands\": [\"cargo test --workspace --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"pnpm bindings:check\", \"test -f docs/verification/m5-services-gate.md\"], \"files_to_touch\": [\"docs/verification/m5-services-gate.md\", \"crates/**\", \"src/**\"], \"prompt_context\": [\"Do not hide real-network requirements in automated tests.\"]}]}, {\"id\": \"07-polish-backup-i18n\", \"title\": \"Backup, Integrations, I18n, Polish\", \"goal\": \"Complete backup, WebDAV, autostart, hotkeys, QR, i18n, theming, accessibility, performance, and smoke automation.\", \"depends_on\": [\"06-service-integrations\"], \"summary\": \"This phase closes user-facing breadth and quality gates before packaging.\", \"entry_criteria\": [\"Major runtime and service workflows are implemented.\"], \"exit_criteria\": [\"The UI and integration surface is complete, localized, accessible, and smoke-tested where automatable.\"], \"risks\": [\"Polish work can sprawl; each batch must stay tied to specific workflows and checks.\"], \"batches\": [{\"id\": \"07-01-backup-webdav\", \"title\": \"Backup, Restore, WebDAV\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement local backup, restore, WebDAV push and pull, zip handling, and Backup UI.\", \"depends_on\": [], \"deliverables\": [\"Backup manager and commands.\", \"WebDAV client using reqwest and quick-xml.\", \"Backup and Restore screen.\", \"Tests for local round trip and fixture WebDAV XML.\"], \"acceptance\": [\"Local backup restores into a clean temp app state in tests.\", \"WebDAV PROPFIND, upload, download, and delete behavior is fixture-tested.\"], \"evidence_to_capture\": [\"docs/verification/backup-webdav.md.\"], \"verify_commands\": [\"cargo test -p voya-net webdav --all-targets\", \"cargo test -p voya-app backup --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/backup-webdav.md\"], \"files_to_touch\": [\"crates/voya-net/**\", \"crates/voya-app/**\", \"src/features/backup/**\", \"docs/verification/backup-webdav.md\"], \"prompt_context\": [\"Avoid live WebDAV in tests; fixture the XML and HTTP responses.\"]}, {\"id\": \"07-02-autostart-hotkeys-qr\", \"title\": \"Autostart, Hotkeys, QR\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Implement per-OS autostart, global hotkeys, QR generation, QR scan frontend path, and related settings UI.\", \"depends_on\": [\"07-01-backup-webdav\"], \"deliverables\": [\"Autostart platform adapters and commands.\", \"Global hotkey registration for show window and four proxy mode actions.\", \"Backend QR generation command.\", \"Frontend QR generate and scan UI hooks.\", \"Tests with fake platform adapters.\"], \"acceptance\": [\"QR generation is backend; scanning remains frontend or platform scoped.\", \"Hotkey actions are represented by the five EGlobalHotkey actions.\", \"Autostart artifacts are documented per OS.\"], \"evidence_to_capture\": [\"docs/verification/autostart-hotkeys-qr.md.\"], \"verify_commands\": [\"cargo test -p voya-platform autostart --all-targets\", \"cargo test -p voya-app hotkey --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"test -f docs/verification/autostart-hotkeys-qr.md\"], \"files_to_touch\": [\"crates/voya-platform/**\", \"crates/voya-app/**\", \"src/features/options/**\", \"src/features/qr/**\", \"docs/verification/autostart-hotkeys-qr.md\"], \"prompt_context\": [\"Prefer Tauri or platform plugins where faithful, but keep behavior aligned with the reference.\"]}, {\"id\": \"07-03-i18n-resx-import\", \"title\": \"I18n Resource Import\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Convert reference resources into i18next locale files, wire missing-key checks, and verify RTL behavior.\", \"depends_on\": [\"07-02-autostart-hotkeys-qr\"], \"deliverables\": [\"Locale files for 8 languages including fa RTL.\", \"Resource conversion script or documented import process.\", \"Missing-key tests.\", \"UI language switch integration.\"], \"acceptance\": [\"No missing i18n keys in tests.\", \"RTL layout can be toggled and is covered by tests or smoke docs.\"], \"evidence_to_capture\": [\"docs/verification/i18n.md.\"], \"verify_commands\": [\"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"test -f docs/verification/i18n.md\"], \"files_to_touch\": [\"src/locales/**\", \"src/i18n/**\", \"scripts/**\", \"docs/verification/i18n.md\"], \"prompt_context\": [\"Use v2rayN resource files as references but do not edit them.\"]}, {\"id\": \"07-04-theme-a11y-perf\", \"title\": \"Theme, Accessibility, Performance\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Polish theme tokens, accent and font settings, accessibility, table performance, modal ergonomics, and UI consistency.\", \"depends_on\": [\"07-03-i18n-resx-import\"], \"deliverables\": [\"Theme and font settings fully persisted and applied.\", \"Accessibility pass for dialogs, menus, table, status controls, and forms.\", \"Measured or tested large-table performance path.\", \"Visual consistency fixes across screens.\"], \"acceptance\": [\"No one-note palette dominates the UI.\", \"Text does not overflow compact controls at desktop and mobile-ish widths.\", \"500 server rows with 1 Hz updates remain responsive in the perf harness or test.\"], \"evidence_to_capture\": [\"docs/verification/ui-polish.md.\"], \"verify_commands\": [\"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"test -f docs/verification/ui-polish.md\"], \"files_to_touch\": [\"src/**\", \"docs/verification/ui-polish.md\"], \"prompt_context\": [\"Follow the frontend design guidance from the global instructions and existing app shell conventions.\"]}, {\"id\": \"07-05-playwright-tauri-smoke\", \"title\": \"Playwright And Tauri Smoke\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Add automated smoke coverage for key app flows through Playwright and tauri-driver where locally feasible.\", \"depends_on\": [\"07-04-theme-a11y-perf\"], \"deliverables\": [\"Playwright setup for frontend flows.\", \"Tauri driver smoke setup or documented platform limitations.\", \"Smoke tests for app shell, profile add, import fixture, connect fake, routing, DNS, and dialogs.\", \"Manual smoke matrix for OS-only flows.\"], \"acceptance\": [\"Frontend smoke tests run non-interactively.\", \"Tauri-driver gaps are documented with exact manual checks.\"], \"evidence_to_capture\": [\"docs/verification/cross-platform-smoke.md.\"], \"verify_commands\": [\"pnpm test -- --run\", \"pnpm typecheck\", \"test -f docs/verification/cross-platform-smoke.md\"], \"files_to_touch\": [\"tests/**\", \"e2e/**\", \"playwright.config.*\", \"docs/verification/cross-platform-smoke.md\"], \"prompt_context\": [\"Do not require real proxies or real OS proxy changes in automated smoke tests.\"]}, {\"id\": \"07-06-polish-phase-gate\", \"title\": \"Polish Phase Gate\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Stabilize backup, integrations, i18n, theme, accessibility, and smoke checks before packaging.\", \"depends_on\": [\"07-05-playwright-tauri-smoke\"], \"deliverables\": [\"docs/verification/m6-polish-gate.md.\", \"Fixes required for UI and integration quality gates.\"], \"acceptance\": [\"Workspace checks pass.\", \"Manual OS smoke matrix is ready for release candidates.\"], \"evidence_to_capture\": [\"M6 polish gate report.\"], \"verify_commands\": [\"cargo test --workspace --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"pnpm bindings:check\", \"test -f docs/verification/m6-polish-gate.md\"], \"files_to_touch\": [\"docs/verification/m6-polish-gate.md\", \"crates/**\", \"src/**\", \"tests/**\"], \"prompt_context\": [\"Keep fixes focused on release-readiness gaps from this phase.\"]}]}, {\"id\": \"08-packaging-release\", \"title\": \"Packaging And Release\", \"goal\": \"Prepare package builds, updater metadata, release CI, runbooks, and final evidence for public beta.\", \"depends_on\": [\"07-polish-backup-i18n\"], \"summary\": \"This phase makes the app shippable while keeping credentials and real publication outside the runner.\", \"entry_criteria\": [\"Feature-complete app and smoke checks are available.\"], \"exit_criteria\": [\"Debug or unsigned packages build, release workflows are configured, and manual signing or publication steps are documented.\"], \"risks\": [\"Packaging can depend on credentials or OS environments unavailable to the runner.\"], \"batches\": [{\"id\": \"08-01-tauri-packaging\", \"title\": \"Tauri Packaging Config\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Configure Tauri bundle targets, updater settings, sidecar strategy, attribution, and first-run core download posture.\", \"depends_on\": [], \"deliverables\": [\"Tauri bundle configuration for macOS, Windows, and Linux targets.\", \"Updater configuration with placeholders for keys and channels.\", \"Attribution and licenses screen or document.\", \"First-run core download flow documentation.\"], \"acceptance\": [\"Debug package build can run locally without signing credentials.\", \"GPL or AGPL cores are not bundled by default.\"], \"evidence_to_capture\": [\"docs/release/packaging.md.\"], \"verify_commands\": [\"pnpm tauri:build --debug\", \"test -f docs/release/packaging.md\"], \"files_to_touch\": [\"src-tauri/**\", \"src/features/about/**\", \"docs/release/packaging.md\", \"package.json\"], \"prompt_context\": [\"If platform prerequisites are missing, document the exact failure and keep config changes deterministic.\"]}, {\"id\": \"08-02-release-ci\", \"title\": \"Release CI Workflows\", \"kind\": \"code\", \"execution\": \"codex\", \"goal\": \"Add release workflows for tests, package builds, updater metadata, checksums, and artifact upload without embedding secrets.\", \"depends_on\": [\"08-01-tauri-packaging\"], \"deliverables\": [\".github/workflows/release.yml.\", \"Artifact naming and checksum scripts.\", \"Updater latest.json generation path with secret placeholders.\", \"Docs for required CI secrets.\"], \"acceptance\": [\"Release workflow is triggerable manually and does not require secrets for dry-run validation.\", \"Secrets are referenced by name but never committed.\"], \"evidence_to_capture\": [\"docs/release/ci-secrets.md.\"], \"verify_commands\": [\"test -f .github/workflows/release.yml\", \"test -f docs/release/ci-secrets.md\", \"pnpm typecheck\"], \"files_to_touch\": [\".github/workflows/release.yml\", \"scripts/**\", \"docs/release/ci-secrets.md\"], \"prompt_context\": [\"Keep real publishing credentials outside the repo.\"]}, {\"id\": \"08-03-release-runbooks\", \"title\": \"Release Runbooks\", \"kind\": \"docs\", \"execution\": \"codex\", \"goal\": \"Write manual runbooks for signing, notarization, updater keys, OS smoke testing, rollback, and beta publication.\", \"depends_on\": [\"08-02-release-ci\"], \"deliverables\": [\"docs/release/runbook.md.\", \"docs/release/signing-notarization.md.\", \"docs/release/os-smoke-matrix.md.\", \"docs/release/rollback.md.\"], \"acceptance\": [\"Every manual release checkpoint has owner, system, verification, and rollback notes.\", \"The runbook separates local debug packaging from public beta publication.\"], \"evidence_to_capture\": [\"Release docs exist and link to verification evidence.\"], \"verify_commands\": [\"test -f docs/release/runbook.md\", \"test -f docs/release/signing-notarization.md\", \"test -f docs/release/os-smoke-matrix.md\", \"test -f docs/release/rollback.md\"], \"files_to_touch\": [\"docs/release/**\"], \"prompt_context\": [\"Do not invent secret values; document the names and how owners supply them.\"]}, {\"id\": \"08-04-final-regression-evidence\", \"title\": \"Final Regression Evidence\", \"kind\": \"verification\", \"execution\": \"codex\", \"goal\": \"Run the final automated regression suite, collect evidence, and document remaining external release prerequisites.\", \"depends_on\": [\"08-03-release-runbooks\"], \"deliverables\": [\"docs/verification/m7-public-beta-gate.md.\", \"Updated README with build, test, dev, and release commands.\", \"Fixes required for final automated checks.\"], \"acceptance\": [\"All global verification commands pass or documented external prerequisites explain any skipped package-only checks.\", \"Remaining work is only external credentials, OS machines, or publication actions.\"], \"evidence_to_capture\": [\"M7 public beta gate report.\"], \"verify_commands\": [\"cargo test --workspace --all-targets\", \"pnpm typecheck\", \"pnpm test -- --run\", \"pnpm lint\", \"pnpm bindings:check\", \"test -f docs/verification/m7-public-beta-gate.md\"], \"files_to_touch\": [\"docs/verification/m7-public-beta-gate.md\", \"README.md\", \"crates/**\", \"src/**\", \"src-tauri/**\", \".github/workflows/**\"], \"prompt_context\": [\"This is a closeout batch; do not add new feature scope unless required to satisfy an existing acceptance criterion.\"]}]}]}"
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
    env = None
    stdin = None
    if capture_output:
        env = os.environ.copy()
        env.setdefault("CI", "1")
        stdin = subprocess.DEVNULL
    return subprocess.run(
        cmd,
        shell=True,
        cwd=cwd,
        check=check,
        capture_output=capture_output,
        text=capture_output,
        stdin=stdin,
        env=env,
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
