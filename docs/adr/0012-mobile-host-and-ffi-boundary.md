# ADR 0012: Mobile Host and the FFI Boundary

Status: Accepted

Date: 2026-09-20

## Context

`apps/mobile` is a bare React Native app that has to reach the same backend the
desktop does: the same profiles, the same generated sing-box config, the same
routing and DNS decisions. That backend is `voya-app`, and it already has no
idea Tauri exists — `AppServices::connect(&Path, AppPaths)` takes a directory
and a paths struct, its eight event sinks are `Arc<dyn Trait>` carrying codes
rather than English strings, and the invalidation scopes it publishes live in
`voya-app::invalidation`.

So the question was never "how do we port the backend to mobile". It was: what
is the *second host* — the thing that plays the part `apps/desktop/src-tauri`
plays — and how thin can its boundary be.

Three prior facts shaped the answer:

- ADR 0005's `NativeTunController` already abstracts "run the core without
  spawning a child process", and `SupervisorActor::plan_start` branches on
  `backend.is_native()` *before* tearing the old core down. A phone's tunnel
  provider is the same shape as the macOS PacketTunnel.
- `ClashApiAccess { port, secret }` is a value carried from config generation
  to `ClashApiEndpoint`, not a global. Loopback on iOS is per device, not per
  process, so the app process can read the Clash API inside the extension
  process exactly as macOS does today.
- The frontend already reaches the backend through one seam. `@voya/contracts`
  describes every command and event; `@voya/client`'s `setVoyaCommands` is the
  only thing a feature ever calls.

## Decision

### A second host, not a refactor

`crates/voya-mobile-ffi` is a host in the same sense the Tauri shell is: it
owns a tokio runtime, calls `AppServices::connect`, injects its dependencies,
and turns the sinks into events. It depends on `voya-app`, `voya-contracts` and
`voya-platform` and never on `voya-core` or `voya-db`, which is the discipline
the shell already follows and which `pnpm run check:architecture` enforces for
both.

Nothing in `voya-app` learns that a phone exists beyond the `TargetOs::Ios` and
`TargetOs::Android` variants and the `TunBackend` they select.

### uniffi carries an envelope, not a model

The uniffi surface is deliberately tiny:

```
VoyaApp::new(data_dir, cache_dir, locale, events, tunnel, probe)
VoyaApp::invoke(command: String, args_json: String) async -> Result<String, String>
VoyaApp::shutdown()

callback EventListener { on_event(channel: String, payload_json: String) }
callback TunnelHost    { start(runtime_config_json, include_all_networks) / stop() / status() }
callback ProbeCoreHost { start(config_json) / stop() }
```

Every DTO crosses as serde JSON, in **the same wire shape Tauri uses** — named
argument objects, camelCase fields, the `{ kind, payload }` tagging the event
enums already have. Two consequences follow, and both are the point:

- `@voya/contracts` is reused at zero cost. The mobile transport is ~30 lines
  over `VOYA_COMMAND_WIRE` (the generated table of each command's wire name and
  argument names) rather than 76 hand-written wrappers, and a command added in
  Rust reaches it without a line of TypeScript.
- `voya-contracts` gains no uniffi dependency, and uniffi gains no say over the
  contract. Modelling all 76 commands and every DTO in UDL would have made the
  IPC contract exist twice, with a generator on each side and nothing checking
  they agree.

The cost is that the boundary is not type-checked by uniffi. That is bought
back by two tests rather than by types: a coverage test that reads the
generated `packages/contracts/commands.json` and fails unless every command has
either a dispatcher or an explicit entry on `UNSUPPORTED_ON_MOBILE`, and an
event-shape test that reads `packages/contracts/events.json` and fails unless
the host's own event enums carry exactly the channels and `kind` values the
generated contract declares.

### Why the event enums are declared twice

`InvalidateEvent`, `TransientStreamEvent` and `AppEvent` carry
`tauri_specta::Event` and therefore live in the shell. Moving the payloads into
`voya-contracts` and leaving wrappers behind does not work: `Event` is a
foreign trait and the payloads would be foreign types, so the shell cannot
implement it for them, and a newtype changes the TypeScript specta emits.

The host therefore declares its own three enums with the same serde attributes,
and `events.json` is what stops them drifting — generated from the same
`bindings.ts` the TypeScript comes from, so the check runs against the real
contract rather than against a copy of it.

### Why not Tauri 2's mobile support

Tauri 2 does run on iOS and Android, and it would have given the IPC layer for
free. It also brings a WebView, its own Rust-side app lifecycle, and its
plugin and permission model — and the app would still need a
NetworkExtension provider and a `VpnService` written by hand, because Tauri has
nothing to say about either. React Native gives the native shell we need for
those, and this crate is the part Tauri would have contributed, at about a
thousand lines.

## Consequences

- The mobile host and the Tauri shell are peers. A decision that belongs to
  both — "does this commit need the core restarted", "is this IPC text
  acceptable" — moves down into `voya-app` rather than being copied, and
  anything needing more than about twenty lines in both hosts is a signal that
  it has not moved yet.
- A backend command is available on mobile the moment it has a dispatcher, and
  is *known to be unavailable* otherwise. There is no silent third state.
- The wire format is JSON on both hosts, so a payload that is expensive to
  serialize is expensive on both. Nothing here streams; the transient channels
  are already batched by the sinks (log lines per window, statistics per
  second, speedtest results per probe).
