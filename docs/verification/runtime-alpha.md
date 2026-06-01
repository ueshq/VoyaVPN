# Runtime Alpha Verification

Batch: `04-03-connect-logs-ui`

## Scope

- Added runtime connect, disconnect, restart, and status commands.
- Connecting the active profile builds the existing core config context, writes `binConfigs/config.json` plus `binConfigs/configPre.json` when a pre-socks context exists, resolves the configured core executable, and starts the supervisor actor.
- Disconnect stops the supervisor and removes generated runtime config files.
- Process stdout and stderr are forwarded through transient `logLine` events.
- Core state events now include active profile id, main/pre PIDs, and running core type.
- The React status bar exposes connect, disconnect, restart, sudo collection, core state, PID, proxy, TUN, and speed display.
- The Logs tab renders transient log events and supports clearing the in-memory log buffer.
- The sudo modal calls the existing begin/submit/clear collection primitive and keeps the password lifecycle in memory.

## Deterministic Evidence

Automated tests use generated config files and fake process runners; they do not require a real server or elevated host mutation.

```text
$ cargo test -p voya-app supervisor --all-targets
6 passed

$ cargo test --workspace --all-targets
93 passed

$ pnpm typecheck
passed

$ pnpm test -- --run
2 files passed, 7 tests passed

$ test -f docs/verification/runtime-alpha.md
passed
```

## Manual Smoke

Real traffic flow remains a manual alpha smoke step because it requires a downloaded core binary, a real server, and host proxy or routing changes:

1. Install or download the selected Xray or sing-box core into the runtime `bin/<core>/` directory.
2. Add or import a real profile and make it active.
3. Run `pnpm tauri dev`.
4. Click Connect in the status bar.
5. Confirm logs appear in the Logs tab, the status bar shows connected state and PID, and traffic flows through the local SOCKS/mixed inbound.
6. Click Disconnect and confirm generated runtime configs are removed and no core process remains.

System proxy and TUN route mutation are intentionally left for the platform smoke matrix because they require privileged host changes.
