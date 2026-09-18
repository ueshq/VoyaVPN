# macOS VPN Acceptance

macOS captures traffic only through the PacketTunnel VPN; there is no system
proxy mode to accept. This flow changes VPN connections. Run it only in an explicitly approved test window. Do not interrupt
an existing VPN merely to run automated repository checks.

## Automated checks (no system network writes)

Run `pnpm run verify:local`. On macOS its Rust-test step also compiles and runs
`scripts/native/macos/test-bridge.mjs`: it injects statuses and a monotonic clock
into the same waiter as the native bridge. Its ten reconnect cycles are
simulations, not signed-package or real-traffic acceptance.

## Signed channel matrix

Repeat the following for the local Apple Development `.appex`, the notarized
Developer ID System Extension, and the App Store/TestFlight sandbox package.
Record macOS version, app revision, channel, architecture, signing identity,
provider bundle/path, observations and timestamps. Retain App Sandbox and the
channel's existing NetworkExtension entitlements. Use only the installed copy.

Before testing, record HTTP/HTTPS/SOCKS/PAC settings for all network services so
the tester can restore their original settings manually. Do not disclose proxy
credentials in evidence. Perform external `pnpm native:macos:ne:doctor` checks
when registration evidence is needed; sandbox discovery-unavailable output in
the app is not proof of a registration defect.

## VPN lifecycle

1. Connect/disconnect ten consecutive times. Each success requires agreement
   between Home, `scutil --nc list`, current NetworkExtension state and actual
   traffic/exit-IP checks from a browser and a terminal. A provider log alone
   is insufficient.
2. During startup confirm initial Disconnected samples do not cause a false
   failure. Confirm the busy indicator settles after success or failure.
3. With a deliberately invalid test-provider configuration, verify startup
   failure triggers cleanup. No tunnel may remain after reported cleanup
   success. Preserve the original error and any cleanup error.
4. If stop times out, Home must show pending cleanup and offer Retry disconnect.
   Retry must address the retained session. Do not reconnect until ownership
   has been resolved. Check delayed provider starts during cleanup as well.

## No system proxy mode

1. Settings -> Network shows no Traffic capture choice, no System proxy group
   and no Per-app proxy shortcut; the Rules page shows no per-app card.
2. Home shows only the Rule / Global traffic mode, and connecting always starts
   the VPN. The first connection asks macOS to add a VPN configuration.
3. Quitting through the tray, Cmd+Q or the window never shows a system proxy
   reminder.

## Cleanup and evidence

After copying, launching or testing any `.app` with the production PacketTunnel
appex, quit the test app and follow the existing NE hygiene flow:
`pnpm native:macos:ne:doctor --fix` (`--app <path>` for a non-default bundle,
`--dev` for the repository release bundle). Do this before deleting entitlement
fixtures. Do not add the mutating doctor command to CI or broad verification.

Attach per-channel results and record any unexecuted cases explicitly. A green
unit/mock suite does not substitute for this signed-package acceptance matrix.
